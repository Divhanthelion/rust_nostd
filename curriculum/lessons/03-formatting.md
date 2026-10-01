# Formatting Without Allocation

`println!("{}", x)` feels like one feature, but it's two: **formatting** (turn
values into text) and **output** (send the text somewhere). Only output needs
an operating system. All of formatting lives in `core::fmt` and never allocates.

## The architecture of core::fmt

```text
 write!(sink, "speed={} km/h", v)
        │
        ▼
 format_args!("speed={} km/h", v)  ──►  fmt::Arguments   (a recipe: pieces + values,
        │                                                 built on the stack, no heap)
        ▼
 sink.write_fmt(args)  ──► calls Display::fmt(&v, &mut Formatter)
        │                         │
        ▼                         ▼
 sink.write_str("speed=") ... sink.write_str("88") ... sink.write_str(" km/h")
```

- **`fmt::Arguments`** is a pre-parsed format string plus references to the
  arguments. `format_args!` builds it with zero allocation.
- **Formatting traits** (`Display`, `Debug`, `LowerHex`, `UpperHex`,
  `Binary`, `Octal`, `LowerExp`…) say *how* a value turns into text. Each has one
  method: `fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result`.
- **`fmt::Write`** is the *sink*: anything that can accept `&str` pieces.
  Implement one method and you get `write!`/`writeln!` for free:

```rust
pub trait Write {
    fn write_str(&mut self, s: &str) -> fmt::Result;          // required
    fn write_char(&mut self, c: char) -> fmt::Result { ... }  // provided
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result { ... } // provided
}
```

`String` implements `fmt::Write` (with alloc), but so can a fixed buffer, a
UART, a log ring buffer, a semihosting channel or a Linux file descriptor.

## Writing a sink

A UART sink usually just pushes bytes out:

```rust
use core::fmt;

pub struct Uart { /* registers */ }

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            self.send_byte(b);    // busy-wait until the TX register is free
        }
        Ok(())
    }
}

// now anywhere:  writeln!(uart, "rpm={} temp={}", rpm, temp).ok();
```

A **buffer** sink has to decide what happens when it fills up. Returning
`Err(fmt::Error)` aborts the rest of the `write!` call. `fmt::Error` carries
no information; it just means "the sink failed". When truncating text, cut
on a `char` boundary (`str::is_char_boundary`) so the buffer stays valid UTF-8.

## Implementing Display and Debug

```rust
use core::fmt;

pub struct Rpm(pub u16);

impl fmt::Display for Rpm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} rpm", self.0)
    }
}
```

`Formatter` carries the format spec the *caller* wrote: `{:>8}`, `{:.2}`,
`{:+}`, `{:#x}`, `{:08}`. Your impl can honour it:

| Spec | Query in your impl | Meaning |
|---|---|---|
| `{:8}` / `{:>8}` / `{:<8}` / `{:^8}` | `f.width()`, `f.align()` | minimum width, alignment |
| `{:*^8}` | `f.fill()` | fill character |
| `{:.3}` | `f.precision()` | precision (decimals for numbers, max chars for strings) |
| `{:+}` | `f.sign_plus()` | always show the sign |
| `{:#}` / `{:#?}` | `f.alternate()` | alternate form / pretty Debug |

Two helpers do the hard work:

- `f.pad(s)` writes `s` honouring width, alignment, fill **and** precision
  (truncation). Build your text in a small stack buffer, then `f.pad(text)`.
- `f.pad_integral(is_nonnegative, prefix, digits)` does the same for numbers
  (handles `+`, `#`, zero-padding).

For `Debug`, the builders produce the standard shapes, including pretty
printing with `{:#?}`:

```rust
impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame")
            .field("id", &format_args!("{:#05x}", self.id))  // custom field rendering
            .field("len", &self.len)
            .finish()
    }
}
```

Note the trick: `format_args!` produces a value that implements `Debug` and
`Display`, so you can control how a field looks without allocating.

## Integers in other bases

```rust
let id = 0x1A3u16;
write!(out, "{:x} {:X} {:#x} {:#06x} {:b} {:08b}", id, id, id, id, 5u8, 5u8)?;
// 1a3 1A3 0x1a3 0x01a3 101 00000101
```

## The cost of formatting

`core::fmt` is designed to keep *code* small: it uses dynamic dispatch
(`&mut dyn Write`) so it isn't monomorphised for every sink. Still, on
microcontrollers it's often the single largest library component:

- the first `write!` pulls in the formatting engine (several KiB);
- every `#[derive(Debug)]` adds code for its type;
- **floating-point formatting** is especially large (`{}` on an `f32` can cost
  10+ KiB because it implements exact shortest round-trip printing);
- **panic messages** use `fmt` too: a single `unwrap()` can drag it all in.

Alternatives used in practice:

- **`defmt`** ("deferred formatting"): the firmware sends only an index of the
  format string plus the raw argument bytes; the host decodes and formats.
  Tiny, fast, and the de-facto logging standard with `probe-rs`.
- **`ufmt`**: a smaller re-implementation of `fmt` with fewer features.
- **hand-written conversions** like your `itoa1`, and fixed-point integers
  formatted as decimals instead of floats.

Measure with `cargo size` (cargo-binutils) or `cargo bloat`.

> **Toyota lens:** production ECUs rarely log free text. Faults are reported as
> **Diagnostic Trouble Codes** (DTCs, e.g. `P0301`) plus freeze-frame data, read
> by a tester over UDS (ISO 14229). Text formatting appears in development
> builds, tooling, and Linux-class ECUs (infotainment, gateways). Knowing how to
> keep formatting *out* of a binary is as valuable as using it.

> **Interview:** "How do you print debug output on a microcontroller without
> std?" Implement `core::fmt::Write` for a UART (or use RTT/semihosting), then use
> `write!`. For production-quality logging, `defmt` over RTT.
