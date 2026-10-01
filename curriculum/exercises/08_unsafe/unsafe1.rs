//! # unsafe1: Sound abstractions over unsafe code
//!
//! Each function below is **safe** to call, so it must be sound for *every*
//! possible input. Implement them with raw pointers and the unsafe APIs
//! named in the doc comments, check every precondition first, and write a
//! `// SAFETY:` comment on every `unsafe` block explaining why it holds.
//! (Edition 2024: inside `unsafe fn sum_raw`, unsafe operations still need
//! their own `unsafe { }` blocks.)
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

use core::ptr;
use core::sync::atomic::{compiler_fence, Ordering};

/// Like `<[T]>::split_at_mut`, built from `as_mut_ptr` +
/// `slice::from_raw_parts_mut`. `None` if `mid > len`.
pub fn split_at_mut<T>(s: &mut [T], mid: usize) -> Option<(&mut [T], &mut [T])> {
    todo!()
}

/// Read a little-endian `u32` at any (possibly unaligned) byte offset, using
/// `ptr::read_unaligned`. `None` if out of bounds.
pub fn read_u32_le(buf: &[u8], offset: usize) -> Option<u32> {
    todo!()
}

/// View bytes as `u32`s (native endianness) without copying. `None` unless
/// the data is 4-aligned and its length is a multiple of 4.
pub fn as_u32_slice(bytes: &[u8]) -> Option<&[u32]> {
    todo!()
}

/// Overwrite a secret with zeros in a way the optimiser can't remove (a
/// plain `fill(0)` on a buffer that's never read again may be deleted as a
/// "dead store"). Use `ptr::write_volatile` per byte, then
/// `compiler_fence(Ordering::SeqCst)`.
pub fn zeroize(secret: &mut [u8]) {
    todo!()
}

/// Sum `len` u16 values starting at `ptr`.
///
/// # Safety
/// `ptr` must be non-null, aligned, and valid for reading `len` consecutive
/// `u16`s.
pub unsafe fn sum_raw(ptr: *const u16, len: usize) -> u32 {
    todo!()
}

/// Swap the first `n` elements of `a` and `b` with
/// `ptr::swap_nonoverlapping`. `None` (and no change) if either is shorter
/// than `n`. Two distinct `&mut` slices can never overlap.
pub fn swap_prefix<T>(a: &mut [T], b: &mut [T], n: usize) -> Option<()> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits() {
        let mut v = [1, 2, 3, 4, 5];
        let (a, b) = split_at_mut(&mut v, 2).unwrap();
        a[0] = 10;
        b[0] = 30;
        assert_eq!(v, [10, 2, 30, 4, 5]);
        assert!(split_at_mut(&mut v, 6).is_none());
        let (a, b) = split_at_mut(&mut v, 5).unwrap();
        assert_eq!((a.len(), b.len()), (5, 0));
        let empty: &mut [u8] = &mut [];
        assert!(split_at_mut(empty, 0).is_some());
    }

    #[test]
    fn unaligned_reads() {
        let buf = [0xFF, 0x78, 0x56, 0x34, 0x12, 0xAA];
        assert_eq!(read_u32_le(&buf, 1), Some(0x1234_5678));
        assert_eq!(read_u32_le(&buf, 2), Some(0xAA12_3456));
        assert_eq!(read_u32_le(&buf, 3), None);
        assert_eq!(read_u32_le(&buf, usize::MAX - 1), None);
    }

    #[test]
    fn reinterprets_aligned_bytes_only() {
        #[repr(C, align(4))]
        struct Aligned([u8; 12]);
        let data = Aligned([1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0]);
        let words = as_u32_slice(&data.0).unwrap();
        assert_eq!(words.len(), 3);
        assert_eq!(words.iter().map(|w| u32::from_le(*w)).sum::<u32>(), 6);
        assert!(as_u32_slice(&data.0[1..9]).is_none(), "misaligned");
        assert!(as_u32_slice(&data.0[..6]).is_none(), "length not a multiple of 4");
        assert_eq!(as_u32_slice(&data.0[..0]).map(<[u32]>::len), Some(0));
    }

    #[test]
    fn zeroizes() {
        let mut key = [0x5Au8; 32];
        zeroize(&mut key);
        assert_eq!(key, [0; 32]);
    }

    #[test]
    fn sums() {
        let data = [1u16, 2, 3, u16::MAX];
        assert_eq!(unsafe { sum_raw(data.as_ptr(), 4) }, 6 + 65535);
        assert_eq!(unsafe { sum_raw(data.as_ptr(), 0) }, 0);
    }

    #[test]
    fn swaps() {
        let mut a = [1, 2, 3];
        let mut b = [7, 8];
        assert_eq!(swap_prefix(&mut a, &mut b, 2), Some(()));
        assert_eq!((a, b), ([7, 8, 3], [1, 2]));
        assert_eq!(swap_prefix(&mut a, &mut b, 3), None);
        assert_eq!((a, b), ([7, 8, 3], [1, 2]));
    }
}
