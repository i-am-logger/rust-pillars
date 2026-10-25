# Test taxonomy and placement

Load this when deciding what kind of test to write and where it goes.

Labels: **shipped** = committed in the companion `rust-template`
(branch `feat/pillars`, commit `a076f93`); **planned** = in the rust-code
plan only.

## 1. Where a test lives

| what it verifies | where | example in the template |
|---|---|---|
| a private invariant of one module | `#[cfg(test)] mod tests` in that file | `crates/<name>-domain/src/class.rs` (exhaustive table), `src/quantity.rs` (saturation by name), `crates/<name>-core/src/machine.rs` (refusals counted) |
| a public claim about the crate | `crates/<c>/tests/<claim>.rs` | `crates/<name>-core/tests/chunk_invariance.rs` |
| a resource property of the hot path | its own test binary | `crates/<name>-core/tests/alloc.rs` (dhat allows one profiler per process) |
| what the compiler must refuse | trybuild `tests/ui/fail/*.rs` + `tests/ui/pass/*.rs` twins | `crates/<name>-domain/tests/typesystem.rs` |
| an example in the docs | doctest | runs in `test:unit` (`cargo test --doc --profile qa`) |
| a fact about the repo (docs, budgets, coverage rows) | `xtask` gate or a meta-test | `cargo xtask docs` |

Rules:
- One file per claim family, named after the claim (`chunk_invariance`,
  `alloc`, `typesystem`), not after the module under test. Why: modules get
  split and renamed; claims do not.
- Test names are sentences: `handling_events_allocates_nothing_and_nothing_grows`.
  The failure line then states what broke.
- Every assertion carries context: `assert_eq!(a, b, "byte {b:#04x}")`. Why:
  an exhaustive sweep that fails must say which input.
- Domain-level tests use the domain's constructors (`Wire::of`,
  `Millis::from_host`), never the raw fields, unless the test is in the same
  module and is about the representation.
- File by protocol level when the code is a protocol. A failure then points at a layer.
- No mocks. Time, randomness and I/O enter the core as events
  (`Event::Tick(Millis::from_host(..))`, `Event::Bytes(Wire::of(..))`), so a
  test constructs the values it wants. DEPARTS from BeautifulRust ("mockable
  I/O/time/randomness", "mocks behind test-util"): a sans-IO core has no
  dependency to mock, and a mock is a second implementation that no test
  checks against the real one. Adapters are tested against the real OS
  resource, on the platform that has it (section 3).

## 2. Kinds of test, and the bar each must clear

### Example tests
Concrete input, exact expected output. Use for the boundary cases a generator
reaches rarely: empty input, maximum length, the value one past it.
```rust
#[test]
fn a_line_length_refuses_rather_than_wraps() {
    assert_eq!(ByteLen(u32::MAX).checked_grow(), None);
    assert_eq!(ByteLen(7).checked_grow(), Some(ByteLen(8)));
}
```
(in-module, so it may name the `pub(crate)` field; from `src/quantity.rs`.)

### Exhaustive tests over finite domains
If the input space is small (a `u8`, two `u8`s = 65 536, an enum, 4^8 SIMD
blocks), test all of it and assert the count:
```rust,ignore
/// Whitespace as `core` defines it, written apart from the table's own
/// `reference` generator; vertical tab (0x0b) is added by name because
/// `is_ascii_whitespace` leaves it out and the table counts it as space.
fn independent(b: u8) -> ByteClass {
    if b == b'\n' {
        ByteClass::Newline
    } else if b.is_ascii_whitespace() || b == 0x0b {
        ByteClass::Space
    } else {
        ByteClass::Other
    }
}

#[test]
fn the_table_agrees_with_an_independent_definition_for_every_byte() {
    let mut checked = 0_u32;
    for b in u8::MIN..=u8::MAX {
        assert_eq!(ByteClass::of(b), independent(b), "byte {b:#04x}");
        checked += 1;
    }
    assert_eq!(checked, 256, "the sweep did not cover every byte");
}
```
(from `crates/<name>-domain/src/class.rs`, shipped, and compiled there; not
compiled here, since `ByteClass::of` is `pub(crate)`.)

Why the count: `for b in 0..u8::MAX` misses 255 and every per-item assertion
still passes. The count is written as a literal, not computed from the same
range, so the two can disagree.

Why `independent`, not the table's generator: until commit `c0f4705` this
test compared the table with `reference`, the `const fn` it is compiled
from, so a wrong `reference` passed. cargo-mutants found it among the first
run's 10 survivors (template commit message). An exhaustive sweep is still
code vs code unless the right-hand side was written separately.

Example: a SIMD backend checked against the portable specification over all
2^16 blocks of a two-symbol alphabet, ending with
`assert_eq!(checked, 65_536)`.

### Backend agreement tests
When two implementations exist (scalar/SIMD, table/`match`, CPU/GPU, two
storage shapes), the simplest is the specification and every other one is
compared to it, exhaustively when the domain is finite, by property when not.
The specification is never deleted "because the fast one is tested".

A backend that needs hardware (a GPU, an ISA extension) must not skip itself
when the hardware is missing; see
[gates-that-can-fail.md](gates-that-can-fail.md), section 3. A CPU/GPU pixel oracle panics without a device, because a
comparison that skips itself reads as a pass.

### Property tests
See [properties-and-oracles.md](properties-and-oracles.md). Mandatory for core:
`cargo xtask arch` fails a core crate with no `chunk_invariance_*` test in
`tests/` (shipped).

### Golden recordings (planned in the template)
Real inputs captured from real programs, replayed through the core, with the
output digested and pinned.

Layout:
```text
tests/recordings/
  README.md          # per recording: source program + version, capture command,
                     # date, host, what it exercises, how to regenerate
  SHA256SUMS         # sha256 of every input file
  <name>.bin         # the captured input
  <name>.digest      # pinned digest of the core's output for that input
```
Rules:
- A test re-hashes every input against `SHA256SUMS` before replaying. Why: a
  recording edited "to make it pass" is no longer a recording.
- A test asserts the number of recordings it replayed equals the number of
  lines in `SHA256SUMS` (exact floor).
- Each digest change is explained in the commit and in `docs/record.md`. An unexplained digest
  change is a regression until shown otherwise.
- Use the recordings again: they seed the fuzz corpus
  ([mutation-fuzz-miri-loom-kani.md](mutation-fuzz-miri-loom-kani.md), section 2)
  and the oracle comparison.
- Never derive a number from an edited recording (rust-measure's rule too).

### Spec inventory test
When a standard lists its own items (every control function, every error
code), a test reads that list from the pinned source and asserts each item is
either implemented or listed as refused with a reason. The count of both is
exact.

### Resource tests
`crates/<name>-core/tests/alloc.rs` (shipped): setup allocates before the
profiler starts; then 1000 rounds of events must leave `total_blocks` and
`curr_bytes` unchanged. dhat is a safe global allocator, so the test needs no
`unsafe impl GlobalAlloc` (pillar 6). Without the `alloc` feature the core
cannot allocate at all, so the test only bites with the feature on:
`test:unit` runs `--all-features` (shipped, commit `65a3876`). Before that
commit `test:unit` passed no feature flags and no member enables
`my-project-core/alloc` (by-reading the manifests), so code behind `alloc`
was never tested by the gate; see
[gates-that-can-fail.md](gates-that-can-fail.md), "Plant through the gate's
own command".

### compile_fail cases
trybuild twins with a pinned `.stderr`, never a bare `compile_fail` doctest
(rustdoc checks that it fails, not why). The rules, the four shipped cases
and how to read a stderr before pinning it are rust-domain's
(`references/compile-fail.md`); `cargo xtask docs` checks the twins.

## 3. Platform tests (planned)
A test that can only run on one platform is gated at compile time and listed:
```toml
# platform-tests.toml
[[test]]
name = "metal_backend_agrees_with_the_cpu"
crate = "my-project-gpu"
cfg = 'target_os = "macos"'
job = "Lint/UT (macos-latest)"
```
The gate (planned) fails when a `#[cfg(target_os = ..)]` test is missing from
the file, or the named job does not exist in `.github/workflows/ci.yml`.
`ci.yml` declares an `ubuntu-latest` + `macos-latest` matrix running
`devenv test`, so a macOS-only test has a place to run. CI triggers on
pushes and PRs to `master`; branch `feat/pillars` has no upstream yet, so
the matrix has not run these gates.

## 4. Ignored tests (shipped: `cargo xtask tests`, `ignores.toml`)
- Every `#[ignore]`d test is listed in `ignores.toml` as `file`, `test`,
  `reason`; the gate fails on an unlisted ignore, on a listed test that no
  longer exists, and on an empty reason. It scans every `#[test]` fn in each
  member's `src/` and `tests/` (not those inside `proptest!`).
- Also write the reason at the site, `#[ignore = "reason"]` (Rust Reference,
  "Testing attributes"), so a reader of the test sees it without the list.
- Why workspace-wide: a check that pins the list but scans only some files
  leaves an `#[ignore]` in the others invisible.

## 5. The test profile
`[profile.qa]` in `Cargo.toml` (shipped):
```toml
[profile.qa]
inherits = "release"
overflow-checks = true
debug-assertions = true
```
`test:unit` runs `cargo nextest run --workspace --all-features --cargo-profile qa`
and `cargo test --workspace --all-features --doc --profile qa`. Why: release speed keeps the
20 000-case and exhaustive runs affordable; the two checks restore what
release drops. Benches use plain `release`.

## 6. What not to write
- A test that asserts nothing (`let _ = parse(x);`) — it raises line coverage
  and is killed by no mutant.
- A test that compares the code with itself only (see
  [properties-and-oracles.md](properties-and-oracles.md)).
- A test with `if !available { return; }` (see never-skip).
- `assert!(results.len() > 40)` (see exact floors).
- A test that depends on another test's side effect (see `qa:shuffle`).
