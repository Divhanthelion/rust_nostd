//! # isr1: Interrupt arbitration, the NVIC's way
//!
//! Model the Cortex-M NVIC's decision: *which interrupt runs next?* Your
//! `select` drives a given simulator (`simulate`) that plays out nesting,
//! preemption and tail-chaining over time, so the tests can check full
//! timelines.
//!
//! Rules (like the real hardware, with 3 implemented priority bits):
//! - 32 IRQs, each with enable, pending and an 8-bit priority. Only the top
//!   3 bits are implemented: `set_priority` stores `prio & 0xE0`.
//! - Lower value = more urgent.
//! - A candidate must be enabled **and** pending.
//! - `primask == true` blocks every interrupt.
//! - If an exception is already running at priority `p`, a candidate must be
//!   **strictly** more urgent (`< p`) to preempt it.
//! - If `basepri != 0`, candidates with priority `>= (basepri & 0xE0)` are masked.
//! - Among the remaining candidates the most urgent wins; ties go to the
//!   lower IRQ number.
//! - `acknowledge` = `select` + clear the winner's pending bit.
//! - Out-of-range IRQ numbers (≥ 32) are rejected (`false`/`None`), never panic.
#![no_std]

pub const NUM_IRQS: usize = 32;

pub struct Nvic {
    enabled: u32,
    pending: u32,
    priority: [u8; NUM_IRQS],
}

impl Nvic {
    pub const fn new() -> Self {
        Nvic { enabled: 0, pending: 0, priority: [0; NUM_IRQS] }
    }

    pub fn enable(&mut self, irq: usize) -> bool {
        todo!()
    }

    pub fn disable(&mut self, irq: usize) -> bool {
        todo!()
    }

    pub fn pend(&mut self, irq: usize) -> bool {
        todo!()
    }

    pub fn is_pending(&self, irq: usize) -> bool {
        todo!()
    }

    pub fn set_priority(&mut self, irq: usize, prio: u8) -> bool {
        todo!()
    }

    pub fn priority(&self, irq: usize) -> Option<u8> {
        todo!()
    }

    /// The IRQ that would be taken now, if any.
    pub fn select(&self, running: Option<u8>, primask: bool, basepri: u8) -> Option<usize> {
        todo!()
    }

    /// Take the selected interrupt (clear its pending bit).
    pub fn acknowledge(&mut self, running: Option<u8>, primask: bool, basepri: u8) -> Option<usize> {
        todo!()
    }
}

// ---------------------------------------------------------------------------
// The simulator (given): runs handlers tick by tick using your arbitration.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trace {
    Start { t: u32, irq: usize },
    End { t: u32, irq: usize },
}

/// Simulate `until` ticks. `arrivals` are (tick, irq) hardware events;
/// `duration(irq)` is how many ticks each handler runs. Returns the number of
/// trace entries written to `out`.
pub fn simulate(nvic: &mut Nvic, arrivals: &[(u32, usize)], duration: impl Fn(usize) -> u32, until: u32, out: &mut [Trace]) -> usize {
    let mut stack = [(0usize, 0u32); NUM_IRQS];
    let mut depth = 0;
    let mut n = 0;
    let mut log = |e: Trace, n: &mut usize| {
        if let Some(slot) = out.get_mut(*n) {
            *slot = e;
            *n += 1;
        }
    };
    for t in 0..until {
        for &(at, irq) in arrivals {
            if at == t {
                nvic.pend(irq);
            }
        }
        loop {
            let running = if depth > 0 { nvic.priority(stack[depth - 1].0) } else { None };
            match nvic.acknowledge(running, false, 0) {
                Some(irq) if depth < NUM_IRQS => {
                    stack[depth] = (irq, duration(irq).max(1));
                    depth += 1;
                    log(Trace::Start { t, irq }, &mut n);
                }
                _ => break,
            }
        }
        if depth > 0 {
            let top = &mut stack[depth - 1];
            top.1 -= 1;
            if top.1 == 0 {
                log(Trace::End { t: t + 1, irq: top.0 }, &mut n);
                depth -= 1;
            }
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use Trace::*;

    fn nvic(prios: &[(usize, u8)]) -> Nvic {
        let mut n = Nvic::new();
        for &(irq, p) in prios {
            assert!(n.enable(irq));
            assert!(n.set_priority(irq, p));
        }
        n
    }

    #[test]
    fn bookkeeping() {
        let mut n = Nvic::new();
        assert!(n.set_priority(4, 0x5F));
        assert_eq!(n.priority(4), Some(0x40), "only the top 3 bits exist");
        assert!(!n.enable(32));
        assert!(!n.pend(99));
        assert_eq!(n.priority(32), None);
        assert!(!n.is_pending(3));
        n.pend(3);
        assert!(n.is_pending(3));
        assert_eq!(n.select(None, false, 0), None, "pending but not enabled");
        n.enable(3);
        assert_eq!(n.select(None, false, 0), Some(3));
        n.disable(3);
        assert_eq!(n.select(None, false, 0), None);
    }

    #[test]
    fn arbitration() {
        let mut n = nvic(&[(1, 0x80), (2, 0x20), (3, 0x20), (4, 0xE0)]);
        for i in 1..=4 {
            n.pend(i);
        }
        assert_eq!(n.select(None, false, 0), Some(2), "most urgent; tie → lower number");
        assert_eq!(n.select(None, true, 0), None, "PRIMASK masks everything");
        assert_eq!(n.select(Some(0x20), false, 0), None, "equal priority can't preempt");
        assert_eq!(n.select(Some(0x40), false, 0), Some(2));
        assert_eq!(n.select(None, false, 0x20), None, "BASEPRI 0x20 masks priorities >= 0x20");
        assert_eq!(n.select(None, false, 0x90), Some(2), "BASEPRI uses only the implemented bits");
        assert_eq!(n.acknowledge(None, false, 0), Some(2));
        assert!(!n.is_pending(2));
        assert_eq!(n.acknowledge(None, false, 0), Some(3));
        assert_eq!(n.acknowledge(None, false, 0), Some(1));
        assert_eq!(n.acknowledge(None, false, 0), Some(4));
        assert_eq!(n.acknowledge(None, false, 0), None);
    }

    #[test]
    fn preemption_and_nesting() {
        let mut n = nvic(&[(5, 0xC0), (2, 0x20)]);
        let mut out = [Start { t: 0, irq: 0 }; 8];
        let k = simulate(&mut n, &[(0, 5), (2, 2)], |irq| if irq == 5 { 5 } else { 2 }, 20, &mut out);
        assert_eq!(&out[..k], &[Start { t: 0, irq: 5 }, Start { t: 2, irq: 2 }, End { t: 4, irq: 2 }, End { t: 7, irq: 5 }]);
    }

    #[test]
    fn equal_priority_waits_then_tail_chains() {
        let mut n = nvic(&[(3, 0x40), (4, 0x5F)]);
        let mut out = [Start { t: 0, irq: 0 }; 8];
        let k = simulate(&mut n, &[(0, 3), (1, 4)], |irq| if irq == 3 { 3 } else { 2 }, 20, &mut out);
        assert_eq!(&out[..k], &[Start { t: 0, irq: 3 }, End { t: 3, irq: 3 }, Start { t: 3, irq: 4 }, End { t: 5, irq: 4 }]);
    }

    #[test]
    fn simultaneous_arrivals_ordered_by_priority_then_number() {
        let mut n = nvic(&[(9, 0x60), (7, 0x60), (8, 0x00)]);
        let mut out = [Start { t: 0, irq: 0 }; 8];
        let k = simulate(&mut n, &[(0, 9), (0, 7), (0, 8)], |_| 1, 10, &mut out);
        assert_eq!(
            &out[..k],
            &[Start { t: 0, irq: 8 }, End { t: 1, irq: 8 }, Start { t: 1, irq: 7 }, End { t: 2, irq: 7 }, Start { t: 2, irq: 9 }, End { t: 3, irq: 9 }]
        );
    }
}
