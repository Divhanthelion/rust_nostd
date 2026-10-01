//! # errors1: Error types that compose
//!
//! A two-layer parser with proper no_std error handling: concrete enums,
//! `Display`, `core::error::Error` with `source()` chains, and `From` impls so
//! that `?` converts errors between layers.
//!
//! **Layer 1, frames:** `[0x7E, len, payload (len bytes), checksum]`, where
//! checksum = XOR of the payload bytes. Check in this order: total length
//! (an empty input or one missing the length byte needs at least 2 bytes; once
//! `len` is known the frame needs `len + 3`), start byte, checksum. Extra bytes
//! after the frame are ignored.
//!
//! **Layer 2, config:** the payload is UTF-8 text `key=value;key=value`.
//! Keys: `id` (u16) and `rate` (u32), both required, in any order. Empty
//! segments are skipped. A segment without `=`, or an unknown key, is
//! `UnknownKey`.
//!
//! The exact `Display` messages are in the tests.
#![no_std]

use core::fmt;
use core::num::ParseIntError;
use core::str::Utf8Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    TooShort { needed: usize, got: usize },
    BadStart(u8),
    BadChecksum { expected: u8, actual: u8 },
}

/// Returns the payload of a valid frame.
pub fn parse_frame(raw: &[u8]) -> Result<&[u8], FrameError> {
    todo!()
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl core::error::Error for FrameError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    Frame(FrameError),
    Utf8(Utf8Error),
    BadValue(ParseIntError),
    UnknownKey,
    Missing(&'static str),
}

// Implement From<FrameError>, From<Utf8Error> and From<ParseIntError> for
// ConfigError so that `?` can convert them.

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl core::error::Error for ConfigError {
    /// The underlying error, if this error wraps one.
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        todo!()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub id: u16,
    pub rate: u32,
}

pub fn load_config(raw: &[u8]) -> Result<Config, ConfigError> {
    todo!()
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use core::error::Error;
    use std::string::ToString;
    use std::vec::Vec;

    fn frame(payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.push(0x7E);
        v.push(payload.len() as u8);
        v.extend_from_slice(payload);
        v.push(payload.iter().fold(0, |a, b| a ^ b));
        v
    }

    #[test]
    fn frames() {
        assert_eq!(parse_frame(&frame(b"abc")), Ok(&b"abc"[..]));
        assert_eq!(parse_frame(&frame(b"")), Ok(&b""[..]));
        let mut extra = frame(b"x");
        extra.push(0xFF);
        assert_eq!(parse_frame(&extra), Ok(&b"x"[..]));
    }

    #[test]
    fn frame_errors_in_order() {
        assert_eq!(parse_frame(&[]), Err(FrameError::TooShort { needed: 2, got: 0 }));
        assert_eq!(parse_frame(&[0x7E]), Err(FrameError::TooShort { needed: 2, got: 1 }));
        assert_eq!(parse_frame(&[0x7E, 3, 1]), Err(FrameError::TooShort { needed: 6, got: 3 }));
        assert_eq!(parse_frame(&[0x00, 0, 0]), Err(FrameError::BadStart(0)));
        assert_eq!(
            parse_frame(&[0x7E, 2, 0x0F, 0xF0, 0x00]),
            Err(FrameError::BadChecksum { expected: 0x00, actual: 0xFF })
        );
    }

    #[test]
    fn frame_messages() {
        assert_eq!(FrameError::TooShort { needed: 6, got: 3 }.to_string(), "frame too short: need 6 bytes, got 3");
        assert_eq!(FrameError::BadStart(0xA).to_string(), "bad start byte 0x0a");
        assert_eq!(
            FrameError::BadChecksum { expected: 1, actual: 0xFF }.to_string(),
            "checksum mismatch: expected 0x01, got 0xff"
        );
    }

    #[test]
    fn loads_configs() {
        assert_eq!(load_config(&frame(b"id=7;rate=100")), Ok(Config { id: 7, rate: 100 }));
        assert_eq!(load_config(&frame(b";rate=5;;id=65535;")), Ok(Config { id: 65535, rate: 5 }));
    }

    #[test]
    fn config_errors() {
        assert!(matches!(load_config(&[0x7E]), Err(ConfigError::Frame(FrameError::TooShort { .. }))));
        assert!(matches!(load_config(&frame(&[0xFF, 0xFE])), Err(ConfigError::Utf8(_))));
        assert!(matches!(load_config(&frame(b"id=x;rate=1")), Err(ConfigError::BadValue(_))));
        assert!(matches!(load_config(&frame(b"id=70000;rate=1")), Err(ConfigError::BadValue(_))));
        assert_eq!(load_config(&frame(b"id=1;speed=2")), Err(ConfigError::UnknownKey));
        assert_eq!(load_config(&frame(b"id=1;rate")), Err(ConfigError::UnknownKey));
        assert_eq!(load_config(&frame(b"rate=1")), Err(ConfigError::Missing("id")));
        assert_eq!(load_config(&frame(b"id=1")), Err(ConfigError::Missing("rate")));
    }

    #[test]
    fn config_messages_and_sources() {
        let e = load_config(&[0x00, 0, 0]).unwrap_err();
        assert_eq!(e.to_string(), "invalid frame");
        assert_eq!(e.source().unwrap().to_string(), "bad start byte 0x00");
        let e = load_config(&frame(b"id=1")).unwrap_err();
        assert_eq!(e.to_string(), "missing key `rate`");
        assert!(e.source().is_none());
        let e = load_config(&frame(b"id=q;rate=1")).unwrap_err();
        assert_eq!(e.to_string(), "bad number");
        assert_eq!(e.source().unwrap().to_string(), "invalid digit found in string");
        assert_eq!(ConfigError::UnknownKey.to_string(), "unknown key");
        let e = load_config(&frame(&[0xFF])).unwrap_err();
        assert_eq!(e.to_string(), "payload is not UTF-8");
        assert!(e.source().is_some());
    }
}
