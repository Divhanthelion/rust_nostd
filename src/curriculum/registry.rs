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
    (args: [$($a:literal),*], $(env: [$(($k:literal, $v:literal)),*],)? stdin: $stdin:expr, stdout: $out:expr, stderr: $err:expr, exit: $code:expr) => {
        Case { args: &[$($a),*], env: &[$($(($k, $v)),*)?], stdin: $stdin, stdout: $out, stderr: $err, exit: $code }
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
    Module {
        num: 6,
        slug: "alloc",
        title: "The alloc Crate & Global Allocators",
        summary: "Vec/String/BTreeMap in no_std, arenas, GlobalAlloc and Layout",
        lesson: lesson!("06-alloc"),
        exercises: &[
            ex!("06_alloc", "alloc1", "The alloc toolbox", Mode::Lib, [
                r#"Add `extern crate alloc;` under `#![no_std]`, then `use alloc::{boxed::Box, collections::BTreeMap, rc::Rc, string::String, vec::Vec};`."#,
                r#"summarize: `let s = map.entry(f.id).or_default();` (Stats derives Default) then update count/bytes/max_len. render: `writeln!(out, "{:#05x} count={} ...", ...)` into a String (`core::fmt::Write` is imported)."#,
                r#"busiest: collect `(id, count)` pairs into a Vec, `sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)))`, then `truncate(n)`."#,
                r#"Filters: `Box::new(move |f| (lo..=hi).contains(&f.id))`. passes: `filters.iter().all(|f| f(frame))`. SharedLog: `Rc::new(RefCell::new(Vec::new()))`, `Rc::clone(&self.lines)`, `self.lines.borrow_mut().push(alloc::format!("{}: {}", self.prefix, msg))`."#,
            ]),
            ex!("06_alloc", "arena1", "A bump arena with a safe API", Mode::Lib, [
                r#"align_up: `Some(addr.checked_add(align - 1)? & !(align - 1))`."#,
                r#"alloc_layout: `let base = self.start as usize; let aligned = align_up(base.checked_add(self.next.get())?, layout.align())?; let offset = aligned - base; let end = offset.checked_add(layout.size())?;` then check `end <= capacity`, set `next`, return `NonNull::new(unsafe { self.start.add(offset) })`."#,
                r#"alloc: `Layout::new::<T>()`, cast the pointer to `*mut T`, `p.write(value)` and return `&mut *p`, all in one unsafe block with a SAFETY comment."#,
                r#"alloc_slice_copy: `Layout::array::<T>(src.len()).ok()?`, then `ptr::copy_nonoverlapping(src.as_ptr(), p, len)` and `slice::from_raw_parts_mut(p, len)`. alloc_str: copy the bytes, then `core::str::from_utf8_unchecked_mut`."#,
            ]),
            ex!("06_alloc", "alloc2", "Write a global allocator", Mode::Lib, [
                r#"Everything happens inside `self.with_state(|s| { ... })`. Compute `base = self.heap.get() as usize`, then the aligned absolute start from `base + s.next`."#,
                r#"Use checked arithmetic and return `ptr::null_mut()` on overflow or when `offset + layout.size() > N`. Otherwise set `s.next`, increment `s.live`, and return `self.heap.get().cast::<u8>().add(offset)`."#,
                r#"dealloc: decrement `s.live` (saturating), and if it is now 0 set `s.next = 0`."#,
            ]),
            ex!("06_alloc", "layout1", "Size, alignment and padding by hand", Mode::Lib, [
                r#"padding_needed: `(align - offset % align) % align`, or the bit trick `offset.wrapping_neg() & (align - 1)`."#,
                r#"repr_c_layout: start from `Layout::from_size_align(0, 1)`, and for each field `let (next, offset) = layout.extend(Layout::from_size_align(size, align).ok()?).ok()?;`. Finish with `pad_to_align()`."#,
                r#"best_size: copy the fields into a local `[(usize, usize); 8]` (return None if more than 8), sort the used part by alignment descending with `sort_unstable_by`, and reuse repr_c_layout. u32_array: `Layout::array::<u32>(n).ok()`."#,
            ]),
        ],
        quiz: quiz!("06-alloc"),
    },
    Module {
        num: 7,
        slug: "sync",
        title: "Interior Mutability, Atomics & Synchronization",
        summary: "Send/Sync, cells, atomics and orderings, spinlocks, critical sections, SPSC queues",
        lesson: lesson!("07-sync"),
        exercises: &[
            ex!("07_sync", "cells1", "Interior mutability without threads", Mode::Lib, [
                r#"Cell: `self.frames.set(self.frames.get() + 1)`. take(): `Cell::take` returns the value and leaves `Default::default()` (0) behind."#,
                r#"Sensor::new: `OnceCell::new()`, `RefCell::new([0; 4])`, and `LazyCell::new(build_lut)` (a plain `fn` works as the initialiser). calibrate is just `self.calibration.set(c)`."#,
                r#"read: `let c = self.calibration.get()?;` then compute in i32 and clamp to the i16 range. Update the history with `let mut h = self.history.borrow_mut(); h.rotate_left(1); h[3] = v;`."#,
                r#"try_clear_history: `*self.history.try_borrow_mut()? = [0; 4]; Ok(())`. The `?` converts nothing here: the error type is already BorrowMutError. lut_entry: LazyCell derefs to the array: `self.lut.get(i).copied()`."#,
            ]),
            ex!("07_sync", "atomics1", "Counters, flags and a mailbox", Mode::Lib, [
                r#"record: `fetch_add(1, Relaxed)` for counts, `fetch_max`/`fetch_min` for latencies. take: `swap(0, Relaxed)` (and `swap(u32::MAX, ..)` for the minimum)."#,
                r#"saturating_inc: load the current value, compute `current.saturating_add(1)`, and `compare_exchange_weak(current, new, ..)`; on `Err(actual)` retry with actual. Return `new` on success."#,
                r#"EventFlags: `fetch_or(mask, Release)` to raise, `swap(0, Acquire)` to take everything at once."#,
                r#"Mailbox::post: if `full.load(Acquire)` return false; else store the payload (Relaxed), then `full.store(true, Release)`. take: if `full.load(Acquire)`, read the payload, then `full.store(false, Release)`."#,
            ]),
            ex!("07_sync", "spinlock1", "A spin lock with an RAII guard", Mode::Lib, [
                r#"The static in the tests needs `SpinLock<u64>: Sync`. Add `unsafe impl<T: Send> Sync for SpinLock<T> {}`: a lock moves *access* to T between threads, which is what Send means."#,
                r#"lock: `while self.locked.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() { core::hint::spin_loop(); }` then return `SpinGuard { lock: self }`."#,
                r#"Deref: `unsafe { &*self.lock.value.get() }`; DerefMut: `unsafe { &mut *self.lock.value.get() }`. Drop: `self.lock.locked.store(false, Ordering::Release)`. get_mut/into_inner: `UnsafeCell::get_mut` / `into_inner`."#,
            ]),
            ex!("07_sync", "once1", "Initialise exactly once, from any thread", Mode::Lib, [
                r#"Start with `unsafe impl<T: Send + Sync> Sync for Once<T> {}`. Readers get &T from many threads (Sync), and the value may be dropped on a different thread than it was created on (Send)."#,
                r#"get: if `state.load(Acquire) == READY`, return `unsafe { (*self.value.get()).assume_init_ref() }`."#,
                r#"get_or_init: fast path via get(). Then `compare_exchange(EMPTY, RUNNING, Acquire, Acquire)`: if Ok you're the winner: write the value, `state.store(READY, Release)`. Everyone (winner included) then loops on get() with spin_loop() until it's Some."#,
                r#"Drop: `if *self.state.get_mut() == READY { unsafe { self.value.get_mut().assume_init_drop() } }`. get_mut on an atomic needs no synchronisation because you have &mut self."#,
            ]),
            ex!("07_sync", "cs1", "Critical sections and the token pattern", Mode::Lib, [
                r#"The statics need `Mutex<..>: Sync`: `unsafe impl<T: Send> Sync for Mutex<T> {}`. That's exactly the bound critical_section::Mutex uses."#,
                r#"borrow: `unsafe { &*self.inner.get() }`. The signature already ties the output to the token's lifetime 'cs."#,
                r#"Counter::increment: `interrupt::free(|cs| { let c = self.count.borrow(cs); c.set(c.get() + 1); c.get() })`."#,
                r#"install: `interrupt::free(|cs| { let mut slot = SHARED_UART.borrow(cs).borrow_mut(); ... })`. with_uart: `...borrow_mut().as_mut().map(f)`. uninstall: `...borrow_mut().take()`."#,
            ]),
            ex!("07_sync", "spsc1", "A lock-free SPSC queue", Mode::Lib, [
                r#"enqueue: `let tail = tail.load(Relaxed); let next = (tail + 1) % N;` If `next == head.load(Acquire)` it's full. Otherwise write the slot, then `tail.store(next, Release)`."#,
                r#"Write a slot with `unsafe { (*self.q.buf[tail].get()).write(value) }`, read with `(*self.q.buf[head].get()).assume_init_read()`."#,
                r#"dequeue: `let head = head.load(Relaxed);` empty if `head == tail.load(Acquire)`. Read the slot, then `head.store((head + 1) % N, Release)`. len: `(tail + N - head) % N`."#,
                r#"Drop has &mut self: read both indices with `get_mut()` and `assume_init_drop()` each slot from head up to tail (wrapping)."#,
            ]),
        ],
        quiz: quiz!("07-sync"),
    },
    Module {
        num: 8,
        slug: "unsafe",
        title: "Unsafe Rust, Raw Pointers & Hardware Registers",
        summary: "safety contracts, raw pointers, volatile MMIO, layouts and type-state APIs",
        lesson: lesson!("08-unsafe-mmio"),
        exercises: &[
            ex!("08_unsafe", "bits1", "Bit manipulation", Mode::Lib, [
                r#"`1u32.checked_shl(n)` is `None` for n >= 32, so `1u32.checked_shl(n).unwrap_or(0)` is a mask that's simply empty for out-of-range bits. Then |, & !, ^ and & do the rest."#,
                r#"field_mask: reject width 0 and `offset.checked_add(width)? > 32`. Build `width` ones (careful: `1 << 32` overflows, special-case width 32) and shift left by offset."#,
                r#"insert: shift the value into place with `checked_shl`, and reject it if any shifted bit lands outside the mask (`shifted & !mask != 0`) or bits fell off the top (`shifted >> offset != value`)."#,
                r#"sign_extend: shift the field to the top (`value << (32 - bits)`), reinterpret as i32, then arithmetic-shift back down: `((value << s) as i32) >> s`."#,
            ]),
            ex!("08_unsafe", "mmio1", "A volatile register driver", Mode::Lib, [
                r#"Get a pointer to one register without creating a reference: `&raw mut (*self.regs).moder`. Read/write it with `core::ptr::read_volatile`/`write_volatile` inside an unsafe block."#,
                r#"set_mode: read MODER, clear the two bits at `2 * pin` with `!(0b11 << (2 * p))`, OR in `(mode as u32) << (2 * p)`, write back."#,
                r#"set_high writes `1 << p` to BSRR, set_low writes `1 << (p + 16)`. Never touch ODR in those two. is_high reads IDR."#,
            ]),
            ex!("08_unsafe", "repr1", "Wire formats, enums and layout attributes", Mode::Lib, [
                r#"TryFrom: `match b { 0x01 => Ok(MsgType::Heartbeat), ..., other => Err(other) }`."#,
                r#"parse: `bytes.split_first_chunk::<HEADER_LEN>().ok_or(ParseError::TooShort)?` gives `&[u8; 8]` plus the rest; index the array with constant indices (can't panic)."#,
                r#"Use `u16::from_be_bytes([h[2], h[3]])` and `u32::from_le_bytes([h[4], h[5], h[6], h[7]])`. The payload is `rest.get(..length as usize)`, else Truncated."#,
                r#"packed_length: a reference to a packed field is rejected. `{ h.length }` copies the value into an aligned temporary first."#,
            ]),
            ex!("08_unsafe", "typestate1", "Make illegal hardware states unrepresentable", Mode::Lib, [
                r#"take(): `TAKEN.swap(true, Ordering::AcqRel)` returns the previous value: if it was already true, someone else owns the peripherals."#,
                r#"split: build each `Pin { port: &*self, _mode: PhantomData }`. The const generic N and the mode come from the field types in `Pins`."#,
                r#"into_mode: compute `shift = 2 * u32::from(N)`, update `moder` with the usual clear-then-set, and return `Pin { port: self.port, _mode: PhantomData }`: same pin, new type."#,
                r#"The generic `pulse` needs `impl<const N: u8> OutputPin for Pin<'_, N, Output>` whose methods call the inherent ones (`Pin::set_high(self)`)."#,
            ]),
            ex!("08_unsafe", "unsafe1", "Sound abstractions over unsafe code", Mode::Lib, [
                r#"Check every precondition with safe code first (bounds, alignment, lengths) and return None early. Only then enter `unsafe`."#,
                r#"split_at_mut: `let p = s.as_mut_ptr();` then `slice::from_raw_parts_mut(p, mid)` and `slice::from_raw_parts_mut(p.add(mid), len - mid)`."#,
                r#"read_u32_le: get the 4 bytes with `buf.get(offset..offset.checked_add(4)?)?`, then `ptr::read_unaligned(bytes.as_ptr().cast::<u32>())` and `u32::from_le`."#,
                r#"as_u32_slice: `ptr.cast::<u32>().is_aligned()` and `len % 4 == 0`. zeroize: `ptr::write_volatile(b, 0)` for each byte, then `compiler_fence(Ordering::SeqCst)`. sum_raw: `unsafe { ptr.add(i).read() }` in the loop."#,
            ]),
        ],
        quiz: quiz!("08-unsafe"),
    },
    Module {
        num: 9,
        slug: "freestanding",
        title: "Freestanding Linux Binaries",
        summary: "_start, system calls, memcpy & co, println! and panic handlers with no libc",
        lesson: lesson!("09-freestanding"),
        exercises: &[
            ex!("09_freestanding", "start1", "Life before main", Mode::Bin { rt: false, cases: &[
                case!(args: [], stdin: "", stdout: Expect::Exact("Hello from _start!\n"), stderr: Expect::Exact(""), exit: 42),
            ] }, [
                r#"Right now `main` hits `todo!()`, the panic handler calls `sys_exit`, which is also `todo!()`... and so on until the stack overflows (SIGSEGV). Implement sys_exit first."#,
                r#"x86_64 exit_group: `asm!("syscall", in("rax") 231usize, in("rdi") code as usize, options(noreturn, nostack))`. aarch64: `asm!("svc #0", in("x0") code as usize, in("x8") 94usize, options(noreturn, nostack))`."#,
                r#"x86_64 write: `inlateout("rax") 1isize => ret`, `in("rdi") fd as usize`, `in("rsi") buf.as_ptr()`, `in("rdx") buf.len()`, plus `lateout("rcx") _, lateout("r11") _` because `syscall` clobbers them. aarch64: number 64 in x8, args in x0-x2, result in x0."#,
                r#"main: `sys_write(1, b"Hello from _start!\n"); 42`"#,
            ]),
            ex!("09_freestanding", "rt1", "The platform contract (memcpy & co)", Mode::Bin { rt: false, cases: &[
                case!(args: [], stdin: "", stdout: Expect::Exact("copy ok\nfill ok\nmove ok\ncompare ok\n"), stderr: Expect::Exact(""), exit: 0),
            ] }, [
                r#"Signatures: `#[unsafe(no_mangle)] pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void`. memset takes `c: c_int`; memcmp/bcmp return `c_int`."#,
                r#"Cast to bytes with `dest.cast::<u8>()` and copy in a `while i < n` loop with `*d.add(i) = *s.add(i)` inside `unsafe { }`. Return the original `dest`/`s` pointer."#,
                r#"memmove must handle overlap: if dest is *after* src, copy backwards (from n-1 down to 0), otherwise forwards. That's what makes `copy_within` correct."#,
                r#"memcmp: return `c_int::from(x) - c_int::from(y)` for the first differing pair (bytes compared as unsigned), else 0. bcmp can simply call memcmp."#,
            ]),
            ex!("09_freestanding", "print1", "println! from scratch", Mode::Bin { rt: false, cases: &[
                case!(args: [], stdin: "", stdout: Expect::Exact("no newline yet, now there is.\nleft    |    42|  mid  |\n0xff 0b101 +7\nSome([1, 2, 3]) 3.14\n\n################################################################\n"), stderr: Expect::Exact("warning: 3 retries\n"), exit: 0),
            ] }, [
                r#"write_all: `while !buf.is_empty() { let n = sys_write(fd, buf); ... }`. On -4 `continue`; on n <= 0 return `Err(n)`; otherwise advance with `buf = &buf[n as usize..]` (or `.get(..)`)."#,
                r#"Stdout's write_str: `write_all(1, s.as_bytes()).map_err(|_| fmt::Error)`."#,
                r#"println!: two rules, `() => { print!("\n") };` and `($($arg:tt)*) => {{ let _ = Stdout.write_fmt(format_args!("{}\n", format_args!($($arg)*))); }};`. Nesting format_args! appends the newline without allocating."#,
            ]),
            ex!("09_freestanding", "panic1", "A panic handler worth having", Mode::Bin { rt: false, cases: &[
                case!(args: [], stdin: "", stdout: Expect::Exact(""), stderr: Expect::Contains(&["PANIC [exercises/09_freestanding/panic1.rs:", "] index out of bounds: the len is 3 but the index is 7\n"]), exit: 101),
                case!(args: ["one"], stdin: "", stdout: Expect::Exact(""), stderr: Expect::Contains(&["PANIC [exercises/09_freestanding/panic1.rs:", "] sensor 7 timed out\n"]), exit: 101),
                case!(args: ["one", "two"], stdin: "", stdout: Expect::Exact("no panic\n"), stderr: Expect::Exact(""), exit: 0),
            ] }, [
                r#"`let mut err = Stderr;` then `match info.location() { Some(loc) => { let _ = write!(err, "PANIC [{}:{}] ", loc.file(), loc.line()); } None => ... }`."#,
                r#"Finish with `let _ = writeln!(err, "{}", info.message());` and `sys_exit(101)`. `info.message()` is a `PanicMessage`, which implements Display."#,
            ]),
            ex!("09_freestanding", "args1", "Arguments and environment, straight off the stack", Mode::Bin { rt: false, cases: &[
                case!(args: ["one", "two words"], env: [("NOSTD_GREETING", "hello")], stdin: "", stdout: Expect::Exact("argc=3\nargv[0]=args1\nargv[1]=one\nargv[2]=two words\ngreeting=hello\n"), stderr: Expect::Exact(""), exit: 0),
                case!(args: [], stdin: "", stdout: Expect::Exact("argc=1\nargv[0]=args1\ngreeting=(unset)\n"), stderr: Expect::Exact(""), exit: 0),
                case!(args: ["--x"], env: [("NOSTD_GREETING_EXTRA", "no"), ("XNOSTD_GREETING", "no")], stdin: "", stdout: Expect::Exact("argc=2\nargv[0]=args1\nargv[1]=--x\ngreeting=(unset)\n"), stderr: Expect::Exact(""), exit: 0),
            ] }, [
                r#"rust_start: `let argc = *sp; let argv = sp.add(1) as *const *const u8; let envp = argv.add(argc + 1);` (all inside one unsafe block). The `+ 1` skips argv's NULL terminator."#,
                r#"cstr_bytes: count bytes until `*p.add(len) == 0`, then `core::slice::from_raw_parts(p, len)`."#,
                r#"Args::get: bounds-check i against argc, then `cstr_bytes(*self.argv.add(i))`."#,
                r#"Env::get: walk envp until you read a null pointer. For each entry, `kv.strip_prefix(name)` and then `rest.strip_prefix(b"=")`: both must succeed, so HOMER=... doesn't match HOME."#,
            ]),
            ex!("09_freestanding", "stdin1", "A streaming filter with a 64-byte buffer", Mode::Bin { rt: false, cases: &[
                case!(args: [], stdin: "Hello, World!\n", stdout: Expect::Exact("Uryyb, Jbeyq!\n"), stderr: Expect::Exact("bytes=14 lines=1\n"), exit: 0),
                case!(args: [], stdin: "The Quick Brown Fox Jumps Over The Lazy Dog. no_std Rust runs anywhere: MCUs, kernels, ECUs!\nThe Quick Brown Fox Jumps Over The Lazy Dog. no_std Rust runs anywhere: MCUs, kernels, ECUs!\nThe Quick Brown Fox Jumps Over The Lazy Dog. no_std Rust runs anywhere: MCUs, kernels, ECUs!\n", stdout: Expect::Exact("Gur Dhvpx Oebja Sbk Whzcf Bire Gur Ynml Qbt. ab_fgq Ehfg ehaf naljurer: ZPHf, xrearyf, RPHf!\nGur Dhvpx Oebja Sbk Whzcf Bire Gur Ynml Qbt. ab_fgq Ehfg ehaf naljurer: ZPHf, xrearyf, RPHf!\nGur Dhvpx Oebja Sbk Whzcf Bire Gur Ynml Qbt. ab_fgq Ehfg ehaf naljurer: ZPHf, xrearyf, RPHf!\n"), stderr: Expect::Exact("bytes=279 lines=3\n"), exit: 0),
                case!(args: [], stdin: "", stdout: Expect::Exact(""), stderr: Expect::Exact("bytes=0 lines=0\n"), exit: 0),
            ] }, [
                r#"sys_read mirrors sys_write: number 0 in rax (x86_64) or 63 in x8 (aarch64), and pass `buf.as_mut_ptr()`."#,
                r#"rot13: `b'a'..=b'z' => (b - b'a' + 13) % 26 + b'a'`, the same for uppercase, everything else unchanged."#,
                r#"main: `loop { let n = sys_read(0, &mut buf); ... }`: -4 → continue, < 0 → return 1, 0 → break. Then transform `&mut buf[..n as usize]`, count, and `write_all(1, chunk)`."#,
                r#"After the loop: `let _ = writeln!(Stderr, "bytes={bytes} lines={lines}");` and return 0."#,
            ]),
        ],
        quiz: quiz!("09-freestanding"),
    },
    Module {
        num: 10,
        slug: "cli",
        title: "Capstone: no_std CLI Tools",
        summary: "streaming tools on the course runtime: fixed buffers, state machines, exit codes",
        lesson: lesson!("10-cli"),
        exercises: &[
            ex!("10_cli", "wc1", "word count, no libc", Mode::Bin { rt: true, cases: &[
                case!(args: [], stdin: "hello world\nthis is no_std\n", stdout: Expect::Exact("      2       5      27\n"), stderr: Expect::Exact(""), exit: 0),
                case!(args: ["-l"], stdin: "hello world\nthis is no_std\n", stdout: Expect::Exact("      2\n"), stderr: Expect::Exact(""), exit: 0),
                case!(args: ["-wc"], stdin: "hello world\nthis is no_std\n", stdout: Expect::Exact("      5      27\n"), stderr: Expect::Exact(""), exit: 0),
                case!(args: ["-c", "-l"], stdin: "a\nb", stdout: Expect::Exact("      1       3\n"), stderr: Expect::Exact(""), exit: 0),
                case!(args: [], stdin: "abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc abc ", stdout: Expect::Exact("      0    3000   12000\n"), stderr: Expect::Exact(""), exit: 0),
                case!(args: [], stdin: "", stdout: Expect::Exact("      0       0       0\n"), stderr: Expect::Exact(""), exit: 0),
                case!(args: ["-x"], stdin: "", stdout: Expect::Exact(""), stderr: Expect::Exact("wc: unknown option: -x\n"), exit: 2),
                case!(args: ["notes.txt"], stdin: "", stdout: Expect::Exact(""), stderr: Expect::Exact("wc: unknown option: notes.txt\n"), exit: 2),
            ] }, [
                r#"Structure: (1) parse flags from `args.iter().skip(1)`, (2) loop `nostd_rt::read(0, &mut buf)` until `Ok(0)`, (3) print."#,
                r#"Flag parsing with slice patterns: `[b'-', flags @ ..] if !flags.is_empty()` then loop over the flag bytes. If no flag was given, turn all three on."#,
                r#"Words across chunks: keep `in_word: bool` *outside* the read loop. A word starts when you see a non-whitespace byte while `in_word` is false."#,
                r#"Printing: `nostd_rt::print!("{value:>7}")` for each selected count, a single space between them, then `println!()`."#,
            ]),
            ex!("10_cli", "grep1", "grep, no libc", Mode::Bin { rt: true, cases: &[
                case!(args: ["no_std"], stdin: "core\nno_std rust\nstd\nNO_STD\n", stdout: Expect::Exact("no_std rust\n"), stderr: Expect::Exact(""), exit: 0),
                case!(args: ["-i", "no_std"], stdin: "core\nno_std rust\nstd\nNO_STD\n", stdout: Expect::Exact("no_std rust\nNO_STD\n"), stderr: Expect::Exact(""), exit: 0),
                case!(args: ["-ni", "std"], stdin: "core\nno_std rust\nstd\nNO_STD\n", stdout: Expect::Exact("2:no_std rust\n3:std\n4:NO_STD\n"), stderr: Expect::Exact(""), exit: 0),
                case!(args: ["-v", "std"], stdin: "core\nno_std rust\nstd\nNO_STD\n", stdout: Expect::Exact("core\nNO_STD\n"), stderr: Expect::Exact(""), exit: 0),
                case!(args: ["-c", "-i", "std"], stdin: "core\nno_std rust\nstd\nNO_STD\n", stdout: Expect::Exact("3\n"), stderr: Expect::Exact(""), exit: 0),
                case!(args: ["zzz"], stdin: "core\nno_std rust\nstd\nNO_STD\n", stdout: Expect::Exact(""), stderr: Expect::Exact(""), exit: 1),
                case!(args: ["match"], stdin: "a\nmatch", stdout: Expect::Exact("match\n"), stderr: Expect::Exact(""), exit: 0),
                case!(args: [], stdin: "", stdout: Expect::Exact(""), stderr: Expect::Exact("usage: grep1 [-invc] PATTERN\n"), exit: 2),
                case!(args: ["std", "extra"], stdin: "", stdout: Expect::Exact(""), stderr: Expect::Exact("usage: grep1 [-invc] PATTERN\n"), exit: 2),
                case!(args: ["x"], stdin: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n", stdout: Expect::Exact(""), stderr: Expect::Exact("grep1: line 1 too long\n"), exit: 2),
            ] }, [
                r#"Parse: flags (`[b'-', flags @ ..]`) are accepted only before the pattern; the first other argument is the pattern; any argument after it → usage error."#,
                r#"Matching: `line.windows(pat.len()).any(|w| w == pat)`, and for -i use `w.eq_ignore_ascii_case(pat)`. Treat an empty pattern as matching everything (windows(0) would panic)."#,
                r#"Assemble lines byte by byte into `[u8; 1024]` with `line.get_mut(len)`: `None` means the line is too long. On `
`, process `&line[..len]` and reset len."#,
                r#"After EOF, process the leftover partial line if len > 0. A line is selected when `contains(..) != invert`. Exit 0/1 depending on whether anything was selected."#,
            ]),
            ex!("10_cli", "rpn1", "A panic-free RPN calculator", Mode::Bin { rt: true, cases: &[
                case!(args: [], stdin: "1 2 +\n3 4 * 5 -\n10 3 %\n-5 neg 2 *\n", stdout: Expect::Exact("3\n7\n1\n10\n"), stderr: Expect::Exact(""), exit: 0),
                case!(args: [], stdin: "1 +\n4 0 /\n9223372036854775807 1 +\n1 2\nfoo\n2 dup *\n", stdout: Expect::Exact("4\n"), stderr: Expect::Exact("rpn: line 1: stack underflow\nrpn: line 2: division by zero\nrpn: line 3: overflow\nrpn: line 4: expected 1 result, found 2\nrpn: line 5: bad token 'foo'\n"), exit: 1),
                case!(args: [], stdin: "1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1\n\n7 swap\n-9223372036854775808 -1 /\n3 4 swap -\n5 drop\n", stdout: Expect::Exact("1\n"), stderr: Expect::Exact("rpn: line 1: stack overflow\nrpn: line 3: stack underflow\nrpn: line 4: overflow\nrpn: line 6: expected 1 result, found 0\n"), exit: 1),
                case!(args: [], stdin: "1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 \n", stdout: Expect::Exact(""), stderr: Expect::Exact("rpn: line 1: line too long\n"), exit: 1),
                case!(args: [], stdin: "\n\n6 7 *", stdout: Expect::Exact("42\n"), stderr: Expect::Exact(""), exit: 0),
            ] }, [
                r#"Separate the concerns: a line assembler (fixed 256-byte buffer + too_long flag), an `eval(&str) -> Result<Option<i64>, Error>` for one line, and main which reports."#,
                r#"Stack: `struct Stack { items: [i64; 16], len: usize }` with `push` returning Err(StackOverflow) via `items.get_mut(len)`, and `pop` returning Err(Underflow) via `len.checked_sub(1)`."#,
                r#"Binary ops: pop b first, then a. Use `checked_add/sub/mul/div/rem`, check b == 0 before `/` and `%` (to report division by zero rather than overflow), and map `None` to Overflow. `i64::MIN / -1` overflows!"#,
                r#"Implement Display for your error enum to produce the exact messages, then `eprintln!("rpn: line {n}: {e}")`. Blank lines: eval returns Ok(None) and nothing is printed, but the line still counts."#,
            ]),
        ],
        quiz: None,
    },
    Module {
        num: 11,
        slug: "ffi",
        title: "Talking to C (FFI)",
        summary: "exported C APIs, opaque handles, callbacks and calling C from Rust",
        lesson: lesson!("11-ffi"),
        exercises: &[
            ex!("11_ffi", "ffi1", "Exporting a no_std library to C", Mode::CLib { harness: "ffi1_harness.c", cases: &[
                case!(args: [], stdin: "", stdout: Expect::Exact("crc32(\"123456789\") = cbf43926\ncrc32(empty) = 00000000\ncrc32(NULL, 5) = 00000000\nstats rc=0 min=-40 max=125 mean=25 count=6\nstats rc=0 min=-4 max=-3 mean=-3 count=2\nstats(NULL samples) = -1\nstats(NULL out) = -1\nstats(empty) = -2\nhex = deadbeef (8)\nhex(small buffer) = 0\nhex(NULL out) = 0\n"), stderr: Expect::Exact(""), exit: 0),
            ] }, [
                r#"Read ffi1_harness.c first: it is the specification. Each Rust function needs `#[unsafe(no_mangle)] pub extern "C" fn name(...)` with matching types (`*const u8` for `const uint8_t *`, `usize` for `size_t`, `*mut Stats` for `struct Stats *`)."#,
                r#"Check pointers with `.is_null()` before anything else, then `let s = unsafe { core::slice::from_raw_parts(ptr, len) };` and work with the safe slice."#,
                r#"CRC-32 (reflected): start with `0xFFFF_FFFF`; per byte `crc ^= byte` then 8 times `crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 }`; return `!crc`."#,
                r#"nostd_hex: compute `needed = 2 * len + 1` with checked arithmetic; return 0 if out_len < needed. Write two digits per byte from a `b"0123456789abcdef"` table, then a 0 byte, and return `needed - 1`."#,
            ], extra: [("ffi1_harness.c")]),
            ex!("11_ffi", "ffi2", "Opaque handles, caller storage and callbacks", Mode::CLib { harness: "ffi2_harness.c", cases: &[
                case!(args: [], stdin: "", stdout: Expect::Exact("size ok: 1, align ok: 1\ninit: ok\npush 10 -> rc=0 avg=10\npush 20 -> rc=0 avg=15\npush 30 -> rc=0 avg=20\npush 40 -> rc=0 avg=30\npush -100 -> rc=0 avg=-10\n  visit 0: 30\n  visit 1: 40\n  visit 2: -100\nfor_each -> 3 (sum=-30, visits=3)\ninit(NULL) = NULL\ninit(too small) = NULL\ninit(misaligned) = NULL\ninit(window 0) = NULL\ninit(window 9) = NULL\npush(NULL) = -1\nfor_each(NULL cb) = -1\n"), stderr: Expect::Exact(""), exit: 0),
            ] }, [
                r#"Filter::push: store at `next`, advance `next = (next + 1) % window`, grow `len` up to window, return `sum / len` (sum in i64). iter: the oldest sample sits at `(next + window - len) % window`."#,
                r#"Exported statics: `#[unsafe(no_mangle)] pub static NOSTD_FILTER_SIZE: usize = size_of::<Filter>();` (and ALIGN with align_of)."#,
                r#"init: `let p = storage.cast::<Filter>();` reject null, `storage_len < size_of::<Filter>()`, `!p.is_aligned()`, and windows outside 1..=8; then `unsafe { p.write(Filter { .. }) }` and return p."#,
                r#"`unsafe { f.as_mut() }` turns a nullable `*mut Filter` into `Option<&mut Filter>`. The callback type is `Option<unsafe extern "C" fn(*mut c_void, usize, i32)>`: match `Some(cb)` and call `unsafe { cb(user, i, v) }`."#,
            ], extra: [("ffi2_harness.c")]),
            ex!("11_ffi", "ffi3", "Calling C from Rust", Mode::CLib { harness: "ffi3_harness.c", cases: &[
                case!(args: [], stdin: "", stdout: Expect::Exact("[L1] poll start\n[L1] ch 0: 21.5 C at 100 ms\n[L1] ch 1: -4.0 C at 200 ms\n[L2] ch 7: read failed\n[L1] ch 2: 100.0 C at 300 ms\nrust_poll returned 3\nrust_poll(NULL) returned -1\n"), stderr: Expect::Exact(""), exit: 0),
            ] }, [
                r#"Declarations: `unsafe extern "C" { fn c_log(level: c_int, msg: *const c_char); fn c_read_sensor(channel: c_int, out: *mut i32) -> c_int; safe fn c_millis() -> u32; }`."#,
                r#"as_cstr: the buffer always contains a 0 byte after the text, so `CStr::from_bytes_until_nul(&self.buf).unwrap_or(c"")` works. log: `unsafe { c_log(level, msg.as_ptr()) }`."#,
                r#"rust_poll: null-check, build the slice, `log(1, c"poll start")`, then for each channel call `c_read_sensor(ch, &mut value)`. Format into a fresh `CBuf` with `write!`."#,
                r#"Negative tenths: print the sign separately and use `value.unsigned_abs()` for the digits: `write!(msg, "ch {ch}: {sign}{}.{} C at {} ms", abs / 10, abs % 10, c_millis())`."#,
            ], extra: [("ffi3_harness.c")]),
        ],
        quiz: quiz!("11-ffi"),
    },
    Module {
        num: 12,
        slug: "bare_metal",
        title: "Bare Metal: Targets, Linkers & Boot",
        summary: "targets, vector tables, linker scripts, reset handlers, semihosting and HardFaults",
        lesson: lesson!("12-bare-metal"),
        exercises: &[
            ex!("12_bare_metal", "boot1", "Startup code and fault decoding, on the host", Mode::Lib, [
                r#"init_memory: validate everything *before* touching RAM. Order of checks: inverted ranges, RAM bounds, flash bounds (`flash.get(sidata..sidata.checked_add(n)?)`), then overlap."#,
                r#"Two non-empty ranges [a, b) and [c, d) overlap exactly when `a < d && c < b`. Then `copy_from_slice` the image and `fill(0)` the bss range."#,
                r#"vector_table: `stack_top % 8 != 0` → error. Word i+2 comes from handlers[i]; check `RESERVED.contains(&word)` first, then `h.unwrap_or(default) | 1`."#,
                r#"decode_ipsr: `match (ipsr & 0x1FF) as u16 { 0 => ThreadMode, 2 => Nmi, ..., n @ 16.. => Irq(n - 16), n => Reserved(n) }`. parse_frame: `*stack.first_chunk::<8>()?` destructures into eight words. fault_reasons: filter CFSR_BITS by `cfsr & (1 << bit) != 0` and zip into `out`."#,
            ]),
            ex!("12_bare_metal", "cortexm1", "Boot a Cortex-M from reset, by hand", Mode::CortexM { link: "cortexm1.x", stdout: "Hello from Cortex-M3!\n.data ok: 0xc0ffee\n.bss ok: 0x0\n" }, [
                r#"Reset vector: `#[unsafe(link_section = ".vector_table.reset_vector")] #[unsafe(no_mangle)] pub static __RESET_VECTOR: unsafe extern "C" fn() -> ! = Reset;`"#,
                r#"Exceptions: `#[unsafe(link_section = ".vector_table.exceptions")] #[unsafe(no_mangle)] pub static __EXCEPTIONS: [Vector; 14] = [ ... ];` Index 0 is word 2 (NMI), index 1 is word 3 (HardFault); reserved words 7-10 and 13 are indices 5-8 and 11: `Vector { reserved: 0 }`."#,
                r#"Zero .bss: `let mut dst = &raw mut _sbss; while dst < &raw mut _ebss { ptr::write_volatile(dst, 0); dst = dst.add(1); }`, all in one unsafe block."#,
                r#"Copy .data: walk `dst` from `&raw mut _sdata` to `&raw mut _edata` and `src` from `&raw const _sidata`, `ptr::write_volatile(dst, ptr::read(src))`. Handlers: `semihosting::write_str("HardFault!\n"); semihosting::exit(false)`."#,
            ], extra: [("cortexm1.x")]),
        ],
        quiz: quiz!("12-bare-metal"),
    },
    Module {
        num: 13,
        slug: "hal",
        title: "Drivers & the embedded-hal Model",
        summary: "generic drivers over OutputPin, DelayNs, I2c and SpiDevice, tested with mocks",
        lesson: lesson!("13-hal"),
        exercises: &[
            ex!("13_hal", "hal1", "Generic drivers for pins and delays", Mode::Lib, [
                r#"blink: `for _ in 0..times { self.pin.set_high()?; self.delay.delay_ms(on_ms); self.pin.set_low()?; self.delay.delay_ms(off_ms); } Ok(())`. The `?` returns the pin's error."#,
                r#"show: `for (i, led) in self.leds.iter_mut().enumerate() { led.set_state(if i < level { PinState::High } else { PinState::Low })?; }`."#,
                r#"poll: sample once. If it equals the stable state, reset the counter and return None. Otherwise increment; when the counter reaches THRESHOLD, flip the stable state, reset the counter and return the edge."#,
            ]),
            ex!("13_hal", "hal2", "An I2C temperature sensor driver", Mode::Lib, [
                r#"Write a helper `fn read_reg<const N: usize>(&mut self, reg: u8) -> Result<[u8; N], Error<I2C::Error>>` that calls `self.i2c.write_read(self.address, &[reg], &mut buf).map_err(Error::Bus)?`."#,
                r#"init: `let [id] = self.read_reg::<1>(0x0F)?;` and compare with 0xA1. temperature: `i16::from_be_bytes(self.read_reg::<2>(0x00)?)`, then `i32::from(raw) * 1000 / 256`."#,
                r#"set_shutdown: read CONFIG, set or clear only bit 0 (`cfg | 0x01` / `cfg & !0x01`), then `self.i2c.write(self.address, &[0x01, new]).map_err(Error::Bus)`."#,
            ]),
            ex!("13_hal", "hal3", "An SPI CAN-controller driver", Mode::Lib, [
                r#"Single-write commands: `self.spi.write(&[CMD_RESET]).map_err(Error::Spi)`, similarly WRITE `[0x02, addr, value]` and BIT MODIFY `[0x05, addr, mask, data]`."#,
                r#"read_register needs write-then-read inside ONE transaction (CS stays low): `self.spi.transaction(&mut [Operation::Write(&[CMD_READ, addr]), Operation::Read(&mut buf)])`."#,
                r#"set_mode: `self.bit_modify(CANCTRL, 0xE0, (mode as u8) << 5)?`, then compare `self.mode()?` with `mode as u8`. set_bit_timing: check `self.mode()? == Mode::Configuration as u8` first, then write `[0x02, 0x28, cnf3, cnf2, cnf1]`."#,
            ]),
        ],
        quiz: quiz!("13-hal"),
    },
];

#[allow(dead_code)]
const _USE: (Option<Case>, Option<Expect>) = (None, None);
