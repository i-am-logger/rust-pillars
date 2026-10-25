# Allocation outside the hot core: tips that a measurement confirms

Load this when an adapter, bin or setup path allocates in a loop, holds
long-lived strings or collections, picks a hash map's hasher, shares state
behind a lock, considers a global allocator, or derives `Copy` on a big type.

Evidence labels: **measured here** (this skill's own checks: rustc 1.94.1,
aarch64-apple-darwin; the blocks below run under `cargo xtask snippets`),
**by-reading** (the cited document says it), **inferred** (predicted, never
measured).

## Where these apply

- **The domain and the core allocate nothing per event**, and the core has
  `external = []`. Their answer is caller-lent storage, sized once
  ([zero-copy.md](zero-copy.md)), and the `alloc.rs` dhat test is the gate.
  Nothing below loosens that.
- These tips are for adapters, bins, tools, and the core's construction (setup
  that runs before the first event). Each one is a hypothesis about cost, so
  each ends the same way: **measure it (the rust-measure skill)**, with an A/B
  arm and a counter, before and after.
- CONFORMS to BeautifulRust's "usability first, optimize after measuring"
  here: write the clear version, measure, then apply the tip the profile
  names. The one departure is SKILL.md's, for the domain and core only, which
  are designed for speed from the start.

## Size the allocation once

- Build a collection with `collect()` from an iterator, or `Vec::with_capacity`
  when the count is known, never `Vec::new()` plus a push loop. A growing `Vec`
  reallocates and copies its contents each time it doubles. Clippy
  `vec_init_then_push` and `same_item_push` flag the simple cases. Measure it
  (the rust-measure skill).
- `collect()` sizes from `size_hint`: exact for a `map` over a slice, a lower
  bound after a `filter`. After a `filter`, `with_capacity` by the upper bound
  trades memory for zero reallocations, a choice a measurement makes.

Measured here, with dhat counting heap blocks (one profiler per test binary,
hence `standalone_crate`):

```rust,standalone_crate
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn blocks(f: impl FnOnce() -> Vec<u32>) -> u64 {
    let before = dhat::HeapStats::get().total_blocks;
    let v = f();
    assert_eq!(v.len(), 1000);
    dhat::HeapStats::get().total_blocks - before
}

let _profiler = dhat::Profiler::builder().testing().build();
let pushed = blocks(|| {
    let mut v = Vec::new();
    for i in 0..1000 {
        v.push(i);
    }
    v
});
let reserved = blocks(|| {
    let mut v = Vec::with_capacity(1000);
    for i in 0..1000 {
        v.push(i);
    }
    v
});
let collected = blocks(|| (0..1000).collect());
assert_eq!((pushed, reserved, collected), (9, 1, 1));
```

- The push loop made 9 blocks (each growth is a new block); `with_capacity`
  and `collect` made 1. Counting blocks is the cheap measurement; whether the
  8 reallocations matter is a timing question for an A/B.

## Let the caller own the buffer in a hot API

- A function called in a loop takes `&mut Vec<u8>` (or `&mut String`, or a
  `&mut [u8]` it fills and returns a length for) and appends, instead of
  returning a fresh `Vec`. The caller clears and reuses one buffer, so the
  steady state allocates nothing. The Rust API Guidelines' C-CALLER-CONTROL
  says the same: the caller decides where data lives. `std::io::Read::read_to_end`
  has this shape. Measure it (the rust-measure skill).
- In the core this is already the rule (lent storage). In an adapter, it is
  how a reader thread hands bytes on without allocating per read.

```rust
/// Appends one record's encoding to `out`; the caller owns and reuses `out`.
fn encode_into(id: u32, name: &str, out: &mut Vec<u8>) {
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(name.as_bytes());
    out.push(b'\n');
}

let mut buf = Vec::with_capacity(64);
for (id, name) in [(1, "tty0"), (2, "tty1")] {
    buf.clear(); // keeps the capacity: no allocation after the first round
    encode_into(id, name, &mut buf);
    assert_eq!(buf.len(), 4 + name.len() + 1);
}
```

## Long-lived immutable data: drop the spare capacity

- Data built once and then only read (a config table, an interned name, a
  loaded font list) is stored as `Box<[T]>`, `Box<str>` or, when shared across
  threads or owners, `Arc<str>`/`Arc<[T]>`. A `Vec`/`String` carries a capacity
  word and usually spare capacity. `Box<str>` is two words, not three, and
  `Arc<str>` needs one allocation where `Arc<String>` needs two. Clippy
  `box_collection` (in `all`, so a gate) flags `Box<Vec<T>>`; `rc_buffer`
  flags `Rc<String>` and `Arc<Vec<T>>` but is restriction and not enabled. Measure it (the rust-measure skill).
- A long-lived collection grown without a reservation gets `shrink_to_fit`
  once building ends. `into_boxed_slice`/`into_boxed_str` shrink as they
  convert. Never `shrink_to_fit` a buffer that will grow again: the next push
  reallocates.

```rust
use std::sync::Arc;

let mut names = String::with_capacity(64);
names.push_str("tty0");
let boxed: Box<str> = names.into_boxed_str(); // drops the 60 spare bytes
let shared: Arc<str> = Arc::from(&*boxed); // one allocation: counts + bytes
assert_eq!((&*boxed, &*shared), ("tty0", "tty0"));
assert_eq!(size_of::<Box<str>>(), 2 * size_of::<usize>());
assert_eq!(size_of::<String>(), 3 * size_of::<usize>());
```

## Hashers: a fast one only for trusted keys

- `std`'s `HashMap` defaults to `RandomState` (SipHash-1-3, randomly seeded),
  which resists HashDoS: an attacker who controls keys cannot force collisions.
  Keep it for any key that comes from outside the program (a network peer, a
  file, terminal input). Measure it (the rust-measure skill).
- A faster non-cryptographic hasher (`foldhash`, `rustc-hash`'s `FxHash`) is
  for keys the program itself made (interned ids, indices). Choose it per map
  with the third type parameter, so the choice is visible where the keys are.
- The core uses no hash map at all. It is `no_std` with `external = []`, and a
  bounded array or a domain index is cheaper and has no hasher to choose.
  DEPARTS from BeautifulRust: it offers the faster hasher wherever keys are
  trusted, but in rust-code's core the map itself is the cost.
- A randomly seeded map iterates in a different order each run, so never let
  that order reach output or a test (sort it first).

```rust,ignore
// ignore: foldhash is not a dependency of this project.
type ById<V> = std::collections::HashMap<InternId, V, foldhash::fast::RandomState>;
```

## Batch instead of locking per item

- Take a lock once per batch, not once per item: build the batch in a local
  buffer, then lock, `extend`, unlock. Each acquire is an atomic
  read-modify-write on a contended cache line ([caches.md](caches.md)), and
  per-item locking serialises the threads on it. Better still is a hand-off
  with no lock (the core's batch, a generation-indexed slot:
  [gpu-shared-memory.md](gpu-shared-memory.md)). No lint finds per-item
  locking: it is review. Measure it (the rust-measure skill).

```rust
use std::sync::Mutex;

let shared: Mutex<Vec<u32>> = Mutex::new(Vec::new());
let batch: Vec<u32> = (0..256).filter(|n| n % 3 == 0).collect(); // no lock held
shared.lock().expect("lock poisoned").extend(&batch); // one acquire, not 86
assert_eq!(shared.lock().expect("lock poisoned").len(), 86);
```

## A global allocator: bins only, and only if measured

- `#[global_allocator]` is set at most once per binary, in the bin crate,
  never in a library. Two in one link graph are a compile error, and a library
  that sets one takes the choice away from every binary that uses it. Measure
  it (the rust-measure skill): an A/B arm of the whole program under its real
  workload, because an allocator's effect depends on the allocation pattern.
- Setting one needs no `unsafe` in your crate: the `unsafe impl GlobalAlloc` is
  in the allocator crate. That crate is still an `unsafe` dependency (and
  mimalloc builds C code), so name it in review.
- DEPARTS from BeautifulRust: it says applications may set a global
  allocator, and rust-code requires the winning A/B first. A program that allocates nothing per
  event, as the core is built to, may gain nothing from a faster allocator.
- The dhat allocator in `crates/<name>-core/tests/alloc.rs` is its own test
  binary, so it does not clash with a bin's allocator.

```rust,ignore
// ignore: mimalloc is not a dependency of this project. In the bin's main.rs.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
```

## No `Copy` on large types

- Derive `Copy` only on small plain values: a `Copy` type is copied by
  `memcpy` at every move, silently. The rule itself is the rust-idioms skill's;
  the architecture's addition is the size check. Pin the size of every `Copy`
  record with `const _: () = assert!(size_of::<T>() <= N);`, so growing it fails
  the build rather than slowing every call. Measure it (the rust-measure skill).
- Clippy flags the symptoms. `large_enum_variant` (in `all`, so a gate)
  catches an enum one variant inflates. Not enabled in the workspace, so
  review: pedantic `large_types_passed_by_value` (big arguments),
  `large_stack_arrays` (big locals) and `trivially_copy_pass_by_ref` (the
  reverse mistake, a tiny `Copy` type passed by reference).

```rust
/// A plain 8-byte value: `Copy` is cheap and the build keeps it that way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    start: u32,
    len: u32,
}
const _: () = assert!(size_of::<Span>() <= 16);
let s = Span { start: 3, len: 4 };
let t = s; // a copy, and `s` stays usable
assert_eq!(s, t);
```
