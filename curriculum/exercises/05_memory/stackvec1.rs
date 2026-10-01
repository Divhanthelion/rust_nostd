//! # stackvec1: A Vec without a heap
//!
//! Build `StackVec<T, N>`: a vector with fixed capacity `N` whose storage
//! lives inline (on the stack, or in a `static`). It's the heart of
//! `heapless::Vec` and `arrayvec::ArrayVec`.
//!
//! Storage is `[MaybeUninit<T>; N]` plus `len`. **The invariant: slots
//! `0..len` are initialised, slots `len..N` are not.** Every method must keep it,
//! and every `unsafe` block's `// SAFETY:` comment should point back to it.
//!
//! Requirements:
//! - `push` returns `Err(value)` when full: give the value back, don't drop it.
//! - Elements are dropped exactly once: by `pop`'s caller, `clear`, `truncate`,
//!   `swap_remove`'s caller, or `StackVec`'s own `Drop`.
//! - `as_slice`/`as_mut_slice` expose the initialised part
//!   (`core::slice::from_raw_parts` + a pointer cast from `*const MaybeUninit<T>`).
//!
//! The tests count drops using `Rc::strong_count`, so leaks and double drops
//! are caught.
#![no_std]

use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};

pub struct StackVec<T, const N: usize> {
    data: [MaybeUninit<T>; N],
    len: usize,
}

impl<T, const N: usize> StackVec<T, N> {
    pub const fn new() -> Self {
        todo!()
    }

    pub const fn capacity(&self) -> usize {
        N
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn is_full(&self) -> bool {
        self.len == N
    }

    /// Append, or give the value back if full.
    pub fn push(&mut self, value: T) -> Result<(), T> {
        todo!()
    }

    /// Remove and return the last element.
    pub fn pop(&mut self) -> Option<T> {
        todo!()
    }

    pub fn as_slice(&self) -> &[T] {
        todo!()
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        todo!()
    }

    /// Shorten to `new_len` elements, dropping the rest. No-op if
    /// `new_len >= len`.
    pub fn truncate(&mut self, new_len: usize) {
        todo!()
    }

    /// Drop every element.
    pub fn clear(&mut self) {
        todo!()
    }

    /// Remove element `index`, replacing it with the last element (O(1),
    /// doesn't preserve order). `None` if out of range.
    pub fn swap_remove(&mut self, index: usize) -> Option<T> {
        todo!()
    }
}

impl<T, const N: usize> Drop for StackVec<T, N> {
    fn drop(&mut self) {
        // TODO: drop the initialised elements (leaking is safe, but wrong here).
    }
}

impl<T, const N: usize> Deref for StackVec<T, N> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, const N: usize> DerefMut for StackVec<T, N> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::rc::Rc;
    use std::string::String;

    #[test]
    fn push_and_pop() {
        let mut v = StackVec::<u32, 3>::new();
        assert!(v.is_empty());
        assert_eq!(v.push(1), Ok(()));
        assert_eq!(v.push(2), Ok(()));
        assert_eq!(v.push(3), Ok(()));
        assert!(v.is_full());
        assert_eq!(v.push(4), Err(4));
        assert_eq!(v.as_slice(), &[1, 2, 3]);
        assert_eq!(v.pop(), Some(3));
        assert_eq!(v.len(), 2);
        assert_eq!(v.pop(), Some(2));
        assert_eq!(v.pop(), Some(1));
        assert_eq!(v.pop(), None);
    }

    #[test]
    fn slices_and_deref() {
        let mut v = StackVec::<i32, 8>::new();
        for i in 0..5 {
            v.push(i).unwrap();
        }
        v.as_mut_slice()[0] = 10;
        v[1] = 11; // DerefMut
        assert_eq!(&v[..], &[10, 11, 2, 3, 4]);
        assert_eq!(v.iter().sum::<i32>(), 30); // slice methods via Deref
        v.sort_unstable_by(|a, b| b.cmp(a));
        assert_eq!(v.first(), Some(&11));
    }

    #[test]
    fn full_push_returns_the_value() {
        let mut v = StackVec::<String, 1>::new();
        v.push(String::from("a")).unwrap();
        let back = v.push(String::from("b")).unwrap_err();
        assert_eq!(back, "b");
    }

    #[test]
    fn drops_exactly_once() {
        let rc = Rc::new(());
        {
            let mut v = StackVec::<Rc<()>, 4>::new();
            for _ in 0..4 {
                v.push(rc.clone()).unwrap();
            }
            assert_eq!(Rc::strong_count(&rc), 5);
            drop(v.pop());
            assert_eq!(Rc::strong_count(&rc), 4);
            v.truncate(1);
            assert_eq!(Rc::strong_count(&rc), 2);
            v.truncate(10);
            assert_eq!(v.len(), 1);
        }
        assert_eq!(Rc::strong_count(&rc), 1, "Drop must drop remaining elements");
    }

    #[test]
    fn clear_drops_everything() {
        let rc = Rc::new(());
        let mut v = StackVec::<Rc<()>, 3>::new();
        v.push(rc.clone()).unwrap();
        v.push(rc.clone()).unwrap();
        v.clear();
        assert!(v.is_empty());
        assert_eq!(Rc::strong_count(&rc), 1);
        v.push(rc.clone()).unwrap();
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn swap_remove_works() {
        let mut v = StackVec::<char, 4>::new();
        for c in ['a', 'b', 'c', 'd'] {
            v.push(c).unwrap();
        }
        assert_eq!(v.swap_remove(1), Some('b'));
        assert_eq!(&v[..], &['a', 'd', 'c']);
        assert_eq!(v.swap_remove(2), Some('c'));
        assert_eq!(v.swap_remove(5), None);
        assert_eq!(&v[..], &['a', 'd']);
    }

    #[test]
    fn zero_capacity_and_const_construction() {
        let mut v = StackVec::<u8, 0>::new();
        assert_eq!(v.push(1), Err(1));
        assert_eq!(v.pop(), None);
        assert_eq!(v.as_slice(), &[] as &[u8]);
        // `new` is const, so a StackVec can live in a static.
        static EMPTY: StackVec<u8, 16> = StackVec::new();
        assert_eq!(EMPTY.capacity(), 16);
    }
}
