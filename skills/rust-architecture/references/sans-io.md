# Sans-IO core: events in, effects out

Load this when you design or change a `core`-role crate, add an event or an
effect, add a buffer, or write or repair the chunk-invariance property.

Evidence labels: **measured** (a number someone measured), **by-reading**
(what the code or notes say, read but not run), **inferred** (a prediction
nobody has measured). "Measured here" means measured in this skill's own
compile checks (rustc 1.94.1, aarch64-apple-darwin, a `git archive` copy of
rust-template).

## The shape

The core is a pure machine. Its whole API is three moves and a set of counters:

| Move | Template (`crates/<name>-core/src/machine.rs`) |
|---|---|
| place input and step | `Machine::handle(&mut self, event: Event<'_>)` |
| drain effects | `Machine::effects(&self) -> &[Effect]`, then `clear_effects()` |
| read what was refused | `Machine::refused(&self) -> Refusals` (`bytes`, `effects`, `after_end`, `total()`) |

The vocabulary lives in the domain (`crates/<name>-domain/src/event.rs`):
`Event<'a> { Bytes(Wire<'a>), Tick(Millis), End }` and
`Effect { Line(LineRecord), Idle(Millis) }`. Both are closed enums whose
payloads are already narrowed domain types. No raw integers cross.

The host loop lives in the bin (`crates/<name>/src/main.rs`). It reads,
sends `Event::Tick(now())`, sends `Event::Bytes` or `Event::End`, drains, and
clears. Draining after every read bounds what one read can produce
(`const EFFECTS: usize = 1024`).

This is the sans-IO shape that quinn-proto and rustls use (by-reading). That
is evidence of practice, not a standard.

## Rules

1. **One entry point that never blocks, allocates or fails.**
   - `handle` returns `()`. What the core cannot keep, it refuses and counts.
   - WHY: a `Result` from the hot entry makes every host invent a policy for a
     state the core already knows how to handle. A count leaves the policy to
     the host and keeps the data.

2. **Effects are queued and polled. They are never called back.**
   - No `fn(..)`, `impl Fn*`, `dyn Fn*`, generic `F: Fn*`, or sink trait object
     (`&mut dyn Sink`, `impl Sink`, `S: Sink` on a method) appears in a core
     public signature.
   - WHY: a callback runs host code inside the core's step. That brings
     re-entrancy, a `&mut self` borrow held across foreign code, and the host's
     latency and errors into the core, and it makes the output something a test
     cannot hold as a value. A queue makes output plain data that a test compares.
   - Gate: `cargo xtask arch`. It flags `fn(..)`, and any `impl`, `dyn` or
     method generic in a core public signature bounded by a trait outside its
     data-trait list (`Iterator`, `IntoIterator`, `AsRef`, `Borrow`, `Into`,
     `Copy`, `Clone`, `Debug`, ...). Its positive control counts
     `pub fn sink(s: &mut dyn Sink)` as a callback.
   - A generic on a *type* (`struct History<S: Storage>`) is not flagged; a
     generic `pub fn` bound by the same trait is (measured here).

3. **Time, randomness and I/O enter as events.**
   - Time:
     - The host reads the clock and sends `Event::Tick(Millis::from_host(ms))`.
     - Intervals exist only as `Elapsed`, made by `Millis::since` or the
       configuration seam `Elapsed::threshold_ms`.
     - A clock that steps back reads as zero time (`saturating_sub`). It is
       never a huge interval.
   - Randomness:
     - A seed arrives as an event or a configuration value.
     - Any PRNG state is a plain field on the machine, so a run replays exactly.
   - I/O:
     - Bytes arrive borrowed (`Wire<'a>`).
     - Output leaves as effects.
     - The core never names a file, socket or process.
   - WHY: a core that reads the clock or the RNG gives different output for the
     same input, so it cannot be replayed, fuzzed or compared against a model.
   - Gates:
     - The crate-local `clippy.toml` `disallowed-types` bans
       `std::time::Instant`, `std::time::SystemTime` and `std::thread::Thread`.
     - `#![no_std]` (`cargo xtask arch`, `test:bare`) makes fs, net, threads and
       process unreachable.
     - `[roles.core] external = []` (`cargo xtask layers`) keeps `rand` and
       every other crate out.

4. **Every buffer is bounded. Overflow is refused and counted.**
   - Template: `Effects<N>` is a fixed array plus a private `Slots(usize)` length.
     - `push` returns `Queued::{Taken, Dropped}` (`#[must_use]`).
     - `Machine::emit` turns `Dropped` into `refused.effects.saturating_incr()`.
   - Never grow, never block, never drop silently.
   - WHY:
     - A growing buffer is unbounded memory and an allocation on the hot path.
     - A blocking push hands the core's progress to the host.
     - A silent drop is data loss that no test sees.
   - Gates:
     - `crates/<name>-core/tests/alloc.rs` (dhat) asserts zero new blocks
       and unchanged live bytes over 1000 rounds.
     - The unit tests `a_full_queue_drops_and_counts` and
       `events_after_the_end_are_refused_and_counted` pin the counters.

5. **Capacities are computed and asserted at compile time.**
   - Take a buffer size from the protocol's own maximum message size, not a
     guess.
   - Assert the relation in a `const`, so a change to either side fails the
     build:

     ```rust
     # use my_project_domain::LineRecord;
     // A record must fit the slot the host reads it from.
     const _: () = assert!(core::mem::size_of::<LineRecord>() == 16);
     ```

   - A const-generic capacity carries its own guard. The template does it in
     `effects.rs`:

     ```rust
     # struct Effects<const N: usize>;
     impl<const N: usize> Effects<N> {
         const NONZERO: () = assert!(N > 0, "an effect queue needs at least one slot");
         // `new` evaluates it: `let () = Self::NONZERO;`
     }
     # let () = Effects::<1>::NONZERO;
     ```

6. **State between events lives on the struct.**
   - A token cut by a read boundary stays in fields, like `Line<Building>` in
     the template.
   - WHY: this is what makes output independent of where the host's reads fell.
     A core that rescans or drops a partial token at the end of a read has a
     defect that only shows up under real I/O timing.

7. **The chunk-invariance property is mandatory.** Every core has
   `crates/<name>-core/tests/chunk_invariance.rs` with two properties (see the template's file):
   - **Split equality:** any split of the input into reads gives the same
     effects as one read.
   - **Model agreement:** one-byte reads give the same effects as an
     *independent model* written against `std` (`split`,
     `split_ascii_whitespace`), not against the machine.
   - WHY the model is required:
     - Split equality compares the implementation with itself, so a defect that
       every split shares passes it.
     - In the template's planted-violation run, a doubled position advance
       passed split equality and failed the model property (measured).
   - Generate **valid** input. Use an alphabet of meaningful atoms
     (`ALPHABET: &[u8] = b"ab \t\r\n"`), not `any::<u8>()`, which spends its
     cases on noise.
   - Write down every symbol you exclude, and why. The template excludes `0x0b`
     because `std` and the machine disagree on it. That difference has a test
     of its own in the domain.
   - Assert that nothing was refused inside the property
     (`assert_eq!((r.bytes.get(), r.effects.get()), (0, 0))`). A dropped effect
     would make the comparison vacuous.
   - Persist failures beside the file
     (`FileFailurePersistence::WithSource("regressions")`) and commit them.
   - Gates:
     - `cargo xtask arch` requires a `fn chunk_invariance_*` in the core's
       `tests/`. It checks only that the marker exists. Whether the model is
       independent is a review question.
     - `test:unit` runs the property under `PROPTEST_CASES=256 PROPTEST_RNG_SEED=1`.
   - Test the read sizes the host really uses, as well as random cuts: for
     example every size from 1 to 64, plus the host's buffer size.

8. **The core is not `async`.**
   - `async` is a way of scheduling I/O, and the core has none.
   - WHY: a core with no executor can be driven by any executor, by a test, or
     by a bare-metal loop. Async lives in adapters.

9. **Errors in the core are `Copy` enums with no payload.** See
   [no-std.md](no-std.md#errors); the rule and its DEPARTS are rust-idioms'.

## Adding an event or an effect

1. Add the variant to the domain enum, with a narrowed payload type and no raw
   integers (`cargo xtask budgets` counts primitives in the domain's public API).
2. Handle it in `Machine::handle`'s `match`. The match is exhaustive, so the
   compiler finds every site.
3. If it can be refused, add a `Tally` field to `Refusals`, include it in
   `total()`, and add a unit test that fills the bound and asserts the count.
4. Extend the independent model and the property's alphabet to cover it.
5. If the host must produce it (time, a seed), add the host-side read to the bin
   or an adapter. Never add it to the core.
6. Call every new pub fn from a test (`cargo xtask coverage`).

## Review checklist

- [ ] No callback or sink appears in a public signature (`cargo xtask arch`;
      and review any change to `DATA_TRAITS` in `xtask/src/scan.rs`).
- [ ] No clock, RNG, thread or I/O is used (clippy, `test:bare`,
      `cargo xtask layers`).
- [ ] Every buffer is a fixed array or caller-lent slice, its overflow has a
      counter, and a test fills it.
- [ ] Sizes are `const`-asserted against the buffers they must fit.
- [ ] A `chunk_invariance_*` test exists, with an independent model, valid
      generators, and an asserted absence of refusals.
- [ ] Zero allocations per event (`crates/<name>-core/tests/alloc.rs` still passes, and its input
      exercises the new path).
