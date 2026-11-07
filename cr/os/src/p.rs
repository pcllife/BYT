use byt_k::{self as d, A, a};
use std::{collections::HashMap, mem::size_of, path::Path};
use windows_sys::Win32::{
    Foundation::*,
    System::{Diagnostics::ToolHelp::*, ProcessStatus::*, Threading::*},
};

pub struct Q(pub HANDLE);
impl Drop for Q {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}
struct E {
    h: Q,
    id: i64,
    tm: u64,
    born: u64,
}
#[derive(Default)]
pub struct V {
    pub pc: usize,
    pub bad: usize,
    pub ct: u64,
    pub ram: u64,
    pub ok: usize,
}
pub struct P {
    es: HashMap<u32, E>,
    pub ids: HashMap<u32, i64>,
    pub cores: f64,
}
fn ft(t: FILETIME) -> u64 {
    ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64
}
fn times(h: HANDLE) -> Option<(u64, u64)> {
    let (mut b, mut e, mut k, mut u) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    (unsafe { GetProcessTimes(h, &mut b, &mut e, &mut k, &mut u) } != 0)
        .then(|| (a::a(ft(k), ft(u)), ft(b)))
}
fn handle(pid: u32) -> Option<Q> {
    let mut h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, 0, pid) };
    if h.is_null() {
        h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    }
    (!h.is_null()).then_some(Q(h))
}
impl Default for P {
    fn default() -> Self {
        Self::new()
    }
}

impl P {
    pub fn new() -> Self {
        Self {
            es: HashMap::new(),
            ids: HashMap::new(),
            cores: std::thread::available_parallelism().map_or(1., |n| n.get() as f64),
        }
    }
    pub fn read(&mut self, apps: &[A]) -> HashMap<i64, V> {
        let mut out: HashMap<i64, V> = apps
            .iter()
            .filter(|a| a.on)
            .map(|a| (a.id, V::default()))
            .collect();
        self.es.retain(|_, e| {
            let mut x = 0;
            apps.iter().any(|a| a.id == e.id && a.on)
                && unsafe { GetExitCodeProcess(e.h.0, &mut x) } != 0
                && x == STILL_ACTIVE as u32
        });
        self.ids.clear();
        if out.is_empty() {
            self.es.clear();
            return out;
        }
        let sn = Q(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) });
        if sn.0 == INVALID_HANDLE_VALUE {
            return out;
        }
        let mut pe = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut rows = Vec::with_capacity(256);
        let mut b = unsafe { Process32FirstW(sn.0, &mut pe) };
        while b != 0 {
            let n = pe
                .szExeFile
                .iter()
                .position(|v| *v == 0)
                .unwrap_or(pe.szExeFile.len());
            rows.push((
                pe.th32ProcessID,
                pe.th32ParentProcessID,
                String::from_utf16_lossy(&pe.szExeFile[..n]).to_lowercase(),
            ));
            b = unsafe { Process32NextW(sn.0, &mut pe) };
        }
        for (pid, _, nm) in &rows {
            if let Some(e) = self.es.get(pid) {
                self.ids.insert(*pid, e.id);
                continue;
            }
            if !apps.iter().any(|a| {
                a.on && Path::new(&a.ex)
                    .file_name()
                    .is_some_and(|v| v.to_string_lossy().eq_ignore_ascii_case(nm))
            }) {
                continue;
            }
            let Some(h) = handle(*pid) else {
                continue;
            };
            let mut path = [0u16; 32768];
            let mut len = path.len() as u32;
            if unsafe { QueryFullProcessImageNameW(h.0, 0, path.as_mut_ptr(), &mut len) } == 0 {
                continue;
            }
            let ex = d::norm(Path::new(&String::from_utf16_lossy(&path[..len as usize])));
            let Some(ap) = apps.iter().find(|a| a.on && a.ex == ex) else {
                continue;
            };
            let (tm, born) = times(h.0).unwrap_or_default();
            self.es.insert(
                *pid,
                E {
                    h,
                    id: ap.id,
                    tm,
                    born,
                },
            );
            self.ids.insert(*pid, ap.id);
        }
        for _ in 0..32 {
            let mut more = false;
            for (pid, pp, _) in &rows {
                if self.ids.contains_key(pid) {
                    continue;
                }
                let Some(id) = self.ids.get(pp).copied() else {
                    continue;
                };
                let Some(h) = handle(*pid) else {
                    continue;
                };
                let Some((tm, born)) = times(h.0) else {
                    continue;
                };
                if self.es.get(pp).is_some_and(|e| e.born > born) {
                    continue;
                }
                self.es.insert(*pid, E { h, id, tm, born });
                self.ids.insert(*pid, id);
                more = true;
            }
            if !more {
                break;
            }
        }
        for e in self.es.values_mut() {
            let Some(v) = out.get_mut(&e.id) else {
                continue;
            };
            v.pc += 1;
            let Some((tm, _)) = times(e.h.0) else {
                v.bad += 1;
                continue;
            };
            let mut pm = PROCESS_MEMORY_COUNTERS {
                cb: size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                ..Default::default()
            };
            if unsafe {
                GetProcessMemoryInfo(e.h.0, &mut pm, size_of::<PROCESS_MEMORY_COUNTERS>() as u32)
            } == 0
            {
                v.bad += 1;
                e.tm = tm;
                continue;
            }
            v.ct = a::a(v.ct, a::d(tm, e.tm));
            e.tm = tm;
            v.ram = a::a(v.ram, pm.WorkingSetSize as u64);
            v.ok += 1;
        }
        out
    }
}
pub fn clock() -> u64 {
    let mut t = 0;
    unsafe { windows_sys::Win32::System::WindowsProgramming::QueryUnbiasedInterruptTime(&mut t) };
    t / 10_000
}
