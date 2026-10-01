//! # wc1: word count, no libc
//!
//! Count lines, words and bytes of standard input, like `wc`.
//!
//! ```text
//! wc1 [-l] [-w] [-c]
//! ```
//!
//! - With no flags, print all three. Flags select which counts to print; they
//!   can be separate (`-l -w`) or combined (`-lw`). Counts always appear in the
//!   order lines, words, bytes.
//! - Output: each selected count right-aligned in **7 columns**, separated by
//!   one space, then a newline: `      2       5      27`.
//! - lines = number of `\n` bytes; words = maximal runs of non-whitespace
//!   (`u8::is_ascii_whitespace`); bytes = total bytes.
//! - Any other argument → print `wc: unknown option: <arg>` to stderr, exit 2.
//!   (Print the whole offending argument, e.g. `-x` or `file.txt`.)
//! - Read stdin in chunks of at most 512 bytes. Words can span chunks!
//!
//! The runtime crate `nostd_rt` provides `_start`, I/O and `println!`: see
//! the lesson or `support/nostd_rt.rs` in your workspace.
#![no_std]
#![no_main]

extern crate nostd_rt;

use nostd_rt::{eprintln, println, Args};

fn main(args: Args) -> i32 {
    // TODO: parse flags, read stdin in 512-byte chunks, count, print.
    let _ = args;
    eprintln!("wc1: not implemented yet");
    99
}

nostd_rt::entry!(main);

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    eprintln!("{info}");
    nostd_rt::exit(101)
}
