//! # arrays1: Arrays and const generics
//!
//! Const generics make sizes part of types, so the compiler checks them and
//! no heap is needed. Implement the `todo!()`s.
//!
//! - `core::array::from_fn(|i| ...)` builds `[T; N]` from the index.
//! - `<[T; N]>::try_from(slice)` (or `slice.try_into()`) converts a slice of
//!   the right length into an array.
//! - Inside a generic `impl<const R: usize, const C: usize>`, `R` and `C` are
//!   ordinary `usize` constants you can use in expressions and types.
#![no_std]

/// The first `N` square numbers: `[0, 1, 4, 9, ...]`.
pub fn squares<const N: usize>() -> [u32; N] {
    todo!()
}

/// Copy a slice into an array of exactly `N` elements, or `None` if the
/// length doesn't match.
pub fn to_array<const N: usize>(s: &[u8]) -> Option<[u8; N]> {
    todo!()
}

/// Count values into `BINS` equal-width bins covering 0..=255:
/// value `v` goes to bin `v as usize * BINS / 256`. `BINS` is at least 1.
pub fn histogram<const BINS: usize>(data: &[u8]) -> [u16; BINS] {
    todo!()
}

/// A row-major `R` x `C` matrix of `i32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Matrix<const R: usize, const C: usize>(pub [[i32; C]; R]);

impl<const R: usize, const C: usize> Matrix<R, C> {
    /// The transpose: element (r, c) moves to (c, r).
    pub fn transpose(&self) -> Matrix<C, R> {
        todo!()
    }

    /// Matrix-vector product `self * v`.
    pub fn mul_vec(&self, v: [i32; C]) -> [i32; R] {
        todo!()
    }

    /// The number of elements, computed from the type alone.
    pub const fn len() -> usize {
        todo!()
    }
}

/// Reverse the byte order of every `u16` in a fixed-size frame.
pub fn swap_words<const N: usize>(words: [u16; N]) -> [u16; N] {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_squares() {
        assert_eq!(squares::<5>(), [0, 1, 4, 9, 16]);
        assert_eq!(squares::<0>(), []);
    }

    #[test]
    fn slices_to_arrays() {
        assert_eq!(to_array::<3>(&[1, 2, 3]), Some([1, 2, 3]));
        assert_eq!(to_array::<3>(&[1, 2]), None);
        assert_eq!(to_array::<2>(&[1, 2, 3]), None);
    }

    #[test]
    fn histograms() {
        assert_eq!(histogram::<4>(&[0, 63, 64, 128, 255, 255]), [2, 1, 1, 2]);
        assert_eq!(histogram::<1>(&[1, 2, 3]), [3]);
        assert_eq!(histogram::<256>(&[7, 7])[7], 2);
    }

    #[test]
    fn transposes() {
        let m = Matrix([[1, 2, 3], [4, 5, 6]]);
        assert_eq!(m.transpose(), Matrix([[1, 4], [2, 5], [3, 6]]));
        assert_eq!(m.transpose().transpose(), m);
    }

    #[test]
    fn multiplies() {
        let m = Matrix([[1, 2, 3], [4, 5, 6]]);
        assert_eq!(m.mul_vec([1, 0, -1]), [-2, -2]);
        let id = Matrix([[1, 0], [0, 1]]);
        assert_eq!(id.mul_vec([7, 9]), [7, 9]);
    }

    #[test]
    fn lengths_from_types() {
        assert_eq!(Matrix::<2, 3>::len(), 6);
        const L: usize = Matrix::<4, 4>::len();
        assert_eq!(L, 16);
    }

    #[test]
    fn swaps() {
        assert_eq!(swap_words([0x1234, 0xABCD]), [0x3412, 0xCDAB]);
    }
}
