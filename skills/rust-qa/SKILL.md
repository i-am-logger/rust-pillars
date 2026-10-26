---
name: rust-qa
description: "Use when writing, reviewing or refuting Rust tests and gates: a test or property, a model or oracle, fixtures and recordings, #[ignore] or platform-only tests, mutation testing, fuzz/Miri/loom/Kani, a planted-violation control, or a refuter's review of a change."
license: CC-BY-NC-SA-4.0
---

# rust-qa: tests that can fail, gates that prove it

A test earns its place by going red when the code is wrong, for the reason it
names. Every rule below serves that. Depth is in `references/`; load the one
you need.

Status labels refer to the companion `rust-template` at commit `a076f93`
(branch `feat/pillars`): **shipped** = committed there; **planned** = in the
rust-pillars plan only. Never cite a planned gate as passing. Evidence labels
(measured / by-reading / inferred / unmeasured) are rust-research's.

## Placement and taxonomy — [references/test-taxonomy.md](references/test-taxonomy.md)

- File tests by what they verify, not by module: `crates/<name>-core/tests/chunk_invariance.rs`,
  `crates/<name>-core/tests/alloc.rs`, `crates/<name>-domain/tests/typesystem.rs`. Why: a reader asking "is the split
  invariant held?" finds one file. (shipped, `crates/<name>-core/tests/`)
- Private invariants in `#[cfg(test)] mod tests` beside the code; public
  claims in `tests/`. Why: integration tests see only what a host sees.
  (CONFORMING with BeautifulRust and the Cargo reference, "Cargo Targets".)
- Name a test as the sentence it proves: `a_full_queue_drops_and_counts`.
  Why: the failure line then says what broke, with no file to open.
- No mocks: time, randomness and I/O arrive as events (`Event::Tick(Millis)`).
  DEPARTS from BeautifulRust ("mockable I/O/time/randomness", "mocks behind
  test-util"): a sans-IO core has nothing to mock, and a mock is a second
  implementation nobody tests.
- Golden recordings: a directory with a `README.md` (provenance, tool, date,
  how to regenerate) and a sha256 list a test re-hashes; each digest change
  explained in `docs/record.md`. Why: a hand-edited fixture stops being
  evidence, silently. (planned)
- Exhaustive over every finite domain (a byte, a pair, an enum): sweep it
  against an independent definition and `assert_eq!(checked, N)`. Why: a loop
  that ran short passes every per-item assertion, and a sweep against the
  table's own generator checks nothing. (shipped:
  `the_table_agrees_with_an_independent_definition_for_every_byte`)
- Two implementations of one thing (scalar/SIMD, CPU/GPU, table/`match`) get
  an agreement test; the simple one is the specification.
- Hot-path allocation is a test: dhat, zero blocks over 1000 rounds, one
  profiler per test binary. (shipped: `crates/<name>-core/tests/alloc.rs`; `test:unit` runs
  `--all-features`, so code behind `alloc` is exercised, not compiled out)

## Properties and oracles — [references/properties-and-oracles.md](references/properties-and-oracles.md)

- Generate VALID inputs from an alphabet of atoms, not `any::<u8>()`. Why:
  random bytes mostly exercise the first rejection branch.
- Check against an INDEPENDENT model (std, a spec table, a vendored
  reference), never only code vs code. Worked lesson (measured: rust-pillars's
  own planted run on the template): a doubled position advance passed
  `chunk_invariance_any_split_gives_the_same_effects` (code vs code) and was
  caught by `chunk_invariance_one_byte_reads_agree_with_the_model` (vs std).
- Stream code: every read size and two-read split give the same effects.
  (shipped; `cargo xtask arch` requires a `chunk_invariance_` test in core)
- Persist failures with `FileFailurePersistence::WithSource("regressions")`
  and commit the file: a case found once is replayed forever. (shipped)
- The gate pins `PROPTEST_CASES=256 PROPTEST_RNG_SEED=1`; `qa:props` runs
  20 000 with fresh seeds. DEPARTS from proptest's default (a fresh seed
  every run): a CI red must be reproducible, and the search belongs in a
  tier of its own. (both shipped: `test:unit`, `qa:props`)
- Never set `cases:` in `ProptestConfig`; spread `..ProptestConfig::default()`
  so the environment decides. Why: a hard-coded count overrides both tiers.
- Differential oracles: vendored reference with exact-anchor patches that
  fail on 0 or >=2 matches; pinned differences must still differ;
  `qa:oracles` regenerates fixtures and diffs. (planned)

## Gates that can fail — [references/gates-that-can-fail.md](references/gates-that-can-fail.md)

- Every gate runs a positive control first and fails if its detector finds
  nothing on a known-bad fixture. Why: a scanner that matches nothing looks
  exactly like a clean tree. (shipped: every `cargo xtask` gate)
- Every gate has a planted violation that must fail with the expected text;
  a fail for another reason is WRONG, not red. (shipped: `cargo xtask
  controls`, `test:controls`, 31 cases; `test:fmt`, `test:doc`, `test:deny`
  and `test:mutants-diff` have no plant yet)
- Plant through the gate's own command (the task's `exec` line). Why: a
  hand-picked `cargo` line proves the test, not the gate.
- Not a red: a link error, a missing import, a test that did not compile.
- Never skip: a check that skips itself reads as a pass — `#[cfg]`-gate the
  test whole, and `panic!` when a resource is missing. (shipped: `cargo xtask
  tests` rejects a return after a `cfg!`/`env::var`/`Command::new`/`which(`
  probe and "skipped" prints; `platform-tests.toml` planned)
- Exact floors only: `assert_eq!(n, K)`, never `assert!(n > K)`. Why: a loose
  floor passes after a fixture is lost: `checked >= 50` over 60 fixtures lets
  10 vanish. (shipped: `cargo xtask tests`; `budgets.toml` fails
  over and under)
- Every `#[ignore]` is listed in `ignores.toml` with its reason; a listed test
  that is gone fails too. Prefer `#[ignore = "reason"]` (Rust Reference,
  "Testing attributes"). (shipped: `cargo xtask tests`)
- `compile_fail` cases are trybuild twins with a pinned `.stderr`; the rules
  and how to read the stderr are rust-domain's (`references/compile-fail.md`).
  (shipped: `tests/ui/{fail,pass}`, twins checked by `cargo xtask docs`)
- Every pub fn of domain and core is called by some test, resolved by name
  from test code. Why: an uncalled fn is dead or untested. (shipped: `cargo
  xtask coverage`, `test:coverage`)
- Docs are tested claims: links, named paths, twins and the CLAUDE.md byte
  budget are checked by `cargo xtask docs` (shipped); coverage-row and
  version meta-tests with `#![deny(dead_code)]` are planned (the workspace's
  `unused = "deny"` already makes a lost `#[test]` a build error).
- Flakiness: retries = 0 (nextest's default; never pass `--retries`), order
  shuffled in `qa:shuffle`. DEPARTS from nextest's retry option: a retry turns
  an intermittent bug into a green. (`qa:shuffle` planned)

## Profile

- `test:unit` runs under `[profile.qa]`: `inherits = "release"`,
  `overflow-checks = true`, `debug-assertions = true` (Cargo reference,
  "Profiles"). Why: a plain `--release` run has neither, so no test sees an
  overflow or a `debug_assert!`. (shipped)

## Strength, not coverage — [references/mutation-fuzz-miri-loom-kani.md](references/mutation-fuzz-miri-loom-kani.md)

- Mutation testing (`cargo-mutants`) is the strength gate: every mutant of a
  changed line in domain/core must be caught (`test:mutants-diff`, in
  `devenv test`); `qa:mutants` mutates all of domain and core. Both pin the
  proptest seed so the verdict is reproducible. (shipped) Template evidence
  (commit `c0f4705` message): the first run found 10 survivors, one a byte
  table tested against its own generator; after the fix, 0 survive.
- Line coverage is informational only: a high line percentage says nothing
  about what was checked. DEPARTS from the
  common coverage-percentage gate: it rewards running code, not checking it.
  (`ci.yml` `coverage` job, `fail_ci_if_error: false`)
- Fuzz every parser entry point with `cargo-fuzz`, corpus seeded from the
  recordings. (`qa:fuzz` planned)
- Miri and sanitizers on quarantine crates only: elsewhere `unsafe_code =
  "forbid"` rules out the UB they look for. (`qa:miri`, `qa:sanitize` planned)
- loom for every cross-thread hand-off (`test:loom`, planned); Kani for every
  "this arithmetic is total" claim (`qa:kani`, planned).

## QA process — [references/refuter-checklist.md](references/refuter-checklist.md)

- builder -> closed-brief refuter -> triage. The refuter gets the claim, the
  diff and the gates, not the builder's reasoning. Why: one who reads the
  argument checks the argument. (rust-template ships it as `.claude/workflows/lanes.js`;
  the checklist ships in this skill)
- Refuter: red first on the base at every read size and split; >=2
  perturbations per mechanism, a non-compiling one is unbuilt; gates on a
  pristine `git archive`; counts taken, not stated; every sentence is a claim
  to check; block only on code.
- The closed brief, the verdict schema (`{verdict, code[], prose[], checked[]}`)
  and triage are rust-research's (`references/refuter-brief.md`); this skill
  owns the code-side refuter rules R1-R8.

## Shipped vs planned in rust-template (commit a076f93)

| shipped | planned |
|---|---|
| `[profile.qa]`; `test:unit` = nextest + doctests, `--all-features`, `--no-fail-fast`, seed 1, 256 cases; `qa:props` (20 000 fresh) | golden recordings, oracles, `qa:oracles` |
| `chunk_invariance.rs`, `alloc.rs`, `typesystem.rs` + 4 twinned ui cases; `cargo xtask coverage` | `platform-tests.toml` |
| positive control in every xtask gate; `cargo xtask controls` (33 plants) | plants for `test:fmt`, `test:doc`, `test:deny`, `test:mutants-diff` |
| `cargo xtask snippets`: every Rust block in the Markdown runs (`rustdoc --test --cfg test`); `.claude/workflows/lanes.js` | |
| `cargo xtask tests`: no skips, exact floors, `ignores.toml` | `qa:fuzz`, `qa:miri`, `qa:sanitize` |
| `test:mutants-diff`, `qa:mutants` (`.cargo/mutants.toml`) | `test:loom`, `qa:kani`, `qa:shuffle` |
| `ci.yml`: `devenv test` on ubuntu, informational coverage | |
