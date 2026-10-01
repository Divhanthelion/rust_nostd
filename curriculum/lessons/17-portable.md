# Portable Crates, Lints & Tooling

The last technical module is about the code *around* your code: how a crate
supports no_std and std users at once, how lints turn conventions into
guarantees, and how tests, size budgets and toolchains are managed in a
professional embedded project.

## One crate, three audiences

The idiomatic layout for a portable library:

```toml
# Cargo.toml
[features]
default = []          # some crates default to ["std"]; for embedded users that's a trap
alloc = []
std = ["alloc"]       # std implies alloc
```

```rust
// lib.rs
#![no_std]                                  // always: the prelude stays the same everywhere

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub fn parse_into(input: &str, out: &mut [u16]) -> Result<usize, Error> { ... }   // core only

#[cfg(feature = "alloc")]
pub fn parse(input: &str) -> Result<alloc::vec::Vec<u16>, Error> { ... }          // needs a heap

#[cfg(feature = "std")]
pub fn parse_reader(r: impl std::io::BufRead) -> Result<alloc::vec::Vec<u16>, ReadError> { ... }
```

The alternative, `#![cfg_attr(not(feature = "std"), no_std)]`, changes the
prelude depending on the feature, which makes it easy to write code that only
compiles with std by accident. Prefer the always-`no_std` form.

Rules:

- **Features must be additive**: enabling one may only *add* API. Cargo
  unifies features across the whole dependency graph, so if any crate enables
  `std` on your dependency, everyone gets it.
- **Depend with `default-features = false`** and forward features explicitly,
  e.g. `std = ["alloc", "serde/std"]` (for dependencies that have such a feature).
- **Test every feature combination** in CI (`--no-default-features`,
  `--features alloc`, `--all-features`), and **build for a bare-metal
  target**, the only proof that no dependency pulled std back in:
  `cargo build --no-default-features --target thumbv7em-none-eabihf`.
- Inspect what enabled what: `cargo tree -e features -i <crate>`.

Other useful `cfg`s: `target_has_atomic = "32"`/`"ptr"` (CAS availability),
`target_os = "none"` (bare metal), `target_arch`, `panic = "abort"`, and
`#[cfg_attr(feature = "defmt", derive(defmt::Format))]` for optional logging
support.

## Lints as guarantees

Compiler lints catch whole classes of problems, and `deny` turns them into
build errors. Configure them per crate in `Cargo.toml` (`[lints]` table, Rust
1.74+) or with attributes:

| Lint | Catches |
|---|---|
| `#![forbid(unsafe_code)]` | any `unsafe` in application/logic crates (confine unsafe to a few audited crates) |
| `unsafe_op_in_unsafe_fn` | unsafe operations without their own block (default warn in 2024) |
| `missing_docs` | public items without documentation |
| `unused_must_use`, `unused_results` | ignored `Result`s and return values |
| `trivial_casts`, `trivial_numeric_casts` | `x as u32` when x is already u32 (noise that hides real casts) |
| `missing_debug_implementations` | public types you can't log or assert on |
| `elided_lifetimes_in_paths` | `Formatter` instead of `Formatter<'_>`: hidden borrows |
| `unused_qualifications` | `core::cmp::max` when `max` is in scope |

**Clippy** adds hundreds more. Its *restriction* group is opt-in and aimed at
safety-critical code:

```toml
[lints.clippy]
unwrap_used = "deny"            # also expect_used, panic, todo, unimplemented
indexing_slicing = "deny"       # v[i] can panic: use get()
arithmetic_side_effects = "warn"  # every + - * that can overflow
as_conversions = "warn"         # prefer From/TryFrom
cast_possible_truncation = "deny"
undocumented_unsafe_blocks = "deny"   # every unsafe block needs a // SAFETY: comment
missing_safety_doc = "deny"     # every unsafe fn needs a # Safety section
float_arithmetic = "warn"       # where floats are banned
```

A mature setup: strict lints in the library crates, `allow` with a written
justification (`#[allow(clippy::indexing_slicing, reason = "...")]`, Rust 1.81)
where a lint is wrong, and the whole configuration under review.

## Testing a no_std codebase

| Level | Tools | Runs on |
|---|---|---|
| unit tests | `cargo test` with `#[cfg(test)] extern crate std` | host |
| property tests | `proptest`, `quickcheck` (generate thousands of inputs) | host |
| fuzzing | `cargo fuzz` (libFuzzer) for parsers and decoders | host |
| UB detection | `cargo +nightly miri test` | host (interpreted) |
| model checking | **Kani**: prove properties for all inputs up to bounds | host |
| mutation testing | `cargo mutants`: are your tests strong enough? | host |
| on-target tests | `embedded-test` / `defmt-test` + probe-rs | the MCU |
| emulation | QEMU in CI (as in module 12) | CI |
| HIL | hardware-in-the-loop rigs | lab |

Coverage: `cargo llvm-cov` (statement/branch); MC/DC support in rustc is
still experimental, so safety projects currently combine tools.

The architecture that makes this work is the one this course has used
throughout: **logic in pure no_std libraries** (parsers, state machines,
controllers, protocols) tested exhaustively on the host, and **thin
hardware layers** behind traits (embedded-hal) tested with mocks and on target.

## Size and speed

```toml
[profile.release]
opt-level = "s"        # or "z" for even smaller, or 3 for speed
lto = "fat"
codegen-units = 1
panic = "abort"
debug = true           # debug info stays in the ELF, never reaches flash
strip = false          # keep symbols for the debugger; flash images are made with objcopy
```

- Measure: `cargo size --release -- -A` (cargo-binutils), `cargo bloat
  --release -n 20`, and the linker map file.
- Big wins: no `core::fmt` (or `defmt`), no float formatting, avoiding
  generic code bloat (monomorphisation), `panic = "abort"` with a minimal
  handler.
- Nightly-only: `-Z build-std=core,alloc` rebuilds core with your settings,
  and the `panic_immediate_abort` feature removes all panic-message code.

## Toolchains and supply chain

- Pin the compiler: `rust-toolchain.toml` (`channel = "1.97.0"`, targets,
  components). Safety projects use a **qualified** toolchain (Ferrocene) at a
  fixed version.
- Declare your MSRV: `rust-version = "1.85"` in Cargo.toml.
- Commit `Cargo.lock` for firmware and binaries.
- Audit dependencies: `cargo deny` (licenses, bans, advisories), `cargo
  audit`, `cargo vet` (human review records). Fewer dependencies = smaller
  review and qualification effort.

> **Toyota lens:** ASPICE and ISO 26262 both require configuration management,
> reproducible builds, documented tool usage and (for tools) qualification or
> confidence arguments. "We pin the toolchain, lock dependencies, deny
> unwraps and unsafe outside audited modules, test every feature combination,
> fuzz the parsers, and build for the real target in CI" is a sentence that
> lands well in an interview for a safety-relevant Rust role.
