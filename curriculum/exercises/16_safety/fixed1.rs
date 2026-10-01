//! # fixed1: Fixed-point math and a PID controller
//!
//! `Q16` is a Q16.16 fixed-point number: the `i32` raw value `r` means
//! `r / 65536`. All arithmetic **saturates** at `i32::MIN`/`i32::MAX` instead
//! of overflowing, and nothing here may panic.
//!
//! 1. Conversions: `from_int`, `from_milli` (thousandths → Q16, rounding toward
//!    zero), `to_milli` (Q16 → thousandths, rounding toward zero).
//! 2. `Add`, `Sub`, `Neg` and `Mul` (saturating). Multiply in `i64`:
//!    `(a * b) >> 16`, then saturate. `checked_div` returns `None` for division by
//!    zero; otherwise `(a << 16) / b` in `i64`, saturated.
//! 3. `Pid::update(setpoint, measurement)`:
//!    ```text
//!    error  = setpoint - measurement
//!    p      = kp * error
//!    d      = kd * (error - previous error)     (0 on the first update)
//!    i_new  = integral + ki * error
//!    out    = p + i_new + d, clamped to [out_min, out_max]
//!    ```
//!    **Anti-windup**: keep `i_new` as the new integral only if `p + i_new + d`
//!    was inside the limits, or if the error points back into range (output
//!    above max with error < 0, or below min with error > 0). Otherwise keep the
//!    old integral. Always remember the error for the next derivative.
#![no_std]

use core::ops::{Add, Mul, Neg, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Q16(pub i32);

fn saturate(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

impl Q16 {
    pub const ZERO: Q16 = Q16(0);
    pub const ONE: Q16 = Q16(1 << 16);
    pub const MAX: Q16 = Q16(i32::MAX);
    pub const MIN: Q16 = Q16(i32::MIN);

    pub fn from_int(v: i16) -> Q16 {
        todo!()
    }

    pub fn from_milli(m: i32) -> Q16 {
        todo!()
    }

    pub fn to_milli(self) -> i32 {
        todo!()
    }

    pub fn checked_div(self, rhs: Q16) -> Option<Q16> {
        todo!()
    }

    pub fn clamp_to(self, lo: Q16, hi: Q16) -> Q16 {
        if self < lo { lo } else if self > hi { hi } else { self }
    }
}

impl Add for Q16 {
    type Output = Q16;
    fn add(self, rhs: Q16) -> Q16 {
        todo!()
    }
}

impl Sub for Q16 {
    type Output = Q16;
    fn sub(self, rhs: Q16) -> Q16 {
        todo!()
    }
}

impl Neg for Q16 {
    type Output = Q16;
    fn neg(self) -> Q16 {
        todo!()
    }
}

impl Mul for Q16 {
    type Output = Q16;
    fn mul(self, rhs: Q16) -> Q16 {
        todo!()
    }
}

pub struct Pid {
    pub kp: Q16,
    pub ki: Q16,
    pub kd: Q16,
    pub out_min: Q16,
    pub out_max: Q16,
    integral: Q16,
    prev_error: Option<Q16>,
}

impl Pid {
    pub fn new(kp: Q16, ki: Q16, kd: Q16, out_min: Q16, out_max: Q16) -> Pid {
        Pid { kp, ki, kd, out_min, out_max, integral: Q16::ZERO, prev_error: None }
    }

    pub fn integral(&self) -> Q16 {
        self.integral
    }

    pub fn update(&mut self, setpoint: Q16, measurement: Q16) -> Q16 {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(v: i16) -> Q16 {
        Q16::from_int(v)
    }

    #[test]
    fn conversions() {
        assert_eq!(Q16::from_int(3), Q16(3 << 16));
        assert_eq!(Q16::from_int(-1), Q16(-65536));
        assert_eq!(Q16::from_milli(1500), Q16(98_304));
        assert_eq!(Q16::from_milli(-250), Q16(-16_384));
        assert_eq!(Q16::from_milli(1), Q16(65), "65.536 rounds toward zero");
        assert_eq!(Q16(98_304).to_milli(), 1500);
        assert_eq!(Q16(-1).to_milli(), 0);
        assert_eq!(Q16(-66).to_milli(), -1);
        assert_eq!(Q16::from_milli(i32::MAX), Q16::MAX, "saturates");
    }

    #[test]
    fn arithmetic() {
        assert_eq!(q(2) + q(3), q(5));
        assert_eq!(q(2) - q(3), q(-1));
        assert_eq!(-q(4), q(-4));
        assert_eq!(Q16::from_milli(1500) * Q16::from_milli(2500), Q16::from_milli(3750));
        assert_eq!(q(-3) * Q16::from_milli(500), Q16::from_milli(-1500));
        assert_eq!(q(7).checked_div(q(2)), Some(Q16::from_milli(3500)));
        assert_eq!(q(1).checked_div(Q16::ZERO), None);
    }

    #[test]
    fn saturation() {
        assert_eq!(Q16::MAX + Q16::ONE, Q16::MAX);
        assert_eq!(Q16::MIN - Q16::ONE, Q16::MIN);
        assert_eq!(-Q16::MIN, Q16::MAX);
        assert_eq!(q(30000) * q(30000), Q16::MAX);
        assert_eq!(q(-30000) * q(30000), Q16::MIN);
        assert_eq!(q(30000).checked_div(Q16(1)), Some(Q16::MAX));
    }

    #[test]
    fn proportional_only() {
        let mut pid = Pid::new(q(2), Q16::ZERO, Q16::ZERO, q(-100), q(100));
        assert_eq!(pid.update(q(10), q(4)), q(12));
        assert_eq!(pid.update(q(10), q(80)), q(-100), "clamped");
    }

    #[test]
    fn integral_and_derivative() {
        let mut pid = Pid::new(q(1), Q16::from_milli(500), q(2), q(-100), q(100));
        // e=4: p=4, i=2, d=0 → 6
        assert_eq!(pid.update(q(4), q(0)), q(6));
        // e=2: p=2, i=2+1=3, d=2*(2-4)=-4 → 1
        assert_eq!(pid.update(q(4), q(2)), q(1));
        assert_eq!(pid.integral(), q(3));
    }

    #[test]
    fn anti_windup() {
        let mut pid = Pid::new(q(1), q(1), Q16::ZERO, q(0), q(10));
        // A large error saturates the output; the integral must not wind up.
        for _ in 0..50 {
            assert_eq!(pid.update(q(20), q(0)), q(10));
        }
        assert!(pid.integral() <= q(10), "integral {:?} wound up", pid.integral());
        // When the error reverses, the output leaves saturation at once.
        let out = pid.update(q(0), q(5));
        assert!(out < q(10), "still saturated: {out:?}");
    }
}
