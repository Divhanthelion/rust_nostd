# Freestanding Linux Binaries

So far you've written no_std *libraries*. Now you'll build complete no_std
*programs* that the Linux kernel runs directly: no libc, no C runtime, no
Rust runtime. Just your code, the CPU and the system-call interface.

Why Linux instead of a microcontroller? Because every concept carries over
one-to-one (entry point, runtime symbols, panic handler, linking), and you can
run, test and debug the result instantly on your machine. When you get to
bare metal in module 12, only the *platform* changes.

## What normally happens before `main`

```text
 kernel ──► _start (crt1.o, from libc) ──► __libc_start_main ──► Rust's std runtime ──► your main
              set up stack/argv              init libc, atexit       stack guard, args,
                                                                     panic hooks, stdio
```

`#![no_main]` tells rustc not to generate the usual `main` shim. Linking
with `-nostartfiles` drops `crt1.o`. Now nothing defines the program's entry
symbol, `_start`, so we define it ourselves.

## The minimal program

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

core::arch::global_asm!(
    ".globl _start",
    "_start:",
    "mov rdi, rsp",          // pass the initial stack pointer to Rust
    "and rsp, -16",          // the SysV ABI wants 16-byte alignment at calls
    "call {entry}",
    "ud2",                   // entry never returns
    entry = sym rust_start,
);

extern "C" fn rust_start(_sp: *const usize) -> ! {
    exit(0)
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    exit(101)
}
```

Why `global_asm!` instead of a Rust `fn _start`? The kernel *jumps* to
`_start` with the stack pointer 16-byte aligned. A Rust function expects to be
*called*, which pushes a return address and leaves it misaligned by 8.
Compiled code may then use aligned SSE stores on the stack and crash. A few
instructions of assembly put things right. (Since Rust 1.88 a
`#[unsafe(naked)]` function with `naked_asm!` can do the same.)

Build flags, all of which the checker passes for you:

| Flag | Why |
|---|---|
| `-C panic=abort` | no unwinder exists |
| `-C link-arg=-nostartfiles` | don't link crt1.o/crti.o: our `_start` is the entry |
| `-C link-arg=-static` | static executable: no dynamic loader involved |
| `-C relocation-model=static` | non-PIE code: nothing needs relocating at load time |

In Cargo, put the link args in a `build.rs` using
`cargo:rustc-link-arg-bins=...` so they apply to the binary only, and set
`panic = "abort"` in `[profile.*]`. `nostd new linux-bin` sets all of this up.

## System calls

A system call is a special instruction that traps into the kernel. The
number in one register selects the service; arguments go in others.

| | x86_64 | aarch64 |
|---|---|---|
| instruction | `syscall` | `svc #0` |
| number in | `rax` | `x8` |
| args in | `rdi, rsi, rdx, r10, r8, r9` | `x0 … x5` |
| result in | `rax` | `x0` |
| clobbered | `rcx`, `r11` | (nothing else) |
| `read` / `write` / `exit_group` | 0 / 1 / 231 | 63 / 64 / 94 |

A negative result in `-4095..=-1` is `-errno` (e.g. `-4` = `EINTR`, `-9` = `EBADF`).

```rust
use core::arch::asm;

#[cfg(target_arch = "x86_64")]
fn write(fd: i32, buf: &[u8]) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") 1isize => ret,          // number in, result out
            in("rdi") fd as usize,
            in("rsi") buf.as_ptr(),
            in("rdx") buf.len(),
            lateout("rcx") _, lateout("r11") _,      // clobbered by `syscall`
            options(nostack),
        );
    }
    ret
}

#[cfg(target_arch = "aarch64")]
fn write(fd: i32, buf: &[u8]) -> isize {
    let ret: isize;
    unsafe {
        asm!("svc #0",
             inlateout("x0") fd as isize => ret,
             in("x1") buf.as_ptr(), in("x2") buf.len(), in("x8") 64usize,
             options(nostack));
    }
    ret
}
```

`asm!` operands: `in(reg) value`, `out(reg) var`, `inout(reg) a => b`,
`lateout(reg) _` (clobbered); `options(noreturn)` for code that never returns
(`exit_group`), `nostack` when the asm doesn't touch the stack.

Robust I/O handles two things beginners forget:

- **short writes**: `write` may write fewer bytes than asked; loop until done.
- **EINTR**: a signal can interrupt the call; just retry.

## The platform contract: symbols the compiler assumes

Your first non-trivial program will fail to link with something like:

```text
rust-lld: error: undefined symbol: memcpy
rust-lld: error: undefined symbol: memset
```

Rust and LLVM assume every platform provides a handful of C functions,
because they're the fastest way to copy, fill and compare memory, and the
compiler emits calls to them for array copies, zero-initialisation, slice
comparisons and large moves:

| Symbol | Emitted for |
|---|---|
| `memcpy(dst, src, n)` | copying arrays/structs, `copy_from_slice` |
| `memmove(dst, src, n)` | overlapping copies: `copy_within`, `ptr::copy` |
| `memset(s, c, n)` | zeroing/filling: `[0u8; 512]` at runtime, `fill` |
| `memcmp(a, b, n)` | ordering byte slices (`a.cmp(b)`) |
| `bcmp(a, b, n)` | equality of byte slices (`a == b`) |
| `strlen(s)` | `CStr::from_ptr` |

Normally libc provides them. On bare-metal targets `compiler_builtins`
provides them. On our libc-free host binary, **you** provide them, with exact
C signatures:

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(s: *mut c_void, c: c_int, n: usize) -> *mut c_void { ... }
```

Two traps:

1. **Infinite recursion.** LLVM's loop-idiom pass recognises a byte-copy loop
   as "this is a memcpy" and replaces it with... a call to `memcpy`. Modern
   LLVM refuses to do this inside a function literally named `memcpy`, but a
   helper function, or a `memset`-like loop inside your `memcpy`, can still turn
   into a self-call → stack overflow → SIGSEGV. The crate-level attribute
   **`#![no_builtins]`** switches the transformation off for the whole crate.
   (Beware: no_builtins crates are excluded from LTO, so keep them tiny and
   don't call into `core` from them.)
2. **`rust_eh_personality`.** The host's precompiled `core` was built with
   unwinding, and its unwind tables reference this symbol. With
   `panic=abort` nothing ever unwinds, so an empty
   `#[unsafe(no_mangle)] extern "C" fn rust_eh_personality() {}` satisfies the
   linker if it complains. Bare-metal targets ship a `core` without this.

Since Rust 1.98, the deny-by-default lint `invalid_runtime_symbol_definitions`
checks that your `memcpy`/`memset`/... definitions have the C signatures.

## The process stack at `_start`

The kernel passes arguments and environment on the initial stack:

```text
 sp ──► argc                      (usize)
        argv[0] … argv[argc-1]    (*const c_char each, NUL-terminated)
        NULL
        envp[0] … envp[n-1]       ("KEY=value", NUL-terminated)
        NULL
        auxv pairs …              (AT_PAGESZ, AT_RANDOM, … for libc's use)
```

So `argc = *sp`, `argv = sp.add(1)`, `envp = argv.add(argc + 1)`. Strings are
C strings: find their length by scanning for the 0 byte (that's `strlen`).
They live for the whole program, so `&'static [u8]` slices into them are
legitimate.

## Panics in a freestanding program

Your `#[panic_handler]` is the last code that runs. A good one for a
command-line tool:

```rust
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let mut err = Stderr;                       // your fmt::Write over fd 2
    if let Some(loc) = info.location() {
        let _ = write!(err, "panicked at {}:{}: ", loc.file(), loc.line());
    }
    let _ = writeln!(err, "{}", info.message());
    exit(101)
}
```

Keep it simple: anything that can panic inside the handler recurses.

## Packaging it: a runtime crate

After writing `_start`, syscalls, `print!` and `memcpy` & co by hand, you'll
want them in one reusable crate. That's exactly what `cortex-m-rt` is for
microcontrollers: it owns the entry point, initialises the world, and calls
your `main`. The course ships one for Linux, `nostd_rt` (you can read it in
`support/nostd_rt.rs` in your workspace), and from module 10 on you use it:

```rust
#![no_std]
#![no_main]
extern crate nostd_rt;
use nostd_rt::{println, Args};

fn main(args: Args) -> i32 {
    println!("{} arguments", args.len());
    0
}
nostd_rt::entry!(main);     // type-checks the signature, like #[cortex_m_rt::entry]
```

> **Toyota lens:** you rarely ship a libc-free Linux binary in a car, but the
> knowledge transfers exactly: AUTOSAR Classic startup code, the C runtime of
> a safety-certified RTOS, and Rust's `cortex-m-rt` all do what you're doing
> here (entry point, memory initialisation, the symbols the compiler needs).
> Engineers who understand that layer debug "it hangs before main" problems
> in minutes instead of days.

> **Interview:** "What happens between power-on (or exec) and `main`?" and "why
> does my no_std binary fail to link with `undefined symbol: memcpy`?" are
> excellent questions to have a crisp answer for.
