//! # can1: CAN identifiers, frames, CAN FD and J1939
//!
//! Build the core CAN types the way `embedded-can` does: **invalid values
//! can't be constructed**, so code that receives an `Id` never re-checks it.
//!
//! 1. `StandardId::new` (≤ 0x7FF) and `ExtendedId::new` (≤ 0x1FFF_FFFF)
//!    return `None` for out-of-range values. `ExtendedId::standard_id` is its
//!    top 11 bits (the part that's sent first).
//! 2. `Ord for Id` = **arbitration order**: `a < b` means `a` wins the bus.
//!    Compare the first 11 bits; if equal, a standard frame beats an extended
//!    one; two extended IDs then compare their remaining 18 bits.
//! 3. `Frame::new(id, data)`: at most 8 data bytes (classical CAN).
//! 4. CAN FD length codes: `fd_dlc_to_len` (0..=8 → same, 9..=15 → 12, 16, 20,
//!    24, 32, 48, 64) and `fd_len_to_dlc` (round **up** to the next valid length).
//! 5. J1939 (`j1939_decode`/`j1939_encode`), see the lesson for the bit layout:
//!    `PGN = EDP<<17 | DP<<16 | PF<<8 | (PS if PF >= 240)`. For PF < 240 (PDU1),
//!    PS is the destination address.
#![no_std]

use core::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StandardId(u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExtendedId(u32);

impl StandardId {
    pub const MAX: u16 = 0x7FF;

    pub const fn new(raw: u16) -> Option<Self> {
        if raw <= Self::MAX { Some(StandardId(raw)) } else { None }
    }

    pub const fn as_raw(self) -> u16 {
        self.0
    }
}

impl ExtendedId {
    pub const MAX: u32 = 0x1FFF_FFFF;

    pub const fn new(raw: u32) -> Option<Self> {
        if raw <= Self::MAX { Some(ExtendedId(raw)) } else { None }
    }

    pub const fn as_raw(self) -> u32 {
        self.0
    }

    /// The 11 most significant bits.
    pub const fn standard_id(self) -> StandardId {
        StandardId((self.0 >> 18) as u16)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Id {
    Standard(StandardId),
    Extended(ExtendedId),
}

impl From<StandardId> for Id {
    fn from(id: StandardId) -> Self {
        Id::Standard(id)
    }
}

impl From<ExtendedId> for Id {
    fn from(id: ExtendedId) -> Self {
        Id::Extended(id)
    }
}

impl Ord for Id {
    fn cmp(&self, other: &Self) -> Ordering {
        // (base 11 bits, IDE bit: standard = dominant 0, remaining 18 bits)
        fn key(id: &Id) -> (u16, u8, u32) {
            match *id {
                Id::Standard(s) => (s.as_raw(), 0, 0),
                Id::Extended(e) => (e.standard_id().as_raw(), 1, e.as_raw() & 0x3FFFF),
            }
        }
        key(self).cmp(&key(other))
    }
}

impl PartialOrd for Id {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    id: Id,
    dlc: u8,
    data: [u8; 8],
}

impl Frame {
    pub fn new(id: impl Into<Id>, data: &[u8]) -> Option<Frame> {
        let mut buf = [0u8; 8];
        buf.get_mut(..data.len())?.copy_from_slice(data);
        Some(Frame { id: id.into(), dlc: data.len() as u8, data: buf })
    }

    pub fn id(&self) -> Id {
        self.id
    }

    pub fn is_extended(&self) -> bool {
        matches!(self.id, Id::Extended(_))
    }

    pub fn dlc(&self) -> usize {
        usize::from(self.dlc)
    }

    pub fn data(&self) -> &[u8] {
        &self.data[..self.dlc()]
    }
}

const FD_LENGTHS: [usize; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 12, 16, 20, 24, 32, 48, 64];

pub fn fd_dlc_to_len(dlc: u8) -> Option<usize> {
    FD_LENGTHS.get(usize::from(dlc)).copied()
}

pub fn fd_len_to_dlc(len: usize) -> Option<u8> {
    FD_LENGTHS.iter().position(|&l| l >= len).map(|i| i as u8)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct J1939Id {
    pub priority: u8,
    pub pgn: u32,
    pub source: u8,
    /// `Some` for PDU1 (PF < 240), `None` for broadcast PDU2.
    pub destination: Option<u8>,
}

pub fn j1939_decode(id: ExtendedId) -> J1939Id {
    let raw = id.as_raw();
    let priority = ((raw >> 26) & 0x7) as u8;
    let edp_dp = (raw >> 24) & 0x3;
    let pf = (raw >> 16) & 0xFF;
    let ps = (raw >> 8) & 0xFF;
    let source = (raw & 0xFF) as u8;
    let (pgn, destination) = if pf < 240 {
        ((edp_dp << 16) | (pf << 8), Some(ps as u8))
    } else {
        ((edp_dp << 16) | (pf << 8) | ps, None)
    };
    J1939Id { priority, pgn, source, destination }
}

/// Build a J1939 identifier. `None` if priority > 7, pgn > 0x3FFFF, or the
/// destination doesn't match the PGN's format (PDU1 needs a destination and
/// a PGN whose low byte is 0; PDU2 must not have one).
pub fn j1939_encode(priority: u8, pgn: u32, source: u8, destination: Option<u8>) -> Option<ExtendedId> {
    if priority > 7 || pgn > 0x3FFFF {
        return None;
    }
    let pf = (pgn >> 8) & 0xFF;
    let ps = match (pf < 240, destination) {
        (true, Some(d)) if pgn & 0xFF == 0 => u32::from(d),
        (false, None) => pgn & 0xFF,
        _ => return None,
    };
    let raw = (u32::from(priority) << 26) | ((pgn >> 16) << 24) | (pf << 16) | (ps << 8) | u32::from(source);
    ExtendedId::new(raw)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    fn s(raw: u16) -> Id {
        StandardId::new(raw).unwrap().into()
    }
    fn e(raw: u32) -> Id {
        ExtendedId::new(raw).unwrap().into()
    }

    #[test]
    fn id_ranges() {
        assert!(StandardId::new(0x7FF).is_some());
        assert!(StandardId::new(0x800).is_none());
        assert!(ExtendedId::new(0x1FFF_FFFF).is_some());
        assert!(ExtendedId::new(0x2000_0000).is_none());
        assert_eq!(ExtendedId::new(0x1234_5678).unwrap().standard_id(), StandardId::new(0x48D).unwrap());
    }

    #[test]
    fn arbitration_order() {
        assert!(s(0x100) < s(0x101), "lower id wins");
        assert!(e(0x100 << 18) > s(0x100), "standard beats extended with the same base");
        assert!(e(0x0FF << 18 | 0x3FFFF) < s(0x100), "base id decides first");
        assert!(e(5) < e(6));
        let mut ids: Vec<Id> = std::vec![s(0x7FF), e(0x1FFF_FFFF), e(0x10), s(0x000), s(0x001), e(0x0004_0000)];
        ids.sort();
        assert_eq!(ids, [s(0x000), e(0x10), s(0x001), e(0x0004_0000), s(0x7FF), e(0x1FFF_FFFF)]);
    }

    #[test]
    fn frames() {
        let f = Frame::new(StandardId::new(0x123).unwrap(), &[1, 2, 3]).unwrap();
        assert_eq!((f.dlc(), f.data(), f.is_extended()), (3, &[1u8, 2, 3][..], false));
        assert_eq!(f.id(), s(0x123));
        let empty = Frame::new(ExtendedId::new(1).unwrap(), &[]).unwrap();
        assert_eq!((empty.dlc(), empty.data().len(), empty.is_extended()), (0, 0, true));
        assert!(Frame::new(StandardId::new(1).unwrap(), &[0; 8]).is_some());
        assert!(Frame::new(StandardId::new(1).unwrap(), &[0; 9]).is_none());
    }

    #[test]
    fn can_fd_lengths() {
        assert_eq!(fd_dlc_to_len(8), Some(8));
        assert_eq!(fd_dlc_to_len(9), Some(12));
        assert_eq!(fd_dlc_to_len(15), Some(64));
        assert_eq!(fd_dlc_to_len(16), None);
        assert_eq!(fd_len_to_dlc(0), Some(0));
        assert_eq!(fd_len_to_dlc(8), Some(8));
        assert_eq!(fd_len_to_dlc(9), Some(9), "9 bytes need a 12-byte frame");
        assert_eq!(fd_len_to_dlc(33), Some(14));
        assert_eq!(fd_len_to_dlc(64), Some(15));
        assert_eq!(fd_len_to_dlc(65), None);
    }

    #[test]
    fn j1939() {
        let eec1 = j1939_decode(ExtendedId::new(0x0CF0_0400).unwrap());
        assert_eq!(eec1, J1939Id { priority: 3, pgn: 0xF004, source: 0x00, destination: None });
        let request = j1939_decode(ExtendedId::new(0x18EA_FF00).unwrap());
        assert_eq!(request, J1939Id { priority: 6, pgn: 0xEA00, source: 0x00, destination: Some(0xFF) });
        let dp = j1939_decode(ExtendedId::new(0x0DF0_0110).unwrap());
        assert_eq!(dp.pgn, 0x1F001);
        assert_eq!(j1939_encode(3, 0xF004, 0x00, None), ExtendedId::new(0x0CF0_0400));
        assert_eq!(j1939_encode(6, 0xEA00, 0x00, Some(0xFF)), ExtendedId::new(0x18EA_FF00));
        assert_eq!(j1939_encode(3, 0x1F001, 0x10, None), ExtendedId::new(0x0DF0_0110));
        assert_eq!(j1939_encode(8, 0xF004, 0, None), None, "priority is 3 bits");
        assert_eq!(j1939_encode(6, 0xEA00, 0, None), None, "PDU1 needs a destination");
        assert_eq!(j1939_encode(6, 0xEA05, 0, Some(1)), None, "PDU1 PGNs have a zero low byte");
        assert_eq!(j1939_encode(3, 0xF004, 0, Some(1)), None, "PDU2 is broadcast");
        assert_eq!(j1939_encode(3, 0x4_0000, 0, None), None);
    }
}
