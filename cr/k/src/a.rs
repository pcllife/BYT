#[cfg(all(windows, target_arch = "x86_64"))]
core::arch::global_asm!(include_str!("a.s"));
#[cfg(all(windows, target_arch = "x86_64"))]
unsafe extern "C" {
    fn bt_d(a: u64, b: u64) -> u64;
    fn bt_a(a: u64, b: u64) -> u64;
}
pub fn d(a: u64, b: u64) -> u64 {
    #[cfg(all(windows, target_arch = "x86_64"))]
    unsafe {
        bt_d(a, b)
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        a.saturating_sub(b)
    }
}
pub fn a(a: u64, b: u64) -> u64 {
    #[cfg(all(windows, target_arch = "x86_64"))]
    unsafe {
        bt_a(a, b)
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        a.saturating_add(b)
    }
}
#[cfg(test)]
mod t {
    #[test]
    fn asm() {
        for (x, y) in [(0, 0), (1, 2), (8, 3), (u64::MAX, 1), (u64::MAX, u64::MAX)] {
            assert_eq!(super::d(x, y), x.saturating_sub(y));
            assert_eq!(super::a(x, y), x.saturating_add(y));
        }
    }
}
