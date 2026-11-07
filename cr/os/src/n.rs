use byt_k::a;
use std::{
    collections::HashMap,
    mem::size_of,
    ptr::null,
    sync::{Arc, Mutex, RwLock},
    thread::JoinHandle,
};
use windows_sys::Win32::System::Diagnostics::Etw::*;

pub struct B {
    ids: RwLock<HashMap<u32, i64>>,
    cnt: Mutex<HashMap<i64, (u64, u64)>>,
}
pub struct N {
    h: CONTROLTRACE_HANDLE,
    p: Vec<u64>,
    nm: Vec<u16>,
    th: Option<JoinHandle<()>>,
    pub err: u32,
    pub lost: u32,
    pub b: Arc<B>,
}
fn dec(op: u8, v: u8, b: &[u8]) -> Option<(u32, u32, bool)> {
    if !(1..=2).contains(&v) || b.len() < 8 {
        return None;
    }
    let tx = match op {
        10 | 26 => true,
        11 | 27 => false,
        _ => return None,
    };
    Some((
        u32::from_le_bytes(b[..4].try_into().ok()?),
        u32::from_le_bytes(b[4..8].try_into().ok()?),
        tx,
    ))
}
unsafe extern "system" fn cb(e: *mut EVENT_RECORD) {
    if e.is_null() {
        return;
    }
    let e = unsafe { &*e };
    let g = e.EventHeader.ProviderId;
    let tcp = g.data1 == 0x9a280ac0
        && g.data2 == 0xc8e0
        && g.data3 == 0x11d1
        && g.data4 == [0x84, 0xe2, 0, 0xc0, 0x4f, 0xb9, 0x98, 0xa2];
    let udp = g.data1 == 0xbf3a50c5
        && g.data2 == 0xa9c9
        && g.data3 == 0x4988
        && g.data4 == [0xa0, 5, 0x2d, 0xf0, 0xb7, 0xc8, 0x0f, 0x80];
    if (!tcp && !udp) || e.UserData.is_null() || e.UserContext.is_null() {
        return;
    }
    let Some((pid, n, tx)) = dec(
        e.EventHeader.EventDescriptor.Opcode,
        e.EventHeader.EventDescriptor.Version,
        unsafe { std::slice::from_raw_parts(e.UserData.cast::<u8>(), e.UserDataLength as usize) },
    ) else {
        return;
    };
    let b = unsafe { &*e.UserContext.cast::<B>() };
    let Ok(ids) = b.ids.read() else {
        return;
    };
    let Some(id) = ids.get(&pid).copied() else {
        return;
    };
    drop(ids);
    if let Ok(mut c) = b.cnt.lock() {
        let v = c.entry(id).or_default();
        if tx {
            v.0 = a::a(v.0, n as u64);
        } else {
            v.1 = a::a(v.1, n as u64);
        }
    }
}
impl Default for N {
    fn default() -> Self {
        Self::new()
    }
}

impl N {
    pub fn new() -> Self {
        let nm: Vec<u16> = format!("BYT_NET_{}", std::process::id())
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let sz = size_of::<EVENT_TRACE_PROPERTIES>() + nm.len() * 2;
        let mut n = Self {
            h: CONTROLTRACE_HANDLE::default(),
            p: vec![0; sz.div_ceil(8)],
            nm,
            th: None,
            err: 0,
            lost: 0,
            b: Arc::new(B {
                ids: RwLock::new(HashMap::new()),
                cnt: Mutex::new(HashMap::new()),
            }),
        };
        let p = n.p.as_mut_ptr().cast::<EVENT_TRACE_PROPERTIES>();
        unsafe {
            (*p).Wnode.BufferSize = sz as u32;
            (*p).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
            let mut guid = windows_sys::core::GUID::from_u128(0);
            let hr = windows_sys::Win32::System::Com::CoCreateGuid(&mut guid);
            if hr < 0 {
                n.err = hr as u32;
                return n;
            }
            (*p).Wnode.Guid = guid;
            (*p).Wnode.ClientContext = 1;
            (*p).BufferSize = 16;
            (*p).MinimumBuffers = 2;
            (*p).MaximumBuffers = 16;
            (*p).FlushTimer = 1;
            (*p).LogFileMode = EVENT_TRACE_REAL_TIME_MODE | EVENT_TRACE_SYSTEM_LOGGER_MODE;
            (*p).EnableFlags = EVENT_TRACE_FLAG_NETWORK_TCPIP;
            (*p).LoggerNameOffset = size_of::<EVENT_TRACE_PROPERTIES>() as u32;
            std::ptr::copy_nonoverlapping(
                n.nm.as_ptr(),
                p.cast::<u8>()
                    .add(size_of::<EVENT_TRACE_PROPERTIES>())
                    .cast(),
                n.nm.len(),
            );
            n.err = StartTraceW(&mut n.h, n.nm.as_ptr(), p);
        }
        if n.err != 0 {
            return n;
        }
        let mut lf = EVENT_TRACE_LOGFILEW {
            LoggerName: n.nm.as_mut_ptr(),
            Context: Arc::as_ptr(&n.b) as *mut _,
            ..Default::default()
        };
        lf.Anonymous1.ProcessTraceMode =
            PROCESS_TRACE_MODE_EVENT_RECORD | PROCESS_TRACE_MODE_REAL_TIME;
        lf.Anonymous2.EventRecordCallback = Some(cb);
        let h = unsafe { OpenTraceW(&mut lf) };
        if h.Value == u64::MAX {
            n.err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            return n;
        }
        let keep = n.b.clone();
        n.th = Some(std::thread::spawn(move || {
            let _b = keep;
            unsafe {
                ProcessTrace(&h, 1, null(), null());
                CloseTrace(h);
            }
        }));
        n
    }
    pub fn ids(&self, ids: &HashMap<u32, i64>) {
        if let Ok(mut m) = self.b.ids.write() {
            m.clone_from(ids);
        }
    }
    pub fn take(&self) -> HashMap<i64, (u64, u64)> {
        self.b
            .cnt
            .lock()
            .map(|mut v| std::mem::take(&mut *v))
            .unwrap_or_default()
    }
    pub fn poll(&mut self) {
        if self.h.Value == 0 {
            return;
        }
        let p = self.p.as_mut_ptr().cast::<EVENT_TRACE_PROPERTIES>();
        if unsafe { ControlTraceW(self.h, self.nm.as_ptr(), p, EVENT_TRACE_CONTROL_QUERY) } == 0 {
            self.lost = unsafe { (*p).EventsLost.saturating_add((*p).RealTimeBuffersLost) };
        }
    }
}
impl Drop for N {
    fn drop(&mut self) {
        if self.h.Value != 0 {
            unsafe {
                ControlTraceW(
                    self.h,
                    self.nm.as_ptr(),
                    self.p.as_mut_ptr().cast(),
                    EVENT_TRACE_CONTROL_STOP,
                );
            }
        }
        if let Some(h) = self.th.take() {
            let _ = h.join();
        }
    }
}
#[cfg(test)]
mod t {
    #[test]
    fn fmt() {
        let b = [42, 0, 0, 0, 0, 4, 0, 0];
        assert_eq!(super::dec(10, 2, &b), Some((42, 1024, true)));
        assert_eq!(super::dec(27, 1, &b), Some((42, 1024, false)));
        assert_eq!(super::dec(14, 2, &b), None);
        assert_eq!(super::dec(10, 0, &b), None);
        assert_eq!(super::dec(10, 2, &[]), None);
    }
}

#[cfg(test)]
mod it {
    use super::*;
    use std::io::{Read, Write};
    #[test]
    fn tcp() {
        let n = N::new();
        if n.err == 5 || n.err == 1314 {
            eprintln!("ETW N/A {}", n.err);
            return;
        }
        assert_eq!(n.err, 0);
        n.ids(&HashMap::from([(std::process::id(), 1)]));
        std::thread::sleep(std::time::Duration::from_millis(250));
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let t = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut b = vec![0u8; 131072];
            s.read_exact(&mut b).unwrap();
            s.write_all(&b).unwrap();
        });
        let mut s = std::net::TcpStream::connect(addr).unwrap();
        s.write_all(&vec![1u8; 131072]).unwrap();
        let mut b = vec![0u8; 131072];
        s.read_exact(&mut b).unwrap();
        drop(s);
        t.join().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2200));
        let v = n.take();
        let (up, dn) = v.get(&1).copied().unwrap_or_default();
        assert!(up >= 131072, "TX {up}");
        assert!(dn >= 131072, "RX {dn}");
    }
}
