use std::{
    mem::size_of,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command},
    ptr::null_mut,
    time::{Duration, Instant},
};
use windows_sys::{
    Win32::{
        Foundation::*,
        UI::{Shell::*, WindowsAndMessaging::*},
    },
    core::w,
};
struct F {
    pid: u32,
    h: HWND,
}
unsafe extern "system" fn cb(h: HWND, v: LPARAM) -> i32 {
    let f = unsafe { &mut *(v as *mut F) };
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(h, &mut pid);
    }
    let mut cls = [0u16; 64];
    let n = unsafe { GetClassNameW(h, cls.as_mut_ptr(), cls.len() as i32) };
    if pid == f.pid && n > 0 && String::from_utf16_lossy(&cls[..n as usize]).starts_with("BYT_UI_")
    {
        f.h = h;
        0
    } else {
        1
    }
}
fn wait(mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(12);
    while !f() {
        assert!(Instant::now() < end, "timeout");
        std::thread::sleep(Duration::from_millis(50));
    }
}
fn spawn(p: &Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_byt"))
        .args(["--data"])
        .arg(p)
        .creation_flags(0x08000000)
        .spawn()
        .unwrap()
}
struct X {
    p: PathBuf,
    ch: Child,
}
impl Drop for X {
    fn drop(&mut self) {
        let h = byt_bg::t::win(&self.p, false);
        if !h.is_null() {
            unsafe {
                PostMessageW(h, byt_bg::t::Q, 0, 0);
            }
        }
        if let Ok(c) = byt_db::open(&self.p) {
            let _ = byt_db::kv(&c, "stop", "1");
        }
        if self.ch.try_wait().ok().flatten().is_none() {
            let _ = self.ch.kill();
            let _ = self.ch.wait();
        }
    }
}
#[test]
fn tray() {
    let p = std::env::temp_dir().join(format!("byt_tr_{}_{}", std::process::id(), byt_db::now()));
    let dir = p.join("files");
    std::fs::create_dir_all(&dir).unwrap();
    let c = byt_db::open(&p).unwrap();
    let side = Path::new(env!("CARGO_BIN_EXE_byt")).with_file_name("bg.exe");
    let native = side.is_file();
    let id = byt_db::add(
        &c,
        if native {
            side.as_path()
        } else {
            Path::new(env!("CARGO_BIN_EXE_byt"))
        },
    )
    .unwrap();
    c.execute(
        "UPDATE a SET ds=?1 WHERE id=?2",
        rusqlite::params![
            serde_json::to_string(&vec![byt_db::norm(&dir)]).unwrap(),
            id
        ],
    )
    .unwrap();
    let mut x = X {
        p: p.clone(),
        ch: spawn(&p),
    };
    let mut f = F {
        pid: x.ch.id(),
        h: null_mut(),
    };
    wait(|| {
        unsafe {
            EnumWindows(Some(cb), (&mut f as *mut F) as isize);
        }
        !f.h.is_null()
    });
    let h = f.h;
    wait(|| byt_db::s(&c, id).ms >= 1000);
    if native {
        wait(|| !byt_bg::t::win(&p, true).is_null());
        if byt_bg::t::ready(&p) {
            native_tray(&p, &c, id, &mut x, h);
            drop(c);
            drop(x);
            std::fs::remove_dir_all(p).unwrap();
            return;
        }
    }
    let ni = NOTIFYICONIDENTIFIER {
        cbSize: size_of::<NOTIFYICONIDENTIFIER>() as u32,
        hWnd: h,
        uID: 1,
        ..Default::default()
    };
    let mut rect = RECT::default();
    let shell = unsafe { Shell_NotifyIconGetRect(&ni, &mut rect) } == 0;
    if shell {
        let nd = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: h,
            uID: 1,
            ..Default::default()
        };
        assert_ne!(unsafe { Shell_NotifyIconW(NIM_DELETE, &nd) }, 0);
        let msg = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
        unsafe {
            PostMessageW(h, msg, 0, 0);
        }
        wait(|| unsafe { Shell_NotifyIconGetRect(&ni, &mut rect) } == 0);
    }
    unsafe {
        PostMessageW(h, WM_CLOSE, 0, 0);
    }
    if shell {
        wait(|| unsafe { IsWindowVisible(h) } == 0);
    } else {
        std::thread::sleep(Duration::from_millis(200));
        eprintln!("TRAY N/A no shell");
    }
    assert_ne!(unsafe { IsWindow(h) }, 0);
    assert!(x.ch.try_wait().unwrap().is_none());
    wait(|| byt_db::s(&c, id).ms >= 1000);
    let ms = byt_db::s(&c, id).ms;
    wait(|| byt_db::s(&c, id).ms >= ms + 1000);
    let mut second = spawn(&p);
    wait(|| second.try_wait().unwrap().is_some());
    assert!(second.wait().unwrap().success());
    wait(|| unsafe { IsWindowVisible(h) } != 0);
    unsafe {
        PostMessageW(h, WM_COMMAND, 102, 0);
    }
    if shell {
        wait(|| unsafe { IsWindowVisible(h) } == 0);
    }
    unsafe {
        PostMessageW(h, WM_COMMAND, 101, 0);
    }
    wait(|| unsafe { IsWindowVisible(h) } != 0);
    unsafe {
        PostMessageW(h, WM_COMMAND, 103, 0);
    }
    wait(|| x.ch.try_wait().unwrap().is_some());
    assert!(x.ch.wait().unwrap().success());
    wait(|| byt_db::val(&c, "bg_pid") == "0");
    if shell {
        assert_ne!(unsafe { Shell_NotifyIconGetRect(&ni, &mut rect) }, 0);
    }
    drop(c);
    drop(x);
    assert!(p.starts_with(std::env::temp_dir()));
    std::fs::remove_dir_all(p).unwrap();
}

fn native_tray(p: &Path, c: &rusqlite::Connection, id: i64, x: &mut X, h: HWND) {
    let bg = byt_bg::t::win(p, true);
    let ni = NOTIFYICONIDENTIFIER {
        cbSize: size_of::<NOTIFYICONIDENTIFIER>() as u32,
        hWnd: bg,
        uID: 1,
        ..Default::default()
    };
    let mut r = RECT::default();
    assert_eq!(unsafe { Shell_NotifyIconGetRect(&ni, &mut r) }, 0);
    let nd = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: bg,
        uID: 1,
        ..Default::default()
    };
    assert_ne!(unsafe { Shell_NotifyIconW(NIM_DELETE, &nd) }, 0);
    unsafe {
        PostMessageW(bg, RegisterWindowMessageW(w!("TaskbarCreated")), 0, 0);
    }
    wait(|| unsafe { Shell_NotifyIconGetRect(&ni, &mut r) } == 0);
    unsafe {
        PostMessageW(h, WM_CLOSE, 0, 0);
    }
    wait(|| x.ch.try_wait().unwrap().is_some());
    assert!(x.ch.wait().unwrap().success());
    assert!(!byt_db::val(c, "ui").is_empty());
    let ms = byt_db::s(c, id).ms;
    wait(|| byt_db::s(c, id).ms >= ms + 1000);
    let mut second = spawn(p);
    wait(|| !byt_bg::t::win(p, false).is_null());
    let h2 = byt_bg::t::win(p, false);
    let mut third = spawn(p);
    wait(|| third.try_wait().unwrap().is_some());
    assert!(third.wait().unwrap().success());
    assert_eq!(byt_bg::t::win(p, false), h2);
    unsafe {
        PostMessageW(bg, WM_COMMAND, 102, 0);
    }
    wait(|| second.try_wait().unwrap().is_some());
    assert!(second.wait().unwrap().success());
    unsafe {
        PostMessageW(bg, WM_COMMAND, 101, 0);
    }
    wait(|| !byt_bg::t::win(p, false).is_null());
    let h3 = byt_bg::t::win(p, false);
    wait(|| unsafe { IsWindowVisible(h3) } != 0);
    unsafe {
        PostMessageW(bg, WM_COMMAND, 103, 0);
    }
    wait(|| byt_db::val(c, "bg_pid") == "0");
    wait(|| unsafe { IsWindow(bg) } == 0);
    wait(|| unsafe { IsWindow(h3) } == 0);
    assert_ne!(unsafe { Shell_NotifyIconGetRect(&ni, &mut r) }, 0);
}

#[test]
fn bg_start() {
    let exe = Path::new(env!("CARGO_BIN_EXE_byt"));
    if !exe.with_file_name("bg.exe").is_file() {
        return;
    }
    let p = std::env::temp_dir().join(format!(
        "byt_start_{}_{}",
        std::process::id(),
        byt_db::now()
    ));
    let c = byt_db::open(&p).unwrap();
    let dir = p.join("files");
    std::fs::create_dir_all(&dir).unwrap();
    let id = byt_db::add(&c, exe).unwrap();
    c.execute(
        "UPDATE a SET ds=?1 WHERE id=?2",
        rusqlite::params![
            serde_json::to_string(&vec![byt_db::norm(&dir)]).unwrap(),
            id
        ],
    )
    .unwrap();
    let mut x = X {
        p: p.clone(),
        ch: Command::new(exe)
            .arg("--bg")
            .arg("--data")
            .arg(&p)
            .creation_flags(0x08000000)
            .spawn()
            .unwrap(),
    };
    wait(|| x.ch.try_wait().unwrap().is_some());
    assert!(x.ch.wait().unwrap().success());
    wait(|| !byt_bg::t::win(&p, true).is_null());
    let h = byt_bg::t::win(&p, true);
    assert!(byt_bg::t::win(&p, false).is_null());
    wait(|| byt_db::val(&c, "bg_pid").parse::<u32>().unwrap_or(0) > 0);
    wait(|| byt_db::s(&c, id).ms >= 1000);
    unsafe {
        PostMessageW(h, WM_COMMAND, 103, 0);
    }
    wait(|| byt_db::val(&c, "bg_pid") == "0");
    wait(|| unsafe { IsWindow(h) } == 0);
    drop(c);
    drop(x);
    assert!(p.starts_with(std::env::temp_dir()));
    std::fs::remove_dir_all(p).unwrap();
}
#[test]
fn standalone() {
    let p = std::env::temp_dir().join(format!("byt_one_{}_{}", std::process::id(), byt_db::now()));
    std::fs::create_dir_all(&p).unwrap();
    let ex = p.join("alone.exe");
    std::fs::copy(env!("CARGO_BIN_EXE_byt"), &ex).unwrap();
    let c = byt_db::open(&p).unwrap();
    let mut x = X {
        p: p.clone(),
        ch: Command::new(ex)
            .arg("--data")
            .arg(&p)
            .creation_flags(0x08000000)
            .spawn()
            .unwrap(),
    };
    let mut f = F {
        pid: x.ch.id(),
        h: null_mut(),
    };
    wait(|| {
        unsafe {
            EnumWindows(Some(cb), (&mut f as *mut F) as isize);
        }
        !f.h.is_null()
    });
    let h = f.h;
    wait(|| byt_db::val(&c, "bg_pid").parse::<u32>().unwrap_or(0) > 0);
    assert!(byt_bg::t::win(&p, true).is_null());
    unsafe {
        PostMessageW(h, WM_CLOSE, 0, 0);
    }
    std::thread::sleep(Duration::from_millis(200));
    assert!(x.ch.try_wait().unwrap().is_none());
    assert_ne!(unsafe { IsWindow(h) }, 0);
    unsafe {
        PostMessageW(h, WM_COMMAND, 103, 0);
    }
    wait(|| x.ch.try_wait().unwrap().is_some());
    assert!(x.ch.wait().unwrap().success());
    wait(|| byt_db::val(&c, "bg_pid") == "0");
    drop(c);
    drop(x);
    assert!(p.starts_with(std::env::temp_dir()));
    std::fs::remove_dir_all(p).unwrap();
}
