# Automotive II: Functional Safety Patterns

Functional safety asks a simple question with hard answers: *when something
goes wrong (a bit flips, a sensor sticks, a task overruns, a message is
lost), does the system still avoid hurting anyone?* This module gives you
the vocabulary of ISO 26262 and the building blocks that appear in nearly
every safety-relevant ECU, implemented in no_std Rust.

## ISO 26262 in one page

- **HARA** (hazard analysis and risk assessment) rates each hazardous event
  by **S**everity, **E**xposure and **C**ontrollability, yielding an **ASIL**:
  QM (normal quality management), then A (lowest) to D (highest). An
  electric power steering failure is typically ASIL D; a rear-light
  failure is lower.
- **Safety goals** ("avoid unintended steering torque") are refined into
  functional and technical safety requirements, then into hardware and
  software requirements: the left side of the **V-model**, verified bottom-up
  on the right side.
- **Safe state**: a condition with no unreasonable risk, for example outputs
  off, torque limited, or "limp home". Systems must reach it within the
  **FTTI** (fault-tolerant time interval) after a fault occurs.
- **Freedom from interference**: lower-ASIL software must not corrupt
  higher-ASIL software, in **space** (memory: MPU, ownership), **time**
  (CPU: watchdogs, timing protection), and **communication** (data: E2E
  protection).
- **ASIL decomposition** splits a requirement into redundant, independent
  parts with lower ASILs (e.g. ASIL D = ASIL B(D) + ASIL B(D)).
- **Part 6 (software)** asks for coding guidelines, defensive programming,
  static analysis, requirement-based unit tests and structural coverage (up
  to **MC/DC** at ASIL D), among other things.

Related standards: **ISO/SAE 21434** (cybersecurity engineering) and
UNECE R155/R156 (cybersecurity/software-update regulation); **ASPICE** for
process maturity; **IEC 61508** is the generic industrial parent of 26262.

## Where Rust fits

| MISRA C concern | Rust mechanism |
|---|---|
| no dynamic memory (21.3) | `#![no_std]`, no `alloc`, fixed-capacity types |
| array bounds, pointer arithmetic (18.x) | slices and bounds checks; raw pointers confined to `unsafe` |
| uninitialised variables (9.1) | definite initialisation is a compile error |
| conversions (10.x) | no implicit conversions; `From`/`TryFrom`; `as` is greppable and lintable |
| unchecked error codes (Dir 4.7) | `Result` + `#[must_use]` |
| undefined behaviour (1.3) | impossible in safe Rust; `unsafe` blocks are explicit and auditable |
| data races | `Send`/`Sync`: compile-time |
| recursion (17.2), unbounded loops | still your job: lints, reviews, analysis tools |

New Rust-specific concerns: panics (policy + avoidance), integer overflow
behaviour (enable `overflow-checks`), stack usage, and the soundness of every
`unsafe` block. Toolchain: **Ferrocene** is qualified for ISO 26262 up to
ASIL D (TCL 3) and IEC 61508, ships a certified subset of `core`, and pins
exact compiler versions; the **Safety-Critical Rust Consortium** (Woven by
Toyota is a member) publishes coding guidelines. Verification tools beyond
tests: Miri (UB detection), **Kani** (bounded model checking of Rust code),
clippy restriction lints, and `-C instrument-coverage` (MC/DC coverage in
rustc is still experimental).

## E2E protection: trusting data from the bus

AUTOSAR's End-to-End (E2E) library protects safety-relevant signals against
the failure modes of communication: corruption, repetition, loss, delay,
insertion, masquerading and wrong sequence. The sender adds a small header;
the receiver checks it every cycle:

| Mechanism | Detects |
|---|---|
| **CRC** over data **plus a Data ID** (the ID isn't transmitted) | corruption; masquerade (wrong message with valid CRC) |
| **alive counter** incremented per message | repetition, loss, wrong sequence |
| **timeout** (no new data within a deadline) | loss, delay |

Profile 1 (classic CAN): CRC-8/SAE-J1850 in byte 0, a 4-bit counter
(0..=14) in byte 1, Data ID mixed into the CRC. The receiver classifies each
cycle as OK, REPEATED, OK-SOME-LOST (counter jumped by less than a maximum),
WRONG-SEQUENCE, WRONG-CRC or NO-NEW-DATA, and the application decides
whether to keep using the value (often with a validity state machine).

## Watchdogs: is the software still doing what it should?

- A **hardware watchdog** resets the MCU unless it's "kicked" in time. A
  *window* watchdog also rejects kicks that come too early (a runaway loop
  kicking constantly is caught too).
- A **watchdog manager** (AUTOSAR WdgM) supervises software entities and
  kicks the hardware watchdog **only while supervision is OK**:
  - **alive supervision**: each entity reports checkpoints; per supervision
    cycle the count must lie within [min, max] (too few = stuck/slow, too many =
    runaway);
  - **deadline supervision**: time between a start and an end checkpoint
    must not exceed a limit;
  - **logical supervision**: checkpoints must occur in an allowed order (a
    control-flow graph). Jumping into the middle means corrupted control flow.
- Failures within a configured tolerance are reported as FAILED; beyond it,
  or for logical violations, the status becomes EXPIRED, kicking stops, and
  the hardware watchdog brings the system into its safe state (reset).

## Fixed-point arithmetic

Many automotive MCUs have no FPU, or projects forbid floats in safety code
because results depend on rounding modes and float libraries need their own
qualification. **Fixed-point** represents a real number as an integer
scaled by 2⁻ⁿ. In **Q16.16**, the raw `i32` value 98 304 means 98304 / 65536 = 1.5.

- add/sub: plain integer add/sub (saturating!);
- multiply: widen to `i64`, multiply, shift right by 16, saturate to `i32`;
- divide: widen, shift left by 16, divide, saturate; division by zero is an
  error, not a panic;
- conversions: decide and document the rounding (toward zero, half away).

Control loops (PID) built on saturating fixed-point are deterministic,
bit-exact across platforms, and testable on the host. A classic safety detail
is **anti-windup**: when the output saturates, stop integrating in the
saturating direction, or the integral term "winds up" and the controller
overshoots badly once the error reverses.

## Mode management and state machines

ECUs run explicit mode machines (Off → Startup → Run → Degraded → SafeState
→ Shutdown), and AUTOSAR's BswM/EcuM/DEM coordinate modes and diagnostics.
Rust makes them precise:

- states and events as enums; the transition function as one exhaustive
  `match (state, event)`, which the compiler checks covers everything;
- **invalid transitions are explicit errors** that leave the state
  unchanged (and are typically reported);
- actions (enable outputs, store a DTC, save NVM data) are returned as data
  rather than performed as side effects, which keeps the logic pure and testable;
- some states are **latched**: leaving SafeState usually requires a power cycle.

Fault detection usually goes through **debouncing**: AUTOSAR DEM's
*counter-based debouncing* increments a counter on each "pre-failed" report
and decrements it on "pre-passed"; reaching the failed threshold qualifies
the fault (stores a DTC), and reaching the passed threshold heals it. That
prevents single glitches from triggering reactions while still detecting
real faults within a bounded time.

> **Toyota lens:** these four patterns (E2E, watchdog supervision, fixed-point
> control, mode/DTC management) are standard in Toyota/Denso and other OEM/
> tier-1 ECUs, usually as AUTOSAR modules in C. Being able to implement them in
> Rust *and* explain the hazards they mitigate ("the alive counter catches a
> frozen sender; the Data ID catches a masquerading message") shows exactly the
> safety literacy that job postings ask for under "ISO 26262 experience".

> **Interview:** "What's ASIL and how is it determined?", "What is freedom from
> interference?", "How does E2E protection detect a repeated message?", "Why
> stop kicking the watchdog?", "Explain integrator windup."
