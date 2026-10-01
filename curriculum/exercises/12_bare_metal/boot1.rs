//! # boot1: Startup code and fault decoding, testable on your PC
//!
//! The logic of a Cortex-M startup sequence and HardFault handler, written as
//! plain functions over slices so you can test it on the host before trusting
//! it on hardware:
//!
//! 1. `init_memory`: copy `.data` from flash and zero `.bss`, after
//!    **validating** the layout (a corrupt linker symbol must not trash memory).
//!    All positions are word indices. Copy `edata - sdata` words from
//!    `flash[sidata..]` to `ram[sdata..edata]`; zero `ram[sbss..ebss]`.
//! 2. `vector_table`: build the 16 architectural words. Word 0 = initial SP
//!    (must be 8-byte aligned), word 1 = reset | 1. Words 2..=15 come from
//!    `handlers[0..14]`: `Some(addr)` → `addr | 1`, `None` → `default | 1`,
//!    except **reserved** words (7, 8, 9, 10, 13), which must be 0 and must
//!    not be given a handler.
//! 3. `decode_ipsr`: which exception is active (IPSR bits 0..=8 hold the number).
//! 4. `parse_frame` and `fault_reasons`: decode a HardFault: the 8-word
//!    stacked frame, and the names of the set CFSR bits (lowest bit first).
#![no_std]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub sidata: usize,
    pub sdata: usize,
    pub edata: usize,
    pub sbss: usize,
    pub ebss: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootError {
    /// a start is after its end
    Inverted,
    /// a RAM range extends past the end of RAM
    OutOfRam,
    /// the .data load image extends past the end of flash
    OutOfFlash,
    /// .data and .bss overlap
    Overlap,
}

pub fn init_memory(flash: &[u32], ram: &mut [u32], l: &Layout) -> Result<(), BootError> {
    todo!()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorError {
    UnalignedStack,
    HandlerInReservedSlot(usize),
}

pub const RESERVED: [usize; 5] = [7, 8, 9, 10, 13];

pub fn vector_table(stack_top: u32, reset: u32, handlers: &[Option<u32>; 14], default: u32) -> Result<[u32; 16], VectorError> {
    todo!()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exception {
    ThreadMode,
    Nmi,
    HardFault,
    MemManage,
    BusFault,
    UsageFault,
    SVCall,
    DebugMonitor,
    PendSV,
    SysTick,
    /// device interrupt n (exception number 16 + n)
    Irq(u16),
    Reserved(u16),
}

pub fn decode_ipsr(ipsr: u32) -> Exception {
    todo!()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExceptionFrame {
    pub r0: u32,
    pub r1: u32,
    pub r2: u32,
    pub r3: u32,
    pub r12: u32,
    pub lr: u32,
    pub pc: u32,
    pub xpsr: u32,
}

/// The frame the CPU pushed on exception entry, lowest address first.
pub fn parse_frame(stack: &[u32]) -> Option<ExceptionFrame> {
    todo!()
}

/// CFSR bit names (bit number, name). Bits not listed are reserved.
pub const CFSR_BITS: [(u32, &str); 17] = [
    (0, "IACCVIOL"),
    (1, "DACCVIOL"),
    (3, "MUNSTKERR"),
    (4, "MSTKERR"),
    (7, "MMARVALID"),
    (8, "IBUSERR"),
    (9, "PRECISERR"),
    (10, "IMPRECISERR"),
    (11, "UNSTKERR"),
    (12, "STKERR"),
    (15, "BFARVALID"),
    (16, "UNDEFINSTR"),
    (17, "INVSTATE"),
    (18, "INVPC"),
    (19, "NOCP"),
    (24, "UNALIGNED"),
    (25, "DIVBYZERO"),
];

/// Write the names of the set CFSR bits into `out` (lowest bit first) and
/// return how many were written (limited by `out.len()`).
pub fn fault_reasons(cfsr: u32, out: &mut [&'static str]) -> usize {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLASH: [u32; 6] = [0xAAAA, 0xBBBB, 0x1111, 0x2222, 0x3333, 0xCCCC];

    #[test]
    fn copies_data_and_zeroes_bss() {
        let mut ram = [0xDEAD_BEEFu32; 8];
        let l = Layout { sidata: 2, sdata: 0, edata: 3, sbss: 3, ebss: 6 };
        init_memory(&FLASH, &mut ram, &l).unwrap();
        assert_eq!(ram, [0x1111, 0x2222, 0x3333, 0, 0, 0, 0xDEAD_BEEF, 0xDEAD_BEEF]);
    }

    #[test]
    fn empty_sections_are_fine() {
        let mut ram = [7u32; 4];
        let l = Layout { sidata: 6, sdata: 0, edata: 0, sbss: 0, ebss: 0 };
        assert_eq!(init_memory(&FLASH, &mut ram, &l), Ok(()));
        assert_eq!(ram, [7; 4]);
    }

    #[test]
    fn rejects_bad_layouts_without_touching_ram() {
        let mut ram = [9u32; 8];
        let bad = [
            (Layout { sidata: 0, sdata: 3, edata: 1, sbss: 4, ebss: 5 }, BootError::Inverted),
            (Layout { sidata: 0, sdata: 0, edata: 1, sbss: 6, ebss: 5 }, BootError::Inverted),
            (Layout { sidata: 0, sdata: 0, edata: 9, sbss: 0, ebss: 0 }, BootError::OutOfRam),
            (Layout { sidata: 0, sdata: 0, edata: 1, sbss: 2, ebss: 9 }, BootError::OutOfRam),
            (Layout { sidata: 4, sdata: 0, edata: 3, sbss: 3, ebss: 4 }, BootError::OutOfFlash),
            (Layout { sidata: usize::MAX, sdata: 0, edata: 3, sbss: 3, ebss: 4 }, BootError::OutOfFlash),
            (Layout { sidata: 0, sdata: 0, edata: 4, sbss: 3, ebss: 6 }, BootError::Overlap),
        ];
        for (l, e) in bad {
            assert_eq!(init_memory(&FLASH, &mut ram, &l), Err(e), "{l:?}");
        }
        assert_eq!(ram, [9; 8]);
    }

    #[test]
    fn builds_vector_tables() {
        let mut h = [None; 14];
        h[1] = Some(0x0000_0400); // HardFault (word 3)
        h[13] = Some(0x0000_0500); // SysTick (word 15)
        let t = vector_table(0x2001_0000, 0x0000_0100, &h, 0x0000_0200).unwrap();
        assert_eq!(t[0], 0x2001_0000);
        assert_eq!(t[1], 0x101);
        assert_eq!(t[2], 0x201, "NMI gets the default handler");
        assert_eq!(t[3], 0x401);
        assert_eq!(t[15], 0x501);
        for r in RESERVED {
            assert_eq!(t[r], 0, "word {r} is reserved");
        }
        assert_eq!(t[11], 0x201);
    }

    #[test]
    fn vector_table_errors() {
        assert_eq!(vector_table(0x2000_0004, 0x100, &[None; 14], 0x200), Err(VectorError::UnalignedStack));
        let mut h = [None; 14];
        h[5] = Some(0x300); // word 7: reserved
        assert_eq!(vector_table(0x2000_0000, 0x100, &h, 0x200), Err(VectorError::HandlerInReservedSlot(7)));
    }

    #[test]
    fn decodes_ipsr() {
        assert_eq!(decode_ipsr(0), Exception::ThreadMode);
        assert_eq!(decode_ipsr(3), Exception::HardFault);
        assert_eq!(decode_ipsr(15), Exception::SysTick);
        assert_eq!(decode_ipsr(16), Exception::Irq(0));
        assert_eq!(decode_ipsr(16 + 37), Exception::Irq(37));
        assert_eq!(decode_ipsr(0x0100_0000 | 11), Exception::SVCall, "upper xPSR bits are ignored");
        assert_eq!(decode_ipsr(1), Exception::Reserved(1));
        assert_eq!(decode_ipsr(7), Exception::Reserved(7));
    }

    #[test]
    fn decodes_faults() {
        let stack = [1, 2, 3, 4, 12, 0x0800_1235, 0x0800_2000, 0x0100_0003, 0xFFFF];
        let f = parse_frame(&stack).unwrap();
        assert_eq!((f.r0, f.r12, f.lr, f.pc, f.xpsr), (1, 12, 0x0800_1235, 0x0800_2000, 0x0100_0003));
        assert_eq!(parse_frame(&stack[..7]), None);

        let mut out = [""; 4];
        let n = fault_reasons((1 << 25) | (1 << 9) | (1 << 15) | (1 << 1), &mut out);
        assert_eq!(&out[..n], &["DACCVIOL", "PRECISERR", "BFARVALID", "DIVBYZERO"]);
        let mut one = [""; 1];
        assert_eq!(fault_reasons(u32::MAX, &mut one), 1);
        assert_eq!(one, ["IACCVIOL"]);
        assert_eq!(fault_reasons(1 << 2, &mut out), 0, "bit 2 is reserved");
    }
}
