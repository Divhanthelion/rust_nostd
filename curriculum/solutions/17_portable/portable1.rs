//! # portable1: One crate for firmware, heap users and std users
//!
//! This library parses comma-separated `u16` lists. It's meant to serve
//! three audiences, selected by Cargo features:
//!
//! | features        | available API                      | built against |
//! |-----------------|------------------------------------|---------------|
//! | (none)          | `parse_into` (caller's buffer)     | core only |
//! | `alloc`         | + `parse_vec` returning a `Vec`    | core + alloc |
//! | `std` (+ alloc) | + `parse_reader` over `std::io::BufRead` | std |
//!
//! Right now everything is compiled unconditionally, so the no-feature build
//! fails (there is no std, and nothing should need alloc either). Gate the
//! crates and functions with `#[cfg(feature = "...")]` so that **each** of the
//! three builds works. The checker builds and tests all three; tests for the
//! optional APIs are already gated.
//!
//! Keep `#![no_std]` unconditional: std is opted into with
//! `extern crate std`, so the prelude is the same in every configuration.
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// Field `index` is not a valid u16.
    BadField { index: usize },
    /// More fields than the output buffer holds.
    TooMany,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::BadField { index } => write!(f, "field {index} is not a number"),
            ParseError::TooMany => f.write_str("too many fields"),
        }
    }
}

impl core::error::Error for ParseError {}

/// Parse `"1, 2,3"` into `out`, returning the number of values.
/// An empty (or all-whitespace) line has zero values.
pub fn parse_into(line: &str, out: &mut [u16]) -> Result<usize, ParseError> {
    if line.trim().is_empty() {
        return Ok(0);
    }
    let mut n = 0;
    for (index, field) in line.split(',').enumerate() {
        let slot = out.get_mut(index).ok_or(ParseError::TooMany)?;
        *slot = field.trim().parse().map_err(|_| ParseError::BadField { index })?;
        n += 1;
    }
    Ok(n)
}

/// Parse into a freshly allocated `Vec`.
#[cfg(feature = "alloc")]
pub fn parse_vec(line: &str) -> Result<alloc::vec::Vec<u16>, ParseError> {
    if line.trim().is_empty() {
        return Ok(alloc::vec::Vec::new());
    }
    line.split(',')
        .enumerate()
        .map(|(index, f)| f.trim().parse().map_err(|_| ParseError::BadField { index }))
        .collect()
}

/// Errors from `parse_reader`.
#[cfg(feature = "std")]
#[derive(Debug)]
pub enum ReadError {
    Io(std::io::Error),
    Parse { line: usize, error: ParseError },
}

/// Parse every line of a reader, concatenating the values.
#[cfg(feature = "std")]
pub fn parse_reader<R: std::io::BufRead>(reader: R) -> Result<alloc::vec::Vec<u16>, ReadError> {
    let mut all = alloc::vec::Vec::new();
    for (i, line) in reader.lines().enumerate() {
        let line = line.map_err(ReadError::Io)?;
        let values = parse_vec(&line).map_err(|error| ReadError::Parse { line: i + 1, error })?;
        all.extend(values);
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_api() {
        let mut out = [0u16; 4];
        assert_eq!(parse_into("1, 2,3", &mut out), Ok(3));
        assert_eq!(out[..3], [1, 2, 3]);
        assert_eq!(parse_into("  ", &mut out), Ok(0));
        assert_eq!(parse_into("1,x", &mut out), Err(ParseError::BadField { index: 1 }));
        assert_eq!(parse_into("1,2,3,4,5", &mut out), Err(ParseError::TooMany));
        assert_eq!(parse_into("70000", &mut out), Err(ParseError::BadField { index: 0 }));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn alloc_api() {
        assert_eq!(parse_vec("10,20"), Ok(alloc::vec![10, 20]));
        assert_eq!(parse_vec(""), Ok(alloc::vec![]));
        assert_eq!(parse_vec("1,,2"), Err(ParseError::BadField { index: 1 }));
    }

    #[cfg(feature = "std")]
    #[test]
    fn std_api() {
        let input = std::io::Cursor::new("1,2\n3\n\n4, 5\n");
        assert_eq!(parse_reader(input).unwrap(), [1, 2, 3, 4, 5]);
        let bad = std::io::Cursor::new("1\n2,oops\n");
        match parse_reader(bad) {
            Err(ReadError::Parse { line, error }) => {
                assert_eq!(line, 2);
                assert_eq!(error, ParseError::BadField { index: 1 });
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
