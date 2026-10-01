//! # ffi1: Exporting a no_std library to C
//!
//! This crate is built as a **staticlib** and linked into a C program
//! (`ffi1_harness.c`, next to this file: read it!). Implement the three
//! exported functions with the exact C signatures the harness declares:
//!
//! ```c
//! uint32_t nostd_crc32(const uint8_t *data, size_t len);
//! int32_t  nostd_stats(const int16_t *samples, size_t len, struct Stats *out);
//! size_t   nostd_hex(const uint8_t *data, size_t len, char *out, size_t out_len);
//! ```
//!
//! - `nostd_crc32`: CRC-32 as used by zlib/Ethernet (reflected poly
//!   `0xEDB88320`, init and final XOR `0xFFFFFFFF`). `NULL` data → 0.
//! - `nostd_stats`: min, max, mean (sum / count, truncated toward zero like C)
//!   and count into `*out`. Return 0, or -1 if a pointer is NULL, -2 if
//!   `len == 0`.
//! - `nostd_hex`: lowercase hex of `data` plus a terminating NUL into `out`.
//!   Return the number of hex digits written, or 0 (writing nothing) if any
//!   pointer is NULL or `out_len < 2 * len + 1`.
//!
//! Each function must be `#[unsafe(no_mangle)] pub extern "C" fn`, validate
//! its pointers, then build safe slices with `core::slice::from_raw_parts`.
//! `Stats` must be `#[repr(C)]` with the same field order and types as in C.
#![no_std]

use core::panic::PanicInfo;

#[repr(C)]
pub struct Stats {
    pub min: i16,
    pub max: i16,
    pub mean: i32,
    pub count: u32,
}

/// Pure-Rust CRC-32 over a safe slice.
fn crc32(data: &[u8]) -> u32 {
    todo!()
}

// TODO: the three #[unsafe(no_mangle)] pub extern "C" functions.

// A no_std staticlib is a final artifact: it must provide the panic handler.
// Here the "platform" is a C program, so we can call C's abort().
unsafe extern "C" {
    safe fn abort() -> !;
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    abort()
}
