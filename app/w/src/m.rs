#![cfg_attr(windows, windows_subsystem = "windows")]
use byt_bg as b;
use byt_db as d;
mod t;
mod u;
fn run(p: &std::path::Path) -> d::R<()> {
    let a: Vec<_> = std::env::args().collect();
    let arg = |k: &str| a.iter().position(|s| s == k).and_then(|i| a.get(i + 1));
    if a.iter().any(|s| s == "--bg") {
        let ticks = arg("--ticks").and_then(|v| v.parse().ok());
        return if ticks.is_none() && b::side().is_some() {
            b::spawn(p)
        } else {
            b::run(p, ticks)
        };
    }
    let c = d::open(p)?;
    if a.iter().any(|s| s == "--stop") {
        return d::kv(&c, "stop", "1");
    }
    if let Some(v) = arg("--add") {
        let id = d::add(&c, std::path::Path::new(v))?;
        let ds: Vec<_> = a
            .windows(2)
            .filter(|s| s[0] == "--dir")
            .map(|s| d::norm(std::path::Path::new(&s[1])))
            .collect();
        if !ds.is_empty() {
            c.execute(
                "UPDATE a SET ds=?1 WHERE id=?2",
                rusqlite::params![serde_json::to_string(&ds)?, id],
            )?;
        }
        return Ok(());
    }
    if let Some(v) = arg("--dump") {
        let apps = d::list(&c)?;
        let data: Vec<_> = apps
            .iter()
            .map(|a| serde_json::json!({"a":a,"s":d::s(&c,a.id),"z":d::z(&c,a.id)}))
            .collect();
        std::fs::write(
            v,
            serde_json::to_vec_pretty(
                &serde_json::json!({"apps":data,"net":d::val(&c,"net"),"gpu":d::val(&c,"gpu"),"fs":d::val(&c,"fs")}),
            )?,
        )?;
        return Ok(());
    }
    if a.iter().any(|s| s == "--ck") {
        let v: String = c.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        if v != "ok" {
            return Err(v.into());
        }
        if c.prepare("PRAGMA foreign_key_check")?
            .query([])?
            .next()?
            .is_some()
        {
            return Err("FK".into());
        }
        std::fs::write(p.join("ck.txt"), "ok")?;
        return Ok(());
    }
    drop(c);
    u::run(p)
}
fn main() {
    let p = d::dir();
    if let Err(e) = run(&p) {
        let _ = std::fs::create_dir_all(&p);
        let _ = std::fs::write(p.join("err.txt"), e.to_string());
        std::process::exit(1);
    }
}
