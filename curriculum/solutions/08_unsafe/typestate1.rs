//! # typestate1: Make illegal hardware states unrepresentable
//!
//! Build a type-state GPIO API in the style of the embedded HALs:
//!
//! - `Peripherals::take()` returns the port **once** (a singleton guarded by
//!   an atomic flag); every later call returns `None`. One owner per piece of
//!   hardware.
//! - `Port::split()` hands out each pin exactly once, in `Input` mode.
//! - A pin's mode is a type parameter: only `Pin<N, Output>` has `set_high`,
//!   only `Pin<N, Input>` has `is_high`. Mode changes **consume** the pin and
//!   return it in the new type, so a stale handle can't be used.
//! - The pin number is a const generic `N`: it costs nothing at runtime.
//!
//! The "hardware" is a `Port` with `Cell<u32>` registers (MODER: 2 bits per
//! pin, 00 input / 01 output / 11 analog; ODR: output bits; IDR: input bits),
//! so the tests can observe what your code does.
//!
//! Things this API should make *impossible* (they'd be compile errors):
//! `pin.set_high()` on an input, using a pin after `into_output(pin)`, taking
//! the peripherals twice and getting two owners.
#![no_std]

use core::cell::Cell;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicBool, Ordering};

pub struct Input;
pub struct Output;
pub struct Analog;

/// Simulated GPIO port registers.
pub struct Port {
    pub moder: Cell<u32>,
    pub odr: Cell<u32>,
    pub idr: Cell<u32>,
}

pub struct Peripherals {
    pub gpioa: Port,
}

static TAKEN: AtomicBool = AtomicBool::new(false);

impl Peripherals {
    /// The one and only instance.
    pub fn take() -> Option<Peripherals> {
        if TAKEN.swap(true, Ordering::AcqRel) {
            None
        } else {
            Some(Peripherals { gpioa: Port::new() })
        }
    }
}

pub struct Pin<'p, const N: u8, MODE> {
    port: &'p Port,
    _mode: PhantomData<MODE>,
}

pub struct Pins<'p> {
    pub pa0: Pin<'p, 0, Input>,
    pub pa5: Pin<'p, 5, Input>,
    pub pa9: Pin<'p, 9, Input>,
}

impl Port {
    /// A fresh port in reset state (all registers zero = all pins inputs).
    pub fn new() -> Self {
        Port { moder: Cell::new(0), odr: Cell::new(0), idr: Cell::new(0) }
    }

    pub fn split(&mut self) -> Pins<'_> {
        let port = &*self;
        Pins {
            pa0: Pin { port, _mode: PhantomData },
            pa5: Pin { port, _mode: PhantomData },
            pa9: Pin { port, _mode: PhantomData },
        }
    }
}

impl<'p, const N: u8, MODE> Pin<'p, N, MODE> {
    pub fn number(&self) -> u8 {
        N
    }

    /// Program this pin's 2-bit MODER field and change the pin's type.
    fn into_mode<NEW>(self, bits: u32) -> Pin<'p, N, NEW> {
        let shift = 2 * u32::from(N);
        let m = self.port.moder.get();
        self.port.moder.set((m & !(0b11 << shift)) | (bits << shift));
        Pin { port: self.port, _mode: PhantomData }
    }

    pub fn into_output(self) -> Pin<'p, N, Output> {
        self.into_mode(0b01)
    }

    pub fn into_input(self) -> Pin<'p, N, Input> {
        self.into_mode(0b00)
    }

    pub fn into_analog(self) -> Pin<'p, N, Analog> {
        self.into_mode(0b11)
    }
}

impl<const N: u8> Pin<'_, N, Input> {
    pub fn is_high(&self) -> bool {
        self.port.idr.get() & (1 << N) != 0
    }
}

impl<const N: u8> Pin<'_, N, Output> {
    pub fn set_high(&mut self) {
        self.port.odr.set(self.port.odr.get() | (1 << N));
    }

    pub fn set_low(&mut self) {
        self.port.odr.set(self.port.odr.get() & !(1 << N));
    }

    pub fn toggle(&mut self) {
        self.port.odr.set(self.port.odr.get() ^ (1 << N));
    }

    pub fn is_set_high(&self) -> bool {
        self.port.odr.get() & (1 << N) != 0
    }
}

/// Drivers can be generic over any output pin.
pub trait OutputPin {
    fn set_high(&mut self);
    fn set_low(&mut self);
}

impl<const N: u8> OutputPin for Pin<'_, N, Output> {
    fn set_high(&mut self) {
        Pin::set_high(self)
    }
    fn set_low(&mut self) {
        Pin::set_low(self)
    }
}

/// Pulse a pin `n` times (high then low).
pub fn pulse<P: OutputPin>(pin: &mut P, n: usize) {
    for _ in 0..n {
        pin.set_high();
        pin.set_low();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn singleton() {
        let p = Peripherals::take();
        assert!(p.is_some());
        assert!(Peripherals::take().is_none());
        assert!(Peripherals::take().is_none());
    }

    #[test]
    fn modes_and_outputs() {
        let mut port = Port::new();
        let pins = port.split();
        let mut led = pins.pa5.into_output();
        assert_eq!(pins.pa0.port.moder.get(), 0b01 << 10);
        led.set_high();
        assert!(led.is_set_high());
        led.toggle();
        assert!(!led.is_set_high());
        led.toggle();
        let analog = pins.pa9.into_analog();
        assert_eq!(analog.number(), 9);
        assert_eq!(led.port.moder.get(), (0b11 << 18) | (0b01 << 10));
        assert_eq!(led.port.odr.get(), 1 << 5);
        let input = led.into_input();
        assert_eq!(input.port.moder.get(), 0b11 << 18);
    }

    #[test]
    fn inputs() {
        let mut port = Port::new();
        port.idr.set(0b10_0000_0001);
        let pins = port.split();
        assert!(pins.pa0.is_high());
        assert!(!pins.pa5.is_high());
        assert!(pins.pa9.is_high());
    }

    #[test]
    fn generic_drivers() {
        let mut port = Port::new();
        let pins = port.split();
        let mut a = pins.pa0.into_output();
        a.set_high();
        pulse(&mut a, 3);
        assert!(!a.is_set_high(), "pulse ends low");
    }

    #[test]
    fn zero_cost() {
        // The mode and pin number are pure type information.
        assert_eq!(core::mem::size_of::<Pin<'static, 3, Output>>(), core::mem::size_of::<&Port>());
    }
}
