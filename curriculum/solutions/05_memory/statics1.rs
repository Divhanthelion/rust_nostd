//! # statics1: Tables in flash and safe global state
//!
//! Three kinds of global data:
//!
//! 1. **Compile-time tables.** Implement `crc8_table` as a `const fn`, so
//!    `CRC8_TABLE` is computed by the compiler and stored in read-only memory.
//!    (In `const fn`, use `while` loops: `for` isn't allowed there yet.)
//!    Then implement table-driven CRC-8/SAE-J1850: start with `0xFF`; for each
//!    byte, `crc = TABLE[(crc ^ byte) as usize]`; finish with `crc ^ 0xFF`.
//! 2. **Counters shared across threads/interrupts.** `record_boot` uses a
//!    `static mut`, which is a data race when called from several threads (the
//!    test does exactly that). Replace it with an atomic.
//! 3. **Raw buffers.** `scratch_ptr` and `fill_scratch` don't compile in
//!    edition 2024: they create references to a `static mut`. Use `&raw mut`
//!    to get a raw pointer without creating a reference.
#![no_std]

use core::sync::atomic::{AtomicU32, Ordering};

/// The CRC-8 lookup table for polynomial `poly` (MSB-first, no reflection):
/// entry `i` is the result of shifting byte `i` through 8 rounds of
/// `crc = if crc & 0x80 != 0 { (crc << 1) ^ poly } else { crc << 1 }`.
pub const fn crc8_table(poly: u8) -> [u8; 256] {
    let mut table = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u8;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 0x80 != 0 { (crc << 1) ^ poly } else { crc << 1 };
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

/// Computed at compile time. Lives in .rodata (flash on an MCU).
pub static CRC8_TABLE: [u8; 256] = crc8_table(0x1D);

/// CRC-8/SAE-J1850 (init 0xFF, xorout 0xFF) using `CRC8_TABLE`.
pub fn crc8_sae_j1850(data: &[u8]) -> u8 {
    let crc = data.iter().fold(0xFFu8, |crc, &b| CRC8_TABLE[usize::from(crc ^ b)]);
    crc ^ 0xFF
}

static BOOT_COUNT: AtomicU32 = AtomicU32::new(0);

/// Increment the boot counter and return the new value. Must be correct when
/// called concurrently.
pub fn record_boot() -> u32 {
    BOOT_COUNT.fetch_add(1, Ordering::Relaxed) + 1
}

pub const SCRATCH_LEN: usize = 64;
static mut SCRATCH: [u8; SCRATCH_LEN] = [0; SCRATCH_LEN];

/// A pointer to the scratch buffer, e.g. to hand to a DMA engine.
pub fn scratch_ptr() -> *mut u8 {
    (&raw mut SCRATCH).cast::<u8>()
}

/// Fill the scratch buffer with `byte`, returning how many bytes were written.
///
/// # Safety
/// No other code may access the scratch buffer while this runs.
pub unsafe fn fill_scratch(byte: u8) -> usize {
    let p = scratch_ptr();
    for i in 0..SCRATCH_LEN {
        // SAFETY: i < SCRATCH_LEN, and the caller guarantees exclusive access.
        unsafe { p.add(i).write(byte) };
    }
    SCRATCH_LEN
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn table_matches_known_entries() {
        assert_eq!(CRC8_TABLE[0], 0x00);
        assert_eq!(CRC8_TABLE[1], 0x1D);
        assert_eq!(CRC8_TABLE[2], 0x3A);
        assert_eq!(CRC8_TABLE[0x80], 0x26);
        assert_eq!(CRC8_TABLE[0xFF], 0xC4);
        assert_eq!(crc8_table(0x07)[1], 0x07);
    }

    #[test]
    fn table_is_computed_at_compile_time() {
        // Using it in a const context proves const-evaluability.
        const T: [u8; 256] = crc8_table(0x1D);
        assert_eq!(T, CRC8_TABLE);
    }

    #[test]
    fn crc_check_value() {
        assert_eq!(crc8_sae_j1850(b"123456789"), 0x4B);
        assert_eq!(crc8_sae_j1850(b""), 0x00);
    }

    #[test]
    fn boot_counter_is_race_free() {
        let start = record_boot();
        let handles: std::vec::Vec<_> = (0..8)
            .map(|_| std::thread::spawn(|| for _ in 0..10_000 { record_boot(); }))
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(record_boot(), start + 80_001);
    }

    #[test]
    fn scratch_buffer() {
        let p = scratch_ptr();
        assert!(!p.is_null());
        assert_eq!(p, scratch_ptr());
        assert_eq!(unsafe { fill_scratch(0xAB) }, SCRATCH_LEN);
        let first = unsafe { p.read() };
        let last = unsafe { p.add(SCRATCH_LEN - 1).read() };
        assert_eq!((first, last), (0xAB, 0xAB));
    }
}
