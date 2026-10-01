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

struct Opts {
    ignore_case: bool,
    number: bool,
    invert: bool,
    count: bool,
}

fn contains(line: &[u8], pat: &[u8], ignore_case: bool) -> bool {
    if pat.is_empty() {
        return true;
    }
    line.windows(pat.len()).any(|w| if ignore_case { w.eq_ignore_ascii_case(pat) } else { w == pat })
}

fn usage() -> i32 {
    eprintln!("usage: grep1 [-invc] PATTERN");
    2
}

fn main(args: Args) -> i32 {
    let mut opts = Opts { ignore_case: false, number: false, invert: false, count: false };
    let mut pattern: Option<&[u8]> = None;
    for arg in args.iter().skip(1) {
        match arg {
            [b'-', flags @ ..] if !flags.is_empty() && pattern.is_none() => {
                for f in flags {
                    match f {
                        b'i' => opts.ignore_case = true,
                        b'n' => opts.number = true,
                        b'v' => opts.invert = true,
                        b'c' => opts.count = true,
                        _ => return usage(),
                    }
                }
            }
            _ if pattern.is_none() => pattern = Some(arg),
            _ => return usage(),
        }
    }
    let Some(pattern) = pattern else { return usage() };

    let mut line = [0u8; 1024];
    let mut len = 0usize;
    let mut line_no = 0u64;
    let mut selected = 0u64;
    let mut buf = [0u8; 512];

    let handle = |line: &[u8], line_no: u64, selected: &mut u64| {
        if contains(line, pattern, opts.ignore_case) != opts.invert {
            *selected += 1;
            if !opts.count {
                let text = core::str::from_utf8(line).unwrap_or("<binary>");
                if opts.number {
                    println!("{line_no}:{text}");
                } else {
                    println!("{text}");
                }
            }
        }
    };

    loop {
        let n = match nostd_rt::read(0, &mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => return 2,
        };
        for &b in buf.get(..n).unwrap_or(&[]) {
            if b == b'\n' {
                line_no += 1;
                handle(line.get(..len).unwrap_or(&[]), line_no, &mut selected);
                len = 0;
            } else if let Some(slot) = line.get_mut(len) {
                *slot = b;
                len += 1;
            } else {
                eprintln!("grep1: line {} too long", line_no + 1);
                return 2;
            }
        }
    }
    if len > 0 {
        line_no += 1;
        handle(line.get(..len).unwrap_or(&[]), line_no, &mut selected);
    }
    if opts.count {
        println!("{selected}");
    }
    if selected > 0 { 0 } else { 1 }
}

nostd_rt::entry!(main);

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    eprintln!("{info}");
    nostd_rt::exit(101)
}
