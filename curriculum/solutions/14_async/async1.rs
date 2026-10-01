//! # async1: async/await with nothing but core
//!
//! Everything async needs except the executor is in `core`. Build a tiny
//! toolkit:
//!
//! 1. `block_on(fut)`: pin the future on the stack (`core::pin::pin!`), make a
//!    `Context` from `Waker::noop()`, and poll in a loop until `Ready`.
//!    `block_on_with(fut, idle)` also calls `idle()` after every `Pending`
//!    (on a microcontroller that's where `wfi` would go; in the tests it
//!    advances a fake clock).
//! 2. `YieldNow`: a future that returns `Pending` once (waking itself first:
//!    `cx.waker().wake_by_ref()`), then `Ready(())`.
//! 3. `join2(a, b)`: run two futures concurrently; finish when both have.
//!    Poll `a` before `b`, and never poll a future again after it completed.
//! 4. `select2(a, b)`: finish with whichever completes first (`a` wins ties);
//!    the other is dropped (that's cancellation in async Rust).
//! 5. `sleep_until(clock, t)`: ready once `clock.now() >= t`.
//!
//! `core::future::poll_fn(|cx| ...)` turns a closure into a future, which is
//! the easiest way to write 3-5. Pin local futures with `pin!` so you can call
//! `.as_mut().poll(cx)` on them.
#![no_std]

use core::cell::Cell;
use core::future::{poll_fn, Future};
use core::pin::{pin, Pin};
use core::task::{Context, Poll, Waker};

pub fn block_on<F: Future>(fut: F) -> F::Output {
    block_on_with(fut, || {})
}

pub fn block_on_with<F: Future>(fut: F, mut idle: impl FnMut()) -> F::Output {
    let mut fut = pin!(fut);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return v;
        }
        idle();
    }
}

pub struct YieldNow {
    yielded: bool,
}

pub fn yield_now() -> YieldNow {
    YieldNow { yielded: false }
}

impl Future for YieldNow {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.yielded {
            Poll::Ready(())
        } else {
            self.yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

pub async fn join2<A: Future, B: Future>(a: A, b: B) -> (A::Output, B::Output) {
    let mut a = pin!(a);
    let mut b = pin!(b);
    let mut ra = None;
    let mut rb = None;
    poll_fn(|cx| {
        if ra.is_none() {
            if let Poll::Ready(v) = a.as_mut().poll(cx) {
                ra = Some(v);
            }
        }
        if rb.is_none() {
            if let Poll::Ready(v) = b.as_mut().poll(cx) {
                rb = Some(v);
            }
        }
        match (ra.take(), rb.take()) {
            (Some(x), Some(y)) => Poll::Ready((x, y)),
            (x, y) => {
                ra = x;
                rb = y;
                Poll::Pending
            }
        }
    })
    .await
}

#[derive(Debug, PartialEq, Eq)]
pub enum Either<A, B> {
    Left(A),
    Right(B),
}

pub async fn select2<A: Future, B: Future>(a: A, b: B) -> Either<A::Output, B::Output> {
    let mut a = pin!(a);
    let mut b = pin!(b);
    poll_fn(|cx| {
        if let Poll::Ready(v) = a.as_mut().poll(cx) {
            return Poll::Ready(Either::Left(v));
        }
        if let Poll::Ready(v) = b.as_mut().poll(cx) {
            return Poll::Ready(Either::Right(v));
        }
        Poll::Pending
    })
    .await
}

/// A fake monotonic clock (in an MCU: a hardware timer).
pub struct Clock {
    now: Cell<u32>,
}

impl Clock {
    pub const fn new() -> Self {
        Clock { now: Cell::new(0) }
    }
    pub fn now(&self) -> u32 {
        self.now.get()
    }
    pub fn advance(&self, dt: u32) {
        self.now.set(self.now.get() + dt);
    }
}

pub fn sleep_until(clock: &Clock, deadline: u32) -> impl Future<Output = ()> + '_ {
    poll_fn(move |cx| {
        if clock.now() >= deadline {
            Poll::Ready(())
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use core::cell::RefCell;
    use std::vec::Vec;

    #[test]
    fn runs_simple_futures() {
        assert_eq!(block_on(async { 6 * 7 }), 42);
        let mut polls = 0;
        block_on_with(
            async {
                yield_now().await;
                yield_now().await;
            },
            || polls += 1,
        );
        assert_eq!(polls, 2, "each yield_now returns Pending exactly once");
    }

    #[test]
    fn join_interleaves() {
        let log = RefCell::new(Vec::new());
        let task = |name: char, steps: u32| {
            let log = &log;
            async move {
                for i in 0..steps {
                    log.borrow_mut().push((name, i));
                    yield_now().await;
                }
                name
            }
        };
        let out = block_on(join2(task('a', 3), task('b', 2)));
        assert_eq!(out, ('a', 'b'));
        assert_eq!(*log.borrow(), [('a', 0), ('b', 0), ('a', 1), ('b', 1), ('a', 2)]);
    }

    #[test]
    fn join_never_repolls_completed_futures() {
        // A future that panics if polled after completion, like many real ones.
        struct Once(bool);
        impl Future for Once {
            type Output = u8;
            fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<u8> {
                assert!(!self.0, "polled after completion");
                self.0 = true;
                Poll::Ready(1)
            }
        }
        let slow = async {
            for _ in 0..3 {
                yield_now().await;
            }
            2
        };
        assert_eq!(block_on(join2(Once(false), slow)), (1, 2));
    }

    #[test]
    fn select_cancels_the_loser() {
        let clock = Clock::new();
        let work = async {
            sleep_until(&clock, 10).await;
            "done"
        };
        let timeout = sleep_until(&clock, 5);
        let r = block_on_with(select2(work, timeout), || clock.advance(1));
        assert_eq!(r, Either::Right(()));
        assert_eq!(clock.now(), 5);
        let r = block_on(select2(async { 1 }, async { 2 }));
        assert_eq!(r, Either::Left(1), "a wins ties");
    }

    #[test]
    fn sleeping_with_a_fake_clock() {
        let clock = Clock::new();
        let out = block_on_with(
            async {
                sleep_until(&clock, 3).await;
                let t1 = clock.now();
                sleep_until(&clock, 7).await;
                (t1, clock.now())
            },
            || clock.advance(1),
        );
        assert_eq!(out, (3, 7));
    }
}
