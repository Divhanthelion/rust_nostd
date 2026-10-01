# Bare Metal: Targets, Linkers & Boot

On a microcontroller there's no kernel to `exec` your program. The CPU
comes out of reset and starts executing whatever the hardware points it at.
Everything from that first instruction to `main` is your responsibility (or
your runtime crate's). This module walks the whole path on an Arm Cortex-M,
the most common MCU core in the world and in cars.

## Targets and toolchains

```console
$ rustup target add thumbv7m-none-eabi     # Cortex-M3
$ cargo build --target thumbv7m-none-eabi
```

| Core | Target | Notes |
|---|---|---|
| Cortex-M0/M0+ | `thumbv6m-none-eabi` | no CAS atomics, no hardware divide |
| Cortex-M3 | `thumbv7m-none-eabi` | |
| Cortex-M4/M7 | `thumbv7em-none-eabi` / `-eabihf` | DSP instructions; `hf` = hardware FPU calling convention |
| Cortex-M23/M33 | `thumbv8m.base-none-eabi` / `thumbv8m.main-none-eabihf` | TrustZone-M |
| Cortex-R4/R5/R52 | `armv7r-none-eabihf`, `armv8r-none-eabihf` | lock-step safety cores (Tier 2) |
| RISC-V MCUs | `riscv32imc-unknown-none-elf`, `riscv32imac-…` | ESP32-C3, GD32V… |

Pin the target in `.cargo/config.toml` so `cargo build` cross-compiles by
default, and set a **runner** so `cargo run` flashes or emulates:

```toml
[build]
target = "thumbv7m-none-eabi"

[target.thumbv7m-none-eabi]
runner = "probe-rs run --chip STM32F103C8"     # or QEMU, see below
```

## The Cortex-M boot sequence

On reset, a Cortex-M does just two memory reads before running code:

1. word 0 of the **vector table** → initial stack pointer (MSP);
2. word 1 → the **reset vector**: the address of the reset handler, which it
   jumps to.

The vector table sits at address 0 (or wherever `VTOR` points, often flash at
`0x0800_0000` on STM32). Its first 16 words are architectural:

| Word | Contents | Word | Contents |
|---|---|---|---|
| 0 | initial SP | 8-10 | reserved |
| 1 | Reset | 11 | SVCall |
| 2 | NMI | 12 | DebugMonitor |
| 3 | HardFault | 13 | reserved |
| 4 | MemManage | 14 | PendSV |
| 5 | BusFault | 15 | SysTick |
| 6 | UsageFault | 16+ | device interrupts (IRQ0, IRQ1, … from the datasheet) |
| 7 | reserved | | |

Every handler address must have **bit 0 set** (the Thumb bit): Cortex-M only
executes Thumb code, and a vector with bit 0 clear faults immediately. The
linker sets it for you when you refer to a function, which is one reason to
build the table from function pointers rather than integers.

## What the reset handler must do

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Reset() -> ! {
    // 1. .bss: zero-initialised statics. RAM powers up with garbage.
    // 2. .data: initialised statics. Their initial values live in flash and
    //    must be copied into RAM.
    // 3. (Cortex-M4F/M7F) enable the FPU in CPACR before any float code runs.
    // 4. (ECC RAM) on MCUs with ECC-protected RAM, write all of RAM once, since
    //    reading uninitialised ECC memory raises a fault.
    // 5. call main, which never returns.
    main()
}
```

Until steps 1-2 are done, every `static` in your program has the wrong
value, and the Rust code running in that window must not depend on any.
This is why `cortex-m-rt` does them **in assembly** before any Rust code
runs. Strictly, Rust code that executes before statics are initialised is
outside the language's guarantees. In the exercise you'll write them in Rust,
carefully, with volatile writes, to see exactly what happens.

## Linker scripts: where everything goes

The linker script tells the linker the memory map and where each section
goes. The key idea is that one section can have two addresses: where it
**lives** (LMA, load address, in flash) and where it **runs** (VMA, in RAM).

```ld
MEMORY {
  FLASH (rx)  : ORIGIN = 0x00000000, LENGTH = 256K
  RAM   (rwx) : ORIGIN = 0x20000000, LENGTH = 64K
}
ENTRY(Reset);
_stack_start = ORIGIN(RAM) + LENGTH(RAM);       /* stack grows down from the top */

SECTIONS {
  .vector_table ORIGIN(FLASH) : {
    LONG(_stack_start);                          /* word 0 */
    KEEP(*(.vector_table.reset_vector));         /* word 1: KEEP survives --gc-sections */
    KEEP(*(.vector_table.exceptions));           /* words 2..15 */
  } > FLASH
  .text   : { *(.text .text.*); } > FLASH
  .rodata : { *(.rodata .rodata.*); } > FLASH
  .data : { _sdata = .; *(.data .data.*); _edata = .; } > RAM AT > FLASH   /* VMA in RAM, LMA in flash */
  _sidata = LOADADDR(.data);                      /* where .data's initial values are stored */
  .bss (NOLOAD) : { _sbss = .; *(.bss .bss.*); _ebss = .; } > RAM
  /DISCARD/ : { *(.ARM.exidx*); }                /* no unwinding, no unwind tables */
}
```

Symbols defined in the script are visible to Rust as `extern` statics whose
*addresses* are the values:

```rust
unsafe extern "C" { static mut _sbss: u32; static mut _ebss: u32; }
let start: *mut u32 = &raw mut _sbss;          // never read `_sbss` itself
```

Place Rust items into the table with `#[unsafe(link_section = "...")]`:

```rust
#[unsafe(link_section = ".vector_table.reset_vector")]
#[unsafe(no_mangle)]
pub static __RESET_VECTOR: unsafe extern "C" fn() -> ! = Reset;
```

With `cortex-m-rt` you only write `memory.x` (the `MEMORY` block); the crate
ships the rest of the script (`link.x`) and generates the vector table from
`#[entry]`, `#[exception]` and the device crate's interrupt list (`device.x`).

## Seeing output: semihosting, RTT, defmt

- **Semihosting**: the program executes `bkpt 0xAB`; an attached debugger or
  QEMU performs host I/O on its behalf (print, read files, exit). Slow, and
  **it hangs or faults without a debugger**: development only.
- **RTT** (Real-Time Transfer): ring buffers in RAM that the debug probe reads
  in the background. Fast, doesn't halt the CPU.
- **defmt** over RTT: compact deferred logging (module 03). The standard with
  `probe-rs`.

QEMU emulates a few Cortex-M boards. The TI LM3S6965 is a Cortex-M3 with 256 KiB
flash at 0x0 and 64 KiB RAM at 0x2000_0000:

```console
$ qemu-system-arm -cpu cortex-m3 -machine lm3s6965evb -nographic \
    -semihosting-config enable=on,target=native -kernel firmware.elf
```

On real hardware: `probe-rs run --chip <CHIP> firmware.elf` flashes, resets
and streams RTT/defmt output; `probe-rs gdb` or VS Code debugging work too.

## When things go wrong: HardFault

Invalid memory access, an undefined instruction, a vector without the Thumb
bit or an unaligned access all end in **HardFault** (or the configurable
MemManage/BusFault/UsageFault handlers). On exception entry the CPU pushes
an **exception frame** onto the stack: `r0, r1, r2, r3, r12, lr, pc, xPSR`.
The stacked `pc` is the faulting instruction. The **CFSR** register
(`0xE000_ED28`) says why, bit by bit (`UNDEFINSTR`, `INVSTATE`, `PRECISERR`,
`DACCVIOL`, `DIVBYZERO`…). Decoding both is the first thing to do with a fault
in the field, and the `boot1` exercise has you write those decoders.

## Other architectures, same ideas

- **RISC-V**: no vector table of addresses by default. `mtvec` points at a trap
  handler, and `riscv-rt` plays cortex-m-rt's role.
- **Cortex-R** (safety MCUs, often in dual-core lock-step): Arm-mode exception
  vectors are *instructions*, not addresses; caches and MPU setup belong to
  boot code too.
- **AArch64 / Cortex-A** bare metal: exception levels (EL3→EL1), MMU setup
  before Rust code can use normal memory semantics.

> **Toyota lens:** startup code is part of the safety case. It is the first
> software that runs, it initialises ECC RAM, sets up the MPU that enforces
> freedom from interference, configures clocks and watchdogs, and may run
> start-up self-tests (RAM/ROM/CPU tests in the style of IEC 60730 or ISO 26262
> latent-fault checks) before the application starts. Many automotive MCUs (e.g.
> NXP S32K3, Infineon AURIX) run cores in **lock-step** and have ECC everywhere,
> so knowing why RAM must be written before it's read is real, practical knowledge.

> **Interview:** "Walk me through what happens from power-on to main on a
> Cortex-M" is the single most common embedded interview question. After
> this module you can answer it from first principles: vector table → SP and
> reset vector → .bss/.data init → main, plus where the linker script defines
> each address.
