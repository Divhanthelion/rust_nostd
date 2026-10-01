//! # slices1: Slicing and dicing
//!
//! Protocol and signal-processing code is mostly slice manipulation. Implement
//! each function with slice methods and iterators: no allocation, no
//! `unsafe`, and **no panics** for any input (avoid `[i]` indexing unless you
//! have just proven `i` is in range).
//!
//! Useful tools: `split_first_chunk`, `windows`, `chunks`, `iter().position`,
//! `zip`, slice patterns (`[a, b, rest @ ..]`).
#![no_std]

/// Split a frame into its 4-byte header and the remaining payload.
/// `None` if the frame is shorter than 4 bytes.
pub fn split_header(frame: &[u8]) -> Option<(&[u8; 4], &[u8])> {
    frame.split_first_chunk::<4>()
}

/// Index of the first occurrence of `pattern` in `stream`.
/// An empty pattern matches at index 0.
pub fn find_sync(stream: &[u8], pattern: &[u8]) -> Option<usize> {
    if pattern.is_empty() {
        return Some(0);
    }
    stream.windows(pattern.len()).position(|w| w == pattern)
}

/// For every window of `window` consecutive samples, write the window's
/// maximum into `out`. Returns how many values were written: stop when either
/// the windows or `out` run out. A `window` of 0 produces nothing.
pub fn moving_max(data: &[i16], window: usize, out: &mut [i16]) -> usize {
    if window == 0 {
        return 0;
    }
    let mut written = 0;
    for (slot, w) in out.iter_mut().zip(data.windows(window)) {
        // windows are never empty here, so max() is always Some
        *slot = w.iter().copied().max().unwrap_or(i16::MIN);
        written += 1;
    }
    written
}

/// XOR all bytes of each `block`-sized chunk of `data` (the last chunk may
/// be shorter) and store one result per chunk in `out`. Returns the number of
/// chunks processed, which is limited by `out.len()`. `block == 0` → 0.
pub fn xor_blocks(data: &[u8], block: usize, out: &mut [u8]) -> usize {
    if block == 0 {
        return 0;
    }
    let mut n = 0;
    for (slot, chunk) in out.iter_mut().zip(data.chunks(block)) {
        *slot = chunk.iter().fold(0, |acc, b| acc ^ b);
        n += 1;
    }
    n
}

/// Commands of a tiny register protocol.
#[derive(Debug, PartialEq, Eq)]
pub enum Command<'a> {
    /// `[0x01, addr]`
    Read { addr: u8 },
    /// `[0x02, addr, value]`
    Write { addr: u8, value: u8 },
    /// `[0x03, payload...]` with at most 8 payload bytes (may be empty)
    Bulk(&'a [u8]),
    /// `[]`
    Empty,
    /// anything else
    Invalid,
}

/// Decode a command frame. Use slice patterns!
pub fn decode(frame: &[u8]) -> Command<'_> {
    match frame {
        [] => Command::Empty,
        [0x01, addr] => Command::Read { addr: *addr },
        [0x02, addr, value] => Command::Write { addr: *addr, value: *value },
        [0x03, payload @ ..] if payload.len() <= 8 => Command::Bulk(payload),
        _ => Command::Invalid,
    }
}

/// Rotate `buf` left by `k` positions (k may exceed the length) and
/// return the new first element, or `None` for an empty buffer.
pub fn rotate_and_peek(buf: &mut [u8], k: usize) -> Option<u8> {
    if buf.is_empty() {
        return None;
    }
    let k = k % buf.len();
    buf.rotate_left(k);
    buf.first().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers() {
        let frame = [1, 2, 3, 4, 5, 6];
        let (h, rest) = split_header(&frame).unwrap();
        assert_eq!(h, &[1, 2, 3, 4]);
        assert_eq!(rest, &[5, 6]);
        assert_eq!(split_header(&[1, 2, 3, 4]).map(|(_, r)| r.len()), Some(0));
        assert_eq!(split_header(&[1, 2, 3]), None);
    }

    #[test]
    fn sync_patterns() {
        let s = [0x00, 0xAA, 0x55, 0xAA, 0x55, 0x01];
        assert_eq!(find_sync(&s, &[0xAA, 0x55]), Some(1));
        assert_eq!(find_sync(&s, &[0x55, 0x01]), Some(4));
        assert_eq!(find_sync(&s, &[0x02]), None);
        assert_eq!(find_sync(&s, &[]), Some(0));
        assert_eq!(find_sync(&[1], &[1, 2]), None);
    }

    #[test]
    fn moving_maximum() {
        let data = [1, 3, 2, 5, 4, -1];
        let mut out = [0i16; 8];
        assert_eq!(moving_max(&data, 2, &mut out), 5);
        assert_eq!(&out[..5], &[3, 3, 5, 5, 4]);
        let mut small = [0i16; 2];
        assert_eq!(moving_max(&data, 3, &mut small), 2);
        assert_eq!(small, [3, 5]);
        assert_eq!(moving_max(&data, 0, &mut out), 0);
        assert_eq!(moving_max(&data, 7, &mut out), 0);
    }

    #[test]
    fn xors() {
        let data = [0x01, 0x02, 0x04, 0x08, 0xFF];
        let mut out = [0u8; 4];
        assert_eq!(xor_blocks(&data, 2, &mut out), 3);
        assert_eq!(&out[..3], &[0x03, 0x0C, 0xFF]);
        let mut one = [0u8; 1];
        assert_eq!(xor_blocks(&data, 2, &mut one), 1);
        assert_eq!(xor_blocks(&data, 0, &mut out), 0);
        assert_eq!(xor_blocks(&[], 4, &mut out), 0);
    }

    #[test]
    fn decodes() {
        assert_eq!(decode(&[]), Command::Empty);
        assert_eq!(decode(&[0x01, 0x10]), Command::Read { addr: 0x10 });
        assert_eq!(decode(&[0x02, 0x10, 0xFF]), Command::Write { addr: 0x10, value: 0xFF });
        assert_eq!(decode(&[0x03]), Command::Bulk(&[]));
        assert_eq!(decode(&[0x03, 1, 2, 3]), Command::Bulk(&[1, 2, 3]));
        assert_eq!(decode(&[0x03, 1, 2, 3, 4, 5, 6, 7, 8, 9]), Command::Invalid);
        assert_eq!(decode(&[0x01]), Command::Invalid);
        assert_eq!(decode(&[0x02, 1]), Command::Invalid);
        assert_eq!(decode(&[0x09, 1]), Command::Invalid);
    }

    #[test]
    fn rotates() {
        let mut b = [1, 2, 3, 4];
        assert_eq!(rotate_and_peek(&mut b, 1), Some(2));
        assert_eq!(b, [2, 3, 4, 1]);
        assert_eq!(rotate_and_peek(&mut b, 7), Some(1));
        assert_eq!(rotate_and_peek(&mut [], 3), None);
    }
}
