#[doc = "# asm - keep the counter bits here"]
pub mod a;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
pub type R<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct A {
    pub id: i64,
    pub nm: String,
    pub ex: String,
    pub ds: Vec<String>,
    pub ts: u64,
    pub on: bool,
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct S {
    pub ms: u64,
    pub cs: f64,
    pub rs: f64,
    pub gs: f64,
    pub cm: u64,
    pub gm: u64,
    pub c: Option<f64>,
    pub r: Option<u64>,
    pub g: Option<f64>,
    pub pc: usize,
    pub up: u64,
    pub dn: u64,
    pub ts: u64,
    pub nm: u64,
    pub bad: usize,
}
impl S {
    pub fn avg_c(&self) -> Option<f64> {
        (self.cm > 0).then(|| self.cs / self.cm as f64)
    }
    pub fn avg_r(&self) -> Option<f64> {
        (self.cm > 0).then(|| self.rs / self.cm as f64)
    }
    pub fn avg_g(&self) -> Option<f64> {
        (self.gm > 0).then(|| self.gs / self.gm as f64)
    }
}
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Z {
    pub sz: u64,
    pub ad: u64,
    pub rm: u64,
    pub cnt: u64,
    pub ok: bool,
    pub err: u64,
    pub ts: u64,
}
#[derive(Clone, Default, Debug)]
pub struct H {
    pub ts: u64,
    pub c: Option<f64>,
    pub r: Option<u64>,
    pub g: Option<f64>,
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn dir() -> PathBuf {
    let v: Vec<_> = std::env::args_os().collect();
    if let Some(i) = v.iter().position(|v| v == "--data")
        && let Some(p) = v.get(i + 1)
    {
        return PathBuf::from(p);
    }
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("BYT")
}

pub fn norm(p: &Path) -> String {
    let s = p.to_string_lossy();
    let mut s = if let Some(s) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{}", s)
    } else {
        s.trim_start_matches(r"\\?\").to_string()
    };
    let hi = unsafe { a::p(s.as_mut_vec()) };
    if hi {
        s = s.to_lowercase();
    }
    while s.ends_with('\\') && s.len() > 3 {
        s.pop();
    }
    s
}
