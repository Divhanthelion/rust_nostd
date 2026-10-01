//! # can2: DBC signals, Intel and Motorola
//!
//! Decode and encode CAN signals as a DBC file describes them. Bits are
//! numbered `byte * 8 + bit`, bit 0 = least significant bit of byte 0.
//!
//! - **Intel** (little-endian, `@1` in DBC): `start_bit` is the signal's LSB;
//!   successive bits are `start, start+1, start+2, ...`.
//! - **Motorola** (big-endian, `@0`): `start_bit` is the signal's **MSB**.
//!   From position `p`, the next (less significant) bit is `p - 1`, except
//!   when `p % 8 == 0`: then it's `p + 15` (bit 7 of the next byte).
//!
//! Implement:
//! - `positions`: the bit positions of the signal, **MSB first**, written into
//!   `out` (returns how many). `None` if `length` is 0 or > 64 or any bit falls
//!   outside `data_len * 8` bits. Both other functions build on this one.
//! - `decode(data)`: the raw value (sign-extended if `signed`), or `None` if
//!   the signal doesn't fit in `data`.
//! - `encode(data, raw)`: write `raw` into the signal's bits, leaving all
//!   other bits untouched. `None` (and no change) if it doesn't fit or `raw` is
//!   out of range for `length` bits (unsigned: `0..2^len`; signed:
//!   `-2^(len-1)..2^(len-1)`).
//! - `to_physical_milli(raw)`: `raw * factor_num / factor_den` in milli-units
//!   plus `offset_milli`, computed in `i64` as `raw * factor_num * 1000 /
//!   factor_den + offset_milli` (rounds toward zero).
#![no_std]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    Intel,
    Motorola,
}

#[derive(Debug, Clone, Copy)]
pub struct Signal {
    pub start_bit: u16,
    pub length: u8,
    pub order: ByteOrder,
    pub signed: bool,
    pub factor_num: i64,
    pub factor_den: i64,
    pub offset_milli: i64,
}

impl Signal {
    pub const fn raw(start_bit: u16, length: u8, order: ByteOrder, signed: bool) -> Signal {
        Signal { start_bit, length, order, signed, factor_num: 1, factor_den: 1, offset_milli: 0 }
    }

    pub fn positions(&self, data_len: usize, out: &mut [u16; 64]) -> Option<usize> {
        todo!()
    }

    pub fn decode(&self, data: &[u8]) -> Option<i64> {
        todo!()
    }

    pub fn encode(&self, data: &mut [u8], raw: i64) -> Option<()> {
        todo!()
    }

    pub fn to_physical_milli(&self, raw: i64) -> i64 {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ByteOrder::*;

    #[test]
    fn intel_basics() {
        let d = [0x34, 0x12, 0, 0, 0, 0, 0, 0];
        assert_eq!(Signal::raw(0, 16, Intel, false).decode(&d), Some(0x1234));
        let d = [0xAB, 0xCD];
        assert_eq!(Signal::raw(4, 12, Intel, false).decode(&d), Some(0xCDA));
        assert_eq!(Signal::raw(0, 1, Intel, false).decode(&d), Some(1));
        assert_eq!(Signal::raw(15, 1, Intel, false).decode(&d), Some(1));
    }

    #[test]
    fn motorola_basics() {
        let d = [0x12, 0x34];
        assert_eq!(Signal::raw(7, 16, Motorola, false).decode(&d), Some(0x1234));
        let d = [0xAB, 0xCD];
        assert_eq!(Signal::raw(3, 8, Motorola, false).decode(&d), Some(0xBC));
        let d = [0, 0, 0x5A, 0xF0];
        assert_eq!(Signal::raw(23, 12, Motorola, false).decode(&d), Some(0x5AF));
    }

    #[test]
    fn bit_positions() {
        let mut pos = [0u16; 64];
        let n = Signal::raw(3, 8, Motorola, false).positions(2, &mut pos).unwrap();
        assert_eq!(&pos[..n], &[3, 2, 1, 0, 15, 14, 13, 12]);
        let n = Signal::raw(4, 6, Intel, false).positions(2, &mut pos).unwrap();
        assert_eq!(&pos[..n], &[9, 8, 7, 6, 5, 4]);
    }

    #[test]
    fn signed_values() {
        let d = [0x00, 0xFE];
        assert_eq!(Signal::raw(8, 8, Intel, true).decode(&d), Some(-2));
        assert_eq!(Signal::raw(8, 8, Intel, false).decode(&d), Some(254));
        assert_eq!(Signal::raw(8, 4, Intel, true).decode(&d), Some(-2));
        assert_eq!(Signal::raw(0, 64, Intel, true).decode(&[0xFF; 8]), Some(-1));
    }

    #[test]
    fn out_of_frame_signals() {
        let d = [0u8; 8];
        assert_eq!(Signal::raw(60, 8, Intel, false).decode(&d), None);
        assert_eq!(Signal::raw(7, 65, Motorola, false).decode(&d), None);
        assert_eq!(Signal::raw(56, 2, Motorola, false).decode(&d), None, "56 → 71 walks off the end");
        assert_eq!(Signal::raw(0, 0, Intel, false).decode(&d), None);
        assert_eq!(Signal::raw(0, 16, Intel, false).decode(&[1]), None);
    }

    #[test]
    fn encoding_round_trips_and_preserves_other_bits() {
        let mut d = [0xFFu8; 8];
        let sig = Signal::raw(3, 8, Motorola, false);
        assert_eq!(sig.encode(&mut d, 0xBC), Some(()));
        assert_eq!(&d[..3], &[0xFB, 0xCF, 0xFF]);
        assert_eq!(sig.decode(&d), Some(0xBC));
        let mut d = [0u8; 8];
        let sig = Signal::raw(10, 13, Intel, true);
        for v in [-4096, -1, 0, 1, 4095] {
            sig.encode(&mut d, v).unwrap();
            assert_eq!(sig.decode(&d), Some(v));
        }
        assert_eq!(sig.encode(&mut d, 4096), None);
        assert_eq!(sig.encode(&mut d, -4097), None);
        let u = Signal::raw(0, 4, Intel, false);
        let before = d;
        assert_eq!(u.encode(&mut d, 16), None);
        assert_eq!(u.encode(&mut d, -1), None);
        assert_eq!(d, before, "failed encodes change nothing");
    }

    #[test]
    fn physical_values() {
        // SG_ EngineSpeed : 24|16@1+ (0.125,0) "rpm"
        let rpm = Signal { factor_num: 1, factor_den: 8, ..Signal::raw(24, 16, Intel, false) };
        let mut d = [0u8; 8];
        rpm.encode(&mut d, 8000).unwrap();
        assert_eq!(&d[3..5], &[0x40, 0x1F]);
        assert_eq!(rpm.to_physical_milli(rpm.decode(&d).unwrap()), 1_000_000);
        // SG_ CoolantTemp : 7|8@0+ (1,-40) "degC"
        let temp = Signal { offset_milli: -40_000, ..Signal::raw(7, 8, Motorola, false) };
        assert_eq!(temp.to_physical_milli(temp.decode(&[0x5A]).unwrap()), 50_000);
        // SG_ Torque : 0|12@1- (0.1,0) "Nm"
        let torque = Signal { factor_num: 1, factor_den: 10, ..Signal::raw(0, 12, Intel, true) };
        assert_eq!(torque.to_physical_milli(-15), -1_500);
    }
}
