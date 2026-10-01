//! # e2e1: End-to-end protection for CAN signals
//!
//! A simplified version of AUTOSAR E2E Profile 1 for an 8-byte frame:
//!
//! ```text
//! byte 0      CRC-8/SAE-J1850 over: data_id low byte, data_id high byte, bytes 1..=7
//! byte 1      low nibble: alive counter 0..=14 (15 is never sent); high nibble: payload
//! bytes 2..=7 payload
//! ```
//!
//! CRC-8/SAE-J1850: poly 0x1D, init 0xFF, final XOR 0xFF, MSB first. Check
//! value: `crc8_j1850(b"123456789") == 0x4B`.
//!
//! - `Protector::protect`: write the current counter into byte 1's low nibble
//!   (keep the high nibble), compute the CRC into byte 0, then advance the
//!   counter (14 wraps to 0).
//! - `Checker::check(frame)` classifies one receive cycle (`None` = no new
//!   frame arrived this cycle):
//!   - `NoNewData`; `WrongCrc` (also for a different data_id); `InvalidCounter`
//!     (counter 15). The remembered counter does **not** change for these.
//!   - the first valid frame → `Initial`;
//!   - otherwise with `delta = (counter - last) mod 15`: 0 → `Repeated`,
//!     1 → `Ok`, `2..=max_delta` → `OkSomeLost`, larger → `WrongSequence`.
//!     Remember the new counter for all of these except `Repeated`.
#![no_std]

pub fn crc8_j1850(data: &[u8]) -> u8 {
    crc8_update(0xFF, data) ^ 0xFF
}

/// Feed more bytes into a running (non-finalised) CRC.
pub fn crc8_update(mut crc: u8, data: &[u8]) -> u8 {
    for &b in data {
        crc ^= b;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 { (crc << 1) ^ 0x1D } else { crc << 1 };
        }
    }
    crc
}

/// The E2E CRC of a frame for a given data id.
pub fn frame_crc(data_id: u16, frame: &[u8; 8]) -> u8 {
    let id = data_id.to_le_bytes();
    let crc = crc8_update(0xFF, &id);
    crc8_update(crc, &frame[1..]) ^ 0xFF
}

pub struct Protector {
    data_id: u16,
    counter: u8,
}

impl Protector {
    pub const fn new(data_id: u16) -> Self {
        Protector { data_id, counter: 0 }
    }

    pub fn protect(&mut self, frame: &mut [u8; 8]) {
        frame[1] = (frame[1] & 0xF0) | self.counter;
        frame[0] = frame_crc(self.data_id, frame);
        self.counter = (self.counter + 1) % 15;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Initial,
    OkSomeLost,
    Repeated,
    WrongSequence,
    WrongCrc,
    InvalidCounter,
    NoNewData,
}

pub struct Checker {
    data_id: u16,
    max_delta: u8,
    last: Option<u8>,
}

impl Checker {
    pub const fn new(data_id: u16, max_delta: u8) -> Self {
        Checker { data_id, max_delta, last: None }
    }

    pub fn check(&mut self, frame: Option<&[u8; 8]>) -> Status {
        let Some(frame) = frame else { return Status::NoNewData };
        if frame_crc(self.data_id, frame) != frame[0] {
            return Status::WrongCrc;
        }
        let counter = frame[1] & 0x0F;
        if counter > 14 {
            return Status::InvalidCounter;
        }
        let Some(last) = self.last else {
            self.last = Some(counter);
            return Status::Initial;
        };
        let delta = (counter + 15 - last) % 15;
        let status = match delta {
            0 => return Status::Repeated,
            1 => Status::Ok,
            d if d <= self.max_delta => Status::OkSomeLost,
            _ => Status::WrongSequence,
        };
        self.last = Some(counter);
        status
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    const ID: u16 = 0x0123;

    fn frames(n: usize) -> Vec<[u8; 8]> {
        let mut p = Protector::new(ID);
        (0..n)
            .map(|i| {
                let mut f = [0, 0xA0, i as u8, 2, 3, 4, 5, 6];
                p.protect(&mut f);
                f
            })
            .collect()
    }

    #[test]
    fn crc() {
        assert_eq!(crc8_j1850(b"123456789"), 0x4B);
        assert_eq!(crc8_j1850(b""), 0x00);
        let f = [0u8, 0xA0, 0, 2, 3, 4, 5, 6];
        assert_eq!(frame_crc(ID, &f), crc8_j1850(&[0x23, 0x01, 0xA0, 0, 2, 3, 4, 5, 6]));
    }

    #[test]
    fn protector_writes_counter_and_crc() {
        let fs = frames(16);
        assert_eq!(fs[0][1], 0xA0);
        assert_eq!(fs[1][1], 0xA1);
        assert_eq!(fs[14][1], 0xAE);
        assert_eq!(fs[15][1], 0xA0, "counter wraps 14 → 0");
        for f in &fs {
            assert_eq!(f[0], frame_crc(ID, f));
        }
    }

    #[test]
    fn normal_reception() {
        let fs = frames(20);
        let mut c = Checker::new(ID, 2);
        assert_eq!(c.check(Some(&fs[0])), Status::Initial);
        for f in &fs[1..] {
            assert_eq!(c.check(Some(f)), Status::Ok, "including across the wrap");
        }
        assert_eq!(c.check(None), Status::NoNewData);
    }

    #[test]
    fn detects_faults() {
        let fs = frames(12);
        let mut c = Checker::new(ID, 2);
        c.check(Some(&fs[0]));
        assert_eq!(c.check(Some(&fs[0])), Status::Repeated);
        assert_eq!(c.check(Some(&fs[2])), Status::OkSomeLost);
        assert_eq!(c.check(Some(&fs[3])), Status::Ok);
        assert_eq!(c.check(Some(&fs[9])), Status::WrongSequence);
        assert_eq!(c.check(Some(&fs[10])), Status::Ok, "resynchronised");
        let mut corrupted = fs[11];
        corrupted[5] ^= 0x10;
        assert_eq!(c.check(Some(&corrupted)), Status::WrongCrc);
        assert_eq!(c.check(Some(&fs[11])), Status::Ok, "a bad frame doesn't change the state");
    }

    #[test]
    fn masquerade_and_invalid_counter() {
        let fs = frames(3);
        let mut other = Checker::new(0x0124, 2);
        assert_eq!(other.check(Some(&fs[0])), Status::WrongCrc, "same bytes, different data id");
        let mut f = [0u8, 0x0F, 0, 0, 0, 0, 0, 0];
        f[0] = frame_crc(ID, &f);
        let mut c = Checker::new(ID, 2);
        assert_eq!(c.check(Some(&f)), Status::InvalidCounter);
        assert_eq!(c.check(Some(&fs[1])), Status::Initial);
    }
}
