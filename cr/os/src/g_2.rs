use super::M;
use byt_k::a;
use std::{
    collections::HashMap,
    mem::size_of,
    ptr::{null, null_mut},
};
use windows_sys::{
    Win32::{
        Foundation::*,
        System::{Performance::*, Registry::*},
    },
    core::{GUID, w},
};
#[doc = "# v2 - read the same gpu counters without the name cache"]
pub struct N {
    h: HANDLE,
    b: Vec<u64>,
    old: HashMap<(u32, u32), u64>,
    tm: i64,
}
fn at<T: Copy>(b: &[u8], i: usize) -> Result<T, u32> {
    if i.checked_add(size_of::<T>()).is_none_or(|n| n > b.len()) {
        return Err(ERROR_INVALID_DATA);
    }
    Ok(unsafe { b.as_ptr().add(i).cast::<T>().read_unaligned() })
}
fn span(end: usize, i: usize, n: usize) -> Result<usize, u32> {
    let x = i.checked_add(n).ok_or(ERROR_INVALID_DATA)?;
    if x > end {
        return Err(ERROR_INVALID_DATA);
    }
    Ok(x)
}
impl N {
    pub fn new() -> Option<Self> {
        unsafe {
            let mut v = [0u16; 32];
            let mut sz = (v.len() * 2) as u32;
            if RegGetValueW(
                HKEY_LOCAL_MACHINE,
                w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"),
                w!("CurrentBuildNumber"),
                RRF_RT_REG_SZ,
                null_mut(),
                v.as_mut_ptr().cast(),
                &mut sz,
            ) != 0
            {
                return None;
            }
            let s = String::from_utf16_lossy(&v[..a::w(&v)]);
            if s.parse::<u32>().ok()? < 17134 {
                return None;
            }
            let mut h = null_mut();
            if PerfOpenQueryHandle(null(), &mut h) != 0 {
                return None;
            }
            let n = Self {
                h,
                b: vec![0; 1024],
                old: HashMap::new(),
                tm: 0,
            };
            let mut b = [0u64; 6];
            let p = b.as_mut_ptr().cast::<PERF_COUNTER_IDENTIFIER>();
            (*p).Size = size_of_val(&b) as u32;
            (*p).CounterSetGuid = GUID::from_u128(0x978c167d_4764_4d9c_9824_14747351dc81);
            (*p).CounterId = 1;
            (*p).InstanceId = u32::MAX;
            let name = p
                .cast::<u8>()
                .add(size_of::<PERF_COUNTER_IDENTIFIER>())
                .cast::<u16>();
            name.write(42);
            name.add(1).write(0);
            if PerfAddCounters(h, p, size_of_val(&b) as u32) != 0 || (*p).Status != 0 {
                return None;
            }
            Some(n)
        }
    }
    pub fn read(&mut self, ids: &HashMap<u32, i64>) -> Result<Option<M>, u32> {
        let mut actual = 0;
        let mut ok = false;
        for _ in 0..4 {
            let r = unsafe {
                PerfQueryCounterData(
                    self.h,
                    self.b.as_mut_ptr().cast(),
                    (self.b.len() * 8) as u32,
                    &mut actual,
                )
            };
            if r == 0 {
                ok = true;
                break;
            }
            if r != ERROR_NOT_ENOUGH_MEMORY
                && r != ERROR_MORE_DATA
                && r != ERROR_INSUFFICIENT_BUFFER
            {
                return Err(r);
            }
            if actual == 0 || actual > 64 * 1024 * 1024 {
                return Err(ERROR_INVALID_DATA);
            }
            self.b.resize((actual as usize).div_ceil(8), 0);
        }
        if !ok {
            return Err(ERROR_MORE_DATA);
        }
        if actual as usize > self.b.len() * 8 {
            return Err(ERROR_INVALID_DATA);
        }
        let b =
            unsafe { std::slice::from_raw_parts(self.b.as_ptr().cast::<u8>(), actual as usize) };
        let h: PERF_DATA_HEADER = at(b, 0)?;
        if h.dwTotalSize as usize > b.len() || h.dwNumCounters != 1 || h.PerfTime100NSec <= 0 {
            return Err(ERROR_INVALID_DATA);
        }
        let start = size_of::<PERF_DATA_HEADER>();
        let ch: PERF_COUNTER_HEADER = at(b, start)?;
        if ch.dwStatus != 0 {
            return Err(ch.dwStatus);
        }
        if ch.dwType != PERF_MULTIPLE_INSTANCES {
            return Err(ERROR_NOT_SUPPORTED);
        }
        let end = span(h.dwTotalSize as usize, start, ch.dwSize as usize)?;
        let mi = start + size_of::<PERF_COUNTER_HEADER>();
        let m: PERF_MULTI_INSTANCES = at(b, mi)?;
        let mut off = mi + size_of::<PERF_MULTI_INSTANCES>();
        if off > end || m.dwInstances as usize > (end - off) / 24 {
            return Err(ERROR_INVALID_DATA);
        }
        let dt = h.PerfTime100NSec.saturating_sub(self.tm);
        let warm = self.tm != 0;
        let mut next = HashMap::new();
        let mut es: HashMap<(i64, &[u16]), f64> = HashMap::new();
        for _ in 0..m.dwInstances {
            if !off.is_multiple_of(2) {
                return Err(ERROR_INVALID_DATA);
            }
            span(end, off, size_of::<PERF_INSTANCE_HEADER>())?;
            let ih: PERF_INSTANCE_HEADER = at(b, off)?;
            if (ih.Size as usize) < size_of::<PERF_INSTANCE_HEADER>() || !ih.Size.is_multiple_of(2)
            {
                return Err(ERROR_INVALID_DATA);
            }
            let data = span(end, off, ih.Size as usize)?;
            let cd: PERF_COUNTER_DATA = at(b, data)?;
            if cd.dwDataSize < 8 || cd.dwSize < 16 {
                return Err(ERROR_INVALID_DATA);
            }
            let finish = span(end, data, cd.dwSize as usize)?;
            let value: u64 = at(b, data + size_of::<PERF_COUNTER_DATA>())?;
            let n0 = off + size_of::<PERF_INSTANCE_HEADER>();
            let name = unsafe {
                std::slice::from_raw_parts(b.as_ptr().add(n0).cast::<u16>(), (data - n0) / 2)
            };
            if let Some((pid, k)) = a::i(name)
                && let Some(app) = ids.get(&pid)
            {
                let key = (pid, ih.InstanceId);
                if let Some(old) = self.old.get(&key)
                    && dt > 0
                {
                    let v = value.saturating_sub(*old) as f64 / dt as f64 * 100.;
                    let tail = &name[k..];
                    let n = a::w(tail);
                    *es.entry((*app, &tail[..n])).or_default() += v;
                }
                next.insert(key, value);
            }
            off = finish;
        }
        self.old = next;
        self.tm = h.PerfTime100NSec;
        let mut out = M::new();
        for ((id, _), v) in es {
            let t = out.entry(id).or_default();
            *t = t.max(v).clamp(0., 100.);
        }
        Ok(warm.then_some(out))
    }
}
impl Drop for N {
    fn drop(&mut self) {
        unsafe {
            PerfCloseQueryHandle(self.h);
        }
    }
}
#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn bounds() {
        assert!(super::at::<u64>(&[0; 7], 0).is_err());
        assert!(super::span(9, 8, 2).is_err());
        assert!(super::span(9, usize::MAX, 1).is_err());
    }
    #[test]
    #[ignore = "live gpu comparison"]
    fn compare() {
        use crate::p::Q;
        use windows_sys::Win32::System::Diagnostics::ToolHelp::*;
        let sn = Q(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) });
        let mut e = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut ids = HashMap::new();
        let mut ok = unsafe { Process32FirstW(sn.0, &mut e) };
        while ok != 0 {
            let name = String::from_utf16_lossy(&e.szExeFile[..a::w(&e.szExeFile)]);
            if name.eq_ignore_ascii_case("dwm.exe")
                || name.eq_ignore_ascii_case("idea64.exe")
                || name.eq_ignore_ascii_case("chrome.exe")
            {
                ids.insert(e.th32ProcessID, e.th32ProcessID as i64);
            }
            ok = unsafe { Process32NextW(sn.0, &mut e) };
        }
        let mut n = super::N::new().expect("v2 init");
        let mut p = crate::g::pd::G::new();
        let _ = n.read(&ids).unwrap();
        let _ = p.read(&ids);
        for i in 0..4 {
            if i >= 2 {
                for id in ids.values_mut() {
                    *id = 1;
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
            let nv = n.read(&ids).unwrap().unwrap();
            let pv = p.read(&ids).unwrap();
            let na = nv.values().copied().fold(0., f64::max);
            let pa = pv.values().copied().fold(0., f64::max);
            eprintln!("V2={na:.4}% PDH={pa:.4}%");
            assert!((na - pa).abs() < 2.);
            if pa > 0.1 {
                assert!(na > 0.);
            }
        }
    }
}
