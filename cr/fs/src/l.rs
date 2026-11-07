use byt_db::{self as d, A, R, Z};
use byt_k::a;
use notify::{
    Event, EventKind, RecursiveMode, Watcher,
    event::{ModifyKind, RenameMode},
};
use rusqlite::{Connection, params};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

type M = HashMap<String, u64>;
struct F {
    a: A,
    z: Z,
    m: M,
    dx: HashMap<String, Option<u64>>,
    base: bool,
}
fn under(p: &str, r: &str) -> bool {
    p == r
        || p.strip_prefix(r)
            .is_some_and(|s| r.ends_with('\\') || s.starts_with('\\'))
}
pub fn roots(ds: &[String]) -> Vec<String> {
    let mut v: Vec<_> = ds
        .iter()
        .map(|p| d::norm(Path::new(p)))
        .filter(|p| !p.is_empty())
        .collect();
    v.sort();
    v.dedup();
    let mut r: Vec<String> = Vec::new();
    for p in v {
        if !r.iter().any(|v| under(&p, v)) {
            r.push(p);
        }
    }
    r
}
fn scan(rs: &[String]) -> (M, u64) {
    let mut m = M::new();
    let mut er = 0;
    let mut q: Vec<PathBuf> = rs.iter().map(PathBuf::from).collect();
    while let Some(p) = q.pop() {
        let Ok(md) = std::fs::symlink_metadata(&p) else {
            er += 1;
            continue;
        };
        use std::os::windows::fs::MetadataExt;
        if md.file_attributes() & 0x400 != 0 {
            continue;
        }
        if md.is_file() {
            m.insert(d::norm(&p), md.len());
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&p) else {
            er += 1;
            continue;
        };
        for e in rd {
            match e {
                Ok(e) => q.push(e.path()),
                Err(_) => er += 1,
            }
        }
    }
    (m, er)
}
impl F {
    fn set(&mut self, p: String, n: Option<u64>, count: bool) {
        let old = self.m.get(&p).copied();
        if old == n {
            return;
        }
        let (o, v) = (old.unwrap_or(0), n.unwrap_or(0));
        if count {
            if v > o {
                self.z.ad = a::a(self.z.ad, v - o);
            } else {
                self.z.rm = a::a(self.z.rm, o - v);
            }
        }
        self.z.sz = a::a(a::d(self.z.sz, o), v);
        if let Some(n) = n {
            self.m.insert(p.clone(), n);
        } else {
            self.m.remove(&p);
        }
        self.z.cnt = self.m.len() as u64;
        self.z.ts = d::now();
        self.dx.insert(p, n);
    }
    fn sync(&mut self, count: bool) {
        let (m, er) = scan(&roots(&self.a.ds));
        self.z.err = er;
        if er == 0 {
            let old: Vec<_> = self
                .m
                .keys()
                .filter(|p| !m.contains_key(*p))
                .cloned()
                .collect();
            for p in old {
                self.set(p, None, count);
            }
        }
        for (p, n) in m {
            self.set(p, Some(n), count);
        }
        self.z.ok = true;
        self.z.ts = d::now();
    }
    fn rm(&mut self, p: &str, count: bool) {
        let ps: Vec<_> = self.m.keys().filter(|v| under(v, p)).cloned().collect();
        for p in ps {
            self.set(p, None, count);
        }
    }
    fn upd(&mut self, p: &Path) {
        if !self.a.on {
            return;
        }
        let ps = d::norm(p);
        if !roots(&self.a.ds).iter().any(|r| under(&ps, r)) {
            return;
        }
        match std::fs::symlink_metadata(p) {
            Ok(md) => {
                use std::os::windows::fs::MetadataExt;
                if md.file_attributes() & 0x400 != 0 {
                    return;
                }
                if md.is_file() {
                    self.set(ps, Some(md.len()), true);
                } else {
                    let (m, e) = scan(&[ps]);
                    self.z.err += e;
                    for (p, n) in m {
                        self.set(p, Some(n), true);
                    }
                }
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    self.z.err += 1;
                }
            }
        }
    }
    fn mv(&mut self, src: &Path, dst: &Path) {
        let (s, t) = (d::norm(src), d::norm(dst));
        let rs = roots(&self.a.ds);
        let si = rs.iter().any(|r| under(&s, r));
        let ti = rs.iter().any(|r| under(&t, r));
        if si && ti {
            let ps: Vec<_> = self
                .m
                .iter()
                .filter(|(p, _)| under(p, &s))
                .map(|(p, n)| (p.clone(), *n))
                .collect();
            if s != t && !ps.is_empty() {
                self.rm(&t, true);
            }
            for (p, n) in ps {
                let np = format!("{}{}", t, &p[s.len()..]);
                self.set(p, None, false);
                self.set(np, Some(n), false);
            }
            if dst.exists() {
                self.upd(dst);
            }
        } else {
            if si {
                self.rm(&s, true);
            }
            if ti && dst.exists() {
                self.upd(dst);
            }
        }
    }
    fn save(&mut self, c: &mut Connection) -> R<()> {
        let tx = c.transaction()?;
        if self.base {
            tx.execute("DELETE FROM f WHERE id=?1", [self.a.id])?;
        }
        {
            let mut put=tx.prepare_cached("INSERT INTO f(id,p,n) VALUES(?1,?2,?3) ON CONFLICT(id,p) DO UPDATE SET n=excluded.n")?;
            let mut del = tx.prepare_cached("DELETE FROM f WHERE id=?1 AND p=?2")?;
            if self.base {
                for (p, n) in &self.m {
                    put.execute(params![self.a.id, p, n])?;
                }
            } else {
                for (p, n) in &self.dx {
                    if let Some(n) = n {
                        put.execute(params![self.a.id, p, n])?;
                    } else {
                        del.execute(params![self.a.id, p])?;
                    }
                }
            }
        }
        tx.execute(
            "INSERT INTO z(id,v) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET v=excluded.v",
            params![self.a.id, serde_json::to_string(&self.z)?],
        )?;
        tx.commit()?;
        self.base = false;
        self.dx.clear();
        Ok(())
    }
}
#[doc = "# fs - watch changes, skip the busy scanning"]
pub struct W {
    pub tx: SyncSender<Vec<A>>,
    quit: Arc<AtomicBool>,
    th: Option<JoinHandle<()>>,
}
impl W {
    pub fn new(p: PathBuf) -> Self {
        let (tx, rx) = mpsc::sync_channel(1);
        let quit = Arc::new(AtomicBool::new(false));
        let q = quit.clone();
        let th = std::thread::spawn(move || {
            if let Err(e) = run(p.clone(), rx, q)
                && let Ok(c) = d::open(&p)
            {
                let _ = d::kv(&c, "fs", &format!("ERR {e}"));
            }
        });
        Self {
            tx,
            quit,
            th: Some(th),
        }
    }
}
impl Drop for W {
    fn drop(&mut self) {
        self.quit.store(true, Ordering::Relaxed);
        if let Some(h) = self.th.take() {
            let _ = h.join();
        }
    }
}
fn run(p: PathBuf, cfg: Receiver<Vec<A>>, quit: Arc<AtomicBool>) -> R<()> {
    let mut c = d::open(&p)?;
    let (et, er) = mpsc::sync_channel::<notify::Result<Event>>(4096);
    let of = Arc::new(AtomicBool::new(false));
    let of2 = of.clone();
    let mut w = notify::recommended_watcher(move |e| {
        if et.try_send(e).is_err() {
            of2.store(true, Ordering::Relaxed);
        }
    })?;
    let mut fs: HashMap<i64, F> = HashMap::new();
    let mut rs: HashSet<String> = HashSet::new();
    let mut flush = Instant::now();
    let mut ck = Instant::now();
    let mut rn: Vec<(PathBuf, Instant)> = Vec::new();
    while !quit.load(Ordering::Relaxed) {
        if let Ok(apps) = cfg.try_recv() {
            let wanted: HashSet<_> = apps
                .iter()
                .filter(|a| a.on)
                .flat_map(|a| roots(&a.ds))
                .collect();
            for r in rs.difference(&wanted) {
                let _ = w.unwatch(Path::new(r));
            }
            let mut we = 0;
            for r in wanted.difference(&rs) {
                if w.watch(Path::new(r), RecursiveMode::Recursive).is_err() {
                    we += 1;
                }
            }
            rs = wanted;
            fs.retain(|id, _| apps.iter().any(|a| a.id == *id));
            for ap in apps {
                if let Some(f) = fs.get_mut(&ap.id) {
                    if f.a.ds != ap.ds || f.a.on != ap.on {
                        f.a = ap;
                        if f.a.on {
                            f.m.clear();
                            f.z.sz = 0;
                            f.sync(false);
                            f.base = true;
                        }
                    } else {
                        f.a = ap;
                    }
                } else {
                    let z = d::z(&c, ap.id);
                    let known = z.ok;
                    let mut f = F {
                        a: ap,
                        z,
                        m: M::new(),
                        dx: HashMap::new(),
                        base: false,
                    };
                    let mut st = c.prepare_cached("SELECT p,n FROM f WHERE id=?1")?;
                    for row in st.query_map([f.a.id], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?))
                    })? {
                        let (p, n) = row?;
                        f.m.insert(p, n);
                    }
                    f.z.sz = f.m.values().copied().fold(0, a::a);
                    f.z.cnt = f.m.len() as u64;
                    if f.a.on {
                        f.sync(known);
                    }
                    fs.insert(f.a.id, f);
                }
            }
            d::kv(&c, "fs", if we == 0 { "WATCH" } else { "SCAN" })?;
            for f in fs.values_mut() {
                if f.a.on {
                    let _ = f.save(&mut c);
                }
            }
        }
        match er.recv_timeout(Duration::from_millis(250)) {
            Ok(Ok(e)) => {
                if e.need_rescan() {
                    of.store(true, Ordering::Relaxed);
                }
                if matches!(e.kind, EventKind::Access(_)) {
                    continue;
                }
                let from = matches!(
                    e.kind,
                    EventKind::Modify(ModifyKind::Name(RenameMode::From))
                );
                let to = matches!(e.kind, EventKind::Modify(ModifyKind::Name(RenameMode::To)));
                if from && e.paths.len() == 1 {
                    rn.push((e.paths[0].clone(), Instant::now()));
                } else if to && e.paths.len() == 1 {
                    let dst = &e.paths[0];
                    let t = d::norm(dst);
                    let idx = rn.iter().position(|(src, _)| {
                        let s = d::norm(src);
                        rs.iter().any(|r| under(&s, r) && under(&t, r))
                    });
                    if let Some(i) = idx {
                        let (src, _) = rn.remove(i);
                        for f in fs.values_mut().filter(|f| f.a.on) {
                            f.mv(&src, dst);
                        }
                    } else {
                        for f in fs.values_mut().filter(|f| f.a.on) {
                            f.upd(dst);
                        }
                    }
                } else {
                    for f in fs.values_mut().filter(|f| f.a.on) {
                        if matches!(
                            e.kind,
                            EventKind::Modify(ModifyKind::Name(RenameMode::Both))
                        ) && e.paths.len() == 2
                        {
                            f.mv(&e.paths[0], &e.paths[1]);
                        } else {
                            for p in &e.paths {
                                if matches!(e.kind, EventKind::Remove(_)) {
                                    f.rm(&d::norm(p), true);
                                } else {
                                    f.upd(p);
                                }
                            }
                        }
                    }
                }
            }
            Ok(Err(_)) => {
                of.store(true, Ordering::Relaxed);
            }
            _ => {}
        }
        let mut j = 0;
        while j < rn.len() {
            if rn[j].1.elapsed() > Duration::from_millis(500) {
                let (p, _) = rn.remove(j);
                for f in fs.values_mut().filter(|f| f.a.on) {
                    f.rm(&d::norm(&p), true);
                }
            } else {
                j += 1;
            }
        }
        if ck.elapsed() > Duration::from_secs(300) || of.swap(false, Ordering::Relaxed) {
            for f in fs.values_mut().filter(|f| f.a.on) {
                f.sync(true);
            }
            ck = Instant::now();
        }
        if flush.elapsed() > Duration::from_secs(3) {
            for f in fs.values_mut() {
                if !f.dx.is_empty() || f.base {
                    let _ = f.save(&mut c);
                }
            }
            flush = Instant::now();
        }
    }
    for (p, _) in rn {
        for f in fs.values_mut().filter(|f| f.a.on) {
            f.rm(&d::norm(&p), true);
        }
    }
    for f in fs.values_mut() {
        let _ = f.save(&mut c);
    }
    Ok(())
}
#[cfg(test)]
mod t {
    use super::*;
    fn f() -> F {
        F {
            a: A {
                id: 1,
                nm: String::new(),
                ex: String::new(),
                ds: vec![r"c:\a".into()],
                ts: 0,
                on: true,
            },
            z: Z::default(),
            m: M::new(),
            dx: HashMap::new(),
            base: false,
        }
    }
    #[test]
    fn delta() {
        let mut f = f();
        f.set("x".into(), Some(100), false);
        f.set("x".into(), Some(150), true);
        f.set("x".into(), Some(90), true);
        f.set("x".into(), None, true);
        assert_eq!((f.z.ad, f.z.rm, f.z.sz), (50, 150, 0));
    }
    #[test]
    fn root() {
        assert_eq!(
            roots(&[r"c:\a".into(), r"C:\a\b".into(), r"c:\ab".into()]),
            vec![r"c:\a", r"c:\ab"]
        );
    }
    #[test]
    fn rename() {
        let mut f = f();
        f.set(r"c:\a\old\x".into(), Some(100), false);
        f.mv(Path::new(r"c:\a\old"), Path::new(r"c:\a\new"));
        assert_eq!(f.z.ad, 0);
        assert!(f.m.contains_key(r"c:\a\new\x"));
    }
}

#[cfg(test)]
mod repl {
    use super::*;
    #[test]
    fn sizes() {
        for (old, new) in [(100, 60), (60, 100)] {
            let mut f = F {
                a: A {
                    id: 1,
                    nm: String::new(),
                    ex: String::new(),
                    ds: vec![r"c:\a".into()],
                    ts: 0,
                    on: true,
                },
                z: Z::default(),
                m: M::new(),
                dx: HashMap::new(),
                base: false,
            };
            f.set(r"c:\a\dst".into(), Some(old), false);
            f.set(r"c:\a\tmp".into(), Some(new), true);
            for _ in 0..2 {
                f.mv(Path::new(r"c:\a\tmp"), Path::new(r"c:\a\dst"));
                assert_eq!((f.z.sz, f.z.ad, f.z.rm, f.z.cnt), (new, new, old, 1));
            }
        }
    }
}
