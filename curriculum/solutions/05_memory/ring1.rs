//! # ring1: A ring buffer that never allocates
//!
//! Ring (circular) buffers are everywhere in firmware: UART receive buffers,
//! the last N sensor samples for a filter, a crash log in persistent RAM.
//!
//! `RingBuffer<T, N>` keeps the **most recent** `N` items. When full, `push`
//! overwrites the oldest item and returns it. Represent the state with the
//! storage array, `head` (index of the oldest item) and `len`.
//!
//! - `pop` removes the oldest item; `iter` yields oldest → newest;
//!   `latest` is the newest item.
//! - Capacity `N == 0` must work: nothing is ever stored, so `push` returns
//!   the value immediately. (Careful: `% N` panics when N is 0.)
//! - `T: Copy + Default` keeps this exercise free of `unsafe`.
#![no_std]

pub struct RingBuffer<T, const N: usize> {
    buf: [T; N],
    head: usize,
    len: usize,
}

impl<T: Copy + Default, const N: usize> RingBuffer<T, N> {
    pub fn new() -> Self {
        RingBuffer { buf: [T::default(); N], head: 0, len: 0 }
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

    /// Append `value`. If the buffer was full, the oldest item is
    /// overwritten and returned.
    pub fn push(&mut self, value: T) -> Option<T> {
        if N == 0 {
            return Some(value);
        }
        if self.len < N {
            self.buf[(self.head + self.len) % N] = value;
            self.len += 1;
            None
        } else {
            let old = core::mem::replace(&mut self.buf[self.head], value);
            self.head = (self.head + 1) % N;
            Some(old)
        }
    }

    /// Remove and return the oldest item.
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let v = self.buf[self.head];
        self.head = (self.head + 1) % N;
        self.len -= 1;
        Some(v)
    }

    /// The oldest item, without removing it.
    pub fn peek(&self) -> Option<&T> {
        if self.len == 0 { None } else { self.buf.get(self.head) }
    }

    /// The newest item.
    pub fn latest(&self) -> Option<&T> {
        if self.len == 0 { None } else { self.buf.get((self.head + self.len - 1) % N) }
    }

    /// Items from oldest to newest.
    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        (0..self.len).map(move |i| &self.buf[(self.head + i) % N])
    }

    /// Push every item; returns how many old items were overwritten.
    pub fn extend_from_slice(&mut self, items: &[T]) -> usize {
        items.iter().filter_map(|&x| self.push(x)).count()
    }

    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    fn items<const N: usize>(r: &RingBuffer<u32, N>) -> Vec<u32> {
        r.iter().copied().collect()
    }

    #[test]
    fn fills_then_overwrites_oldest() {
        let mut r = RingBuffer::<u32, 3>::new();
        assert_eq!(r.push(1), None);
        assert_eq!(r.push(2), None);
        assert_eq!(r.push(3), None);
        assert!(r.is_full());
        assert_eq!(r.push(4), Some(1));
        assert_eq!(r.push(5), Some(2));
        assert_eq!(items(&r), [3, 4, 5]);
        assert_eq!(r.peek(), Some(&3));
        assert_eq!(r.latest(), Some(&5));
    }

    #[test]
    fn pops_in_fifo_order_across_the_wrap() {
        let mut r = RingBuffer::<u32, 3>::new();
        r.extend_from_slice(&[1, 2, 3, 4]);
        assert_eq!(r.pop(), Some(2));
        r.push(5);
        r.push(6);
        assert_eq!(items(&r), [4, 5, 6]);
        assert_eq!(r.pop(), Some(4));
        assert_eq!(r.pop(), Some(5));
        assert_eq!(r.pop(), Some(6));
        assert_eq!(r.pop(), None);
        assert!(r.is_empty());
        assert_eq!(r.peek(), None);
        assert_eq!(r.latest(), None);
    }

    #[test]
    fn extend_counts_overwrites() {
        let mut r = RingBuffer::<u32, 4>::new();
        assert_eq!(r.extend_from_slice(&[1, 2]), 0);
        assert_eq!(r.extend_from_slice(&[3, 4, 5, 6, 7]), 3);
        assert_eq!(items(&r), [4, 5, 6, 7]);
    }

    #[test]
    fn clear_resets() {
        let mut r = RingBuffer::<u32, 2>::new();
        r.extend_from_slice(&[1, 2, 3]);
        r.clear();
        assert!(r.is_empty());
        r.push(9);
        assert_eq!(items(&r), [9]);
    }

    #[test]
    fn zero_capacity() {
        let mut r = RingBuffer::<u32, 0>::new();
        assert_eq!(r.push(1), Some(1));
        assert_eq!(r.pop(), None);
        assert_eq!(r.extend_from_slice(&[1, 2]), 2);
        assert_eq!(r.iter().count(), 0);
        assert_eq!(r.latest(), None);
    }

    #[test]
    fn moving_average_use_case() {
        let mut r = RingBuffer::<u32, 4>::new();
        let mut avgs = Vec::new();
        for s in [10, 20, 30, 40, 50, 60] {
            r.push(s);
            avgs.push(r.iter().sum::<u32>() / r.len() as u32);
        }
        assert_eq!(avgs, [10, 15, 20, 25, 35, 45]);
    }
}
