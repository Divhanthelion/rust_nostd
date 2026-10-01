# The no_std Landscape

Rust's standard library is really three libraries stacked on top of each other.
Writing no_std code means choosing to stand on a lower layer.

```text
 ┌──────────────────────────────────────────────────────────────┐
 │ std    files, threads, sockets, time, env, process, stdio,   │
 │        HashMap, Mutex, println!, panic unwinding, float math  │
 │        needs: an operating system                            │
 ├──────────────────────────────────────────────────────────────┤
 │ alloc  Box, Vec, String, Rc, Arc, BTreeMap, VecDeque, format! │
 │        needs: a global allocator (#[global_allocator])        │
 ├──────────────────────────────────────────────────────────────┤
 │ core   the language itself: primitives, Option/Result,       │
 │        slices, str, iterators, traits, fmt, cells, atomics,  │
 │        ptr, mem, Future, panicking                            │
 │        needs: nothing (well, almost: see "the platform contract") │
 └──────────────────────────────────────────────────────────────┘
```

`std` re-exports almost everything from `core` and `alloc` at the same paths:
`std::fmt` *is* `core::fmt`, `std::vec::Vec` *is* `alloc::vec::Vec`. That's why
porting code to no_std is often mostly a matter of changing `std::` to `core::`.
The hard part is the remaining 10%: the things that genuinely need an OS or
a heap.

## What `#![no_std]` actually does

The attribute goes at the top of the crate root (`lib.rs`/`main.rs`):

```rust
#![no_std]
```

It does exactly three things:

1. **It stops the implicit `extern crate std;`.** Normally every crate gets
   `std` linked and added to the "extern prelude". Under no_std, writing
   `std::anything` fails to resolve.
2. **It links `core` instead** (`extern crate core;` is implied).
3. **It switches the prelude** from `std::prelude` to `core::prelude`. You still
   get `Option`, `Some`, `Result`, `Ok`, `Iterator`, `Copy`, `Clone`, `Drop`,
   `From`/`Into`, `TryFrom`/`TryInto`… but not `Vec`, `String`, `Box` or
   `ToString`, which live in `alloc`.

It does **not** change code generation, add a runtime, or remove any language
feature. Traits, generics, closures, pattern matching, iterators, `async`/`await`,
`unsafe`: all of the language works the same.

`alloc` is opt-in even under no_std:

```rust
#![no_std]
extern crate alloc;                 // now alloc:: paths resolve

use alloc::vec::Vec;
use alloc::string::String;
use alloc::collections::BTreeMap;
```

## Why go no_std?

- **No operating system.** Microcontrollers (Cortex-M, RISC-V, AURIX, RH850)
  boot straight into your code. There is no filesystem, no threads, no `malloc`
  unless you provide one.
- **You *are* the operating system.** Kernels, hypervisors, bootloaders and
  firmware can't depend on services they are supposed to implement.
- **Determinism and certification.** Safety-critical software avoids dynamic
  memory and keeps its dependency surface small and auditable. `core` is small
  enough to be certified.
- **Portability.** A no_std library runs everywhere: in firmware, in a Linux
  service, in WebAssembly, in a kernel module. Many popular crates (`serde`,
  `heapless`, `nom`, `crc`, `bitflags`…) are no_std precisely so they can be used
  anywhere.
- **Size.** A freestanding binary can be a few kilobytes.

## What you give up (and what replaces it)

| std feature | Why it needs std | no_std replacement |
|---|---|---|
| `Vec`, `String`, `Box` | need a heap | `alloc` + an allocator, or fixed-capacity types (`heapless`, your own) |
| `HashMap`/`HashSet` | random keys from the OS | `BTreeMap` (alloc), `hashbrown`, `heapless::IndexMap` |
| `println!` | stdout is an OS service | `core::fmt::Write` over a UART/RTT/syscall, `defmt` |
| `std::io::{Read, Write}` | OS I/O | `embedded-io` traits, your own traits |
| `File`, `TcpStream` | OS services | flash drivers, `smoltcp`, `embassy-net` |
| `thread::spawn`, `Mutex` | OS scheduler | interrupts, RTIC, Embassy, critical sections, spinlocks |
| `Instant`, `sleep` | OS clock | hardware timers, `DelayNs`, tick counters |
| `env::args`, `process::exit` | OS process model | read the initial stack yourself; system calls; reset |
| panic unwinding, `catch_unwind` | unwinder runtime | `panic = "abort"` + your `#[panic_handler]` |
| `f32::sqrt`, `sin`, `powf` | platform libm | the `libm` crate, fixed-point math |

What you **keep** is the whole language plus a remarkable amount of library:
`Option`/`Result` and their combinators, every iterator adapter, slices and their
algorithms (`sort_unstable`, `binary_search`, `chunks`, `windows`), `str` methods,
`char`, all integer arithmetic (checked/wrapping/saturating), `core::fmt`
(Display, Debug, `write!`), `Cell`/`RefCell`/`OnceCell`, atomics, raw pointers,
`MaybeUninit`, `Future`/`Waker`, `core::error::Error`, `core::net::Ipv4Addr`,
`core::ffi::CStr`, `core::time::Duration`.

> **Try it:** `nostd where Mutex`, `nostd where sqrt`, `nostd where BTreeMap`.

## Libraries vs. final artifacts

A no_std **library** needs nothing beyond `#![no_std]`. It must *not* make
choices that belong to the final program.

A no_std **final artifact** (a binary, or a static library linked into C) is
where the platform gets decided, so it has obligations:

| Obligation | Who provides it | When |
|---|---|---|
| `#[panic_handler]` | exactly one crate in the final link, usually the binary or a `panic-*` crate | always |
| entry point (`#![no_main]` + reset handler / `_start`) | the binary or a runtime crate (`cortex-m-rt`) | binaries |
| `#[global_allocator]` | the binary or an allocator crate | only if anything uses `alloc` |
| `memcpy`, `memset`, `memcmp`… | libc, or `compiler_builtins` on bare-metal targets, or you | always (implicitly) |
| `panic = "abort"` | Cargo profile | binaries (no unwinder) |

Rule of thumb: **libraries never define `#[panic_handler]` or
`#[global_allocator]`**. If two crates both do, the program fails to link.

## Targets

Rust names platforms with *target triples*: `arch-vendor-os-abi`.

| Target | What it is |
|---|---|
| `x86_64-unknown-linux-gnu` | a normal Linux PC (has std) |
| `thumbv6m-none-eabi` | Cortex-M0/M0+: no FPU, no compare-and-swap atomics |
| `thumbv7m-none-eabi` | Cortex-M3 |
| `thumbv7em-none-eabihf` | Cortex-M4F/M7F with hardware floating point |
| `thumbv8m.main-none-eabihf` | Cortex-M33 (TrustZone-M) |
| `armv7r-none-eabihf` | Cortex-R (lock-step safety MCUs, e.g. in brake/steering ECUs) |
| `riscv32imac-unknown-none-elf` | 32-bit RISC-V microcontrollers |
| `aarch64-unknown-none` | 64-bit Arm bare metal (hypervisors, bootloaders) |

`none` in the OS position means bare metal: those targets ship **only** `core`
and `alloc`. Installing one is a single command:

```console
$ rustup target add thumbv7em-none-eabihf
$ cargo build --target thumbv7em-none-eabihf
```

Building your library for such a target is the gold-standard proof that it is
really no_std, because there's no std to accidentally link. (Even a crate
with `#![no_std]` can pull std back in through a dependency that isn't no_std.)

## How this course proves your code is no_std

Building for a microcontroller proves no_std-ness, but then you can't run your
tests on the laptop. The checker gets both by building your library twice:

1. against a **core-only sysroot**: a copy of your toolchain's library folder
   that contains just `core`, `alloc` and `compiler_builtins` for your host. Any
   use of `std` is now a hard compile error (`can't find crate for std`), exactly
   as on a bare-metal target;
2. normally, as a test binary, so the `#[cfg(test)]` module can use `std` for
   convenience (`extern crate std;` inside the tests is fine).

Tests are allowed to use std because they run on your development machine.
That's also standard practice in real projects: keep the library no_std, test
it on the host.

## Edition 2024 notes

The course uses edition 2024 (Rust 1.85+). It tightens rules that matter a lot
in low-level code, and you'll meet each of them:

- `#[no_mangle]`, `#[export_name]` and `#[link_section]` must be written as
  `#[unsafe(no_mangle)]` etc.: they can break soundness at link time.
- `extern "C" { ... }` blocks must be `unsafe extern "C" { ... }`.
- Taking a reference to a `static mut` is an error. Use `&raw const`/`&raw mut`
  or, better, a safe abstraction.
- Inside an `unsafe fn`, unsafe operations still need their own `unsafe { }`
  block (`unsafe_op_in_unsafe_fn` warns).

> **Toyota lens:** Ferrocene, the qualified Rust toolchain from Ferrous Systems,
> is certified for ISO 26262 up to ASIL D (TCL 3), and it now ships a certified
> *subset of `core`*. Certification effort scales with code size, so it is
> `core`, not `std`, that safety projects can lean on. Thinking in `core` first
> is exactly the habit those projects want. The Safety-Critical Rust Consortium
> (Woven by Toyota is a founding member) is writing coding guidelines on the same
> foundation.

## Looking things up

- `nostd where <Item>` gives a quick answer with alternatives.
- The API docs exist per layer: <https://doc.rust-lang.org/core/>,
  <https://doc.rust-lang.org/alloc/>. If an item is documented in `core`, you
  can use it.
- On docs.rs, a crate that supports no_std usually says so, often behind
  `default-features = false`.

> **Interview:** "What's the difference between `core`, `alloc` and `std`?" and
> "What does a no_std binary need that a no_std library doesn't?" are the two
> most common opening questions. You should be able to answer both from the
> tables above.
