//! # lints1: Hardening a crate with lints
//!
//! The crate attributes below are what a safety-oriented team might enforce
//! on a logic library. The code compiles fine without them, but with them it
//! fails in several ways. Make it pass **without removing or weakening any
//! lint attribute**, and without changing behaviour (the tests pin it down).
//!
//! Expect to: document every public item, replace an `unsafe` shortcut with a
//! safe equivalent, stop ignoring results, remove pointless casts and
//! qualifications, spell out elided lifetimes, and derive `Debug`.
//!
//! (In a real project you'd add clippy's restriction lints too, e.g.
//! `clippy::unwrap_used` and `clippy::indexing_slicing`; rustc alone doesn't
//! run them.)
#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(unused_must_use, unused_results)]
#![deny(trivial_casts, trivial_numeric_casts)]
#![deny(missing_debug_implementations)]
#![deny(unused_qualifications)]
#![deny(elided_lifetimes_in_paths)]

//! A tiny telemetry-record codec.

use core::cmp::max;
use core::fmt::{self, Write};

/// One telemetry record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Record {
    /// Sensor channel.
    pub channel: u8,
    /// Value in milli-units.
    pub value: i32,
}

/// Errors while decoding records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// Fewer than 5 bytes.
    TooShort,
    /// The name bytes aren't UTF-8.
    BadName,
}

/// Encode a record as 5 bytes: channel, then the value little-endian.
pub fn encode(r: &Record) -> [u8; 5] {
    let v = r.value.to_le_bytes();
    [r.channel, v[0], v[1], v[2], v[3]]
}

/// Decode 5 bytes produced by [`encode`].
pub fn decode(bytes: &[u8]) -> Result<Record, DecodeError> {
    let (head, _) = bytes.split_first_chunk::<5>().ok_or(DecodeError::TooShort)?;
    let [channel, a, b, c, d] = *head;
    Ok(Record { channel, value: i32::from_le_bytes([a, b, c, d]) })
}

/// Interpret bytes as a channel name.
pub fn channel_name(bytes: &[u8]) -> Result<&str, DecodeError> {
    core::str::from_utf8(bytes).map_err(|_| DecodeError::BadName)
}

/// The larger of two record values.
pub fn peak(a: &Record, b: &Record) -> i32 {
    max(a.value, b.value)
}

/// Render `ch<channel>=<value>` into any writer.
pub fn render(out: &mut dyn Write, r: &Record) -> fmt::Result {
    write!(out, "ch{}=", r.channel)?;
    write!(out, "{}", r.value)
}

/// Display wrapper for a record.
#[derive(Debug)]
pub struct Show<'a>(pub &'a Record);

impl fmt::Display for Show<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        render(f, self.0)
    }
}

/// Swap two records' values, returning nothing.
pub fn swap_values(a: &mut Record, b: &mut Record) {
    core::mem::swap(&mut a.value, &mut b.value);
}

/// Total of the record values, saturating.
pub fn total(records: &[Record]) -> i32 {
    records.iter().fold(0i32, |acc, r| acc.saturating_add(r.value))
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::string::{String, ToString};

    const R: Record = Record { channel: 7, value: -1234 };

    #[test]
    fn round_trip() {
        assert_eq!(decode(&encode(&R)), Ok(R));
        assert_eq!(decode(&[1, 2, 3]), Err(DecodeError::TooShort));
    }

    #[test]
    fn names() {
        assert_eq!(channel_name(b"oil"), Ok("oil"));
        assert_eq!(channel_name(&[0xFF]), Err(DecodeError::BadName));
    }

    #[test]
    fn rendering() {
        let mut s = String::new();
        render(&mut s, &R).unwrap();
        assert_eq!(s, "ch7=-1234");
        assert_eq!(Show(&R).to_string(), "ch7=-1234");
    }

    #[test]
    fn values() {
        let mut a = R;
        let mut b = Record { channel: 1, value: 50 };
        assert_eq!(peak(&a, &b), 50);
        swap_values(&mut a, &mut b);
        assert_eq!((a.value, b.value), (50, -1234));
        assert_eq!(total(&[a, b]), -1184);
        assert_eq!(total(&[Record { channel: 0, value: i32::MAX }, a]), i32::MAX, "saturates");
    }
}
