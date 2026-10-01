//! # async2: A static executor with real wakers
//!
//! `block_on` polls in a loop. A real executor polls a task **only when it
//! was woken**, and sleeps otherwise. Build one the way embedded executors do:
//! no heap, a fixed number of task slots, and a bitmask of ready tasks.
//!
//! 1. **Wakers.** A waker's data pointer encodes *which executor and which
//!    task*: `(exec << 8) | task`. Waking sets bit `task` in `READY[exec]`
//!    (`fetch_or`, Release). Implement the four vtable functions and
//!    `waker_for`. (Our wakers own no resources, so `clone` just copies the
//!    pointer and `drop` does nothing.)
//! 2. **`spawn`**: put the future in the first free slot, mark it ready, return
//!    its index; if all slots are full, give the future back.
//! 3. **`run_until_idle`**: repeatedly take the ready bits
//!    (`swap(0, Acquire)`) and poll each ready task (lowest index first) with
//!    its own waker; remove tasks that complete. Stop when nothing is ready.
//!    Return how many polls happened.
//! 4. **`Signal`**: a one-slot channel. `wait()` completes when a value has been
//!    signalled (taking it); otherwise it stores the waker and returns
//!    `Pending`. `signal(v)` stores the value and wakes the stored waker.
//!
//! The tests check *how many times* tasks are polled: a waiting task must not
//! be polled again until someone wakes it.
#![no_std]

use core::cell::{Cell, RefCell};
use core::future::{poll_fn, Future};
use core::pin::Pin;
use core::sync::atomic::{AtomicU32, Ordering};
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

pub const MAX_EXECUTORS: usize = 8;

/// One ready-mask per executor id.
pub static READY: [AtomicU32; MAX_EXECUTORS] = [const { AtomicU32::new(0) }; MAX_EXECUTORS];

static VTABLE: RawWakerVTable = RawWakerVTable::new(waker_clone, waker_wake, waker_wake_by_ref, waker_drop);

unsafe fn waker_clone(data: *const ()) -> RawWaker {
    todo!()
}

unsafe fn waker_wake(data: *const ()) {
    todo!()
}

unsafe fn waker_wake_by_ref(data: *const ()) {
    todo!()
}

unsafe fn waker_drop(_data: *const ()) {}

pub fn waker_for(exec: usize, task: usize) -> Waker {
    todo!()
}

pub type Task<'a> = Pin<&'a mut dyn Future<Output = ()>>;

pub struct Executor<'a, const N: usize> {
    id: usize,
    tasks: [Option<Task<'a>>; N],
}

impl<'a, const N: usize> Executor<'a, N> {
    pub fn new(id: usize) -> Self {
        assert!(id < MAX_EXECUTORS && N <= 32);
        READY[id].store(0, Ordering::Relaxed);
        Executor { id, tasks: [const { None }; N] }
    }

    pub fn spawn(&mut self, task: Task<'a>) -> Result<usize, Task<'a>> {
        todo!()
    }

    pub fn run_until_idle(&mut self) -> usize {
        todo!()
    }

    pub fn live_tasks(&self) -> usize {
        self.tasks.iter().filter(|t| t.is_some()).count()
    }
}

/// A single-slot, single-threaded signal (like `embassy_sync::signal::Signal`).
pub struct Signal<T> {
    value: Cell<Option<T>>,
    waker: RefCell<Option<Waker>>,
}

impl<T> Signal<T> {
    pub const fn new() -> Self {
        Signal { value: Cell::new(None), waker: RefCell::new(None) }
    }

    pub fn signal(&self, v: T) {
        todo!()
    }

    pub fn wait(&self) -> impl Future<Output = T> + '_ {
        poll_fn(|_cx| -> Poll<T> { todo!() })
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use core::pin::pin;
    use std::vec::Vec;

    #[test]
    fn wakers_set_ready_bits() {
        READY[7].store(0, Ordering::Relaxed);
        let w = waker_for(7, 5);
        w.wake_by_ref();
        assert_eq!(READY[7].load(Ordering::Relaxed), 1 << 5);
        let w2 = w.clone();
        w2.wake();
        drop(w);
        assert_eq!(READY[7].load(Ordering::Relaxed), 1 << 5);
        waker_for(7, 0).wake();
        assert_eq!(READY[7].load(Ordering::Relaxed), (1 << 5) | 1);
    }

    #[test]
    fn runs_tasks_to_completion() {
        let mut ex = Executor::<4>::new(1);
        let mut a = pin!(async {});
        let mut b = pin!(async {});
        assert_eq!(ex.spawn(a.as_mut()).ok(), Some(0));
        assert_eq!(ex.spawn(b.as_mut()).ok(), Some(1));
        assert_eq!(ex.run_until_idle(), 2);
        assert_eq!(ex.live_tasks(), 0);
        assert_eq!(ex.run_until_idle(), 0);
    }

    #[test]
    fn spawn_fails_when_full() {
        let mut ex = Executor::<1>::new(2);
        let mut a = pin!(async {});
        let mut b = pin!(async {});
        assert!(ex.spawn(a.as_mut()).is_ok());
        assert!(ex.spawn(b.as_mut()).is_err());
    }

    #[test]
    fn waiting_tasks_are_not_polled_until_woken() {
        let sig = Signal::new();
        let log = RefCell::new(Vec::new());
        let mut ex = Executor::<2>::new(3);
        let mut consumer = pin!(async {
            for _ in 0..3 {
                let v: u32 = sig.wait().await;
                log.borrow_mut().push(v);
            }
        });
        assert!(ex.spawn(consumer.as_mut()).is_ok());
        assert_eq!(ex.run_until_idle(), 1, "polled once, then parked");
        assert_eq!(ex.run_until_idle(), 0, "nobody woke it");
        sig.signal(10);
        assert_eq!(ex.run_until_idle(), 1);
        sig.signal(20);
        sig.signal(30); // overwrites 20: a Signal keeps only the latest value
        assert_eq!(ex.run_until_idle(), 1);
        assert_eq!(*log.borrow(), [10, 30]);
        assert_eq!(ex.live_tasks(), 1);
    }

    #[test]
    fn ping_pong_between_tasks() {
        let ping = Signal::new();
        let pong = Signal::new();
        let log = RefCell::new(Vec::new());
        let mut ex = Executor::<2>::new(4);
        let mut a = pin!(async {
            for i in 0..3u32 {
                ping.signal(i);
                let r: u32 = pong.wait().await;
                log.borrow_mut().push(r);
            }
        });
        let mut b = pin!(async {
            for _ in 0..3 {
                let v: u32 = ping.wait().await;
                pong.signal(v * 10);
            }
        });
        assert!(ex.spawn(a.as_mut()).is_ok());
        assert!(ex.spawn(b.as_mut()).is_ok());
        let polls = ex.run_until_idle();
        assert_eq!(*log.borrow(), [0, 10, 20]);
        assert_eq!(ex.live_tasks(), 0);
        assert!(polls <= 8, "tasks must only be polled when woken (got {polls} polls)");
    }
}
