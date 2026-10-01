# Errors, Panics & Overflow

Firmware can't print a stack trace and exit. A brake controller can't "crash
and restart" at an arbitrary moment. So this module is about one discipline:
**every failure must be expected, represented, and handled on purpose.**

## Errors as values

`Result<T, E>` and `?` work exactly as in std. What changes is the error type.
`Box<dyn Error>` needs alloc, and `anyhow`-style catch-alls aren't
appropriate where you must react to each failure differently. no_std code
uses **concrete error enums**:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    TooShort { needed: usize, got: usize },
    BadStart(u8),
    BadChecksum { expected: u8, actual: u8 },
}
```

They are small (`Copy`, a few bytes), carry exactly the data a handler needs,
and `match` forces you to consider each case.

### Layering with `From` and `?`

Higher layers wrap lower-layer errors. Implement `From`, and `?` converts
automatically:

```rust
#[derive(Debug, PartialEq)]
pub enum ConfigError {
    Frame(FrameError),
    BadValue(core::num::ParseIntError),
    Missing(&'static str),
}

impl From<FrameError> for ConfigError {
    fn from(e: FrameError) -> Self { ConfigError::Frame(e) }
}

fn load(raw: &[u8]) -> Result<Config, ConfigError> {
    let frame = parse_frame(raw)?;           // FrameError → ConfigError
    ...
}
```

### Display and core::error::Error

Since Rust 1.81 the `Error` trait lives in `core`, so no_std libraries can
implement it and interoperate with std users:

```rust
impl core::fmt::Display for FrameError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FrameError::TooShort { needed, got } => write!(f, "frame too short: need {needed} bytes, got {got}"),
            FrameError::BadStart(b) => write!(f, "bad start byte {b:#04x}"),
            FrameError::BadChecksum { expected, actual } =>
                write!(f, "checksum mismatch: expected {expected:#04x}, got {actual:#04x}"),
        }
    }
}

impl core::error::Error for FrameError {}

impl core::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            ConfigError::Frame(e) => Some(e),     // the error chain
            ConfigError::BadValue(e) => Some(e),
            ConfigError::Missing(_) => None,
        }
    }
}
```

Design tips:

- Mark public error enums `#[non_exhaustive]` if you may add variants later.
- Don't stringify: keep data structured (`{ expected, actual }`), so callers
  can log, count or react without parsing text.
- Keep errors `Copy` where possible: they get passed through ISR queues and stored
  in fault logs.

## Panics in no_std

A panic means "a bug was detected; continuing is unsafe". In std it
unwinds the stack and prints a message. In no_std, **you decide** by defining
exactly one panic handler in the final binary:

```rust
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // info.location() -> Option<&Location>: file, line, column
    // info.message()  -> PanicMessage: implements Display
    loop {}   // must never return: the type is `-> !`
}
```

Common policies:

| Policy | Typical crate | Use |
|---|---|---|
| halt in a loop | `panic-halt` | simplest; a watchdog will reset the chip |
| breakpoint / report to debugger | `panic-probe`, `panic-semihosting` | development |
| log to persistent RAM, then reset | custom | field diagnostics |
| enter a **safe state**, then reset | custom | safety-relevant systems |

no_std binaries use `panic = "abort"` in their Cargo profiles: there's no
unwinder, so drop glue doesn't run on panic. The handler is the last code
that executes.

> **Warning:** a panic handler that can itself panic (e.g. formatting into a
> full buffer with `unwrap()`) recurses. Keep handlers trivially simple.

## Where panics hide

Each of these can panic at runtime:

| Code | Panics when | Panic-free alternative |
|---|---|---|
| `v[i]`, `&s[a..b]` | out of range | `v.get(i)`, `s.get(a..b)`, iterators, `split_at_checked` |
| `x.unwrap()`, `x.expect(..)` | `None`/`Err` | `?`, `match`, `unwrap_or`, `let else` |
| `a / b`, `a % b` | `b == 0` (and `MIN / -1`) | `checked_div`, `checked_rem` |
| `a + b` etc. | overflow (with overflow checks on) | `checked_*`, `wrapping_*`, `saturating_*` |
| `RefCell::borrow_mut` | already borrowed | `try_borrow_mut` |
| `slice.copy_from_slice(src)` | lengths differ | check first, or slice both to `min(len)` |
| `chunks(0)`, `windows(0)` | zero size | guard |

A practical technique for *proving* a function never panics: compile it
into a binary with a panic handler that references a symbol that doesn't exist.
If any panic path survives optimisation, the **link fails**. That's the idea
behind the `panic-never` and `no-panic` crates.

## Integer overflow: choose your semantics

Rust's `+` on integers has two behaviours:

- with **overflow checks on** (debug builds by default): panic;
- with them **off** (release by default): wrap around silently, two's complement.

Neither is what you want by accident. Safety-relevant code enables
`overflow-checks = true` in release *and* states the intended behaviour
explicitly:

| Method | `250u8 + 10` | Use when |
|---|---|---|
| `checked_add` | `None` | overflow is an error to handle |
| `wrapping_add` | `4` | modular arithmetic: counters, checksums, sequence numbers |
| `saturating_add` | `255` | physical quantities that should clamp: PWM duty, sensor scaling |
| `overflowing_add` | `(4, true)` | you need both the result and the carry |

`core::num::Wrapping<T>` and `Saturating<T>` make the policy part of the type.

### Casts truncate silently

`as` never fails: `300i32 as u8 == 44`, `-1i32 as u32 == 4294967295`,
`f32::NAN as u8 == 0`. Prefer `u8::try_from(x)` (returns `Result`) or the
lossless `From`/`Into` conversions (`u32::from(x_u16)`) that only exist where
no information can be lost.

### The timer wraparound bug

A 32-bit millisecond tick counter wraps after ~49.7 days. This classic
bug has hit real aircraft and cars:

```rust
// WRONG: breaks when `now` wraps past u32::MAX and `start` hasn't
if now - start >= timeout { ... }          // panics in debug, and...
if now >= start + timeout { ... }          // ...this is wrong near the wrap

// RIGHT: modular difference is correct across one wrap
if now.wrapping_sub(start) >= timeout { ... }
```

`now.wrapping_sub(start)` is the elapsed time modulo 2³², which is correct as
long as the true elapsed time is less than 2³² ticks. To ask "is time `a`
before time `b`?" on a wrapping clock, look at the sign of the wrapped
difference: `(b.wrapping_sub(a) as i32) > 0` (valid for differences under 2³¹).

> **Toyota lens:** ISO 26262-6 asks for defensive implementation techniques and
> robust handling of erroneous inputs; MISRA C has whole sections on
> conversions (Rule 10.x) and on checking error information (Dir 4.7). In
> Rust, typed errors, `TryFrom` and explicit overflow methods implement those
> rules in the type system. Expect to justify every `unwrap()` in a code review,
> and to have a defined, tested panic strategy (safe state + reset) for the rest.

> **Interview:** "What happens on integer overflow in Rust?" Panic in debug,
> wrap in release, unless `overflow-checks` says otherwise. Then: "so which
> should firmware use?" Explicit methods that encode intent, plus overflow
> checks enabled in release for safety-relevant code.
