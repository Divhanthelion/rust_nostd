# nostd: learn `#![no_std]` Rust from the command line

`nostd` is a single, dependency-free CLI that teaches you to write Rust without
the standard library, from `core` basics through freestanding Linux programs,
bare-metal Cortex-M firmware, embedded-hal drivers, async executors, and
automotive patterns (CAN, ISO-TP, E2E protection, watchdog supervision).

It works like rustlings, with one difference: exercises are checked
against a **core-only sysroot**, so `std` is physically absent. You can't pass
by accident with a `Vec` or `println!`.

- **19 modules, 61 exercises**, each with progressive hints and a reference solution
- **Lessons** rendered in the terminal (with "Toyota lens" and interview callouts)
- **Quizzes** per module (pass mark 80%)
- **86 interview flashcards** with spaced repetition (`nostd drill`)
- **`nostd where ITEM`**: is it in core, alloc, std, or a crate? What to use instead?
- **`nostd new`** project templates: portable lib, freestanding Linux binary, Cortex-M firmware (bare and cortex-m-rt)

## Install

```console
$ cargo install --path .
$ nostd doctor            # check rustc, targets, C compiler, QEMU
$ nostd init              # create ./nostd-workspace
$ cd nostd-workspace
$ nostd next              # start: the next lesson, exercise or quiz
```

Typical loop: `nostd learn` → `nostd watch` (re-checks on every save) →
`nostd hint` when stuck → `nostd quiz` → `nostd drill` daily.

## Requirements

| Needed for | Requirement |
|---|---|
| everything | Rust 1.85+ (edition 2024) via rustup |
| modules 09-10 (freestanding binaries) | Linux on x86_64 or aarch64 (elsewhere they're type-checked only) |
| module 11 (FFI) | a C compiler (`cc`) |
| module 12 (bare metal) | `rustup target add thumbv7m-none-eabi`; `qemu-system-arm` to actually boot the firmware |
| `--target` cross-checks | any extra bare-metal target, e.g. `thumbv6m-none-eabi` |

Modules whose tools are missing are reported as *blocked*, not failed.
`nostd lsp` writes a `rust-project.json` so rust-analyzer understands the
workspace.

## Curriculum

| # | Module | Exercises | Covers |
|---|---|---|---|
| 00 | Welcome | - | how the course and its checker work |
| 01 | The no_std Landscape | 3 | core, alloc and std; what `#![no_std]` really changes |
| 02 | Data Without a Heap | 4 | arrays, const generics, slices, iterators, zero-copy text |
| 03 | Formatting Without Allocation | 3 | `core::fmt`, `fmt::Write` sinks, Display/Debug, formatting cost |
| 04 | Errors, Panics & Overflow | 3 | error enums, panic handlers, panic-free code, overflow semantics |
| 05 | Memory Without a Heap | 4 | const vs static, `MaybeUninit`, ring buffers, pools |
| 06 | The alloc Crate & Global Allocators | 4 | collections in no_std, arenas, `GlobalAlloc`, `Layout` |
| 07 | Interior Mutability, Atomics & Sync | 6 | Send/Sync, cells, orderings, spinlocks, critical sections, SPSC |
| 08 | Unsafe Rust & Hardware Registers | 5 | safety contracts, volatile MMIO, `repr`, type-state APIs |
| 09 | Freestanding Linux Binaries | 6 | `_start`, syscalls via `asm!`, memcpy & co, panic handlers, no libc |
| 10 | Capstone: no_std CLI Tools | 3 | `wc`, `grep` and an RPN calculator on your own runtime |
| 11 | Talking to C (FFI) | 3 | exported C APIs, opaque handles, callbacks, calling C |
| 12 | Bare Metal: Targets, Linkers & Boot | 2 | vector tables, linker scripts, reset handlers, QEMU |
| 13 | Drivers & embedded-hal | 3 | generic drivers over `OutputPin`, `I2c`, `SpiDevice`, mocks |
| 14 | Interrupts & Async | 3 | NVIC priorities, ISR patterns, RTIC ideas, wakers, executors |
| 15 | Automotive I: CAN, Signals & ISO-TP | 3 | CAN/CAN FD, J1939, DBC Intel/Motorola signals, ISO-TP |
| 16 | Automotive II: Functional Safety | 4 | ISO 26262, E2E protection, watchdog manager, fixed-point PID, modes |
| 17 | Portable Crates, Lints & Tooling | 2 | feature gating, strict lints, testing strategy, size, toolchains |
| 18 | The Ecosystem & Next Steps | - | crate map, templates, projects, interview preparation |

## How exercises are checked

| Kind | How |
|---|---|
| library | built against a core-only sysroot (so `std` can't sneak in), then its tests run on the host |
| feature matrix | the same, once per feature set (`[]`, `alloc`, `alloc + std`) |
| freestanding binary | linked with `-nostartfiles -static`, run with test inputs; stdout, stderr and exit code compared |
| C library | built as a staticlib and linked into a C harness with `cc` |
| Cortex-M firmware | built for `thumbv7m-none-eabi`, the ELF's vector table inspected, then booted in QEMU |

Progress lives in `.nostd/state.txt` inside the workspace.

## For maintainers

The curriculum is embedded at compile time from `curriculum/`, so rebuild
before verifying:

```console
$ cargo build && cargo test
$ ./target/debug/nostd dev verify -j 8     # every solution passes, every untouched exercise fails,
                                           # quizzes, flashcards and the `where` table are valid
$ ./target/debug/nostd dev verify can1 15  # a subset: exercise names or module numbers
$ ./target/debug/nostd dev verify --strict # also fail if a tool is missing (CI)
```
