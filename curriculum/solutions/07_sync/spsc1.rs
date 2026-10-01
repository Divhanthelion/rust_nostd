//! # spsc1: A lock-free single-producer/single-consumer queue
//!
//! The classic way to move data from an interrupt handler to the main loop
//! (or between two cores) without locks: a ring buffer where
//!
//! - only the **producer** writes `tail` (the next slot to fill), and
//! - only the **consumer** writes `head` (the next slot to read).
//!
//! One slot always stays empty, so `head == tail` means *empty* and
//! `tail + 1 == head` (mod N) means *full*: capacity is `N - 1`.
//!
//! Orderings are the whole exercise:
//! - producer: write the slot, *then* `tail.store(next, Release)` (publish);
//! - consumer: `tail.load(Acquire)` before reading the slot; after reading,
//!   `head.store(next, Release)` so the producer may reuse it;
//! - producer checks fullness with `head.load(Acquire)`.
//! Each side may read its *own* index with `Relaxed`.
//!
//! `split(&mut self)` hands out exactly one `Producer` and one `Consumer`; the
//! `&mut self` borrow guarantees there can't be a second of either.
//! `Queue::new` rejects `N < 2` at compile time (`const { assert!(..) }`).
//! `Drop` must drop any items still in the queue.
#![no_std]

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicUsize, Ordering};

pub struct Queue<T, const N: usize> {
    buf: [UnsafeCell<MaybeUninit<T>>; N],
    head: AtomicUsize,
    tail: AtomicUsize,
}

// SAFETY: slots are accessed by at most one producer and one consumer, and
// the head/tail protocol ensures they never touch the same slot at once.
// Items move between threads, so T must be Send.
unsafe impl<T: Send, const N: usize> Sync for Queue<T, N> {}

pub struct Producer<'a, T, const N: usize> {
    q: &'a Queue<T, N>,
}

pub struct Consumer<'a, T, const N: usize> {
    q: &'a Queue<T, N>,
}

impl<T, const N: usize> Queue<T, N> {
    pub const fn new() -> Self {
        const { assert!(N >= 2, "an SPSC queue needs N >= 2 (capacity is N - 1)") };
        Queue {
            buf: [const { UnsafeCell::new(MaybeUninit::uninit()) }; N],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    pub const fn capacity(&self) -> usize {
        N - 1
    }

    pub fn split(&mut self) -> (Producer<'_, T, N>, Consumer<'_, T, N>) {
        (Producer { q: self }, Consumer { q: self })
    }
}

impl<T, const N: usize> Producer<'_, T, N> {
    /// Add an item, or give it back if the queue is full.
    pub fn enqueue(&mut self, value: T) -> Result<(), T> {
        let tail = self.q.tail.load(Ordering::Relaxed);
        let next = (tail + 1) % N;
        if next == self.q.head.load(Ordering::Acquire) {
            return Err(value);
        }
        // SAFETY: slot `tail` is outside head..tail, so the consumer won't read
        // it until we publish the new tail below.
        unsafe { (*self.q.buf[tail].get()).write(value) };
        self.q.tail.store(next, Ordering::Release);
        Ok(())
    }

    pub fn is_full(&self) -> bool {
        let tail = self.q.tail.load(Ordering::Relaxed);
        (tail + 1) % N == self.q.head.load(Ordering::Acquire)
    }
}

impl<T, const N: usize> Consumer<'_, T, N> {
    /// Remove the oldest item.
    pub fn dequeue(&mut self) -> Option<T> {
        let head = self.q.head.load(Ordering::Relaxed);
        if head == self.q.tail.load(Ordering::Acquire) {
            return None;
        }
        // SAFETY: head != tail, so slot `head` was initialised and published by
        // the producer (Acquire above), and the producer won't touch it until we
        // advance head.
        let value = unsafe { (*self.q.buf[head].get()).assume_init_read() };
        self.q.head.store((head + 1) % N, Ordering::Release);
        Some(value)
    }

    /// Number of items currently queued.
    pub fn len(&self) -> usize {
        let head = self.q.head.load(Ordering::Relaxed);
        let tail = self.q.tail.load(Ordering::Acquire);
        (tail + N - head) % N
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T, const N: usize> Drop for Queue<T, N> {
    fn drop(&mut self) {
        let mut head = *self.head.get_mut();
        let tail = *self.tail.get_mut();
        while head != tail {
            // SAFETY: slots in head..tail are initialised; &mut self means no
            // producer or consumer exists any more.
            unsafe { self.buf[head].get_mut().assume_init_drop() };
            head = (head + 1) % N;
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn fifo_order_and_capacity() {
        let mut q = Queue::<u32, 4>::new();
        assert_eq!(q.capacity(), 3);
        let (mut p, mut c) = q.split();
        assert!(c.is_empty());
        assert_eq!(p.enqueue(1), Ok(()));
        assert_eq!(p.enqueue(2), Ok(()));
        assert_eq!(p.enqueue(3), Ok(()));
        assert!(p.is_full());
        assert_eq!(p.enqueue(4), Err(4));
        assert_eq!(c.len(), 3);
        assert_eq!(c.dequeue(), Some(1));
        assert_eq!(p.enqueue(4), Ok(()));
        assert_eq!(c.dequeue(), Some(2));
        assert_eq!(c.dequeue(), Some(3));
        assert_eq!(c.dequeue(), Some(4));
        assert_eq!(c.dequeue(), None);
    }

    #[test]
    fn wraps_around_many_times() {
        let mut q = Queue::<u16, 3>::new();
        let (mut p, mut c) = q.split();
        for i in 0..1000u16 {
            p.enqueue(i).unwrap();
            if i % 2 == 1 {
                assert_eq!(c.dequeue(), Some(i - 1));
                assert_eq!(c.dequeue(), Some(i));
            }
        }
        assert!(c.is_empty());
    }

    #[test]
    fn two_threads() {
        let mut q = Queue::<u64, 16>::new();
        let (mut p, mut c) = q.split();
        thread::scope(|s| {
            s.spawn(move || {
                for i in 0..200_000u64 {
                    let mut v = i;
                    while let Err(back) = p.enqueue(v) {
                        v = back;
                        std::hint::spin_loop();
                    }
                }
            });
            let mut expected = 0;
            while expected < 200_000 {
                if let Some(v) = c.dequeue() {
                    assert_eq!(v, expected);
                    expected += 1;
                }
            }
        });
    }

    #[test]
    fn drops_remaining_items() {
        let rc = Arc::new(());
        {
            let mut q = Queue::<Arc<()>, 8>::new();
            let (mut p, mut c) = q.split();
            for _ in 0..5 {
                p.enqueue(rc.clone()).unwrap();
            }
            drop(c.dequeue());
            assert_eq!(Arc::strong_count(&rc), 5);
        }
        assert_eq!(Arc::strong_count(&rc), 1);
    }
}
