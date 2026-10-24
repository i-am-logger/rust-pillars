# Cache-aware layout: geometry, pitch, hot/cold, tables, prefetch, alignment

Load this when you lay out a grid or table, split a struct, add a field to a
hot struct, size a lookup table, consider a prefetch, share state across
threads, or explain a timing change that "should not" have happened.

Evidence labels:
- **by-reading**: the documentation or code says it.
- **measured here**: this skill's own checks on rustc 1.94.1,
  aarch64-apple-darwin, in a `git archive` copy of rust-template.
- **unmeasured**: a rule with its reason, not yet measured here. Measure it on
  your own workload before you quote a number for it.

## The two invariants

Every change to a hot path in domain or core is judged by two quantities:

1. **Write amplification**: bytes written per byte ingested.
   - A copy of data that has not changed (moving rows on scroll instead of
     rotating an index) is pure write amplification.
2. **Working set ≤ cache**: the bytes the hot loop touches must fit the level
   it is meant to run from.

- Both need "a measurement, not an argument". A design review states the
  expected value of each, and a measurement checks it. The method is in the
  `rust-measure` skill.
- Review: a deterministic CI counter (bytes written per byte) is planned, not
  in the template yet.

## Probe geometry in an adapter, use it as a pure value

- **Probe in an adapter**, never in domain or core. The sources, per platform:
  - macOS: `sysctl` keys `hw.perflevel0.l1dcachesize`, the matching L2 key, and
    `hw.cachelinesize`.
  - x86: CPUID leaf 4 (Intel) or leaf 0x8000001D (AMD).
  - Linux: sysfs `/sys/devices/system/cpu/cpu0/cache/index*/`, read under an
    *injectable root* so a test can point it at a fake tree.
- Adapters are under `forbid(unsafe_code)` too:
  - The sysfs reads are plain safe `std::fs`.
  - `sysctl` and CPUID need a safe binding crate or the quarantine
    ([no-unsafe.md](no-unsafe.md)). Check that a binding is safe before you
    claim it.
- **The probe produces a pure `CacheGeometry`, which has a valid `UNKNOWN`.** A
  failed or implausible probe falls back to `UNKNOWN`, and layout stays correct
  with it, only less tuned.
- **Only the line size drives layout**: the pitch and the
  alignment come from it. Cross-thread padding is fixed at compile time to the
  largest line you target (128, below). Cache *sizes* go into the working-set
  budgets you write down, not into an automatic layout.
- `UNKNOWN` carries a real line size (64 is a safe default) and never 0:
  `row_pitch` below divides by it.
- **`Pitch` is not `Cols`.** A padded distance between rows is its own newtype,
  so a width cannot be used as a pitch, or the other way round.

## Row pitch: whole lines, an odd number, clear of L1 set wrap

Why: in a set-associative L1, addresses one way size apart map to the same
set. Rows whose pitch is a multiple of a way size, or within a line of one,
therefore compete for the same few sets when a loop walks a column, and evict
each other. A pad that overshoots wastes working set instead (unmeasured).

The rule: the pitch is an odd number of lines, stepped clear of the L1 set
wrap for way sizes from 4 to 32 KB. The threshold below is this skill's
formalisation of that reasoning, not a measured optimum, so measure it on your
access pattern. A pitch of at least one way must not land within one line of a
multiple of that way.

This code passed clippy `-D warnings` in a domain-role crate, and its
exhaustive test passed (measured here). It does not show the newtypes or
`CacheGeometry::from_host`; `cargo xtask coverage` needs a test calling each
new pub fn:

```rust
# pub struct LineBytes(pub(crate) u32);
# pub struct CacheGeometry { pub(crate) line: LineBytes }
# pub struct RowBytes(pub(crate) u32);
# pub struct Pitch(pub(crate) u32);
/// L1 way sizes the pitch must step clear of: 4 KiB to 32 KiB.
const WAYS: [u32; 4] = [4096, 8192, 16384, 32768];

/// Whether a pitch of at least one way lands within one line of a multiple
/// of that way size.
const fn wraps(pitch: u32, line: u32) -> bool {
    let mut i = 0;
    while i < WAYS.len() {
        let way = WAYS[i];
        let r = pitch % way;
        if pitch >= way && (r <= line || r >= way - line) {
            return true;
        }
        i += 1;
    }
    false
}

/// The pitch for a row: whole lines, an odd number of them, stepped until it
/// is clear of every way size. Saturates (a 4 GiB row is not a layout).
#[must_use]
pub const fn row_pitch(row: RowBytes, geo: CacheGeometry) -> Pitch {
    let line = geo.line.0;
    let mut lines = row.0.div_ceil(line);
    if lines % 2 == 0 {
        lines += 1;
    }
    while wraps(lines.saturating_mul(line), line) && lines < u32::MAX - 2 {
        lines += 2;
    }
    Pitch(lines.saturating_mul(line))
}
# assert_eq!(row_pitch(RowBytes(100), CacheGeometry { line: LineBytes(64) }).0, 192);
```

- `RowBytes`, `Pitch` and `LineBytes` are domain newtypes with `pub(crate)`
  fields.
- Budget delta: domain `seam` +3 (`RowBytes::from_host`,
  `CacheGeometry::from_host` and `Pitch::get`). Raise the budget in a commit of
  its own.
- `CacheGeometry::from_host(line: u32) -> Option<Self>` accepts only a power of
  two from 16 to 512.
- The test sweeps every row from 1 to 65,536 bytes at line sizes of 64 and 128.
  It asserts that each pitch is whole lines, odd, not short, and not wrapping,
  and it ends with `assert_eq!(checked, 2 * 65_536)`.
- **One layout function for every allocator.** GPU, mmap and heap allocations
  all take their pitch from this one function, so no two of them disagree.
- **Align the base, not only the pitch.** A pitch of whole lines is wasted when
  row 0 starts mid-line.
  - A plain `Vec<T>` is aligned only to `T`, not to a cache line.
  - Without `unsafe`, alignment comes from the element type. Allocate rows as
    `#[repr(C, align(128))]` chunks: 128 covers both 64- and 128-byte lines.
  - Review: a row-alignment test (base address % line size == 0) is planned,
    not in the template yet.

## Hot/cold split, sizes pinned

- **Split by access frequency.** Fields touched per byte go in `HotState`, which is
  small and copied into locals per batch. Everything else goes in `ColdState`.
  [zero-copy.md](zero-copy.md#caller-lent-storage-unpacked-once-per-batch-core-role)
  has a compiled example.
- **Pin both sizes**, for example
  `const _: () = assert!(core::mem::size_of::<ColdState>() == 16);`. Growing either
  struct is then a reviewed edit with a reason in the commit. Keep a note of
  what each byte is for next to the assert.
- **A field in the wrong half costs on every byte.** A field the hot loop reads
  that sits in `ColdState` pulls a cold line into the loop (unmeasured here).
- **Use one induction variable.** A second counter advanced alongside the first
  is one more live register and one more store per step (unmeasured here).
- **Watch store merging.** Splitting one flags field into two narrower ones can
  stop LLVM merging the store; check the disassembly and measure (unmeasured
  here).
- **Intern wide attributes.** An index into an interned table in place of an
  inline multi-byte attribute shrinks every hot record (unmeasured here).

## Tables: built at compile time, budgeted, tested exhaustively

- **Build with `const fn` into a `static`.** The table lands in read-only data,
  with no initialiser at startup and one load per lookup. The template's
  `crates/<name>-domain/src/class.rs` does this:
  - `static CLASS: [ByteClass; 256] = build();` from a readable
    `const fn reference(b: u8)`;
  - the one `as` in it is counted by `cargo xtask budgets`;
  - an exhaustive test checks every input and asserts `checked == 256`.
- **For large sparse domains, use a two-stage trie**, such as Unicode
  properties: a block index plus deduplicated blocks.
  - A protocol automaton fits the same form: a `const fn` building a
    `static` transition table of one row per state and one column per input
    byte, in rodata with no initialiser.
  - An action may consume a variable width, so a multi-byte character can be
    one lookup, and a mode change can switch to another table.
- **Write each table's budget down and assert it.**

  ```rust
  # use my_project_domain::ByteClass;
  // The class table's budget: one byte per input, in read-only data.
  const _: () = assert!(core::mem::size_of::<[ByteClass; 256]>() <= 256);
  ```

  - A budget is a number in bytes per table, written down before the table is
    built, so a design that needs a far bigger table is refused or argued for,
    not slipped in.
- **If a table is too big for `const fn`, or is generated from external data,**
  generate it at build time and embed it once
  (`static T: &[u8] = include_bytes!(..)`), with a drift test. The
  `rust-measure` skill covers this. Review: `xtask gen` is planned, not in the
  template yet.

## Bounded interning

- Use power-of-two open addressing indexed by the *high* bits of a Fibonacci
  hash: the low bits of `k * C mod 2^w` depend only on the low bits of `k`,
  so keys that differ only in their high bits collide there.
- Inline the hit path. Put the miss path in a `#[cold] #[inline(never)]` fn.
- Sweep tombstones before evicting.
- Keep a write log so downstream consumers update incrementally.
- The table is bounded: when it is full it refuses and counts
  ([sans-io.md](sans-io.md)). It never grows.

## Prefetch only with a winning A/B

- An explicit prefetch is one more load, issued alongside the demand loads and
  competing with them for the same bandwidth; it is not free advice.
- Rule: no explicit prefetch without an A/B that shows a win. Prefetch
  intrinsics would need the quarantine in any case.

## Code layout is a variable

- Function placement alone can move a timing: identical code laid out at
  different addresses straddles fetch and cache-line boundaries differently.
  When hot code is near a change, build every arm with the same
  `-C llvm-args=-align-all-functions=6` against that noise (the
  `rust-measure` skill).
- Data that nothing reads still moves the code and data after it, so adding
  it can change a hot loop's timing.
- Rule: a timing delta smaller than the placement noise is no result. The
  method is in the `rust-measure` skill.

## False sharing

- **Pad cross-thread hot state to a line of its own.** Apple's lines are
  128 bytes (by-reading), so use `#[repr(align(128))]`. The example shows the
  layout only: the state two threads share is an atomic in the adapter that
  owns the hand-off, never a plain `Tally` written from both. This compiled in
  the domain with both asserts (measured here):

  ```rust
  # use my_project_domain::Tally;
  /// Shared between a producer and a consumer thread: on a line of its own.
  #[derive(Debug)]
  #[repr(C, align(128))]
  pub struct ProducerSide {
      written: Tally,
  }

  const _: () = assert!(core::mem::align_of::<ProducerSide>() == 128);
  const _: () = assert!(core::mem::size_of::<ProducerSide>() == 128);
  ```

- Cross-thread hand-offs belong in adapters. Test them with loom (`test:loom`,
  planned).

## Review checklist

- [ ] The expected write amplification and working set are stated and
      measured.
- [ ] Geometry comes from an adapter probe into a pure value with an
      `UNKNOWN`, and only the line size drives layout.
- [ ] The pitch comes from the one layout function, and the base is
      line-aligned.
- [ ] Hot and cold sizes are `const`-asserted, and any change to them is
      explained.
- [ ] Tables are `const fn`-built `static`s with a size assert and an exhaustive
      test that asserts its count.
- [ ] There is no prefetch without a winning A/B.
- [ ] Cross-thread hot fields are padded.
