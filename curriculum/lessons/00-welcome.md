# Welcome to no_std Rust

This course teaches you to write Rust **without the standard library**: the Rust
that runs on microcontrollers, in bootloaders and kernels, inside engine and
brake ECUs, and in any library that wants to run *everywhere*.

By the end you will be able to:

- explain exactly what `#![no_std]` changes and what `core`, `alloc` and `std` each provide;
- design APIs that never allocate and never panic, using fixed-capacity data structures;
- format text, handle errors and choose overflow behaviour without std's conveniences;
- write a panic handler, a global allocator, a spinlock, a lock-free queue and an async executor;
- use `unsafe` responsibly: raw pointers, volatile memory-mapped registers, safety contracts;
- build freestanding Linux programs with no libc, and link Rust into C programs;
- write a Cortex-M vector table and reset handler by hand, and boot it in QEMU;
- write portable drivers against `embedded-hal` traits;
- implement automotive building blocks: CAN frames and signals, ISO-TP, E2E protection,
  watchdog supervision, fixed-point control loops and mode managers.

## How the course works

The tool you are using is your teacher, your compiler driver and your grader.
Each **module** has a lesson (like this one), several **exercises**, and a
**quiz**. The usual loop:

```console
$ nostd next          # opens the next lesson, exercise or quiz
$ nostd watch         # re-checks the current exercise every time you save
$ nostd hint          # reveals one more hint
$ nostd               # where am I?
```

Exercises are Rust files in `exercises/`. Each starts with a `//!` comment that
explains the task. Most contain `todo!()` placeholders and a test module at the
bottom. Edit the file in your editor, save, and the checker tells you what
is still wrong.

### How exercises are checked

The checks reproduce real no_std conditions as closely as a laptop can:

| Exercise kind | What the checker does |
|---|---|
| no_std library | Builds your file against a sysroot that contains **only** `core`, `alloc` and `compiler_builtins`, so std is physically absent, then runs your tests on the host. |
| freestanding binary | Links your program with **no libc and no C runtime**, runs it, and compares its output and exit code. |
| C interop | Builds your file as a no_std static library and links it into a real C program. |
| firmware | Cross-compiles for a Cortex-M3, inspects the vector table byte by byte, and boots it in QEMU. |

When a build fails, the checker adds a short **why?** note for the classic
no_std errors (missing panic handler, `std::` paths, `memcpy` undefined, …).

## Prerequisites

You should be comfortable with ordinary Rust: ownership and borrowing, structs
and enums, traits and generics, closures, iterators, `Option`/`Result` and
modules. If any of those feel shaky, read the matching chapters of *The Rust
Programming Language* first. Everything else, including `unsafe`, is taught here.

Toolchain:

- Rust **1.85 or newer** (the course uses edition 2024). `rustup update stable`.
- For module 12: `rustup target add thumbv7m-none-eabi`, and optionally QEMU
  (`qemu-system-arm`) to boot firmware.
- For module 11: a C compiler (`cc`, `gcc` or `clang`).
- Modules 09-10 run freestanding binaries natively on **Linux x86_64 or
  aarch64**. On macOS or Windows they are type-checked only, so use a Linux VM,
  container or WSL to run them for real.

Run `nostd doctor` to check all of this.

## Editor setup

Each exercise file is its own tiny crate, so `cargo` doesn't know about them.
Run `nostd lsp` once in your workspace: it writes a `rust-project.json` so that
rust-analyzer understands every exercise.

## The roadmap

| # | Module | You will build |
|---|---|---|
| 01 | The no_std landscape | your first `#![no_std]` crates |
| 02 | Data without a heap | slice/array/str algorithms, number formatting |
| 03 | Formatting without allocation | a fixed-capacity string, Display impls, a hexdump |
| 04 | Errors, panics & overflow | error enums, panic-free code, wrap-safe timers |
| 05 | Memory without a heap | compile-time tables, `StackVec`, ring buffer, object pool |
| 06 | The alloc crate | collections, an arena, a `GlobalAlloc` |
| 07 | Interior mutability & atomics | spinlock, `Once`, critical sections, lock-free SPSC queue |
| 08 | Unsafe & hardware registers | bit fields, volatile MMIO, packet layouts, type-state GPIO |
| 09 | Freestanding Linux binaries | `_start`, syscalls, `memcpy` & co, `println!`, a panic handler |
| 10 | Capstone: no_std CLI tools | `wc`, `grep`, an RPN calculator with no libc |
| 11 | Talking to C | exported C APIs, callbacks, calling C from Rust |
| 12 | Bare metal: targets & boot | startup code, a Cortex-M vector table booted in QEMU |
| 13 | Drivers & embedded-hal | generic drivers for GPIO, I2C and SPI devices |
| 14 | Interrupts & async | ISR event flags, `block_on`, a static async executor |
| 15 | Automotive I: CAN | CAN/CAN FD frames, J1939, DBC signals, ISO-TP |
| 16 | Automotive II: functional safety | E2E protection, watchdog supervision, fixed-point PID, mode manager |
| 17 | Portable crates & tooling | feature-gated `std`/`alloc`, strict lints, testing strategy |
| 18 | The ecosystem & next steps | crates, templates, interview preparation |

> **Toyota lens:** look for these callouts. They connect a topic to automotive
> practice: ISO 26262 functional safety, AUTOSAR, MISRA-style coding rules,
> ECUs and CAN. Modern vehicle software (including Toyota's and Woven by
> Toyota's) is a large mixed C/C++/Rust world, and Rust's no_std discipline
> maps directly onto the constraints safety standards impose: no dynamic
> memory, no undefined behaviour, bounded execution and explicit error handling.

## Tools beyond exercises

- `nostd where HashMap` tells you whether an item is in `core`, `alloc`, std
  only, or an ecosystem crate, and suggests no_std alternatives.
- `nostd drill` runs interview-style flashcards with spaced repetition.
- `nostd new <template> <name>` scaffolds real Cargo projects: a portable
  library, a freestanding Linux binary, and Cortex-M firmware with or without
  the ecosystem crates.

## How to study

1. Read each lesson fully once. Type out the code snippets you don't fully get.
2. Attempt every exercise **before** asking for hints. Compiler errors are the
   curriculum: the error messages you meet here are the ones you'll meet at work.
3. Hints come in stages, from a nudge to almost the answer. `nostd solution`
   only works once you've passed (or with `--force`). Compare your solution
   with the reference after passing; there's usually something to learn.
4. Take the quiz at the end of each module. 80% passes.
5. Five minutes of `nostd drill` a day will make interview answers automatic.

Run `nostd next` to start module 01.
