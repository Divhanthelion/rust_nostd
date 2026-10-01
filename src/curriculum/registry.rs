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
];

#[allow(dead_code)]
const _USE: (Option<Case>, Option<Expect>) = (None, None);
