# The alloc Crate & Global Allocators

Not every no_std program must avoid the heap. Linux-class ECUs, gateways,
tooling and many non-safety MCUs use one happily. The `alloc` crate gives you
`Box`, `Vec`, `String`, `Rc`, `Arc` and the `BTreeMap`/`BTreeSet`/`VecDeque`/
`BinaryHeap` collections. All it asks for is **an allocator**.

## Turning alloc on

```rust
#![no_std]
extern crate alloc;                       // link the alloc crate

use alloc::{boxed::Box, string::String, vec::Vec, collections::BTreeMap};
use alloc::{format, vec};                 // the macros live here too
```

In a library, that's all. In the final binary, exactly one crate must register
a global allocator:

```rust
#[global_allocator]
static HEAP: MyAllocator = MyAllocator::new();
```

If something uses alloc and nobody provides an allocator, the link fails
with a message about a missing `#[global_allocator]`.

## The GlobalAlloc contract

```rust
pub unsafe trait GlobalAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8;
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout);
    // provided, override if you can do better:
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 { ... }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 { ... }
}
```

The trait is `unsafe` to *implement*: the rest of the program trusts you to
follow these rules:

- return a pointer aligned to `layout.align()`, valid for `layout.size()`
  bytes, not overlapping any other live allocation; **or null** on failure;
- never unwind (panicking inside an allocator is a bad idea);
- `dealloc` receives exactly the pointer and layout from a previous `alloc`.

Note the receiver: `&self`, not `&mut self`. The allocator is a `static` used
from everywhere, so it needs interior mutability (atomics, a lock, or a
critical section), and it must be `Sync`.

`GlobalAlloc` and `Layout` live in **`core::alloc`**, so you can implement and
test an allocator without linking alloc at all.

## Layout: the allocator's vocabulary

A `Layout` is a (size, alignment) pair, where alignment is a power of two.

```rust
use core::alloc::Layout;

let l = Layout::new::<u64>();                 // size 8, align 8
let a = Layout::array::<u16>(10)?;            // size 20, align 2 (Err on overflow)
let (s, offset) = Layout::new::<u8>().extend(Layout::new::<u32>())?;
// s = size 8, align 4; the u32 starts at offset 4 (3 bytes of padding)
let s = s.pad_to_align();                     // round size up to a multiple of align
```

The core arithmetic of every allocator is rounding an address up to an
alignment, which for a power-of-two `align` is a bit trick:

```rust
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)         // overflow-check this in real code!
}
```

## Allocator designs

| Design | alloc | free | Fragmentation | Real-time | Notes |
|---|---|---|---|---|---|
| **Bump / arena** | O(1) | no-op (or reset all) | none | yes | perfect for "allocate at init" |
| **Fixed-size pool** | O(1) | O(1) | none | yes | one block size per pool |
| **Linked list (first fit)** | O(n) | O(n) | yes | no | `embedded-alloc` `LlffHeap` |
| **TLSF** | O(1) | O(1) | low | yes | `embedded-alloc` `TlsfHeap`; bounded time |
| **Buddy** | O(log n) | O(log n) | internal | mostly | power-of-two blocks |

A popular firmware compromise is **allocate during initialisation, never
afterwards**: build your `Vec`s and `Box`es at boot from configuration, then
freeze. You get flexible startup code and deterministic runtime behaviour. A
bump allocator plus a "locked" flag enforces it.

The heap itself is just a static region:

```rust
use core::mem::MaybeUninit;
static mut HEAP_MEM: [MaybeUninit<u8>; 32 * 1024] = [MaybeUninit::uninit(); 32 * 1024];
// with embedded-alloc:  unsafe { HEAP.init((&raw mut HEAP_MEM) as usize, 32 * 1024) }
```

## Out of memory

When `alloc` returns null, `Vec`/`Box` call `alloc::alloc::handle_alloc_error`.
In no_std its default behaviour is to **panic** (a custom
`#[alloc_error_handler]` is still unstable). So a heap exhaustion becomes a panic,
and your panic handler decides what happens.

To handle exhaustion as a value instead, use the fallible APIs:

```rust
let mut v: Vec<u8> = Vec::new();
v.try_reserve(4096).map_err(|_| Error::OutOfMemory)?;  // stable
```

(`Box::try_new` and the `allocator_api` for per-collection allocators are still
unstable as of this writing.)

## Arenas: allocation without a global allocator

You don't need `#[global_allocator]` to allocate. An **arena** hands out
references into a buffer you own, with lifetimes tied to the buffer:

```rust
let mut storage = [0u8; 1024];
let arena = Arena::new(&mut storage);
let a: &mut u32 = arena.alloc(5).unwrap();
let b: &mut [u8] = arena.alloc_slice_copy(b"hello").unwrap();
// all of it is freed at once when `storage` goes out of scope
```

The borrow checker guarantees nothing outlives the arena. Inside, the
implementation is a bump allocator over raw pointers. It's a perfect first
encounter with writing a safe API over `unsafe` internals, and you'll build
one.

> **Toyota lens:** for ASIL C/D software, ISO 26262-6 recommends avoiding
> dynamic objects; when they're unavoidable (e.g. on an Adaptive AUTOSAR
> platform running POSIX), the usual rules are "allocate at startup only",
> memory budgets per component, and allocators with bounded execution time
> (TLSF, pools). In interviews, show you know both sides: alloc is *available*
> in no_std, and *whether* to use it is a system-level safety decision.

> **Interview:** "Implement a bump allocator" is a classic. Know the alignment
> math, why `alloc` takes `&self`, what returning null means, and why dealloc
> is a no-op (or a counter that resets the bump pointer when it hits zero).
