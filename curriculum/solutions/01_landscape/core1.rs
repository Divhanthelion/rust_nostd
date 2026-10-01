//! # core1: Leaving std behind
//!
//! This file is a `#![no_std]` library written by someone who still thinks in
//! `std::`. It doesn't compile here: the checker builds your code against a
//! sysroot that contains **no std at all**, exactly like a microcontroller
//! target would.
//!
//! **Your task:** make it compile **without changing any logic**, by fixing
//! the paths only.
//!
//! - Everything std re-exports from `core` lives at the same path under `core::`.
//! - `nostd where <Item>` tells you where an item lives.
//! - The test module at the bottom may use std: tests run on your PC.
//!
//! Done when the no_std build and the tests pass.
#![no_std]

use core::cmp::{max, min};
use core::fmt;
use core::mem;
use core::num::ParseIntError;
use core::str::FromStr;

/// Clamp `value` into `lo..=hi`.
pub fn clamp_i32(value: i32, lo: i32, hi: i32) -> i32 {
    max(lo, min(value, hi))
}

/// Swap the two bytes of a pair in place.
pub fn swap_pair(pair: &mut (u8, u8)) {
    mem::swap(&mut pair.0, &mut pair.1);
}

/// A sensor frame with C layout, as it might arrive from a DMA buffer.
#[repr(C)]
pub struct SensorFrame {
    pub id: u16,
    pub flags: u8,
    pub value: i32,
}

/// Size in bytes of a [`SensorFrame`] (padding included).
pub fn frame_size() -> usize {
    mem::size_of::<SensorFrame>()
}

/// A temperature in tenths of a degree Celsius, e.g. `DeciCelsius(215)` = 21.5 °C.
pub struct DeciCelsius(pub i16);

impl fmt::Display for DeciCelsius {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let v = i32::from(self.0);
        let sign = if v < 0 { "-" } else { "" };
        write!(f, "{}{}.{} °C", sign, v.abs() / 10, v.abs() % 10)
    }
}

/// Parse a decimal device id such as `" 42 "`.
pub fn parse_id(s: &str) -> Result<u16, ParseIntError> {
    u16::from_str(s.trim())
}

#[cfg(test)]
mod tests {
    // Tests run on the host, where std exists. This is the usual pattern.
    extern crate std;
    use super::*;
    use std::string::ToString;

    #[test]
    fn clamps() {
        assert_eq!(clamp_i32(5, 0, 10), 5);
        assert_eq!(clamp_i32(-3, 0, 10), 0);
        assert_eq!(clamp_i32(99, 0, 10), 10);
    }

    #[test]
    fn swaps() {
        let mut p = (1, 2);
        swap_pair(&mut p);
        assert_eq!(p, (2, 1));
    }

    #[test]
    fn frame_has_padding() {
        // 2 (id) + 1 (flags) + 1 (padding) + 4 (value)
        assert_eq!(frame_size(), 8);
    }

    #[test]
    fn displays_temperatures() {
        assert_eq!(DeciCelsius(215).to_string(), "21.5 °C");
        assert_eq!(DeciCelsius(-5).to_string(), "-0.5 °C");
        assert_eq!(DeciCelsius(0).to_string(), "0.0 °C");
    }

    #[test]
    fn parses_ids() {
        assert_eq!(parse_id(" 42 "), Ok(42));
        assert!(parse_id("70000").is_err());
        assert!(parse_id("abc").is_err());
    }
}
