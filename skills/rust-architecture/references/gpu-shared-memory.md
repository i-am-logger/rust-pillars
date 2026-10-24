# Shared CPU/GPU memory: explicit hand-off, frames in flight

Load this when the CPU produces data that a GPU, another process, or another
thread reads in place: shared Metal or Vulkan buffers, shm, mmap rings.

Evidence labels:
- **by-reading**: the documentation or code says it.
- **measured here**: this skill's own checks on rustc 1.94.1, in a
  `git archive` copy of rust-template.

## The rule

- **Zero-copy across a device boundary needs an explicit hand-off.** A consumer
  that reads memory the producer is still writing is a data race, even when no
  Rust `&mut` aliases it. The GPU is not in Rust's borrow checker.
- **Use generations, or frames in flight. The producer never writes what the
  consumer holds.**
  - Each slot has one holder at a time: the producer or the consumer.
  - The producer writes only slots it holds, then submits them.
  - The consumer's completion hands a slot back.
- **Frame sets = frames in flight + 1.** With N slots, the producer always has
  one to write while N−1 are queued or being read (by-reading).
- **When no slot is free, the producer skips the frame and counts it**
  (refuse-and-count, [sans-io.md](sans-io.md)). It never waits inside the core
  and never overwrites a slot in flight.
- **The completion is an event.** A fence, a completion handler or a semaphore
  is read by the adapter, which tells the core "slot k completed". The core
  never polls a fence or registers a callback.
- **Shared records are zerocopy POD** (`repr(C)`, `FromBytes`/`IntoBytes`/
  `Immutable`/`KnownLayout`, size `const`-asserted). They are laid out by the
  one pitch function that every allocator uses ([caches.md](caches.md)).
- **GPU retains are RAII guards.** A retain passed to `mem::forget` is never
  released. The template bans `mem::forget` through clippy
  `disallowed_methods`, and the fix is a guard that releases in `Drop`
  ([zero-copy.md](zero-copy.md#own-dont-leak)).

## The slot hand-off as a pure state machine (core role)

This compiled with clippy `-D warnings` in a core-role crate, and its test
passed (measured here):

```rust
/// Which of the `N` frame slots. Private field: only the ring mints these.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Slot(usize);

/// Who holds a slot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Holder {
    /// Free: the producer may write it.
    Producer,
    /// Submitted: the consumer may be reading it; the producer must not write.
    Consumer,
}

/// `N` slots handed between a producer (CPU) and a consumer (GPU). The
/// consumer's completion arrives as an event; nothing here waits or calls back.
#[derive(Debug)]
pub struct Frames<const N: usize> {
    holder: [Holder; N],
    next: usize,
}

impl<const N: usize> Frames<N> {
    const AT_LEAST_TWO: () = assert!(N >= 2, "frames in flight + 1 needs two slots");

    /// Every slot free.
    #[must_use]
    pub const fn new() -> Self {
        let () = Self::AT_LEAST_TWO;
        Self {
            holder: [Holder::Producer; N],
            next: 0,
        }
    }

    /// The next slot the producer may write, or `None` when it is still with
    /// the consumer: the producer skips the frame (and counts it).
    #[must_use]
    pub fn acquire(&self) -> Option<Slot> {
        match self.holder.get(self.next) {
            Some(Holder::Producer) => Some(Slot(self.next)),
            _ => None,
        }
    }

    /// The producer finished writing `slot` and submitted it.
    pub fn submit(&mut self, slot: Slot) {
        if let Some(h) = self.holder.get_mut(slot.0) {
            *h = Holder::Consumer;
        }
        self.next = (slot.0 + 1) % N;
    }

    /// The consumer's completion event for `slot`.
    pub fn completed(&mut self, slot: Slot) {
        if let Some(h) = self.holder.get_mut(slot.0) {
            *h = Holder::Producer;
        }
    }
}

impl<const N: usize> Default for Frames<N> {
    fn default() -> Self {
        Self::new()
    }
}
```

- The test submits all three slots of a `Frames<3>`, asserts that `acquire()`
  is `None`, then calls `completed(a)` for the first slot and asserts that
  `acquire()` is exactly `Some(a)`. `cargo xtask coverage` requires such a test
  for every pub fn here.
- This sketch trusts its caller: a stale or repeated `submit` is not refused.
  A real port refuses and counts it.
- In a real port, use a sealed typestate `Slot<Writing>` → `Slot<Submitted>` so
  that "write after submit" does not compile (the `rust-domain` skill). Also
  add a property test: under any interleaving of `acquire`, `submit` and
  `completed`, the producer never holds a slot the consumer holds.
- Test the cross-thread adapter around this machine with loom (`test:loom`,
  planned).

## Copy-per-frame or ring: an open question

Nothing in this section is measured here: every option is a candidate.

Why a design copies at all: a GPU reading the live grid races the producer
that is still writing it, and pausing the producer to avoid the race stalls
everything upstream of it.

Options to evaluate against the two invariants ([caches.md](caches.md)) and
latency:

| Option | Write amplification | Hand-off | Open risk |
|---|---|---|---|
| (a) Copy the visible window into slot k per frame | window bytes per *frame*, not per byte ingested; bounded | slot k is the producer's until submitted | the per-frame copy grows with the window |
| (b) Ring of N screens with generations; the producer writes into GPU-visible slot memory | none extra | a generation per slot; the producer moves to the next free slot | scroll and rotation across slots; the producer stalls or skips when every slot is in flight |
| (c) Dirty-row deltas into double-buffered slots | dirty bytes per frame | as (a) | correctness of the delta log; needs a CPU⇔GPU pixel oracle |

Decide with a decision-register entry (D-n: claim, Evidence, Cost,
"Reverses if"), using the `rust-research` skill, and with an A/B (`rust-measure`).
The verification must include a CPU⇔GPU pixel oracle that *panics* without a
device, because a comparison that skips itself reads as a pass (the `rust-qa`
skill).

**Status:** open as of 2026-10-24. Until a measured comparison exists, treat
every option above as a candidate, not a recommendation.
