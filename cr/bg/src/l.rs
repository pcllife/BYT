pub mod t;
use byt_db::{self as d, R, S};
use byt_fs::W;
use byt_k::a;
use byt_os::{
    g::G,
    n::N,
    p::{self, P, Q},
};
use rusqlite::params;
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    os::windows::process::CommandExt,
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};
use windows_sys::Win32::{Foundation::*, System::Threading::*};
pub fn side() -> Option<std::path::PathBuf> {
    let ex = std::env::current_exe().ok()?;
    let p = ex.parent()?.join("bg.exe");
    p.is_file().then_some(p)
}
pub fn entry() -> R<std::path::PathBuf> {
    Ok(side().unwrap_or(std::env::current_exe()?))
}
pub fn spawn(p: &Path) -> R<()> {
    let ex = std::env::current_exe()?;
    let side = side();
    let mut c = Command::new(side.as_ref().unwrap_or(&ex));
    c.args(["--bg", "--data"]).arg(p);
    if side.is_some() {
        c.arg("--ui").arg(&ex);
    }
    c.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()?;
    if side.is_some() {
        for _ in 0..100 {
            if !t::win(p, true).is_null() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    Ok(())
}
#[doc = "# bg - one loop for each data dir"]
pub fn run(p: &Path, ticks: Option<u64>) -> R<()> {
    let mut hs = std::collections::hash_map::DefaultHasher::new();
    d::norm(p).hash(&mut hs);
    let nm: Vec<u16> = format!("Local\\BYT_BG_{:x}", hs.finish())
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mu = Q(unsafe { CreateMutexW(std::ptr::null(), 0, nm.as_ptr()) });
    if mu.0.is_null() {
        return Err("MUTEX".into());
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return Ok(());
    }
    let mut c = d::open(p)?;
    d::kv(&c, "stop", "0")?;
    d::kv(&c, "bg_pid", &std::process::id().to_string())?;
    let fs = W::new(p.to_path_buf());
    let mut pr = P::new();
    let mut gpu = G::new();
    let mut net: Option<N> = None;
    let mut ss: HashMap<i64, S> = HashMap::new();
    let mut cfg = String::new();
    let mut n = 0u64;
    let mut prev = p::clock();
    let exe = std::env::current_exe()?;
    let own = d::norm(&exe);
    let args: Vec<_> = std::env::args_os().collect();
    let host = if exe
        .file_name()
        .is_some_and(|v| v.eq_ignore_ascii_case("bg.exe"))
    {
        args.iter()
            .position(|a| a == "--ui")
            .and_then(|i| args.get(i + 1))
            .map(|p| {
                let p = std::path::PathBuf::from(p);
                d::norm(&p.canonicalize().unwrap_or(p))
            })
    } else {
        None
    };
    loop {
        let at = std::time::Instant::now();
        let now = d::now();
        let apps = d::list(&c)?;
        let text = serde_json::to_string(&apps)?;
        if cfg != text && fs.tx.try_send(apps.clone()).is_ok() {
            cfg = text;
        }
        ss.retain(|id, _| apps.iter().any(|a| a.id == *id));
        for ap in &apps {
            ss.entry(ap.id).or_insert_with(|| d::s(&c, ap.id));
        }
        if apps.iter().any(|a| a.on) && net.is_none() {
            net = Some(N::new());
        }
        if !apps.iter().any(|a| a.on) {
            net = None;
        }
        let mut roots = Vec::new();
        if let Some(host) = &host
            && !apps.iter().any(|a| a.ex == own)
            && let Some(a) = apps.iter().find(|a| a.on && a.ex == *host)
        {
            let mut a = a.clone();
            a.ex = own.clone();
            roots = apps.clone();
            roots.push(a);
        }
        let ps = pr.read(if roots.is_empty() { &apps } else { &roots });
        if let Some(net) = &net {
            net.ids(&pr.ids);
        }
        let gs = gpu.read(&pr.ids);
        let nt = net.as_ref().map(N::take).unwrap_or_default();
        let t = p::clock();
        let dt = a::d(t, prev);
        prev = t;
        for ap in &apps {
            let s = ss.get_mut(&ap.id).unwrap();
            s.ts = now;
            s.pc = 0;
            s.c = None;
            s.r = None;
            s.g = None;
            s.bad = 0;
            if !ap.on {
                continue;
            }
            if let Some(v) = ps.get(&ap.id) {
                s.pc = v.pc;
                s.bad = v.bad;
                if v.pc > 0 && dt > 0 {
                    s.ms = a::a(s.ms, dt);
                    if v.ok > 0 {
                        let cpu =
                            (v.ct as f64 / (dt as f64 * 10_000. * pr.cores) * 100.).clamp(0., 100.);
                        s.c = Some(cpu);
                        s.r = Some(v.ram);
                        s.cs += cpu * dt as f64;
                        s.rs += v.ram as f64 * dt as f64;
                        s.cm = a::a(s.cm, dt);
                    }
                    if let Some(gs) = &gs {
                        let g = gs.get(&ap.id).copied().unwrap_or(0.);
                        s.g = Some(g);
                        s.gs += g * dt as f64;
                        s.gm = a::a(s.gm, dt);
                    }
                    if net.as_ref().is_some_and(|n| n.err == 0) {
                        s.nm = a::a(s.nm, dt);
                    }
                }
            }
            if let Some((up, dn)) = nt.get(&ap.id) {
                s.up = a::a(s.up, *up);
                s.dn = a::a(s.dn, *dn);
            }
        }
        if n.is_multiple_of(15)
            && let Some(net) = &mut net
        {
            net.poll();
        }
        let tx = c.transaction()?;
        {
            let mut q = tx.prepare_cached(
                "INSERT INTO s(id,v) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET v=excluded.v",
            )?;
            for (id, s) in &ss {
                q.execute(params![id, serde_json::to_string(s)?])?;
            }
        }
        if n.is_multiple_of(3) {
            let mut q =
                tx.prepare_cached("INSERT OR REPLACE INTO h(id,t,c,r,g) VALUES(?1,?2,?3,?4,?5)")?;
            for (id, s) in &ss {
                if s.pc > 0 {
                    q.execute(params![id, now, s.c, s.r, s.g])?;
                }
            }
        }
        d::kv(&tx, "hb", &now.to_string())?;
        d::kv(
            &tx,
            "net",
            &net.as_ref()
                .map(|n| {
                    if n.err == 0 {
                        format!("ETW / LOST {}", n.lost)
                    } else {
                        format!("N/A / {}", n.err)
                    }
                })
                .unwrap_or_else(|| "IDLE".into()),
        )?;
        d::kv(
            &tx,
            "gpu",
            if gs.is_some() {
                gpu.kind
            } else if pr.ids.is_empty() {
                "IDLE"
            } else {
                "N/A"
            },
        )?;
        if n.is_multiple_of(1800) {
            tx.execute("DELETE FROM h WHERE t<?1", [now.saturating_sub(7 * 86400)])?;
        }
        tx.commit()?;
        n += 1;
        if d::val(&c, "stop") == "1" || ticks.is_some_and(|v| n >= v) {
            break;
        }
        std::thread::sleep(Duration::from_secs(2).saturating_sub(at.elapsed()));
    }
    drop(net);
    drop(fs);
    d::kv(&c, "hb", "0")?;
    d::kv(&c, "bg_pid", "0")?;
    c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
    Ok(())
}
