//! # cells1: Interior mutability without threads
//!
//! `Cell`, `RefCell`, `OnceCell` and `LazyCell` let you mutate through `&self`
//! in single-threaded code: a driver object shared by several components, or
//! state touched only from one execution context.
//!
//! - `Stats`: counters behind `&self` using `Cell` (`get`/`set`/`replace`/`take`).
//! - `Sensor`: calibration that can be set **once** (`OnceCell`), a history of
//!   the last 4 readings (`RefCell<[i16; 4]>`, newest last), and a lookup table
//!   computed on first use (`LazyCell`).
//! - `try_clear_history` must **not** panic when the history is already
//!   borrowed: use `try_borrow_mut` and report the conflict.
#![no_std]

use core::cell::{BorrowMutError, Cell, LazyCell, OnceCell, RefCell};

pub struct Stats {
    frames: Cell<u32>,
    errors: Cell<u32>,
    last_id: Cell<Option<u16>>,
}

impl Stats {
    pub const fn new() -> Self {
        Stats { frames: Cell::new(0), errors: Cell::new(0), last_id: Cell::new(None) }
    }

    /// Count a frame (and an error if `!ok`), remembering its id.
    pub fn record(&self, id: u16, ok: bool) {
        self.frames.set(self.frames.get() + 1);
        if !ok {
            self.errors.set(self.errors.get() + 1);
        }
        self.last_id.set(Some(id));
    }

    pub fn frames(&self) -> u32 {
        self.frames.get()
    }

    pub fn errors(&self) -> u32 {
        self.errors.get()
    }

    pub fn last_id(&self) -> Option<u16> {
        self.last_id.get()
    }

    /// Return (frames, errors) and reset both counters to zero.
    pub fn take(&self) -> (u32, u32) {
        (self.frames.take(), self.errors.take())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Calibration {
    pub offset: i16,
    /// Gain in 1/256 units (256 = ×1).
    pub gain: u16,
}

pub struct Sensor {
    calibration: OnceCell<Calibration>,
    history: RefCell<[i16; 4]>,
    /// `lut[i] = i * i * 10`, computed on first access.
    lut: LazyCell<[u32; 16]>,
}

fn build_lut() -> [u32; 16] {
    core::array::from_fn(|i| (i * i * 10) as u32)
}

impl Sensor {
    pub fn new() -> Self {
        Sensor { calibration: OnceCell::new(), history: RefCell::new([0; 4]), lut: LazyCell::new(build_lut) }
    }

    /// Set the calibration. Only the first call wins; later calls give the
    /// value back as `Err`.
    pub fn calibrate(&self, c: Calibration) -> Result<(), Calibration> {
        self.calibration.set(c)
    }

    /// `None` until calibrated. Otherwise `(raw + offset) * gain / 256`
    /// (computed in i32, saturated into i16), appended to the history.
    pub fn read(&self, raw: i16) -> Option<i16> {
        let c = self.calibration.get()?;
        let v = (i32::from(raw) + i32::from(c.offset)) * i32::from(c.gain) / 256;
        let v = v.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        let mut h = self.history.borrow_mut();
        h.rotate_left(1);
        h[3] = v;
        Some(v)
    }

    /// The last four values, oldest first (zeros before the first readings).
    pub fn history(&self) -> [i16; 4] {
        *self.history.borrow()
    }

    /// Run `f` with a borrow of the history.
    pub fn with_history<R>(&self, f: impl FnOnce(&[i16; 4]) -> R) -> R {
        f(&self.history.borrow())
    }

    /// Zero the history, or report that it is currently borrowed.
    pub fn try_clear_history(&self) -> Result<(), BorrowMutError> {
        *self.history.try_borrow_mut()? = [0; 4];
        Ok(())
    }

    pub fn lut_entry(&self, i: usize) -> Option<u32> {
        self.lut.get(i).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_through_shared_references() {
        let s = Stats::new();
        let a = &s;
        let b = &s;
        a.record(0x100, true);
        b.record(0x200, false);
        a.record(0x300, false);
        assert_eq!((s.frames(), s.errors(), s.last_id()), (3, 2, Some(0x300)));
        assert_eq!(s.take(), (3, 2));
        assert_eq!((s.frames(), s.errors()), (0, 0));
        assert_eq!(s.last_id(), Some(0x300));
    }

    #[test]
    fn calibration_is_set_once() {
        let s = Sensor::new();
        assert_eq!(s.read(10), None);
        let c = Calibration { offset: -10, gain: 512 };
        assert_eq!(s.calibrate(c), Ok(()));
        let other = Calibration { offset: 0, gain: 256 };
        assert_eq!(s.calibrate(other), Err(other));
        assert_eq!(s.read(110), Some(200));
    }

    #[test]
    fn readings_saturate_and_fill_history() {
        let s = Sensor::new();
        s.calibrate(Calibration { offset: 0, gain: 256 * 4 }).unwrap();
        for raw in [1, 2, 3] {
            s.read(raw);
        }
        assert_eq!(s.history(), [0, 4, 8, 12]);
        assert_eq!(s.read(i16::MAX), Some(i16::MAX));
        assert_eq!(s.read(i16::MIN), Some(i16::MIN));
        assert_eq!(s.history(), [8, 12, i16::MAX, i16::MIN]);
    }

    #[test]
    fn clearing_while_borrowed_reports_instead_of_panicking() {
        let s = Sensor::new();
        s.calibrate(Calibration { offset: 1, gain: 256 }).unwrap();
        s.read(1);
        let conflict = s.with_history(|_h| s.try_clear_history());
        assert!(conflict.is_err());
        assert_eq!(s.history(), [0, 0, 0, 2]);
        assert!(s.try_clear_history().is_ok());
        assert_eq!(s.history(), [0; 4]);
    }

    #[test]
    fn lazy_table() {
        let s = Sensor::new();
        assert_eq!(s.lut_entry(0), Some(0));
        assert_eq!(s.lut_entry(3), Some(90));
        assert_eq!(s.lut_entry(15), Some(2250));
        assert_eq!(s.lut_entry(16), None);
    }
}
