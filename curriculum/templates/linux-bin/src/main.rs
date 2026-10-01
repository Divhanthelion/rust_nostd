//! `{{name}}`: a freestanding Linux program.
//!
//! There is no std, no libc and no C runtime here. The kernel jumps to
//! `_start` (defined in the `nostd_rt` crate), which calls `main` below.
#![no_std]
#![no_main]

use nostd_rt::{eprintln, println, Args};

fn main(args: Args) -> i32 {
    println!("hello from a freestanding Rust program!");
    for (i, arg) in args.iter().enumerate() {
        match core::str::from_utf8(arg) {
            Ok(s) => println!("argv[{i}] = {s}"),
            Err(_) => println!("argv[{i}] = <{} bytes, not UTF-8>", arg.len()),
        }
    }
    if let Some(user) = nostd_rt::env(b"USER") {
        println!("USER = {}", core::str::from_utf8(user).unwrap_or("?"));
    }

    // Echo stdin, upper-cased, through a fixed 4 KiB buffer.
    let mut buf = [0u8; 4096];
    loop {
        match nostd_rt::read(0, &mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let chunk = &mut buf[..n];
                chunk.make_ascii_uppercase();
                if nostd_rt::write_all(1, chunk).is_err() {
                    return 1;
                }
            }
            Err(e) => {
                eprintln!("read failed: errno {}", -e);
                return 1;
            }
        }
    }
    0
}

nostd_rt::entry!(main);

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    eprintln!("{info}");
    nostd_rt::exit(101)
}
