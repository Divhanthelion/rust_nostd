//! # print1: println! from scratch
//!
//! Build the printing stack that std normally gives you, on top of the raw
//! `write` system call:
//!
//! 1. `write_all(fd, buf)`: keep calling `sys_write` until every byte is
//!    written. `write` may write **fewer** bytes than asked (a short write); if
//!    it returns `-4` (`-EINTR`) just retry; any other result `<= 0` is an
//!    error: return it.
//! 2. `Stdout` (fd 1) and `Stderr` (fd 2) implementing `core::fmt::Write`.
//! 3. The macros `println!` and `eprintln!`. `print!` is done for you as an
//!    example: note how `format_args!` passes the arguments through without
//!    allocating.
//!
//! `main` is given: the checker compares its stdout and stderr exactly.
#![no_std]
#![no_main]
#![no_builtins]

use core::ffi::{c_int, c_void};
use core::fmt::{self, Write};

fn write_all(fd: i32, buf: &[u8]) -> Result<(), isize> {
    let mut buf = buf;
    while !buf.is_empty() {
        let n = sys_write(fd, buf);
        if n == -4 {
            continue;
        }
        if n <= 0 {
            return Err(n);
        }
        buf = buf.get(n as usize..).unwrap_or(&[]);
    }
    Ok(())
}

pub struct Stdout;
pub struct Stderr;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_all(1, s.as_bytes()).map_err(|_| fmt::Error)
    }
}

impl Write for Stderr {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_all(2, s.as_bytes()).map_err(|_| fmt::Error)
    }
}

macro_rules! print {
    ($($arg:tt)*) => {{
        let _ = Stdout.write_fmt(format_args!($($arg)*));
    }};
}

macro_rules! println {
    () => { print!("\n") };
    ($($arg:tt)*) => {{
        let _ = Stdout.write_fmt(format_args!("{}\n", format_args!($($arg)*)));
    }};
}

macro_rules! eprintln {
    ($($arg:tt)*) => {{
        let _ = Stderr.write_fmt(format_args!("{}\n", format_args!($($arg)*)));
    }};
}

fn main() -> i32 {
    print!("no newline yet");
    println!(", now there is.");
    println!("{:<8}|{:>6}|{:^7}|", "left", 42, "mid");
    println!("{:#x} {:#b} {:+}", 255, 5, 7);
    println!("{:?} {:.2}", Some([1, 2, 3]), 3.14159_f32);
    println!();
    let big = [b'#'; 64];
    if write_all(1, &big).is_err() {
        return 1;
    }
    println!();
    eprintln!("warning: {} retries", 3);
    0
}

extern "C" fn rust_start(_sp: *const usize) -> ! {
    sys_exit(main())
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    sys_exit(101)
}

// ---------------------------------------------------------------------------
// Support code from earlier exercises (given): entry point, system calls and
// the symbols the compiler expects. Read it, but you shouldn't need to edit it.

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

// memcpy & co (from rt1). This file is #![no_builtins] for their sake.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    let (d, s) = (dest.cast::<u8>(), src.cast::<u8>());
    let mut i = 0;
    while i < n {
        // SAFETY: C memcpy contract.
        unsafe { *d.add(i) = *s.add(i) };
        i += 1;
    }
    dest
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    let (d, s) = (dest.cast::<u8>(), src.cast::<u8>());
    if (d as usize) <= (s as usize) {
        let mut i = 0;
        while i < n {
            // SAFETY: C memmove contract; forward copy is safe here.
            unsafe { *d.add(i) = *s.add(i) };
            i += 1;
        }
    } else {
        let mut i = n;
        while i > 0 {
            i -= 1;
            // SAFETY: C memmove contract; backward copy is safe here.
            unsafe { *d.add(i) = *s.add(i) };
        }
    }
    dest
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(s: *mut c_void, c: c_int, n: usize) -> *mut c_void {
    let p = s.cast::<u8>();
    let mut i = 0;
    while i < n {
        // SAFETY: C memset contract.
        unsafe { *p.add(i) = c as u8 };
        i += 1;
    }
    s
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(a: *const c_void, b: *const c_void, n: usize) -> c_int {
    let (a, b) = (a.cast::<u8>(), b.cast::<u8>());
    let mut i = 0;
    while i < n {
        // SAFETY: C memcmp contract.
        let (x, y) = unsafe { (*a.add(i), *b.add(i)) };
        if x != y {
            return c_int::from(x) - c_int::from(y);
        }
        i += 1;
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bcmp(a: *const c_void, b: *const c_void, n: usize) -> c_int {
    // SAFETY: same contract as memcmp.
    unsafe { memcmp(a, b, n) }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_eh_personality() {}
