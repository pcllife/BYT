use std::{io::Write, time::Duration};
#[test]
fn bg_fs() {
    let p = std::env::temp_dir().join(format!("byt_it_{}_{}", std::process::id(), byt_db::now()));
    let dir = p.join("files");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("b"), vec![0u8; 100]).unwrap();
    let c = byt_db::open(&p).unwrap();
    let id = byt_db::add(&c, &std::env::current_exe().unwrap()).unwrap();
    c.execute(
        "UPDATE a SET ds=?1 WHERE id=?2",
        rusqlite::params![
            serde_json::to_string(&vec![byt_db::norm(&dir)]).unwrap(),
            id
        ],
    )
    .unwrap();
    let pp = p.clone();
    let t = std::thread::spawn(move || byt_bg::run(&pp, Some(5)));
    for _ in 0..80 {
        if byt_db::z(&c, id).ok {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(byt_db::z(&c, id).ok);
    std::fs::rename(dir.join("b"), dir.join("c")).unwrap();
    std::thread::sleep(Duration::from_millis(350));
    std::fs::write(dir.join("n"), vec![1u8; 50]).unwrap();
    std::thread::sleep(Duration::from_millis(350));
    std::fs::OpenOptions::new()
        .append(true)
        .open(dir.join("n"))
        .unwrap()
        .write_all(&[2u8; 25])
        .unwrap();
    std::thread::sleep(Duration::from_millis(350));
    std::fs::OpenOptions::new()
        .write(true)
        .open(dir.join("n"))
        .unwrap()
        .set_len(20)
        .unwrap();
    std::thread::sleep(Duration::from_millis(350));
    std::fs::remove_file(dir.join("n")).unwrap();
    t.join().unwrap().unwrap();
    let z = byt_db::z(&c, id);
    let s = byt_db::s(&c, id);
    assert_eq!((z.sz, z.ad, z.rm, z.cnt), (100, 75, 75, 1));
    assert!(s.ms >= 6000);
    assert!(s.avg_r().unwrap_or(0.) > 0.);
    assert!(!byt_db::hist(&c, id).is_empty());
    assert_eq!(byt_db::val(&c, "bg_pid"), "0");
    drop(c);
    assert!(p.starts_with(std::env::temp_dir()));
    std::fs::remove_dir_all(p).unwrap();
}
