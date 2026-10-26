# Gates that can fail: positive controls and planted violations

Load this when adding or changing a gate (an `xtask` check, a devenv task, a
CI job, a meta-test), or when a gate's green needs to be believed.

A gate that has never been seen red is a claim, not a check. Two disciplines
prove it can fail: a **positive control** inside the gate on every run, and a
**planted violation** per gate, run when the gate changes.

## 1. Positive controls (shipped in every `cargo xtask` gate)
Before a gate trusts a clean result it runs its detector on a fixture it
knows is wrong, and fails if the detector finds nothing.

From `xtask/src/main.rs`: "A scanner that silently matches nothing looks
exactly like a clean tree, and the control is what tells the two apart."

Shipped controls:
| gate | control |
|---|---|
| `cargo xtask budgets` | `scan::control()`: a fixture with a known count of every kind (raw primitive, bool param, `as` cast, `unsafe`, callback, pub field...) must be counted exactly |
| `cargo xtask arch` | the same scanner control, plus the `no_std` detector must fire on a file with `#![no_std]` and not on one without |
| `cargo xtask layers` | core -> domain must be accepted, core -> bin and an unlisted external rejected |
| `cargo xtask docs` | a broken link and a missing named path must be seen |
| `cargo xtask sources` | an unknown citation must be seen; a short digest rejected, a real sha256 accepted |
| `cargo xtask tests` | a fixture with one skip-after-`cfg!`, one "skipped" print, one `assert!(n > 3)`, one `#[ignore]` and one clean test must yield exactly those four problems |

Rules for a new gate:
- The control asserts an **exact** count on its fixture, not "found
  something". Why: a scanner that finds one of three kinds passes "found
  something".
- The control runs on every invocation, before the real scan, and its
  failure is a distinct message (`positive control failed: ...`). Why: a
  broken gate must not read as a dirty tree either.
- The control fixture is inline in the gate's source (`scan::FIXTURE`, the
  fixture string in `tests_gate::control`) so it cannot be deleted without
  the gate failing to build.
- The control also has a unit test (`the_scanner_counts_the_fixture_exactly`
  in `xtask/src/scan.rs`) so `test:unit` catches a broken scanner even when
  the gate is not run.

The same rule applies outside xtask: a grep-based CI check, a "no TODO"
script, a licence scan — each needs a known-bad input it must flag.
rust-research applies it to sources: "A source you cannot search is not a
source you have read."

## 2. Planted violations
For each gate, a planted edit that the gate must turn red, applied to a
throwaway copy of the tree (never the working tree).

Shipped: `cargo xtask controls` (`xtask/src/controls.rs`), run by
`test:controls`, last in `devenv test`. It copies the workspace to
`target/controls`, requires `cargo xtask all` green on the unplanted copy,
then applies each of its 32 cases and requires the gate's output to contain
the case's expected text. A gate that fails for any other reason is reported
**WRONG**, not red. Commit `a076f93` reports 33 of 33 failing as planted (the
newest plants a Rust block in the docs that does not compile, for `cargo
xtask snippets`). Their commands cover the xtask gates,
check, clippy, bare, features and trybuild; `test:fmt`, `test:doc`,
`test:deny` and `test:mutants-diff` have no plant yet (a gap: add one with
each new gate).

Representative cases (from `controls.rs`):
| plant | command | expected text in the output |
|---|---|---|
| raw primitive in a domain pub fn | `cargo xtask budgets` | the primitive finding |
| primitive through a trait impl (`impl From<u32>`) | `cargo xtask budgets` | the primitive finding |
| a budget above its count | `cargo xtask budgets` | count **under** budget |
| `fn chunk_invariance_` renamed | `cargo xtask arch` | `chunk_invariance_` |
| `#![no_std]` removed | `cargo xtask arch` | ``must be `#![no_std]` `` |
| core depends on a foreign workspace dependency | `cargo xtask layers` | the edge finding |
| a `ui/fail` case with no twin | `cargo xtask docs` | `no passing twin` |
| a `ui/fail` case that compiles | `cargo test -p <domain> --test typesystem` | `Expected test case to fail to compile` |
| a test that returns after `cfg!` | `cargo xtask tests` | `returns early after probing` |
| `assert!(n > 3)` | `cargo xtask tests` | `lower bound against a literal` |
| `#[ignore]` not in `ignores.toml` | `cargo xtask tests` | `not listed in ignores.toml` |
| `std` used in the core | `cargo build -p <core> --no-default-features --target x86_64-unknown-none` | ``can't find crate for `std` `` |
| a feature that does not build alone | `cargo hack check --each-feature` | ``cannot find type `PlantedMissing` `` |

Before `controls` shipped, a one-off scratch suite (measured: rust-pillars's
own planted run, 2026-10-24) found two things worth keeping as lessons:
- The leak bans in `clippy.toml` were written under `std::` paths only; in
  the no_std domain and core a `std::` path names nothing, so clippy banned
  nothing, silently. Only a plant revealed it; `clippy.toml` now lists the
  `alloc::` and `core::` paths too (commit `c31655b`).
- The doubled-position-advance plant passed the split property and failed
  the model property ([properties-and-oracles.md](properties-and-oracles.md)).
  `controls` plants no behavioural defect under a property or the alloc
  test; finding tests that miss a defect is cargo-mutants' job
  ([mutation-fuzz-miri-loom-kani.md](mutation-fuzz-miri-loom-kani.md)).

### Plant through the gate's own command
Run each plant through the command the gate runs (the devenv task's `exec`
line), not a cargo invocation chosen for the plant. Why: a plant red under
`cargo test -p my-project-core --features alloc --test alloc` proves the
test, not the gate. Before commit `65a3876`, `test:unit` passed no feature
flags and no member enables `my-project-core/alloc` (by-reading
`devenv.nix` and the manifests), so an allocation behind `alloc` was
invisible to the gate; that commit added `--all-features`.

The shipped `controls` follows this for every xtask case, but its compile
cases run narrowed commands (`cargo check -p <bin>`, `cargo clippy -p
<crate>`, `cargo test -p <domain> --test typesystem`) rather than the
`test:clippy`/`test:unit` exec lines. Known gap: a plant that only those
tasks' flags (`--all-targets --all-features`, `--cargo-profile qa`) would
reach is not proven red.

### A red must be for the intended reason
- Record the expected text per plant (lint name, rustc error code, finding
  text, trybuild message) and assert it, not just a non-zero exit.
- Not a red: a link error, a missing import, a syntax error in the plant, a
  test binary that did not compile, a timeout, a tool not installed. Each
  exits non-zero and proves nothing about the gate.
- A plant aimed at a test must compile and the test must **run and fail its
  assertion**. A plant that does not compile is unbuilt, never red
  (cargo-mutants calls this "unviable" for the same reason).
- A plant aimed at a lint is red only when the output names that lint (e.g.
  `unsafe_code`, `disallowed_methods`): for `cargo check`/clippy plants the
  compile error *is* the gate, so match the lint, not "error".
- Require green on the unplanted copy first: a gate already red turns every
  plant "red".

### Rules for plants
- Prefer appends, new files and stable markers (`#![no_std]`,
  `[lints] workspace = true`) over search-and-replace in code: an anchor that
  moved makes the plant a no-op. Where a replace is needed, require exactly
  one match (same rule as oracle patches).
- Guard the path: every edit resolves inside the scratch copy, or is refused.
- One plant per mechanism minimum; two for a scanner-based gate (an item form
  the scanner may miss, e.g. a primitive inside `impl From<u32>`).
- Add the plant in the same change that adds the gate.

## 3. Never skip
A comparison that skips itself reads as a pass: a test that prints
"skipped" and returns is green on every machine that lacks what it needs.

Write:
```rust
#[cfg(target_os = "macos")]
#[test]
fn the_metal_backend_agrees_with_the_cpu() {
    let Some(device) = gpu::Device::system_default() else {
        panic!("no GPU device on a macOS runner: this test must run here (platform-tests.toml)");
    };
    assert_eq!(gpu::render(&device, &SCENE), cpu::render(&SCENE));
}
```
Never write:
```rust
#[test]
fn the_metal_backend_agrees_with_the_cpu() {
    let Some(device) = gpu::Device::system_default() else {
        eprintln!("skipped: no device");
        return;
    };
    assert_eq!(gpu::render(&device, &SCENE), cpu::render(&SCENE));
}
```
Rules:
- Select platforms at compile time (`#[cfg(..)]` on the whole test); inside,
  a missing resource panics. Where it runs is listed (planned
  `platform-tests.toml`) and the CI matrix runs it (ubuntu + macos in
  `ci.yml`; not yet run, nothing pushed).
- Shipped gate, `cargo xtask tests` (`xtask/src/tests_gate.rs`, task
  `test:tests`): in every `#[test]` fn under each member's `src/` and
  `tests/`, it rejects a `return` inside an `if` whose condition probes the
  environment (`cfg!(`, `env::var`, `Command::new`, `which(`) and any print
  saying it skipped.
- Its limits, stated so nobody relies on more: tests inside `proptest!` are
  not parsed, and a return after a probe it does not know (the
  `Device::system_default()` above) is caught only by the "skipped" print.
  Review for both. (The `gpu`/`cpu` paths above are placeholders for an
  adapter's API, not template code.)
- `#[ignore = "reason"]` is the only legal way not to run a test, and each
  one is listed in `ignores.toml` with its reason (shipped, same gate).

## 4. Exact floors
`assert_eq!(n, K)`, never `assert!(n > K)` or `>=`.
- `checked >= 50` over 60 fixtures lets 10 fixtures vanish unnoticed. It is
  the same mistake as a budget above its count, which measures nothing.
- Shipped: `cargo xtask tests` rejects `>`/`>=` (and `<`/`<=` with the
  literal on the left) against a literal in an assertion; an upper bound
  (`assert!(n < limit())`) is allowed.
- The same holds for budgets (`budgets.toml` fails over **and** under,
  shipped), sweep counts, recording counts, oracle agreement counts, and the
  `#[ignore]` count.
- When the right count changes, the change edits the literal in the same
  commit and says why.

## 5. Coverage of the API and of the docs (planned unless marked shipped)
- API coverage (shipped: `cargo xtask coverage`, `test:coverage`): every
  public fn and inherent method of domain and core is called from a test,
  resolved by name over `tests/`, `#[cfg(test)]` modules and the token
  streams of test macros such as `proptest!`. An uncalled pub fn is either
  dead (delete it) or untested (test it). Adapters and bins are not
  scanned (review).
- Meta-tests, each file `#![deny(dead_code)]`, so a helper that lost its
  `#[test]` attribute is a compile error rather than a silently unrun test.
  (The template's `[workspace.lints.rust] unused = "deny"` already covers
  this for every target; the inner attribute keeps it when a file is copied
  into a project without workspace lints.)
  - every path and test name cited in docs exists (paths and links shipped
    in `cargo xtask docs`)
  - every `done` row of a coverage table names a test that exists
  - the CLAUDE.md byte budget (shipped in `cargo xtask docs`, `budgets.toml`
    `[docs] claude_md_bytes`, exact)
  - version strings agree across manifests

## 6. Flakiness
- Retries = 0. nextest's default is 0; never pass `--retries` or set
  `NEXTEST_RETRIES`, never set `retries` in `.config/nextest.toml`. DEPARTS
  from nextest's optional retry policy: a retried failure is a green with a
  bug in it.
- Order: nextest runs each test in its own process, which already isolates
  in-process globals. `qa:shuffle` (planned) runs libtest with
  `-Z unstable-options --shuffle` on nightly to catch dependence on shared
  external state (files, ports, env).
- A flaky test is a bug report: fix or delete; never `#[ignore]` without a
  counted reason.
- `--no-fail-fast` in the gate (shipped in `test:unit`) so one red does not
  hide the others.
