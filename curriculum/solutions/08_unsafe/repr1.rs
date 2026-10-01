//! # repr1: Wire formats, enums and layout attributes
//!
//! Parse and encode an 8-byte message header **without** casting bytes to a
//! struct. (Transmuting raw bytes into a `#[repr(C)]` struct is a classic
//! source of UB: unchecked enum values, alignment, endianness.) Decode field
//! by field instead:
//!
//! ```text
//! byte 0     : version (high nibble, must be 1) | flags (low nibble)
//! byte 1     : message type (MsgType discriminant)
//! bytes 2..4 : payload length, u16 BIG-endian
//! bytes 4..8 : sequence number, u32 LITTLE-endian
//! then `length` payload bytes
//! ```
//!
//! Also:
//! - `TryFrom<u8> for MsgType`, returning the bad byte as the error.
//! - `packed_length`: the `#[repr(C, packed)]` struct's fields may be
//!   unaligned, so taking a reference to one is an error. Copy the field out
//!   instead (`{ h.length }`) or use `read_unaligned`.
#![no_std]

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgType {
    Heartbeat = 0x01,
    Data = 0x02,
    Ack = 0x03,
    Error = 0x7F,
}

impl TryFrom<u8> for MsgType {
    type Error = u8;
    fn try_from(b: u8) -> Result<Self, u8> {
        match b {
            0x01 => Ok(MsgType::Heartbeat),
            0x02 => Ok(MsgType::Data),
            0x03 => Ok(MsgType::Ack),
            0x7F => Ok(MsgType::Error),
            other => Err(other),
        }
    }
}

pub const HEADER_LEN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub version: u8,
    pub flags: u8,
    pub kind: MsgType,
    pub length: u16,
    pub seq: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    TooShort,
    BadVersion(u8),
    UnknownType(u8),
    Truncated { declared: u16, available: usize },
}

/// Parse a header and return it with its payload (exactly `length` bytes;
/// trailing bytes are ignored).
pub fn parse(bytes: &[u8]) -> Result<(Header, &[u8]), ParseError> {
    let (h, rest) = bytes.split_first_chunk::<HEADER_LEN>().ok_or(ParseError::TooShort)?;
    let version = h[0] >> 4;
    if version != 1 {
        return Err(ParseError::BadVersion(version));
    }
    let kind = MsgType::try_from(h[1]).map_err(ParseError::UnknownType)?;
    let length = u16::from_be_bytes([h[2], h[3]]);
    let seq = u32::from_le_bytes([h[4], h[5], h[6], h[7]]);
    let payload = rest
        .get(..usize::from(length))
        .ok_or(ParseError::Truncated { declared: length, available: rest.len() })?;
    Ok((Header { version, flags: h[0] & 0x0F, kind, length, seq }, payload))
}

impl Header {
    /// Inverse of `parse` for the header bytes. `version`/`flags` are masked
    /// to 4 bits each.
    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        out[0] = (self.version & 0x0F) << 4 | (self.flags & 0x0F);
        out[1] = self.kind as u8;
        out[2..4].copy_from_slice(&self.length.to_be_bytes());
        out[4..8].copy_from_slice(&self.seq.to_le_bytes());
        out
    }
}

/// A packed on-the-wire struct: no padding, alignment 1.
#[repr(C, packed)]
pub struct PackedHeader {
    pub kind: u8,
    pub length: u16,
    pub seq: u32,
}

/// Return `h.length` (in native byte order).
pub fn packed_length(h: &PackedHeader) -> u16 {
    // Braces copy the (possibly unaligned) field into an aligned temporary.
    { h.length }
}

/// A newtype with exactly the layout of its field, safe to pass across FFI
/// wherever an `i16` is expected.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeciCelsius(pub i16);

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, size_of};

    const SAMPLE: [u8; 11] = [0x13, 0x02, 0x00, 0x03, 0x2A, 0x00, 0x00, 0x00, b'a', b'b', b'c'];

    #[test]
    fn enum_conversion() {
        assert_eq!(MsgType::try_from(0x02), Ok(MsgType::Data));
        assert_eq!(MsgType::try_from(0x7F), Ok(MsgType::Error));
        assert_eq!(MsgType::try_from(0x04), Err(0x04));
        assert_eq!(MsgType::Ack as u8, 3);
    }

    #[test]
    fn parses_headers() {
        let (h, payload) = parse(&SAMPLE).unwrap();
        assert_eq!(h, Header { version: 1, flags: 3, kind: MsgType::Data, length: 3, seq: 42 });
        assert_eq!(payload, b"abc");
        let mut longer = [0u8; 12];
        longer[..11].copy_from_slice(&SAMPLE);
        assert_eq!(parse(&longer).unwrap().1, b"abc");
    }

    #[test]
    fn parse_errors() {
        assert_eq!(parse(&SAMPLE[..7]), Err(ParseError::TooShort));
        let mut bad = SAMPLE;
        bad[0] = 0x23;
        assert_eq!(parse(&bad), Err(ParseError::BadVersion(2)));
        let mut bad = SAMPLE;
        bad[1] = 0x09;
        assert_eq!(parse(&bad), Err(ParseError::UnknownType(9)));
        assert_eq!(parse(&SAMPLE[..10]), Err(ParseError::Truncated { declared: 3, available: 2 }));
    }

    #[test]
    fn encodes_round_trip() {
        let h = Header { version: 1, flags: 0xA, kind: MsgType::Heartbeat, length: 0x1234, seq: 0xDEAD_BEEF };
        let bytes = h.encode();
        assert_eq!(bytes, [0x1A, 0x01, 0x12, 0x34, 0xEF, 0xBE, 0xAD, 0xDE]);
        let mut buf = [0u8; HEADER_LEN + 0x1234];
        buf[..HEADER_LEN].copy_from_slice(&bytes);
        assert_eq!(parse(&buf).unwrap().0, h);
    }

    #[test]
    fn layouts() {
        assert_eq!(size_of::<PackedHeader>(), 7);
        assert_eq!(align_of::<PackedHeader>(), 1);
        let p = PackedHeader { kind: 1, length: 500, seq: 9 };
        assert_eq!(packed_length(&p), 500);
        assert_eq!(size_of::<DeciCelsius>(), size_of::<i16>());
        assert_eq!(size_of::<Option<MsgType>>(), 1, "niche: unused discriminants encode None");
    }
}
