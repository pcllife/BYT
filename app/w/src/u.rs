use crate::t;
use byt_bg as b;
use byt_db::{self as d, A, H, R, S, Z};
use rusqlite::Connection;
use std::{
    ffi::c_void,
    mem::size_of,
    path::{Path, PathBuf},
    ptr::{null, null_mut},
};
use windows_sys::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{Com::*, LibraryLoader::*, Registry::*},
        UI::{Controls::Dialogs::*, Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
    },
    core::w,
};

const BK: u32 = 0x000000;
const WH: u32 = 0xffffff;
const RD: u32 = 0x0000ff;
const GR: u32 = 0x00ff00;
const BL: u32 = 0xff0000;
const PU: u32 = 0x800080;
const YL: u32 = 0x00ffff;
const BR: u32 = 0x2a2aa5;
#[derive(Clone, Copy)]
enum I {
    Add,
    Run,
    Del,
    Pause,
    Auto,
    Bg,
    Quit,
    Adm,
    DirAdd,
    DirDel,
    DirOpen,
    Export,
    Sel(usize),
    Dir(usize),
}
#[doc = "# ui - reuse the buffer, no redraw spin"]
struct C {
    dc: HDC,
    bm: HBITMAP,
    old: HGDIOBJ,
    w: i32,
    h: i32,
}
impl C {
    fn new() -> Self {
        Self {
            dc: null_mut(),
            bm: null_mut(),
            old: null_mut(),
            w: 0,
            h: 0,
        }
    }
    unsafe fn fit(&mut self, dc: HDC, w: i32, h: i32) {
        unsafe {
            if self.w == w && self.h == h {
                return;
            }
            if self.dc.is_null() {
                self.dc = CreateCompatibleDC(dc);
            }
            if !self.bm.is_null() {
                SelectObject(self.dc, self.old);
                DeleteObject(self.bm);
            }
            self.bm = CreateCompatibleBitmap(dc, w.max(1), h.max(1));
            self.old = SelectObject(self.dc, self.bm);
            self.w = w;
            self.h = h;
        }
    }
}
impl Drop for C {
    fn drop(&mut self) {
        unsafe {
            if !self.dc.is_null() {
                SelectObject(self.dc, self.old);
                DeleteObject(self.bm);
                DeleteDC(self.dc);
            }
        }
    }
}
struct U {
    c: Connection,
    p: PathBuf,
    apps: Vec<A>,
    ss: Vec<(S, Z)>,
    sel: usize,
    off: usize,
    di: usize,
    doff: usize,
    h: Vec<H>,
    hits: Vec<(RECT, I)>,
    f: HFONT,
    bf: HFONT,
    fr: HANDLE,
    cv: C,
    ver: i64,
    net: String,
    gpu: String,
    fs: String,
    hb: u64,
    au: bool,
    tr: Option<t::T>,
    hid: bool,
    tt: u32,
    er: Option<String>,
}
fn wz(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn rc(x: i32, y: i32, w: i32, h: i32) -> RECT {
    RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    }
}
fn byt(v: f64) -> String {
    let mut n = v;
    let mut u = "B";
    for a in ["KiB", "MiB", "GiB", "TiB"] {
        if n < 1024. {
            break;
        }
        n /= 1024.;
        u = a;
    }
    if u == "B" {
        format!("{n:.0} B")
    } else {
        format!("{n:.2} {u}")
    }
}
fn pct(v: Option<f64>) -> String {
    v.map(|v| format!("{v:.2}%"))
        .unwrap_or_else(|| "N/A".into())
}
fn tm(ms: u64) -> String {
    let s = ms / 1000;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}
fn date(ts: u64) -> String {
    let v = (ts + 11_644_473_600) * 10_000_000;
    let ft = FILETIME {
        dwLowDateTime: v as u32,
        dwHighDateTime: (v >> 32) as u32,
    };
    let mut st = SYSTEMTIME::default();
    unsafe {
        windows_sys::Win32::System::Time::FileTimeToSystemTime(&ft, &mut st);
    }
    format!("{:04}-{:02}-{:02}", st.wYear, st.wMonth, st.wDay)
}
unsafe fn fill(dc: HDC, r: RECT, c: u32) {
    unsafe {
        let b = CreateSolidBrush(c);
        FillRect(dc, &r, b);
        DeleteObject(b);
    }
}
unsafe fn frame(dc: HDC, r: RECT, c: u32) {
    unsafe {
        let b = CreateSolidBrush(c);
        FrameRect(dc, &r, b);
        DeleteObject(b);
    }
}
unsafe fn txt(dc: HDC, r: RECT, s: &str, c: u32, f: HFONT) {
    unsafe {
        SetTextColor(dc, c);
        SetBkMode(dc, TRANSPARENT as i32);
        let old = SelectObject(dc, f);
        let mut r = r;
        let mut t = wz(s);
        DrawTextW(
            dc,
            t.as_mut_ptr(),
            (t.len() - 1) as i32,
            &mut r,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        SelectObject(dc, old);
    }
}
unsafe fn msg(h: HWND, s: &str) {
    unsafe {
        MessageBoxW(h, wz(s).as_ptr(), w!("BYT"), MB_OK | MB_ICONERROR);
    }
}
unsafe fn confirm(h: HWND, s: &str) -> bool {
    unsafe { MessageBoxW(h, wz(s).as_ptr(), w!("BYT"), MB_YESNO | MB_ICONQUESTION) == IDYES }
}
fn auto_get() -> bool {
    unsafe {
        let mut k = null_mut();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            0,
            KEY_QUERY_VALUE,
            &mut k,
        ) != 0
        {
            return false;
        }
        let mut sz = 0;
        let n = RegQueryValueExW(k, w!("BYT"), null(), null_mut(), null_mut(), &mut sz);
        RegCloseKey(k);
        n == 0
    }
}
fn auto_set(on: bool, p: &Path) -> R<()> {
    unsafe {
        let mut k = null_mut();
        let n = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            0,
            null(),
            0,
            KEY_SET_VALUE,
            null(),
            &mut k,
            null_mut(),
        );
        if n != 0 {
            return Err(format!("REG {n}").into());
        }
        let n = if on {
            let s = wz(&format!(
                "\"{}\" --bg --data \"{}\"",
                std::env::current_exe()?.display(),
                p.display()
            ));
            RegSetValueExW(
                k,
                w!("BYT"),
                0,
                REG_SZ,
                s.as_ptr().cast(),
                (s.len() * 2) as u32,
            )
        } else {
            RegDeleteValueW(k, w!("BYT"))
        };
        RegCloseKey(k);
        if n != 0 && n != 2 {
            return Err(format!("REG {n}").into());
        }
        Ok(())
    }
}
unsafe fn pick(h: HWND, save: bool) -> Option<PathBuf> {
    unsafe {
        let mut buf = [0u16; 32768];
        let filter = if save {
            w!("CSV\0*.csv\0\0")
        } else {
            w!("EXE\0*.exe\0\0")
        };
        let mut of = OPENFILENAMEW {
            lStructSize: size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: h,
            lpstrFilter: filter,
            lpstrFile: buf.as_mut_ptr(),
            nMaxFile: buf.len() as u32,
            Flags: OFN_EXPLORER
                | OFN_NOCHANGEDIR
                | if save {
                    OFN_OVERWRITEPROMPT
                } else {
                    OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST
                },
            lpstrDefExt: if save { w!("csv") } else { w!("exe") },
            ..Default::default()
        };
        let ok = if save {
            GetSaveFileNameW(&mut of)
        } else {
            GetOpenFileNameW(&mut of)
        };
        if ok == 0 {
            return None;
        }
        let n = buf.iter().position(|c| *c == 0)?;
        Some(PathBuf::from(String::from_utf16_lossy(&buf[..n])))
    }
}
unsafe fn folder(h: HWND) -> Option<String> {
    unsafe {
        let bi = BROWSEINFOW {
            hwndOwner: h,
            lpszTitle: w!("DIR"),
            ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
            ..Default::default()
        };
        let id = SHBrowseForFolderW(&bi);
        if id.is_null() {
            return None;
        }
        let mut buf = [0u16; 32768];
        let ok = SHGetPathFromIDListEx(id, buf.as_mut_ptr(), buf.len() as u32, 0);
        CoTaskMemFree(id.cast());
        if ok == 0 {
            return None;
        }
        let n = buf.iter().position(|v| *v == 0)?;
        Some(d::norm(Path::new(&String::from_utf16_lossy(&buf[..n]))))
    }
}
impl U {
    fn new(p: &Path) -> R<Self> {
        let c = d::open(p)?;
        let data = include_bytes!("../../../res/f.ttf");
        let n = std::cell::UnsafeCell::new(0u32);
        let fr = unsafe {
            AddFontMemResourceEx(
                data.as_ptr() as *const c_void,
                data.len() as u32,
                null(),
                n.get(),
            )
        };
        if fr.is_null() {
            return Err("FONT".into());
        }
        let f = unsafe {
            CreateFontW(
                16,
                0,
                0,
                0,
                FW_NORMAL as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                NONANTIALIASED_QUALITY as u32,
                FIXED_PITCH as u32,
                w!("Silkscreen"),
            )
        };
        let bf = unsafe {
            CreateFontW(
                32,
                0,
                0,
                0,
                FW_NORMAL as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                NONANTIALIASED_QUALITY as u32,
                FIXED_PITCH as u32,
                w!("Silkscreen"),
            )
        };
        let mut u = Self {
            c,
            p: p.to_path_buf(),
            apps: vec![],
            ss: vec![],
            sel: 0,
            off: 0,
            di: 0,
            doff: 0,
            h: vec![],
            hits: vec![],
            f,
            bf,
            fr,
            cv: C::new(),
            ver: -1,
            net: String::new(),
            gpu: String::new(),
            fs: String::new(),
            hb: 0,
            au: auto_get(),
            tr: None,
            hid: false,
            tt: 0,
            er: None,
        };
        u.load(true)?;
        Ok(u)
    }
    fn load(&mut self, force: bool) -> R<bool> {
        let ver = self.c.query_row("PRAGMA data_version", [], |r| r.get(0))?;
        if !force && self.ver == ver {
            return Ok(false);
        }
        self.ver = ver;
        let id = self.apps.get(self.sel).map(|a| a.id);
        self.apps = d::list(&self.c)?;
        self.sel = id
            .and_then(|id| self.apps.iter().position(|a| a.id == id))
            .unwrap_or(self.sel)
            .min(self.apps.len().saturating_sub(1));
        self.ss = self
            .apps
            .iter()
            .map(|a| (d::s(&self.c, a.id), d::z(&self.c, a.id)))
            .collect();
        self.h = self
            .apps
            .get(self.sel)
            .map(|a| d::hist(&self.c, a.id))
            .unwrap_or_default();
        self.net = d::val(&self.c, "net");
        self.gpu = d::val(&self.c, "gpu");
        self.fs = d::val(&self.c, "fs");
        self.hb = d::val(&self.c, "hb").parse().unwrap_or(0);
        Ok(true)
    }
    unsafe fn btn(&mut self, dc: HDC, r: RECT, s: &str, i: I, c: u32) {
        unsafe {
            fill(dc, r, c);
            frame(dc, r, BK);
            txt(
                dc,
                rc(r.left + 8, r.top, r.right - r.left - 12, r.bottom - r.top),
                s,
                if c == BL || c == BK || c == RD || c == PU || c == BR {
                    WH
                } else {
                    BK
                },
                self.f,
            );
            self.hits.push((r, i));
        }
    }
    unsafe fn paint(&mut self, _h: HWND, dc: HDC, w: i32, ht: i32) {
        unsafe {
            let dst = dc;
            self.cv.fit(dc, w, ht);
            let dc = self.cv.dc;
            fill(dc, rc(0, 0, w, ht), WH);
            self.hits.clear();
            fill(dc, rc(0, 0, w, 44), BK);
            txt(dc, rc(12, 4, 100, 36), "BYT", WH, self.bf);
            self.btn(dc, rc(116, 8, 96, 28), "ADD EXE", I::Add, WH);
            self.btn(dc, rc(220, 8, 88, 28), "RUN", I::Run, GR);
            self.btn(dc, rc(316, 8, 88, 28), "PAUSE", I::Pause, YL);
            self.btn(dc, rc(412, 8, 80, 28), "REMOVE", I::Del, RD);
            self.btn(dc, rc(500, 8, 80, 28), "EXPORT", I::Export, WH);
            self.btn(
                dc,
                rc(w - 428, 8, 104, 28),
                if self.au { "AUTO [x]" } else { "AUTO [ ]" },
                I::Auto,
                WH,
            );
            self.btn(dc, rc(w - 316, 8, 96, 28), "BG", I::Bg, BL);
            self.btn(dc, rc(w - 212, 8, 96, 28), "NET ADMIN", I::Adm, PU);
            self.btn(dc, rc(w - 108, 8, 96, 28), "QUIT ALL", I::Quit, RD);
            frame(dc, rc(8, 52, 224, ht - 88), BK);
            fill(dc, rc(9, 53, 222, 30), BK);
            txt(
                dc,
                rc(16, 54, 208, 28),
                &format!("APP {}", self.apps.len()),
                WH,
                self.f,
            );
            let rows = ((ht - 135) / 46).max(1) as usize;
            self.off = self.off.min(self.apps.len().saturating_sub(rows));
            for i in self.off..(self.off + rows).min(self.apps.len()) {
                let y = 90 + (i - self.off) as i32 * 46;
                let a = &self.apps[i];
                let s = &self.ss[i].0;
                let on = a.on;
                let nm = a.nm.clone();
                let t = tm(s.ms);
                let pc = s.pc;
                fill(dc, rc(12, y, 216, 42), if i == self.sel { BL } else { WH });
                txt(
                    dc,
                    rc(18, y, 200, 22),
                    &nm,
                    if i == self.sel { WH } else { BK },
                    self.f,
                );
                txt(
                    dc,
                    rc(18, y + 22, 200, 18),
                    &format!(
                        "{}  {}",
                        if !on {
                            "PAUSE"
                        } else if pc > 0 {
                            "RUN"
                        } else {
                            "STOP"
                        },
                        t
                    ),
                    if i == self.sel {
                        WH
                    } else {
                        if pc > 0 { BL } else { BK }
                    },
                    self.f,
                );
                self.hits.push((rc(12, y, 216, 42), I::Sel(i)));
            }
            let x = 244;
            let rw = w - x - 12;
            if let Some(ap) = self.apps.get(self.sel).cloned() {
                let (s, z) = self.ss[self.sel].clone();
                txt(dc, rc(x, 52, rw - 112, 36), &ap.nm, BK, self.bf);
                fill(
                    dc,
                    rc(w - 116, 56, 104, 28),
                    if !ap.on {
                        YL
                    } else if s.pc > 0 {
                        GR
                    } else {
                        WH
                    },
                );
                frame(dc, rc(w - 116, 56, 104, 28), BK);
                txt(
                    dc,
                    rc(w - 108, 56, 92, 28),
                    if !ap.on {
                        "PAUSE"
                    } else if s.pc > 0 {
                        "RUN"
                    } else {
                        "STOP"
                    },
                    BK,
                    self.f,
                );
                txt(dc, rc(x, 92, rw, 22), &ap.ex, BK, self.f);
                let cells = vec![
                    ("TIME", tm(s.ms), String::new(), BK),
                    (
                        "FILES",
                        if z.ok { byt(z.sz as f64) } else { "N/A".into() },
                        format!("{} FILE", z.cnt),
                        BK,
                    ),
                    ("CREATED", byt(z.ad as f64), String::new(), BL),
                    ("REMOVED", byt(z.rm as f64), String::new(), RD),
                    ("CPU AVG", pct(s.avg_c()), format!("NOW {}", pct(s.c)), BL),
                    (
                        "RAM AVG",
                        s.avg_r().map(byt).unwrap_or_else(|| "N/A".into()),
                        format!(
                            "NOW {}",
                            s.r.map(|v| byt(v as f64)).unwrap_or_else(|| "N/A".into())
                        ),
                        BK,
                    ),
                    ("GPU AVG", pct(s.avg_g()), format!("NOW {}", pct(s.g)), PU),
                    ("PID", s.pc.to_string(), format!("ERR {}", s.bad), BK),
                    (
                        "UPLOAD",
                        if s.nm > 0 {
                            byt(s.up as f64)
                        } else {
                            "N/A".into()
                        },
                        String::new(),
                        BL,
                    ),
                    (
                        "DOWNLOAD",
                        if s.nm > 0 {
                            byt(s.dn as f64)
                        } else {
                            "N/A".into()
                        },
                        String::new(),
                        GR,
                    ),
                    ("SINCE UTC", date(ap.ts), String::new(), BR),
                    ("DIR", ap.ds.len().to_string(), format!("ERR {}", z.err), BK),
                ];
                let cw = (rw - 18) / 4;
                for (i, (label, val, sub, color)) in cells.iter().enumerate() {
                    let cx = x + (i % 4) as i32 * (cw + 6);
                    let cy = 122 + (i / 4) as i32 * 94;
                    frame(dc, rc(cx, cy, cw, 88), BK);
                    txt(dc, rc(cx + 8, cy + 4, cw - 16, 20), label, BK, self.f);
                    txt(
                        dc,
                        rc(cx + 8, cy + 26, cw - 16, 36),
                        val,
                        if *color == GR { BK } else { *color },
                        self.bf,
                    );
                    txt(dc, rc(cx + 8, cy + 64, cw - 16, 20), sub, BK, self.f);
                }
                let gy = 414;
                let gh = (ht - gy - 190).clamp(100, 240);
                frame(dc, rc(x, gy, rw, gh), BK);
                txt(dc, rc(x + 8, gy + 4, 80, 20), "CPU %", BL, self.f);
                txt(dc, rc(x + 96, gy + 4, 80, 20), "GPU %", PU, self.f);
                txt(dc, rc(w - 190, gy + 4, 170, 20), "6s / 240", BK, self.f);
                let px = x + 38;
                let py = gy + 30;
                let pw = rw - 48;
                let ph = gh - 44;
                for (v, yy) in [(100, py), (50, py + ph / 2), (0, py + ph)] {
                    txt(dc, rc(x + 4, yy - 8, 32, 16), &v.to_string(), BK, self.f);
                    let pen = CreatePen(PS_DOT, 1, BK);
                    let old = SelectObject(dc, pen);
                    MoveToEx(dc, px, yy, null_mut());
                    LineTo(dc, px + pw, yy);
                    SelectObject(dc, old);
                    DeleteObject(pen);
                }
                let ram = self.h.iter().filter_map(|h| h.r).max().unwrap_or(1).max(1) as f64;
                txt(
                    dc,
                    rc(x + 184, gy + 4, 300, 20),
                    &format!("RAM / {}", byt(ram)),
                    BR,
                    self.f,
                );
                for (g, col) in [(0, BL), (1, PU), (2, BR)] {
                    let pen = CreatePen(PS_SOLID, 1, col);
                    let old = SelectObject(dc, pen);
                    let mut last = None;
                    for (i, v) in self.h.iter().enumerate() {
                        let val = match g {
                            1 => v.g,
                            2 => v.r.map(|r| r as f64 / ram * 100.),
                            _ => v.c,
                        };
                        if let Some(n) = val {
                            let xx = px + (i as i32 * pw / 239);
                            let yy = py + ph - ((n.clamp(0., 100.) / 100.) * ph as f64) as i32;
                            if last.is_some_and(|t| v.ts - t <= 10) {
                                LineTo(dc, xx, yy);
                            } else {
                                MoveToEx(dc, xx, yy, null_mut());
                            }
                            last = Some(v.ts);
                        } else {
                            last = None;
                        }
                    }
                    SelectObject(dc, old);
                    DeleteObject(pen);
                }
                let dy = gy + gh + 10;
                self.btn(dc, rc(x, dy, 88, 28), "DIR +", I::DirAdd, WH);
                self.btn(dc, rc(x + 96, dy, 88, 28), "DIR -", I::DirDel, WH);
                self.btn(dc, rc(x + 192, dy, 88, 28), "OPEN", I::DirOpen, WH);
                txt(
                    dc,
                    rc(x + 292, dy, rw - 292, 28),
                    &format!("FS {} / {}", if z.ok { "READY" } else { "INIT" }, z.err),
                    if z.err > 0 { RD } else { BK },
                    self.f,
                );
                let nr = ((ht - dy - 72) / 22).max(1) as usize;
                self.doff = self.doff.min(ap.ds.len().saturating_sub(nr));
                for (i, p) in ap.ds.iter().enumerate().skip(self.doff).take(nr) {
                    let y = dy + 36 + (i - self.doff) as i32 * 22;
                    let sel = i == self.di;
                    fill(dc, rc(x, y, rw, 22), if sel { BK } else { WH });
                    txt(
                        dc,
                        rc(x + 4, y, rw - 8, 22),
                        p,
                        if sel { WH } else { BK },
                        self.f,
                    );
                    self.hits.push((rc(x, y, rw, 22), I::Dir(i)));
                }
            } else {
                txt(dc, rc(x, 64, rw, 36), "0 APP", BK, self.bf);
            }
            fill(dc, rc(0, ht - 28, w, 28), BK);
            let online = d::now().saturating_sub(self.hb) < 7;
            txt(
                dc,
                rc(12, ht - 26, w - 24, 24),
                &format!(
                    "BG {} | NET {} | GPU {} | FS {}",
                    if online { "ON" } else { "OFF" },
                    self.net,
                    self.gpu,
                    self.fs
                ),
                if online { GR } else { YL },
                self.f,
            );
            BitBlt(dst, 0, 0, w, ht, self.cv.dc, 0, 0, SRCCOPY);
        }
    }
    fn csv(&self, p: &Path) -> R<()> {
        let mut s = String::from(
            "\u{feff}id,nm,exe,ms,bytes,created,removed,cpu_avg,ram_avg,gpu_avg,up,dn,net_ms,fs_err\r\n",
        );
        for a in &self.apps {
            let v = d::s(&self.c, a.id);
            let z = d::z(&self.c, a.id);
            let esc = |x: &str| format!("\"{}\"", x.replace('"', "\"\""));
            s.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{}\r\n",
                a.id,
                esc(&a.nm),
                esc(&a.ex),
                v.ms,
                z.sz,
                z.ad,
                z.rm,
                v.avg_c().map(|v| v.to_string()).unwrap_or_default(),
                v.avg_r().map(|v| v.to_string()).unwrap_or_default(),
                v.avg_g().map(|v| v.to_string()).unwrap_or_default(),
                v.up,
                v.dn,
                v.nm,
                z.err
            ));
        }
        std::fs::write(p, s)?;
        Ok(())
    }
    unsafe fn act(&mut self, h: HWND, i: I) -> R<()> {
        unsafe {
            let ap = self.apps.get(self.sel).cloned();
            match i {
                I::Add => {
                    if let Some(p) = pick(h, false) {
                        let id = d::add(&self.c, &p)?;
                        self.load(true)?;
                        self.sel = self.apps.iter().position(|a| a.id == id).unwrap_or(0);
                        self.di = 0;
                    }
                }
                I::Sel(n) => {
                    self.sel = n;
                    self.di = 0;
                    self.doff = 0;
                }
                I::Dir(n) => {
                    self.di = n;
                }
                I::Run => {
                    if let Some(a) = ap {
                        std::process::Command::new(&a.ex)
                            .current_dir(Path::new(&a.ex).parent().unwrap_or(Path::new(".")))
                            .spawn()?;
                    }
                }
                I::Del => {
                    if let Some(a) = ap
                        && confirm(h, &format!("REMOVE {} ?", a.nm))
                    {
                        self.c.execute("DELETE FROM a WHERE id=?1", [a.id])?;
                    }
                }
                I::Pause => {
                    if let Some(a) = ap {
                        self.c.execute(
                            "UPDATE a SET en=?1 WHERE id=?2",
                            rusqlite::params![!a.on, a.id],
                        )?;
                    }
                }
                I::Auto => {
                    auto_set(!self.au, &self.p)?;
                    self.au = !self.au;
                }
                I::Bg => {
                    PostMessageW(h, WM_CLOSE, 0, 0);
                }
                I::Quit => {
                    d::kv(&self.c, "stop", "1")?;
                    PostMessageW(h, t::Q, 0, 0);
                }
                I::Export => {
                    if let Some(p) = pick(h, true) {
                        self.csv(&p)?;
                    }
                }
                I::DirAdd => {
                    if let Some(mut a) = ap
                        && let Some(p) = folder(h)
                        && !a.ds.contains(&p)
                    {
                        a.ds.push(p);
                        self.di = a.ds.len() - 1;
                        self.c.execute(
                            "UPDATE a SET ds=?1 WHERE id=?2",
                            rusqlite::params![serde_json::to_string(&a.ds)?, a.id],
                        )?;
                    }
                }
                I::DirDel => {
                    if let Some(mut a) = ap
                        && self.di < a.ds.len()
                        && confirm(h, "REMOVE DIR ?")
                    {
                        a.ds.remove(self.di);
                        self.c.execute(
                            "UPDATE a SET ds=?1 WHERE id=?2",
                            rusqlite::params![serde_json::to_string(&a.ds)?, a.id],
                        )?;
                        self.di = self.di.min(a.ds.len().saturating_sub(1));
                    }
                }
                I::DirOpen => {
                    if let Some(a) = ap
                        && let Some(p) = a.ds.get(self.di)
                    {
                        std::process::Command::new("explorer.exe").arg(p).spawn()?;
                    }
                }
                I::Adm => {
                    d::kv(&self.c, "stop", "1")?;
                    let p = self.p.clone();
                    std::thread::spawn(move || {
                        for _ in 0..40 {
                            if let Ok(c) = d::open(&p)
                                && d::val(&c, "bg_pid") == "0"
                            {
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(250));
                        }
                        if let Ok(ex) = std::env::current_exe() {
                            ShellExecuteW(
                                null_mut(),
                                w!("runas"),
                                wz(&ex.to_string_lossy()).as_ptr(),
                                wz(&format!("--bg --data \"{}\"", p.display())).as_ptr(),
                                null(),
                                SW_HIDE,
                            );
                        }
                    });
                }
            }
            self.load(true)?;
            InvalidateRect(h, null(), 0);
            Ok(())
        }
    }
}
impl Drop for U {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.f);
            DeleteObject(self.bf);
            if !self.fr.is_null() {
                RemoveFontMemResourceEx(self.fr);
            }
        }
    }
}
unsafe extern "system" fn wp(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
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
        let p = GetWindowLongPtrW(h, GWLP_USERDATA) as *mut U;
        if !p.is_null() {
            let u = &mut *p;
            if u.tr.as_ref().is_some_and(|t| t.msg != 0 && t.msg == m) {
                let ok = u.tr.as_mut().is_some_and(t::T::add);
                if !ok && u.hid {
                    u.hid = false;
                    SetTimer(h, 1, 2000, None);
                    ShowWindow(h, SW_SHOWNA);
                }
                return 0;
            }
            match m {
                WM_CREATE => {
                    u.tr = Some(t::T::new(h));
                    SetTimer(h, 1, 2000, None);
                    return 0;
                }
                WM_CLOSE => {
                    let ok = u.tr.as_mut().is_some_and(t::T::ready);
                    if ok {
                        KillTimer(h, 1);
                        u.hid = true;
                        u.cv = C::new();
                        ShowWindow(h, SW_HIDE);
                    } else {
                        ShowWindow(h, SW_MINIMIZE);
                    }
                    return 0;
                }
                t::M => {
                    if matches!(
                        l as u32 & 0xffff,
                        NIN_SELECT | 0x401 | WM_LBUTTONUP | WM_LBUTTONDBLCLK
                    ) {
                        u.hid = false;
                        let _ = u.load(true);
                        SetTimer(h, 1, 2000, None);
                        InvalidateRect(h, null(), 0);
                        ShowWindow(h, SW_RESTORE);
                        SetForegroundWindow(h);
                    }
                    return 0;
                }
                WM_COMMAND => {
                    match w as u32 & 0xffff {
                        t::OPEN => {
                            PostMessageW(h, t::M, 0, NIN_SELECT as isize);
                        }
                        t::HIDE => {
                            PostMessageW(h, WM_CLOSE, 0, 0);
                        }
                        t::QUIT => {
                            if let Err(e) = u.act(h, I::Quit) {
                                msg(h, &e.to_string());
                            }
                        }
                        _ => {}
                    }
                    return 0;
                }
                WM_TIMER => {
                    if w == 2 {
                        match u.tt {
                            0 => {
                                u.tt = 1;
                                ShowWindow(h, SW_SHOWMINNOACTIVE);
                                PostMessageW(h, WM_CLOSE, 0, 0);
                            }
                            1 => {
                                let ok = u.tr.as_ref().is_some_and(|t| t.ok);
                                if ok && (!u.hid || IsWindowVisible(h) != 0) {
                                    u.er = Some("TRAY HIDE".into());
                                }
                                u.tt = 2;
                                PostMessageW(h, t::M, 0, NIN_SELECT as isize);
                            }
                            2 => {
                                if IsWindowVisible(h) == 0 || u.hid {
                                    u.er = Some("TRAY OPEN".into());
                                }
                                u.tt = 3;
                                PostMessageW(h, WM_CLOSE, 0, 0);
                            }
                            _ => {
                                let ok = u.tr.as_ref().is_some_and(|t| t.ok);
                                if ok && (!u.hid || IsWindowVisible(h) != 0) {
                                    u.er = Some("TRAY CLOSE".into());
                                }
                                let text = if let Some(e) = &u.er {
                                    e.as_str()
                                } else if ok {
                                    "ok"
                                } else {
                                    "no-shell"
                                };
                                let _ = std::fs::write(u.p.join("ui.txt"), text);
                                KillTimer(h, 2);
                                PostMessageW(h, t::Q, 0, 0);
                            }
                        }
                        return 0;
                    }
                    match u.load(false) {
                        Ok(true) => {
                            InvalidateRect(h, null(), 0);
                        }
                        Err(e) => {
                            let _ = d::kv(&u.c, "ui_err", &e.to_string());
                        }
                        _ => {}
                    }
                    return 0;
                }
                WM_ERASEBKGND => return 1,
                WM_PAINT => {
                    let mut ps = PAINTSTRUCT::default();
                    let dc = BeginPaint(h, &mut ps);
                    let mut r = RECT::default();
                    GetClientRect(h, &mut r);
                    u.paint(h, dc, r.right, r.bottom);
                    EndPaint(h, &ps);
                    return 0;
                }
                WM_LBUTTONDOWN => {
                    let x = (l as u16) as i16 as i32;
                    let y = ((l as usize >> 16) as u16) as i16 as i32;
                    if let Some((_, i)) = u
                        .hits
                        .iter()
                        .find(|(r, _)| x >= r.left && x < r.right && y >= r.top && y < r.bottom)
                    {
                        let i = *i;
                        if let Err(e) = u.act(h, i) {
                            msg(h, &e.to_string());
                        }
                    }
                    return 0;
                }
                WM_MOUSEWHEEL => {
                    let down = ((w >> 16) as u16 as i16) < 0;
                    let mut pt = POINT {
                        x: (l as u16) as i16 as i32,
                        y: ((l as usize >> 16) as u16) as i16 as i32,
                    };
                    ScreenToClient(h, &mut pt);
                    if pt.x < 232 {
                        u.off = if down {
                            u.off.saturating_add(3)
                        } else {
                            u.off.saturating_sub(3)
                        };
                    } else {
                        u.doff = if down {
                            u.doff.saturating_add(3)
                        } else {
                            u.doff.saturating_sub(3)
                        };
                    }
                    InvalidateRect(h, null(), 0);
                    return 0;
                }
                WM_KEYDOWN => {
                    let i = match w as u16 {
                        VK_UP => Some(I::Sel(u.sel.saturating_sub(1))),
                        VK_DOWN => Some(I::Sel((u.sel + 1).min(u.apps.len().saturating_sub(1)))),
                        VK_DELETE => Some(I::Del),
                        _ => None,
                    };
                    if let Some(i) = i {
                        let _ = u.act(h, i);
                    }
                    return 0;
                }
                WM_SIZE => {
                    InvalidateRect(h, null(), 0);
                    return 0;
                }
                WM_GETMINMAXINFO => {
                    let mm = &mut *(l as *mut MINMAXINFO);
                    mm.ptMinTrackSize = POINT { x: 1180, y: 780 };
                    return 0;
                }
                WM_DESTROY => {
                    u.tr.take();
                    KillTimer(h, 2);
                    KillTimer(h, 1);
                    PostQuitMessage(0);
                    return 0;
                }
                _ => {}
            }
        }
        DefWindowProcW(h, m, w, l)
    }
}
pub fn run(p: &Path) -> R<()> {
    let (_lock, cls, old) = t::one(p)?;
    if old {
        return t::wake(&cls);
    }
    unsafe {
        SetProcessDPIAware();
        CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32);
        let mut u = Box::new(U::new(p)?);
        let smoke = std::env::args().any(|a| a == "--ui-test");
        if !smoke && d::now().saturating_sub(u.hb) > 6 {
            b::spawn(p)?;
        }
        let ins = GetModuleHandleW(null());

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wp),
            hInstance: ins,
            lpszClassName: cls.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let h = CreateWindowExW(
            0,
            cls.as_ptr(),
            w!("BYT"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1260,
            840,
            null_mut(),
            null_mut(),
            ins,
            (&mut *u as *mut U).cast(),
        );
        if h.is_null() {
            return Err(format!("WIN {}", GetLastError()).into());
        }
        if smoke {
            SetTimer(h, 2, 450, None);
        }
        let corner = 1u32;
        windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
            h,
            33,
            (&corner as *const u32).cast(),
            4,
        );
        let mut m = MSG::default();
        while GetMessageW(&mut m, null_mut(), 0, 0) > 0 {
            TranslateMessage(&m);
            DispatchMessageW(&m);
        }
        let er = u.er.take();
        drop(u);
        CoUninitialize();
        if let Some(e) = er {
            Err(e.into())
        } else {
            Ok(())
        }
    }
}
#[cfg(test)]
mod ut {
    #[test]
    fn fmt() {
        assert_eq!(super::byt(1024.), "1.00 KiB");
        assert_eq!(super::tm(3_661_000), "01:01:01");
        assert_eq!(super::pct(None), "N/A");
    }
}

#[cfg(test)]
mod res {
    #[test]
    fn font() {
        let b = include_bytes!("../../../res/f.ttf");
        let u16b = |i| u16::from_be_bytes([b[i], b[i + 1]]) as usize;
        let u32b = |i| u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize;
        assert_eq!(&b[..4], &[0, 1, 0, 0]);
        for i in 0..u16b(4) {
            let k = 12 + i * 16;
            assert!(u32b(k + 8) + u32b(k + 12) <= b.len());
        }
        assert!(include_str!("u.rs").is_ascii());
        let bad = [34u8, 63, 63, 34];
        assert!(!include_bytes!("u.rs").windows(4).any(|w| w == bad));
    }
}
