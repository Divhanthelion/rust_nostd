//! # itoa1: Numbers to text, by hand
//!
//! `core::fmt` can format numbers, but it brings kilobytes of machinery
//! with it. On a small microcontroller you often format by hand into a
//! buffer. Implement these converters. Each one writes into a buffer the
//! caller provides and returns a `&str` that borrows from it.
//!
//! Technique: produce digits least-significant first, filling the buffer from
//! the **end**, then return the used tail. Size each buffer for the worst case.
//!
//! Rules: no `core::fmt` (`write!`, `format_args!`) in this file, and no panics.
//! The bytes you produce are ASCII, so `core::str::from_utf8(..)` will succeed.
//! Use `.unwrap_or("")` rather than `unwrap()` to stay panic-free.
#![no_std]

/// Decimal digits of `n`, e.g. `4294967295`. A u32 has at most 10 digits.
pub fn u32_to_dec(n: u32, buf: &mut [u8; 10]) -> &str {
    let mut n = n;
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    core::str::from_utf8(&buf[i..]).unwrap_or("")
}

/// Decimal digits of `n` with a leading `-` for negatives.
/// Careful: `-i32::MIN` overflows. `unsigned_abs()` doesn't.
pub fn i32_to_dec(n: i32, buf: &mut [u8; 11]) -> &str {
    let mut m = n.unsigned_abs();
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (m % 10) as u8;
        m /= 10;
        if m == 0 {
            break;
        }
    }
    if n < 0 {
        i -= 1;
        buf[i] = b'-';
    }
    core::str::from_utf8(&buf[i..]).unwrap_or("")
}

/// Two lowercase hex digits for a byte: `0xA5` → `*b"a5"`.
pub fn u8_to_hex(b: u8) -> [u8; 2] {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    [DIGITS[usize::from(b >> 4)], DIGITS[usize::from(b & 0x0F)]]
}

/// Format a value in thousandths with exactly three decimals:
/// `21500` → `"21.500"`, `-1250` → `"-1.250"`, `5` → `"0.005"`,
/// `i32::MIN` → `"-2147483.648"`.
pub fn milli_to_dec(milli: i32, buf: &mut [u8; 16]) -> &str {
    let mut m = milli.unsigned_abs();
    let mut i = buf.len();
    let mut digits = 0;
    loop {
        if digits == 3 {
            i -= 1;
            buf[i] = b'.';
        }
        i -= 1;
        buf[i] = b'0' + (m % 10) as u8;
        m /= 10;
        digits += 1;
        if m == 0 && digits >= 4 {
            break;
        }
    }
    if milli < 0 {
        i -= 1;
        buf[i] = b'-';
    }
    core::str::from_utf8(&buf[i..]).unwrap_or("")
}

/// Parse a decimal `u16` from ASCII bytes without `str::parse`: only digits
/// are allowed (no sign, no spaces, not empty) and the value must fit.
pub fn parse_u16(digits: &[u8]) -> Option<u16> {
    if digits.is_empty() {
        return None;
    }
    let mut value: u16 = 0;
    for &d in digits {
        if !d.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(u16::from(d - b'0'))?;
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsigned() {
        let mut b = [0; 10];
        assert_eq!(u32_to_dec(0, &mut b), "0");
        assert_eq!(u32_to_dec(7, &mut b), "7");
        assert_eq!(u32_to_dec(1200, &mut b), "1200");
        assert_eq!(u32_to_dec(u32::MAX, &mut b), "4294967295");
    }

    #[test]
    fn signed() {
        let mut b = [0; 11];
        assert_eq!(i32_to_dec(0, &mut b), "0");
        assert_eq!(i32_to_dec(-7, &mut b), "-7");
        assert_eq!(i32_to_dec(42, &mut b), "42");
        assert_eq!(i32_to_dec(i32::MAX, &mut b), "2147483647");
        assert_eq!(i32_to_dec(i32::MIN, &mut b), "-2147483648");
    }

    #[test]
    fn hex() {
        assert_eq!(&u8_to_hex(0x00), b"00");
        assert_eq!(&u8_to_hex(0xA5), b"a5");
        assert_eq!(&u8_to_hex(0x0F), b"0f");
        assert_eq!(&u8_to_hex(0xFF), b"ff");
    }

    #[test]
    fn fixed_point() {
        let mut b = [0; 16];
        assert_eq!(milli_to_dec(21500, &mut b), "21.500");
        assert_eq!(milli_to_dec(-1250, &mut b), "-1.250");
        assert_eq!(milli_to_dec(5, &mut b), "0.005");
        assert_eq!(milli_to_dec(-5, &mut b), "-0.005");
        assert_eq!(milli_to_dec(0, &mut b), "0.000");
        assert_eq!(milli_to_dec(1000, &mut b), "1.000");
        assert_eq!(milli_to_dec(i32::MIN, &mut b), "-2147483.648");
    }

    #[test]
    fn parses() {
        assert_eq!(parse_u16(b"0"), Some(0));
        assert_eq!(parse_u16(b"65535"), Some(65535));
        assert_eq!(parse_u16(b"65536"), None);
        assert_eq!(parse_u16(b"99999999"), None);
        assert_eq!(parse_u16(b""), None);
        assert_eq!(parse_u16(b"12a"), None);
        assert_eq!(parse_u16(b"-1"), None);
        assert_eq!(parse_u16(b" 1"), None);
    }
}
