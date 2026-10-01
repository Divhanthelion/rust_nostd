//! # pool1: An object pool with generational handles
//!
//! Firmware often needs "allocate an object, free it later" (timers,
//! messages, connections) but with no heap and O(1) time. The answer is a
//! **pool**: `N` slots, a free list threaded through the free slots, and
//! **handles** (index + generation) instead of pointers.
//!
//! The generation is what makes it safe: every time a slot is freed its
//! generation increments, so an old handle to a reused slot no longer matches
//! and `get` returns `None` instead of someone else's data. (Use-after-free
//! becomes a checked error.)
//!
//! Requirements:
//! - `alloc` takes the slot at the head of the free list; `Err(value)` when full.
//! - `free` pushes the slot onto the head of the free list (so it is reused
//!   first), bumps the generation with `wrapping_add`, and returns the value.
//! - A stale or double-freed handle is rejected everywhere.
//! - `new()` builds the initial free list 0 → 1 → … → N-1. `N` must fit in a
//!   `u16` (assume it does).
//! - No `unsafe`: `core::mem::replace` moves values out of slots.
#![no_std]

use core::mem;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handle {
    index: u16,
    generation: u16,
}

enum Slot<T> {
    Free { next_free: Option<u16>, generation: u16 },
    Used { value: T, generation: u16 },
}

pub struct Pool<T, const N: usize> {
    slots: [Slot<T>; N],
    free_head: Option<u16>,
    len: usize,
}

impl<T, const N: usize> Pool<T, N> {
    pub fn new() -> Self {
        Pool {
            slots: core::array::from_fn(|i| Slot::Free {
                next_free: if i + 1 < N { Some((i + 1) as u16) } else { None },
                generation: 0,
            }),
            free_head: if N > 0 { Some(0) } else { None },
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_full(&self) -> bool {
        self.len == N
    }

    pub fn alloc(&mut self, value: T) -> Result<Handle, T> {
        let Some(index) = self.free_head else { return Err(value) };
        let Some(slot) = self.slots.get_mut(usize::from(index)) else { return Err(value) };
        let Slot::Free { next_free, generation } = *slot else { return Err(value) };
        *slot = Slot::Used { value, generation };
        self.free_head = next_free;
        self.len += 1;
        Ok(Handle { index, generation })
    }

    pub fn get(&self, h: Handle) -> Option<&T> {
        match self.slots.get(usize::from(h.index))? {
            Slot::Used { value, generation } if *generation == h.generation => Some(value),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, h: Handle) -> Option<&mut T> {
        match self.slots.get_mut(usize::from(h.index))? {
            Slot::Used { value, generation } if *generation == h.generation => Some(value),
            _ => None,
        }
    }

    pub fn free(&mut self, h: Handle) -> Option<T> {
        let slot = self.slots.get_mut(usize::from(h.index))?;
        match slot {
            Slot::Used { generation, .. } if *generation == h.generation => {}
            _ => return None,
        }
        let freed = Slot::Free { next_free: self.free_head, generation: h.generation.wrapping_add(1) };
        match mem::replace(slot, freed) {
            Slot::Used { value, .. } => {
                self.free_head = Some(h.index);
                self.len -= 1;
                Some(value)
            }
            Slot::Free { .. } => None, // unreachable: checked above
        }
    }

    /// All live objects with their handles, in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (Handle, &T)> + '_ {
        self.slots.iter().enumerate().filter_map(|(i, s)| match s {
            Slot::Used { value, generation } => Some((Handle { index: i as u16, generation: *generation }, value)),
            Slot::Free { .. } => None,
        })
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    #[test]
    fn alloc_until_full() {
        let mut p = Pool::<&str, 3>::new();
        let a = p.alloc("a").unwrap();
        let b = p.alloc("b").unwrap();
        let c = p.alloc("c").unwrap();
        assert!(p.is_full());
        assert_eq!(p.alloc("d"), Err("d"));
        assert_eq!((p.get(a), p.get(b), p.get(c)), (Some(&"a"), Some(&"b"), Some(&"c")));
        assert_eq!(p.len(), 3);
    }

    #[test]
    fn free_and_reuse_lifo() {
        let mut p = Pool::<u32, 3>::new();
        let a = p.alloc(1).unwrap();
        let b = p.alloc(2).unwrap();
        assert_eq!(p.free(a), Some(1));
        assert_eq!(p.free(b), Some(2));
        let c = p.alloc(3).unwrap();
        // b's slot was freed last, so it is reused first
        assert_eq!(c.index, b.index);
        assert_ne!(c, b);
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn stale_handles_are_rejected() {
        let mut p = Pool::<u32, 2>::new();
        let old = p.alloc(10).unwrap();
        p.free(old);
        let new = p.alloc(20).unwrap();
        assert_eq!(new.index, old.index);
        assert_eq!(p.get(old), None);
        assert_eq!(p.get_mut(old), None);
        assert_eq!(p.free(old), None);
        assert_eq!(p.get(new), Some(&20));
    }

    #[test]
    fn double_free_is_harmless() {
        let mut p = Pool::<u32, 2>::new();
        let h = p.alloc(5).unwrap();
        assert_eq!(p.free(h), Some(5));
        assert_eq!(p.free(h), None);
        assert_eq!(p.len(), 0);
        // the free list is intact: we can fill the pool again
        assert!(p.alloc(1).is_ok());
        assert!(p.alloc(2).is_ok());
        assert!(p.alloc(3).is_err());
    }

    #[test]
    fn get_mut_and_iter() {
        let mut p = Pool::<u32, 4>::new();
        let a = p.alloc(1).unwrap();
        let b = p.alloc(2).unwrap();
        let c = p.alloc(3).unwrap();
        p.free(b);
        *p.get_mut(c).unwrap() += 10;
        let live: Vec<(Handle, u32)> = p.iter().map(|(h, v)| (h, *v)).collect();
        assert_eq!(live, [(a, 1), (c, 13)]);
    }

    #[test]
    fn zero_capacity() {
        let mut p = Pool::<u8, 0>::new();
        assert_eq!(p.alloc(1), Err(1));
        assert_eq!(p.iter().count(), 0);
    }

    #[test]
    fn generations_wrap() {
        let mut p = Pool::<u8, 1>::new();
        let first = p.alloc(0).unwrap();
        p.free(first);
        for _ in 0..u16::MAX {
            let h = p.alloc(0).unwrap();
            p.free(h);
        }
        // after 65536 frees the generation wraps around to the first handle's
        let again = p.alloc(7).unwrap();
        assert_eq!(again, first);
    }
}
