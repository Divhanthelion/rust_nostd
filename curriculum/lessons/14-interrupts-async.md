# Interrupts & Async

Firmware is reactive: a byte arrives, a timer expires, a CAN frame lands, a
button bounces. This module covers the three ways Rust firmware organises
that concurrency (interrupt handlers, RTIC, and async executors) and the
mechanics underneath each.

## Interrupts on a Cortex-M

The **NVIC** (Nested Vectored Interrupt Controller) manages device interrupts:

- each IRQ has an **enable** bit, a **pending** bit and a **priority**;
- **lower priority number = more urgent**. Only the top bits of the 8-bit
  priority field are implemented (often 3 or 4 bits: 8 or 16 levels);
- an interrupt **preempts** the running code only if its priority is
  *strictly* more urgent than the current execution priority; otherwise it
  stays pending until the current handler returns (*tail-chaining* then runs
  it without a full context restore);
- **PRIMASK** masks all configurable interrupts (`cpsid i`): this is what a
  single-core critical section does;
- **BASEPRI** masks only interrupts with priority values `>=` BASEPRI. RTIC uses
  it to lock a resource without blocking more urgent, unrelated interrupts.

With `cortex-m-rt` and a PAC, a handler is a function named after its vector:

```rust
#[interrupt]
fn USART1() {
    // read the data register (this also clears the RX interrupt flag)
}
```

### Rules for interrupt handlers

1. **Keep them short.** Copy the data, clear the flag, signal the main loop.
   Long work raises the latency of everything at equal or lower priority.
2. **Never block**: no spinning on locks held by lower priority code (deadlock),
   no busy-waits for slow hardware.
3. **Always clear the interrupt source**, or the handler re-enters forever.
4. **Share data safely**: atomics, critical-section mutexes, lock-free
   queues (module 07).
5. **No allocation**, no panicking paths if you can help it.

The classic structure is *top half / bottom half*: the ISR does the minimum
and raises an event flag; the main loop does the rest.

```rust
static EVENTS: AtomicU32 = AtomicU32::new(0);
const RX_LINE: u32 = 1 << 0;

#[interrupt]
fn USART1() { /* ...buffer the byte... */ EVENTS.fetch_or(RX_LINE, Ordering::Release); }

loop {
    let ev = EVENTS.swap(0, Ordering::Acquire);
    if ev & RX_LINE != 0 { handle_line(); }
    if ev == 0 { sleep(); }
}
```

### The lost-wakeup race

`if no events { wfi() }` has a race: an interrupt may arrive *after* the check
and *before* `wfi`; then the CPU sleeps with an event waiting. The fix uses a
hardware detail: `wfi` wakes on a **pending** interrupt even while PRIMASK
masks it. So disable interrupts, check, `wfi`, then re-enable:

```rust
cortex_m::interrupt::disable();
if EVENTS.load(Ordering::Relaxed) == 0 { cortex_m::asm::wfi(); }
unsafe { cortex_m::interrupt::enable() };      // the pending ISR runs now
```

## RTIC: interrupts as a scheduler

**RTIC** (Real-Time Interrupt-driven Concurrency) turns the NVIC into a
scheduler. Tasks are bound to interrupts with priorities. Shared resources
are declared up front, and the framework computes each resource's **priority
ceiling** at compile time, so a lock just raises BASEPRI to the ceiling: no
deadlocks, no priority inversion, bounded blocking. Accessing a resource
without a lock when it's needed is a compile error. It is the
*Stack Resource Policy* / priority-ceiling protocol, also used by AUTOSAR OS
resources, statically checked.

## async/await without an operating system

`async fn` compiles to a **state machine** (an enum holding the locals live
across each `.await`) implementing `core::future::Future`:

```rust
pub trait Future {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}
```

- `poll` makes as much progress as it can. If it can't finish, it stores
  `cx.waker()` somewhere and returns `Poll::Pending`.
- When the awaited event happens (often in an **interrupt handler**), someone
  calls `waker.wake()`, and the executor polls the task again.
- An **executor** owns tasks and polls the ready ones. It sleeps (`wfi`) when
  none are ready.

All of that is in `core`: `Future`, `Poll`, `Context`, `Waker`, `RawWaker`,
`RawWakerVTable`, `pin!`, `poll_fn`, `ready`, and `Waker::noop()` (Rust 1.85).
What core does *not* include is an executor. You'll write two.

### Wakers from scratch

A `Waker` is a data pointer plus a table of four functions:

```rust
static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
unsafe fn clone(p: *const ()) -> RawWaker { RawWaker::new(p, &VTABLE) }
unsafe fn wake(p: *const ()) { wake_by_ref(p) }
unsafe fn wake_by_ref(p: *const ()) { /* mark the task ready, e.g. set a bit */ }
unsafe fn drop(_: *const ()) {}
let waker = unsafe { Waker::from_raw(RawWaker::new(task_id as *const (), &VTABLE)) };
```

Embedded executors avoid reference counting entirely: the data pointer
encodes *which task* to mark ready in a static bitmask or run queue.

### Embassy

**Embassy** is the leading embedded async framework: a static executor
(tasks allocated at compile time with `#[embassy_executor::task]`), async HALs
whose futures are woken from interrupts, `embassy-time` timers, and
`embassy-sync` channels/mutexes/signals. A typical task:

```rust
#[embassy_executor::task]
async fn blink(mut led: Output<'static>) {
    loop {
        led.toggle();
        Timer::after_millis(500).await;   // the core sleeps; a timer IRQ wakes the task
    }
}
```

Things to know about async firmware:

- **futures are values**: their size is the size of the state machine. Large
  locals held across `.await` make big futures (and big task arenas);
- **cancellation = drop**: `select` drops the losing future mid-flight, so code
  must stay correct if dropped at any `.await` (e.g. don't leave a peripheral
  half-configured);
- **cooperative**: a task that never awaits starves the others on the same
  executor. Run hard real-time work in interrupts or a higher-priority
  executor (Embassy supports interrupt-driven executors per priority).

## Choosing a concurrency model

| Model | Strengths | Watch out for |
|---|---|---|
| superloop + ISRs + flags | simple, transparent | ad-hoc state machines grow messy |
| RTIC | hard real-time, compile-time-checked sharing, tiny | static task set |
| Embassy async | readable sequential code, low power, rich ecosystem | cooperative scheduling, future sizes |
| RTOS (FreeRTOS, Zephyr, AUTOSAR OS via FFI) | preemptive threads, certified options | per-thread stacks, C integration |

> **Toyota lens:** AUTOSAR Classic OS schedules statically configured
> *tasks* and *ISRs* (category 1: no OS calls; category 2: OS-managed), shares
> data through *resources* using the priority-ceiling protocol, and enforces
> *timing protection* (execution budgets). The same ideas appear in RTIC.
> Safety analyses ask for worst-case execution time and worst-case response
> time, so your design must keep ISRs short and bounded, and you must be able to
> argue about interrupt latency.

> **Interview:** "How do you share data between an ISR and main?", "What is
> priority inversion and how does the priority ceiling protocol prevent it?",
> "What does a Waker do?", "How would you write a minimal executor?", and the
> lost-wakeup race with `wfi`.
