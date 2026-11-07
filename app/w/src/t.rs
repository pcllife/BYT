use byt_k::{R, norm};
use std::{
    hash::{Hash, Hasher},
    mem::size_of,
    path::Path,
    ptr::{null, null_mut},
};
use windows_sys::{
    Win32::{
        Foundation::*,
        System::Threading::*,
        UI::{Shell::*, WindowsAndMessaging::*},
    },
    core::w,
};
pub const M: u32 = WM_APP + 17;
pub const Q: u32 = WM_APP + 18;
pub const OPEN: u32 = 101;
pub const HIDE: u32 = 102;
pub const QUIT: u32 = 103;
pub struct L(HANDLE);
impl Drop for L {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
pub fn one(p: &Path) -> R<(L, Vec<u16>, bool)> {
    let mut hs = std::collections::hash_map::DefaultHasher::new();
    norm(p).hash(&mut hs);
    let key = hs.finish();
    let nm: Vec<u16> = format!("Local\\BYT_UI_{key:x}")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let h = unsafe { CreateMutexW(null(), 0, nm.as_ptr()) };
    if h.is_null() {
        return Err("UI MUTEX".into());
    }
    let old = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    let cls: Vec<u16> = format!("BYT_UI_{key:x}")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    Ok((L(h), cls, old))
}
pub fn wake(cls: &[u16]) -> R<()> {
    for _ in 0..40 {
        let h = unsafe { FindWindowW(cls.as_ptr(), null()) };
        if !h.is_null() {
            unsafe {
                PostMessageW(h, M, 0, NIN_SELECT as isize);
            };
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Err("UI START".into())
}
#[doc = "# tray - keep one icon and put it back if explorer restarts"]
pub struct T {
    d: NOTIFYICONDATAW,
    pub msg: u32,
    pub ok: bool,
}
impl T {
    pub fn new(h: HWND) -> Self {
        let mut b = [0u8; 16 * 16 * 4];
        for y in 0..16 {
            for x in 0..16 {
                let p = (y * 16 + x) * 4;
                let bar = (x == 4 && (3..=12).contains(&y))
                    || ((y == 3 || y == 7 || y == 12) && (4..=9).contains(&x))
                    || (x == 10 && ((4..7).contains(&y) || (8..12).contains(&y)));
                if bar {
                    b[p..p + 3].fill(255);
                }
                if x >= 12 && y >= 12 {
                    b[p + 1] = 255;
                }
                b[p + 3] = 255;
            }
        }
        let ic = unsafe { CreateIcon(null_mut(), 16, 16, 1, 32, [0u8; 32].as_ptr(), b.as_ptr()) };
        let mut d = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: h,
            uID: 1,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
            uCallbackMessage: M,
            hIcon: ic,
            ..Default::default()
        };
        d.szTip[..4].copy_from_slice(&[66, 89, 84, 0]);
        let mut t = Self {
            d,
            msg: unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
            ok: false,
        };
        if t.msg != 0 {
            unsafe {
                ChangeWindowMessageFilterEx(h, t.msg, MSGFLT_ALLOW, null_mut());
            }
        }
        t.add();
        t
    }
    pub fn add(&mut self) -> bool {
        self.ok = unsafe { Shell_NotifyIconW(NIM_ADD, &self.d) } != 0;
        if self.ok {
            self.d.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            unsafe {
                Shell_NotifyIconW(NIM_SETVERSION, &self.d);
            }
        }
        self.ok
    }
    pub fn ready(&mut self) -> bool {
        self.ok || self.add()
    }
}
impl Drop for T {
    fn drop(&mut self) {
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &self.d);
            if !self.d.hIcon.is_null() {
                DestroyIcon(self.d.hIcon);
            }
        }
    }
}
pub fn menu(h: HWND) -> u32 {
    unsafe {
        let m = CreatePopupMenu();
        if m.is_null() {
            return 0;
        }
        AppendMenuW(m, MF_STRING, OPEN as usize, w!("OPEN"));
        AppendMenuW(m, MF_STRING, HIDE as usize, w!("HIDE"));
        AppendMenuW(m, MF_SEPARATOR, 0, null());
        AppendMenuW(m, MF_STRING, QUIT as usize, w!("QUIT ALL"));
        let mut p = POINT::default();
        GetCursorPos(&mut p);
        SetForegroundWindow(h);
        let id = TrackPopupMenu(m, TPM_RETURNCMD | TPM_RIGHTBUTTON, p.x, p.y, 0, h, null());
        DestroyMenu(m);
        PostMessageW(h, WM_NULL, 0, 0);
        id as u32
    }
}
