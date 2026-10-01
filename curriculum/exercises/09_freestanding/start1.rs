//! # start1: Life before main
//!
//! A complete program with no std, no libc and no C runtime. The kernel
//! jumps to `_start` (given below, in assembly), which calls `rust_start`.
//! Everything else is yours:
//!
//! 1. `sys_write(fd, buf)`: the `write` system call via `core::arch::asm!`.
//! 2. `sys_exit(code)`: the `exit_group` system call. It never returns.
//! 3. `main`: write exactly `Hello from _start!\n` to stdout (fd 1) and
//!    return 42, which becomes the process exit code.
//!
//! You only need the variant for your machine's architecture (`uname -m`):
//! the other one is compiled out by `#[cfg]`. Syscall numbers and registers
//! are in the lesson's table (`nostd learn 9`).
//!
//! The checker links this with `-nostartfiles -static` and runs it.
#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

// The entry point. The kernel jumps here with the stack pointer pointing at
// argc. We align the stack and call into Rust. (Provided: read it, keep it.)
#[cfg(target_arch = "x86_64")]
core::arch::global_asm!(
    ".globl _start",
    "_start:",
    "xor rbp, rbp",
    "mov rdi, rsp",
    "and rsp, -16",
    "call {entry}",
    "ud2",
    entry = sym rust_start,
);

#[cfg(target_arch = "aarch64")]
core::arch::global_asm!(
    ".globl _start",
    "_start:",
    "mov x29, xzr",
    "mov x0, sp",
    "bl {entry}",
    "brk #0",
    entry = sym rust_start,
);

extern "C" fn rust_start(_sp: *const usize) -> ! {
    let code = main();
    sys_exit(code)
}

#[cfg(target_arch = "x86_64")]
fn sys_write(fd: i32, buf: &[u8]) -> isize {
    todo!()
}

#[cfg(target_arch = "x86_64")]
fn sys_exit(code: i32) -> ! {
    todo!()
}

#[cfg(target_arch = "aarch64")]
fn sys_write(fd: i32, buf: &[u8]) -> isize {
    todo!()
}

#[cfg(target_arch = "aarch64")]
fn sys_exit(code: i32) -> ! {
    todo!()
}

fn main() -> i32 {
    todo!()
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys_exit(101)
}

// The host's precompiled `core` has unwind tables that name this symbol, and
// `todo!()`/panics pull them in. With panic=abort it is never called, so an
// empty definition satisfies the linker. (Bare-metal targets don't need it.)
#[unsafe(no_mangle)]
pub extern "C" fn rust_eh_personality() {}
