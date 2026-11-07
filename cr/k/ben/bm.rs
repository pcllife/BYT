use byt_k::{a, norm};
use std::{hint::black_box, path::Path, time::Instant};
fn old(p: &Path) -> String {
    let s = p.to_string_lossy();
    let s = if let Some(s) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{}", s)
    } else {
        s.trim_start_matches(r"\\?\").to_string()
    };
    let mut s = s.replace('/', "\\").to_lowercase();
    while s.ends_with('\\') && s.len() > 3 {
        s.pop();
    }
    s
}
fn tick(n: usize, f: &mut impl FnMut(usize) -> u64) -> f64 {
    let t = Instant::now();
    let mut sum = 0;
    for i in 0..n {
        sum ^= f(black_box(i));
    }
    black_box(sum);
    t.elapsed().as_nanos() as f64 / n as f64
}
fn bm(tag: &str, n: usize, mut x: impl FnMut(usize) -> u64, mut y: impl FnMut(usize) -> u64) {
    for i in 0..128 {
        black_box(x(i));
        black_box(y(i));
    }
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for i in 0..9 {
        if i % 2 == 0 {
            a.push(tick(n, &mut x));
            b.push(tick(n, &mut y));
        } else {
            b.push(tick(n, &mut y));
            a.push(tick(n, &mut x));
        }
    }
    a.sort_by(f64::total_cmp);
    b.sort_by(f64::total_cmp);
    println!(
        "{tag}: old_ns={:.2} asm_ns={:.2} speedup={:.2}x",
        a[4],
        b[4],
        a[4] / b[4]
    );
}
fn main() {
    let paths: Vec<String> = (0..128)
        .map(|i| {
            format!(
                "C:/Program Files/APP_{i}/Data/Cache/{}/FILE_{i}.BIN",
                if i % 16 == 0 {
                    "\u{0130}\u{4e2d}\u{6587}".to_string()
                } else {
                    "UPPER_CASE_DIRECTORY_".repeat(i % 7 + 1)
                }
            )
        })
        .collect();
    for p in &paths {
        assert_eq!(old(Path::new(p)), norm(Path::new(p)));
    }
    bm(
        "path",
        100_000,
        |i| old(Path::new(black_box(&paths[i % 128]))).len() as u64,
        |i| norm(Path::new(black_box(&paths[i % 128]))).len() as u64,
    );
    let names: Vec<Vec<u16>> = (0..128)
        .map(|i| {
            let mut v: Vec<u16> = format!("APP_{i}_WORKER.EXE").encode_utf16().collect();
            v.resize(260, 0);
            v
        })
        .collect();
    bm(
        "utf16",
        500_000,
        |i| {
            black_box(&names[i % 128])
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(260) as u64
        },
        |i| a::w(black_box(&names[i % 128])) as u64,
    );
    let pid: Vec<Vec<u16>> = (0..128)
        .map(|i| {
            format!(
                "pid_{}_luid_0x00000000_0x00001234_phys_0_eng_0_engtype_3D\0",
                i * 31 + 1
            )
            .encode_utf16()
            .collect()
        })
        .collect();
    bm(
        "gpu_pid",
        300_000,
        |i| {
            let p = black_box(&pid[i % 128]);
            let n = p.iter().position(|c| *c == 0).unwrap_or(p.len());
            let s = String::from_utf16_lossy(&p[..n]);
            let x = s
                .strip_prefix("pid_")
                .and_then(|s| s.split_once('_'))
                .and_then(|(s, _)| s.parse::<u32>().ok())
                .unwrap();
            x as u64
        },
        |i| a::i(black_box(&pid[i % 128])).unwrap().0 as u64,
    );
    let target: Vec<u16> = "powershell.exe".encode_utf16().collect();
    let procs: Vec<Vec<u16>> = [
        "POWERSHELL.EXE",
        "PowerShell.exe",
        "PowerShell.EXE",
        "powershell.exe",
    ]
    .iter()
    .map(|s| s.encode_utf16().collect())
    .collect();
    bm(
        "proc_name",
        300_000,
        |i| {
            String::from_utf16_lossy(black_box(&procs[i % 4]))
                .to_lowercase()
                .eq_ignore_ascii_case("powershell.exe") as u64
        },
        |i| a::e(black_box(&procs[i % 4]), black_box(&target)) as u64,
    );
}
