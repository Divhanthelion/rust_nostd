//! # arena1: A bump arena with a safe API
//!
//! An arena hands out `&'a mut T` references carved from a byte buffer the
//! caller owns. Allocation is O(1) (bump a pointer), and everything is freed at
//! once when the buffer goes away. The borrow checker guarantees nothing
//! outlives the buffer: the lifetime `'a` ties every allocation to it.
//!
//! Your job is the `unsafe` core behind the safe API:
//!
//! 1. `align_up(addr, align)`: round `addr` up to a multiple of `align` (a
//!    power of two); `None` on overflow.
//! 2. `alloc_layout`: align the **actual address** `start + next` (not just the
//!    offset: the buffer itself may be unaligned), check the end fits, bump
//!    `next`, return the pointer. Zero-size layouts succeed without consuming
//!    space (but still respect alignment).
//! 3. `alloc`, `alloc_slice_copy`, `alloc_str` on top of it.
//!
//! Values are never dropped (like `Box::leak`), which is safe: leaking is
//! allowed in Rust. Every `unsafe` block needs a `// SAFETY:` argument: why is
//! the pointer valid, aligned, and not aliased?
#![no_std]

use core::alloc::Layout;
use core::cell::Cell;
use core::marker::PhantomData;
use core::ptr::NonNull;

pub fn align_up(addr: usize, align: usize) -> Option<usize> {
    todo!()
}

pub struct Arena<'a> {
    start: *mut u8,
    capacity: usize,
    next: Cell<usize>,
    _buf: PhantomData<&'a mut [u8]>,
}

impl<'a> Arena<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        Arena { start: buf.as_mut_ptr(), capacity: buf.len(), next: Cell::new(0), _buf: PhantomData }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn used(&self) -> usize {
        self.next.get()
    }

    pub fn remaining(&self) -> usize {
        self.capacity - self.next.get()
    }

    /// Reserve space for `layout`, returning a pointer to it.
    fn alloc_layout(&self, layout: Layout) -> Option<NonNull<u8>> {
        todo!()
    }

    /// Move `value` into the arena. Gives the value back if it doesn't fit.
    pub fn alloc<T>(&self, value: T) -> Result<&'a mut T, T> {
        todo!()
    }

    /// Copy a slice into the arena.
    pub fn alloc_slice_copy<T: Copy>(&self, src: &[T]) -> Option<&'a mut [T]> {
        todo!()
    }

    /// Copy a string into the arena.
    pub fn alloc_str(&self, s: &str) -> Option<&'a mut str> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_up() {
        assert_eq!(align_up(0, 8), Some(0));
        assert_eq!(align_up(1, 8), Some(8));
        assert_eq!(align_up(8, 8), Some(8));
        assert_eq!(align_up(9, 4), Some(12));
        assert_eq!(align_up(13, 1), Some(13));
        assert_eq!(align_up(usize::MAX, 2), None);
    }

    #[test]
    fn allocates_values() {
        let mut buf = [0u8; 64];
        let arena = Arena::new(&mut buf);
        let a = arena.alloc(1u8).unwrap();
        let b = arena.alloc(0x1122_3344_5566_7788u64).unwrap();
        let c = arena.alloc([7u16; 3]).unwrap();
        *a += 1;
        assert_eq!(*a, 2);
        assert_eq!(*b, 0x1122_3344_5566_7788);
        assert_eq!(*c, [7, 7, 7]);
        assert_eq!(b as *mut u64 as usize % 8, 0, "u64 must be 8-aligned");
    }

    #[test]
    fn aligns_actual_addresses_even_in_unaligned_buffers() {
        #[repr(align(8))]
        struct Aligned([u8; 40]);
        let mut storage = Aligned([0; 40]);
        // Start the arena at an odd address on purpose.
        let arena = Arena::new(&mut storage.0[1..]);
        let x = arena.alloc(5u32).unwrap();
        assert_eq!(x as *mut u32 as usize % 4, 0);
        assert_eq!(arena.used(), 7, "3 padding bytes + 4 bytes");
    }

    #[test]
    fn exhaustion_returns_the_value() {
        #[repr(align(4))]
        struct Aligned([u8; 8]);
        let mut buf = Aligned([0; 8]);
        let arena = Arena::new(&mut buf.0);
        assert!(arena.alloc(1u32).is_ok());
        assert!(arena.alloc(2u32).is_ok());
        assert_eq!(arena.alloc(3u32), Err(3));
        assert_eq!(arena.remaining(), 0);
        // zero-sized values never need space
        assert!(arena.alloc(()).is_ok());
    }

    #[test]
    fn slices_and_strings() {
        let mut buf = [0u8; 32];
        let arena = Arena::new(&mut buf);
        let s = arena.alloc_str("hello").unwrap();
        s.make_ascii_uppercase();
        let nums = arena.alloc_slice_copy(&[1u32, 2, 3]).unwrap();
        nums[0] = 10;
        assert_eq!(s, "HELLO");
        assert_eq!(nums, &[10, 2, 3]);
        assert_eq!(nums.as_ptr() as usize % 4, 0);
        assert!(arena.alloc_slice_copy(&[0u64; 4]).is_none());
        assert_eq!(arena.alloc_slice_copy::<u8>(&[]).map(|s| s.len()), Some(0));
    }

    #[test]
    fn allocations_do_not_overlap() {
        let mut buf = [0u8; 128];
        let arena = Arena::new(&mut buf);
        let mut refs = [None, None, None, None, None, None, None, None];
        for (i, r) in refs.iter_mut().enumerate() {
            *r = Some(arena.alloc([i as u8; 13]).unwrap());
        }
        for (i, r) in refs.iter().enumerate() {
            assert_eq!(r.as_ref().unwrap(), &&mut [i as u8; 13]);
        }
        assert_eq!(arena.used(), 8 * 13);
    }
}
