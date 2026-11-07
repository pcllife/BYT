#[cfg(all(windows, target_arch = "x86_64"))]
core::arch::global_asm!(include_str!("a.s"));
#[cfg(all(windows, target_arch = "x86_64"))]
unsafe extern "C" {
    fn bt_w(p: *const u16, n: usize) -> usize;
    fn bt_p(p: *mut u8, n: usize) -> u32;
    fn bt_i(p: *const u16, n: usize) -> u64;
    fn bt_e(a: *const u16, b: *const u16, n: usize) -> u32;
}
#[inline]
pub fn d(mut a: u64, b: u64) -> u64 {
    #[cfg(all(windows, target_arch = "x86_64"))]
    unsafe {
        core::arch::asm!("sub {a}, {b}","cmovb {a}, {z}",a=inout(reg)a,b=in(reg)b,z=in(reg)0u64,options(pure,nomem,nostack));
        a
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        a.saturating_sub(b)
    }
}
#[inline]
pub fn a(mut a: u64, b: u64) -> u64 {
    #[cfg(all(windows, target_arch = "x86_64"))]
    unsafe {
        core::arch::asm!("add {a}, {b}","cmovb {a}, {m}",a=inout(reg)a,b=in(reg)b,m=in(reg)u64::MAX,options(pure,nomem,nostack));
        a
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        a.saturating_add(b)
    }
}
pub fn w(s: &[u16]) -> usize {
    #[cfg(all(windows, target_arch = "x86_64"))]
    unsafe {
        bt_w(s.as_ptr(), s.len())
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        s.iter().position(|v| *v == 0).unwrap_or(s.len())
    }
}
#[doc = "# ascii - leave multibyte utf8 alone"]
pub fn p(s: &mut [u8]) -> bool {
    #[cfg(all(windows, target_arch = "x86_64"))]
    unsafe {
        bt_p(s.as_mut_ptr(), s.len()) != 0
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        let mut hi = false;
        for c in s {
            hi |= *c >= 128;
            *c = if *c == b'/' {
                b'\\'
            } else {
                c.to_ascii_lowercase()
            };
        }
        hi
    }
}
pub fn i(s: &[u16]) -> Option<(u32, usize)> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        let v = unsafe { bt_i(s.as_ptr(), s.len()) };
        if v == u64::MAX {
            None
        } else {
            Some((v as u32, (v >> 32) as usize))
        }
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        if !s.starts_with(&[112, 105, 100, 95]) {
            return None;
        }
        let mut v = 0u32;
        for (k, c) in s.iter().enumerate().skip(4) {
            if *c == 95 && k > 4 {
                return Some((v, k + 1));
            }
            let d = c.checked_sub(48)?;
            if d > 9 || k >= 14 {
                return None;
            }
            v = v.checked_mul(10)?.checked_add(d as u32)?;
        }
        None
    }
}
pub fn e(x: &[u16], y: &[u16]) -> bool {
    if x.len() == y.len() {
        #[cfg(all(windows, target_arch = "x86_64"))]
        {
            match unsafe { bt_e(x.as_ptr(), y.as_ptr(), x.len()) } {
                0 => return false,
                1 => return true,
                _ => {}
            }
        }
        #[cfg(not(all(windows, target_arch = "x86_64")))]
        {
            if x.iter().chain(y).all(|c| *c < 128) {
                return x
                    .iter()
                    .zip(y)
                    .all(|(a, b)| (*a as u8).eq_ignore_ascii_case(&(*b as u8)));
            }
        }
    } else if x.iter().chain(y).all(|c| *c < 128) {
        return false;
    }
    String::from_utf16_lossy(x).to_lowercase() == String::from_utf16_lossy(y).to_lowercase()
}
#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn sat() {
        for (x, y) in [(0, 0), (1, 2), (8, 3), (u64::MAX, 1), (u64::MAX, u64::MAX)] {
            assert_eq!(d(x, y), x.saturating_sub(y));
            assert_eq!(a(x, y), x.saturating_add(y));
        }
    }
    #[test]
    fn simd() {
        for n in 0..130 {
            for off in 0..16 {
                let mut v = vec![0u8; n + off];
                for (k, c) in v[off..].iter_mut().enumerate() {
                    *c = ((k * 73 + n) % 256) as u8;
                }
                let mut r = v[off..].to_vec();
                let hi = r.iter().any(|c| *c >= 128);
                for c in &mut r {
                    *c = if *c == 47 { 92 } else { c.to_ascii_lowercase() };
                }
                assert_eq!(p(&mut v[off..]), hi);
                assert_eq!(&v[off..], &r);
                let mut u = vec![65u16; n + off];
                assert_eq!(w(&u[off..]), n);
                for k in 0..n {
                    u[off + k] = 0;
                    assert_eq!(w(&u[off..]), k);
                    u[off + k] = 65;
                }
            }
        }
    }
    #[test]
    fn pid() {
        for (s, r) in [
            ("pid_0_x", Some((0, 6))),
            ("pid_4294967295_x", Some((u32::MAX, 15))),
            ("pid_4294967296_x", None),
            ("pid_01_x", Some((1, 7))),
            ("pid__x", None),
            ("pid_12", None),
            ("pid_12345678901_x", None),
            ("foo_1_x", None),
            ("", None),
        ] {
            assert_eq!(i(&s.encode_utf16().collect::<Vec<_>>()), r, "{s}");
        }
    }
    #[test]
    fn eq() {
        for (x, y) in [
            ("PowerShell.EXE", "powershell.exe"),
            ("APP[1].EXE", "app{1}.exe"),
            ("", ""),
            ("A", "B"),
            ("ABCDEFGHIJKLMNOPQ", "abcdefghijklmnopq"),
            ("\u{0130}.EXE", "i\u{0307}.exe"),
            ("\u{03a3}.exe", "\u{03c3}.exe"),
        ] {
            assert_eq!(
                e(
                    &x.encode_utf16().collect::<Vec<_>>(),
                    &y.encode_utf16().collect::<Vec<_>>()
                ),
                x.to_lowercase() == y.to_lowercase()
            );
        }
    }
}
