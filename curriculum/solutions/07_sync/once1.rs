//! # once1: Initialise exactly once, from any thread
//!
//! `std::sync::OnceLock` isn't available in no_std (the losing thread needs
//! to *block*, which needs an OS). Build `Once<T>` with an atomic state machine:
//!
//! ```text
//!   EMPTY ──(winner: CAS EMPTY→RUNNING)──► RUNNING ──(value written)──► READY
//!                 losers spin while RUNNING, then read the value
//! ```
//!
//! - `get_or_init(f)`: if READY, return the value. Otherwise try to move
//!   EMPTY→RUNNING with `compare_exchange` (Acquire). The winner runs `f`,
//!   writes the value, stores READY with **Release**, and returns it. Everyone
//!   else spins until READY (loads with **Acquire**).
//! - `get`: `Some` only when READY.
//! - `set(v)`: like get_or_init, but returns `Err(v)` if already set (or being set).
//! - `Drop`: if READY, drop the value (it lives in a `MaybeUninit`).
//! - `Sync`: when is it sound to share `&Once<T>`? Threads get `&T` (needs
//!   `T: Sync`) and the value may be created on one thread and dropped on
//!   another (needs `T: Send`).
//!
//! (If `f` panics the state stays RUNNING forever. Real implementations add
//! "poisoning"; you can ignore it here.)
#![no_std]

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU8, Ordering};

const EMPTY: u8 = 0;
const RUNNING: u8 = 1;
const READY: u8 = 2;

pub struct Once<T> {
    state: AtomicU8,
    value: UnsafeCell<MaybeUninit<T>>,
}

// SAFETY: the value is written once by a single winner before READY is
// published (Release) and only read after READY is observed (Acquire). Shared
// access hands out &T (needs Sync); the value may be dropped elsewhere (Send).
unsafe impl<T: Send + Sync> Sync for Once<T> {}

impl<T> Once<T> {
    pub const fn new() -> Self {
        Once { state: AtomicU8::new(EMPTY), value: UnsafeCell::new(MaybeUninit::uninit()) }
    }

    pub fn get(&self) -> Option<&T> {
        if self.state.load(Ordering::Acquire) == READY {
            // SAFETY: READY means the value was fully written and never changes.
            Some(unsafe { (*self.value.get()).assume_init_ref() })
        } else {
            None
        }
    }

    pub fn get_or_init(&self, f: impl FnOnce() -> T) -> &T {
        if let Some(v) = self.get() {
            return v;
        }
        if self.state.compare_exchange(EMPTY, RUNNING, Ordering::Acquire, Ordering::Acquire).is_ok() {
            // SAFETY: we won the race: nobody else reads or writes the value
            // until we publish READY.
            unsafe { (*self.value.get()).write(f()) };
            self.state.store(READY, Ordering::Release);
        }
        loop {
            if let Some(v) = self.get() {
                return v;
            }
            core::hint::spin_loop();
        }
    }

    pub fn set(&self, value: T) -> Result<(), T> {
        if self.state.compare_exchange(EMPTY, RUNNING, Ordering::Acquire, Ordering::Acquire).is_err() {
            return Err(value);
        }
        // SAFETY: as in get_or_init: we are the only writer.
        unsafe { (*self.value.get()).write(value) };
        self.state.store(READY, Ordering::Release);
        Ok(())
    }
}

impl<T> Drop for Once<T> {
    fn drop(&mut self) {
        if *self.state.get_mut() == READY {
            // SAFETY: READY means initialised; we have &mut self so no one else
            // can observe the value afterwards.
            unsafe { self.value.get_mut().assume_init_drop() };
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use core::sync::atomic::AtomicUsize;
    use std::sync::Arc;
    use std::thread;
    use std::vec::Vec;

    #[test]
    fn initialises_once() {
        let o = Once::new();
        assert_eq!(o.get(), None);
        assert_eq!(*o.get_or_init(|| 5), 5);
        assert_eq!(*o.get_or_init(|| 6), 5);
        assert_eq!(o.get(), Some(&5));
        assert_eq!(o.set(7), Err(7));
    }

    #[test]
    fn set_first() {
        let o = Once::new();
        assert_eq!(o.set("config"), Ok(()));
        assert_eq!(o.get_or_init(|| "other"), &"config");
    }

    #[test]
    fn racing_threads_run_init_exactly_once() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        static TABLE: Once<[u32; 64]> = Once::new();
        let hs: Vec<_> = (0..16)
            .map(|_| {
                thread::spawn(|| {
                    let t = TABLE.get_or_init(|| {
                        CALLS.fetch_add(1, Ordering::SeqCst);
                        thread::sleep(std::time::Duration::from_millis(20));
                        core::array::from_fn(|i| i as u32 * 3)
                    });
                    t[63]
                })
            })
            .collect();
        for h in hs {
            assert_eq!(h.join().unwrap(), 189);
        }
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn drops_the_value() {
        let rc = Arc::new(());
        {
            let o = Once::new();
            o.get_or_init(|| rc.clone());
            assert_eq!(Arc::strong_count(&rc), 2);
        }
        assert_eq!(Arc::strong_count(&rc), 1);
        let empty: Once<Arc<()>> = Once::new();
        drop(empty); // must not touch the uninitialised value
    }
}
