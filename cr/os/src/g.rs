use std::collections::HashMap;
#[path = "g_1.rs"]
mod pd;
#[path = "g_2.rs"]
mod v2;
pub type M = HashMap<i64, f64>;
pub struct G {
    n: Option<v2::N>,
    p: Option<pd::G>,
    tried: bool,
    pub err: u32,
    pub kind: &'static str,
}
impl Default for G {
    fn default() -> Self {
        Self::new()
    }
}
impl G {
    pub fn new() -> Self {
        Self {
            n: None,
            p: None,
            tried: false,
            err: 0,
            kind: "IDLE",
        }
    }
    pub fn read(&mut self, ids: &HashMap<u32, i64>) -> Option<M> {
        if ids.is_empty() {
            self.p = None;
            self.n = None;
            self.tried = false;
            self.kind = "IDLE";
            return None;
        }
        if !self.tried {
            self.n = v2::N::new();
            self.tried = true;
        }
        if let Some(n) = &mut self.n {
            match n.read(ids) {
                Ok(v) => {
                    self.kind = "V2";
                    return v;
                }
                Err(e) => {
                    self.err = e;
                    self.n = None;
                }
            }
        }
        let p = self.p.get_or_insert_with(pd::G::new);
        let v = p.read(ids);
        self.err = p.err;
        self.kind = if v.is_some() { "PDH" } else { "N/A" };
        v
    }
}
