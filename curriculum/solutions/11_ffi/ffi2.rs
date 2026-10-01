//! # ffi2: Opaque handles, caller-provided storage and callbacks
//!
//! A moving-average filter with a C API, in the style of embedded C
//! libraries that never call malloc: **C provides the memory**, Rust
//! initialises a private `Filter` in it and hands back an opaque pointer.
//! Read `ffi2_harness.c` for how it's used.
//!
//! ```c
//! extern const size_t NOSTD_FILTER_SIZE, NOSTD_FILTER_ALIGN;
//! Filter *nostd_filter_init(void *storage, size_t storage_len, uint8_t window);
//! int32_t nostd_filter_push(Filter *f, int32_t sample, int32_t *out_avg);
//! typedef void (*visit_fn)(void *user, size_t index, int32_t value);
//! int32_t nostd_filter_for_each(const Filter *f, visit_fn cb, void *user);
//! ```
//!
//! - `init`: return NULL if `storage` is NULL, smaller than the filter,
//!   misaligned, or `window` is not 1..=8. Otherwise write a fresh `Filter` into
//!   the storage (`ptr.write`) and return the pointer.
//! - `push`: add a sample; write the average of the samples currently in the
//!   window (integer division, truncating) to `*out_avg` if it's non-NULL;
//!   return 0, or -1 if `f` is NULL.
//! - `for_each`: call `cb(user, i, value)` for the samples in the window,
//!   oldest first; return how many; -1 if `f` is NULL or `cb` is NULL.
//!   A nullable C function pointer is `Option<unsafe extern "C" fn(...)>`.
//! - Export `NOSTD_FILTER_SIZE`/`NOSTD_FILTER_ALIGN` as `#[unsafe(no_mangle)]`
//!   statics computed with `size_of`/`align_of`.
#![no_std]

use core::ffi::c_void;
use core::mem::{align_of, size_of};
use core::panic::PanicInfo;

pub struct Filter {
    samples: [i32; 8],
    window: usize,
    len: usize,
    next: usize,
}

impl Filter {
    fn push(&mut self, sample: i32) -> i32 {
        self.samples[self.next] = sample;
        self.next = (self.next + 1) % self.window;
        self.len = (self.len + 1).min(self.window);
        let sum: i64 = self.iter().map(i64::from).sum();
        (sum / self.len as i64) as i32
    }

    /// Samples in the window, oldest first.
    fn iter(&self) -> impl Iterator<Item = i32> + '_ {
        let start = (self.next + self.window - self.len) % self.window;
        (0..self.len).map(move |i| self.samples[(start + i) % self.window])
    }
}

#[unsafe(no_mangle)]
pub static NOSTD_FILTER_SIZE: usize = size_of::<Filter>();

#[unsafe(no_mangle)]
pub static NOSTD_FILTER_ALIGN: usize = align_of::<Filter>();

#[unsafe(no_mangle)]
pub extern "C" fn nostd_filter_init(storage: *mut c_void, storage_len: usize, window: u8) -> *mut Filter {
    let p = storage.cast::<Filter>();
    if p.is_null() || storage_len < size_of::<Filter>() || !p.is_aligned() || !(1..=8).contains(&window) {
        return core::ptr::null_mut();
    }
    // SAFETY: non-null, aligned, and large enough (checked above); the caller
    // owns the storage and promises not to touch it while the filter is in use.
    unsafe { p.write(Filter { samples: [0; 8], window: usize::from(window), len: 0, next: 0 }) };
    p
}

#[unsafe(no_mangle)]
pub extern "C" fn nostd_filter_push(f: *mut Filter, sample: i32, out_avg: *mut i32) -> i32 {
    // SAFETY: a non-null `f` came from nostd_filter_init (C API contract).
    let Some(f) = (unsafe { f.as_mut() }) else { return -1 };
    let avg = f.push(sample);
    if !out_avg.is_null() {
        // SAFETY: non-null; the caller provides a writable int32_t.
        unsafe { out_avg.write(avg) };
    }
    0
}

pub type VisitFn = Option<unsafe extern "C" fn(user: *mut c_void, index: usize, value: i32)>;

#[unsafe(no_mangle)]
pub extern "C" fn nostd_filter_for_each(f: *const Filter, cb: VisitFn, user: *mut c_void) -> i32 {
    // SAFETY: a non-null `f` came from nostd_filter_init.
    let (Some(f), Some(cb)) = (unsafe { f.as_ref() }, cb) else { return -1 };
    let mut n = 0;
    for (i, v) in f.iter().enumerate() {
        // SAFETY: calling the C callback with the user pointer C gave us.
        unsafe { cb(user, i, v) };
        n += 1;
    }
    n
}

unsafe extern "C" {
    safe fn abort() -> !;
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    abort()
}
