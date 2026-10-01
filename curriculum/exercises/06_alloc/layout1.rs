//! # layout1: Size, alignment and padding by hand
//!
//! Allocators, DMA descriptors, FFI structs and wire formats all depend on
//! *layout*: how big a type is, how it must be aligned, and where padding goes.
//! Compute layouts yourself and check them against the compiler.
//!
//! `#[repr(C)]` lays fields out in declaration order: each field starts at the
//! next offset that is a multiple of its alignment; the struct's alignment is
//! the largest field alignment; its size is rounded up to a multiple of that.
//! (Default `repr(Rust)` may reorder fields to reduce padding.)
//!
//! A field is described as `(size, align)`. Implement the functions using
//! `core::alloc::Layout` (`from_size_align`, `extend`, `pad_to_align`, `array`).
#![no_std]

use core::alloc::Layout;

/// Padding needed to bring `offset` up to a multiple of `align` (a power of two).
pub fn padding_needed(offset: usize, align: usize) -> usize {
    todo!()
}

/// The `#[repr(C)]` layout of a struct with these fields, in order, plus
/// each field's offset (at most 8 fields; unused offsets are 0). `None` for an
/// invalid field (alignment not a power of two) or on overflow. A struct with
/// no fields has size 0, align 1.
pub fn repr_c_layout(fields: &[(usize, usize)]) -> Option<(Layout, [usize; 8])> {
    todo!()
}

/// The smallest `repr(C)` size achievable by reordering the fields.
/// For fields whose size is a multiple of their alignment (true for all
/// primitive types), sorting by alignment, largest first, is optimal.
pub fn best_size(fields: &[(usize, usize)]) -> Option<usize> {
    todo!()
}

/// Layout of `[u32; n]`, or `None` if it would overflow.
pub fn u32_array(n: usize) -> Option<Layout> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, offset_of, size_of};

    #[repr(C)]
    struct Header {
        kind: u8,
        id: u32,
        flags: u16,
    }

    #[repr(C)]
    struct Mixed {
        a: u8,
        b: u64,
        c: u16,
        d: u8,
        e: u32,
    }

    #[test]
    fn padding() {
        assert_eq!(padding_needed(0, 4), 0);
        assert_eq!(padding_needed(1, 4), 3);
        assert_eq!(padding_needed(5, 8), 3);
        assert_eq!(padding_needed(8, 8), 0);
        assert_eq!(padding_needed(7, 1), 0);
    }

    #[test]
    fn matches_the_compiler_for_header() {
        let (l, off) = repr_c_layout(&[(1, 1), (4, 4), (2, 2)]).unwrap();
        assert_eq!(l.size(), size_of::<Header>());
        assert_eq!(l.align(), align_of::<Header>());
        assert_eq!(&off[..3], &[offset_of!(Header, kind), offset_of!(Header, id), offset_of!(Header, flags)]);
        assert_eq!((l.size(), l.align()), (12, 4));
    }

    #[test]
    fn matches_the_compiler_for_mixed() {
        let fields = [(1, 1), (8, 8), (2, 2), (1, 1), (4, 4)];
        let (l, off) = repr_c_layout(&fields).unwrap();
        assert_eq!(l.size(), size_of::<Mixed>());
        assert_eq!(l.size(), 24);
        assert_eq!(off[4], offset_of!(Mixed, e));
        assert_eq!(best_size(&fields), Some(16));
    }

    #[test]
    fn edge_cases() {
        let (l, _) = repr_c_layout(&[]).unwrap();
        assert_eq!((l.size(), l.align()), (0, 1));
        assert!(repr_c_layout(&[(4, 3)]).is_none());
        assert!(repr_c_layout(&[(1, 1); 9]).is_none());
        assert_eq!(best_size(&[(2, 2), (1, 1), (4, 4), (1, 1)]), Some(8));
    }

    #[test]
    fn arrays() {
        let l = u32_array(10).unwrap();
        assert_eq!((l.size(), l.align()), (40, 4));
        assert_eq!(u32_array(0).map(|l| l.size()), Some(0));
        assert!(u32_array(usize::MAX / 2).is_none());
    }
}
