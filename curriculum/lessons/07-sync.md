# Interior Mutability, Atomics & Synchronization

Even a single-core microcontroller is concurrent: an **interrupt** can fire
between any two instructions of your main loop and run a handler that touches
the same data. Multicore MCUs (AURIX, RH850, dual Cortex-M7/M4 parts) add true
parallelism. Rust's answer is the same in both cases: the `Send`/`Sync` rules plus
a small toolbox of primitives you mostly build yourself in no_std.

## Send and Sync, for real this time

- `T: Send`: a `T` can be *moved* to another thread (or into an interrupt handler).
- `T: Sync`: a `&T` can be *shared* with another thread. Equivalently, `&T: Send`.

Every `static` is shared with every thread and every interrupt handler,
so **every static's type must be `Sync`**. That one rule is why you can't put
a `Cell<u32>` or `RefCell<Vec<u8>>` in a static: they allow mutation through `&`
without synchronisation, so they are `!Sync`.

## UnsafeCell: the only legal door

Mutating through a shared reference is undefined behaviour, *except* through
`core::cell::UnsafeCell<T>`. Its `get()` returns a `*mut T`, and the compiler
then assumes the contents may change behind a `&`. Every interior-mutability
type is a safe wrapper around it:

| Type | Mechanism | Sync? | Failure mode |
|---|---|---|---|
| `Cell<T>` | copy values in/out, never hand out references | no | none |
| `RefCell<T>` | runtime borrow counter | no | panics on conflicting borrow (`try_borrow*` returns `Err`) |
| `OnceCell<T>` | set once, then read-only | no | `set` returns `Err` if already set |
| `LazyCell<T, F>` | initialise on first access | no | |
| atomics | hardware atomic instructions | **yes** | |
| `critical_section::Mutex<T>` | access only inside a critical section | **yes** | |
| spinlock | atomic flag + busy waiting | **yes** | deadlock if an ISR spins on a lock held by the code it interrupted |

## Atomics

`core::sync::atomic` provides `AtomicBool`, `AtomicU8..64`, `AtomicI*`,
`AtomicUsize`, `AtomicPtr`. Every operation takes a memory `Ordering`:

```rust
use core::sync::atomic::{AtomicU32, Ordering};

static FRAMES: AtomicU32 = AtomicU32::new(0);

FRAMES.fetch_add(1, Ordering::Relaxed);           // returns the previous value
let n = FRAMES.load(Ordering::Relaxed);
FRAMES.fetch_max(n, Ordering::Relaxed);
let old = FRAMES.swap(0, Ordering::AcqRel);        // read-and-clear
// compare-and-swap loop: increment, but never beyond 100
let _ = FRAMES.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| (v < 100).then_some(v + 1));
```

### Memory ordering in one example

```rust
static DATA: AtomicU32 = AtomicU32::new(0);
static READY: AtomicBool = AtomicBool::new(false);

// producer (e.g. an ISR)
DATA.store(42, Ordering::Relaxed);
READY.store(true, Ordering::Release);    // everything before this...

// consumer (main loop)
if READY.load(Ordering::Acquire) {       // ...is visible after this
    assert_eq!(DATA.load(Ordering::Relaxed), 42);
}
```

- **Relaxed**: atomic, but no ordering with other memory. Fine for counters and
  statistics.
- **Release** (on a store) + **Acquire** (on the load that sees it): everything
  written before the release is visible after the acquire. This is how you
  *publish* data.
- **AcqRel**: both, for read-modify-write operations like `swap`/`compare_exchange`
  that both consume and publish.
- **SeqCst**: additionally one global order of all SeqCst operations. Rarely
  needed; often used "to be safe" (which works, at some cost).

On a single core, the CPU won't reorder its own memory operations as seen
by an interrupt on the same core, but **the compiler will**. Orderings
constrain both. `compiler_fence` constrains only the compiler; `fence`
constrains both.

### Not every target has every atomic

`thumbv6m` (Cortex-M0/M0+) has atomic loads and stores but **no
compare-and-swap**: no `fetch_add`, no `compare_exchange`. Check with
`#[cfg(target_has_atomic = "32")]` (CAS available for 32-bit) or use the
`portable-atomic` crate, which emulates them with critical sections. Few
32-bit MCUs have 64-bit atomics.

## Critical sections

On a single-core MCU, the simplest way to make a sequence of operations
atomic with respect to interrupts is to **disable interrupts** for the duration:
a *critical section*. The `critical-section` crate abstracts this portably:

```rust
use core::cell::RefCell;
use critical_section::Mutex;

static QUEUE: Mutex<RefCell<Option<Uart>>> = Mutex::new(RefCell::new(None));

fn main_loop() {
    critical_section::with(|cs| {
        let mut slot = QUEUE.borrow_ref_mut(cs);   // only callable with a token
        // ... use the UART ...
    });
}
```

The design is worth understanding, because it's a pattern you'll reuse:

- `critical_section::with(f)` disables interrupts, calls `f` with a
  **`CriticalSection<'cs>` token**, then restores them.
- `Mutex<T>::borrow(&'cs self, cs: CriticalSection<'cs>) -> &'cs T` hands out
  access **only if you can show a token**. The token can't be created outside
  `with`, and its lifetime ties the reference to the critical section, so the
  compiler proves you never touch the data with interrupts enabled.
- `Mutex<T>` is `Sync` (if `T: Send`) because of that guarantee, so it can live
  in a static. Combine with `RefCell` or `Cell` for mutation.
- The implementation (disable IRQs on Cortex-M, a global lock on hosted
  platforms, a hardware spinlock on some multicore parts) is chosen **once, by
  the final binary**, like the panic handler.

Keep critical sections short: interrupts are blocked for their whole duration,
and that adds to worst-case interrupt latency.

## Spinlocks, and why they bite

A spinlock is an `AtomicBool` plus busy-waiting:

```rust
while lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
    core::hint::spin_loop();
}
// critical region
lock.store(false, Ordering::Release);
```

On a **multicore** system with no interrupt sharing, that's fine. On a
**single core**, if main code holds the lock and an interrupt handler tries to take
it, the handler spins forever: the holder can't run until the handler
returns. **Deadlock.** Rule: data shared with interrupt handlers needs
critical sections (or lock-free structures), not spinlocks.

## Lock-free single-producer/single-consumer queues

The workhorse for moving data from an ISR to the main loop: a ring buffer
where the producer only writes `tail` and the consumer only writes `head`.
Each index has exactly one writer, so no locks are needed, just the right
orderings:

```text
producer: write slot[tail]; tail.store(next, Release)        // publish the item
consumer: if tail.load(Acquire) != head { read slot[head]; head.store(next, Release) }
producer: if next == head.load(Acquire) → full
```

One slot is always left empty to distinguish "full" from "empty", so capacity
is `N - 1`. This is `heapless::spsc::Queue`, and you'll write it.

> **Toyota lens:** "freedom from interference" (ISO 26262-6) includes
> interference *in time and data* between software components. AUTOSAR OS gives
> you resources (priority ceiling: a task temporarily runs at the highest
> priority of anything sharing the resource) and spinlocks for multicore. RTIC
> implements the same priority-ceiling protocol in Rust, checked at compile time.
> Be ready to explain priority inversion, why disabling interrupts bounds
> latency, and why a spinlock shared with an ISR deadlocks.

> **Interview:** "Implement a lock-free SPSC queue", "explain Acquire/Release",
> "why can't I put a RefCell in a static?", and "how do you share data between
> an interrupt handler and main?" (critical-section `Mutex<RefCell<T>>`, atomics,
> or an SPSC queue). All of these are in this module's exercises.
