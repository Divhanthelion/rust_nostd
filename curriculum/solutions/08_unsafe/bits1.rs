//! # bits1: Bit manipulation
//!
//! Registers, protocol fields and flags are all bits. Implement these
//! helpers on `u32`. They must never panic: bit positions ≥ 32 and impossible
//! fields are handled as documented (`checked_shl` helps; plain `<<` panics
//! in debug builds when shifting by ≥ the bit width).
//!
//! A *field* is `width` bits starting at bit `offset` (bit 0 = least
//! significant). E.g. offset 4, width 3 covers bits 4..=6, mask `0b111_0000`.
#![no_std]

/// Set bit `n`. Out-of-range `n` leaves `v` unchanged.
pub fn set_bit(v: u32, n: u32) -> u32 {
    v | 1u32.checked_shl(n).unwrap_or(0)
}

/// Clear bit `n`. Out-of-range `n` leaves `v` unchanged.
pub fn clear_bit(v: u32, n: u32) -> u32 {
    v & !1u32.checked_shl(n).unwrap_or(0)
}

/// Toggle bit `n`. Out-of-range `n` leaves `v` unchanged.
pub fn toggle_bit(v: u32, n: u32) -> u32 {
    v ^ 1u32.checked_shl(n).unwrap_or(0)
}

/// Is bit `n` set? `false` for out-of-range `n`.
pub fn test_bit(v: u32, n: u32) -> bool {
    v & 1u32.checked_shl(n).unwrap_or(0) != 0
}

/// Mask for a field, or `None` if `width == 0` or the field doesn't fit in 32 bits.
pub fn field_mask(offset: u32, width: u32) -> Option<u32> {
    if width == 0 || offset.checked_add(width)? > 32 {
        return None;
    }
    let ones = if width == 32 { u32::MAX } else { (1u32 << width) - 1 };
    Some(ones << offset)
}

/// Read a field's value (shifted down to bit 0).
pub fn extract(reg: u32, offset: u32, width: u32) -> Option<u32> {
    Some((reg & field_mask(offset, width)?) >> offset)
}

/// Write `value` into a field, leaving other bits alone. `None` if the field
/// is invalid or `value` doesn't fit in `width` bits.
pub fn insert(reg: u32, offset: u32, width: u32, value: u32) -> Option<u32> {
    let mask = field_mask(offset, width)?;
    let shifted = value.checked_shl(offset)?;
    if shifted >> offset != value || shifted & !mask != 0 {
        return None;
    }
    Some((reg & !mask) | shifted)
}

/// Interpret the low `bits` bits of `value` as a two's-complement signed
/// number (1 ≤ bits ≤ 32), e.g. `sign_extend(0b1111, 4) == -1`.
/// Out-of-range `bits` → `None`.
pub fn sign_extend(value: u32, bits: u32) -> Option<i32> {
    if bits == 0 || bits > 32 {
        return None;
    }
    let shift = 32 - bits;
    Some(((value << shift) as i32) >> shift)
}

/// True if an odd number of bits are set.
pub fn odd_parity(v: u32) -> bool {
    v.count_ones() % 2 == 1
}

/// Index of the lowest set bit, or `None` for 0.
pub fn lowest_set_bit(v: u32) -> Option<u32> {
    if v == 0 { None } else { Some(v.trailing_zeros()) }
}

/// Round up to a power of two (`0` → `1`), or `None` if it would overflow.
pub fn round_up_pow2(v: u32) -> Option<u32> {
    v.checked_next_power_of_two()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_bits() {
        assert_eq!(set_bit(0, 0), 1);
        assert_eq!(set_bit(0b1000, 31), 0x8000_0008);
        assert_eq!(set_bit(5, 32), 5);
        assert_eq!(clear_bit(0xFF, 3), 0xF7);
        assert_eq!(clear_bit(0xFF, 99), 0xFF);
        assert_eq!(toggle_bit(0b101, 1), 0b111);
        assert_eq!(toggle_bit(0b101, 2), 0b001);
        assert!(test_bit(0x8000_0000, 31));
        assert!(!test_bit(0x8000_0000, 30));
        assert!(!test_bit(u32::MAX, 32));
    }

    #[test]
    fn masks() {
        assert_eq!(field_mask(4, 3), Some(0b111_0000));
        assert_eq!(field_mask(0, 32), Some(u32::MAX));
        assert_eq!(field_mask(31, 1), Some(0x8000_0000));
        assert_eq!(field_mask(30, 3), None);
        assert_eq!(field_mask(0, 0), None);
        assert_eq!(field_mask(u32::MAX, 2), None);
    }

    #[test]
    fn fields() {
        // a STM32-style MODER register: 2 bits per pin
        let moder = 0b01_00_11_10;
        assert_eq!(extract(moder, 0, 2), Some(0b10));
        assert_eq!(extract(moder, 2, 2), Some(0b11));
        assert_eq!(extract(moder, 6, 2), Some(0b01));
        assert_eq!(insert(moder, 2, 2, 0b01), Some(0b01_00_01_10));
        assert_eq!(insert(0, 28, 4, 0xF), Some(0xF000_0000));
        assert_eq!(insert(0, 0, 32, 0xDEAD_BEEF), Some(0xDEAD_BEEF));
        assert_eq!(insert(0, 4, 3, 8), None, "8 doesn't fit in 3 bits");
        assert_eq!(insert(0, 30, 4, 1), None);
    }

    #[test]
    fn signs() {
        assert_eq!(sign_extend(0b1111, 4), Some(-1));
        assert_eq!(sign_extend(0b0111, 4), Some(7));
        assert_eq!(sign_extend(0x800, 12), Some(-2048));
        assert_eq!(sign_extend(0xFFFF_FFFF, 32), Some(-1));
        assert_eq!(sign_extend(0x1_0000, 16), Some(0), "bits above `bits` are ignored");
        assert_eq!(sign_extend(1, 0), None);
        assert_eq!(sign_extend(1, 33), None);
    }

    #[test]
    fn misc() {
        assert!(odd_parity(0b1011));
        assert!(!odd_parity(0b1001));
        assert_eq!(lowest_set_bit(0b1000), Some(3));
        assert_eq!(lowest_set_bit(0), None);
        assert_eq!(round_up_pow2(0), Some(1));
        assert_eq!(round_up_pow2(5), Some(8));
        assert_eq!(round_up_pow2(64), Some(64));
        assert_eq!(round_up_pow2(0x8000_0001), None);
    }
}
