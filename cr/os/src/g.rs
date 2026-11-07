use byt_k::a;
use std::{collections::HashMap, ptr::null_mut};
use windows_sys::Win32::System::Performance::*;

pub struct G {
    q: PDH_HQUERY,
    c: PDH_HCOUNTER,
    b: Vec<u64>,
    warm: bool,
    pub err: u32,
}
pub type M = HashMap<i64, f64>;
impl Default for G {
    fn default() -> Self {
        Self::new()
    }
}

impl G {
    pub fn new() -> Self {
        Self {
            q: null_mut(),
            c: null_mut(),
            b: Vec::new(),
            warm: false,
            err: 0,
        }
    }
    pub fn read(&mut self, ids: &HashMap<u32, i64>) -> Option<M> {
        if ids.is_empty() {
            return None;
        }
        if self.q.is_null() {
            self.err = unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut self.q) };
            if self.err != 0 {
                return None;
            }
            let s: Vec<u16> = r"\GPU Engine(*)\Utilization Percentage"
                .encode_utf16()
                .chain(Some(0))
                .collect();
            self.err = unsafe { PdhAddEnglishCounterW(self.q, s.as_ptr(), 0, &mut self.c) };
            if self.err != 0 {
                unsafe { PdhCloseQuery(self.q) };
                self.q = null_mut();
                return None;
            }
        }
        self.err = unsafe { PdhCollectQueryData(self.q) };
        if self.err != 0 {
            return None;
        }
        if !self.warm {
            self.warm = true;
            return None;
        }
        let (mut sz, mut ct) = (0u32, 0u32);
        let e = unsafe {
            PdhGetFormattedCounterArrayW(
                self.c,
                PDH_FMT_DOUBLE | 0x8000,
                &mut sz,
                &mut ct,
                null_mut(),
            )
        };
        if e != PDH_MORE_DATA && e != 0 {
            self.err = e;
            return None;
        }
        if sz == 0 {
            return Some(HashMap::new());
        }
        self.b.resize((sz as usize).div_ceil(8), 0);
        self.err = unsafe {
            PdhGetFormattedCounterArrayW(
                self.c,
                PDH_FMT_DOUBLE | 0x8000,
                &mut sz,
                &mut ct,
                self.b.as_mut_ptr().cast(),
            )
        };
        if self.err != 0 {
            return None;
        }
        let mut es: HashMap<(i64, &[u16]), f64> = HashMap::new();
        let items = unsafe {
            std::slice::from_raw_parts(
                self.b.as_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>(),
                ct as usize,
            )
        };
        for it in items {
            if it.FmtValue.CStatus > 1 || it.szName.is_null() {
                continue;
            }
            let start = it.szName as usize;
            let end = self.b.as_ptr() as usize + sz as usize;
            if start < self.b.as_ptr() as usize || start >= end || !start.is_multiple_of(2) {
                continue;
            }
            let name = unsafe { std::slice::from_raw_parts(it.szName, (end - start) / 2) };
            let Some((pid, k)) = a::i(name) else {
                continue;
            };
            let Some(app) = ids.get(&pid).copied() else {
                continue;
            };
            let key = &name[k..];
            let n = a::w(key);
            let v = unsafe { it.FmtValue.Anonymous.doubleValue };
            if v.is_finite() {
                *es.entry((app, &key[..n])).or_default() += v.max(0.);
            }
        }
        let out = agg(es);

        Some(out)
    }
}
impl Drop for G {
    fn drop(&mut self) {
        if !self.q.is_null() {
            unsafe { PdhCloseQuery(self.q) };
        }
    }
}
fn agg(es: HashMap<(i64, &[u16]), f64>) -> M {
    let mut out = M::new();
    for ((id, _), v) in es {
        let n = out.entry(id).or_default();
        *n = n.max(v).clamp(0., 100.);
    }
    out
}
#[cfg(test)]
mod t {
    #[test]
    fn eng() {
        let a: Vec<u16> = "engine_a".encode_utf16().collect();
        let b: Vec<u16> = "engine_b".encode_utf16().collect();
        let mut es = std::collections::HashMap::new();
        for (id, k, v) in [
            (1, a.as_slice(), 30.),
            (1, a.as_slice(), 40.),
            (1, b.as_slice(), 90.),
            (2, a.as_slice(), 5.),
        ] {
            *es.entry((id, k)).or_default() += v;
        }
        let m = super::agg(es);
        assert_eq!(m[&1], 90.);
        assert_eq!(m[&2], 5.);
    }
}
