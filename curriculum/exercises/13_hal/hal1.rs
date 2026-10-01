//! # hal1: Generic drivers for pins and delays
//!
//! Write three tiny drivers that work with *any* microcontroller, because
//! they only use the embedded-hal traits. (The traits are copied into `mod hal`
//! with the same shapes as embedded-hal 1.0, so this file needs no
//! dependencies; in a real project you'd `use embedded_hal::...`.)
//!
//! 1. `Blinky<P, D>`: `blink(times, on_ms, off_ms)` sets the pin high, waits
//!    `on_ms`, sets it low, waits `off_ms`, `times` times (using `delay_ms`).
//!    Errors from the pin are returned immediately. `release()` gives the
//!    pin and delay back.
//! 2. `LedBar<P, N>`: `show(level)` lights the first `level` LEDs and turns
//!    the rest off (`level` larger than N lights all of them). Every LED is
//!    written on every call (`set_state` is handy).
//! 3. `Debouncer<I>`: `poll()` samples the input once. The *stable* state
//!    changes only after `THRESHOLD` consecutive samples that agree with each
//!    other and differ from the stable state. It then returns `Some(Edge)`;
//!    otherwise `None`. The initial stable state is low.
//!
//! The tests use mock pins that record what your drivers do.
#![no_std]

pub mod hal {
    pub mod digital {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ErrorKind {
            Other,
        }
        pub trait Error: core::fmt::Debug {
            fn kind(&self) -> ErrorKind;
        }
        impl Error for core::convert::Infallible {
            fn kind(&self) -> ErrorKind {
                match *self {}
            }
        }
        pub trait ErrorType {
            type Error: Error;
        }
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum PinState {
            Low,
            High,
        }
        pub trait OutputPin: ErrorType {
            fn set_low(&mut self) -> Result<(), Self::Error>;
            fn set_high(&mut self) -> Result<(), Self::Error>;
            fn set_state(&mut self, state: PinState) -> Result<(), Self::Error> {
                match state {
                    PinState::Low => self.set_low(),
                    PinState::High => self.set_high(),
                }
            }
        }
        pub trait InputPin: ErrorType {
            fn is_high(&mut self) -> Result<bool, Self::Error>;
            fn is_low(&mut self) -> Result<bool, Self::Error> {
                self.is_high().map(|h| !h)
            }
        }
    }

    pub mod delay {
        pub trait DelayNs {
            fn delay_ns(&mut self, ns: u32);
            fn delay_us(&mut self, mut us: u32) {
                const MAX: u32 = u32::MAX / 1_000;
                while us > MAX {
                    us -= MAX;
                    self.delay_ns(MAX * 1_000);
                }
                self.delay_ns(us * 1_000);
            }
            fn delay_ms(&mut self, mut ms: u32) {
                const MAX: u32 = u32::MAX / 1_000_000;
                while ms > MAX {
                    ms -= MAX;
                    self.delay_ns(MAX * 1_000_000);
                }
                self.delay_ns(ms * 1_000_000);
            }
        }
    }
}

use hal::delay::DelayNs;
use hal::digital::{InputPin, OutputPin, PinState};

pub struct Blinky<P, D> {
    pin: P,
    delay: D,
}

impl<P: OutputPin, D: DelayNs> Blinky<P, D> {
    pub fn new(pin: P, delay: D) -> Self {
        Blinky { pin, delay }
    }

    pub fn blink(&mut self, times: u32, on_ms: u32, off_ms: u32) -> Result<(), P::Error> {
        todo!()
    }

    pub fn release(self) -> (P, D) {
        todo!()
    }
}

pub struct LedBar<P, const N: usize> {
    leds: [P; N],
}

impl<P: OutputPin, const N: usize> LedBar<P, N> {
    pub fn new(leds: [P; N]) -> Self {
        LedBar { leds }
    }

    pub fn show(&mut self, level: usize) -> Result<(), P::Error> {
        todo!()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Rising,
    Falling,
}

pub const THRESHOLD: u8 = 3;

pub struct Debouncer<I> {
    input: I,
    stable_high: bool,
    count: u8,
}

impl<I: InputPin> Debouncer<I> {
    pub fn new(input: I) -> Self {
        Debouncer { input, stable_high: false, count: 0 }
    }

    pub fn poll(&mut self) -> Result<Option<Edge>, I::Error> {
        todo!()
    }

    pub fn is_high(&self) -> bool {
        self.stable_high
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::hal::digital::{ErrorKind, ErrorType};
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::vec::Vec;

    #[derive(Debug, Clone, PartialEq)]
    enum Ev {
        High(u8),
        Low(u8),
        Wait(u64),
    }

    type Log = Rc<RefCell<Vec<Ev>>>;

    #[derive(Debug, PartialEq)]
    struct PinError;
    impl hal::digital::Error for PinError {
        fn kind(&self) -> ErrorKind {
            ErrorKind::Other
        }
    }

    struct MockPin {
        id: u8,
        log: Log,
        fail_after: Option<usize>,
    }
    impl ErrorType for MockPin {
        type Error = PinError;
    }
    impl MockPin {
        fn record(&mut self, e: Ev) -> Result<(), PinError> {
            if let Some(n) = self.fail_after.as_mut() {
                if *n == 0 {
                    return Err(PinError);
                }
                *n -= 1;
            }
            self.log.borrow_mut().push(e);
            Ok(())
        }
    }
    impl OutputPin for MockPin {
        fn set_low(&mut self) -> Result<(), PinError> {
            let id = self.id;
            self.record(Ev::Low(id))
        }
        fn set_high(&mut self) -> Result<(), PinError> {
            let id = self.id;
            self.record(Ev::High(id))
        }
    }

    struct MockDelay(Log);
    impl DelayNs for MockDelay {
        fn delay_ns(&mut self, ns: u32) {
            self.0.borrow_mut().push(Ev::Wait(u64::from(ns)));
        }
    }

    struct Samples(Vec<bool>);
    impl ErrorType for Samples {
        type Error = core::convert::Infallible;
    }
    impl InputPin for Samples {
        fn is_high(&mut self) -> Result<bool, Self::Error> {
            Ok(self.0.remove(0))
        }
    }

    #[test]
    fn blinks() {
        let log: Log = Rc::default();
        let pin = MockPin { id: 1, log: log.clone(), fail_after: None };
        let mut b = Blinky::new(pin, MockDelay(log.clone()));
        b.blink(2, 100, 50).unwrap();
        assert_eq!(
            *log.borrow(),
            [Ev::High(1), Ev::Wait(100_000_000), Ev::Low(1), Ev::Wait(50_000_000), Ev::High(1), Ev::Wait(100_000_000), Ev::Low(1), Ev::Wait(50_000_000)]
        );
        let (pin, _delay) = b.release();
        assert_eq!(pin.id, 1);
    }

    #[test]
    fn blink_stops_on_error() {
        let log: Log = Rc::default();
        let pin = MockPin { id: 7, log: log.clone(), fail_after: Some(3) };
        let mut b = Blinky::new(pin, MockDelay(log.clone()));
        assert_eq!(b.blink(5, 1, 1), Err(PinError));
        assert_eq!(*log.borrow(), [Ev::High(7), Ev::Wait(1_000_000), Ev::Low(7), Ev::Wait(1_000_000), Ev::High(7), Ev::Wait(1_000_000)]);
    }

    #[test]
    fn led_bar_levels() {
        let log: Log = Rc::default();
        let leds = core::array::from_fn(|i| MockPin { id: i as u8, log: log.clone(), fail_after: None });
        let mut bar: LedBar<MockPin, 4> = LedBar::new(leds);
        bar.show(2).unwrap();
        assert_eq!(*log.borrow(), [Ev::High(0), Ev::High(1), Ev::Low(2), Ev::Low(3)]);
        log.borrow_mut().clear();
        bar.show(9).unwrap();
        assert_eq!(*log.borrow(), [Ev::High(0), Ev::High(1), Ev::High(2), Ev::High(3)]);
        log.borrow_mut().clear();
        bar.show(0).unwrap();
        assert!(log.borrow().iter().all(|e| matches!(e, Ev::Low(_))));
    }

    #[test]
    fn debounces() {
        let samples = [false, true, false, true, true, true, true, false, false, true, false, false, false];
        let mut d = Debouncer::new(Samples(samples.to_vec()));
        let events: Vec<Option<Edge>> = (0..samples.len()).map(|_| d.poll().unwrap()).collect();
        assert_eq!(
            events,
            [None, None, None, None, None, Some(Edge::Rising), None, None, None, None, None, None, Some(Edge::Falling)]
        );
        assert!(!d.is_high());
    }
}
