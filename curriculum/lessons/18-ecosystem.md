# The Ecosystem & Your Next Steps

You've built, by hand, most of what the no_std ecosystem provides:
fixed-capacity collections, a global allocator, spinlocks, critical sections,
lock-free queues, a runtime crate, a vector table, drivers, executors, CAN
codecs and safety mechanisms. This last lesson maps that knowledge onto the
crates you'll use at work, and gives you a plan for what to do next.

## The crate map

Versions as of late 2026; always check crates.io.

| You built | The ecosystem crate | Notes |
|---|---|---|
| `StackVec`, `FixedString`, `RingBuffer`, `spsc::Queue` | **heapless** 0.9 | `Vec<T, N>`, `String<N>`, `Deque`, `IndexMap`, `spsc`. `push` returns `Err(item)` when full |
| `cs1` Mutex + token | **critical-section** 1.2 | `critical_section::with(\|cs\| ..)`; the binary picks the implementation (e.g. `cortex-m`'s `critical-section-single-core` feature) |
| atomics on thumbv6m | **portable-atomic** 1.x | CAS emulation via critical sections |
| `once1` / init-once statics | **static_cell** 2.1 | `StaticCell::init` gives a `&'static mut T` once (common with Embassy) |
| `alloc2` | **embedded-alloc** 0.7 | `LlffHeap` (linked list) or `TlsfHeap` (bounded time) |
| `cortexm1` vector table + reset | **cortex-m-rt** 0.7 | `#[entry]`, `#[exception]`, `memory.x` |
| SysTick/NVIC access | **cortex-m** 0.7 | core peripherals, `asm::wfi`, `interrupt::free` |
| `hal1..3` traits | **embedded-hal** 1.0, **embedded-hal-async** 1.0, **embedded-hal-bus** 0.3, **embedded-io** 0.7 | the driver contract |
| `async2` executor | **embassy-executor** 0.10, **embassy-time** 0.5, **embassy-sync** 0.8 | async HALs for STM32, nRF, RP2040/RP2350, ESP32 |
| `isr1` priorities | **RTIC** 2.3 | priority-ceiling resources, checked at compile time |
| `print1`, hexdump | **defmt** 1.x + **probe-rs** 0.32 | deferred logging over RTT; flashing and debugging |
| `fixed1` | **fixed**, **libm** 0.2 | fixed-point types; pure-Rust float math for no_std |
| `can1` | **embedded-can** 0.4, **socketcan** 4.0, bxcan/fdcan | CAN traits, Linux SocketCAN, STM32 CAN peripherals |
| `nostd_rt` | **origin**/**eyra** (Linux), **riscv-rt**, **cortex-ar** | runtimes for other platforms |

Also worth knowing: `bitflags`, `zerocopy`/`bytemuck` (safe byte↔struct
conversions), `nom`/`winnow` (no_std parser combinators), `crc`, `postcard`
(compact serde for embedded), `serde` with `default-features = false`, `smoltcp`
(TCP/IP), `littlefs2` (flash filesystem), `embedded-storage`, `rtt-target`,
`panic-probe`, `flip-link` (stack overflow protection), `cargo-binutils`.

## Starting real projects

The tool can scaffold four starting points that use the ideas from this
course:

```console
$ nostd new lib my-codec           # portable no_std library, feature flags, strict lints, cross-target CI
$ nostd new linux-bin my-tool      # freestanding Linux program with its own runtime (9 KB)
$ nostd new cortex-m my-fw         # zero-dependency firmware: vector table, reset, SysTick, QEMU
$ nostd new cortex-m-rt my-fw2     # the same firmware on cortex-m-rt + semihosting
```

Project ideas that build a portfolio, roughly in increasing difficulty:

1. **A DBC-driven CAN decoder crate** (no_std, `nom` or hand-written), with
   property tests and fuzzing; use it in a Linux `socketcan` logger.
2. **An ISO-TP + UDS server** (ReadDataByIdentifier, DiagnosticSessionControl)
   in no_std, tested against your `isotp1` sender on a `vcan0` bus.
3. **Embassy firmware** on an STM32 Nucleo or RP2040 board: read a sensor over
   I2C (your `hal2`-style driver), publish values on CAN through an MCP2515
   (`hal3`), log with defmt.
4. **A safety island**: an E2E-protected heartbeat between two boards, a
   watchdog manager supervising tasks, and a mode manager that enters a safe
   state, with fault injection tests.
5. **Contribute** to an embedded-hal driver or HAL: real code review is the
   fastest teacher.

## Learning resources

- *The Embedded Rust Book*, *The Embedonomicon* (how runtimes and linker
  scripts work, which you've now done), *Discovery* (hands-on boards).
- *The Rustonomicon* (unsafe Rust), and Miri's documentation.
- The Embassy book, the RTIC book, probe-rs docs.
- The Ferrocene Language Specification (adopted by the Rust project in 2025)
  and the Safety-Critical Rust Consortium's coding guidelines.
- Google's *Comprehensive Rust* includes bare-metal and concurrency days.

## Preparing for a Toyota / automotive Rust interview

From public postings by Woven by Toyota and Toyota Connected, and from what
automotive teams look for, expect a mix of:

- **Rust fundamentals under pressure**: ownership in concurrent code, `Send`/
  `Sync`, trait design, error handling, `unsafe` soundness arguments.
- **Embedded fundamentals**: boot sequence, interrupts and priorities,
  volatile/MMIO, memory sections, stack usage, DMA, watchdogs.
- **Automotive protocols**: CAN/CAN FD frames and arbitration, signal
  encoding, ISO-TP/UDS, SOME/IP and Ethernet on bigger ECUs, J1939 for trucks.
- **Safety and security literacy**: ISO 26262 vocabulary (ASIL, safe state,
  FTTI, freedom from interference), E2E protection, MISRA-style rules and how
  Rust addresses them, ISO/SAE 21434 basics.
- **C/C++ integration**: most production code there is still C/C++ (and Linux,
  QNX, AUTOSAR). FFI skills (module 11) are a differentiator.
- **Linux** for gateway/infotainment/ADAS roles: SocketCAN, systemd,
  cross-compilation, Yocto/Buildroot basics.

Use `nostd drill` daily: the flashcards cover exactly these topics. When you
answer, practise structure: *what*, *why*, *trade-off*, *example from your
own code*. You now have a codebase of examples from this course to draw on.

> **Toyota lens:** Woven by Toyota is a founding member of the Safety-Critical
> Rust Consortium, and Toyota's software-defined-vehicle platform (Arene) is
> shipping in production vehicles. Rust is still a minority skill in most
> postings next to C/C++. That's an opportunity: someone who can write
> panic-free, allocation-free, well-tested no_std Rust *and* integrate it with
> existing C, CAN and safety processes is rare. That's what this course trained.

Congratulations on finishing the material. Take the final quiz with
`nostd quiz 18`, then keep the habit with `nostd drill`.
