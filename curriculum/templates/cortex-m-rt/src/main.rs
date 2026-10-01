//! `{{name}}`: the same firmware as `nostd new cortex-m`, written with the
//! ecosystem crates. Compare the two side by side: cortex-m-rt generates the
//! vector table and Reset handler you would otherwise write yourself.
#![no_std]
#![no_main]

use core::sync::atomic::{AtomicU32, Ordering};

use cortex_m::peripheral::syst::SystClkSource;
use cortex_m::peripheral::Peripherals;
use cortex_m_rt::{entry, exception};
use cortex_m_semihosting::{debug, hprintln};
use panic_semihosting as _; // links the #[panic_handler]

static TICKS: AtomicU32 = AtomicU32::new(0);

#[entry]
fn main() -> ! {
    // `take()` hands out the singleton exactly once (it uses a critical section).
    let Some(p) = Peripherals::take() else { panic!("peripherals already taken") };
    let mut syst = p.SYST;
    syst.set_clock_source(SystClkSource::Core);
    syst.set_reload(12_000 - 1);
    syst.clear_current();
    syst.enable_counter();
    syst.enable_interrupt();

    hprintln!("booted with cortex-m-rt");
    while TICKS.load(Ordering::Relaxed) < 10 {
        cortex_m::asm::wfi();
    }
    hprintln!("handled {} SysTick interrupts", TICKS.load(Ordering::Relaxed));
    debug::exit(debug::EXIT_SUCCESS);
    loop {}
}

#[exception]
fn SysTick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}
