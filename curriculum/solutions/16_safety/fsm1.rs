//! # fsm1: An ECU mode manager and fault debouncing
//!
//! **Part 1: modes.** Implement the transition function as one exhaustive
//! `match (mode, event)`. It returns the next mode and the actions to perform
//! (as data: the caller executes them), or `None` for an invalid transition.
//!
//! | From | Event | To | Actions |
//! |---|---|---|---|
//! | Off | PowerOn | Startup | RunSelfTest |
//! | Startup | SelfTestPassed | Run | EnableOutputs |
//! | Startup | SelfTestFailed | SafeState | DisableOutputs, StoreDtc(0x0001) |
//! | Run | FaultMinor | Degraded | LimitTorque, StoreDtc(0x0100) |
//! | Run, Degraded | FaultMajor | SafeState | DisableOutputs, StoreDtc(0x0200) |
//! | Degraded | FaultCleared | Run | EnableOutputs |
//! | Run, Degraded | PowerOffRequest | Shutdown | DisableOutputs, SaveNvm |
//! | SafeState | PowerOffRequest | Shutdown | SaveNvm |
//! | Shutdown | ShutdownComplete | Off | PowerDown |
//!
//! Everything else is invalid. In particular SafeState is **latched**: a
//! cleared fault doesn't leave it, only a power cycle does. `ModeManager`
//! applies transitions, leaving the mode unchanged and counting the rejection
//! when a transition is invalid.
//!
//! **Part 2: DEM-style counter-based debouncing.** `report(prefailed)`
//! moves an internal counter by `+inc` (pre-failed) or `-dec` (pre-passed),
//! clamped to `[pass_threshold, fail_threshold]`. Reaching `fail_threshold`
//! qualifies the fault: return `Some(Verdict::Failed)` **once**, on the report
//! that reaches it. Reaching `pass_threshold` heals it: `Some(Verdict::Passed)`
//! once, when coming from a failed state (or on the first time ever reaching it).
//! Otherwise `None`.
#![no_std]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Off,
    Startup,
    Run,
    Degraded,
    SafeState,
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    PowerOn,
    SelfTestPassed,
    SelfTestFailed,
    FaultMinor,
    FaultMajor,
    FaultCleared,
    PowerOffRequest,
    ShutdownComplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    RunSelfTest,
    EnableOutputs,
    DisableOutputs,
    LimitTorque,
    StoreDtc(u16),
    SaveNvm,
    PowerDown,
}

pub fn transition(mode: Mode, event: Event) -> Option<(Mode, &'static [Action])> {
    use Action::*;
    use Event::*;
    use Mode::*;
    Some(match (mode, event) {
        (Off, PowerOn) => (Startup, &[RunSelfTest]),
        (Startup, SelfTestPassed) => (Run, &[EnableOutputs]),
        (Startup, SelfTestFailed) => (SafeState, &[DisableOutputs, StoreDtc(0x0001)]),
        (Run, FaultMinor) => (Degraded, &[LimitTorque, StoreDtc(0x0100)]),
        (Run | Degraded, FaultMajor) => (SafeState, &[DisableOutputs, StoreDtc(0x0200)]),
        (Degraded, FaultCleared) => (Run, &[EnableOutputs]),
        (Run | Degraded, PowerOffRequest) => (Shutdown, &[DisableOutputs, SaveNvm]),
        (SafeState, PowerOffRequest) => (Shutdown, &[SaveNvm]),
        (Shutdown, ShutdownComplete) => (Off, &[PowerDown]),
        _ => return None,
    })
}

pub struct ModeManager {
    mode: Mode,
    rejected: u32,
}

impl ModeManager {
    pub const fn new() -> Self {
        ModeManager { mode: Mode::Off, rejected: 0 }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn rejected(&self) -> u32 {
        self.rejected
    }

    /// Apply `event`. On success returns the actions; on an invalid transition
    /// returns `Err(current mode)` and leaves the mode unchanged.
    pub fn handle(&mut self, event: Event) -> Result<&'static [Action], Mode> {
        match transition(self.mode, event) {
            Some((next, actions)) => {
                self.mode = next;
                Ok(actions)
            }
            None => {
                self.rejected = self.rejected.saturating_add(1);
                Err(self.mode)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Passed,
    Failed,
}

pub struct Debounce {
    counter: i16,
    inc: i16,
    dec: i16,
    fail_threshold: i16,
    pass_threshold: i16,
    last: Option<Verdict>,
}

impl Debounce {
    pub const fn new(inc: i16, dec: i16, fail_threshold: i16, pass_threshold: i16) -> Self {
        Debounce { counter: 0, inc, dec, fail_threshold, pass_threshold, last: None }
    }

    pub fn counter(&self) -> i16 {
        self.counter
    }

    pub fn report(&mut self, prefailed: bool) -> Option<Verdict> {
        self.counter = if prefailed {
            self.counter.saturating_add(self.inc).min(self.fail_threshold)
        } else {
            self.counter.saturating_sub(self.dec).max(self.pass_threshold)
        };
        let verdict = if self.counter >= self.fail_threshold {
            Verdict::Failed
        } else if self.counter <= self.pass_threshold {
            Verdict::Passed
        } else {
            return None;
        };
        if self.last == Some(verdict) {
            return None;
        }
        self.last = Some(verdict);
        Some(verdict)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Action::*;
    use Event::*;
    use Mode::*;

    #[test]
    fn happy_path() {
        let mut m = ModeManager::new();
        assert_eq!(m.handle(PowerOn), Ok(&[RunSelfTest][..]));
        assert_eq!(m.handle(SelfTestPassed), Ok(&[EnableOutputs][..]));
        assert_eq!(m.mode(), Run);
        assert_eq!(m.handle(PowerOffRequest), Ok(&[DisableOutputs, SaveNvm][..]));
        assert_eq!(m.handle(ShutdownComplete), Ok(&[PowerDown][..]));
        assert_eq!(m.mode(), Off);
        assert_eq!(m.rejected(), 0);
    }

    #[test]
    fn degradation_and_recovery() {
        let mut m = ModeManager::new();
        m.handle(PowerOn).unwrap();
        m.handle(SelfTestPassed).unwrap();
        assert_eq!(m.handle(FaultMinor), Ok(&[LimitTorque, StoreDtc(0x0100)][..]));
        assert_eq!(m.mode(), Degraded);
        assert_eq!(m.handle(FaultMinor), Err(Degraded), "already degraded");
        assert_eq!(m.handle(FaultCleared), Ok(&[EnableOutputs][..]));
        assert_eq!(m.mode(), Run);
        assert_eq!(m.rejected(), 1);
    }

    #[test]
    fn safe_state_is_latched() {
        let mut m = ModeManager::new();
        m.handle(PowerOn).unwrap();
        m.handle(SelfTestPassed).unwrap();
        m.handle(FaultMinor).unwrap();
        assert_eq!(m.handle(FaultMajor), Ok(&[DisableOutputs, StoreDtc(0x0200)][..]));
        for e in [FaultCleared, SelfTestPassed, PowerOn, FaultMinor, FaultMajor] {
            assert_eq!(m.handle(e), Err(SafeState), "{e:?} must not leave SafeState");
        }
        assert_eq!(m.handle(PowerOffRequest), Ok(&[SaveNvm][..]));
        assert_eq!(m.mode(), Shutdown);
    }

    #[test]
    fn self_test_failure() {
        assert_eq!(transition(Startup, SelfTestFailed), Some((SafeState, &[DisableOutputs, StoreDtc(0x0001)][..])));
        assert_eq!(transition(Off, SelfTestPassed), None);
        assert_eq!(transition(Shutdown, PowerOn), None);
    }

    #[test]
    fn every_pair_is_handled() {
        let modes = [Off, Startup, Run, Degraded, SafeState, Shutdown];
        let events = [PowerOn, SelfTestPassed, SelfTestFailed, FaultMinor, FaultMajor, FaultCleared, PowerOffRequest, ShutdownComplete];
        let valid = modes.iter().flat_map(|&m| events.iter().map(move |&e| transition(m, e))).filter(Option::is_some).count();
        assert_eq!(valid, 11);
    }

    #[test]
    fn debouncing() {
        // +3 per pre-failed, -1 per pre-passed, failed at 9, passed at -4
        let mut d = Debounce::new(3, 1, 9, -4);
        assert_eq!(d.report(true), None);
        assert_eq!(d.report(false), None, "a single glitch is filtered");
        assert_eq!(d.report(true), None);
        assert_eq!(d.report(true), None);
        assert_eq!(d.counter(), 8);
        assert_eq!(d.report(true), Some(Verdict::Failed));
        assert_eq!(d.counter(), 9, "clamped at the threshold");
        assert_eq!(d.report(true), None, "reported once");
        for _ in 0..12 {
            assert_eq!(d.report(false), None);
        }
        assert_eq!(d.report(false), Some(Verdict::Passed));
        assert_eq!(d.counter(), -4);
        assert_eq!(d.report(false), None);
    }

    #[test]
    fn first_pass_is_reported() {
        let mut d = Debounce::new(1, 1, 2, -2);
        assert_eq!(d.report(false), None);
        assert_eq!(d.report(false), Some(Verdict::Passed));
    }
}
