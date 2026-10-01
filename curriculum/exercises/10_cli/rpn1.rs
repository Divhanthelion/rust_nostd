//! # rpn1: A panic-free RPN calculator
//!
//! Each line of standard input is an expression in Reverse Polish Notation:
//! `3 4 + 2 *` means (3 + 4) * 2. Evaluate every line and print its result.
//!
//! Tokens (separated by ASCII whitespace):
//! - integers (`i64`, optional leading `-`): push;
//! - `+ - * / %`: pop b, pop a, push a∘b (`/` and `%` truncate like Rust);
//! - `neg` (negate), `dup` (duplicate top), `swap` (swap top two), `drop` (pop).
//!
//! Rules:
//! - The stack holds **at most 16** values (a fixed array, no heap).
//! - Arithmetic is **checked**: any overflow is an error, as is dividing by 0.
//! - Blank lines are skipped (no output). After a non-blank line the stack must
//!   hold exactly one value: print it on stdout.
//! - Errors go to **stderr** as `rpn: line <N>: <message>` (N counts every line,
//!   blank ones included), then continue with the next line. Messages:
//!   `stack underflow`, `stack overflow`, `division by zero`, `overflow`,
//!   `bad token '<tok>'`, `expected 1 result, found <k>`.
//! - Exit status: 0 if every line succeeded, 1 otherwise.
//! - Lines are at most 256 bytes; a longer line is reported as
//!   `rpn: line <N>: line too long` and skipped. (Note: once you've decided a
//!   line is too long, keep reading until its newline: that's still the same line.)
//!
//! It must never panic, whatever the input.
#![no_std]
#![no_main]

extern crate nostd_rt;

use core::fmt;
use nostd_rt::{eprintln, println, Args};

fn main(_args: Args) -> i32 {
    // TODO: read lines (fixed 256-byte buffer), evaluate each with a fixed
    // 16-slot stack and checked arithmetic, report results and errors.
    eprintln!("rpn1: not implemented yet");
    99
}

nostd_rt::entry!(main);

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    eprintln!("{info}");
    nostd_rt::exit(101)
}
