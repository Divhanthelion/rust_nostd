//! # core3: std habits, no_std replacements
//!
//! Five functions, five classic std habits that don't survive in no_std.
//! Some fixes are just paths; others need a different tool:
//!
//! 1. `HashMap` isn't in `alloc` at all (its default hasher wants OS
//!    randomness). Use the ordered map that *is* in `alloc`.
//! 2. `String`, `Vec` and `format!` are fine, but they come from the
//!    `alloc` crate, which a no_std crate must link explicitly with
//!    `extern crate alloc;`.
//! 3. `println!` needs an OS stdout. The function already receives a
//!    `core::fmt::Write` sink: write to that instead.
//! 4. `f64::sqrt` needs a platform math library. Integers have `isqrt`.
//!
//! The tests describe the exact expected behaviour. Don't change them.
#![no_std]

use std::collections::HashMap;
use std::fmt::Write;
use std::string::String;
use std::vec::Vec;

/// Count how many frames each CAN id produced.
pub fn histogram(ids: &[u16]) -> HashMap<u16, usize> {
    let mut map = HashMap::new();
    for &id in ids {
        *map.entry(id).or_insert(0) += 1;
    }
    map
}

/// A report line such as `"id 0x123: 4 frames"`.
pub fn report(id: u16, count: usize) -> String {
    format!("id {:#05x}: {} frames", id, count)
}

/// Write `warning: <msg>` followed by a newline to `out`.
pub fn log_warning(out: &mut impl Write, msg: &str) -> core::fmt::Result {
    println!("warning: {}", msg);
    Ok(())
}

/// Straight-line distance for an offset of (dx, dy) millimetres, rounded down.
pub fn distance_mm(dx: i32, dy: i32) -> u32 {
    ((dx as f64 * dx as f64 + dy as f64 * dy as f64).sqrt()) as u32
}

/// The even values, in order.
pub fn evens(xs: &[u32]) -> Vec<u32> {
    xs.iter().copied().filter(|x| x % 2 == 0).collect()
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::string::String;
    use std::vec;

    #[test]
    fn histogram_counts() {
        let h = histogram(&[0x100, 0x200, 0x100, 0x100]);
        assert_eq!(h.len(), 2);
        assert_eq!(h.get(&0x100), Some(&3));
        assert_eq!(h.get(&0x200), Some(&1));
        assert_eq!(h.get(&0x300), None);
    }

    #[test]
    fn report_formats() {
        assert_eq!(report(0x123, 4), "id 0x123: 4 frames");
        assert_eq!(report(0x7, 1), "id 0x007: 1 frames");
    }

    #[test]
    fn warnings_go_to_the_sink() {
        let mut s = String::new();
        log_warning(&mut s, "low battery").unwrap();
        log_warning(&mut s, "door open").unwrap();
        assert_eq!(s, "warning: low battery\nwarning: door open\n");
    }

    #[test]
    fn distances() {
        assert_eq!(distance_mm(3, 4), 5);
        assert_eq!(distance_mm(-6, 8), 10);
        assert_eq!(distance_mm(1, 1), 1);
        assert_eq!(distance_mm(0, 0), 0);
        assert_eq!(distance_mm(i32::MIN, i32::MIN), 3_037_000_499);
    }

    #[test]
    fn filters_evens() {
        assert_eq!(evens(&[1, 2, 3, 4, 10]), vec![2, 4, 10]);
        assert!(evens(&[1, 3]).is_empty());
    }
}
