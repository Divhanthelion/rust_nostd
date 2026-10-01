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
        todo!()
    }

    /// Samples in the window, oldest first.
    fn iter(&self) -> impl Iterator<Item = i32> + '_ {
        todo!()
    }
}

// TODO: the two exported statics and three exported functions.

unsafe extern "C" {
    safe fn abort() -> !;
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    abort()
}
