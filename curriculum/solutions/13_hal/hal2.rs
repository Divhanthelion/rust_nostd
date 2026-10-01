//! # hal2: An I2C temperature sensor driver
//!
//! A driver for a (fictional, LM75-like) temperature sensor, generic over the
//! embedded-hal 1.0 `I2c` trait. Register protocol, as on most I2C sensors:
//! write the register address, then read; or write the address followed by
//! data bytes.
//!
//! | Register | Address | Contents |
//! |---|---|---|
//! | TEMP     | 0x00 | 2 bytes, **big-endian i16**, in 1/256 °C |
//! | CONFIG   | 0x01 | bit 0: shutdown; bits 5-6: resolution |
//! | WHO_AM_I | 0x0F | always `0xA1` |
//!
//! Implement:
//! - `init`: read WHO_AM_I (`write_read`); anything but `0xA1` →
//!   `Error::WrongDevice(id)`.
//! - `temperature_milli`: read TEMP and convert to milli-°C:
//!   `raw * 1000 / 256`, computed in `i32` (rounds toward zero).
//! - `set_shutdown(on)`: **read-modify-write** CONFIG, changing only bit 0,
//!   with a single `write` of `[0x01, new_value]`.
//! - `release`: give the bus back.
//! - Every bus error is wrapped as `Error::Bus(e)` (`map_err`), never unwrapped.
#![no_std]

pub mod hal {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ErrorKind {
        Bus,
        NoAcknowledge,
        Other,
    }
    pub trait Error: core::fmt::Debug {
        fn kind(&self) -> ErrorKind;
    }
    pub trait ErrorType {
        type Error: Error;
    }
    pub type SevenBitAddress = u8;
    pub enum Operation<'a> {
        Read(&'a mut [u8]),
        Write(&'a [u8]),
    }
    /// Same shape as `embedded_hal::i2c::I2c`: only `transaction` is required.
    pub trait I2c: ErrorType {
        fn transaction(&mut self, address: SevenBitAddress, operations: &mut [Operation<'_>]) -> Result<(), Self::Error>;
        fn read(&mut self, address: SevenBitAddress, read: &mut [u8]) -> Result<(), Self::Error> {
            self.transaction(address, &mut [Operation::Read(read)])
        }
        fn write(&mut self, address: SevenBitAddress, write: &[u8]) -> Result<(), Self::Error> {
            self.transaction(address, &mut [Operation::Write(write)])
        }
        fn write_read(&mut self, address: SevenBitAddress, write: &[u8], read: &mut [u8]) -> Result<(), Self::Error> {
            self.transaction(address, &mut [Operation::Write(write), Operation::Read(read)])
        }
    }
}

use hal::I2c;

pub const DEFAULT_ADDRESS: u8 = 0x48;
const REG_TEMP: u8 = 0x00;
const REG_CONFIG: u8 = 0x01;
const REG_WHO_AM_I: u8 = 0x0F;
const DEVICE_ID: u8 = 0xA1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error<E> {
    Bus(E),
    WrongDevice(u8),
}

pub struct Tmp<I2C> {
    i2c: I2C,
    address: u8,
}

impl<I2C: I2c> Tmp<I2C> {
    pub fn new(i2c: I2C, address: u8) -> Self {
        Tmp { i2c, address }
    }

    fn read_reg<const N: usize>(&mut self, reg: u8) -> Result<[u8; N], Error<I2C::Error>> {
        let mut buf = [0u8; N];
        self.i2c.write_read(self.address, &[reg], &mut buf).map_err(Error::Bus)?;
        Ok(buf)
    }

    pub fn init(&mut self) -> Result<(), Error<I2C::Error>> {
        let [id] = self.read_reg::<1>(REG_WHO_AM_I)?;
        if id != DEVICE_ID {
            return Err(Error::WrongDevice(id));
        }
        Ok(())
    }

    pub fn temperature_milli(&mut self) -> Result<i32, Error<I2C::Error>> {
        let raw = i16::from_be_bytes(self.read_reg::<2>(REG_TEMP)?);
        Ok(i32::from(raw) * 1000 / 256)
    }

    pub fn set_shutdown(&mut self, on: bool) -> Result<(), Error<I2C::Error>> {
        let [cfg] = self.read_reg::<1>(REG_CONFIG)?;
        let new = if on { cfg | 0x01 } else { cfg & !0x01 };
        self.i2c.write(self.address, &[REG_CONFIG, new]).map_err(Error::Bus)
    }

    pub fn release(self) -> I2C {
        self.i2c
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::hal::{ErrorKind, ErrorType, Operation};
    use super::*;
    use std::vec::Vec;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Nack;
    impl hal::Error for Nack {
        fn kind(&self) -> ErrorKind {
            ErrorKind::NoAcknowledge
        }
    }

    /// Simulates the sensor: a register file with an auto-incrementing pointer.
    struct FakeSensor {
        address: u8,
        regs: [u8; 16],
        pointer: usize,
        writes: Vec<Vec<u8>>,
        present: bool,
    }

    impl FakeSensor {
        fn new() -> Self {
            let mut regs = [0u8; 16];
            regs[0x0F] = 0xA1;
            regs[0x01] = 0b0110_0000;
            FakeSensor { address: 0x48, regs, pointer: 0, writes: Vec::new(), present: true }
        }
        fn set_temp(&mut self, raw: i16) {
            let b = raw.to_be_bytes();
            self.regs[0] = b[0];
            self.regs[1] = b[1];
        }
    }

    impl ErrorType for FakeSensor {
        type Error = Nack;
    }

    impl I2c for FakeSensor {
        fn transaction(&mut self, address: u8, ops: &mut [Operation<'_>]) -> Result<(), Nack> {
            if address != self.address || !self.present {
                return Err(Nack);
            }
            for op in ops {
                match op {
                    Operation::Write(bytes) => {
                        self.writes.push(bytes.to_vec());
                        if let Some((&reg, data)) = bytes.split_first() {
                            self.pointer = usize::from(reg);
                            for &d in data {
                                self.regs[self.pointer % 16] = d;
                                self.pointer += 1;
                            }
                        }
                    }
                    Operation::Read(buf) => {
                        for b in buf.iter_mut() {
                            *b = self.regs[self.pointer % 16];
                            self.pointer += 1;
                        }
                    }
                }
            }
            Ok(())
        }
    }

    #[test]
    fn identifies_the_device() {
        let mut t = Tmp::new(FakeSensor::new(), DEFAULT_ADDRESS);
        assert_eq!(t.init(), Ok(()));
        let mut fake = FakeSensor::new();
        fake.regs[0x0F] = 0x42;
        let mut t = Tmp::new(fake, DEFAULT_ADDRESS);
        assert_eq!(t.init(), Err(Error::WrongDevice(0x42)));
    }

    #[test]
    fn converts_temperatures() {
        let cases = [(0x1900, 25_000), (-0x1900, -25_000), (0x0080, 500), (0x0001, 3), (-1, -3), (0x7FFF, 127_996), (i16::MIN, -128_000)];
        for (raw, milli) in cases {
            let mut fake = FakeSensor::new();
            fake.set_temp(raw);
            let mut t = Tmp::new(fake, DEFAULT_ADDRESS);
            assert_eq!(t.temperature_milli(), Ok(milli), "raw {raw:#06x}");
        }
    }

    #[test]
    fn shutdown_is_read_modify_write() {
        let mut t = Tmp::new(FakeSensor::new(), DEFAULT_ADDRESS);
        t.set_shutdown(true).unwrap();
        t.set_shutdown(false).unwrap();
        t.set_shutdown(true).unwrap();
        let fake = t.release();
        assert_eq!(fake.regs[0x01], 0b0110_0001, "resolution bits must survive");
        let config_writes: Vec<_> = fake.writes.iter().filter(|w| w.len() == 2).cloned().collect();
        assert_eq!(config_writes, [std::vec![0x01, 0x61], std::vec![0x01, 0x60], std::vec![0x01, 0x61]]);
    }

    #[test]
    fn bus_errors_are_wrapped() {
        let mut fake = FakeSensor::new();
        fake.present = false;
        let mut t = Tmp::new(fake, DEFAULT_ADDRESS);
        assert_eq!(t.init(), Err(Error::Bus(Nack)));
        assert_eq!(t.temperature_milli(), Err(Error::Bus(Nack)));
        assert_eq!(t.set_shutdown(true), Err(Error::Bus(Nack)));
        let mut wrong_addr = Tmp::new(FakeSensor::new(), 0x49);
        assert_eq!(wrong_addr.init(), Err(Error::Bus(Nack)));
    }
}
