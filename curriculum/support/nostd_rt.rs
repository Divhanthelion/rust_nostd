//! # nostd_rt: the course runtime for freestanding Linux programs
//!
//! This crate is to Linux what `cortex-m-rt` is to a Cortex-M microcontroller:
//!
//! * it owns the real entry point (`_start`), which the kernel jumps to;
//! * it collects what the platform hands over (argc/argv/envp on the stack);
//! * it calls *your* main function, registered with [`entry!`];
//! * it provides the handful of symbols the compiler assumes exist
//!   (`memcpy`, `memmove`, `memset`, `memcmp`, `bcmp`, `strlen`,
//!   `rust_eh_personality`).
//!
//! You build every piece of this by hand in module 09. From module 10 on,
//! exercises link this packaged version with `extern crate nostd_rt;`.
//!
//! Supported: Linux on x86_64 and aarch64.

#![no_std]
// Stop LLVM from recognising the byte loops below as "memcpy idioms" and
// replacing them with calls to... memcpy. That would recurse forever.
#![no_builtins]

use core::ffi::{c_char, c_int, c_void};
use core::fmt;

// ---------------------------------------------------------------------------
// Raw system calls

/// Thin wrappers over the Linux system calls this runtime needs.
pub mod sys {
    #[cfg(target_arch = "x86_64")]
    mod nr {
        pub const READ: usize = 0;
        pub const WRITE: usize = 1;
        pub const EXIT_GROUP: usize = 231;
    }
    #[cfg(target_arch = "aarch64")]
    mod nr {
        pub const READ: usize = 63;
        pub const WRITE: usize = 64;
        pub const EXIT_GROUP: usize = 94;
    }

    /// `errno` value for "interrupted system call": just retry.
    pub const EINTR: isize = 4;

    /// Raw three-argument system call. Returns the kernel's result:
    /// non-negative on success, `-errno` on failure.
    ///
    /// # Safety
    /// The arguments must be valid for the chosen system call.
    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    pub unsafe fn syscall3(n: usize, a0: usize, a1: usize, a2: usize) -> isize {
        let ret: isize;
        // SAFETY: forwarded to the caller. `syscall` clobbers rcx and r11.
        unsafe {
            core::arch::asm!(
                "syscall",
                inlateout("rax") n as isize => ret,
                in("rdi") a0, in("rsi") a1, in("rdx") a2,
                lateout("rcx") _, lateout("r11") _,
                options(nostack),
            );
        }
        ret
    }

    /// Raw three-argument system call. Returns the kernel's result:
    /// non-negative on success, `-errno` on failure.
    ///
    /// # Safety
    /// The arguments must be valid for the chosen system call.
    #[cfg(target_arch = "aarch64")]
    #[inline(always)]
    pub unsafe fn syscall3(n: usize, a0: usize, a1: usize, a2: usize) -> isize {
        let ret: isize;
        // SAFETY: forwarded to the caller.
        unsafe {
            core::arch::asm!(
                "svc #0",
                inlateout("x0") a0 as isize => ret,
                in("x1") a1, in("x2") a2, in("x8") n,
                options(nostack),
            );
        }
        ret
    }

    /// write(2): returns bytes written or `-errno`.
    pub fn write(fd: i32, buf: &[u8]) -> isize {
        // SAFETY: the pointer/length pair comes from a valid slice.
        unsafe { syscall3(nr::WRITE, fd as usize, buf.as_ptr() as usize, buf.len()) }
    }

    /// read(2): returns bytes read (0 = end of file) or `-errno`.
    pub fn read(fd: i32, buf: &mut [u8]) -> isize {
        // SAFETY: the pointer/length pair comes from a valid mutable slice.
        unsafe { syscall3(nr::READ, fd as usize, buf.as_mut_ptr() as usize, buf.len()) }
    }

    /// exit_group(2): terminate the whole process.
    pub fn exit(code: i32) -> ! {
        // SAFETY: exit_group takes a plain integer and never returns.
        unsafe {
            syscall3(nr::EXIT_GROUP, code as usize, 0, 0);
        }
        // Unreachable, but the compiler can't know that the kernel won't return.
        #[allow(clippy::empty_loop)]
        loop {}
    }
}

/// Write all of `buf` to `fd`, retrying on short writes and EINTR.
pub fn write_all(fd: i32, mut buf: &[u8]) -> Result<(), isize> {
    while !buf.is_empty() {
        let n = sys::write(fd, buf);
        if n == -sys::EINTR {
            continue;
        }
        if n <= 0 {
            return Err(n);
        }
        buf = &buf[n as usize..];
    }
    Ok(())
}

/// Read into `buf` from `fd`, retrying on EINTR. `Ok(0)` means end of file.
pub fn read(fd: i32, buf: &mut [u8]) -> Result<usize, isize> {
    loop {
        let n = sys::read(fd, buf);
        if n == -sys::EINTR {
            continue;
        }
        return if n < 0 { Err(n) } else { Ok(n as usize) };
    }
}

/// Terminate the process with `code`.
pub fn exit(code: i32) -> ! {
    sys::exit(code)
}

// ---------------------------------------------------------------------------
// Formatted output

/// Standard output as a `core::fmt::Write` sink.
pub struct Stdout;
/// Standard error as a `core::fmt::Write` sink.
pub struct Stderr;

impl fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_all(1, s.as_bytes()).map_err(|_| fmt::Error)
    }
}

impl fmt::Write for Stderr {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_all(2, s.as_bytes()).map_err(|_| fmt::Error)
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    let _ = fmt::Write::write_fmt(&mut Stdout, args);
}

#[doc(hidden)]
pub fn _eprint(args: fmt::Arguments) {
    let _ = fmt::Write::write_fmt(&mut Stderr, args);
}

/// Like std's `print!`, writing straight to file descriptor 1.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => { $crate::_print(format_args!($($arg)*)) };
}

/// Like std's `println!`.
#[macro_export]
macro_rules! println {
    () => { $crate::_print(format_args!("\n")) };
    ($($arg:tt)*) => { $crate::_print(format_args!("{}\n", format_args!($($arg)*))) };
}

/// Like std's `eprint!`, writing to file descriptor 2.
#[macro_export]
macro_rules! eprint {
    ($($arg:tt)*) => { $crate::_eprint(format_args!($($arg)*)) };
}

/// Like std's `eprintln!`.
#[macro_export]
macro_rules! eprintln {
    () => { $crate::_eprint(format_args!("\n")) };
    ($($arg:tt)*) => { $crate::_eprint(format_args!("{}\n", format_args!($($arg)*))) };
}

// ---------------------------------------------------------------------------
// Process arguments and environment

/// The command-line arguments, as raw byte strings (no allocation, no UTF-8
/// assumption: Linux arguments are arbitrary bytes).
#[derive(Clone, Copy)]
pub struct Args {
    argc: usize,
    argv: *const *const c_char,
}

impl Args {
    /// Number of arguments, including the program name.
    pub fn len(&self) -> usize {
        self.argc
    }

    /// True when there are no arguments at all (unusual on Linux).
    pub fn is_empty(&self) -> bool {
        self.argc == 0
    }

    /// Argument `i` as bytes (without the trailing NUL).
    pub fn get(&self, i: usize) -> Option<&'static [u8]> {
        if i >= self.argc {
            return None;
        }
        // SAFETY: the kernel guarantees argv[0..argc] are valid NUL-terminated
        // strings that live for the whole process.
        unsafe {
            let p = *self.argv.add(i);
            Some(core::slice::from_raw_parts(p as *const u8, cstr_len(p)))
        }
    }

    /// Argument `i` as `&str`, if it is valid UTF-8.
    pub fn get_str(&self, i: usize) -> Option<&'static str> {
        self.get(i).and_then(|b| core::str::from_utf8(b).ok())
    }

    /// Iterate over all arguments, program name first.
    pub fn iter(&self) -> impl Iterator<Item = &'static [u8]> + '_ {
        (0..self.argc).filter_map(move |i| self.get(i))
    }
}

/// Length of a NUL-terminated string.
///
/// # Safety
/// `p` must point to a NUL-terminated string.
unsafe fn cstr_len(p: *const c_char) -> usize {
    let mut n = 0;
    // SAFETY: forwarded to the caller.
    while unsafe { *p.add(n) } != 0 {
        n += 1;
    }
    n
}

static mut ENVP: *const *const c_char = core::ptr::null();

/// Look up an environment variable, e.g. `env(b"HOME")`.
pub fn env(name: &[u8]) -> Option<&'static [u8]> {
    // SAFETY: ENVP is written once in `__nostd_rt_start` before any user code
    // runs and never again; the process is single-threaded.
    let mut p = unsafe { (&raw const ENVP).read() };
    if p.is_null() {
        return None;
    }
    loop {
        // SAFETY: envp is a NULL-terminated array of NUL-terminated strings.
        let s = unsafe { *p };
        if s.is_null() {
            return None;
        }
        // SAFETY: as above.
        let bytes = unsafe { core::slice::from_raw_parts(s as *const u8, cstr_len(s)) };
        if bytes.len() > name.len() && bytes.starts_with(name) && bytes[name.len()] == b'=' {
            return Some(&bytes[name.len() + 1..]);
        }
        // SAFETY: we have not reached the terminating NULL yet.
        p = unsafe { p.add(1) };
    }
}

// ---------------------------------------------------------------------------
// Entry point

/// Register your main function: `nostd_rt::entry!(main);` where
/// `fn main(args: nostd_rt::Args) -> i32`. The return value is the exit code.
///
/// The macro type-checks the signature, exactly like `#[cortex_m_rt::entry]`.
#[macro_export]
macro_rules! entry {
    ($path:path) => {
        #[unsafe(export_name = "__nostd_rt_main")]
        pub fn __nostd_rt_main(args: $crate::Args) -> i32 {
            let f: fn($crate::Args) -> i32 = $path;
            f(args)
        }
    };
}

#[cfg(target_arch = "x86_64")]
core::arch::global_asm!(
    ".globl _start",
    ".type _start,@function",
    "_start:",
    "xor rbp, rbp",        // mark the outermost frame for debuggers
    "mov rdi, rsp",        // arg 0: pointer to argc
    "and rsp, -16",        // the SysV ABI wants 16-byte alignment at calls
    "call {start}",
    "ud2",
    start = sym __nostd_rt_start,
);

#[cfg(target_arch = "aarch64")]
core::arch::global_asm!(
    ".globl _start",
    ".type _start,%function",
    "_start:",
    "mov x29, xzr",
    "mov x30, xzr",
    "mov x0, sp",
    "bl {start}",
    "brk #0",
    start = sym __nostd_rt_start,
);

unsafe extern "C" fn __nostd_rt_start(sp: *const usize) -> ! {
    unsafe extern "Rust" {
        fn __nostd_rt_main(args: Args) -> i32;
    }
    // Initial stack: [argc][argv0..argvN-1][NULL][envp...][NULL][auxv...]
    // SAFETY: this is the layout the Linux kernel guarantees at process entry.
    let (argc, argv, envp) = unsafe {
        let argc = *sp;
        let argv = sp.add(1) as *const *const c_char;
        let envp = argv.add(argc + 1);
        (argc, argv, envp)
    };
    // SAFETY: single-threaded, before any user code runs.
    unsafe { (&raw mut ENVP).write(envp) };
    // SAFETY: provided by the `entry!` macro in the binary crate.
    let code = unsafe { __nostd_rt_main(Args { argc, argv }) };
    sys::exit(code)
}

// >>> mem: symbols the compiler expects from "the platform"
// (`nostd new linux-bin` moves this section into its own #![no_builtins]
// crate: a no_builtins crate is excluded from LTO, so it must not call into
// `core`, or LTO can internalise the functions it needs.)

/// Copy `n` bytes; regions must not overlap.
///
/// # Safety
/// Standard C `memcpy` contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    let (d, s) = (dest as *mut u8, src as *const u8);
    let mut i = 0;
    while i < n {
        // SAFETY: caller guarantees both regions are valid for n bytes.
        unsafe { *d.add(i) = *s.add(i) };
        i = i.wrapping_add(1);
    }
    dest
}

/// Copy `n` bytes; regions may overlap.
///
/// # Safety
/// Standard C `memmove` contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    let (d, s) = (dest as *mut u8, src as *const u8);
    if (d as usize) <= (s as usize) {
        let mut i = 0;
        while i < n {
            // SAFETY: caller guarantees validity; forward copy is safe when dest <= src.
            unsafe { *d.add(i) = *s.add(i) };
            i = i.wrapping_add(1);
        }
    } else {
        let mut i = n;
        while i > 0 {
            i = i.wrapping_sub(1);
            // SAFETY: backward copy is safe when dest > src.
            unsafe { *d.add(i) = *s.add(i) };
        }
    }
    dest
}

/// Fill `n` bytes with `c as u8`.
///
/// # Safety
/// Standard C `memset` contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(s: *mut c_void, c: c_int, n: usize) -> *mut c_void {
    let p = s as *mut u8;
    let mut i = 0;
    while i < n {
        // SAFETY: caller guarantees the region is valid for n bytes.
        unsafe { *p.add(i) = c as u8 };
        i = i.wrapping_add(1);
    }
    s
}

/// Compare `n` bytes, returning <0, 0 or >0.
///
/// # Safety
/// Standard C `memcmp` contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(a: *const c_void, b: *const c_void, n: usize) -> c_int {
    let (a, b) = (a as *const u8, b as *const u8);
    let mut i = 0;
    while i < n {
        // SAFETY: caller guarantees both regions are valid for n bytes.
        let (x, y) = unsafe { (*a.add(i), *b.add(i)) };
        if x != y {
            return c_int::from(x).wrapping_sub(c_int::from(y));
        }
        i = i.wrapping_add(1);
    }
    0
}

/// Like `memcmp` but only equality matters (LLVM emits this for `==` on slices).
///
/// # Safety
/// Standard C `bcmp` contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bcmp(a: *const c_void, b: *const c_void, n: usize) -> c_int {
    // SAFETY: same contract as memcmp.
    unsafe { memcmp(a, b, n) }
}

/// Length of a NUL-terminated string (used by `CStr::from_ptr`).
///
/// # Safety
/// `s` must point to a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strlen(s: *const c_char) -> usize {
    let mut n = 0;
    // SAFETY: caller guarantees a terminating NUL exists.
    while unsafe { *s.add(n) } != 0 {
        n = n.wrapping_add(1);
    }
    n
}

/// The host's precompiled `core` contains unwind tables that name this
/// symbol. With `panic = "abort"` nothing ever unwinds, so it is never called.
#[unsafe(no_mangle)]
pub extern "C" fn rust_eh_personality() {}
// <<< mem
