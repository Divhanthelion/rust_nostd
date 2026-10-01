//! # atomics1: Counters, flags and a mailbox
//!
//! Atomics are the lowest-level `Sync` building block: they work from
//! threads and interrupt handlers alike, with no locks and no `unsafe`.
//!
//! - `Telemetry`: counters updated concurrently. Use `fetch_add`,
//!   `fetch_max`, `fetch_min`, and `swap` for read-and-reset. `Relaxed` is
//!   enough for statistics.
//! - `saturating_inc`: increment but stop at 255. Needs a compare-and-swap
//!   loop (`compare_exchange_weak`) or `fetch_update`. Return the new value.
//! - `EventFlags`: an ISR raises bits; the main loop takes all pending bits at
//!   once (`fetch_or`, `swap(0)`).
//! - `Mailbox`: one producer posts a `u32`, one consumer takes it. The
//!   payload is published with **Release** and consumed with **Acquire**. If you
//!   use `Relaxed` on the flag, the consumer could see `full == true` but read a
//!   stale payload on weakly-ordered hardware.
#![no_std]

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};

pub struct Telemetry {
    frames: AtomicU32,
    errors: AtomicU32,
    max_latency: AtomicU32,
    min_latency: AtomicU32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub frames: u32,
    pub errors: u32,
    pub max_latency: u32,
    /// `None` if no frames were recorded.
    pub min_latency: Option<u32>,
}

impl Telemetry {
    pub const fn new() -> Self {
        Telemetry {
            frames: AtomicU32::new(0),
            errors: AtomicU32::new(0),
            max_latency: AtomicU32::new(0),
            min_latency: AtomicU32::new(u32::MAX),
        }
    }

    pub fn record(&self, latency_us: u32, ok: bool) {
        todo!()
    }

    pub fn snapshot(&self) -> Snapshot {
        todo!()
    }

    /// Read every counter and reset it to its initial value.
    pub fn take(&self) -> Snapshot {
        todo!()
    }
}

/// Increment, saturating at `u8::MAX`. Returns the new value.
pub fn saturating_inc(a: &AtomicU8) -> u8 {
    todo!()
}

pub struct EventFlags(AtomicU32);

impl EventFlags {
    pub const fn new() -> Self {
        EventFlags(AtomicU32::new(0))
    }

    /// Set the bits in `mask` (called from an "interrupt").
    pub fn raise(&self, mask: u32) {
        todo!()
    }

    /// Take all pending bits, clearing them atomically.
    pub fn take_all(&self) -> u32 {
        todo!()
    }

    pub fn pending(&self, mask: u32) -> bool {
        todo!()
    }
}

pub struct Mailbox {
    payload: AtomicU32,
    full: AtomicBool,
}

impl Mailbox {
    pub const fn new() -> Self {
        Mailbox { payload: AtomicU32::new(0), full: AtomicBool::new(false) }
    }

    /// Post a value; `false` if the previous one hasn't been taken yet.
    pub fn post(&self, value: u32) -> bool {
        todo!()
    }

    /// Take the posted value, if any.
    pub fn take(&self) -> Option<u32> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::vec::Vec;

    #[test]
    fn telemetry_single_thread() {
        let t = Telemetry::new();
        assert_eq!(t.snapshot(), Snapshot { frames: 0, errors: 0, max_latency: 0, min_latency: None });
        t.record(30, true);
        t.record(10, false);
        t.record(20, true);
        assert_eq!(t.snapshot(), Snapshot { frames: 3, errors: 1, max_latency: 30, min_latency: Some(10) });
        assert_eq!(t.take().frames, 3);
        assert_eq!(t.snapshot(), Snapshot { frames: 0, errors: 0, max_latency: 0, min_latency: None });
    }

    #[test]
    fn telemetry_concurrent() {
        let t = Arc::new(Telemetry::new());
        let hs: Vec<_> = (0..8u32)
            .map(|i| {
                let t = t.clone();
                thread::spawn(move || {
                    for j in 0..1000u32 {
                        t.record(i * 1000 + j, j % 10 != 0);
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        assert_eq!(t.snapshot(), Snapshot { frames: 8000, errors: 800, max_latency: 7999, min_latency: Some(0) });
    }

    #[test]
    fn saturates() {
        let a = Arc::new(AtomicU8::new(250));
        assert_eq!(saturating_inc(&a), 251);
        let hs: Vec<_> = (0..4)
            .map(|_| {
                let a = a.clone();
                thread::spawn(move || {
                    for _ in 0..100 {
                        saturating_inc(&a);
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        assert_eq!(a.load(Ordering::Relaxed), 255);
        assert_eq!(saturating_inc(&a), 255);
    }

    #[test]
    fn flags() {
        let f = EventFlags::new();
        f.raise(0b0001);
        f.raise(0b0100);
        assert!(f.pending(0b0100));
        assert!(!f.pending(0b0010));
        assert_eq!(f.take_all(), 0b0101);
        assert_eq!(f.take_all(), 0);
    }

    #[test]
    fn mailbox_passes_values_in_order() {
        let m = Arc::new(Mailbox::new());
        let producer = {
            let m = m.clone();
            thread::spawn(move || {
                for v in 1..=20_000u32 {
                    while !m.post(v) {
                        std::hint::spin_loop();
                    }
                }
            })
        };
        let mut expected = 1;
        while expected <= 20_000 {
            if let Some(v) = m.take() {
                assert_eq!(v, expected);
                expected += 1;
            }
        }
        producer.join().unwrap();
        assert_eq!(m.take(), None);
    }
}
