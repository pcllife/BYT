use std::{
    collections::{HashMap, HashSet},
    ptr::null_mut,
};
use windows_sys::Win32::System::Performance::*;

pub struct G {
    q: PDH_HQUERY,
    c: PDH_HCOUNTER,
    b: Vec<u64>,
    warm: bool,
    pub err: u32,
}
pub type M = HashMap<u32, Vec<(String, f64)>>;
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
        let mut out = M::new();
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
            let mut n = 0;
            unsafe {
                while *it.szName.add(n) != 0 && n < 4096 {
                    n += 1;
                }
            }
            let nm = unsafe { String::from_utf16_lossy(std::slice::from_raw_parts(it.szName, n)) };
            let Some(rest) = nm.strip_prefix("pid_") else {
                continue;
            };
            let Some((pid, key)) = rest.split_once('_') else {
                continue;
            };
            let Ok(pid) = pid.parse::<u32>() else {
                continue;
            };
            if !ids.contains_key(&pid) {
                continue;
            }
            let v = unsafe { it.FmtValue.Anonymous.doubleValue };
            if v.is_finite() {
                out.entry(pid)
                    .or_default()
                    .push((key.to_string(), v.max(0.)));
            }
        }
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
pub fn sum(m: &M, ids: &HashSet<u32>) -> f64 {
    let mut es: HashMap<&str, f64> = HashMap::new();
    for id in ids {
        if let Some(v) = m.get(id) {
            for (k, n) in v {
                *es.entry(k).or_default() += n;
            }
        }
    }
    es.values().copied().fold(0., f64::max).clamp(0., 100.)
}
#[cfg(test)]
mod t {
    #[test]
    fn eng() {
        let m = std::collections::HashMap::from([
            (1, vec![("a".into(), 30.), ("b".into(), 10.)]),
            (2, vec![("a".into(), 40.), ("b".into(), 90.)]),
        ]);
        assert_eq!(
            super::sum(&m, &std::collections::HashSet::from([1, 2])),
            100.
        );
    }
}
