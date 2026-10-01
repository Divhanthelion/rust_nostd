//! # mmio1: A volatile register driver
//!
//! Write a driver for an STM32-style GPIO port. On real hardware the
//! register block lives at a fixed address (e.g. `0x4002_0000`); in the tests
//! it lives in ordinary memory so you can check what the driver writes.
//!
//! Register map (`#[repr(C)]`, all `u32`):
//!
//! | offset | name    | meaning |
//! |--------|---------|---------|
//! | 0x00   | MODER   | 2 bits per pin: 00 input, 01 output, 10 alternate, 11 analog |
//! | 0x04   | OTYPER  | output type |
//! | 0x08   | OSPEEDR | speed |
//! | 0x0C   | PUPDR   | pull-up/down |
//! | 0x10   | IDR     | input data (read-only) |
//! | 0x14   | ODR     | output data |
//! | 0x18   | BSRR    | bit set/reset (write-only): bit n sets pin n, bit n+16 resets pin n |
//!
//! Rules:
//! - every register access is **volatile** (`ptr::read_volatile`/`write_volatile`)
//!   through a raw pointer to the field: `&raw mut (*self.regs).moder`.
//!   Don't create `&`/`&mut` references to the register block;
//! - `set_high`/`set_low` use a **single write to BSRR**, with no read-modify-write
//!   of ODR (so an interrupt can't lose an update);
//! - pins are 0..=15; anything else is `Err(PinError)`.
#![no_std]

use core::ptr::{read_volatile, write_volatile};

#[repr(C)]
#[derive(Debug, Default)]
pub struct GpioRegisters {
    pub moder: u32,
    pub otyper: u32,
    pub ospeedr: u32,
    pub pupdr: u32,
    pub idr: u32,
    pub odr: u32,
    pub bsrr: u32,
}

// Compile-time layout checks: if these fail, the driver would poke the wrong
// registers. (A failing const assertion is a compile error.)
const _: () = assert!(core::mem::offset_of!(GpioRegisters, idr) == 0x10);
const _: () = assert!(core::mem::offset_of!(GpioRegisters, bsrr) == 0x18);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Input = 0b00,
    Output = 0b01,
    Alternate = 0b10,
    Analog = 0b11,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinError(pub u8);

pub struct Gpio {
    regs: *mut GpioRegisters,
}

impl Gpio {
    /// # Safety
    /// `regs` must point to a GPIO register block that stays valid while the
    /// `Gpio` exists, and no other code may access that block meanwhile.
    pub unsafe fn new(regs: *mut GpioRegisters) -> Self {
        Gpio { regs }
    }

    fn check(pin: u8) -> Result<u32, PinError> {
        if pin < 16 { Ok(u32::from(pin)) } else { Err(PinError(pin)) }
    }

    /// Program the 2-bit MODER field of `pin` (read-modify-write).
    pub fn set_mode(&mut self, pin: u8, mode: Mode) -> Result<(), PinError> {
        let p = Self::check(pin)?;
        // SAFETY: `regs` is valid per `new`'s contract; volatile because it's MMIO.
        unsafe {
            let moder = &raw mut (*self.regs).moder;
            let v = read_volatile(moder);
            let v = (v & !(0b11 << (2 * p))) | ((mode as u32) << (2 * p));
            write_volatile(moder, v);
        }
        Ok(())
    }

    pub fn mode(&self, pin: u8) -> Result<Mode, PinError> {
        let p = Self::check(pin)?;
        // SAFETY: as above.
        let bits = unsafe { read_volatile(&raw const (*self.regs).moder) } >> (2 * p) & 0b11;
        Ok(match bits {
            0b00 => Mode::Input,
            0b01 => Mode::Output,
            0b10 => Mode::Alternate,
            _ => Mode::Analog,
        })
    }

    pub fn set_high(&mut self, pin: u8) -> Result<(), PinError> {
        let p = Self::check(pin)?;
        // SAFETY: as above. A single write: atomic with respect to interrupts.
        unsafe { write_volatile(&raw mut (*self.regs).bsrr, 1 << p) };
        Ok(())
    }

    pub fn set_low(&mut self, pin: u8) -> Result<(), PinError> {
        let p = Self::check(pin)?;
        // SAFETY: as above.
        unsafe { write_volatile(&raw mut (*self.regs).bsrr, 1 << (p + 16)) };
        Ok(())
    }

    pub fn is_high(&self, pin: u8) -> Result<bool, PinError> {
        let p = Self::check(pin)?;
        // SAFETY: as above.
        Ok(unsafe { read_volatile(&raw const (*self.regs).idr) } & (1 << p) != 0)
    }

    /// Write all 16 output bits at once.
    pub fn write_port(&mut self, value: u16) {
        // SAFETY: as above.
        unsafe { write_volatile(&raw mut (*self.regs).odr, u32::from(value)) };
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::boxed::Box;

    /// Fake register block in host memory, accessed only through raw pointers.
    struct Fake(*mut GpioRegisters);
    impl Fake {
        fn new() -> Self {
            Fake(Box::into_raw(Box::new(GpioRegisters::default())))
        }
        fn read(&self, f: fn(*mut GpioRegisters) -> *mut u32) -> u32 {
            unsafe { read_volatile(f(self.0)) }
        }
        fn write(&self, f: fn(*mut GpioRegisters) -> *mut u32, v: u32) {
            unsafe { write_volatile(f(self.0), v) }
        }
    }
    impl Drop for Fake {
        fn drop(&mut self) {
            drop(unsafe { Box::from_raw(self.0) });
        }
    }
    fn moder(p: *mut GpioRegisters) -> *mut u32 { unsafe { &raw mut (*p).moder } }
    fn idr(p: *mut GpioRegisters) -> *mut u32 { unsafe { &raw mut (*p).idr } }
    fn odr(p: *mut GpioRegisters) -> *mut u32 { unsafe { &raw mut (*p).odr } }
    fn bsrr(p: *mut GpioRegisters) -> *mut u32 { unsafe { &raw mut (*p).bsrr } }

    #[test]
    fn modes_are_two_bit_fields() {
        let hw = Fake::new();
        let mut gpio = unsafe { Gpio::new(hw.0) };
        gpio.set_mode(5, Mode::Output).unwrap();
        assert_eq!(hw.read(moder), 0b01 << 10);
        gpio.set_mode(0, Mode::Analog).unwrap();
        gpio.set_mode(15, Mode::Alternate).unwrap();
        assert_eq!(hw.read(moder), (0b10 << 30) | (0b01 << 10) | 0b11);
        gpio.set_mode(5, Mode::Input).unwrap();
        assert_eq!(hw.read(moder), (0b10 << 30) | 0b11, "other pins untouched");
        assert_eq!(gpio.mode(15), Ok(Mode::Alternate));
        assert_eq!(gpio.mode(0), Ok(Mode::Analog));
        assert_eq!(gpio.mode(5), Ok(Mode::Input));
    }

    #[test]
    fn set_and_reset_use_bsrr_only() {
        let hw = Fake::new();
        hw.write(odr, 0xAAAA);
        let mut gpio = unsafe { Gpio::new(hw.0) };
        gpio.set_high(3).unwrap();
        assert_eq!(hw.read(bsrr), 1 << 3);
        gpio.set_low(3).unwrap();
        assert_eq!(hw.read(bsrr), 1 << 19);
        gpio.set_low(15).unwrap();
        assert_eq!(hw.read(bsrr), 1 << 31);
        assert_eq!(hw.read(odr), 0xAAAA, "ODR must not be read-modify-written");
    }

    #[test]
    fn inputs_and_port_writes() {
        let hw = Fake::new();
        hw.write(idr, 0b1000_0001);
        let mut gpio = unsafe { Gpio::new(hw.0) };
        assert_eq!(gpio.is_high(0), Ok(true));
        assert_eq!(gpio.is_high(1), Ok(false));
        assert_eq!(gpio.is_high(7), Ok(true));
        gpio.write_port(0xBEEF);
        assert_eq!(hw.read(odr), 0xBEEF);
    }

    #[test]
    fn bad_pins() {
        let hw = Fake::new();
        let mut gpio = unsafe { Gpio::new(hw.0) };
        assert_eq!(gpio.set_mode(16, Mode::Output), Err(PinError(16)));
        assert_eq!(gpio.set_high(200), Err(PinError(200)));
        assert_eq!(gpio.set_low(16), Err(PinError(16)));
        assert_eq!(gpio.is_high(16), Err(PinError(16)));
        assert_eq!(gpio.mode(16), Err(PinError(16)));
        assert_eq!(hw.read(moder) | hw.read(bsrr), 0);
    }
}
