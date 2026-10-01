//! # fmt1: A string without a heap
//!
//! Build `FixedString<N>`: an N-byte buffer that implements
//! `core::fmt::Write`, so `write!` works on it. It's the core idea behind
//! `heapless::String`.
//!
//! Rules:
//! - Contents must always be valid UTF-8, so `as_str` never fails.
//! - If a write doesn't fit, store **as much as fits without splitting a
//!   character**, then return `Err(fmt::Error)`.
//! - No `unsafe` needed: keep `as_str` honest with `core::str::from_utf8`.
#![no_std]

use core::fmt;

pub struct FixedString<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> FixedString<N> {
    pub const fn new() -> Self {
        FixedString { buf: [0; N], len: 0 }
    }

    /// The text written so far.
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Bytes still available.
    pub fn remaining(&self) -> usize {
        N - self.len
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Append `s` completely or not at all. Returns false if it doesn't fit.
    pub fn try_push_str(&mut self, s: &str) -> bool {
        if s.len() > self.remaining() {
            return false;
        }
        self.buf[self.len..self.len + s.len()].copy_from_slice(s.as_bytes());
        self.len += s.len();
        true
    }
}

impl<const N: usize> fmt::Write for FixedString<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.try_push_str(s) {
            return Ok(());
        }
        // Store the longest prefix that ends on a char boundary.
        let mut cut = self.remaining();
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        let _ = self.try_push_str(&s[..cut]);
        Err(fmt::Error)
    }
}

impl<const N: usize> fmt::Display for FixedString<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `pad` honours width/alignment/precision from the caller's format spec.
        f.pad(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use core::fmt::Write;
    use std::format;

    #[test]
    fn writes_formatted_text() {
        let mut s = FixedString::<32>::new();
        write!(s, "rpm={} gear={}", 2500, 4).unwrap();
        assert_eq!(s.as_str(), "rpm=2500 gear=4");
        assert_eq!(s.len(), 15);
        assert_eq!(s.remaining(), 17);
    }

    #[test]
    fn appends_across_writes() {
        let mut s = FixedString::<16>::new();
        s.write_str("ab").unwrap();
        s.write_char('c').unwrap();
        writeln!(s, "{}", 1).unwrap();
        assert_eq!(s.as_str(), "abc1\n");
    }

    #[test]
    fn overflow_truncates_and_errors() {
        let mut s = FixedString::<8>::new();
        assert!(write!(s, "{}", "0123456789").is_err());
        assert_eq!(s.as_str(), "01234567");
        assert_eq!(s.remaining(), 0);
        assert!(s.write_str("x").is_err());
        assert_eq!(s.as_str(), "01234567");
    }

    #[test]
    fn never_splits_a_character() {
        let mut s = FixedString::<5>::new();
        // 'é' is 2 bytes: "aaaé" needs 5, "aaaéé" needs 7.
        assert!(s.write_str("aaaéé").is_err());
        assert_eq!(s.as_str(), "aaaé");
        let mut t = FixedString::<4>::new();
        assert!(t.write_str("aaaé").is_err());
        assert_eq!(t.as_str(), "aaa");
    }

    #[test]
    fn all_or_nothing_push() {
        let mut s = FixedString::<4>::new();
        assert!(s.try_push_str("ab"));
        assert!(!s.try_push_str("cde"));
        assert_eq!(s.as_str(), "ab");
        s.clear();
        assert!(s.is_empty());
        assert!(s.try_push_str("wxyz"));
        assert_eq!(s.as_str(), "wxyz");
    }

    #[test]
    fn zero_capacity() {
        let mut s = FixedString::<0>::new();
        assert!(s.write_str("").is_ok());
        assert!(s.write_str("a").is_err());
        assert_eq!(s.as_str(), "");
    }

    #[test]
    fn displays_with_padding() {
        let mut s = FixedString::<8>::new();
        s.write_str("ok").unwrap();
        assert_eq!(format!("[{:>5}]", s), "[   ok]");
        assert_eq!(format!("[{:-<4}]", s), "[ok--]");
    }
}
