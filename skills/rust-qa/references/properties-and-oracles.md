# Properties and oracles

Load this when writing a property test, a model, or a differential oracle.

A property is only as strong as what it compares against. Two rules carry the
whole file: generate **valid** input, and compare against something the code
under test did not write.

## 1. Generators: valid inputs from an alphabet of atoms

`any::<u8>()` input to a parser spends almost every case in the first
rejection branch. Build inputs from the grammar's atoms instead, including
deliberately truncated ones:

```rust
use proptest::prelude::*;

/// The bytes a line counter cares about, and nothing else.
const ALPHABET: &[u8] = b"ab \t\r\n";

fn input() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(prop::sample::select(ALPHABET), 0..512)
}
```
(from `crates/<name>-core/tests/chunk_invariance.rs`, shipped.)

For a protocol, atoms are whole tokens plus their prefixes:
```rust
# use proptest::prelude::*;
fn token() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        4 => prop::sample::select(&b"abc "[..]).prop_map(|b| vec![b]),
        2 => Just(b"\x1b[1m".to_vec()),   // a complete sequence
        1 => Just(b"\x1b[".to_vec()),     // a sequence cut short
        1 => (0_u16..=999).prop_map(|n| format!("\x1b[{n}m").into_bytes()),
    ]
}

fn stream() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(token(), 0..64).prop_map(|v| v.concat())
}
```
Rules:
- Weight the atoms so the interesting ones (separators, sequence starts) are
  common, and generate from an alphabet of atoms, not `any::<u8>()`.
- When the generator leaves something out, say why in a comment and test it
  by hand. The template leaves out `0x0b` because `std`'s
  `split_ascii_whitespace` does not treat it as a separator while the machine
  does. Since commit `c0f4705` the domain's exhaustive byte-table test names
  `0x0b` as space in its independent definition; before it, nothing pinned
  the difference although the generator's comment said it was "tested by
  hand". An omission is covered only when a test you can name covers it.
- Keep a separate, small `any::<u8>()` property for "never panics, every
  refusal counted". It checks robustness; the atom generator checks meaning.

## 2. The independent model

The model is a second, obviously-correct computation of the same answer,
written against something the implementation did not write: `std`, a table
from the standard, a vendored reference, a brute-force search.

```rust
/// The independent model: (start, len, words) per line, from `std`.
fn model(input: &[u8]) -> Vec<(u64, u32, u32)> {
    let mut out = Vec::new();
    let mut start = 0_u64;
    let pieces: Vec<&[u8]> = input.split(|&b| b == b'\n').collect();
    let last = pieces.len() - 1;
    for (i, line) in pieces.iter().enumerate() {
        if i == last && line.is_empty() {
            break;
        }
        let text = core::str::from_utf8(line).expect("the alphabet is ASCII");
        let words = text.split_ascii_whitespace().count();
        out.push((
            start,
            u32::try_from(line.len()).expect("short"),
            u32::try_from(words).expect("short"),
        ));
        start += u64::try_from(line.len()).expect("short") + 1;
    }
    out
}
```
Rules:
- The model shares **no code** with the implementation: no shared helper, no
  shared table, no shared constant beyond the input alphabet. Why: a defect in
  shared code passes both sides.
- The model is slow and plain. A clever model is a second implementation with
  its own bugs.
- A property of the form `f(split(x)) == f(x)` (code vs code) is necessary
  but not sufficient. Pair it with `f(x) == model(x)`.

### The worked lesson (measured: rust-pillars's own planted run on rust-template)
Plant: a doubled position advance (the position moves twice per byte in
`Machine::bytes`).
- `chunk_invariance_any_split_gives_the_same_effects` **passed**: the whole
  read and the split reads were both wrong in the same way.
- `chunk_invariance_one_byte_reads_agree_with_the_model` **failed**: `std`
  does not double-count.

A suite whose properties all compare the implementation against itself can
miss every planted defect that all its sides share.

### Assert the property was not vacuous
A property over effects is vacuous if the effects were dropped. The template
asserts nothing was refused before comparing:
```rust
# use my_project_core::{Elapsed, Machine};
# let m: Machine<64> = Machine::new(Elapsed::threshold_ms(1));
let r = m.refused();
assert_eq!((r.bytes.get(), r.effects.get()), (0, 0), "nothing may be refused");
```
Same for "no tick was sent, so no idle can be owed": an unexpected variant
panics rather than being filtered out.

## 3. Chunk invariance (pillar 2, mandatory in core)
State lives on the struct between events, so the effects of an input cannot
depend on how the host split it. Test:
- arbitrary cut points (`prop::sample::Index`, sorted) vs the whole input;
- one-byte reads vs the model;
- for a protocol core, also every fixed read size 1..=64 and 4096, and every
  two-read split of each golden recording.

`cargo xtask arch` requires a `fn chunk_invariance_` in each core crate's
`tests/` (shipped); the `cargo xtask controls` case "no chunk-invariance
property" (renaming both tests) turns it red.

## 4. Seeds, case counts and regressions

| tier | where | cases | seed |
|---|---|---|---|
| gate | `test:unit` (shipped) | `PROPTEST_CASES=256` | `PROPTEST_RNG_SEED=1` |
| search | `qa:props` (shipped) | `PROPTEST_CASES=20000` | unset, so fresh per run |

Rules:
- The gate is deterministic: the same commit gives the same verdict. DEPARTS
  from proptest's default (a fresh random seed per run): it makes a CI red
  unreproducible and a CI green a sample.
- The search tier is not deterministic on purpose: it finds new cases.
- Never set `cases` in `ProptestConfig`. Spread the default so the
  environment governs both tiers:
  ```rust
  # use proptest::prelude::*;
  # use proptest::test_runner::FileFailurePersistence;
  proptest! {
      #![proptest_config(ProptestConfig {
          failure_persistence: Some(Box::new(FileFailurePersistence::WithSource("regressions"))),
          ..ProptestConfig::default()
      })]
      // tests
  }
  ```
- `WithSource("regressions")` writes `chunk_invariance.regressions` beside the
  test file. Commit it. Why: proptest replays persisted cases before
  generating new ones, so a case found by `qa:props` becomes part of the
  deterministic gate. The default persistence looks for `lib.rs`/`main.rs`,
  which an integration test does not have.
- A regression file is never deleted to make a test pass.

## 5. Differential oracles (planned in the template)
An oracle is another implementation of the same standard, run on the same
input, with its output compared to ours.

### Vendoring
- Vendor the reference at a pinned commit, with its licence, in a `tool` or
  dev-only crate; record it in `docs/research/sources/MANIFEST.toml` with
  sha256 (`cargo xtask sources`, shipped).
- Patches needed to build or instrument it are applied in `build.rs` by
  **exact anchor**: each anchor must match exactly once.
  ```rust
  /// Applies one patch to vendored source. Panics unless the anchor matches
  /// exactly once: zero means upstream moved, two means the patch is ambiguous.
  fn patch(src: &str, anchor: &str, with: &str) -> String {
      let n = src.matches(anchor).count();
      assert_eq!(n, 1, "patch anchor {anchor:?} matched {n} times");
      src.replacen(anchor, with, 1)
  }
  ```
  Why: a `replace` that matches nothing builds an unpatched oracle and the
  comparison silently tests the wrong thing.

### Pinned expected differences
Where we deliberately differ from the oracle (a DEPARTS decision, or an
oracle bug), pin the input and both outputs:
```toml
# tests/oracle/differences.toml
[[difference]]
input = "tests/oracle/inputs/tab-past-margin.bin"
ours = "<digest of our output>"
theirs = "<digest of the oracle's output>"
decision = "D-7"   # the decision-register entry that justifies the difference
```
The test asserts each pinned input **still differs**, and every other input
agrees. Why: a pinned difference must still differ -- if
the oracle or our code changed so they now agree, the pin is stale and its
decision needs re-reading.

### Drift
Fixtures generated once from an oracle drift silently when the oracle or its
inputs change. `qa:oracles` (planned) regenerates every oracle fixture from
the pinned reference and diffs against the committed ones; a diff is a red.

### Fixture generators
When the reference is a system library (a font shaper, a keymap, a colour
API), write a generator program that queries it and writes fixtures, commit
the generator and the fixtures, and record the library version in the
fixture README. The generator is a `tool` crate, never a dependency of
shipped code (`layers.toml`).

## 6. Checklist for a new property
- [ ] Generator draws from atoms; omissions commented and hand-tested.
- [ ] Compared to a model or oracle sharing no code with the implementation.
- [ ] Vacuity guarded (refusals zero, unexpected variants panic).
- [ ] No `cases:` override; regressions persisted and committed.
- [ ] Planted defect in the mechanism turns it red (see
      [gates-that-can-fail.md](gates-that-can-fail.md)).
