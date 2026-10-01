//! `{{name}}`: a portable `no_std` library.
//!
//! * Always `#![no_std]`; `std`/`alloc` are opt-in features.
//! * Builds for bare metal: `cargo build --target thumbv7em-none-eabihf`
//! * Tested on the host: `cargo test --all-features`
#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

use core::fmt;

/// Errors returned by this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// A window of length zero was requested.
    EmptyWindow,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::EmptyWindow => f.write_str("window length must be at least 1"),
        }
    }
}

impl core::error::Error for Error {}

/// Moving average over the last `N` samples, with no heap and no panics.
#[derive(Debug, Clone)]
pub struct MovingAverage<const N: usize> {
    buf: [i32; N],
    len: usize,
    next: usize,
    sum: i64,
}

impl<const N: usize> MovingAverage<N> {
    /// Create an empty filter. Fails for `N == 0`.
    pub const fn new() -> Result<Self, Error> {
        if N == 0 {
            return Err(Error::EmptyWindow);
        }
        Ok(Self { buf: [0; N], len: 0, next: 0, sum: 0 })
    }

    /// Add a sample and return the current average (rounded toward zero).
    pub fn push(&mut self, sample: i32) -> i32 {
        if let Some(slot) = self.buf.get_mut(self.next) {
            if self.len == N {
                self.sum = self.sum.saturating_sub(i64::from(*slot));
            } else {
                self.len = self.len.saturating_add(1);
            }
            *slot = sample;
            self.sum = self.sum.saturating_add(i64::from(sample));
        }
        self.next = self.next.wrapping_add(1).checked_rem(N).unwrap_or(0);
        let len = i64::try_from(self.len).unwrap_or(i64::MAX).max(1);
        // |sum / len| never exceeds i32::MAX, so the conversion cannot fail.
        i32::try_from(self.sum.checked_div(len).unwrap_or(0)).unwrap_or(sample)
    }
}

/// CRC-8/SAE-J1850 (poly 0x1D, init 0xFF, xorout 0xFF), as used by
/// AUTOSAR E2E profiles 1 and 2.
pub fn crc8_sae_j1850(data: &[u8]) -> u8 {
    let mut crc: u8 = 0xFF;
    for &byte in data {
        crc ^= byte;
        for _ in 0..8 {
            let shifted = crc.wrapping_shl(1);
            crc = if crc & 0x80 != 0 { shifted ^ 0x1D } else { shifted };
        }
    }
    crc ^ 0xFF
}

/// Hex-encode bytes (only with the `alloc` feature).
#[cfg(feature = "alloc")]
pub fn to_hex(data: &[u8]) -> alloc::string::String {
    use core::fmt::Write;
    let mut s = alloc::string::String::with_capacity(data.len().saturating_mul(2));
    for b in data {
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_check_value() {
        // The standard check input for CRC catalogues.
        assert_eq!(crc8_sae_j1850(b"123456789"), 0x4B);
    }

    #[test]
    fn moving_average() {
        let mut f = MovingAverage::<3>::new().unwrap();
        assert_eq!(f.push(3), 3);
        assert_eq!(f.push(6), 4);
        assert_eq!(f.push(9), 6);
        assert_eq!(f.push(12), 9);
        assert!(MovingAverage::<0>::new().is_err());
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn hex() {
        assert_eq!(to_hex(&[0xde, 0xad]), "dead");
    }
}
