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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Error<'a> {
    Underflow,
    StackOverflow,
    DivByZero,
    Overflow,
    BadToken(&'a str),
    Leftover(usize),
    TooLong,
}

impl fmt::Display for Error<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Underflow => f.write_str("stack underflow"),
            Error::StackOverflow => f.write_str("stack overflow"),
            Error::DivByZero => f.write_str("division by zero"),
            Error::Overflow => f.write_str("overflow"),
            Error::BadToken(t) => write!(f, "bad token '{t}'"),
            Error::Leftover(n) => write!(f, "expected 1 result, found {n}"),
            Error::TooLong => f.write_str("line too long"),
        }
    }
}

struct Stack {
    items: [i64; 16],
    len: usize,
}

impl Stack {
    fn push(&mut self, v: i64) -> Result<(), Error<'static>> {
        let slot = self.items.get_mut(self.len).ok_or(Error::StackOverflow)?;
        *slot = v;
        self.len += 1;
        Ok(())
    }
    fn pop(&mut self) -> Result<i64, Error<'static>> {
        self.len = self.len.checked_sub(1).ok_or(Error::Underflow)?;
        self.items.get(self.len).copied().ok_or(Error::Underflow)
    }
}

fn eval(line: &str) -> Result<Option<i64>, Error<'_>> {
    let mut st = Stack { items: [0; 16], len: 0 };
    let mut any = false;
    for tok in line.split_ascii_whitespace() {
        any = true;
        match tok {
            "+" | "-" | "*" | "/" | "%" => {
                let b = st.pop()?;
                let a = st.pop()?;
                let r = match tok {
                    "+" => a.checked_add(b),
                    "-" => a.checked_sub(b),
                    "*" => a.checked_mul(b),
                    _ if b == 0 => return Err(Error::DivByZero),
                    "/" => a.checked_div(b),
                    _ => a.checked_rem(b),
                };
                st.push(r.ok_or(Error::Overflow)?)?;
            }
            "neg" => {
                let a = st.pop()?;
                st.push(a.checked_neg().ok_or(Error::Overflow)?)?;
            }
            "dup" => {
                let a = st.pop()?;
                st.push(a)?;
                st.push(a)?;
            }
            "swap" => {
                let b = st.pop()?;
                let a = st.pop()?;
                st.push(b)?;
                st.push(a)?;
            }
            "drop" => {
                st.pop()?;
            }
            _ => st.push(tok.parse().map_err(|_| Error::BadToken(tok))?)?,
        }
    }
    if !any {
        return Ok(None);
    }
    match st.len {
        1 => Ok(st.items.first().copied()),
        n => Err(Error::Leftover(n)),
    }
}

fn main(_args: Args) -> i32 {
    let mut status = 0;
    let mut line = [0u8; 256];
    let mut len = 0usize;
    let mut too_long = false;
    let mut line_no = 0u64;
    let mut buf = [0u8; 512];

    let finish = |bytes: &[u8], too_long: bool, line_no: u64, status: &mut i32| {
        let result = if too_long {
            Err(Error::TooLong)
        } else {
            match core::str::from_utf8(bytes) {
                Ok(text) => eval(text),
                Err(_) => Err(Error::BadToken("<not utf-8>")),
            }
        };
        match result {
            Ok(Some(v)) => println!("{v}"),
            Ok(None) => {}
            Err(e) => {
                eprintln!("rpn: line {line_no}: {e}");
                *status = 1;
            }
        }
    };

    loop {
        let n = match nostd_rt::read(0, &mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => return 1,
        };
        for &b in buf.get(..n).unwrap_or(&[]) {
            if b == b'\n' {
                line_no += 1;
                finish(line.get(..len).unwrap_or(&[]), too_long, line_no, &mut status);
                len = 0;
                too_long = false;
            } else if let Some(slot) = line.get_mut(len) {
                *slot = b;
                len += 1;
            } else {
                too_long = true;
            }
        }
    }
    if len > 0 || too_long {
        line_no += 1;
        finish(line.get(..len).unwrap_or(&[]), too_long, line_no, &mut status);
    }
    status
}

nostd_rt::entry!(main);

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    eprintln!("{info}");
    nostd_rt::exit(101)
}
