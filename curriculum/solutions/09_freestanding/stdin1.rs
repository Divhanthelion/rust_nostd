//! # stdin1: A streaming filter with a 64-byte buffer
//!
//! Write a Unix filter: read standard input until end of file, transform
//! it, write it to standard output. Memory is fixed: one 64-byte buffer,
//! however large the input is.
//!
//! 1. `sys_read(fd, buf)`: the `read` system call (number 0 on x86_64, 63 on
//!    aarch64). It returns the number of bytes read, **0 at end of file**, or
//!    `-errno`.
//! 2. `main`: loop: read into the buffer (retry on `-4`/EINTR; any other
//!    negative result → exit code 1), apply ROT13 to ASCII letters in place
//!    (`a`↔`n`, `B`↔`O`, …; everything else unchanged), write the chunk out with
//!    `write_all`, and count bytes and `\n` characters.
//! 3. At end of file, write `bytes=<N> lines=<M>\n` to **stderr** and return 0.
//!
//! The checker feeds inputs larger than 64 bytes, so your loop must handle
//! many reads.
#![no_std]
#![no_main]
#![no_builtins]

use core::ffi::{c_int, c_void};
use core::fmt::Write;

const BUF_SIZE: usize = 64;

#[cfg(target_arch = "x86_64")]
fn sys_read(fd: i32, buf: &mut [u8]) -> isize {
    let ret: isize;
    // SAFETY: read(2) writes at most buf.len() bytes into a valid buffer.
    unsafe {
        core::arch::asm!("syscall", inlateout("rax") 0isize => ret, in("rdi") fd as usize,
            in("rsi") buf.as_mut_ptr(), in("rdx") buf.len(), lateout("rcx") _, lateout("r11") _, options(nostack));
    }
    ret
}

#[cfg(target_arch = "aarch64")]
fn sys_read(fd: i32, buf: &mut [u8]) -> isize {
    let ret: isize;
    // SAFETY: read(2) writes at most buf.len() bytes into a valid buffer.
    unsafe {
        core::arch::asm!("svc #0", inlateout("x0") fd as isize => ret, in("x1") buf.as_mut_ptr(),
            in("x2") buf.len(), in("x8") 63usize, options(nostack));
    }
    ret
}

fn rot13(b: u8) -> u8 {
    match b {
        b'a'..=b'z' => (b - b'a' + 13) % 26 + b'a',
        b'A'..=b'Z' => (b - b'A' + 13) % 26 + b'A',
        _ => b,
    }
}

fn main() -> i32 {
    let mut buf = [0u8; BUF_SIZE];
    let (mut bytes, mut lines) = (0usize, 0usize);
    loop {
        let n = sys_read(0, &mut buf);
        if n == -4 {
            continue;
        }
        if n < 0 {
            return 1;
        }
        if n == 0 {
            break;
        }
        let chunk = &mut buf[..n as usize];
        bytes += chunk.len();
        lines += chunk.iter().filter(|&&b| b == b'\n').count();
        for b in chunk.iter_mut() {
            *b = rot13(*b);
        }
        if write_all(1, chunk).is_err() {
            return 1;
        }
    }
    let _ = writeln!(Stderr, "bytes={bytes} lines={lines}");
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

// Output (from print1).
fn write_all(fd: i32, mut buf: &[u8]) -> Result<(), isize> {
    while !buf.is_empty() {
        let n = sys_write(fd, buf);
        if n == -4 {
            continue; // EINTR
        }
        if n <= 0 {
            return Err(n);
        }
        buf = buf.get(n as usize..).unwrap_or(&[]);
    }
    Ok(())
}

struct Stdout;
struct Stderr;

impl core::fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        write_all(1, s.as_bytes()).map_err(|_| core::fmt::Error)
    }
}

impl core::fmt::Write for Stderr {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        write_all(2, s.as_bytes()).map_err(|_| core::fmt::Error)
    }
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
