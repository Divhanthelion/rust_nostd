//! # spinlock1: A spin lock with an RAII guard
//!
//! Build `SpinLock<T>`, a mutual-exclusion lock for **multicore** systems
//! (or hosted tests), with the same shape as `std::sync::Mutex`:
//! `lock()` returns a guard that derefs to `T` and unlocks when dropped.
//!
//! 1. `lock`: loop on `compare_exchange_weak(false, true, Acquire, Relaxed)`,
//!    calling `core::hint::spin_loop()` while it fails. *Acquire* makes the
//!    previous holder's writes visible to you.
//! 2. `try_lock`: a single attempt.
//! 3. The guard's `Drop` stores `false` with **Release**, publishing your
//!    writes to the next holder.
//! 4. `SpinLock<T>` must be usable from a `static` shared by threads, which
//!    requires `Sync`. The compiler can't prove that (it sees an `UnsafeCell`),
//!    so you promise it with an `unsafe impl`. For which `T` is that promise
//!    true? (Think about what a lock lets you do with the `T`: move it between
//!    threads.)
//!
//! Remember the lesson's warning: never share a spin lock with an interrupt
//! handler on a single core.
#![no_std]

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

pub struct SpinLock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

// TODO: tell the compiler when SpinLock<T> may be shared between threads.

pub struct SpinGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        SpinLock { locked: AtomicBool::new(false), value: UnsafeCell::new(value) }
    }

    pub fn lock(&self) -> SpinGuard<'_, T> {
        todo!()
    }

    pub fn try_lock(&self) -> Option<SpinGuard<'_, T>> {
        todo!()
    }

    /// No locking needed: `&mut self` already proves exclusive access.
    pub fn get_mut(&mut self) -> &mut T {
        todo!()
    }

    pub fn into_inner(self) -> T {
        todo!()
    }
}

impl<T> Deref for SpinGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        todo!()
    }
}

impl<T> DerefMut for SpinGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        todo!()
    }
}

impl<T> Drop for SpinGuard<'_, T> {
    fn drop(&mut self) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::thread;
    use std::vec::Vec;

    static COUNTER: SpinLock<u64> = SpinLock::new(0);

    #[test]
    fn mutual_exclusion() {
        let hs: Vec<_> = (0..8)
            .map(|_| {
                thread::spawn(|| {
                    for _ in 0..20_000 {
                        // A non-atomic read-modify-write: only correct under the lock.
                        let mut g = COUNTER.lock();
                        let v = *g;
                        *g = v + 1;
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        assert_eq!(*COUNTER.lock(), 160_000);
    }

    #[test]
    fn try_lock_fails_while_held() {
        let l = SpinLock::new([0u8; 4]);
        let mut g = l.lock();
        g[0] = 7;
        assert!(l.try_lock().is_none());
        drop(g);
        let g2 = l.try_lock().expect("unlocked after the guard dropped");
        assert_eq!(g2[0], 7);
    }

    #[test]
    fn exclusive_access_without_locking() {
        let mut l = SpinLock::new(std::string::String::from("a"));
        l.get_mut().push('b');
        assert_eq!(l.into_inner(), "ab");
    }
}
