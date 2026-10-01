//! # alloc1: The alloc toolbox
//!
//! With `extern crate alloc;` a no_std crate gets `Vec`, `String`, `Box`,
//! `Rc`, `BTreeMap` and `format!`. The final binary supplies the allocator; in
//! the tests, the host's allocator is used.
//!
//! Build a small CAN-bus traffic analyser:
//! - `summarize`: per-id statistics in a `BTreeMap` (ordered by id).
//! - `render`: one line per id, e.g. `0x101 count=2 bytes=16 max=8\n` (ids as
//!   `{:#05x}`), using `alloc::format!` or `write!` into a `String`.
//! - `busiest`: the `n` ids with the highest count, ties broken by lower id.
//!   (Sort a `Vec`; note that the stable `sort_by` needs alloc,
//!   `sort_unstable_by` doesn't.)
//! - `id_range`/`min_len`: filters as boxed closures; `passes` = all accept.
//! - `SharedLog`: two handles to one log via `Rc<RefCell<Vec<String>>>`.
//!
//! Start by adding `extern crate alloc;` and the imports you need.
#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub id: u16,
    pub len: u8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub count: u32,
    pub bytes: u32,
    pub max_len: u8,
}

pub fn summarize(frames: &[Frame]) -> BTreeMap<u16, Stats> {
    let mut map: BTreeMap<u16, Stats> = BTreeMap::new();
    for f in frames {
        let s = map.entry(f.id).or_default();
        s.count += 1;
        s.bytes += u32::from(f.len);
        s.max_len = s.max_len.max(f.len);
    }
    map
}

pub fn render(stats: &BTreeMap<u16, Stats>) -> String {
    let mut out = String::new();
    for (id, s) in stats {
        let _ = writeln!(out, "{:#05x} count={} bytes={} max={}", id, s.count, s.bytes, s.max_len);
    }
    out
}

pub fn busiest(stats: &BTreeMap<u16, Stats>, n: usize) -> Vec<(u16, u32)> {
    let mut v: Vec<(u16, u32)> = stats.iter().map(|(id, s)| (*id, s.count)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.truncate(n);
    v
}

pub type Filter = Box<dyn Fn(&Frame) -> bool>;

/// Accept frames whose id is in `lo..=hi`.
pub fn id_range(lo: u16, hi: u16) -> Filter {
    Box::new(move |f| (lo..=hi).contains(&f.id))
}

/// Accept frames with at least `n` data bytes.
pub fn min_len(n: u8) -> Filter {
    Box::new(move |f| f.len >= n)
}

pub fn passes(filters: &[Filter], frame: &Frame) -> bool {
    filters.iter().all(|f| f(frame))
}

/// A log that several components can append to.
#[derive(Clone)]
pub struct SharedLog {
    lines: Rc<RefCell<Vec<String>>>,
    prefix: &'static str,
}

impl SharedLog {
    pub fn new(prefix: &'static str) -> Self {
        SharedLog { lines: Rc::new(RefCell::new(Vec::new())), prefix }
    }

    /// Another handle to the *same* log, with a different prefix.
    pub fn with_prefix(&self, prefix: &'static str) -> Self {
        SharedLog { lines: Rc::clone(&self.lines), prefix }
    }

    /// Append `"<prefix>: <msg>"`.
    pub fn log(&self, msg: &str) {
        self.lines.borrow_mut().push(alloc::format!("{}: {}", self.prefix, msg));
    }

    pub fn lines(&self) -> Vec<String> {
        self.lines.borrow().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn traffic() -> [Frame; 6] {
        [
            Frame { id: 0x200, len: 8 },
            Frame { id: 0x101, len: 8 },
            Frame { id: 0x101, len: 8 },
            Frame { id: 0x7DF, len: 3 },
            Frame { id: 0x200, len: 2 },
            Frame { id: 0x050, len: 1 },
        ]
    }

    #[test]
    fn summarizes() {
        let s = summarize(&traffic());
        assert_eq!(s.len(), 4);
        assert_eq!(s[&0x101], Stats { count: 2, bytes: 16, max_len: 8 });
        assert_eq!(s[&0x200], Stats { count: 2, bytes: 10, max_len: 8 });
        assert!(summarize(&[]).is_empty());
    }

    #[test]
    fn renders_in_id_order() {
        let s = summarize(&traffic());
        assert_eq!(
            render(&s),
            "0x050 count=1 bytes=1 max=1\n\
             0x101 count=2 bytes=16 max=8\n\
             0x200 count=2 bytes=10 max=8\n\
             0x7df count=1 bytes=3 max=3\n"
        );
    }

    #[test]
    fn busiest_ids() {
        let s = summarize(&traffic());
        assert_eq!(busiest(&s, 3), [(0x101, 2), (0x200, 2), (0x050, 1)]);
        assert_eq!(busiest(&s, 10).len(), 4);
        assert!(busiest(&s, 0).is_empty());
    }

    #[test]
    fn filters() {
        let fs = [id_range(0x100, 0x2FF), min_len(4)];
        let accepted: Vec<u16> = traffic().iter().filter(|f| passes(&fs, f)).map(|f| f.id).collect();
        assert_eq!(accepted, [0x200, 0x101, 0x101]);
        assert!(passes(&[], &Frame { id: 1, len: 0 }));
    }

    #[test]
    fn shared_log() {
        let engine = SharedLog::new("engine");
        let brakes = engine.with_prefix("brakes");
        engine.log("start");
        brakes.log("pressure ok");
        engine.log("idle");
        let expected = ["engine: start", "brakes: pressure ok", "engine: idle"];
        assert_eq!(engine.lines(), expected);
        assert_eq!(brakes.lines(), expected);
    }
}
