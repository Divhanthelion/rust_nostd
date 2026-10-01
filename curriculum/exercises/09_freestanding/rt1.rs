//! # rt1: The platform contract (memcpy & co)
//!
//! This program copies, fills, moves and compares memory. The compiler turns
//! those operations into calls to `memcpy`, `memset`, `memmove`, `memcmp` and
//! `bcmp`, which normally come from libc. There is no libc here, so the link
//! fails with `undefined symbol: memcpy` (and friends) until **you** provide
//! them.
//!
//! Implement all five with their exact C signatures (`core::ffi::c_void`,
//! `c_int`, `usize` for `size_t`), exported with `#[unsafe(no_mangle)]` and
//! `extern "C"`:
//!
//! ```text
//! void *memcpy(void *dest, const void *src, size_t n);   // regions don't overlap
//! void *memmove(void *dest, const void *src, size_t n);  // regions may overlap!
//! void *memset(void *s, int c, size_t n);                // fills with (unsigned char)c
//! int   memcmp(const void *a, const void *b, size_t n);  // <0, 0, >0 by first differing byte (as unsigned)
//! int   bcmp(const void *a, const void *b, size_t n);    // 0 if equal, non-zero otherwise
//! ```
//!
//! Notice `#![no_builtins]` below. LLVM's "loop idiom" pass recognises byte
//! loops as "a memcpy"/"a memset" and replaces them with calls to those very
//! functions. Modern LLVM refuses to do that inside a function *named*
//! `memcpy`, but not in helpers or in the other mem functions, where it can
//! still produce infinite recursion. `#![no_builtins]` turns the transformation
//! off for the whole crate, which is a guarantee rather than a heuristic.
//!
//! Expected output: `copy ok`, `fill ok`, `move ok`, `compare ok`, one per line.
#![no_std]
#![no_main]
#![no_builtins]

use core::cmp::Ordering;
use core::ffi::{c_int, c_void};
use core::hint::black_box;

// TODO: define memcpy, memmove, memset, memcmp and bcmp here.

fn main() -> i32 {
    // black_box hides values from the optimiser so the operations really happen.
    let n = black_box(256usize);
    let mut a = [0u8; 256];
    for (i, b) in a.iter_mut().enumerate() {
        *b = black_box(i as u8);
    }

    let copy = black_box(a); // memcpy
    if copy[..n] == a[..n] {
        // bcmp
        out(b"copy ok\n");
    }

    let mut z = black_box([0xA5u8; 300]);
    z.fill(black_box(0)); // memset
    if z.iter().all(|&x| x == 0) {
        out(b"fill ok\n");
    }

    let mut m = a;
    m.copy_within(black_box(0..200), black_box(10)); // memmove (overlapping!)
    if m[10] == 0 && m[209] == 199 && m[9] == 9 {
        out(b"move ok\n");
    }

    let mut bigger = a;
    bigger[black_box(100)] = 255;
    if a[..].cmp(&bigger[..]) == Ordering::Less && bigger[..].cmp(&a[..]) == Ordering::Greater {
        // memcmp
        out(b"compare ok\n");
    }
    0
}

// ---------------------------------------------------------------------------
// Support code (from start1): entry point, syscalls, panic handler.

fn out(s: &[u8]) {
    sys_write(1, s);
}

#[cfg(target_arch = "x86_64")]
core::arch::global_asm!(
    ".globl _start", "_start:", "xor rbp, rbp", "mov rdi, rsp", "and rsp, -16", "call {entry}", "ud2",
    entry = sym rust_start,
);

#[cfg(target_arch = "aarch64")]
core::arch::global_asm!(
    ".globl _start", "_start:", "mov x29, xzr", "mov x0, sp", "bl {entry}", "brk #0",
    entry = sym rust_start,
);

extern "C" fn rust_start(_sp: *const usize) -> ! {
    sys_exit(main())
}

#[cfg(target_arch = "x86_64")]
fn sys_write(fd: i32, buf: &[u8]) -> isize {
    let ret: isize;
    // SAFETY: write(2) with a valid buffer.
    unsafe {
        core::arch::asm!("syscall", inlateout("rax") 1isize => ret, in("rdi") fd as usize,
            in("rsi") buf.as_ptr(), in("rdx") buf.len(), lateout("rcx") _, lateout("r11") _, options(nostack));
    }
    ret
}

#[cfg(target_arch = "x86_64")]
fn sys_exit(code: i32) -> ! {
    // SAFETY: exit_group(2) never returns.
    unsafe { core::arch::asm!("syscall", in("rax") 231usize, in("rdi") code as usize, options(noreturn, nostack)) }
}

#[cfg(target_arch = "aarch64")]
fn sys_write(fd: i32, buf: &[u8]) -> isize {
    let ret: isize;
    // SAFETY: write(2) with a valid buffer.
    unsafe {
        core::arch::asm!("svc #0", inlateout("x0") fd as isize => ret, in("x1") buf.as_ptr(),
            in("x2") buf.len(), in("x8") 64usize, options(nostack));
    }
    ret
}

#[cfg(target_arch = "aarch64")]
fn sys_exit(code: i32) -> ! {
    // SAFETY: exit_group(2) never returns.
    unsafe { core::arch::asm!("svc #0", in("x0") code as usize, in("x8") 94usize, options(noreturn, nostack)) }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    sys_exit(101)
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_eh_personality() {}
