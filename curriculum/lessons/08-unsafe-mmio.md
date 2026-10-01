# Unsafe Rust, Raw Pointers & Hardware Registers

At the bottom of every embedded stack, software talks to hardware by reading
and writing magic addresses. That needs `unsafe`. The skill this module
teaches isn't "using unsafe"; it's **building safe abstractions** so that
`unsafe` appears in a few small, audited places and everything above them is
checked by the compiler.

## What `unsafe` unlocks

An `unsafe` block lets you do exactly five extra things:

1. dereference a raw pointer (`*const T`, `*mut T`);
2. call an `unsafe fn` (including foreign functions);
3. access a `static mut`;
4. implement an `unsafe trait` (`Send`, `Sync`, `GlobalAlloc`…);
5. access fields of a `union`.

Everything else (borrow checking, type checking) still applies. `unsafe`
means "I have verified the conditions the compiler can't". The conditions are
the **safety contract**, and a mismatch is **undefined behaviour (UB)**: the
optimiser may then do anything.

## The contract discipline

- An `unsafe fn` documents its contract in a `# Safety` section.
- Every `unsafe { }` block carries a `// SAFETY:` comment arguing why the
  contract holds *here*.
- A safe function that contains unsafe code must be sound for **every** input
  a caller could pass. If it can't be, it must be an `unsafe fn` itself.

```rust
/// Read a little-endian u32 at `offset`.
pub fn read_u32_le(buf: &[u8], offset: usize) -> Option<u32> {
    let bytes = buf.get(offset..offset.checked_add(4)?)?;     // bounds proven here
    // SAFETY: `bytes` has exactly 4 readable bytes; read_unaligned has no
    // alignment requirement.
    Some(u32::from_le(unsafe { bytes.as_ptr().cast::<u32>().read_unaligned() }))
}
```

Edition 2024 enforces this granularity: inside an `unsafe fn`, each unsafe
operation still needs its own `unsafe { }` block (`unsafe_op_in_unsafe_fn`).

## Undefined behaviour to know by heart

- dereferencing a null, dangling, or misaligned pointer;
- reading uninitialised memory as a value;
- creating two `&mut` to the same data, or `&mut` while a `&` is live (aliasing);
- producing an invalid value: a `bool` that isn't 0/1, an enum with an
  undefined discriminant, a `char` above `0x10FFFF`, a null reference;
- data races (unsynchronised concurrent access with at least one write);
- breaking a `str`'s UTF-8 invariant;
- calling a function through a pointer of the wrong signature/ABI.

Tooling: **Miri** (`cargo +nightly miri test`) interprets your tests and detects
most of these. Run it on any crate with non-trivial unsafe code.

## Raw pointers

```rust
let mut x = 10u32;
let p: *mut u32 = &raw mut x;           // no reference created (edition 2024 style)
unsafe {
    p.write(11);                        // *p = 11
    let y = p.read();
    let q = p.add(1);                   // pointer arithmetic, in units of T
}
let bytes: *const u8 = p.cast::<u8>().cast_const();
assert!(p.is_aligned());
```

- `p.add(n)` must stay within the same allocation (or one past its end).
- `read_unaligned`/`write_unaligned` for packed or wire-format data.
- `core::ptr::copy_nonoverlapping` = `memcpy`; `core::ptr::copy` = `memmove`.
- `core::slice::from_raw_parts(ptr, len)` builds a slice; you vouch for validity.

## Memory-mapped I/O and volatile

Peripherals expose **registers** at fixed physical addresses. Writing
`0x0000_0020` to address `0x4002_0018` might switch on an LED. Two properties
make registers unlike normal memory:

- **accesses have side effects**: reading a UART data register pops a byte;
  writing a "set" register changes pins;
- **values change on their own**: a status register reflects hardware state.

The optimiser assumes memory only changes when your code changes it. So it
may merge two writes into one, drop a read whose result looks unused, or move
accesses around. For registers that's a bug. **Volatile** accesses tell the
compiler "perform exactly this access, now, in program order":

```rust
use core::ptr::{read_volatile, write_volatile};

const GPIOA_ODR: *mut u32 = 0x4002_0014 as *mut u32;

unsafe {
    let v = read_volatile(GPIOA_ODR);
    write_volatile(GPIOA_ODR, v | (1 << 5));   // read-modify-write
}
```

Volatile is **not** atomic, and it is **not** a memory barrier for other data.
A read-modify-write like the one above can lose an update if an interrupt
modifies the same register in between. That's why many MCUs provide
*set/clear* registers (STM32's `BSRR`: writing bit n sets pin n, writing bit
n+16 clears it), which turn an RMW into a single write.

### Register blocks

Registers come in blocks at fixed offsets from a base address, described in
the chip's reference manual (and machine-readably in its **SVD** file):

```rust
#[repr(C)]
pub struct GpioRegisters {
    pub moder: u32,   // 0x00 mode register
    pub otyper: u32,  // 0x04
    pub ospeedr: u32, // 0x08
    pub pupdr: u32,   // 0x0C
    pub idr: u32,     // 0x10 input data
    pub odr: u32,     // 0x14 output data
    pub bsrr: u32,    // 0x18 bit set/reset (write-only)
}
const _: () = assert!(core::mem::offset_of!(GpioRegisters, bsrr) == 0x18);
```

Wrap the base pointer in a type whose methods do volatile accesses to
`&raw mut (*regs).field`. (Avoid creating `&`/`&mut` references to MMIO
memory: references let the compiler insert extra reads.)

### PAC → HAL → driver → application

| Layer | Example crates | Provides |
|---|---|---|
| PAC (peripheral access crate) | `stm32f4`, generated by `svd2rust` from SVD | typed registers and bit fields: `gpioa.odr().modify(\|_, w\| w.odr5().set_bit())` |
| HAL | `stm32f4xx-hal`, `embassy-stm32`, `nrf-hal` | safe, chip-specific APIs: `pin.into_push_pull_output().set_high()` |
| driver | `bme280`, `ssd1306`, `mcp2515` | chip-independent drivers written against `embedded-hal` traits |
| BSP / application | board crates, your firmware | glue and logic |

## Type-state: encode hardware state in types

A GPIO pin can be input, output or analog. Calling `set_high` on an input is
a bug the compiler can catch, if the mode is a type parameter:

```rust
pub struct Pin<const N: u8, MODE> { _mode: PhantomData<MODE> }
pub struct Input;
pub struct Output;

impl<const N: u8> Pin<N, Input> {
    pub fn into_output(self) -> Pin<N, Output> { /* configure MODER */ Pin { _mode: PhantomData } }
}
impl<const N: u8> Pin<N, Output> {
    pub fn set_high(&mut self) { /* write BSRR */ }
}
```

The pin is a **zero-sized type**: the type-state costs nothing at runtime.
Conversions take `self` by value, so the old state can't be used afterwards.
Combined with a `take()`-once singleton for the peripherals, this guarantees
that one owner per peripheral exists and every operation is legal for the
current configuration.

## repr and layout control

| Attribute | Effect | Use |
|---|---|---|
| `#[repr(C)]` | C layout: declaration order, C padding | FFI, register blocks, DMA descriptors |
| `#[repr(transparent)]` | same layout as the single non-ZST field | newtypes passed across FFI |
| `#[repr(u8)]` (on enums) | fixed discriminant size and values | protocol enums, register fields |
| `#[repr(packed)]` | no padding, alignment 1 | wire formats. References to fields are unaligned, so use `read_unaligned` or copy fields out |
| `#[repr(align(N))]` | raise alignment | DMA buffers, cache-line alignment |

Converting raw bytes into an enum must check the value (`TryFrom<u8>`):
transmuting an unknown discriminant is UB.

> **Toyota lens:** in AUTOSAR Classic the lowest software layer is the **MCAL**
> (microcontroller abstraction layer): drivers for ports, ADC, CAN and so on.
> The PAC/HAL split is Rust's version of it. MISRA C restricts pointer
> arithmetic and casts between pointers and integers (Rules 11.x, 18.x), and
> ISO 26262 asks for "limited use of pointers". Rust's answer is to confine
> such operations to small `unsafe` modules with documented contracts, and expose
> type-safe APIs, which also makes the safety argument reviewable.

> **Interview:** "Why do you need volatile for MMIO?", "is volatile the same as
> atomic?" (no), "what is type-state?", and "walk me through making this unsafe
> function sound" are standard embedded-Rust questions.
