//! # ffi3: Calling C from Rust
//!
//! Now the direction is reversed: the C side (`ffi3_harness.c`) offers
//! platform services, and your Rust component uses them.
//!
//! ```c
//! void     c_log(int level, const char *msg);     // prints "[L<level>] <msg>"
//! int      c_read_sensor(int channel, int32_t *out); // 0 ok, -1 error; tenths of °C
//! uint32_t c_millis(void);                         // a timestamp
//! int32_t  rust_poll(const int32_t *channels, size_t n);  // ← you export this
//! ```
//!
//! 1. Declare the three C functions in an `unsafe extern "C"` block. Mark
//!    `c_millis` as `safe fn`: it takes no pointers, so calling it can't cause UB.
//! 2. `rust_poll`: if `channels` is NULL return -1. Otherwise log
//!    `poll start` at level 1 (use a `c"..."` literal), then for each channel:
//!    - read the sensor; on success log at level 1
//!      `ch <c>: <deg>.<tenth> C at <ms> ms` (e.g. `ch 1: -4.0 C at 200 ms`,
//!      calling `c_millis()` once per successful reading);
//!    - on failure log at level 2 `ch <c>: read failed`.
//!    Return the number of successful reads.
//! 3. Formatted messages must become NUL-terminated C strings **without a
//!    heap**: `CBuf` (given) is a fixed buffer implementing `fmt::Write` that
//!    keeps room for the NUL; finish `as_cstr` with `CStr::from_bytes_until_nul`.
#![no_std]

use core::ffi::{c_char, c_int, CStr};
use core::fmt::{self, Write};
use core::panic::PanicInfo;

// TODO: declare c_log, c_read_sensor and c_millis.

/// A fixed-size, always NUL-terminated text buffer.
pub struct CBuf {
    buf: [u8; 64],
    len: usize,
}

impl CBuf {
    pub fn new() -> Self {
        CBuf { buf: [0; 64], len: 0 }
    }

    /// View the contents as a C string.
    pub fn as_cstr(&self) -> &CStr {
        todo!()
    }
}

impl Write for CBuf {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // Keep the final byte free for the NUL terminator; reject interior NULs.
        let end = self.len.checked_add(s.len()).ok_or(fmt::Error)?;
        if end >= self.buf.len() || s.as_bytes().contains(&0) {
            return Err(fmt::Error);
        }
        self.buf[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;
        self.buf[end] = 0;
        Ok(())
    }
}

fn log(level: c_int, msg: &CStr) {
    todo!()
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_poll(channels: *const i32, n: usize) -> i32 {
    todo!()
}

unsafe extern "C" {
    safe fn abort() -> !;
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    abort()
}
