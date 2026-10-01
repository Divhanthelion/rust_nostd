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
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

#[unsafe(no_mangle)]
pub extern "C" fn nostd_crc32(data: *const u8, len: usize) -> u32 {
    if data.is_null() {
        return 0;
    }
    // SAFETY: non-null; the C caller guarantees `len` readable bytes.
    crc32(unsafe { core::slice::from_raw_parts(data, len) })
}

#[unsafe(no_mangle)]
pub extern "C" fn nostd_stats(samples: *const i16, len: usize, out: *mut Stats) -> i32 {
    if samples.is_null() || out.is_null() {
        return -1;
    }
    if len == 0 {
        return -2;
    }
    // SAFETY: non-null; the caller guarantees `len` readable samples.
    let s = unsafe { core::slice::from_raw_parts(samples, len) };
    let min = s.iter().copied().min().unwrap_or(0);
    let max = s.iter().copied().max().unwrap_or(0);
    let sum: i64 = s.iter().map(|&x| i64::from(x)).sum();
    let mean = (sum / len as i64) as i32;
    // SAFETY: non-null; the caller guarantees `out` points to a writable Stats.
    unsafe { out.write(Stats { min, max, mean, count: len as u32 }) };
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn nostd_hex(data: *const u8, len: usize, out: *mut u8, out_len: usize) -> usize {
    let Some(needed) = len.checked_mul(2).and_then(|n| n.checked_add(1)) else { return 0 };
    if data.is_null() || out.is_null() || out_len < needed {
        return 0;
    }
    // SAFETY: non-null; the caller guarantees the lengths it passed.
    let (src, dst) = unsafe { (core::slice::from_raw_parts(data, len), core::slice::from_raw_parts_mut(out, out_len)) };
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    for (pair, &b) in dst.chunks_exact_mut(2).zip(src) {
        pair[0] = DIGITS[usize::from(b >> 4)];
        pair[1] = DIGITS[usize::from(b & 0xF)];
    }
    dst[needed - 1] = 0;
    needed - 1
}

// A no_std staticlib is a final artifact: it must provide the panic handler.
// Here the "platform" is a C program, so we can call C's abort().
unsafe extern "C" {
    safe fn abort() -> !;
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    abort()
}
