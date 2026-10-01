//! # fmt2: Display, Debug and friends, done properly
//!
//! Implement formatting traits that respect the caller's format spec
//! (`{:>12}`, `{:.1}`, `{:#x}`…), without floats and without allocation.
//!
//! - `Milli(i32)`: thousandths, e.g. `Milli(21500)` is 21.5. `{}` shows 3
//!   decimals; `{:.N}` shows N decimals (N ≤ 3; larger behaves like 3),
//!   **rounding half away from zero**. Never print `-0`: the sign appears only if
//!   the *rounded* value is non-zero. Width/alignment apply to the whole number.
//! - `Reading`: `S<sensor>: <value> <unit>`, where the value is formatted like
//!   `Milli` with the same precision, and width/alignment apply to the whole line.
//! - `Debug for Reading`: `Reading { sensor: 1, value: 21.500, unit: Celsius }`.
//! - `CanId`: Display as `0x123` (3 hex digits) for standard ids ≤ 0x7FF, else
//!   `0x1abcdef0`-style (8 digits). `{:x}`/`{:#x}`/`{:08x}` behave exactly like on
//!   a plain `u32`.
//! - `Flags`: Display the set bits' names joined with `|` (bit 0 first), or `-`.
//!
//! Tip: build text in the provided `StackBuf`, then hand it to `f.pad(..)`.
#![no_std]

use core::fmt::{self, Write};

/// A small fixed buffer implementing `fmt::Write` (you built this in fmt1).
pub struct StackBuf {
    buf: [u8; 48],
    len: usize,
}

impl StackBuf {
    pub fn new() -> Self {
        StackBuf { buf: [0; 48], len: 0 }
    }
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
}

impl Write for StackBuf {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len.checked_add(s.len()).filter(|&e| e <= self.buf.len()).ok_or(fmt::Error)?;
        self.buf[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

pub struct Milli(pub i32);

impl Milli {
    /// Write the number with `precision` decimals (0..=3) into `out`.
    fn write_to(&self, out: &mut impl Write, precision: usize) -> fmt::Result {
        let p = precision.min(3) as u32;
        let scale = 10u64.pow(3 - p);
        let abs = u64::from(self.0.unsigned_abs());
        let rounded = (abs + scale / 2) / scale;
        let pow = 10u64.pow(p);
        let (int, frac) = (rounded / pow, rounded % pow);
        if self.0 < 0 && rounded != 0 {
            out.write_char('-')?;
        }
        if p == 0 {
            write!(out, "{int}")
        } else {
            write!(out, "{int}.{frac:0width$}", width = p as usize)
        }
    }
}

impl fmt::Display for Milli {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut b = StackBuf::new();
        self.write_to(&mut b, f.precision().unwrap_or(3))?;
        // `pad` would treat precision as "max characters" and truncate, so use
        // `pad_integral`, which handles width, fill, alignment, `+` and `0`.
        match b.as_str().strip_prefix('-') {
            Some(digits) => f.pad_integral(false, "", digits),
            None => f.pad_integral(true, "", b.as_str()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Celsius,
    Volt,
    Rpm,
}

impl Unit {
    pub fn symbol(self) -> &'static str {
        match self {
            Unit::Celsius => "°C",
            Unit::Volt => "V",
            Unit::Rpm => "rpm",
        }
    }
}

pub struct Reading {
    pub sensor: u8,
    pub milli: i32,
    pub unit: Unit,
}

impl fmt::Display for Reading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut b = StackBuf::new();
        write!(b, "S{}: ", self.sensor)?;
        Milli(self.milli).write_to(&mut b, f.precision().unwrap_or(3))?;
        write!(b, " {}", self.unit.symbol())?;
        // `pad` would also apply precision (as truncation!), so pad by hand:
        let width = f.width().unwrap_or(0);
        let len = b.as_str().chars().count();
        let fill = width.saturating_sub(len);
        let (before, after) = match f.align() {
            Some(fmt::Alignment::Right) => (fill, 0),
            Some(fmt::Alignment::Center) => (fill / 2, fill - fill / 2),
            _ => (0, fill),
        };
        for _ in 0..before {
            f.write_char(f.fill())?;
        }
        f.write_str(b.as_str())?;
        for _ in 0..after {
            f.write_char(f.fill())?;
        }
        Ok(())
    }
}

impl fmt::Debug for Reading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reading")
            .field("sensor", &self.sensor)
            .field("value", &format_args!("{}", Milli(self.milli)))
            .field("unit", &self.unit)
            .finish()
    }
}

#[derive(Clone, Copy)]
pub struct CanId(pub u32);

impl fmt::Display for CanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 <= 0x7FF {
            write!(f, "{:#05x}", self.0)
        } else {
            write!(f, "{:#010x}", self.0)
        }
    }
}

impl fmt::LowerHex for CanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

/// Status flags; bit i has the name `NAMES[i]`.
pub struct Flags(pub u8);

impl Flags {
    pub const NAMES: [&'static str; 8] = ["ENGINE", "DOOR", "BELT", "ABS", "ESP", "TPMS", "OIL", "BATT"];
}

impl fmt::Display for Flags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            return f.write_str("-");
        }
        let mut first = true;
        for (i, name) in Self::NAMES.iter().enumerate() {
            if self.0 & (1 << i) != 0 {
                if !first {
                    f.write_char('|')?;
                }
                f.write_str(name)?;
                first = false;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::format;

    #[test]
    fn milli_default_precision() {
        assert_eq!(format!("{}", Milli(21500)), "21.500");
        assert_eq!(format!("{}", Milli(-1250)), "-1.250");
        assert_eq!(format!("{}", Milli(7)), "0.007");
        assert_eq!(format!("{}", Milli(i32::MIN)), "-2147483.648");
    }

    #[test]
    fn milli_precision_rounds_half_away() {
        assert_eq!(format!("{:.1}", Milli(21550)), "21.6");
        assert_eq!(format!("{:.1}", Milli(21549)), "21.5");
        assert_eq!(format!("{:.1}", Milli(-1250)), "-1.3");
        assert_eq!(format!("{:.0}", Milli(21500)), "22");
        assert_eq!(format!("{:.2}", Milli(999)), "1.00");
        assert_eq!(format!("{:.5}", Milli(1)), "0.001");
    }

    #[test]
    fn milli_never_negative_zero() {
        assert_eq!(format!("{:.0}", Milli(-400)), "0");
        assert_eq!(format!("{:.1}", Milli(-49)), "0.0");
        assert_eq!(format!("{:.1}", Milli(-50)), "-0.1");
    }

    #[test]
    fn milli_width() {
        assert_eq!(format!("[{:>8.1}]", Milli(-1250)), "[    -1.3]");
        assert_eq!(format!("[{:<7}]", Milli(5)), "[0.005  ]");
        assert_eq!(format!("[{:+.1}]", Milli(1250)), "[+1.3]");
        assert_eq!(format!("[{:08.1}]", Milli(-1250)), "[-00001.3]");
    }

    #[test]
    fn readings() {
        let r = Reading { sensor: 1, milli: 21500, unit: Unit::Celsius };
        assert_eq!(format!("{}", r), "S1: 21.500 °C");
        assert_eq!(format!("{:.1}", r), "S1: 21.5 °C");
        assert_eq!(format!("[{:>14.1}]", r), "[   S1: 21.5 °C]");
        assert_eq!(format!("[{:*^13.0}]", Reading { sensor: 2, milli: 12000, unit: Unit::Volt }), "[**S2: 12 V***]");
    }

    #[test]
    fn debug_reading() {
        let r = Reading { sensor: 3, milli: -40000, unit: Unit::Rpm };
        assert_eq!(format!("{:?}", r), "Reading { sensor: 3, value: -40.000, unit: Rpm }");
        assert!(format!("{:#?}", r).contains("\n    value: -40.000,\n"));
    }

    #[test]
    fn can_ids() {
        assert_eq!(format!("{}", CanId(0x123)), "0x123");
        assert_eq!(format!("{}", CanId(0x7)), "0x007");
        assert_eq!(format!("{}", CanId(0x1ABC_DEF0)), "0x1abcdef0");
        assert_eq!(format!("{}", CanId(0x800)), "0x00000800");
        assert_eq!(format!("{:x}", CanId(0x1A)), "1a");
        assert_eq!(format!("{:#x}", CanId(0x1A)), "0x1a");
        assert_eq!(format!("{:08x}", CanId(0x1A)), "0000001a");
    }

    #[test]
    fn flags() {
        assert_eq!(format!("{}", Flags(0)), "-");
        assert_eq!(format!("{}", Flags(0b0000_0001)), "ENGINE");
        assert_eq!(format!("{}", Flags(0b1000_0110)), "DOOR|BELT|BATT");
    }
}
