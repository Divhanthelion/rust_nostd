//! # core2: Living in the core prelude
//!
//! No `Vec`, no `collect()` into a container, no `String`. Just slices,
//! `Option`, `Result` and iterators, which all live in `core` and never
//! allocate.
//!
//! Implement every function marked `todo!()`. Each has a doc comment and tests.
//!
//! A pattern you will use all course long: **the caller provides the storage.**
//! `parse_all` can't return a `Vec<u32>`, so it writes into an `&mut [u32]` the
//! caller owns and returns how many slots it filled. C programmers know this
//! pattern well; Rust makes it safe.
#![no_std]

/// Sum of all bytes, wrapping around at 256 (a classic 8-bit checksum).
pub fn checksum(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b))
}

/// Index and value of the first non-zero fault code, if any.
pub fn first_fault(codes: &[u16]) -> Option<(usize, u16)> {
    codes.iter().copied().enumerate().find(|&(_, c)| c != 0)
}

/// Integer average (rounded toward zero) or `None` for an empty slice.
/// Must not overflow even for many large samples: accumulate in `i64`.
pub fn average(samples: &[i32]) -> Option<i32> {
    if samples.is_empty() {
        return None;
    }
    let sum: i64 = samples.iter().map(|&s| i64::from(s)).sum();
    // The average of i32 values always fits in an i32.
    i32::try_from(sum / samples.len() as i64).ok()
}

/// Length of the longest run of consecutive `byte` values in `data`.
pub fn longest_run(data: &[u8], byte: u8) -> usize {
    let mut best = 0;
    let mut current = 0;
    for &b in data {
        if b == byte {
            current += 1;
            best = best.max(current);
        } else {
            current = 0;
        }
    }
    best
}

/// How many items satisfy `pred`.
pub fn count_where<T>(items: &[T], pred: impl Fn(&T) -> bool) -> usize {
    items.iter().filter(|x| pred(x)).count()
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    /// More fields than the output buffer can hold.
    TooMany,
    /// Field number `index` is not a valid `u32`.
    BadField { index: usize },
}

/// Parse each field as a `u32` into `out`, returning how many were written.
///
/// Fails with `TooMany` if `fields` has more entries than `out`, and with
/// `BadField` for the first unparsable field (leading/trailing spaces are
/// allowed: trim them).
pub fn parse_all(fields: &[&str], out: &mut [u32]) -> Result<usize, ParseError> {
    if fields.len() > out.len() {
        return Err(ParseError::TooMany);
    }
    for (index, (slot, field)) in out.iter_mut().zip(fields).enumerate() {
        *slot = field.trim().parse().map_err(|_| ParseError::BadField { index })?;
    }
    Ok(fields.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_wraps() {
        assert_eq!(checksum(&[]), 0);
        assert_eq!(checksum(&[1, 2, 3]), 6);
        assert_eq!(checksum(&[200, 100]), 44);
        assert_eq!(checksum(&[0xFF; 256]), 0);
    }

    #[test]
    fn finds_first_fault() {
        assert_eq!(first_fault(&[0, 0, 7, 9]), Some((2, 7)));
        assert_eq!(first_fault(&[0, 0]), None);
        assert_eq!(first_fault(&[]), None);
    }

    #[test]
    fn averages() {
        assert_eq!(average(&[]), None);
        assert_eq!(average(&[4, 6]), Some(5));
        assert_eq!(average(&[-3, -4]), Some(-3));
        assert_eq!(average(&[i32::MAX, i32::MAX, i32::MAX]), Some(i32::MAX));
    }

    #[test]
    fn runs() {
        assert_eq!(longest_run(&[], 1), 0);
        assert_eq!(longest_run(&[1, 1, 0, 1, 1, 1, 0], 1), 3);
        assert_eq!(longest_run(&[0, 0, 0], 1), 0);
        assert_eq!(longest_run(&[5, 5, 5, 5], 5), 4);
    }

    #[test]
    fn counts() {
        assert_eq!(count_where(&[1, 2, 3, 4, 5], |x| x % 2 == 1), 3);
        assert_eq!(count_where(&["can", "lin", "can-fd"], |s| s.starts_with("can")), 2);
        assert_eq!(count_where::<u8>(&[], |_| true), 0);
    }

    #[test]
    fn parses_into_caller_buffer() {
        let mut out = [0u32; 4];
        assert_eq!(parse_all(&["1", " 22", "333 "], &mut out), Ok(3));
        assert_eq!(&out[..3], &[1, 22, 333]);
        assert_eq!(parse_all(&[], &mut out), Ok(0));
        assert_eq!(parse_all(&["1", "x", "y"], &mut out), Err(ParseError::BadField { index: 1 }));
        assert_eq!(parse_all(&["1", "2", "3", "4", "5"], &mut out), Err(ParseError::TooMany));
    }
}
