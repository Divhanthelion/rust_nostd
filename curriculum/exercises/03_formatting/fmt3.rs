//! # fmt3: A hexdump formatter
//!
//! Write a classic hexdump into any `core::fmt::Write` sink. It works
//! the same whether the sink is a `String` in a test, a UART on a board, or a
//! debug console in an ECU. Format, per 16-byte line:
//!
//! ```text
//! 00000000  48 65 6c 6c 6f 2c 20 6e  6f 5f 73 74 64 21 0a 00  |Hello, no_std!..|
//! 00000010  ff 01                                             |..|
//! ```
//!
//! - offset: `base + line_start` as 8 lowercase hex digits, then two spaces;
//! - 16 byte columns `xx ` (two hex digits + space), with **one extra space
//!   after the 8th column**. Missing columns on the last line are blank
//!   (three spaces each, plus the extra space if the line is shorter than 9 bytes),
//!   so the ASCII column always lines up;
//! - then ` |`, the bytes as ASCII (printable `0x20..=0x7e`, others as `.`),
//!   and `|`, and a newline;
//! - empty input produces no output at all.
//!
//! Also implement `HexBytes`, a wrapper whose `Display` writes bytes as
//! lowercase hex with no separators, and with `{:#}` (alternate) separates
//! bytes with `:`, like `de:ad:be:ef`.
#![no_std]

use core::fmt::{self, Write};

pub fn hexdump(out: &mut impl Write, base: u32, data: &[u8]) -> fmt::Result {
    todo!()
}

pub struct HexBytes<'a>(pub &'a [u8]);

impl fmt::Display for HexBytes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::format;
    use std::string::String;

    fn dump(base: u32, data: &[u8]) -> String {
        let mut s = String::new();
        hexdump(&mut s, base, data).unwrap();
        s
    }

    #[test]
    fn full_line() {
        assert_eq!(
            dump(0, b"Hello, no_std!\n\0"),
            "00000000  48 65 6c 6c 6f 2c 20 6e  6f 5f 73 74 64 21 0a 00  |Hello, no_std!..|\n"
        );
    }

    #[test]
    fn partial_last_line() {
        let mut data = [0u8; 18];
        data[..16].copy_from_slice(b"0123456789abcdef");
        data[16] = 0xff;
        data[17] = 0x01;
        assert_eq!(
            dump(0x1000, &data),
            "00001000  30 31 32 33 34 35 36 37  38 39 61 62 63 64 65 66  |0123456789abcdef|\n\
             00001010  ff 01                                             |..|\n"
        );
    }

    #[test]
    fn exactly_eight_and_nine_bytes() {
        assert_eq!(
            dump(0, b"ABCDEFGH"),
            "00000000  41 42 43 44 45 46 47 48                           |ABCDEFGH|\n"
        );
        assert_eq!(
            dump(0, b"ABCDEFGHI"),
            "00000000  41 42 43 44 45 46 47 48  49                       |ABCDEFGHI|\n"
        );
    }

    #[test]
    fn every_line_has_the_same_width_up_to_the_ascii_column() {
        let s = dump(0, &[0x7f; 37]);
        for line in s.lines() {
            assert_eq!(line.find('|'), Some(60), "{line:?}");
        }
        assert_eq!(s.lines().count(), 3);
    }

    #[test]
    fn empty_and_offsets() {
        assert_eq!(dump(0, &[]), "");
        assert!(dump(0xFFFF_FFF0, &[0; 32]).contains("00000000  00"));
    }

    #[test]
    fn hex_bytes() {
        assert_eq!(format!("{}", HexBytes(&[0xde, 0xad, 0xbe, 0xef])), "deadbeef");
        assert_eq!(format!("{:#}", HexBytes(&[0xde, 0xad, 0xbe, 0xef])), "de:ad:be:ef");
        assert_eq!(format!("{:#}", HexBytes(&[0x01])), "01");
        assert_eq!(format!("{}", HexBytes(&[])), "");
    }
}
