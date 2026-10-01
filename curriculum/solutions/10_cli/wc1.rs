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
    let (mut want_l, mut want_w, mut want_c) = (false, false, false);
    for arg in args.iter().skip(1) {
        match arg {
            [b'-', flags @ ..] if !flags.is_empty() && flags.iter().all(|f| b"lwc".contains(f)) => {
                for f in flags {
                    match f {
                        b'l' => want_l = true,
                        b'w' => want_w = true,
                        _ => want_c = true,
                    }
                }
            }
            other => {
                eprintln!("wc: unknown option: {}", core::str::from_utf8(other).unwrap_or("?"));
                return 2;
            }
        }
    }
    if !(want_l || want_w || want_c) {
        (want_l, want_w, want_c) = (true, true, true);
    }

    let (mut lines, mut words, mut bytes) = (0u64, 0u64, 0u64);
    let mut in_word = false;
    let mut buf = [0u8; 512];
    loop {
        let n = match nostd_rt::read(0, &mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                eprintln!("wc: read error {}", -e);
                return 1;
            }
        };
        let chunk = buf.get(..n).unwrap_or(&[]);
        bytes += n as u64;
        for &b in chunk {
            if b == b'\n' {
                lines += 1;
            }
            if b.is_ascii_whitespace() {
                in_word = false;
            } else if !in_word {
                in_word = true;
                words += 1;
            }
        }
    }

    let mut first = true;
    for (wanted, value) in [(want_l, lines), (want_w, words), (want_c, bytes)] {
        if wanted {
            if !first {
                nostd_rt::print!(" ");
            }
            nostd_rt::print!("{value:>7}");
            first = false;
        }
    }
    println!();
    0
}

nostd_rt::entry!(main);

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    eprintln!("{info}");
    nostd_rt::exit(101)
}
