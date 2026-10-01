//! # panicfree1: Make it impossible to panic
//!
//! Every function below compiles and works on "nice" inputs, and **panics**
//! on hostile ones. Rewrite each so that it never panics for *any* input,
//! returning `None`/`Err` instead, as documented. The tests throw edge cases
//! at them: empty inputs, zero divisors, huge values, short buffers, odd UTF-8.
//!
//! Banned in your final code: `unwrap()`, `expect()`, `[i]` indexing,
//! `&s[a..b]` range slicing, unchecked `/`, `%` and arithmetic that can
//! overflow. (Use `get`, iterators, `checked_*`, `?` and `let else`.)
//!
//! This is the daily discipline of safety-relevant code: a panic in a brake
//! controller is a failure, even if the panic handler resets cleanly.
#![no_std]

/// Average of the samples. `None` for an empty slice. Never overflows.
pub fn average(xs: &[u32]) -> Option<u32> {
    if xs.is_empty() {
        return None;
    }
    let sum: u64 = xs.iter().map(|&x| u64::from(x)).sum();
    u32::try_from(sum / xs.len() as u64).ok()
}

/// The bytes at `i` and `i + 1`. `None` if either is out of range
/// (including when `i + 1` overflows).
pub fn pair_at(data: &[u8], i: usize) -> Option<(u8, u8)> {
    let next = i.checked_add(1)?;
    Some((*data.get(i)?, *data.get(next)?))
}

/// `raw * num / den`, computed without intermediate overflow.
/// `None` if `den == 0` or the result doesn't fit in a `u32`.
pub fn scale(raw: u32, num: u32, den: u32) -> Option<u32> {
    let wide = u64::from(raw).checked_mul(u64::from(num))?.checked_div(u64::from(den))?;
    u32::try_from(wide).ok()
}

/// The `n`-th comma-separated field, trimmed. `None` if there is no such field.
pub fn field(record: &str, n: usize) -> Option<&str> {
    record.split(',').nth(n).map(str::trim)
}

/// The first `n` characters (not bytes!) of `s`, or all of `s` if it's shorter.
pub fn prefix_chars(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((byte_index, _)) => s.get(..byte_index).unwrap_or(s),
        None => s,
    }
}

/// Decode a little-endian u16 length prefix followed by that many bytes.
/// Returns the body, or `None` if the buffer is too short.
pub fn length_prefixed(buf: &[u8]) -> Option<&[u8]> {
    let (len, rest) = buf.split_first_chunk::<2>()?;
    rest.get(..usize::from(u16::from_le_bytes(*len)))
}

/// Percentage `part / whole * 100`, rounded down. `None` if `whole == 0`.
/// Values above 100% are fine (e.g. 150).
pub fn percent(part: u32, whole: u32) -> Option<u32> {
    let p = u64::from(part).checked_mul(100)?.checked_div(u64::from(whole))?;
    u32::try_from(p).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn averages() {
        assert_eq!(average(&[]), None);
        assert_eq!(average(&[2, 4]), Some(3));
        assert_eq!(average(&[u32::MAX, u32::MAX]), Some(u32::MAX));
    }

    #[test]
    fn pairs() {
        assert_eq!(pair_at(&[1, 2, 3], 1), Some((2, 3)));
        assert_eq!(pair_at(&[1, 2, 3], 2), None);
        assert_eq!(pair_at(&[], 0), None);
        assert_eq!(pair_at(&[1, 2], usize::MAX), None);
    }

    #[test]
    fn scales() {
        assert_eq!(scale(1000, 3, 4), Some(750));
        assert_eq!(scale(u32::MAX, 2, 2), Some(u32::MAX));
        assert_eq!(scale(u32::MAX, 2, 1), None);
        assert_eq!(scale(5, 1, 0), None);
    }

    #[test]
    fn fields() {
        assert_eq!(field("a, b ,c", 1), Some("b"));
        assert_eq!(field("a,b", 2), None);
        assert_eq!(field("", 0), Some(""));
        assert_eq!(field("1,2,3,4,5,6,7,8,9,10", 9), Some("10"));
    }

    #[test]
    fn prefixes() {
        assert_eq!(prefix_chars("hello", 2), "he");
        assert_eq!(prefix_chars("hi", 10), "hi");
        assert_eq!(prefix_chars("Grüße", 3), "Grü");
        assert_eq!(prefix_chars("", 1), "");
        assert_eq!(prefix_chars("😀😀", 1), "😀");
    }

    #[test]
    fn length_prefixes() {
        assert_eq!(length_prefixed(&[2, 0, 9, 8, 7]), Some(&[9, 8][..]));
        assert_eq!(length_prefixed(&[0, 0]), Some(&[][..]));
        assert_eq!(length_prefixed(&[5, 0, 1]), None);
        assert_eq!(length_prefixed(&[1]), None);
        assert_eq!(length_prefixed(&[0xFF, 0xFF]), None);
    }

    #[test]
    fn percents() {
        assert_eq!(percent(1, 4), Some(25));
        assert_eq!(percent(3, 2), Some(150));
        assert_eq!(percent(1, 0), None);
        assert_eq!(percent(u32::MAX, 1), None);
        assert_eq!(percent(u32::MAX, u32::MAX), Some(100));
    }
}
