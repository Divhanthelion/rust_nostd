//! `{{name}}`: Cortex-M3 firmware written from first principles.
//!
//! No cortex-m-rt, no HAL, no dependencies: this file contains the vector
//! table, the reset handler (.bss zeroing and .data copying), an exception
//! handler driven by the SysTick timer, semihosting I/O, and a panic handler.
//! `cargo run` boots it in QEMU.
#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ptr;
use core::sync::atomic::{AtomicU32, Ordering};

// ---------------------------------------------------------------------------
// Vector table

/// Word 1 of the vector table: where the CPU starts executing after reset.
#[unsafe(link_section = ".vector_table.reset_vector")]
#[unsafe(no_mangle)]
pub static __RESET_VECTOR: unsafe extern "C" fn() -> ! = Reset;

/// An entry in the exception table: a handler address, or a reserved zero.
pub union Vector {
    handler: unsafe extern "C" fn(),
    reserved: usize,
}

/// Words 2..=15: the Cortex-M system exceptions.
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
    Vector { handler: SysTick },        // 15 SysTick
];

// ---------------------------------------------------------------------------
// Reset

/// Initialised statics prove that .data was copied from flash.
static mut BOOT_MAGIC: u32 = 0xC0FFEE;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn Reset() -> ! {
    // Symbols defined by link.x. Only their *addresses* are meaningful.
    unsafe extern "C" {
        static mut _sbss: u32;
        static mut _ebss: u32;
        static mut _sdata: u32;
        static mut _edata: u32;
        static _sidata: u32;
    }
    // SAFETY: we run before anything else; the linker script guarantees the
    // regions are word-aligned and in RAM/flash. Volatile writes stop the
    // compiler from replacing these loops with a memset/memcpy call.
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

// ---------------------------------------------------------------------------
// Exceptions

static TICKS: AtomicU32 = AtomicU32::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn SysTick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
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
// SysTick: the 24-bit timer every Cortex-M has

mod systick {
    use core::ptr::write_volatile;

    const CSR: *mut u32 = 0xE000_E010 as *mut u32; // control and status
    const RVR: *mut u32 = 0xE000_E014 as *mut u32; // reload value
    const CVR: *mut u32 = 0xE000_E018 as *mut u32; // current value

    /// Fire the SysTick exception every `reload + 1` core clock cycles.
    pub fn start(reload: u32) {
        // SAFETY: these are the architecturally defined SysTick registers.
        unsafe {
            write_volatile(RVR, reload & 0x00FF_FFFF);
            write_volatile(CVR, 0);
            // ENABLE | TICKINT | CLKSOURCE = processor clock
            write_volatile(CSR, 0b111);
        }
    }
}

// ---------------------------------------------------------------------------
// Semihosting: let the debugger/QEMU do I/O for us

mod semihosting {
    const SYS_WRITE0: usize = 0x04;
    const SYS_EXIT: usize = 0x18;
    const ADP_STOPPED_APPLICATION_EXIT: usize = 0x2_0026;
    const ADP_STOPPED_RUNTIME_ERROR: usize = 0x2_0023;

    fn call(op: usize, arg: usize) -> usize {
        let r;
        // SAFETY: `bkpt 0xAB` is the semihosting trap; r0 = op, r1 = argument.
        // Without a debugger/QEMU attached this would fault: semihosting is a
        // development tool, never leave it in production firmware.
        unsafe { core::arch::asm!("bkpt #0xAB", inout("r0") op => r, in("r1") arg, options(nostack)) };
        r
    }

    pub fn write_str(s: &str) {
        // SYS_WRITE0 prints a NUL-terminated string: copy chunks into a buffer.
        let mut buf = [0u8; 65];
        for chunk in s.as_bytes().chunks(64) {
            buf[..chunk.len()].copy_from_slice(chunk);
            buf[chunk.len()] = 0;
            call(SYS_WRITE0, buf.as_ptr() as usize);
        }
    }

    pub fn exit(success: bool) -> ! {
        let reason = if success { ADP_STOPPED_APPLICATION_EXIT } else { ADP_STOPPED_RUNTIME_ERROR };
        call(SYS_EXIT, reason);
        loop {}
    }

    /// `core::fmt::Write` adapter so `write!` works.
    pub struct Writer;
    impl core::fmt::Write for Writer {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            write_str(s);
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// The application

fn main() -> ! {
    use core::fmt::Write;
    let mut out = semihosting::Writer;

    // SAFETY: single-threaded; no interrupt touches BOOT_MAGIC.
    let magic = unsafe { (&raw const BOOT_MAGIC).read() };
    let _ = writeln!(out, "booted: .data says {magic:#x}");

    systick::start(12_000 - 1);
    while TICKS.load(Ordering::Relaxed) < 10 {
        // Sleep until the next interrupt.
        // SAFETY: wfi has no memory effects.
        unsafe { core::arch::asm!("wfi") };
    }
    let _ = writeln!(out, "handled {} SysTick interrupts", TICKS.load(Ordering::Relaxed));
    semihosting::exit(true)
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use core::fmt::Write;
    let _ = writeln!(semihosting::Writer, "{info}");
    semihosting::exit(false)
}
