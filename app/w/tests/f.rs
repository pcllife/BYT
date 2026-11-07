use byt_db::{self as d, R, Z};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

fn snap(z: &Z) -> [u64; 4] {
    [z.sz, z.ad, z.rm, z.cnt]
}
fn root(p: &Path) -> bool {
    p.parent() == Some(std::env::temp_dir().as_path())
        && p.file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("byt_fc_"))
}
fn dl(p: &Path, two: bool) -> R<Value> {
    let addr = fs::read_to_string(p.join("addr"))?;
    let mut s = TcpStream::connect(addr.trim())?;
    s.set_read_timeout(Some(Duration::from_secs(8)))?;
    let req = format!(
        "GET /{} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        if two { "two" } else { "one" }
    );
    s.write_all(req.as_bytes())?;
    let mut rd = BufReader::new(s);
    let mut line = String::new();
    let mut len = 0u64;
    loop {
        line.clear();
        if rd.read_line(&mut line)? == 0 {
            return Err("HTTP EOF".into());
        }
        if line == "\r\n" {
            break;
        }
        if let Some(v) = line.strip_prefix("Content-Length: ") {
            len = v.trim().parse()?;
        }
    }
    let files = p.join("files");
    let mut f = File::create(files.join("d.part"))?;
    let (mut total, mut sum) = (0u64, 0u64);
    let mut b = [0u8; 16384];
    loop {
        let n = rd.read(&mut b)?;
        if n == 0 {
            break;
        }
        f.write_all(&b[..n])?;
        total += n as u64;
        sum += b[..n].iter().map(|v| *v as u64).sum::<u64>();
        thread::sleep(Duration::from_millis(2));
    }
    f.sync_all()?;
    drop(f);
    if total != len {
        return Err("HTTP LEN".into());
    }
    fs::rename(files.join("d.part"), files.join("d.bin"))?;
    Ok(json!({"bytes":total,"sum":sum,"request":req.len()}))
}
fn op(p: &Path, k: &str) -> R<Value> {
    let f = p.join("files");
    let mut out = json!({});
    match k {
        "create" => fs::write(f.join("c.bin"), vec![1u8; 4096])?,
        "download" => out = dl(p, false)?,
        "overwrite" => out = dl(p, true)?,
        "rename" => fs::rename(f.join("c.bin"), f.join("r.bin"))?,
        "append" => OpenOptions::new()
            .append(true)
            .open(f.join("r.bin"))?
            .write_all(&[2u8; 2048])?,
        "truncate" => OpenOptions::new()
            .write(true)
            .open(f.join("r.bin"))?
            .set_len(1024)?,
        "delete_dl" => fs::remove_file(f.join("d.bin"))?,
        "delete_seed" => fs::remove_file(f.join("seed.bin"))?,
        "delete_file" => fs::remove_file(f.join("r.bin"))?,
        "batch" => {
            fs::create_dir_all(f.join("batch/sub"))?;
            for n in 1..=10 {
                fs::write(f.join(format!("batch/sub/{n}.bin")), vec![3u8; n * 1024])?;
                thread::sleep(Duration::from_millis(5));
            }
        }
        "delete_batch" => fs::remove_dir_all(f.join("batch"))?,
        "burst" => {
            for n in 0..128 {
                let q = f.join(format!("tmp_{n}"));
                fs::write(&q, [7u8; 4096])?;
                fs::remove_file(q)?;
            }
        }
        _ => return Err("OP".into()),
    }
    Ok(out)
}
#[test]
#[ignore = "fixture entry"]
fn io() {
    let Some(p) = std::env::var_os("BYT_FC") else {
        return;
    };
    let p = PathBuf::from(p);
    assert!(root(&p));
    let end = Instant::now() + Duration::from_secs(180);
    while !p.join("exit").exists() {
        assert!(Instant::now() < end, "fixture timeout");
        if let Ok(s) = fs::read_to_string(p.join("cmd")) {
            fs::remove_file(p.join("cmd")).unwrap();
            let v: Value = serde_json::from_str(&s).unwrap();
            let out = match op(&p, v["op"].as_str().unwrap()) {
                Ok(v) => json!({"id":v.get("id").cloned().unwrap_or(Value::Null),"out":v}),
                Err(e) => json!({"err":e.to_string()}),
            };
            let out = json!({"id":v["id"],"result":out});
            fs::write(p.join("done.tmp"), serde_json::to_vec(&out).unwrap()).unwrap();
            fs::rename(p.join("done.tmp"), p.join("done")).unwrap();
        }
        thread::sleep(Duration::from_millis(10));
    }
}
struct F {
    p: PathBuf,
    c: Connection,
    id: i64,
    ch: Child,
    bg: Option<JoinHandle<R<()>>>,
    srv: Option<JoinHandle<()>>,
    quit: Arc<AtomicBool>,
    seq: u64,
    log: Vec<Value>,
    tag: &'static str,
}
impl F {
    fn new(tag: &'static str) -> Self {
        let p = std::env::temp_dir().join(format!(
            "byt_fc_{}_{}_{}",
            std::process::id(),
            d::now(),
            tag
        ));
        assert!(root(&p));
        fs::create_dir_all(p.join("files")).unwrap();
        fs::write(p.join("files/seed.bin"), [0u8; 8192]).unwrap();
        let exe = p.join("cli.exe");
        fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
        let c = d::open(&p).unwrap();
        let id = d::add(&c, &exe).unwrap();
        c.execute(
            "UPDATE a SET ds=?1 WHERE id=?2",
            params![
                serde_json::to_string(&vec![d::norm(&p.join("files"))]).unwrap(),
                id
            ],
        )
        .unwrap();
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.set_nonblocking(true).unwrap();
        fs::write(p.join("addr"), l.local_addr().unwrap().to_string()).unwrap();
        let ch = Command::new(exe)
            .args(["--ignored", "--exact", "io", "--nocapture"])
            .env("BYT_FC", &p)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        let quit = Arc::new(AtomicBool::new(false));
        let q = quit.clone();
        let srv = thread::spawn(move || {
            while !q.load(Ordering::Relaxed) {
                match l.accept() {
                    Ok((mut s, _)) => {
                        s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                        s.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
                        let mut rd = BufReader::new(s.try_clone().unwrap());
                        let mut line = String::new();
                        rd.read_line(&mut line).unwrap();
                        let two = line.starts_with("GET /two ");
                        loop {
                            line.clear();
                            if rd.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                                break;
                            }
                        }
                        let n = if two { 262144 } else { 1048576 };
                        write!(
                            s,
                            "HTTP/1.1 200 OK\r\nContent-Length: {n}\r\nConnection: close\r\n\r\n"
                        )
                        .unwrap();
                        s.write_all(&vec![if two { 165 } else { 90 }; n]).unwrap();
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => panic!("server {e}"),
                }
            }
        });
        let pp = p.clone();
        let bg = thread::spawn(move || byt_bg::run(&pp, None));
        let mut f = Self {
            p,
            c,
            id,
            ch,
            bg: Some(bg),
            srv: Some(srv),
            quit,
            seq: 0,
            log: vec![],
            tag,
        };
        f.wait(|f| d::z(&f.c, f.id).ok && d::s(&f.c, f.id).pc > 0);
        assert_eq!(snap(&d::z(&f.c, f.id)), [8192, 0, 0, 1]);
        f
    }
    fn wait(&mut self, mut pred: impl FnMut(&Self) -> bool) {
        let end = Instant::now() + Duration::from_secs(16);
        loop {
            if pred(self) {
                return;
            }
            assert!(self.ch.try_wait().unwrap().is_none(), "client exit");
            assert!(
                Instant::now() < end,
                "timeout: {:?}",
                snap(&d::z(&self.c, self.id))
            );
            thread::sleep(Duration::from_millis(50));
        }
    }
    fn cmd(&mut self, k: &str) -> Value {
        let _ = fs::remove_file(self.p.join("done"));
        self.seq += 1;
        fs::write(
            self.p.join("cmd.tmp"),
            serde_json::to_vec(&json!({"id":self.seq,"op":k})).unwrap(),
        )
        .unwrap();
        fs::rename(self.p.join("cmd.tmp"), self.p.join("cmd")).unwrap();
        self.wait(|f| {
            fs::read_to_string(f.p.join("done"))
                .ok()
                .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                .is_some_and(|v| v["id"].as_u64() == Some(f.seq))
        });
        let v: Value = serde_json::from_slice(&fs::read(self.p.join("done")).unwrap()).unwrap();
        assert!(v["result"]["err"].is_null(), "{v}");
        v["result"]["out"].clone()
    }
    fn save(&self) {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.local");
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join(format!("f_{}.json",self.tag)),serde_json::to_vec_pretty(&json!({"root":self.p,"steps":self.log,"s":d::s(&self.c,self.id),"z":d::z(&self.c,self.id),"net":d::val(&self.c,"net")})).unwrap()).unwrap();
    }
    fn step(&mut self, k: &str, expected: [u64; 4]) -> Value {
        let t = Instant::now();
        let out = self.cmd(k);
        let end = Instant::now() + Duration::from_secs(12);
        loop {
            let z = d::z(&self.c, self.id);
            let key = k != "rename"
                || self
                    .c
                    .query_row(
                        "SELECT COUNT(*) FROM f WHERE id=?1 AND p=?2",
                        params![self.id, d::norm(&self.p.join("files/r.bin"))],
                        |r| r.get::<_, i64>(0),
                    )
                    .unwrap()
                    == 1;
            if snap(&z) == expected && key {
                break;
            }
            if Instant::now() >= end {
                self.log
                    .push(json!({"op":k,"expected":expected,"actual":snap(&z),"ok":false}));
                self.save();
                assert_eq!(snap(&z), expected, "{k}");
                panic!("{k} index");
            }
            thread::sleep(Duration::from_millis(50));
        }
        let z = d::z(&self.c, self.id);
        println!(
            "{k}: expected={expected:?} actual={:?} lag_ms={}",
            snap(&z),
            t.elapsed().as_millis()
        );
        self.log.push(json!({"op":k,"expected":expected,"actual":snap(&z),"ok":true,"lag_ms":t.elapsed().as_millis(),"out":out}));
        self.save();
        out
    }
}
impl Drop for F {
    fn drop(&mut self) {
        let _ = fs::write(self.p.join("exit"), []);
        let _ = d::kv(&self.c, "stop", "1");
        if self.ch.try_wait().ok().flatten().is_none() {
            let _ = self.ch.kill();
            let _ = self.ch.wait();
        }
        self.quit.store(true, Ordering::Relaxed);
        if let Some(t) = self.srv.take() {
            let _ = t.join();
        }
        if let Some(t) = self.bg.take() {
            let _ = t.join();
        }
        if let Ok(c) = Connection::open_in_memory() {
            drop(std::mem::replace(&mut self.c, c));
        }
        if !thread::panicking() && root(&self.p) {
            let _ = fs::remove_dir_all(&self.p);
        }
    }
}
#[test]
fn flow() {
    let mut f = F::new("flow");
    f.step("create", [12288, 4096, 0, 2]);
    let v = f.step("download", [1060864, 1052672, 0, 3]);
    assert_eq!(v["bytes"], 1048576);
    assert_eq!(v["sum"], 94371840);
    f.step("rename", [1060864, 1052672, 0, 3]);
    f.step("append", [1062912, 1054720, 0, 3]);
    f.step("truncate", [1057792, 1054720, 5120, 3]);
    let v = f.step("overwrite", [271360, 1316864, 1053696, 3]);
    assert_eq!(v["bytes"], 262144);
    assert_eq!(v["sum"], 43253760);
    let v = f.step("download", [1057792, 2365440, 1315840, 3]);
    assert_eq!(v["bytes"], 1048576);
    f.step("delete_dl", [9216, 2365440, 2364416, 2]);
    f.step("delete_seed", [1024, 2365440, 2372608, 1]);
    f.step("delete_file", [0, 2365440, 2373632, 0]);
    f.step("batch", [56320, 2421760, 2373632, 10]);
    f.step("delete_batch", [0, 2421760, 2429952, 0]);
    let net = d::val(&f.c, "net");
    if net.starts_with("ETW") {
        f.wait(|f| d::s(&f.c, f.id).dn >= 2359296);
        let s = d::s(&f.c, f.id);
        assert!(s.up > 0 && s.up < 16384, "client upload {}", s.up);
        println!("network: up={} dn={} {net}", s.up, s.dn);
    } else {
        println!("network: N/A {net}");
    }
    f.save();
}
#[test]
#[ignore = "diagnostic: short-lived files may lose size"]
fn fast() {
    let mut f = F::new("fast");
    f.step("burst", [8192, 524288, 524288, 1]);
}
