use byt_bg::{self as b, t};
use byt_db::{self as d, R};
use std::{
    path::{Path, PathBuf},
    ptr::{null, null_mut},
    thread::JoinHandle,
};
use windows_sys::{
    Win32::{
        Foundation::*,
        System::LibraryLoader::*,
        UI::{Shell::NIN_SELECT, WindowsAndMessaging::*},
    },
    core::w,
};
#[doc = "# tray - no font, no canvas, keep the collector alive"]
struct W {
    p: PathBuf,
    ui: PathBuf,
    tr: Option<t::T>,
    bg: Option<JoinHandle<R<()>>>,
}
fn open(p: &Path, ex: &Path) -> R<()> {
    let h = t::win(p, false);
    if !h.is_null() {
        unsafe {
            PostMessageW(h, t::M, 0, NIN_SELECT as isize);
        }
    } else {
        use std::os::windows::process::CommandExt;
        std::process::Command::new(ex)
            .arg("--data")
            .arg(p)
            .creation_flags(0x08000000)
            .spawn()?;
    }
    Ok(())
}
fn stop(p: &Path) {
    if let Ok(c) = d::open(p) {
        let _ = d::kv(&c, "stop", "1");
    }
    let h = t::win(p, false);
    if !h.is_null() {
        unsafe {
            PostMessageW(h, t::Q, 0, 0);
        }
    }
}
unsafe extern "system" fn wp(h: HWND, m: u32, a: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if m == WM_NCCREATE {
            let cs = &*(l as *const CREATESTRUCTW);
            SetWindowLongPtrW(h, GWLP_USERDATA, cs.lpCreateParams as isize);
        }
        if m == t::Q {
            DestroyWindow(h);
            return 0;
        }
        if m == t::M && matches!(l as u32 & 0xffff, WM_CONTEXTMENU | WM_RBUTTONUP) {
            let id = t::menu(h);
            if id != 0 {
                PostMessageW(h, WM_COMMAND, id as usize, 0);
            }
            return 0;
        }
        let p = GetWindowLongPtrW(h, GWLP_USERDATA) as *mut W;
        if !p.is_null() {
            let s = &mut *p;
            if s.tr.as_ref().is_some_and(|t| t.msg != 0 && m == t.msg) {
                if let Some(t) = &mut s.tr {
                    t.add();
                }
                return 0;
            }
            match m {
                WM_CREATE => {
                    s.tr = Some(t::T::new(h));
                    return 0;
                }
                t::M => {
                    if matches!(
                        l as u32 & 0xffff,
                        NIN_SELECT | 0x401 | WM_LBUTTONUP | WM_LBUTTONDBLCLK
                    ) && let Err(e) = open(&s.p, &s.ui)
                    {
                        let _ = std::fs::write(s.p.join("err.txt"), e.to_string());
                    }
                    return 0;
                }
                WM_COMMAND => {
                    match a as u32 & 0xffff {
                        t::OPEN => {
                            let _ = open(&s.p, &s.ui);
                        }
                        t::HIDE => {
                            let h = t::win(&s.p, false);
                            if !h.is_null() {
                                PostMessageW(h, WM_CLOSE, 0, 0);
                            }
                        }
                        t::QUIT => stop(&s.p),
                        _ => {}
                    }
                    return 0;
                }
                WM_CLOSE => {
                    stop(&s.p);
                    return 0;
                }
                WM_DESTROY => {
                    s.tr.take();
                    PostQuitMessage(0);
                    return 0;
                }
                _ => {}
            }
        }
        DefWindowProcW(h, m, a, l)
    }
}
pub fn run(p: &Path) -> R<()> {
    let (_lock, cls, old) = t::srv(p)?;
    if old {
        return Ok(());
    }
    let args: Vec<_> = std::env::args_os().collect();
    let ui = args
        .iter()
        .position(|s| s == "--ui")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or(std::env::current_exe()?.with_file_name("byt.exe"));
    let mut state = Box::new(W {
        p: p.to_path_buf(),
        ui,
        tr: None,
        bg: None,
    });
    unsafe {
        let ins = GetModuleHandleW(null());
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wp),
            hInstance: ins,
            lpszClassName: cls.as_ptr(),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let h = CreateWindowExW(
            0,
            cls.as_ptr(),
            w!("BYT BG"),
            0,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            ins,
            (&mut *state as *mut W).cast(),
        );
        if h.is_null() {
            return Err(format!("BG WIN {}", GetLastError()).into());
        }
        let hwnd = h as usize;
        let path = p.to_path_buf();
        state.bg = Some(std::thread::spawn(move || {
            let out = b::run(&path, None);
            if let Err(e) = &out {
                let _ = std::fs::write(path.join("err.txt"), e.to_string());
            }
            PostMessageW(hwnd as HWND, t::Q, 0, 0);
            out
        }));
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if let Some(bg) = state.bg.take() {
            bg.join().map_err(|_| "BG JOIN")??;
        }
        drop(state);
        Ok(())
    }
}
