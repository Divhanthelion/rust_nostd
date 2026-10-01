//! # isotp1: ISO-TP segmentation and reassembly
//!
//! ISO 15765-2 carries messages of up to 4095 bytes over 8-byte classical CAN
//! frames (the transport under UDS diagnostics). Implement both directions
//! with fixed buffers.
//!
//! Frames (byte 0's high nibble is the type):
//!
//! ```text
//! Single      0L | data (L = 1..=7 bytes)
//! First       1L LL | 6 data bytes            (12-bit length, 8..=4095)
//! Consecutive 2N | up to 7 data bytes         (N = 1, 2, …, 15, 0, 1, …)
//! FlowControl 3S BS ST                         (we send 30 00 00: "continue, no limits")
//! ```
//!
//! **Sender**, `segment(msg, i)`: frame `i` of the message as 8 bytes, unused
//! bytes padded with `0xCC`; `None` past the last frame, for empty messages,
//! or for messages over 4095 bytes.
//!
//! **Receiver**, `on_frame(frame)`:
//! - Single Frame: needs `1 <= L <= 7` and enough bytes (else `InvalidLength`),
//!   fits the buffer (else `TooLarge`). Completes immediately with
//!   `Complete(L)`. A Single Frame while receiving aborts the old message.
//! - First Frame: must be exactly 8 bytes (else `InvalidFrame`), length ≥ 8
//!   (else `InvalidLength`) and ≤ N (else `TooLarge`). Store 6 bytes and return
//!   `SendFlowControl([0x30, 0, 0])`. A new First Frame restarts reception.
//! - Consecutive Frame: only while receiving (else `UnexpectedConsecutive`);
//!   wrong sequence number → reset to idle, `WrongSequence { expected, got }`;
//!   it must carry `min(7, remaining)` bytes (else `InvalidLength`, reset).
//!   Returns `InProgress` or `Complete(total)`.
//! - Flow Control frames are for senders: `Ignored`. Empty frame or unknown
//!   type: `InvalidFrame`.
//! - `message()` returns the last completed message.
#![no_std]

pub const PAD: u8 = 0xCC;
pub const MAX_LEN: usize = 4095;

/// Frame `index` of `msg`, padded to 8 bytes.
pub fn segment(msg: &[u8], index: usize) -> Option<[u8; 8]> {
    let len = msg.len();
    if len == 0 || len > MAX_LEN {
        return None;
    }
    let mut f = [PAD; 8];
    if len <= 7 {
        if index != 0 {
            return None;
        }
        f[0] = len as u8;
        f[1..1 + len].copy_from_slice(msg);
        return Some(f);
    }
    if index == 0 {
        f[0] = 0x10 | (len >> 8) as u8;
        f[1] = (len & 0xFF) as u8;
        f[2..8].copy_from_slice(&msg[..6]);
        return Some(f);
    }
    let start = 6 + 7 * (index - 1);
    if start >= len {
        return None;
    }
    let chunk = &msg[start..len.min(start + 7)];
    f[0] = 0x20 | (index & 0x0F) as u8;
    f[1..1 + chunk.len()].copy_from_slice(chunk);
    Some(f)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Complete(usize),
    InProgress,
    SendFlowControl([u8; 3]),
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsoTpError {
    InvalidFrame,
    InvalidLength,
    TooLarge,
    UnexpectedConsecutive,
    WrongSequence { expected: u8, got: u8 },
}

pub struct Receiver<const N: usize> {
    buf: [u8; N],
    /// Total length of the message being received (0 = idle).
    expected: usize,
    received: usize,
    next_seq: u8,
    /// Length of the last completed message in `buf`.
    complete: usize,
}

impl<const N: usize> Receiver<N> {
    pub const fn new() -> Self {
        Receiver { buf: [0; N], expected: 0, received: 0, next_seq: 0, complete: 0 }
    }

    pub fn is_receiving(&self) -> bool {
        self.expected != 0
    }

    pub fn message(&self) -> &[u8] {
        &self.buf[..self.complete]
    }

    pub fn on_frame(&mut self, frame: &[u8]) -> Result<Event, IsoTpError> {
        let Some(&pci) = frame.first() else { return Err(IsoTpError::InvalidFrame) };
        match pci >> 4 {
            0 => {
                let len = usize::from(pci & 0x0F);
                if len == 0 || len > 7 || frame.len() < 1 + len {
                    return Err(IsoTpError::InvalidLength);
                }
                if len > N {
                    return Err(IsoTpError::TooLarge);
                }
                self.expected = 0;
                self.buf[..len].copy_from_slice(&frame[1..1 + len]);
                self.complete = len;
                Ok(Event::Complete(len))
            }
            1 => {
                if frame.len() != 8 {
                    return Err(IsoTpError::InvalidFrame);
                }
                let len = (usize::from(pci & 0x0F) << 8) | usize::from(frame[1]);
                if len < 8 {
                    return Err(IsoTpError::InvalidLength);
                }
                if len > N {
                    self.expected = 0;
                    return Err(IsoTpError::TooLarge);
                }
                self.buf[..6].copy_from_slice(&frame[2..8]);
                self.expected = len;
                self.received = 6;
                self.next_seq = 1;
                Ok(Event::SendFlowControl([0x30, 0, 0]))
            }
            2 => {
                if self.expected == 0 {
                    return Err(IsoTpError::UnexpectedConsecutive);
                }
                let got = pci & 0x0F;
                if got != self.next_seq {
                    let expected = self.next_seq;
                    self.expected = 0;
                    return Err(IsoTpError::WrongSequence { expected, got });
                }
                let n = (self.expected - self.received).min(7);
                let Some(data) = frame.get(1..1 + n) else {
                    self.expected = 0;
                    return Err(IsoTpError::InvalidLength);
                };
                self.buf[self.received..self.received + n].copy_from_slice(data);
                self.received += n;
                self.next_seq = (self.next_seq + 1) & 0x0F;
                if self.received == self.expected {
                    self.complete = self.expected;
                    self.expected = 0;
                    Ok(Event::Complete(self.complete))
                } else {
                    Ok(Event::InProgress)
                }
            }
            3 => Ok(Event::Ignored),
            _ => Err(IsoTpError::InvalidFrame),
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    fn frames(msg: &[u8]) -> Vec<[u8; 8]> {
        (0..).map_while(|i| segment(msg, i)).collect()
    }

    #[test]
    fn single_frames() {
        assert_eq!(segment(b"\x22\xF1\x90", 0), Some([0x03, 0x22, 0xF1, 0x90, PAD, PAD, PAD, PAD]));
        assert_eq!(segment(b"1234567", 0), Some([0x07, b'1', b'2', b'3', b'4', b'5', b'6', b'7']));
        assert_eq!(segment(b"abc", 1), None);
        assert_eq!(segment(b"", 0), None);
    }

    #[test]
    fn multi_frame_segmentation() {
        let msg: Vec<u8> = (0..20).collect();
        let f = frames(&msg);
        assert_eq!(f.len(), 3);
        assert_eq!(f[0], [0x10, 20, 0, 1, 2, 3, 4, 5]);
        assert_eq!(f[1], [0x21, 6, 7, 8, 9, 10, 11, 12]);
        assert_eq!(f[2], [0x22, 13, 14, 15, 16, 17, 18, 19]);
        let big = [7u8; 4095];
        let f = frames(&big);
        assert_eq!(f.len(), 1 + 585);
        assert_eq!(f[0][..2], [0x1F, 0xFF]);
        assert_eq!(f[16][0], 0x20, "sequence numbers wrap 15 → 0");
        assert_eq!(f[585], [0x29, 7, PAD, PAD, PAD, PAD, PAD, PAD], "4095 = 6 + 7*584 + 1");
        assert!(segment(&[0u8; 4096], 0).is_none());
    }

    #[test]
    fn round_trips() {
        for len in [1usize, 7, 8, 13, 14, 100, 4095] {
            let msg: Vec<u8> = (0..len).map(|i| (i * 31 % 251) as u8).collect();
            let mut rx = Receiver::<4095>::new();
            let fs = frames(&msg);
            for (i, f) in fs.iter().enumerate() {
                let ev = rx.on_frame(f).unwrap();
                let last = i + 1 == fs.len();
                match ev {
                    Event::Complete(n) => assert!(last && n == len),
                    Event::SendFlowControl(fc) => assert!(i == 0 && fc == [0x30, 0, 0]),
                    Event::InProgress => assert!(!last),
                    Event::Ignored => panic!("unexpected"),
                }
            }
            assert_eq!(rx.message(), &msg[..], "len {len}");
            assert!(!rx.is_receiving());
        }
    }

    #[test]
    fn receiver_errors() {
        let mut rx = Receiver::<64>::new();
        assert_eq!(rx.on_frame(&[]), Err(IsoTpError::InvalidFrame));
        assert_eq!(rx.on_frame(&[0x00, 1]), Err(IsoTpError::InvalidLength));
        assert_eq!(rx.on_frame(&[0x05, 1, 2]), Err(IsoTpError::InvalidLength));
        assert_eq!(rx.on_frame(&[0x21, 1, 2, 3, 4, 5, 6, 7]), Err(IsoTpError::UnexpectedConsecutive));
        assert_eq!(rx.on_frame(&[0x10, 5, 1, 2, 3, 4, 5, 6]), Err(IsoTpError::InvalidLength));
        assert_eq!(rx.on_frame(&[0x10, 65, 1, 2, 3, 4, 5, 6]), Err(IsoTpError::TooLarge));
        assert_eq!(rx.on_frame(&[0x10, 20, 1, 2, 3]), Err(IsoTpError::InvalidFrame));
        assert_eq!(rx.on_frame(&[0x30, 0, 0]), Ok(Event::Ignored));
        assert_eq!(rx.on_frame(&[0x40]), Err(IsoTpError::InvalidFrame));
        let mut small = Receiver::<4>::new();
        assert_eq!(small.on_frame(&[0x05, 1, 2, 3, 4, 5]), Err(IsoTpError::TooLarge));
    }

    #[test]
    fn sequence_errors_abort() {
        let msg = [9u8; 30];
        let fs = frames(&msg);
        let mut rx = Receiver::<64>::new();
        rx.on_frame(&fs[0]).unwrap();
        rx.on_frame(&fs[1]).unwrap();
        assert_eq!(rx.on_frame(&fs[3]), Err(IsoTpError::WrongSequence { expected: 2, got: 3 }));
        assert!(!rx.is_receiving());
        assert_eq!(rx.on_frame(&fs[2]), Err(IsoTpError::UnexpectedConsecutive));
    }

    #[test]
    fn single_frame_interrupts_reception() {
        let mut rx = Receiver::<64>::new();
        rx.on_frame(&segment(&[1u8; 20], 0).unwrap()).unwrap();
        assert!(rx.is_receiving());
        assert_eq!(rx.on_frame(&[0x02, 0x3E, 0x00]), Ok(Event::Complete(2)));
        assert_eq!(rx.message(), &[0x3E, 0x00]);
        assert!(!rx.is_receiving());
        assert_eq!(rx.on_frame(&[0x21, 1, 1, 1, 1, 1, 1, 1]), Err(IsoTpError::UnexpectedConsecutive));
    }

    #[test]
    fn short_consecutive_frame_is_rejected() {
        let mut rx = Receiver::<64>::new();
        rx.on_frame(&segment(&[1u8; 20], 0).unwrap()).unwrap();
        assert_eq!(rx.on_frame(&[0x21, 1, 2]), Err(IsoTpError::InvalidLength));
        assert!(!rx.is_receiving());
    }
}
