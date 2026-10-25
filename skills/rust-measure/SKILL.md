---
name: rust-measure
description: "Use when a Rust change claims to be faster, smaller or quicker to start, before keeping a hot-path change, or when writing a benchmark, A/B comparison, profile, startup timing or MEASUREMENTS.md row, or deciding to embed precomputed data in the binary."
---

# rust-measure

A performance claim is a measurement or it is "unmeasured". This skill says how
to get a measurement that can be wrong in only the ways you checked.

Domain and core are designed for performance from the start (rust-architecture,
which records the DEPARTS from BeautifulRust); this skill is how every change
is then measured, in every crate.

## Tooling status (rust-template)

| Interface | State |
|---|---|
| `crates/<name>-core/tests/alloc.rs` (dhat: 0 blocks, 0 bytes over 1000 rounds) | shipped, runs in `test:unit` |
| `crates/<name>-domain/src/class.rs` (`const fn` table + exhaustive agreement test) | shipped, runs in `test:unit` |
| `[profile.release]` lto + codegen-units=1 (benches); `[profile.qa]` (gates) | shipped |
| `MEASUREMENTS.md` header rules | shipped |
| domain/core `clippy.toml` bans `Instant`/`SystemTime`/`Thread` | shipped, `test:clippy` |
| `cargo xtask controls` (one planted violation per gate, must fail for its reason) | shipped, `test:controls` |
| `cargo xtask arms <rev> <rev>` (git-archive builds at one path, binary + text compared first) | shipped |
| `cargo xtask bench --arm n=<bin> --aa` (rotated, first round dropped, ranges, floor, refuses battery) | shipped |
| `crates/<name>/src/marks.rs` (`startup <ms> <stage>` on stderr under `MY_PROJECT_STARTUP=1`) | shipped |
| `cargo xtask gen` (artefact generators) | **planned, not shipped** |
| `unexpected_cfgs` check-cfg for `cfg(arm)` in the root `Cargo.toml` | **not in the template**: add it with the first arm |
| artefact sha MANIFEST, `__TEXT`/binary-size ratchet, bytes-out-per-byte-in and instructions-per-op counters | **planned, not shipped** |

Until a planned tool ships, follow its protocol by hand and say so in the row.
The only shipped `MANIFEST.toml` is `docs/research/sources/` (research sources).

## The two invariants

- Every hot-path change states its effect on **write amplification** (bytes
  the hot path writes per input byte) and **working set vs cache** (hot
  structs + tables vs L1/L2 of each target) -- they are what speed is made of.
- Both need **a measurement, not an argument**: a counter, a const-asserted
  `size_of`, or an A/B row. Reasoning about them is a hypothesis.

## A/B hygiene -> [references/ab-protocol.md](references/ab-protocol.md)

- Build every arm from `git archive <rev>` extracted at **one** absolute path,
  one after another; never from a worktree -- the path is compiled into the
  binary and shifts layout (inferred mechanism: ab-protocol.md). `cargo xtask arms` does
  this for two revisions; `cfg(arm)` builds are by hand.
- Add an **A/A arm** (a copy of A); its spread is the noise floor of the run.
- `cmp` the arms' `__TEXT,__text` (ELF: `.text`) **before timing**: byte-identical
  text is "neutral, proven" and stands in for a timing.
- An arm that **should** differ but is byte-identical to A was not built as
  intended: a mistyped `--cfg arm="..."` value builds A silently (check-cfg
  checks only cfgs written in source). Fix the arm; never record it "neutral".
- Rotate arm order over **>= 4 rounds, drop the first** -- round one pays page-in
  and cache warm-up for whichever arm ran first.
- Same host for every arm; record power state (AC/battery, low-power mode,
  governor) -- frequency policy is a variable.
- Write down **before running** what result would prove the change wrong.
- Ranges (min..max of kept rounds) **overlap -> no result**. **Disjoint -> a
  result.** Never report a median difference inside overlapping ranges.
- Every row has a **floor**; a disjoint change under it is **reverted, and the row
  is still recorded** -- a lost attempt is evidence the next attempt needs.
- Read the **work counters on both arms before the rate**: an arm that does less
  work is a different workload, not a faster one.
- A **ceiling arm** (remove named work) must remove **only** that work; prove it
  with an invariant counter equal on both arms: a stub can remove other work
  with it, and only the counter shows that.
- Arms are `cfg(arm = "...")`, declared once as `unexpected_cfgs` check-cfg in
  the root `[workspace.lints.rust]` (add it; rustc book, "Checking conditional
  configurations"), never Cargo features -- arm features are not additive and
  fail `--all-features -D warnings` (`test:clippy`).
- Pass arm flags as `RUSTFLAGS="$RUSTFLAGS ..."`: the devenv shell already sets
  `RUSTFLAGS` to `-D warnings` (`nix/rust.nix`); a bare assignment drops it
  from one arm, and the arms no longer differ by one thing.
- Arm builds keep the workspace lints: cfg-gate whatever only the removed work
  used (`unused` is denied; domain/core suppression budgets are 0). `cargo
  xtask budgets` parses with `syn`, skipping only `cfg(test)`: arm-only casts and
  `#[expect]`s count like shipped ones.
- **Code placement is a variable**: build all arms with the same
  `-C llvm-args=-align-all-functions=6` when hot code is near a change: placement alone moves a timing, and bytes
  that nothing reads can move a hot fn.
- **DEPARTS from BeautifulRust** ("criterion/divan with `black_box`"): evidence
  rows come from separately built arms run interleaved with an A/A arm
  (`harness = false`); an in-process baseline has no A/A and no placement control.

## Instruments -> [references/instruments.md](references/instruments.md)

- Know what each profiler counts: **sampling** profilers (macOS `sample`) count
  blocked time, **on-CPU** profilers (`dtrace profile`) do not, so the two can disagree
  about the same run.
- Identify an unfamiliar instrument with **control programs** (one that sleeps,
  one that spins, one with a known instruction count) before trusting it.
- Hardware counters only with control programs that identify each column;
  **never assign a counter from its magnitude**.
- **Never measure an edited recording**; use real recordings (pinned by sha256)
  and ceiling arms to ask "what if this were free".
- No number you did not measure: write **"unmeasured"**; label every claim
  measured / inferred / by-reading.

## Startup -> [references/startup-and-embedding.md](references/startup-and-embedding.md)

- Stage marks live in the **bin or adapter** only (`marks.rs`: `startup <ms> <stage>`
  on stderr under `MY_PROJECT_STARTUP=1`, read once with `var_os`, parsed by
  `cargo xtask bench`); core has no clock (`test:clippy` bans `Instant`).
- Measure **cold and warm** arms; make cold by **moving caches aside** (rename,
  restore after; `bench --cold <dir>`), never by deleting or guessing.
- Record wall-to-first-useful-output from outside the process; marks explain
  where the time went inside it.

## Embed at build time -> [references/startup-and-embedding.md](references/startup-and-embedding.md#embed-at-build-time)

- Precompute anything deterministic -- tables, shader binaries, font subsets,
  parsed default config -- so startup does no work it could have done at build.
- Small computed tables: `const fn` into a `static` needing no initialiser
  (`class.rs`); drift test = exhaustive agreement with the readable definition.
- Blobs: one `static X: &[u8] = include_bytes!(...)`, **never a `const`** -- a
  const is inlined per use and may copy the bytes at each use; a static is one
  location (Rust Reference, "Constant items" vs "Static items").
- Generators are tool-role (planned `cargo xtask gen`); domain/core get no
  `build.rs` deps (`cargo xtask layers` checks build edges too).
- Each artefact has a **drift test**, a **pinned sha**, and a **cold/warm startup
  row** proving it pays; costs it exposes are recorded as open, not hidden.

## CI -> [references/ci-counters.md](references/ci-counters.md)

- CI gates on **deterministic work counters**, never timings: shared runners
  cannot time (no fixed host, no power control, no A/A).
- Counters: allocations per event (shipped, `alloc.rs`), bytes written per byte
  in, instructions per op (optional profile), binary/`__TEXT` size, embedded
  artefact sizes -- each ratcheted **budget = count**, like `cargo xtask budgets`.
- Every counter gate has a positive control: plant the regression, see red
  (`cargo xtask controls` does this per gate; it has no case for `alloc.rs`
  yet, so plant that one by hand).
- Timings are taken locally under the A/B protocol and land in MEASUREMENTS.md.

## Recording -> [references/measurements-format.md](references/measurements-format.md)

- One row per metric x arm: commit, host, OS, toolchain, power, statistic,
  reps, median, range, A/A range, floor, verdict.
- Current numbers only; superseded rows live in git history (template rule).
- Verdict is one of `disjoint`, `no result`, `under floor -- reverted`,
  `byte-identical -- neutral` (the strings `bench`/`arms` print map onto these:
  [ab-protocol §6](references/ab-protocol.md#6-verdict)).

## Before handing back a performance change

1. Falsifier written down. 2. Arms from `git archive` at one path, A/A included.
3. Text `cmp`ed. 4. Work + invariant counters equal where they must be.
5. >= 4 rotated rounds, first dropped. 6. Verdict from ranges and floor.
7. Row recorded, including reverts. 8. Anything not done says "unmeasured".
