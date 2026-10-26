# Mutation testing, fuzzing, Miri, sanitizers, loom, Kani

Load this when judging a suite's strength, fuzzing a parser, testing a
quarantine crate, testing a cross-thread hand-off, or proving arithmetic total.

Status in rust-template (commit `67e068d`): `test:mutants-diff` and
`qa:mutants` are **shipped**; `cargo-mutants` and `cargo-llvm-cov` are in
`devenv.nix` `packages`; every other task below is **planned**. Checked here: cargo-mutants flags against its
27.0.0 `--help`; the loom example under clippy `-D warnings` and loom 0.7.2
(green as written, red under each section-4 perturbation); the fuzz-target
body against the template's core. Miri, sanitizer and Kani snippets: not run.

## 1. Mutation testing is the strength gate

`cargo-mutants` rewrites one function body or operator at a time (return a
default value, flip `==`/`!=`, replace `+` with `-`, delete a `!` or a match
arm), builds, and runs the tests. A mutant the tests still pass on is
**missed**: a behaviour no test checks.

Shipped tasks (`devenv.nix`, abridged; `$crates` is
`cargo xtask crates --role domain,core`, i.e. `-p` per crate from
`layers.toml`):
```sh
# test:mutants-diff — in `devenv test`: lines changed since the merge base
git diff "$base" -- crates > target/mutants.diff   # no merge base = exit 1
PROPTEST_CASES=256 PROPTEST_RNG_SEED=1 cargo mutants $crates --in-diff target/mutants.diff

# qa:mutants — by hand or on a schedule: every mutant of domain and core
PROPTEST_CASES=256 PROPTEST_RNG_SEED=1 cargo mutants $crates
```
`.cargo/mutants.toml` sets `test_workspace = true` (a domain mutant is often
caught only by the core's properties; testing the domain alone reports it
missed) and `test_tool = "nextest"`. Results land in `mutants.out/`:
`caught.txt`, `missed.txt`, `timeout.txt`, `unviable.txt`. Keep
`mutants.out/` out of git (`.gitignore`): it is a run's output, not a
fixture.

Template evidence (commit `44736e5` message): the first run found 10
survivors, among them a byte-table test that compared the table with the
definition it was compiled from; after the fixes, 0 survive.

Rules:
- Mutate by role (domain, core), test with the whole workspace, and pin the
  proptest seed and case count exactly as the unit gate does. Why: an
  unpinned seed makes the missed count change between runs of one commit.
  Pass the unit gate's `--all-features` too: the shipped tasks do not, so a
  mutant behind `alloc` would go untested (none exists at `403c836`).
- Changed lines: zero missed (the shipped gate). A tree that starts with
  survivors ratchets the missed count like `budgets.toml`: exact, only goes
  down, raised only in a commit of its own that says why.
- A gate that cannot see the diff has checked nothing: no merge base is an
  error, not a pass (CI checks out with `fetch-depth: 0` for it).
- `unviable` mutants (do not compile) are not caught and not missed: they
  prove nothing. Same rule as a non-compiling planted violation.
- `timeout` counts as caught only if the timeout is a real hang the mutant
  caused (an infinite loop); otherwise raise `--timeout` and rerun.
- Keep the default `--baseline run`: the unmutated tree must pass first,
  which is the tool's own positive control.
- Exclusions (`--exclude`, `--exclude-re`, `#[mutants::skip]`) carry a reason
  in a comment and are counted like `#[ignore]`. Typical legitimate ones:
  `Debug` impls, error `Display` text.
- A missed mutant is fixed by a test that asserts the behaviour, never by
  excluding the function.

### Why not line coverage
Line coverage counts lines executed, not outcomes checked: `let _ = f(x);`
covers every line of `f` and asserts nothing, so a line percentage can stay
high while nothing is checked. The template's CI `coverage` job uploads `cargo llvm-cov`
output with `fail_ci_if_error: false` (shipped): informational, for finding
untested regions to point mutants at. DEPARTS from the common
coverage-percentage threshold: it rewards executing code, not checking it.

## 2. Fuzzing
`qa:fuzz` (planned): `cargo-fuzz` (libFuzzer, nightly) on every parser entry
point, i.e. every `pub fn` that takes bytes from outside.

- `fuzz/` is its own Cargo workspace (it needs nightly and libFuzzer), role
  `tool` in spirit: never a dependency of shipped code. Because it is not a
  member, it inherits no `[workspace.lints]` and `cargo xtask arch`/`layers`
  do not see it, so copy the lints into its manifest, `unsafe_code =
  "forbid"` included (pillar 6 has one exception, the declared quarantine
  crate). Unverified here: whether `fuzz_target!`'s generated entry point
  trips that lint; if it does, that is a decision for the register, not a
  silent `deny`.
- Target a sans-IO core through its event API, with the split chosen by the
  fuzzer, and assert what must always hold. Here: never panics, and every
  line is either delivered or counted as refused (conservation against a
  plain count, not against the machine):
  ```rust,ignore
  #![no_main]
  use libfuzzer_sys::fuzz_target;
  use my_project_core::{Effect, Elapsed, Event, Machine, Wire};

  /// Lines delivered since the last drain; forgets them, as a host would.
  fn drain(m: &mut Machine<64>) -> usize {
      let n = m.effects().iter().filter(|e| matches!(e, Effect::Line(_))).count();
      m.clear_effects();
      n
  }

  fuzz_target!(|input: (Vec<u8>, u8)| {
      let (bytes, step) = input;
      let step = usize::from(step.max(1));
      let mut m: Box<Machine<64>> = Box::new(Machine::new(Elapsed::threshold_ms(1)));
      let mut delivered = 0_usize;
      for chunk in bytes.chunks(step) {
          m.handle(Event::Bytes(Wire::of(chunk)));
          delivered += drain(&mut m);
      }
      m.handle(Event::End);
      delivered += drain(&mut m);

      let newlines = bytes.iter().filter(|&&b| b == b'\n').count();
      let tail = bytes.rsplit(|&b| b == b'\n').next().is_some_and(|t| !t.is_empty());
      let expected = u64::try_from(newlines + usize::from(tail)).expect("fits");
      let refused = m.refused();
      assert_eq!(refused.after_end.get(), 0, "no event followed End");
      assert_eq!(
          u64::try_from(delivered).expect("fits") + refused.effects.get(),
          expected,
          "a line was neither delivered nor counted"
      );
  });
  ```
  A chunk of up to 255 bytes can hold more than 64 newlines, so the
  refusal path is exercised too. Stronger still: compare the delivered
  records against the property test's independent model
  ([properties-and-oracles.md](properties-and-oracles.md)). Not compiled
  here: it needs `libfuzzer_sys` and nightly, which the gate does not link.
- Seed the corpus from the golden recordings and the committed proptest
  regressions: real inputs reach deep states that random bytes do not.
- Every crash becomes a committed regression test (example test or a
  `.proptest-regressions` entry) in the same commit as the fix.
- Build fuzz targets with overflow checks and debug assertions on (check
  `cargo fuzz build --help` for the flag your version uses): a fuzzer that
  cannot see an overflow cannot report one.

## 3. Miri and sanitizers: quarantine crates only
`qa:miri`, `qa:sanitize` (planned).

Every crate except a declared `quarantine` crate (`layers.toml`) has
`unsafe_code = "forbid"`, so the undefined behaviour Miri and sanitizers look
for cannot originate there. Run them where `unsafe` lives:
```sh
# Q = the crate declared `quarantine` in layers.toml (the template declares none yet)
cargo +nightly miri test -p "$Q"
MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test -p "$Q"
RUSTFLAGS="-Zsanitizer=address" cargo +nightly test -Zbuild-std \
  --target aarch64-apple-darwin -p "$Q"
```
- The quarantine crate's tests exercise every safe wrapper with the inputs
  that reach each `unsafe` block's boundary (empty, maximum, misaligned
  where the wrapper accepts it).
- Miri cannot execute foreign calls: separate the pure-Rust `unsafe` (Miri)
  from the FFI calls (sanitizers on a real host).
- CONFORMING with BeautifulRust's "Miri for unsafe"; narrower in scope
  because rust-pillars forbids unsafe everywhere else.

## 4. loom for cross-thread hand-offs
`test:loom` (planned). The core is single-threaded and sans-IO; threads
appear in adapters (a reader thread handing bytes to the core thread, a
renderer taking frames). Every such hand-off — an SPSC ring, a
generation counter, frames in flight — gets a loom model. Ordering bugs in
atomics are safe-code bugs: `forbid(unsafe_code)` does not prevent them.

In the adapter's `src/` (`my_project_io` below is a hypothetical adapter
crate; `loom = "0.7"` under `[target.'cfg(loom)'.dependencies]`):
```rust
#[cfg(loom)]
use loom::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(loom))]
use std::sync::atomic::{AtomicU64, Ordering};

/// One value handed from a producer thread to a consumer: the producer
/// writes the value, then publishes its generation, so a consumer that sees
/// the generation sees the value.
#[derive(Debug)]
pub struct Slot {
    value: AtomicU64,
    generation: AtomicU64,
}

impl Slot {
    /// Nothing published yet: generation 0.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            value: AtomicU64::new(0),
            generation: AtomicU64::new(0),
        }
    }

    /// Producer only: write the value, then publish its generation.
    pub fn publish(&self, generation: u64, value: u64) {
        self.value.store(value, Ordering::Relaxed);
        self.generation.store(generation, Ordering::Release);
    }

    /// The newest generation this thread can see, and the value with it.
    #[must_use]
    pub fn read(&self) -> (u64, u64) {
        let generation = self.generation.load(Ordering::Acquire);
        (generation, self.value.load(Ordering::Relaxed))
    }
}
```
In `crates/<name>-core/tests/loom.rs` (not compiled here: it needs loom and the adapter crate):
```rust,ignore
//! Cross-thread hand-offs under every interleaving loom can build.
#![cfg(loom)]

use loom::sync::Arc;
use my_project_io::Slot;

#[test]
fn a_reader_that_sees_the_generation_sees_its_value() {
    loom::model(|| {
        let slot = Arc::new(Slot::empty());
        let producer = {
            let slot = Arc::clone(&slot);
            loom::thread::spawn(move || slot.publish(1, 42))
        };
        let (generation, value) = slot.read();
        if generation == 1 {
            assert_eq!(value, 42, "the generation was visible before its value");
        }
        producer.join().unwrap();
    });
}
```
(Adapter code; raw `u64` is fine outside domain and core. In a domain/core
crate the generation would be a newtype.)

- The perturbation that must go red (R2 in
  [refuter-checklist.md](refuter-checklist.md)): weaken the `Release` store
  or the `Acquire` load to `Relaxed`; loom must then find an interleaving
  where the reader sees generation 1 with value 0. A model that stays green
  under that perturbation tests nothing. (Two loads of one atomic on one
  thread never go backwards whatever the ordering, so "the generation never
  decreases" is not such a test.)
- `#![cfg(loom)]` on the test file keeps it out of ordinary builds, where
  its imports would not resolve.
- Declare the cfg so `unexpected_cfgs` does not fire: in
  `[workspace.lints.rust]`, `unexpected_cfgs = { level = "deny", check-cfg = ['cfg(loom)', 'cfg(kani)'] }`
  (not yet in the template; add it with the first loom or Kani test).
- Run: `RUSTFLAGS="--cfg loom" cargo test --release -p <adapter> --test loom`.
- Keep the model small (2-3 threads, few operations): loom explores every
  interleaving, so cost is exponential.

## 5. Kani for "this arithmetic is total"
`qa:kani` (planned). Where the type system claims an operation cannot
overflow, wrap or panic (a saturating constructor, a narrowing proof type, a
bound that "can only shrink"), prove it for **every** input with Kani rather
than sampling it:
```rust
// in crates/<name>-domain/src/quantity.rs
#[cfg(kani)]
mod proofs {
    use super::*;

    #[kani::proof]
    fn saturating_advance_never_panics_and_never_moves_back() {
        let pos = BytePos(kani::any());
        let by = ReadLen(kani::any());
        assert!(pos.saturating_advance(by) >= pos);
    }

    #[kani::proof]
    fn checked_grow_is_exactly_one_more_or_none() {
        let len = ByteLen(kani::any());
        match len.checked_grow() {
            Some(next) => assert_eq!(next.get(), len.get() + 1),
            None => assert_eq!(len.get(), u32::MAX),
        }
    }
}
```
Run: `cargo kani -p my-project-domain`. Kani checks arithmetic overflow and
panics by default, so a proof that passes means no input reaches either.

- Proofs live in the module (they may name `pub(crate)` fields) behind
  `#[cfg(kani)]`, with the cfg declared as above.
- Bounded loops need `#[kani::unwind(N)]`; an unbounded proof is not a
  proof. Prefer proving the loop body's step function.
- Use Kani for claims over integers; use the exhaustive sweep for finite
  domains small enough to enumerate (a `u8`, a pair), which is cheaper and
  runs in `test:unit`.

## 6. Which tool for which claim
| claim | tool | tier |
|---|---|---|
| tests check behaviour, not just run it | cargo-mutants | `test:mutants-diff`, `qa:mutants` |
| parser never panics or loses input | cargo-fuzz | `qa:fuzz` |
| `unsafe` in the quarantine crate is sound | Miri, ASan | `qa:miri`, `qa:sanitize` |
| a cross-thread hand-off is correct under every interleaving | loom | `test:loom` |
| integer arithmetic is total for every input | Kani | `qa:kani` |
| finite-domain function matches its spec | exhaustive test | `test:unit` |
