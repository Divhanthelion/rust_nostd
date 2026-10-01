//! # hal3: An SPI CAN-controller driver
//!
//! Many ECUs and dev boards add CAN through an external controller on SPI,
//! like the Microchip MCP2515. Write the command layer of such a driver, generic
//! over the embedded-hal 1.0 `SpiDevice` trait. Each `transaction` asserts
//! chip-select, runs its operations, and deasserts it: **one command = one
//! transaction**.
//!
//! SPI commands (first byte of the transaction):
//!
//! | Command | Bytes on the wire |
//! |---|---|
//! | RESET       | `[0xC0]` |
//! | READ        | `[0x03, addr]`, then read 1 byte |
//! | WRITE       | `[0x02, addr, data...]` (sequential registers) |
//! | BIT MODIFY  | `[0x05, addr, mask, data]`: only bits set in `mask` change |
//!
//! Registers: `CANSTAT` (0x0E) bits 7-5 = current mode; `CANCTRL` (0x0F)
//! bits 7-5 = requested mode; `CNF3`/`CNF2`/`CNF1` at 0x28/0x29/0x2A hold the
//! bit timing.
//!
//! Implement:
//! - `reset`, `read_register`, `write_register`, `bit_modify` (one transaction each);
//! - `set_mode(mode)`: BIT MODIFY CANCTRL with mask `0xE0`, data `mode << 5`;
//!   then read CANSTAT and check bits 7-5. If the controller didn't switch:
//!   `Error::ModeNotEntered { wanted, actual }` (actual = CANSTAT >> 5);
//! - `set_bit_timing(cnf1, cnf2, cnf3)`: only allowed in Configuration mode
//!   (check CANSTAT first, else `Error::NotInConfigMode`); writes all three with
//!   **one** WRITE command starting at CNF3: `[0x02, 0x28, cnf3, cnf2, cnf1]`.
#![no_std]

pub mod hal {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ErrorKind {
        Other,
    }
    pub trait Error: core::fmt::Debug {
        fn kind(&self) -> ErrorKind;
    }
    pub trait ErrorType {
        type Error: Error;
    }
    /// Same shape as `embedded_hal::spi::Operation<'_, u8>`.
    pub enum Operation<'a> {
        Read(&'a mut [u8]),
        Write(&'a [u8]),
        Transfer(&'a mut [u8], &'a [u8]),
        TransferInPlace(&'a mut [u8]),
        DelayNs(u32),
    }
    /// Same shape as `embedded_hal::spi::SpiDevice<u8>`.
    pub trait SpiDevice: ErrorType {
        fn transaction(&mut self, operations: &mut [Operation<'_>]) -> Result<(), Self::Error>;
        fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
            self.transaction(&mut [Operation::Write(buf)])
        }
        fn read(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
            self.transaction(&mut [Operation::Read(buf)])
        }
    }
}

use hal::{Operation, SpiDevice};

pub const CMD_RESET: u8 = 0xC0;
pub const CMD_READ: u8 = 0x03;
pub const CMD_WRITE: u8 = 0x02;
pub const CMD_BIT_MODIFY: u8 = 0x05;
pub const CANSTAT: u8 = 0x0E;
pub const CANCTRL: u8 = 0x0F;
pub const CNF3: u8 = 0x28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Mode {
    Normal = 0b000,
    Sleep = 0b001,
    Loopback = 0b010,
    ListenOnly = 0b011,
    Configuration = 0b100,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error<E> {
    Spi(E),
    ModeNotEntered { wanted: u8, actual: u8 },
    NotInConfigMode,
}

pub struct CanController<SPI> {
    spi: SPI,
}

impl<SPI: SpiDevice> CanController<SPI> {
    pub fn new(spi: SPI) -> Self {
        CanController { spi }
    }

    pub fn reset(&mut self) -> Result<(), Error<SPI::Error>> {
        self.spi.write(&[CMD_RESET]).map_err(Error::Spi)
    }

    pub fn read_register(&mut self, addr: u8) -> Result<u8, Error<SPI::Error>> {
        let mut buf = [0u8; 1];
        self.spi
            .transaction(&mut [Operation::Write(&[CMD_READ, addr]), Operation::Read(&mut buf)])
            .map_err(Error::Spi)?;
        Ok(buf[0])
    }

    pub fn write_register(&mut self, addr: u8, value: u8) -> Result<(), Error<SPI::Error>> {
        self.spi.write(&[CMD_WRITE, addr, value]).map_err(Error::Spi)
    }

    pub fn bit_modify(&mut self, addr: u8, mask: u8, data: u8) -> Result<(), Error<SPI::Error>> {
        self.spi.write(&[CMD_BIT_MODIFY, addr, mask, data]).map_err(Error::Spi)
    }

    pub fn mode(&mut self) -> Result<u8, Error<SPI::Error>> {
        Ok(self.read_register(CANSTAT)? >> 5)
    }

    pub fn set_mode(&mut self, mode: Mode) -> Result<(), Error<SPI::Error>> {
        let wanted = mode as u8;
        self.bit_modify(CANCTRL, 0xE0, wanted << 5)?;
        let actual = self.mode()?;
        if actual != wanted {
            return Err(Error::ModeNotEntered { wanted, actual });
        }
        Ok(())
    }

    pub fn set_bit_timing(&mut self, cnf1: u8, cnf2: u8, cnf3: u8) -> Result<(), Error<SPI::Error>> {
        if self.mode()? != Mode::Configuration as u8 {
            return Err(Error::NotInConfigMode);
        }
        self.spi.write(&[CMD_WRITE, CNF3, cnf3, cnf2, cnf1]).map_err(Error::Spi)
    }

    pub fn release(self) -> SPI {
        self.spi
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::hal::{ErrorKind, ErrorType};
    use super::*;
    use std::vec::Vec;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct BusFault;
    impl hal::Error for BusFault {
        fn kind(&self) -> ErrorKind {
            ErrorKind::Other
        }
    }

    /// Simulates an MCP2515-style controller behind chip-select.
    struct FakeMcp {
        regs: [u8; 128],
        transactions: Vec<Vec<u8>>, // bytes written per transaction
        stuck: bool,                 // ignores mode requests
        broken: bool,                // every transaction fails
    }

    impl FakeMcp {
        fn new() -> Self {
            let mut regs = [0u8; 128];
            regs[CANSTAT as usize] = 0b100 << 5; // configuration mode after power-up
            FakeMcp { regs, transactions: Vec::new(), stuck: false, broken: false }
        }
    }

    impl ErrorType for FakeMcp {
        type Error = BusFault;
    }

    impl SpiDevice for FakeMcp {
        fn transaction(&mut self, ops: &mut [Operation<'_>]) -> Result<(), BusFault> {
            if self.broken {
                return Err(BusFault);
            }
            let mut written = Vec::new();
            let mut read_addr = None;
            for op in ops.iter_mut() {
                match op {
                    Operation::Write(bytes) => written.extend_from_slice(bytes),
                    Operation::Read(buf) => {
                        let addr = read_addr.unwrap_or_else(|| match written.as_slice() {
                            [CMD_READ, addr] => *addr as usize,
                            _ => panic!("READ without a READ command: {written:02x?}"),
                        });
                        for (i, b) in buf.iter_mut().enumerate() {
                            *b = self.regs[(addr + i) % 128];
                        }
                        read_addr = Some(addr + buf.len());
                    }
                    _ => panic!("this driver only needs Write and Read operations"),
                }
            }
            match written.as_slice() {
                [CMD_RESET] => {
                    *self = FakeMcp { transactions: core::mem::take(&mut self.transactions), ..FakeMcp::new() };
                }
                [CMD_WRITE, addr, data @ ..] => {
                    for (i, d) in data.iter().enumerate() {
                        self.regs[(*addr as usize + i) % 128] = *d;
                    }
                }
                [CMD_BIT_MODIFY, addr, mask, data] => {
                    let r = &mut self.regs[*addr as usize % 128];
                    *r = (*r & !mask) | (data & mask);
                    if *addr == CANCTRL && !self.stuck {
                        let req = self.regs[CANCTRL as usize] & 0xE0;
                        self.regs[CANSTAT as usize] = (self.regs[CANSTAT as usize] & 0x1F) | req;
                    }
                }
                [CMD_READ, _] => {}
                other => panic!("unexpected command bytes {other:02x?}"),
            }
            self.transactions.push(written);
            Ok(())
        }
    }

    #[test]
    fn register_access() {
        let mut c = CanController::new(FakeMcp::new());
        c.write_register(0x31, 0xAB).unwrap();
        assert_eq!(c.read_register(0x31), Ok(0xAB));
        c.bit_modify(0x31, 0x0F, 0x05).unwrap();
        assert_eq!(c.read_register(0x31), Ok(0xA5));
        let fake = c.release();
        assert_eq!(fake.transactions, [std::vec![0x02, 0x31, 0xAB], std::vec![0x03, 0x31], std::vec![0x05, 0x31, 0x0F, 0x05], std::vec![0x03, 0x31]]);
    }

    #[test]
    fn modes() {
        let mut c = CanController::new(FakeMcp::new());
        assert_eq!(c.mode(), Ok(Mode::Configuration as u8));
        c.set_mode(Mode::Loopback).unwrap();
        assert_eq!(c.mode(), Ok(Mode::Loopback as u8));
        c.set_mode(Mode::Normal).unwrap();
        let fake = c.release();
        assert!(fake.transactions.contains(&std::vec![0x05, CANCTRL, 0xE0, 0x00]));
        assert!(fake.transactions.contains(&std::vec![0x05, CANCTRL, 0xE0, 0x40]));
    }

    #[test]
    fn mode_change_is_verified() {
        let mut fake = FakeMcp::new();
        fake.stuck = true;
        let mut c = CanController::new(fake);
        assert_eq!(c.set_mode(Mode::Normal), Err(Error::ModeNotEntered { wanted: 0, actual: 0b100 }));
    }

    #[test]
    fn bit_timing_needs_config_mode() {
        let mut c = CanController::new(FakeMcp::new());
        c.set_bit_timing(0x00, 0x90, 0x02).unwrap();
        c.set_mode(Mode::Normal).unwrap();
        assert_eq!(c.set_bit_timing(0x01, 0x02, 0x03), Err(Error::NotInConfigMode));
        let fake = c.release();
        assert_eq!(&fake.regs[0x28..0x2B], &[0x02, 0x90, 0x00], "CNF3, CNF2, CNF1");
        assert!(fake.transactions.contains(&std::vec![0x02, 0x28, 0x02, 0x90, 0x00]), "one sequential WRITE");
    }

    #[test]
    fn reset_and_errors() {
        let mut c = CanController::new(FakeMcp::new());
        c.set_mode(Mode::Sleep).unwrap();
        c.reset().unwrap();
        assert_eq!(c.mode(), Ok(Mode::Configuration as u8));
        let mut broken = FakeMcp::new();
        broken.broken = true;
        let mut c = CanController::new(broken);
        assert_eq!(c.reset(), Err(Error::Spi(BusFault)));
        assert_eq!(c.read_register(0), Err(Error::Spi(BusFault)));
        assert_eq!(c.set_mode(Mode::Normal), Err(Error::Spi(BusFault)));
    }
}
