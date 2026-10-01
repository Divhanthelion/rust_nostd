# Data Without a Heap

In std Rust, the reflex for "a bunch of things" is `Vec`, and for text it's
`String`. Both live on the heap and grow as needed. In no_std the reflexes
change:

- **sizes are known at compile time, or bounded**: arrays `[T; N]`, const generics;
- **the caller provides the storage**: functions take `&mut [T]` buffers and
  report how much they used;
- **borrow instead of own**: return `&str`/`&[u8]` that point into the input
  instead of copying into a new owned value ("zero-copy").

These aren't workarounds. They're the techniques that make firmware fast,
predictable and impossible to run out of memory at runtime.

## Arrays and const generics

An array `[T; N]` is `N` values laid out back to back, on the stack or wherever
its owner lives. Its length is part of its type.

```rust
let mut samples = [0i16; 64];          // 128 bytes, all zero
let header: [u8; 4] = [0xAA, 0x55, 0x01, 0x08];
let squares: [u32; 8] = core::array::from_fn(|i| (i * i) as u32);
let doubled = squares.map(|x| x * 2);  // arrays have `map`
```

**Const generics** let functions and types be generic over the length:

```rust
pub struct RingBuffer<T, const N: usize> {
    data: [T; N],
    len: usize,
}

fn sum<const N: usize>(values: [u32; N]) -> u32 {
    values.iter().sum()
}
```

Converting a slice to an array checks the length at runtime and fails cleanly:

```rust
let bytes: &[u8] = &[1, 2, 3, 4, 5];
let first4: [u8; 4] = bytes[..4].try_into().unwrap();   // copies
let as_ref: &[u8; 4] = bytes[..4].try_into().unwrap();  // borrows
let (head, rest) = bytes.split_first_chunk::<4>().unwrap(); // (&[u8; 4], &[u8])
```

> **Pitfall:** big arrays on the stack. A microcontroller stack might be 2-8 KiB
> with **no guard page**: overflow silently corrupts other memory. `let buf =
> [0u8; 16384];` inside a function is a bug there. Large buffers belong in a
> `static` (module 05) or are passed in by the caller.

## Slices: the universal view

A slice `&[T]` is a pointer plus a length (a *fat pointer*). It can view an
array, part of an array, a `Vec`, a `static` table, or a DMA buffer, so
functions that take slices work with all of them.

The slice API is huge and entirely in `core`. Some highlights:

| Need | Method |
|---|---|
| safe indexing | `get(i)`, `get_mut(i)`, `first()`, `last()` return `Option` |
| splitting | `split_at(mid)`, `split_first()`, `split_first_chunk::<N>()`, `split(|b| *b == b',')` |
| blocks | `chunks(n)`, `chunks_exact(n)` (+ `remainder()`), `windows(n)` |
| copying | `copy_from_slice(src)` (lengths must match), `fill(v)`, `clone_from_slice` |
| reordering | `swap(a, b)`, `reverse()`, `rotate_left(k)`, `sort_unstable()` |
| searching | `contains(&x)`, `starts_with(..)`, `iter().position(..)`, `binary_search(&x)` |

**Slice patterns** destructure slices by shape: superb for protocol parsing.

```rust
enum Cmd<'a> { Read(u8), Write(u8, u8), Bulk(&'a [u8]), Invalid }

fn decode(frame: &[u8]) -> Cmd<'_> {
    match frame {
        [0x01, addr] => Cmd::Read(*addr),
        [0x02, addr, value] => Cmd::Write(*addr, *value),
        [0x03, payload @ ..] if payload.len() <= 8 => Cmd::Bulk(payload),
        _ => Cmd::Invalid,
    }
}
```

## Iterators: computation without containers

Iterators are lazy and allocation-free. A chain like
`data.iter().filter(..).map(..).sum()` compiles to one tight loop. In no_std
you consume iterators by:

- reducing them: `sum`, `count`, `fold`, `min`, `max`, `any`, `all`, `find`, `position`;
- writing into a caller's buffer:

```rust
fn scale_into(input: &[i16], gain: i16, out: &mut [i16]) -> usize {
    let mut n = 0;
    for (dst, &src) in out.iter_mut().zip(input) {
        *dst = src.saturating_mul(gain);
        n += 1;
    }
    n // how many were written: min(input.len(), out.len())
}
```

- returning them, so the caller decides what to do:

```rust
fn errors(log: &[u16]) -> impl Iterator<Item = u16> + '_ {
    log.iter().copied().filter(|code| *code >= 0x8000)
}
```

`collect()` works when the target type can be built without a heap, for
example `Option<..>`/`Result<..>` folding, or with `alloc`. Not into arrays:
use `core::array::from_fn` or a loop for that.

## Strings are bytes

`&str` is a slice of bytes guaranteed to be valid UTF-8. Everything that
doesn't allocate is in core:

```rust
let line = "  SET speed 120 \r\n";
let t = line.trim();                               // "SET speed 120"
let mut words = t.split_ascii_whitespace();
let verb = words.next();                           // Some("SET")
let n: Result<u16, _> = "120".parse();             // Ok(120)
let (key, value) = "mode=eco".split_once('=').unwrap();
let ok = verb.is_some_and(|v| v.eq_ignore_ascii_case("set"));
```

Things to keep straight:

- `s.len()` is in **bytes**, not characters. Slicing (`&s[a..b]`) must land on
  character boundaries or it panics; `s.get(a..b)` returns `Option`.
- Incoming data is `&[u8]`. Convert with `core::str::from_utf8(bytes)`, which
  checks validity and borrows (no copy).
- Many embedded protocols are ASCII. `u8` has `is_ascii_digit()`,
  `to_ascii_uppercase()`, and `[u8]` has `make_ascii_uppercase()`, `eq_ignore_ascii_case()`.
- `to_uppercase()`, `to_string()`, `format!`, `String` all need alloc.

A function that returns a sub-slice of its input is **zero-copy**. The lifetime
in its signature ties the output to the input:

```rust
/// "key=value" → ("key", "value"), borrowing from `line`.
fn kv(line: &str) -> Option<(&str, &str)> {
    let (k, v) = line.split_once('=')?;
    Some((k.trim(), v.trim()))
}
```

## Numbers to text, the manual way

`core::fmt` can format numbers (module 03), but it costs code size: pulling in
the formatting machinery adds kilobytes, which matters on a 32 KiB flash
part. Converting an integer to decimal by hand is short:

```rust
fn u16_to_dec(mut n: u16, buf: &mut [u8; 5]) -> &[u8] {
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 { break; }
    }
    &buf[i..]           // digits are at the end of the buffer
}
```

Notice the shape: the buffer is sized for the worst case (`u16::MAX` = 65535
has 5 digits), digits are produced least-significant first from the end, and
the function returns a borrowed view of the part it used.

## Byte order

Wire formats specify endianness. Integers convert to and from byte arrays
explicitly, with no casting tricks needed:

```rust
let raw = [0x12, 0x34, 0x56, 0x78];
assert_eq!(u32::from_be_bytes(raw), 0x1234_5678);   // network / Motorola order
assert_eq!(u32::from_le_bytes(raw), 0x7856_3412);   // Intel order, most MCUs
let out: [u8; 2] = 1000u16.to_le_bytes();
```

> **Toyota lens:** MISRA C forbids dynamic memory allocation after start-up
> (Rule 21.3 bans `malloc`/`free` outright), and ISO 26262-6 recommends static
> resource allocation for higher ASILs. Fixed-size arrays, caller-provided
> buffers and zero-copy parsing are the everyday techniques of ECU software in C.
> In Rust, the same designs get bounds checking and lifetime checking for free.

> **Interview:** "How would you return a variable number of results from a
> function without a heap?" Answer: the caller passes `&mut [T]` and you return
> the count (or a sub-slice); or return a fixed-capacity container like
> `heapless::Vec<T, N>`; or return a lazy iterator.
