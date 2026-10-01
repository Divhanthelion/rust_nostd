//! # cortexm1: Boot a Cortex-M from reset, by hand
//!
//! Real firmware for a Cortex-M3, cross-compiled for `thumbv7m-none-eabi` and
//! booted in QEMU. No cortex-m-rt: you write what it normally generates.
//!
//! The linker script `cortexm1.x` (next to this file: read it) places
//! `LONG(_stack_start)` first, then the section `.vector_table.reset_vector`,
//! then `.vector_table.exceptions`.
//!
//! Your tasks:
//! 1. `__RESET_VECTOR`: a static function pointer to `Reset`, placed in
//!    `.vector_table.reset_vector` with `#[unsafe(link_section = "...")]`.
//! 2. `__EXCEPTIONS`: 14 `Vector`s for words 2..=15, placed in
//!    `.vector_table.exceptions`: `HardFault` in word 3, `DefaultHandler`
//!    everywhere else, and `Vector { reserved: 0 }` for reserved words 7, 8, 9,
//!    10 and 13.
//! 3. In `Reset`: zero `.bss` (`_sbss`..`_ebss`) and copy `.data` from
//!    `_sidata` (flash) to `_sdata`..`_edata` (RAM), word by word with
//!    `ptr::write_volatile`, **before** calling `main`.
//! 4. `HardFault` and `DefaultHandler`: print a message over semihosting and
//!    exit with failure.
//!
//! The checker inspects the ELF (vector table at 0x0, SP, Reset with the Thumb
//! bit, HardFault in word 3), then boots it in QEMU (if installed) and expects:
//!
//! ```text
//! Hello from Cortex-M3!
//! .data ok: 0xc0ffee
//! .bss ok: 0x0
//! ```
//!
//! Install the target first: `rustup target add thumbv7m-none-eabi`.
#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr;

/// An exception table entry: a handler address, or a reserved zero word.
pub union Vector {
    handler: unsafe extern "C" fn(),
    reserved: usize,
}

#[unsafe(link_section = ".vector_table.reset_vector")]
#[unsafe(no_mangle)]
pub static __RESET_VECTOR: unsafe extern "C" fn() -> ! = Reset;

#[unsafe(link_section = ".vector_table.exceptions")]
#[unsafe(no_mangle)]
pub static __EXCEPTIONS: [Vector; 14] = [
    Vector { handler: DefaultHandler }, // 2  NMI
    Vector { handler: HardFault },      // 3  HardFault
    Vector { handler: DefaultHandler }, // 4  MemManage
    Vector { handler: DefaultHandler }, // 5  BusFault
    Vector { handler: DefaultHandler }, // 6  UsageFault
    Vector { reserved: 0 },             // 7
    Vector { reserved: 0 },             // 8
    Vector { reserved: 0 },             // 9
    Vector { reserved: 0 },             // 10
    Vector { handler: DefaultHandler }, // 11 SVCall
    Vector { handler: DefaultHandler }, // 12 DebugMonitor
    Vector { reserved: 0 },             // 13
    Vector { handler: DefaultHandler }, // 14 PendSV
    Vector { handler: DefaultHandler }, // 15 SysTick
];

// Symbols from the linker script. Only their addresses mean anything.
unsafe extern "C" {
    static mut _sbss: u32;
    static mut _ebss: u32;
    static mut _sdata: u32;
    static mut _edata: u32;
    static _sidata: u32;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Reset() -> ! {
    // Real RAM powers up with random contents. QEMU zeroes it, which would hide
    // a missing .bss initialisation, so scribble on .bss first. (Given.)
    // SAFETY: nothing has been initialised yet; we only touch .bss.
    unsafe { dirty_bss() };
    // SAFETY: the linker script defines these word-aligned regions; no other
    // code is running and no static has been read yet.
    unsafe {
        let mut dst = &raw mut _sbss;
        while dst < &raw mut _ebss {
            ptr::write_volatile(dst, 0);
            dst = dst.add(1);
        }
        let mut dst = &raw mut _sdata;
        let mut src = &raw const _sidata;
        while dst < &raw mut _edata {
            ptr::write_volatile(dst, ptr::read(src));
            dst = dst.add(1);
            src = src.add(1);
        }
    }
    main()
}

#[unsafe(no_mangle)]
pub extern "C" fn HardFault() {
    semihosting::write_str("HardFault!\n");
    semihosting::exit(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn DefaultHandler() {
    semihosting::write_str("unexpected exception\n");
    semihosting::exit(false)
}

// ---------------------------------------------------------------------------
// The application (given)

static mut MAGIC: u32 = 0xC0FFEE; // .data
static mut ZEROED: [u32; 4] = [0; 4]; // .bss

unsafe fn dirty_bss() {
    // SAFETY: called first thing in Reset; .bss is ours to scribble on.
    unsafe {
        let mut p = &raw mut _sbss;
        while p < &raw mut _ebss {
            ptr::write_volatile(p, 0xDEAD_BEEF);
            p = p.add(1);
        }
    }
}

fn main() -> ! {
    use core::fmt::Write;
    let mut out = semihosting::Writer;
    // Volatile reads stop the compiler from assuming the initial values.
    // SAFETY: single-threaded, no interrupts touch these.
    let (magic, zero) = unsafe { ((&raw const MAGIC).read_volatile(), (&raw const ZEROED).cast::<u32>().add(2).read_volatile()) };
    let _ = writeln!(out, "Hello from Cortex-M3!");
    let _ = writeln!(out, ".data ok: {magic:#x}");
    let _ = writeln!(out, ".bss ok: {zero:#x}");
    semihosting::exit(magic == 0xC0FFEE && zero == 0)
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    semihosting::write_str("panic!\n");
    semihosting::exit(false)
}

/// Semihosting: ask the debugger (here, QEMU) to do I/O for us. (Given.)
mod semihosting {
    const SYS_WRITE0: usize = 0x04;
    const SYS_EXIT: usize = 0x18;

    fn call(op: usize, arg: usize) -> usize {
        let r;
        // SAFETY: `bkpt 0xAB` is the semihosting trap (r0 = operation, r1 = argument).
        unsafe { core::arch::asm!("bkpt #0xAB", inout("r0") op => r, in("r1") arg, options(nostack)) };
        r
    }

    pub fn write_str(s: &str) {
        let mut buf = [0u8; 65];
        for chunk in s.as_bytes().chunks(64) {
            buf[..chunk.len()].copy_from_slice(chunk);
            buf[chunk.len()] = 0;
            call(SYS_WRITE0, buf.as_ptr() as usize);
        }
    }

    pub fn exit(success: bool) -> ! {
        // ADP_Stopped_ApplicationExit (QEMU exits 0) or RunTimeErrorUnknown (exits 1).
        call(SYS_EXIT, if success { 0x2_0026 } else { 0x2_0023 });
        loop {}
    }

    pub struct Writer;
    impl core::fmt::Write for Writer {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            write_str(s);
            Ok(())
        }
    }
}
