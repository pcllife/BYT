#![cfg_attr(windows, windows_subsystem = "windows")]
mod w;
fn main() {
    let p = byt_db::dir();
    if let Err(e) = w::run(&p) {
        let _ = std::fs::create_dir_all(&p);
        let _ = std::fs::write(p.join("err.txt"), e.to_string());
        std::process::exit(1);
    }
}
