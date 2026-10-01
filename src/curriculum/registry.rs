//! The ordered list of modules and exercises.

use super::{Case, Exercise, Expect, Mode, Module};

macro_rules! lesson {
    ($file:literal) => {
        include_str!(concat!("../../curriculum/lessons/", $file, ".md"))
    };
}

macro_rules! quiz {
    ($file:literal) => {
        Some(include_str!(concat!("../../curriculum/quizzes/", $file, ".quiz")))
    };
}

macro_rules! ex {
    ($dir:literal, $name:literal, $title:literal, $mode:expr, [$($hint:literal),* $(,)?] $(, extra: [$(($xf:literal)),* $(,)?])?) => {
        Exercise {
            name: $name,
            title: $title,
            dir: $dir,
            source: include_str!(concat!("../../curriculum/exercises/", $dir, "/", $name, ".rs")),
            solution: include_str!(concat!("../../curriculum/solutions/", $dir, "/", $name, ".rs")),
            extra: &[$($(($xf, include_str!(concat!("../../curriculum/exercises/", $dir, "/", $xf)))),*)?],
            mode: $mode,
            hints: &[$($hint),*],
        }
    };
}

#[allow(unused_macros)]
macro_rules! case {
    (args: [$($a:literal),*], stdin: $stdin:expr, stdout: $out:expr, stderr: $err:expr, exit: $code:expr) => {
        Case { args: &[$($a),*], stdin: $stdin, stdout: $out, stderr: $err, exit: $code }
    };
}

pub static MODULES: &[Module] = &[
    Module {
        num: 0,
        slug: "welcome",
        title: "Welcome",
        summary: "how this course and its checker work",
        lesson: lesson!("00-welcome"),
        exercises: &[],
        quiz: None,
    },
    Module {
        num: 1,
        slug: "landscape",
        title: "The no_std Landscape",
        summary: "core, alloc and std; what #![no_std] really changes",
        lesson: lesson!("01-landscape"),
        exercises: &[
            ex!("01_landscape", "core1", "Leaving std behind", Mode::Lib, [
                "Every `use std::...` line at the top refers to something that also exists in `core`. The logic doesn't need to change.",
                "Replace `std::` with `core::` in the imports, e.g. `use core::cmp::{max, min};`. Leave the `extern crate std;` inside the test module alone: tests run on the host.",
            ]),
            ex!("01_landscape", "core2", "Living in the core prelude", Mode::Lib, [
                "Iterators are your friend: `iter()`, `enumerate()`, `find()`, `filter()`, `count()`, `fold()` and `sum()` all live in core and never allocate.",
                "checksum: `data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b))`. first_fault: `codes.iter().copied().enumerate().find(|&(_, c)| c != 0)`.",
                "average: return None for empty input, then sum as i64: `samples.iter().map(|&s| i64::from(s)).sum::<i64>()` and divide by `samples.len() as i64`.",
                "parse_all: check `fields.len() > out.len()` first, then `for (index, (slot, field)) in out.iter_mut().zip(fields).enumerate()` and use `field.trim().parse()` with `map_err` and `?`.",
            ]),
            ex!("01_landscape", "core3", "std habits, no_std replacements", Mode::Lib, [
                "Start with the alloc crate: add `extern crate alloc;` below `#![no_std]`, then import `String`, `Vec` and `format!` from `alloc::` paths.",
                "`alloc::collections::BTreeMap` has the same `entry().or_insert()` API as HashMap. The tests only use `len()` and `get()`.",
                "log_warning: `writeln!(out, \"warning: {}\", msg)` writes into the sink and returns the `fmt::Result`. Import the trait from `core::fmt::Write`.",
                "distance_mm: work in u64 to avoid overflow (`i32::unsigned_abs()` then `u64::from`), sum the squares and call `.isqrt()` (stable since Rust 1.84).",
            ]),
        ],
        quiz: quiz!("01-landscape"),
    },
    Module {
        num: 2,
        slug: "data",
        title: "Data Without a Heap",
        summary: "arrays, const generics, slices, iterators and zero-copy text",
        lesson: lesson!("02-data"),
        exercises: &[
            ex!("02_data", "slices1", "Slicing and dicing", Mode::Lib, [
                "Look at the slice methods table in the lesson: each function maps to one or two methods. `split_first_chunk::<4>()` returns exactly the `Option<(&[u8; 4], &[u8])>` you need.",
                "find_sync: `stream.windows(pattern.len()).position(|w| w == pattern)`. Handle the empty pattern first (windows(0) panics).",
                "moving_max / xor_blocks: zip the output buffer with `data.windows(n)` / `data.chunks(n)` so the shorter one stops the loop, and count iterations. Guard against n == 0 (both methods panic on 0).",
                "decode: `match frame { [] => .., [0x01, addr] => .., [0x02, addr, value] => .., [0x03, payload @ ..] if payload.len() <= 8 => .., _ => .. }`",
            ]),
            ex!("02_data", "arrays1", "Arrays and const generics", Mode::Lib, [
                "`core::array::from_fn(|i| ...)` builds an array of any length `N` from the index: perfect for squares, transpose and mul_vec.",
                "to_array: `s.try_into().ok()`. The compiler knows the target type `[u8; N]` from the return type.",
                "transpose: `Matrix(core::array::from_fn(|c| core::array::from_fn(|r| self.0[r][c])))`: the outer array has C rows now.",
                "len is a `const fn` and R, C are constants: just `R * C`. swap_words: arrays have `map`, and `u16::swap_bytes` is a function you can pass directly.",
            ]),
            ex!("02_data", "strings1", "A zero-copy console parser", Mode::Lib, [
                "Trim the line, then split off the verb with `split_once(|c: char| c.is_ascii_whitespace())`; if there is no whitespace the whole line is the verb.",
                "Compare verbs with `eq_ignore_ascii_case`. For the arguments use `rest.split_ascii_whitespace()` and `.next().ok_or(ParseError::MissingArgument)?`.",
                "For echo, return the trimmed rest of the line as-is (internal spaces preserved). For set/get/reset, check `args.next().is_some()` at the end for TooManyArguments.",
                "NMEA: `strip_prefix('$')?`, then `split_once('*')?`, then fold the payload bytes with XOR. To validate, parse the two hex digits with `u8::from_str_radix(hex, 16)` after trimming a trailing `\\r\\n` and checking the length is 2.",
            ]),
            ex!("02_data", "itoa1", "Numbers to text, by hand", Mode::Lib, [
                "Loop: `i -= 1; buf[i] = b'0' + (n % 10) as u8; n /= 10;` until n == 0, starting with `i = buf.len()`. Use `loop { .. if n == 0 { break } }` so that 0 still produces one digit.",
                "For i32, work on `n.unsigned_abs()` (a u32 that can hold 2147483648) and prepend `b'-'` at the end if n < 0.",
                "milli_to_dec: emit digits like before, but insert `b'.'` after exactly three digits, and keep going until you've emitted at least four digits (so 5 becomes 0.005).",
                "parse_u16: `value.checked_mul(10)?.checked_add(u16::from(d - b'0'))?` catches overflow; reject non-digits and the empty input.",
            ]),
        ],
        quiz: quiz!("02-data"),
    },
    Module {
        num: 3,
        slug: "formatting",
        title: "Formatting Without Allocation",
        summary: "core::fmt, fmt::Write sinks, Display/Debug and the cost of formatting",
        lesson: lesson!("03-formatting"),
        exercises: &[
            ex!("03_formatting", "fmt1", "A string without a heap", Mode::Lib, [
                "Keep the invariant: `buf[..len]` is always valid UTF-8. Then `as_str` is just `core::str::from_utf8(&self.buf[..self.len]).unwrap_or(\"\")`.",
                "try_push_str: if `s.len() > self.remaining()` return false; otherwise `copy_from_slice` into `buf[len..len + s.len()]` and bump len.",
                "write_str: try the whole string first. If it doesn't fit, start at `cut = remaining()` and walk down until `s.is_char_boundary(cut)`, push `&s[..cut]`, and return `Err(fmt::Error)`.",
            ]),
            ex!("03_formatting", "fmt2", "Display, Debug and friends", Mode::Lib, [
                "Do the arithmetic on `u64::from(self.0.unsigned_abs())`. For p decimals, divide by `10^(3-p)` with rounding: `(abs + scale/2) / scale`. Then split into `rounded / 10^p` and `rounded % 10^p`.",
                "Print the fraction zero-padded: `write!(out, \"{int}.{frac:0width$}\", width = p)`. Only print '-' if the original was negative AND the rounded value isn't 0.",
                "Display for Milli: render into a StackBuf, then `f.pad_integral(is_nonnegative, \"\", digits_without_sign)`, which respects width/fill/`+`/`0`. Display for Reading: render the whole line into a StackBuf and pad it yourself using `f.width()`, `f.align()`, `f.fill()`.",
                "Debug: `f.debug_struct(\"Reading\").field(\"sensor\", &self.sensor).field(\"value\", &format_args!(\"{}\", Milli(self.milli))).field(\"unit\", &self.unit).finish()`. LowerHex: delegate with `fmt::LowerHex::fmt(&self.0, f)`.",
            ]),
            ex!("03_formatting", "fmt3", "A hexdump formatter", Mode::Lib, [
                "Iterate `data.chunks(16).enumerate()`. The offset is `base.wrapping_add((line * 16) as u32)`, printed with `{:08x}`.",
                "For the hex columns loop `for col in 0..16` and use `chunk.get(col)`: `Some(b)` → `write!(out, \"{b:02x} \")`, `None` → three spaces. After col 7 write one extra space.",
                "HexBytes: `f.alternate()` is true for `{:#}`. Write ':' before every byte except the first in that case.",
            ]),
        ],
        quiz: quiz!("03-formatting"),
    },
    Module {
        num: 4,
        slug: "errors",
        title: "Errors, Panics & Overflow",
        summary: "error enums, panic handlers, panic-free code and explicit overflow semantics",
        lesson: lesson!("04-errors"),
        exercises: &[
            ex!("04_errors", "errors1", "Error types that compose", Mode::Lib, [
                r#"parse_frame: destructure with a slice pattern first: `let (start, len) = match raw { [s, l, ..] => (*s, usize::from(*l)), _ => return Err(TooShort { needed: 2, got: raw.len() }) };` then check `raw.len() < len + 3`."#,
                r#"Display: `match self { FrameError::TooShort { needed, got } => write!(f, "frame too short: need {needed} bytes, got {got}"), ... }`. `{b:#04x}` prints `0x0a` (the width includes the 0x)."#,
                r#"For `?` to convert, write three small impls such as `impl From<FrameError> for ConfigError { fn from(e: FrameError) -> Self { ConfigError::Frame(e) } }`."#,
                r#"load_config: `let text = core::str::from_utf8(parse_frame(raw)?)?;` then loop over `text.split(';').filter(|s| !s.is_empty())`, use `split_once('=').ok_or(ConfigError::UnknownKey)?` and `value.parse()?`. Keep `id`/`rate` in `Option`s and finish with `.ok_or(ConfigError::Missing("id"))?`."#,
            ]),
            ex!("04_errors", "panicfree1", "Make it impossible to panic", Mode::Lib, [
                r#"Find the panic sources in each function: indexing, slicing, `/`, `+`, `*`. Each has a non-panicking counterpart: `get`, `checked_*`, iterators."#,
                r#"average and percent: widen to u64 before summing/multiplying, use `checked_div`, then `u32::try_from(..).ok()`."#,
                r#"pair_at: `let next = i.checked_add(1)?; Some((*data.get(i)?, *data.get(next)?))`. field: `record.split(',').nth(n).map(str::trim)`, no array needed."#,
                r#"prefix_chars: `s.char_indices().nth(n)` gives the byte index where character n starts (or None if s is shorter), so slice with `s.get(..idx)`. length_prefixed: `buf.split_first_chunk::<2>()?` then `rest.get(..len)`."#,
            ]),
            ex!("04_errors", "overflow1", "Choosing overflow semantics", Mode::Lib, [
                r#"Counters, checksums and sequence numbers are modular: use `wrapping_sub`/`wrapping_add`. Physical values clamp: `saturating_*` or compute wide then clamp."#,
                r#"deadline_reached: `(now.wrapping_sub(deadline) as i32) >= 0`. The wrapped difference is "small positive" if now is after the deadline, and "huge", i.e. negative as i32, if before."#,
                r#"apply_gain: compute in u32 (`u32::from(raw) * u32::from(gain) / 256`) and clamp with `u16::try_from(x).unwrap_or(u16::MAX)`. mul_full: multiply as u64 and split with `>> 32` and `as u32`."#,
                r#"midpoint without a wider type: `(a & b) + ((a ^ b) >> 1)`, i.e. the bits both have, plus half of the bits only one has. (`u32::midpoint` also exists since 1.85, but do the trick by hand once.)"#,
            ]),
        ],
        quiz: quiz!("04-errors"),
    },
    Module {
        num: 5,
        slug: "memory",
        title: "Memory Without a Heap",
        summary: "const vs static, compile-time tables, MaybeUninit, ring buffers and pools",
        lesson: lesson!("05-memory"),
        exercises: &[
            ex!("05_memory", "statics1", "Tables in flash and safe global state", Mode::Lib, [
                r#"crc8_table: two nested `while` loops (`let mut i = 0; while i < 256 { ...; i += 1; }`). Start each entry with `crc = i as u8` and apply the 8 shift rounds from the doc comment."#,
                r#"crc8_sae_j1850: `data.iter().fold(0xFFu8, |crc, &b| CRC8_TABLE[usize::from(crc ^ b)]) ^ 0xFF`."#,
                r#"record_boot: declare `static BOOT_COUNT: AtomicU32 = AtomicU32::new(0);` and return `BOOT_COUNT.fetch_add(1, Ordering::Relaxed) + 1` (fetch_add returns the *previous* value)."#,
                r#"scratch_ptr: `(&raw mut SCRATCH).cast::<u8>()` creates a raw pointer without a reference (no unsafe needed!). fill_scratch: write each byte with `unsafe { p.add(i).write(byte) }` for i in 0..SCRATCH_LEN."#,
            ]),
            ex!("05_memory", "stackvec1", "A Vec without a heap", Mode::Lib, [
                r#"new: `StackVec { data: [const { MaybeUninit::uninit() }; N], len: 0 }`. The inline `const { }` lets you repeat a non-Copy value."#,
                r#"push: `self.data.get_mut(self.len)` gives `Some(slot)` if there's room: `slot.write(value); self.len += 1;`. Otherwise `Err(value)`."#,
                r#"pop: decrement len first, then `unsafe { self.data[self.len].assume_init_read() }`. Because len no longer covers that slot, it will never be read or dropped again."#,
                r#"as_slice: `unsafe { core::slice::from_raw_parts(self.data.as_ptr().cast::<T>(), self.len) }`. truncate can just call pop in a loop; clear = truncate(0); Drop = clear(). swap_remove: swap with the last element, then pop."#,
            ]),
            ex!("05_memory", "ring1", "A ring buffer that never allocates", Mode::Lib, [
                r#"The item at logical position i (0 = oldest) lives at physical index `(head + i) % N`. The next free slot is at `(head + len) % N`."#,
                r#"push when full: replace `buf[head]` (the oldest) with the new value using `core::mem::replace`, then advance `head = (head + 1) % N`. Return early with `Some(value)` when N == 0."#,
                r#"iter: `(0..self.len).map(move |i| &self.buf[(self.head + i) % N])`. extend_from_slice: `items.iter().filter_map(|&x| self.push(x)).count()`."#,
            ]),
            ex!("05_memory", "pool1", "An object pool with generational handles", Mode::Lib, [
                r#"new: `core::array::from_fn(|i| Slot::Free { next_free: if i + 1 < N { Some((i + 1) as u16) } else { None }, generation: 0 })`, and free_head = Some(0) unless N == 0."#,
                r#"alloc: take `self.free_head`; read the slot's `next_free` and `generation`; overwrite it with `Slot::Used { value, generation }`; set `free_head = next_free`."#,
                r#"get/get_mut: `match self.slots.get(index)? { Slot::Used { value, generation } if *generation == h.generation => Some(value), _ => None }`."#,
                r#"free: validate first (Used + matching generation). Then `mem::replace(slot, Slot::Free { next_free: self.free_head, generation: h.generation.wrapping_add(1) })` gives you the old Used slot to take the value from; set free_head = Some(h.index)."#,
            ]),
        ],
        quiz: quiz!("05-memory"),
    },
];

#[allow(dead_code)]
const _USE: (Option<Case>, Option<Expect>) = (None, None);
