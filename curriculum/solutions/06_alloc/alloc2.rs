//! # alloc2: Write a global allocator
//!
//! Implement `GlobalAlloc` for a bump allocator over a fixed static region,
//! the kind you'd register in firmware with:
//!
//! ```text
//! #[global_allocator]
//! static HEAP: BumpAllocator<{ 16 * 1024 }> = BumpAllocator::new();
//! ```
//!
//! Behaviour:
//! - `alloc`: align the **absolute address** of the next free byte, check the
//!   block fits inside the heap, bump `next`, count one more live allocation,
//!   and return the pointer. Return **null** if it doesn't fit (never panic).
//! - `dealloc`: count one fewer live allocation; when the count reaches zero,
//!   reset `next` to 0, so the whole heap is reusable again.
//! - `align_up` as in arena1.
//!
//! `GlobalAlloc` methods take `&self`: the allocator is a shared static, so its
//! state needs interior mutability. A tiny spin lock (`with_state`, provided)
//! guards it. (On a single-core MCU you'd use a critical section instead, since a
//! spin lock taken by an interrupt handler while main code holds it never gets
//! released. More in module 07.)
#![no_std]

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};

pub fn align_up(addr: usize, align: usize) -> Option<usize> {
    Some(addr.checked_add(align - 1)? & !(align - 1))
}

struct State {
    /// Offset of the next free byte in `heap`.
    next: usize,
    /// Number of allocations not yet deallocated.
    live: usize,
}

pub struct BumpAllocator<const N: usize> {
    heap: UnsafeCell<[MaybeUninit<u8>; N]>,
    lock: AtomicBool,
    state: UnsafeCell<State>,
}

// SAFETY: all access to `state` happens under `lock`, and the heap bytes are
// handed out as disjoint blocks.
unsafe impl<const N: usize> Sync for BumpAllocator<N> {}

impl<const N: usize> BumpAllocator<N> {
    pub const fn new() -> Self {
        BumpAllocator {
            heap: UnsafeCell::new([MaybeUninit::uninit(); N]),
            lock: AtomicBool::new(false),
            state: UnsafeCell::new(State { next: 0, live: 0 }),
        }
    }

    /// Run `f` with exclusive access to the allocator state.
    fn with_state<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        while self.lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            core::hint::spin_loop();
        }
        // SAFETY: we hold the lock, so nobody else has a reference to the state.
        let result = f(unsafe { &mut *self.state.get() });
        self.lock.store(false, Ordering::Release);
        result
    }

    /// Bytes of the heap currently consumed (including alignment padding).
    pub fn used(&self) -> usize {
        self.with_state(|s| s.next)
    }

    /// Allocations not yet freed.
    pub fn live(&self) -> usize {
        self.with_state(|s| s.live)
    }
}

unsafe impl<const N: usize> GlobalAlloc for BumpAllocator<N> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let base = self.heap.get() as usize;
        self.with_state(|s| {
            let Some(start) = base.checked_add(s.next).and_then(|a| align_up(a, layout.align())) else {
                return ptr::null_mut();
            };
            let offset = start - base;
            let Some(end) = offset.checked_add(layout.size()) else { return ptr::null_mut() };
            if end > N {
                return ptr::null_mut();
            }
            s.next = end;
            s.live += 1;
            // SAFETY: offset <= N, so the pointer is inside (or one past) the heap.
            unsafe { self.heap.get().cast::<u8>().add(offset) }
        })
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        self.with_state(|s| {
            s.live = s.live.saturating_sub(1);
            if s.live == 0 {
                s.next = 0;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    fn layout(size: usize, align: usize) -> Layout {
        Layout::from_size_align(size, align).unwrap()
    }

    #[test]
    fn aligned_allocations() {
        let a = BumpAllocator::<256>::new();
        unsafe {
            let p1 = a.alloc(layout(1, 1));
            let p2 = a.alloc(layout(8, 8));
            let p3 = a.alloc(layout(4, 4));
            assert!(!p1.is_null() && !p2.is_null() && !p3.is_null());
            assert_eq!(p2 as usize % 8, 0);
            assert_eq!(p3 as usize % 4, 0);
            assert!(p2 as usize >= p1 as usize + 1);
            assert!(p3 as usize >= p2 as usize + 8);
            p2.cast::<u64>().write(u64::MAX);
            p3.cast::<u32>().write(7);
            assert_eq!(p2.cast::<u64>().read(), u64::MAX);
            assert_eq!(a.live(), 3);
        }
    }

    #[test]
    fn exhaustion_returns_null() {
        let a = BumpAllocator::<64>::new();
        unsafe {
            assert!(!a.alloc(layout(48, 1)).is_null());
            assert!(a.alloc(layout(32, 1)).is_null());
            assert!(!a.alloc(layout(16, 1)).is_null());
            assert!(a.alloc(layout(1, 1)).is_null());
            assert!(a.alloc(layout(isize::MAX as usize - 8, 1)).is_null());
        }
    }

    #[test]
    fn resets_when_everything_is_freed() {
        let a = BumpAllocator::<128>::new();
        unsafe {
            let l = layout(40, 8);
            let p = a.alloc(l);
            let q = a.alloc(l);
            a.dealloc(p, l);
            assert!(a.used() > 0, "q is still live");
            a.dealloc(q, l);
            assert_eq!(a.live(), 0);
            assert_eq!(a.used(), 0);
            let r = a.alloc(layout(100, 1));
            assert!(!r.is_null());
        }
    }

    #[test]
    fn concurrent_allocations_never_overlap() {
        static HEAP: BumpAllocator<{ 64 * 1024 }> = BumpAllocator::new();
        let threads: Vec<_> = (0..8u8)
            .map(|t| {
                std::thread::spawn(move || {
                    let l = layout(64, 16);
                    let mut ptrs = Vec::new();
                    for _ in 0..50 {
                        let p = unsafe { HEAP.alloc(l) };
                        assert!(!p.is_null());
                        unsafe { p.write_bytes(t, 64) };
                        ptrs.push(p as usize);
                    }
                    for &p in &ptrs {
                        let block = unsafe { core::slice::from_raw_parts(p as *const u8, 64) };
                        assert!(block.iter().all(|&b| b == t), "another thread overwrote our block");
                    }
                    ptrs
                })
            })
            .collect();
        let all: Vec<Vec<usize>> = threads.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(HEAP.live(), 400);
        for ptrs in all {
            for p in ptrs {
                unsafe { HEAP.dealloc(p as *mut u8, layout(64, 16)) };
            }
        }
        assert_eq!(HEAP.used(), 0);
    }
}
