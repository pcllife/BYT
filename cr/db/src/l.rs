pub use byt_k::*;
use rusqlite::{Connection, params};
use std::{path::Path, time::Duration};
#[doc = "# db - batch the writes and move on"]
pub fn open(p: &Path) -> R<Connection> {
    std::fs::create_dir_all(p)?;
    let c = Connection::open(p.join("d.db"))?;
    c.busy_timeout(Duration::from_secs(5))?;
    c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;
        CREATE TABLE IF NOT EXISTS a(id INTEGER PRIMARY KEY,nm TEXT NOT NULL,ex TEXT NOT NULL UNIQUE,ds TEXT NOT NULL,ts INTEGER NOT NULL,en INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS s(id INTEGER PRIMARY KEY REFERENCES a(id) ON DELETE CASCADE,v TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS z(id INTEGER PRIMARY KEY REFERENCES a(id) ON DELETE CASCADE,v TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS f(id INTEGER NOT NULL REFERENCES a(id) ON DELETE CASCADE,p TEXT NOT NULL,n INTEGER NOT NULL,PRIMARY KEY(id,p)) WITHOUT ROWID;
        CREATE TABLE IF NOT EXISTS h(id INTEGER NOT NULL REFERENCES a(id) ON DELETE CASCADE,t INTEGER NOT NULL,c REAL,r INTEGER,g REAL,PRIMARY KEY(id,t)) WITHOUT ROWID;
        CREATE TABLE IF NOT EXISTS k(k TEXT PRIMARY KEY,v TEXT NOT NULL) WITHOUT ROWID;
        PRAGMA user_version=1;")?;
    Ok(c)
}
pub fn list(c: &Connection) -> R<Vec<A>> {
    let mut q = c.prepare_cached("SELECT id,nm,ex,ds,ts,en FROM a ORDER BY id")?;
    let v = q.query_map([], |r| {
        Ok(A {
            id: r.get(0)?,
            nm: r.get(1)?,
            ex: r.get(2)?,
            ds: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default(),
            ts: r.get(4)?,
            on: r.get(5)?,
        })
    })?;
    Ok(v.collect::<Result<Vec<_>, _>>()?)
}
pub fn add(c: &Connection, p: &Path) -> R<i64> {
    let p = p.canonicalize()?;
    if !p.is_file() || !p.extension().is_some_and(|s| s.eq_ignore_ascii_case("exe")) {
        return Err("EXE".into());
    }
    let ex = norm(&p);
    if list(c)?.iter().any(|v| v.ex == ex) {
        return Err("DUP".into());
    }
    let nm = p
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let ds = vec![norm(p.parent().ok_or("DIR")?)];
    c.execute(
        "INSERT INTO a(nm,ex,ds,ts,en) VALUES(?1,?2,?3,?4,1)",
        params![nm, ex, serde_json::to_string(&ds)?, now()],
    )?;
    Ok(c.last_insert_rowid())
}
pub fn s(c: &Connection, id: i64) -> S {
    get(c, "s", id).unwrap_or_default()
}
pub fn z(c: &Connection, id: i64) -> Z {
    get(c, "z", id).unwrap_or_default()
}
fn get<T: serde::de::DeserializeOwned>(c: &Connection, t: &str, id: i64) -> Option<T> {
    let s: String = c
        .query_row(&format!("SELECT v FROM {t} WHERE id=?1"), [id], |r| {
            r.get(0)
        })
        .ok()?;
    serde_json::from_str(&s).ok()
}
pub fn kv(c: &Connection, k: &str, v: &str) -> R<()> {
    c.execute(
        "INSERT INTO k(k,v) VALUES(?1,?2) ON CONFLICT(k) DO UPDATE SET v=excluded.v",
        params![k, v],
    )?;
    Ok(())
}
pub fn val(c: &Connection, k: &str) -> String {
    c.query_row("SELECT v FROM k WHERE k=?1", [k], |r| r.get(0))
        .unwrap_or_default()
}
pub fn hist(c: &Connection, id: i64) -> Vec<H> {
    let Ok(mut q) = c.prepare_cached("SELECT t,c,r,g FROM (SELECT t,c,r,g FROM h WHERE id=?1 ORDER BY t DESC LIMIT 240) ORDER BY t") else { return vec![]; };
    let Ok(v) = q.query_map([id], |r| {
        Ok(H {
            ts: r.get(0)?,
            c: r.get(1)?,
            r: r.get(2)?,
            g: r.get(3)?,
        })
    }) else {
        return vec![];
    };
    v.filter_map(Result::ok).collect()
}
#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn db() {
        let p = std::env::temp_dir().join(format!("byt_db_{}_{}", std::process::id(), now()));
        let c = open(&p).unwrap();
        assert!(list(&c).unwrap().is_empty());
        kv(&c, "a", "b").unwrap();
        assert_eq!(val(&c, "a"), "b");
        c.execute("INSERT INTO a VALUES(1,'x','x','[]',0,1)", [])
            .unwrap();
        c.execute("INSERT INTO s VALUES(1,'{}')", []).unwrap();
        c.execute("DELETE FROM a WHERE id=1", []).unwrap();
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM s", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        drop(c);
        std::fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn avg() {
        let s = S {
            cs: 2000.,
            rs: 10000.,
            cm: 1000,
            gs: 600.,
            gm: 200,
            ..Default::default()
        };
        assert_eq!(s.avg_c(), Some(2.));
        assert_eq!(s.avg_g(), Some(3.));
        assert_eq!(S::default().avg_c(), None);
    }
    #[test]
    fn path() {
        assert_eq!(norm(Path::new(r"\\?\C:\A\B\")), r"c:\a\b");
    }
}
