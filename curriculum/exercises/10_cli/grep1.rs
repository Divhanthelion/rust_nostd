//! # grep1: grep, no libc
//!
//! Print the lines of standard input that contain a pattern.
//!
//! ```text
//! grep1 [-i] [-n] [-v] [-c] PATTERN
//! ```
//!
//! - A line matches if it contains `PATTERN` as a byte substring. `-i` makes
//!   the comparison ASCII case-insensitive.
//! - `-v` selects the lines that do **not** match.
//! - `-n` prefixes each printed line with its 1-based line number and `:`.
//! - `-c` prints only the number of selected lines.
//! - Flags may be combined (`-in`). The first non-flag argument is the
//!   pattern; anything after it is a usage error.
//! - Exit status: 0 if any line was selected, 1 if none, 2 on errors.
//! - Usage errors (no pattern, unknown flag, extra arguments): print
//!   `usage: grep1 [-invc] PATTERN` to stderr and exit 2.
//! - Lines are assembled in a **fixed 1024-byte buffer** (the newline doesn't
//!   count). A longer line is an error: print `grep1: line <N> too long` to
//!   stderr and exit 2. Read stdin in chunks of at most 512 bytes.
//! - A final line without a trailing newline is still processed (and printed
//!   with a newline).
#![no_std]
#![no_main]

extern crate nostd_rt;

use nostd_rt::{eprintln, println, Args};

fn main(args: Args) -> i32 {
    // TODO: parse options, assemble lines in a 1024-byte buffer, match, print.
    let _ = args;
    eprintln!("grep1: not implemented yet");
    99
}

nostd_rt::entry!(main);

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    eprintln!("{info}");
    nostd_rt::exit(101)
}
