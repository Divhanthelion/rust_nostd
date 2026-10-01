//! # cs1: Critical sections and the token pattern
//!
//! On a single-core MCU, the standard way to share data between main code and
//! interrupt handlers is: disable interrupts, touch the data, re-enable. The
//! `critical-section` crate wraps that in a safe API that the compiler checks.
//! Here you'll build that API yourself.
//!
//! The `interrupt` module (provided) emulates a single-core MCU on your PC:
//! `interrupt::free(|cs| ...)` runs the closure with "interrupts disabled"
//! (a global lock, so host threads behave like interrupts that can't fire), and
//! hands it a `CriticalSection<'cs>` token that cannot be created elsewhere.
//!
//! Your tasks:
//! 1. `Mutex<T>`: `borrow(&'cs self, cs: CriticalSection<'cs>) -> &'cs T`. The
//!    token proves interrupts are off; its lifetime stops the reference from
//!    escaping the critical section. Add the `unsafe impl Sync` (bounds?).
//! 2. `Counter`: a `Mutex<Cell<u32>>` incremented inside `interrupt::free`.
//! 3. The **peripheral hand-off** pattern: main code creates a `Uart` and moves
//!    it into a global `Mutex<RefCell<Option<Uart>>>` so an interrupt handler can
//!    use it: `install`, `with_uart`, `uninstall`.
#![no_std]

use core::cell::{Cell, RefCell, UnsafeCell};

/// Emulated interrupt control (provided: don't change).
pub mod interrupt {
    use core::marker::PhantomData;
    use core::sync::atomic::{AtomicBool, Ordering};

    static INTERRUPTS_MASKED: AtomicBool = AtomicBool::new(false);

    /// Proof that interrupts are disabled. Zero-sized, `Copy`, and only
    /// `free` can create one.
    #[derive(Clone, Copy)]
    pub struct CriticalSection<'cs> {
        _private: PhantomData<&'cs ()>,
    }

    /// Run `f` with interrupts disabled. (Not re-entrant: don't nest.)
    pub fn free<R>(f: impl FnOnce(CriticalSection<'_>) -> R) -> R {
        while INTERRUPTS_MASKED
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        let r = f(CriticalSection { _private: PhantomData });
        INTERRUPTS_MASKED.store(false, Ordering::Release);
        r
    }
}

use interrupt::CriticalSection;

pub struct Mutex<T> {
    inner: UnsafeCell<T>,
}

// SAFETY: the inner value is only reachable through `borrow`, which requires a
// critical-section token, so accesses never overlap. Values may effectively be
// handed between execution contexts, hence T: Send.
unsafe impl<T: Send> Sync for Mutex<T> {}

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Mutex { inner: UnsafeCell::new(value) }
    }

    /// Access the value inside a critical section.
    pub fn borrow<'cs>(&'cs self, _cs: CriticalSection<'cs>) -> &'cs T {
        // SAFETY: the token proves we're in a critical section, so no other
        // context can access `inner` while the returned reference lives.
        unsafe { &*self.inner.get() }
    }

    /// `&mut self` proves exclusive access: no critical section needed.
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }
}

pub struct Counter {
    count: Mutex<Cell<u32>>,
}

impl Counter {
    pub const fn new() -> Self {
        Counter { count: Mutex::new(Cell::new(0)) }
    }

    pub fn increment(&self) -> u32 {
        interrupt::free(|cs| {
            let c = self.count.borrow(cs);
            c.set(c.get() + 1);
            c.get()
        })
    }

    pub fn get(&self) -> u32 {
        interrupt::free(|cs| self.count.borrow(cs).get())
    }
}

/// A pretend UART peripheral that records what was sent.
#[derive(Debug, PartialEq)]
pub struct Uart {
    pub sent: [u8; 32],
    pub len: usize,
}

impl Uart {
    pub fn new() -> Self {
        Uart { sent: [0; 32], len: 0 }
    }
    pub fn send(&mut self, b: u8) {
        if let Some(slot) = self.sent.get_mut(self.len) {
            *slot = b;
            self.len += 1;
        }
    }
}

static SHARED_UART: Mutex<RefCell<Option<Uart>>> = Mutex::new(RefCell::new(None));

/// Move the UART into the global slot. Gives it back if one is already there.
pub fn install(uart: Uart) -> Result<(), Uart> {
    interrupt::free(|cs| {
        let mut slot = SHARED_UART.borrow(cs).borrow_mut();
        if slot.is_some() {
            return Err(uart);
        }
        *slot = Some(uart);
        Ok(())
    })
}

/// Run `f` on the installed UART (e.g. from an interrupt handler).
/// `None` if no UART is installed.
pub fn with_uart<R>(f: impl FnOnce(&mut Uart) -> R) -> Option<R> {
    interrupt::free(|cs| SHARED_UART.borrow(cs).borrow_mut().as_mut().map(f))
}

/// Take the UART back out of the global slot.
pub fn uninstall() -> Option<Uart> {
    interrupt::free(|cs| SHARED_UART.borrow(cs).borrow_mut().take())
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::thread;
    use std::vec::Vec;

    static COUNTER: Counter = Counter::new();

    #[test]
    fn counter_is_consistent_across_contexts() {
        let hs: Vec<_> = (0..8)
            .map(|_| thread::spawn(|| for _ in 0..5_000 { COUNTER.increment(); }))
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        assert_eq!(COUNTER.get(), 40_000);
    }

    #[test]
    fn get_mut_needs_no_critical_section() {
        let mut m = Mutex::new(5);
        *m.get_mut() += 1;
        assert_eq!(interrupt::free(|cs| *m.borrow(cs)), 6);
    }

    #[test]
    fn peripheral_handoff() {
        assert_eq!(with_uart(|u| u.send(b'x')), None);
        install(Uart::new()).unwrap();
        let second = install(Uart::new());
        assert!(second.is_err(), "the slot is occupied");
        // "interrupt handlers" on other threads use the shared UART
        let hs: Vec<_> = (0..4u8)
            .map(|i| thread::spawn(move || with_uart(|u| u.send(b'a' + i)).unwrap()))
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        let uart = uninstall().unwrap();
        assert_eq!(uart.len, 4);
        let mut sent: Vec<u8> = uart.sent[..4].to_vec();
        sent.sort();
        assert_eq!(sent, b"abcd");
        assert_eq!(uninstall(), None);
    }
}
