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
fn key(p: &Path) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    norm(p).hash(&mut h);
    h.finish()
}
pub fn cls(p: &Path, bg: bool) -> Vec<u16> {
    format!("BYT_{}_{:x}", if bg { "BG" } else { "UI" }, key(p))
        .encode_utf16()
        .chain(Some(0))
        .collect()
}
fn lock(p: &Path, bg: bool) -> R<(L, Vec<u16>, bool)> {
    let nm: Vec<u16> = format!("Local\\BYT_{}_{:x}", if bg { "TR" } else { "UI" }, key(p))
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let h = unsafe { CreateMutexW(null(), 0, nm.as_ptr()) };
    if h.is_null() {
        return Err("MUTEX".into());
    }
    let old = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    Ok((L(h), cls(p, bg), old))
}
pub fn one(p: &Path) -> R<(L, Vec<u16>, bool)> {
    lock(p, false)
}
pub fn srv(p: &Path) -> R<(L, Vec<u16>, bool)> {
    lock(p, true)
}
pub fn win(p: &Path, bg: bool) -> HWND {
    unsafe { FindWindowW(cls(p, bg).as_ptr(), null()) }
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
    #[doc = "# Safety
h must be a live window on this thread."]
    pub unsafe fn new(h: HWND) -> Self {
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
        let shell = S::new();
        self.ok = shell.as_ref().is_some_and(|s| s.call(NIM_ADD, &self.d));
        unsafe {
            SetPropW(
                self.d.hWnd,
                w!("BYT_TRAY"),
                if self.ok {
                    1usize as HANDLE
                } else {
                    null_mut()
                },
            );
        }
        if self.ok {
            self.d.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            if let Some(s) = &shell {
                s.call(NIM_SETVERSION, &self.d);
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
            if let Some(s) = S::new() {
                s.call(NIM_DELETE, &self.d);
            }
            RemovePropW(self.d.hWnd, w!("BYT_TRAY"));
            if !self.d.hIcon.is_null() {
                DestroyIcon(self.d.hIcon);
            }
        }
    }
}
#[doc = "# Safety
h must be a live window on this thread."]
pub unsafe fn menu(h: HWND) -> u32 {
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

pub fn ready(p: &Path) -> bool {
    let h = win(p, true);
    !h.is_null() && unsafe { !GetPropW(h, w!("BYT_TRAY")).is_null() }
}

struct S {
    h: HMODULE,
    f: unsafe extern "system" fn(u32, *const NOTIFYICONDATAW) -> i32,
}
impl S {
    fn new() -> Option<Self> {
        unsafe {
            use windows_sys::Win32::System::LibraryLoader::*;
            let h = LoadLibraryExW(w!("shell32.dll"), null_mut(), LOAD_LIBRARY_SEARCH_SYSTEM32);
            if h.is_null() {
                return None;
            }
            if let Some(p) = GetProcAddress(h, windows_sys::core::s!("Shell_NotifyIconW")) {
                Some(Self {
                    h,
                    f: std::mem::transmute::<
                        unsafe extern "system" fn() -> isize,
                        unsafe extern "system" fn(u32, *const NOTIFYICONDATAW) -> i32,
                    >(p),
                })
            } else {
                FreeLibrary(h);
                None
            }
        }
    }
    fn call(&self, msg: u32, d: &NOTIFYICONDATAW) -> bool {
        unsafe { (self.f)(msg, d) != 0 }
    }
}
impl Drop for S {
    fn drop(&mut self) {
        unsafe {
            FreeLibrary(self.h);
        }
    }
}
