//! `nostd where <item>`: where does a familiar std item live in no_std land?
//!
//! Every `Core`/`Alloc` path below is compiled by `nostd dev verify` against
//! the core-only sysroot, so the table cannot silently rot.

use crate::markdown;
use crate::term;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Avail {
    /// In `core`: always available.
    Core,
    /// In `alloc`: needs `extern crate alloc` and a global allocator.
    Alloc,
    /// Only in `std`: needs an operating system.
    Std,
    /// Not in the standard library at all; an ecosystem crate.
    Crate,
}

pub struct Entry {
    pub names: &'static [&'static str],
    pub avail: Avail,
    /// A `use`-able path (checked for Core/Alloc), or a description for methods.
    pub path: &'static str,
    pub note: &'static str,
}

macro_rules! e {
    ([$($n:literal),+], $a:ident, $p:literal, $note:literal) => {
        Entry { names: &[$($n),+], avail: Avail::$a, path: $p, note: $note }
    };
}

pub const TABLE: &[Entry] = &[
    // ---- prelude-ish basics
    e!(["Option", "Some", "None"], Core, "core::option::Option", "In the core prelude: no import needed."),
    e!(["Result", "Ok", "Err"], Core, "core::result::Result", "In the core prelude: no import needed."),
    e!(["Iterator", "IntoIterator", "DoubleEndedIterator", "ExactSizeIterator"], Core, "core::iter::Iterator", "All adapters (map, filter, zip, fold, take, chain…) are in core. Only `collect()` into heap containers needs alloc; collect into arrays isn't possible directly, use `core::array::from_fn` or fill a buffer."),
    e!(["from_fn", "successors", "repeat", "once", "empty", "repeat_n"], Core, "core::iter::from_fn", "Iterator constructors live in core::iter. (`core::array::from_fn` builds arrays.)"),
    e!(["array::from_fn"], Core, "core::array::from_fn", "Build `[T; N]` from a closure over the index: the no-heap replacement for collect()."),
    // ---- formatting
    e!(["fmt", "Display", "Debug", "Formatter", "Arguments"], Core, "core::fmt::Display", "All of the formatting machinery is in core. Only the *destinations* (String, stdout) are missing."),
    e!(["fmt::Write", "Write (fmt)"], Core, "core::fmt::Write", "Implement `write_str` for your buffer/UART and you get write!/writeln! for free (exercise fmt1)."),
    e!(["write!", "writeln!", "format_args!"], Core, "core::write", "Macros in core. `format_args!` builds a lazily-formatted `fmt::Arguments` without allocating."),
    e!(["format!"], Alloc, "alloc::format", "Returns a String, so it needs alloc. No-heap: write! into a fixed buffer."),
    e!(["println!", "print!", "eprintln!", "eprint!", "dbg!"], Std, "std::println", "Needs an OS stdout. No-heap/no-OS: implement core::fmt::Write over a UART/syscall/RTT and use writeln!, or use `defmt` on microcontrollers (exercise print1)."),
    e!(["ToString", "to_string"], Alloc, "alloc::string::ToString", "Allocates a String. Prefer writing into a buffer with write!."),
    // ---- memory
    e!(["mem", "size_of", "align_of", "swap", "replace", "take", "forget", "transmute", "offset_of!"], Core, "core::mem::size_of", "core::mem has the whole toolkit; `offset_of!` is stable since 1.77."),
    e!(["MaybeUninit"], Core, "core::mem::MaybeUninit", "Uninitialised storage for fixed-capacity containers (exercise stackvec1)."),
    e!(["ManuallyDrop"], Core, "core::mem::ManuallyDrop", "Suppress automatic drop; useful in containers and FFI."),
    e!(["ptr", "NonNull", "null", "null_mut", "read_volatile", "write_volatile", "copy_nonoverlapping", "read_unaligned", "addr_of!"], Core, "core::ptr::NonNull", "Raw-pointer tools including volatile access for MMIO (exercise mmio1). Prefer `&raw const x` / `&raw mut x` (1.82) over addr_of!."),
    e!(["slice::from_raw_parts", "from_raw_parts"], Core, "core::slice::from_raw_parts", "Build a slice from pointer + length; the caller vouches for validity."),
    e!(["Box"], Alloc, "alloc::boxed::Box", "Needs a global allocator. `Box::leak` turns a boxed value into a `&'static mut` (common at init time)."),
    e!(["Vec", "vec!"], Alloc, "alloc::vec::Vec", "Needs a global allocator. Fixed-capacity alternatives: heapless::Vec<T, N>, arrays + length, or the StackVec you write in stackvec1."),
    e!(["String"], Alloc, "alloc::string::String", "Needs a global allocator. Fixed-capacity alternatives: heapless::String<N>, or a byte buffer implementing fmt::Write (exercise fmt1)."),
    e!(["Rc", "Weak"], Alloc, "alloc::rc::Rc", "Single-threaded reference counting, needs alloc."),
    e!(["Arc"], Alloc, "alloc::sync::Arc", "Atomic refcounting: needs alloc *and* pointer-sized atomics (`cfg(target_has_atomic = \"ptr\")`), so not on thumbv6m without portable-atomic tricks."),
    e!(["Cow"], Alloc, "alloc::borrow::Cow", "Needs alloc (the Owned variant allocates)."),
    e!(["Borrow", "BorrowMut"], Core, "core::borrow::Borrow", "The traits are in core; ToOwned is in alloc."),
    e!(["ToOwned", "to_owned", "to_vec"], Alloc, "alloc::borrow::ToOwned", "Producing owned heap copies needs alloc."),
    e!(["GlobalAlloc", "Layout"], Core, "core::alloc::GlobalAlloc", "The allocator *interface* lives in core; you implement it and register with #[global_allocator] (exercise alloc2)."),
    e!(["alloc::alloc", "dealloc", "handle_alloc_error"], Alloc, "alloc::alloc::alloc", "The global allocation entry points live in alloc."),
    // ---- collections
    e!(["VecDeque"], Alloc, "alloc::collections::VecDeque", "No-heap: heapless::Deque or a ring buffer (exercise ring1)."),
    e!(["BTreeMap", "BTreeSet"], Alloc, "alloc::collections::BTreeMap", "The ordered maps are in alloc and need no randomness: the go-to map type for no_std + alloc."),
    e!(["BinaryHeap"], Alloc, "alloc::collections::BinaryHeap", "No-heap: heapless::BinaryHeap."),
    e!(["LinkedList"], Alloc, "alloc::collections::LinkedList", "Rarely the right choice; intrusive lists or index-based pools are common in firmware."),
    e!(["HashMap", "HashSet"], Std, "std::collections::HashMap", "Not in alloc: the default hasher (RandomState) needs OS randomness for HashDoS resistance. Use alloc::collections::BTreeMap, the `hashbrown` crate (no_std), or heapless::IndexMap / FnvIndexMap."),
    e!(["Hash", "Hasher", "BuildHasher"], Core, "core::hash::Hasher", "The hashing traits are in core; there is just no randomly-keyed default hasher."),
    e!(["sort", "sort_by", "sort_by_key"], Alloc, "slice::sort (stable sort)", "The *stable* slice sorts allocate a scratch buffer, so they need alloc. `sort_unstable*` are in core and sort in place."),
    e!(["sort_unstable", "sort_unstable_by", "select_nth_unstable", "binary_search"], Core, "slice::sort_unstable", "In-place, allocation-free: available in core."),
    e!(["concat", "join", "repeat"], Alloc, "slice::concat / str::repeat", "These build new owned collections, so they need alloc."),
    // ---- strings & text
    e!(["str", "from_utf8", "from_utf8_unchecked", "Utf8Error"], Core, "core::str::from_utf8", "&str and all its non-allocating methods are in core."),
    e!(["FromStr", "parse"], Core, "core::str::FromStr", "Parsing integers *and floats* from &str works in core."),
    e!(["to_uppercase", "to_lowercase"], Alloc, "str::to_uppercase", "Return a new String. In place without alloc: `make_ascii_uppercase` on `&mut str`/`&mut [u8]` (core)."),
    e!(["make_ascii_uppercase", "make_ascii_lowercase", "eq_ignore_ascii_case", "is_ascii_digit"], Core, "u8::make_ascii_uppercase", "ASCII helpers are in core and never allocate."),
    e!(["char", "char::from_u32", "char::from_digit", "to_digit", "encode_utf8"], Core, "core::char::from_u32", "Everything char is core. `encode_utf8` writes into a caller buffer."),
    e!(["from_utf8_lossy"], Alloc, "String::from_utf8_lossy", "Returns a Cow<str>, so alloc. No-heap: `core::str::Utf8Chunks` via `<[u8]>::utf8_chunks()`."),
    e!(["ascii"], Core, "core::ascii::escape_default", "ASCII escaping helpers live in core::ascii."),
    // ---- numbers
    e!(["checked_add", "wrapping_add", "saturating_add", "overflowing_add", "pow", "ilog2", "isqrt", "abs_diff", "rem_euclid (integers)"], Core, "integer methods", "All integer arithmetic is in core. Choose overflow behaviour explicitly in safety-related code (exercise overflow1)."),
    e!(["NonZero", "NonZeroU32", "NonZeroUsize"], Core, "core::num::NonZero", "Niche-optimised: Option<NonZero<u32>> is 4 bytes."),
    e!(["Wrapping", "Saturating"], Core, "core::num::Wrapping", "Newtypes that make the overflow policy part of the type."),
    e!(["ParseIntError", "ParseFloatError", "TryFromIntError"], Core, "core::num::ParseIntError", "Error types of number parsing/conversion live in core::num."),
    e!(["abs", "signum", "copysign", "min", "max", "clamp", "to_bits", "from_bits", "total_cmp", "is_nan", "to_radians", "recip"], Core, "f32/f64 methods", "These float methods are available in core (abs/signum/copysign moved to core in 1.84)."),
    e!(["sqrt", "sin", "cos", "tan", "powf", "powi", "exp", "ln", "log10", "floor", "ceil", "round", "trunc", "mul_add", "hypot", "atan2", "rem_euclid (floats)"], Std, "f32::sqrt etc.", "Float math that needs a libm is std-only on stable (core's versions are behind the unstable `core_float_math` feature). Use the `libm` crate (libm::sqrtf), fixed-point math (exercise fixed1), or integer algorithms like `u32::isqrt`."),
    // ---- traits / language support
    e!(["Add", "Sub", "Mul", "Deref", "DerefMut", "Drop", "Fn", "FnMut", "FnOnce", "Index", "Range", "RangeInclusive"], Core, "core::ops::Deref", "Operator traits are core."),
    e!(["Ordering (cmp)", "PartialOrd", "Ord", "Reverse", "cmp::min", "cmp::max"], Core, "core::cmp::Ordering", "Comparison is core."),
    e!(["From", "Into", "TryFrom", "TryInto", "AsRef", "AsMut", "Infallible"], Core, "core::convert::TryFrom", "Conversion traits are core; TryFrom/TryInto are in the 2021+ prelude."),
    e!(["PhantomData", "Send", "Sync", "Copy", "Sized", "Unpin", "PhantomPinned"], Core, "core::marker::PhantomData", "Marker types/traits are core. PhantomData powers type-state APIs (exercise typestate1)."),
    e!(["Any", "TypeId", "type_name"], Core, "core::any::Any", "Reflection basics are core."),
    e!(["Error", "error::Error"], Core, "core::error::Error", "Stable in core since 1.81, so no_std libraries can implement the standard Error trait."),
    e!(["Box<dyn Error>"], Alloc, "alloc::boxed::Box", "Boxing errors needs alloc. In firmware prefer concrete error enums."),
    e!(["panic!", "assert!", "assert_eq!", "debug_assert!", "unreachable!", "todo!", "unimplemented!"], Core, "core::panic", "Panicking macros are in core; *what happens* on panic is decided by your #[panic_handler]."),
    e!(["matches!", "concat!", "stringify!", "include_bytes!", "include_str!", "env!", "cfg!", "line!", "file!", "column!", "module_path!", "compile_error!"], Core, "core::matches", "Built-in macros are all available."),
    e!(["PanicInfo", "Location"], Core, "core::panic::PanicInfo", "Passed to your #[panic_handler]. `info.location()` and `info.message()` work without allocation (exercise panic1)."),
    e!(["catch_unwind", "resume_unwind", "panic::set_hook", "take_hook"], Std, "std::panic::catch_unwind", "There is no unwinding in no_std (panic=abort). Panic policy = your #[panic_handler]: log, enter a safe state, reset."),
    e!(["hint", "black_box", "spin_loop", "unreachable_unchecked", "assert_unchecked"], Core, "core::hint::spin_loop", "Optimiser hints; spin_loop() emits a PAUSE/YIELD-style instruction in busy-wait loops."),
    e!(["asm!", "global_asm!", "naked_asm!"], Core, "core::arch::asm", "Inline/global assembly is in core::arch (naked functions stable since 1.88)."),
    // ---- cells & sync
    e!(["Cell", "RefCell", "UnsafeCell"], Core, "core::cell::RefCell", "Interior mutability is core. None of these are Sync: you can't put them in a `static` directly."),
    e!(["OnceCell", "LazyCell"], Core, "core::cell::OnceCell", "Single-threaded lazy init (not Sync)."),
    e!(["OnceLock", "LazyLock", "Once"], Std, "std::sync::OnceLock", "Need blocking (OS) for the losing thread. no_std: an atomic state machine (exercise once1), `critical_section`-based cells, or `static_cell::StaticCell` for init-once ownership."),
    e!(["Mutex", "RwLock", "Condvar", "Barrier"], Std, "std::sync::Mutex", "OS-backed blocking primitives. no_std: `critical_section::Mutex<RefCell<T>>` (interrupt-safe), embassy-sync's async Mutex, RTIC resources, or a spinlock (exercise spinlock1) when there are real parallel cores."),
    e!(["AtomicBool", "AtomicU8", "AtomicU32", "AtomicUsize", "AtomicPtr", "fence", "compiler_fence"], Core, "core::sync::atomic::AtomicU32", "Atomics are core, but availability depends on the target: check `cfg(target_has_atomic = \"32\")`. thumbv6m (Cortex-M0) has load/store but no compare-and-swap; use `portable-atomic` there."),
    e!(["Ordering (atomic)", "Relaxed", "Acquire", "Release", "AcqRel", "SeqCst"], Core, "core::sync::atomic::Ordering", "Memory orderings for atomics (exercise atomics1)."),
    e!(["mpsc", "channel", "sync_channel"], Std, "std::sync::mpsc", "no_std: lock-free SPSC queues (exercise spsc1, heapless::spsc), embassy_sync::channel::Channel."),
    e!(["thread::spawn", "thread", "JoinHandle", "scope"], Std, "std::thread::spawn", "No threads without an OS. Concurrency comes from interrupts, RTOS tasks, RTIC, or async executors like Embassy."),
    e!(["thread_local!"], Std, "std::thread_local", "Use statics with interior mutability; on multicore MCUs, per-core data indexed by core id."),
    e!(["sleep", "thread::sleep"], Std, "std::thread::sleep", "Use a hardware timer, `embedded_hal::delay::DelayNs`, or an async timer (embassy_time::Timer)."),
    // ---- time
    e!(["Duration"], Core, "core::time::Duration", "Duration is core (it's just numbers)."),
    e!(["Instant", "SystemTime", "UNIX_EPOCH"], Std, "std::time::Instant", "Needs an OS clock. Use a hardware timer/tick counter, `embassy_time::Instant`, or the `fugit` crate's tick-based types. Compare tick counts with wrapping arithmetic (exercise overflow1)."),
    // ---- async
    e!(["Future", "IntoFuture", "ready", "poll_fn", "pending"], Core, "core::future::Future", "The Future trait and helpers are core; *executors* are not (exercise async1)."),
    e!(["Poll", "Context", "Waker", "RawWaker", "RawWakerVTable", "Waker::noop"], Core, "core::task::Waker", "Task wake-up plumbing is core. Waker::noop is stable since 1.85."),
    e!(["Pin", "pin!"], Core, "core::pin::Pin", "Pinning is core; `pin!` pins on the stack without Box."),
    // ---- FFI
    e!(["CStr", "c_char", "c_int", "c_void", "c_uint", "c_long"], Core, "core::ffi::CStr", "C types and CStr are core (1.64). c\"text\" literals produce &CStr (1.77)."),
    e!(["CString"], Alloc, "alloc::ffi::CString", "Owned C strings allocate."),
    e!(["OsString", "OsStr", "Path", "PathBuf"], Std, "std::path::Path", "OS-specific; firmware deals in bytes and &str."),
    // ---- net
    e!(["Ipv4Addr", "Ipv6Addr", "SocketAddr", "IpAddr"], Core, "core::net::Ipv4Addr", "Address types moved to core::net in 1.77."),
    e!(["TcpStream", "TcpListener", "UdpSocket"], Std, "std::net::TcpStream", "no_std networking: smoltcp, embassy-net."),
    // ---- OS interfaces
    e!(["File", "fs", "OpenOptions", "read_to_string"], Std, "std::fs::File", "No filesystem. Firmware stores data in flash via a HAL (embedded-storage traits) or a filesystem crate like littlefs2."),
    e!(["io::Read", "io::Write", "BufReader", "stdin", "stdout", "Read", "Write (io)"], Std, "std::io::Write", "std::io isn't in core/alloc. no_std: the `embedded-io` traits, core::fmt::Write for text, or raw syscalls (exercises stdin1/print1)."),
    e!(["env::args", "args", "env::var", "vars"], Std, "std::env::args", "No runtime gathers them for you. On Linux, read argc/argv from the initial stack (exercise args1)."),
    e!(["process::exit", "exit", "abort", "ExitCode", "Command"], Std, "std::process::exit", "Exit via the exit_group syscall (exercise start1), semihosting, or a system reset on a microcontroller."),
    e!(["Backtrace"], Std, "std::backtrace::Backtrace", "Needs unwind tables + symbolication. Use panic locations, defmt, or a debugger (probe-rs)."),
    // ---- ecosystem crates
    e!(["heapless", "heapless::Vec", "heapless::String"], Crate, "heapless", "Fixed-capacity Vec/String/Deque/IndexMap/spsc::Queue with const-generic capacity (0.9.x). `push` returns Err when full."),
    e!(["critical_section", "critical-section", "CriticalSection"], Crate, "critical_section::Mutex", "Portable 'disable interrupts' abstraction; `critical_section::with(|cs| ...)` + `Mutex<RefCell<T>>` (exercise cs1)."),
    e!(["portable-atomic", "portable_atomic"], Crate, "portable_atomic::AtomicU32", "Atomics (including CAS) on targets without native support, e.g. thumbv6m, via critical sections."),
    e!(["static_cell", "StaticCell"], Crate, "static_cell::StaticCell", "Initialise a `&'static mut T` exactly once at runtime (common with Embassy)."),
    e!(["embedded-hal", "embedded_hal", "OutputPin", "InputPin", "SpiDevice", "SpiBus", "I2c", "DelayNs"], Crate, "embedded_hal::digital::OutputPin", "embedded-hal 1.0 traits: write drivers once, run on any MCU (exercises hal1/hal2)."),
    e!(["embedded-can", "embedded_can", "Frame", "StandardId", "ExtendedId"], Crate, "embedded_can::Frame", "CAN frame/ID traits shared by bxcan, fdcan, socketcan… (exercise can1)."),
    e!(["defmt", "defmt::info!"], Crate, "defmt::info", "Deferred formatting: logs are encoded as indexes + args and formatted on the host. Tiny and fast; pairs with probe-rs."),
    e!(["libm", "sqrtf", "sinf"], Crate, "libm::sqrtf", "Pure-Rust port of musl's libm for no_std float math."),
    e!(["cortex-m-rt", "cortex_m_rt", "entry", "#[entry]", "exception"], Crate, "cortex_m_rt::entry", "Startup runtime for Cortex-M: vector table, .data/.bss init, `#[entry]`, `#[exception]` and memory.x (exercise cortexm1 builds one by hand)."),
    e!(["embassy", "embassy_executor", "Spawner"], Crate, "embassy_executor::Spawner", "Async executor + HALs for MCUs; tasks are statically allocated."),
    e!(["rtic", "RTIC"], Crate, "rtic::app", "Real-Time Interrupt-driven Concurrency: hardware-scheduled tasks with compile-time-checked resource sharing (priority ceiling protocol)."),
    e!(["embedded-alloc", "embedded_alloc", "LlffHeap", "TlsfHeap"], Crate, "embedded_alloc::LlffHeap", "A #[global_allocator] for MCUs over a static region (exercise alloc2 builds a simpler one)."),
];

pub fn lookup(query: &str) -> Vec<&'static Entry> {
    let q = normalize(query);
    if q.is_empty() {
        return Vec::new();
    }
    let exact: Vec<&Entry> = TABLE
        .iter()
        .filter(|e| e.names.iter().any(|n| normalize(n) == q) || normalize(last_segment(e.path)) == q)
        .collect();
    if !exact.is_empty() {
        return exact;
    }
    TABLE
        .iter()
        .filter(|e| e.names.iter().any(|n| normalize(n).contains(&q)) || normalize(e.path).contains(&q))
        .collect()
}

fn last_segment(p: &str) -> &str {
    p.rsplit("::").next().unwrap_or(p)
}

fn normalize(s: &str) -> String {
    let s = s.trim().to_ascii_lowercase();
    let s = s
        .trim_start_matches("std::")
        .trim_start_matches("core::")
        .trim_start_matches("alloc::")
        .trim_end_matches("!")
        .trim_end_matches("()");
    s.to_string()
}

pub fn badge(a: Avail) -> String {
    match a {
        Avail::Core => term::paint(" CORE ", "1;30;42"),
        Avail::Alloc => term::paint(" ALLOC ", "1;30;43"),
        Avail::Std => term::paint(" STD ONLY ", "1;37;41"),
        Avail::Crate => term::paint(" CRATE ", "1;30;46"),
    }
}

pub fn print(entries: &[&Entry]) {
    let width = term::text_width();
    for e in entries {
        println!();
        println!("  {}  {}", badge(e.avail), term::bold(&e.names.join(", ")));
        println!("     {} {}", term::dim("path:"), term::cyan(e.path));
        let needs = match e.avail {
            Avail::Core => "nothing: core is always there",
            Avail::Alloc => "`extern crate alloc;` + a #[global_allocator] in the final binary",
            Avail::Std => "an operating system (not available in no_std)",
            Avail::Crate => "adding the crate to Cargo.toml (with default-features = false where relevant)",
        };
        println!("     {} {}", term::dim("needs:"), markdown::inline(needs));
        let wrapped = term::wrap_plain(&term::strip_ansi(&markdown::inline(e.note)), width.saturating_sub(5), "     ");
        print!("{wrapped}");
    }
    println!();
}

/// Generate a source file that imports every checkable Core/Alloc path.
pub fn verification_source() -> String {
    let mut s = String::from("#![no_std]\n#![allow(unused_imports, deprecated)]\nextern crate alloc;\n");
    for (i, e) in TABLE.iter().enumerate() {
        let checkable = e.path.starts_with("core::") || e.path.starts_with("alloc::");
        if matches!(e.avail, Avail::Core | Avail::Alloc) && checkable && !e.path.contains(' ') {
            s.push_str(&format!("use {} as __item{i};\n", e.path));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_common_items() {
        assert_eq!(lookup("Vec")[0].avail, Avail::Alloc);
        assert_eq!(lookup("HashMap")[0].avail, Avail::Std);
        assert_eq!(lookup("std::fmt::Write")[0].avail, Avail::Core);
        assert_eq!(lookup("println!")[0].avail, Avail::Std);
        assert!(lookup("sqrt").iter().any(|e| e.avail == Avail::Std));
        assert!(!lookup("atomic").is_empty());
    }
}
