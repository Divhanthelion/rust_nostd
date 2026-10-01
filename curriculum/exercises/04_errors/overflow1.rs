//! # overflow1: Choosing overflow semantics
//!
//! Each function is written the naive way and fails its tests with an
//! overflow panic (tests run with overflow checks on, like a debug build).
//! Fix each one by stating the *intended* behaviour explicitly with
//! `wrapping_*`, `saturating_*`, `checked_*`, `TryFrom`, or a wider type.
//!
//! The timer functions model a free-running 32-bit millisecond tick counter
//! that wraps from `u32::MAX` back to 0 every ~49.7 days.
#![no_std]

use core::num::TryFromIntError;

/// Milliseconds elapsed since `start`, correct across one wrap of the counter.
pub fn ticks_elapsed(now: u32, start: u32) -> u32 {
    now - start
}

/// Has `deadline` been reached at time `now`? Both are tick values on the
/// wrapping clock, and the true distance between them is less than 2^31.
/// (Hint: look at the *sign* of the wrapped difference.)
pub fn deadline_reached(now: u32, deadline: u32) -> bool {
    now >= deadline
}

/// Apply a gain (in 1/256 units) to an ADC reading, clamping at `u16::MAX`.
/// `gain = 256` means ×1.0.
pub fn apply_gain(raw: u16, gain: u16) -> u16 {
    raw * gain / 256
}

/// Add a calibration offset; `None` if the result doesn't fit.
pub fn add_offset(value: i16, offset: i16) -> Option<i16> {
    Some(value + offset)
}

/// 16-bit additive checksum: the sum of all bytes modulo 2^16.
pub fn checksum16(data: &[u8]) -> u16 {
    data.iter().map(|&b| b as u16).sum()
}

/// Convert to a byte, failing instead of truncating.
pub fn to_u8(x: i32) -> Result<u8, TryFromIntError> {
    Ok(x as u8)
}

/// The midpoint of two values, rounded down, without overflowing.
pub fn midpoint(a: u32, b: u32) -> u32 {
    (a + b) / 2
}

/// Sequence numbers are 8-bit and wrap. How many frames were lost between
/// `prev` and `next`? (Consecutive frames: `next == prev + 1` → 0 lost.)
pub fn frames_lost(prev: u8, next: u8) -> u8 {
    next - prev - 1
}

/// The full 64-bit product of two u32 values, as (high word, low word).
pub fn mul_full(a: u32, b: u32) -> (u32, u32) {
    let p = a * b;
    (0, p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_across_wrap() {
        assert_eq!(ticks_elapsed(1500, 1000), 500);
        assert_eq!(ticks_elapsed(5, u32::MAX - 4), 10);
        assert_eq!(ticks_elapsed(0, 0), 0);
    }

    #[test]
    fn deadlines() {
        assert!(deadline_reached(1000, 1000));
        assert!(deadline_reached(1001, 1000));
        assert!(!deadline_reached(999, 1000));
        // deadline just after the wrap, now just before it: not yet
        assert!(!deadline_reached(u32::MAX - 10, 5));
        // now has wrapped past the deadline
        assert!(deadline_reached(20, u32::MAX - 10));
    }

    #[test]
    fn gains() {
        assert_eq!(apply_gain(1000, 256), 1000);
        assert_eq!(apply_gain(1000, 512), 2000);
        assert_eq!(apply_gain(1000, 128), 500);
        assert_eq!(apply_gain(60000, 512), u16::MAX);
        assert_eq!(apply_gain(u16::MAX, u16::MAX), u16::MAX);
    }

    #[test]
    fn offsets() {
        assert_eq!(add_offset(100, -50), Some(50));
        assert_eq!(add_offset(i16::MAX, 1), None);
        assert_eq!(add_offset(i16::MIN, -1), None);
    }

    #[test]
    fn checksums() {
        assert_eq!(checksum16(&[1, 2, 3]), 6);
        assert_eq!(checksum16(&[0xFF; 300]), (300u32 * 255 % 65536) as u16);
        assert_eq!(checksum16(&[]), 0);
    }

    #[test]
    fn conversions() {
        assert_eq!(to_u8(200), Ok(200));
        assert!(to_u8(300).is_err());
        assert!(to_u8(-1).is_err());
    }

    #[test]
    fn midpoints() {
        assert_eq!(midpoint(2, 4), 3);
        assert_eq!(midpoint(3, 4), 3);
        assert_eq!(midpoint(u32::MAX, u32::MAX), u32::MAX);
        assert_eq!(midpoint(u32::MAX, u32::MAX - 2), u32::MAX - 1);
        assert_eq!(midpoint(0, u32::MAX), u32::MAX / 2);
    }

    #[test]
    fn sequence_numbers() {
        assert_eq!(frames_lost(10, 11), 0);
        assert_eq!(frames_lost(10, 14), 3);
        assert_eq!(frames_lost(255, 0), 0);
        assert_eq!(frames_lost(250, 2), 7);
    }

    #[test]
    fn full_products() {
        assert_eq!(mul_full(3, 4), (0, 12));
        assert_eq!(mul_full(u32::MAX, u32::MAX), (0xFFFF_FFFE, 0x0000_0001));
        assert_eq!(mul_full(0x1_0000, 0x1_0000), (1, 0));
    }
}
