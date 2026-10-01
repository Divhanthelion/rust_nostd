//! # args1: Arguments and environment, straight off the stack
//!
//! At `_start`, the kernel has laid out the process arguments on the stack:
//!
//! ```text
//! sp ──► argc
//!        argv[0] … argv[argc-1]   (pointers to NUL-terminated strings)
//!        NULL
//!        envp[0] … envp[n-1]      ("KEY=value" strings)
//!        NULL
//! ```
//!
//! Implement:
//! 1. `cstr_bytes(p)`: the bytes of a NUL-terminated string (without the
//!    NUL), by scanning for the terminator: your own `strlen`.
//! 2. `rust_start(sp)`: decode argc, argv and envp into `Args` and `Env`.
//! 3. `Args::get(i)` and `Env::get(name)`. `get(b"HOME")` must match
//!    `HOME=...` exactly: not `HOMER=...`, not `HOME_DIR=...`.
//!
//! `main` (given) prints the argument count, each argument (only the file
//! name of `argv[0]`, since the full path depends on your machine), and the
//! value of `NOSTD_GREETING`. The checker runs it with different arguments
//! and environments.
#![no_std]
#![no_main]
#![no_builtins]

use core::ffi::{c_int, c_void};
use core::fmt::Write;

/// The bytes of the NUL-terminated string at `p`.
///
/// # Safety
/// `p` must point to a NUL-terminated string that lives for the rest of the
/// program (true for argv/envp strings).
unsafe fn cstr_bytes(p: *const u8) -> &'static [u8] {
    let mut len = 0;
    // SAFETY: the caller guarantees a terminating NUL exists.
    while unsafe { *p.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: the `len` bytes before the NUL are readable and live forever.
    unsafe { core::slice::from_raw_parts(p, len) }
}

pub struct Args {
    argc: usize,
    argv: *const *const u8,
}

impl Args {
    pub fn len(&self) -> usize {
        self.argc
    }

    pub fn get(&self, i: usize) -> Option<&'static [u8]> {
        if i >= self.argc {
            return None;
        }
        // SAFETY: argv[0..argc] are valid string pointers (kernel guarantee).
        Some(unsafe { cstr_bytes(*self.argv.add(i)) })
    }
}

pub struct Env {
    envp: *const *const u8,
}

impl Env {
    /// The value of environment variable `name`, if set.
    pub fn get(&self, name: &[u8]) -> Option<&'static [u8]> {
        let mut p = self.envp;
        loop {
            // SAFETY: envp is a NULL-terminated array of string pointers.
            let entry = unsafe { *p };
            if entry.is_null() {
                return None;
            }
            // SAFETY: each entry is a valid NUL-terminated string.
            let kv = unsafe { cstr_bytes(entry) };
            if let Some(rest) = kv.strip_prefix(name) {
                if let Some(value) = rest.strip_prefix(b"=") {
                    return Some(value);
                }
            }
            // SAFETY: we haven't reached the terminating NULL yet.
            p = unsafe { p.add(1) };
        }
    }
}

extern "C" fn rust_start(sp: *const usize) -> ! {
    // SAFETY: this is the layout the kernel guarantees at process entry.
    let (args, env) = unsafe {
        let argc = *sp;
        let argv = sp.add(1) as *const *const u8;
        let envp = argv.add(argc + 1);
        (Args { argc, argv }, Env { envp })
    };
    sys_exit(main(&args, &env))
}

fn main(args: &Args, env: &Env) -> i32 {
    let mut out = Stdout;
    let _ = writeln!(out, "argc={}", args.len());
    for i in 0..args.len() {
        let arg = args.get(i).unwrap_or(b"?");
        let shown = if i == 0 { arg.rsplit(|&b| b == b'/').next().unwrap_or(arg) } else { arg };
        let _ = writeln!(out, "argv[{}]={}", i, core::str::from_utf8(shown).unwrap_or("<not utf-8>"));
    }
    match env.get(b"NOSTD_GREETING") {
        Some(v) => {
            let _ = writeln!(out, "greeting={}", core::str::from_utf8(v).unwrap_or("<not utf-8>"));
        }
        None => {
            let _ = writeln!(out, "greeting=(unset)");
        }
    }
    0
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
