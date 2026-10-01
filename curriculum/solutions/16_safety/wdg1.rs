//! # wdg1: A watchdog manager
//!
//! A software supervisor in the style of AUTOSAR's WdgM. It decides whether
//! the hardware watchdog may be kicked. Three kinds of supervision:
//!
//! - **Alive**: entity `i` must call `alive(i)` between `min` and `max` times
//!   per supervision cycle (inclusive).
//! - **Deadline**: `deadline_end(now)` must follow `deadline_start(t0)` within
//!   `deadline_max` ticks (use `wrapping_sub`: the tick counter wraps). An end
//!   without a start, or a late end, is a violation of the current cycle.
//! - **Logical**: checkpoints (0..16) must follow the allowed control flow:
//!   `transitions[a]` has bit `b` set if checkpoint `b` may follow `a`. The
//!   first checkpoint is always allowed. A violation (or a checkpoint ≥ 16) is
//!   corrupted control flow: the status becomes `Expired` **immediately**.
//!
//! `end_cycle()` evaluates alive and deadline supervision for the cycle that
//! just ended, then resets the per-cycle counters and deadline flag:
//! - all good → `Ok` (and the count of consecutive failed cycles resets);
//! - otherwise the consecutive-failure count increments: `Failed` while it is
//!   `<= tolerance`, `Expired` once it exceeds it.
//! - `Expired` is permanent (only a reset recovers).
//!
//! `kick_allowed()` is true unless the status is `Expired`: then the
//! hardware watchdog bites and the MCU resets into a safe state.
//! Out-of-range entity indices are ignored by `alive` (return false).
#![no_std]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Failed,
    Expired,
}

#[derive(Debug, Clone, Copy)]
pub struct AliveCfg {
    pub min: u8,
    pub max: u8,
}

pub struct Supervisor<const N: usize> {
    alive_cfg: [AliveCfg; N],
    alive_count: [u8; N],
    tolerance: u8,
    failed_cycles: u8,
    status: Status,
    transitions: [u16; 16],
    last_checkpoint: Option<u8>,
    deadline_max: u32,
    deadline_start: Option<u32>,
    deadline_violated: bool,
}

impl<const N: usize> Supervisor<N> {
    pub fn new(alive_cfg: [AliveCfg; N], tolerance: u8, transitions: [u16; 16], deadline_max: u32) -> Self {
        Supervisor {
            alive_cfg,
            alive_count: [0; N],
            tolerance,
            failed_cycles: 0,
            status: Status::Ok,
            transitions,
            last_checkpoint: None,
            deadline_max,
            deadline_start: None,
            deadline_violated: false,
        }
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn kick_allowed(&self) -> bool {
        self.status != Status::Expired
    }

    pub fn alive(&mut self, entity: usize) -> bool {
        match self.alive_count.get_mut(entity) {
            Some(c) => {
                *c = c.saturating_add(1);
                true
            }
            None => false,
        }
    }

    pub fn checkpoint(&mut self, cp: u8) {
        let allowed = match (self.last_checkpoint, self.transitions.get(usize::from(cp))) {
            (_, None) => false,
            (None, Some(_)) => true,
            (Some(prev), Some(_)) => self.transitions.get(usize::from(prev)).is_some_and(|t| t & (1 << cp) != 0),
        };
        if !allowed {
            self.status = Status::Expired;
        }
        self.last_checkpoint = Some(cp);
    }

    pub fn deadline_start(&mut self, now: u32) {
        self.deadline_start = Some(now);
    }

    pub fn deadline_end(&mut self, now: u32) {
        match self.deadline_start.take() {
            Some(t0) if now.wrapping_sub(t0) <= self.deadline_max => {}
            _ => self.deadline_violated = true,
        }
    }

    pub fn end_cycle(&mut self) -> Status {
        let alive_ok = self.alive_cfg.iter().zip(&self.alive_count).all(|(cfg, &n)| n >= cfg.min && n <= cfg.max);
        let ok = alive_ok && !self.deadline_violated;
        self.alive_count = [0; N];
        self.deadline_violated = false;
        if self.status == Status::Expired {
            return self.status;
        }
        if ok {
            self.failed_cycles = 0;
            self.status = Status::Ok;
        } else {
            self.failed_cycles = self.failed_cycles.saturating_add(1);
            self.status = if self.failed_cycles > self.tolerance { Status::Expired } else { Status::Failed };
        }
        self.status
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Control flow 0 → 1 → 2 → 0 …, with 1 → 3 → 2 as an alternative path.
    fn flow() -> [u16; 16] {
        let mut t = [0u16; 16];
        t[0] = 1 << 1;
        t[1] = (1 << 2) | (1 << 3);
        t[3] = 1 << 2;
        t[2] = 1 << 0;
        t
    }

    fn sup(tolerance: u8) -> Supervisor<2> {
        Supervisor::new([AliveCfg { min: 1, max: 2 }, AliveCfg { min: 3, max: 5 }], tolerance, flow(), 100)
    }

    fn healthy_cycle(s: &mut Supervisor<2>) {
        s.alive(0);
        for _ in 0..4 {
            s.alive(1);
        }
    }

    #[test]
    fn healthy_system() {
        let mut s = sup(1);
        for _ in 0..5 {
            healthy_cycle(&mut s);
            assert_eq!(s.end_cycle(), Status::Ok);
        }
        assert!(s.kick_allowed());
    }

    #[test]
    fn alive_too_few_or_too_many() {
        let mut s = sup(5);
        s.alive(1);
        s.alive(1);
        s.alive(1);
        assert_eq!(s.end_cycle(), Status::Failed, "entity 0 never reported");
        s.alive(0);
        s.alive(0);
        s.alive(0);
        for _ in 0..3 {
            s.alive(1);
        }
        assert_eq!(s.end_cycle(), Status::Failed, "entity 0 reported 3 > max 2: runaway");
        healthy_cycle(&mut s);
        assert_eq!(s.end_cycle(), Status::Ok, "recovers within tolerance");
        assert!(!s.alive(7));
    }

    #[test]
    fn tolerance_then_expiry() {
        let mut s = sup(2);
        assert_eq!(s.end_cycle(), Status::Failed);
        assert_eq!(s.end_cycle(), Status::Failed);
        assert!(s.kick_allowed());
        assert_eq!(s.end_cycle(), Status::Expired, "3 consecutive failures > tolerance 2");
        assert!(!s.kick_allowed());
        healthy_cycle(&mut s);
        assert_eq!(s.end_cycle(), Status::Expired, "expired is permanent");
    }

    #[test]
    fn deadlines() {
        let mut s = sup(3);
        healthy_cycle(&mut s);
        s.deadline_start(u32::MAX - 10);
        s.deadline_end(50);
        assert_eq!(s.end_cycle(), Status::Ok, "61 ticks across the wrap");
        healthy_cycle(&mut s);
        s.deadline_start(0);
        s.deadline_end(101);
        assert_eq!(s.end_cycle(), Status::Failed);
        healthy_cycle(&mut s);
        s.deadline_end(5);
        assert_eq!(s.end_cycle(), Status::Failed, "end without start");
        healthy_cycle(&mut s);
        assert_eq!(s.end_cycle(), Status::Ok);
    }

    #[test]
    fn logical_supervision() {
        let mut s = sup(3);
        for cp in [0, 1, 2, 0, 1, 3, 2, 0] {
            s.checkpoint(cp);
        }
        healthy_cycle(&mut s);
        assert_eq!(s.end_cycle(), Status::Ok);
        s.checkpoint(2); // 0 → 2 is not allowed
        assert_eq!(s.status(), Status::Expired, "immediately");
        assert!(!s.kick_allowed());

        let mut s = sup(3);
        s.checkpoint(1);
        s.checkpoint(16);
        assert_eq!(s.status(), Status::Expired);
    }
}
