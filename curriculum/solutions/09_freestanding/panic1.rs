//! # panic1: A panic handler worth having
//!
//! When something panics in a no_std program, your `#[panic_handler]` is
//! the last code that runs. Make it report *where* and *what*, on stderr,
//! in exactly this format, then exit with status 101:
//!
//! ```text
//! PANIC [<file>:<line>] <message>
//! ```
//!
//! - `info.location()` gives an `Option<&Location>` with `file()` and `line()`.
//!   Without a location print `PANIC [unknown] <message>`.
//! - `info.message()` implements `Display`: it renders the panic message,
//!   e.g. `index out of bounds: the len is 3 but the index is 7`.
//! - Write with `write!`/`writeln!` into the provided `Stderr` and **ignore
//!   errors** (`let _ = ...`). A panic handler must never panic itself.
//!
//! `main` (given) panics differently depending on how many arguments the
//! program receives; the checker runs all three variants.
#![no_std]
#![no_main]
#![no_builtins]

use core::ffi::{c_int, c_void};
use core::fmt::Write;
use core::hint::black_box;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let mut err = Stderr;
    match info.location() {
        Some(loc) => {
            let _ = write!(err, "PANIC [{}:{}] ", loc.file(), loc.line());
        }
        None => {
            let _ = write!(err, "PANIC [unknown] ");
        }
    }
    let _ = writeln!(err, "{}", info.message());
    sys_exit(101)
}

fn main(argc: usize) -> i32 {
    let readings = [10, 20, 30];
    match argc {
        1 => {
            let i = black_box(argc + 6);
            let _ = writeln!(Stdout, "reading {}", readings[i]);
        }
        2 => panic!("sensor {} timed out", black_box(7)),
        _ => {
            let _ = writeln!(Stdout, "no panic");
        }
    }
    0
}

extern "C" fn rust_start(sp: *const usize) -> ! {
    // SAFETY: the kernel puts argc at the initial stack pointer.
    let argc = unsafe { *sp };
    sys_exit(main(argc))
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
