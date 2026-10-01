# Capstone: no_std Command-Line Tools

Time to build real programs on top of the runtime you assembled in module 09.
The three capstones (`wc`, `grep`, and an RPN calculator) are small, but they
force the design decisions every no_std program faces: fixed memory,
streaming, state carried across buffer boundaries, and errors that become exit
codes instead of panics.

## The nostd_rt API

Your workspace contains the runtime's source in `support/nostd_rt.rs`. Read it:
it's the code from module 09, tidied into a crate.

```rust
#![no_std]
#![no_main]

extern crate nostd_rt;
use nostd_rt::{eprintln, println, Args};

fn main(args: Args) -> i32 {
    // args.len(), args.get(i) -> Option<&'static [u8]>, args.get_str(i), args.iter()
    let mut buf = [0u8; 512];
    loop {
        match nostd_rt::read(0, &mut buf) {     // retries EINTR for you
            Ok(0) => break,                     // end of input
            Ok(n) => { /* process &buf[..n] */ }
            Err(errno) => { eprintln!("read failed: {}", -errno); return 1; }
        }
    }
    0                                           // becomes the exit code
}

nostd_rt::entry!(main);

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    eprintln!("{info}");
    nostd_rt::exit(101)
}
```

Also available: `nostd_rt::write_all(fd, bytes)`, `nostd_rt::exit(code)`,
`nostd_rt::env(b"NAME")`, the `Stdout`/`Stderr` writers, and `print!`/`eprint!`.

## Streaming with fixed buffers

Input arrives in chunks of arbitrary size. A chunk boundary can split a word,
a line, a number or a UTF-8 character. Two techniques handle it.

**1. Carry state across chunks.** Counting words needs only one bit of
memory: "was the previous byte inside a word?". Process byte by byte and keep
the flag outside the read loop:

```rust
let mut in_word = false;
for &b in chunk {
    if b.is_ascii_whitespace() {
        in_word = false;
    } else if !in_word {
        in_word = true;
        words += 1;
    }
}
```

**2. Assemble records in a second buffer.** For line-oriented tools, copy bytes
into a fixed `line` buffer until you see `\n`, process the line, reset. Decide
up front what happens when a line exceeds the buffer: truncate, process in
pieces, or report an error. There's no `Vec` to silently grow, so this is a
*design decision*, and writing it down is part of the job.

```rust
struct LineBuf<const N: usize> { buf: [u8; N], len: usize }

impl<const N: usize> LineBuf<N> {
    fn push(&mut self, b: u8) -> Result<(), TooLong> { ... }
    fn take(&mut self) -> &[u8] { let n = self.len; self.len = 0; &self.buf[..n] }
}
```

Don't forget the **last line without a trailing newline**: after end-of-file,
process whatever is left in the line buffer.

## Arguments without allocation

`args.get(i)` returns `&'static [u8]` slices pointing straight into the
initial stack, so parsing them never copies. A typical flag parser:

```rust
for arg in args.iter().skip(1) {
    match arg {
        [b'-', flags @ ..] if !flags.is_empty() => {
            for &f in flags {
                match f {
                    b'l' => opts.lines = true,
                    b'w' => opts.words = true,
                    _ => return usage_error(f),
                }
            }
        }
        _ => pattern = Some(arg),
    }
}
```

## Errors become exit codes

Command-line tools communicate failure through the exit status, by
convention:

| Code | Meaning |
|---|---|
| 0 | success (for `grep`: at least one line matched) |
| 1 | "no result" or per-record errors (for `grep`: nothing matched) |
| 2 | usage error: bad flags, missing arguments, unrecoverable conditions |
| 101 | (Rust convention) panic |

Messages go to **stderr** and results to **stdout**, so pipelines stay clean:
`./grep1 -c ERROR < log | ./wc1 -l` must not see error text in its input.

The capstones are panic-free on any input: overflowing numbers, huge lines,
empty input, binary garbage. Every failure is a decision you made, reported
with a message and a code.

> **Toyota lens:** this is the same discipline as an ECU's diagnostic
> interface: bounded buffers per message, explicit handling of malformed
> frames, and a *defined* reaction to every error (negative response codes in
> UDS) instead of undefined behaviour. A reviewer will ask "what happens with
> a 2 KB line?" and you need an answer that isn't "it crashes".
