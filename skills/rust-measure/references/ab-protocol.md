# A/B protocol

How to compare two builds so that the comparison can only fail in the ways you
checked. Two shipped tools automate part of it (rust-template `xtask/src/measure.rs`):
`cargo xtask arms <rev-a> <rev-b> [--bin <name>]` builds both revisions from
`git archive` at one path (`target/arms/src`) and compares the binaries, then
their text sections, before anyone times anything; `cargo xtask bench --arm
name=<binary> ... --aa` times whole-process launches, rotated, first round
dropped, and prints a verdict. `cfg(arm)` builds and in-process throughput
runs are not automated: run those steps by hand and write "by hand, per
rust-measure A/B protocol" in the row.

## 0. Before anything runs

- Write the hypothesis and its **falsifier** in `docs/record.md` first:
  "B is wrong if the A/B ranges on recording R overlap, or if the number of
  output lines differs between arms." A falsifier written after the numbers
  is a story.
- Name the **metric**, the **statistic** (median of kept rounds), the **floor**
  (smallest change worth keeping), and the **work counters** you will read.
- Name the **inputs**: real recordings, pinned by sha256, each with a README
  saying how it was captured. Never an edited recording (see instruments.md).

## 1. Build the arms

Each arm is built from a pristine tree, at the **same absolute path**, one after
the other. The path is part of the build: it ends up in the binary (panic
locations, debug info, `file!()`), and a different length shifts sizes and so
placement (inferred mechanism); a worktree also brings its own `target/`.

```sh
root=/tmp/arms            # one path, reused for every arm
for arm in A:"$BASE" B:"$HEAD"; do
  name=${arm%%:*} rev=${arm#*:}
  rm -rf "$root/src" && mkdir -p "$root/src" "$root/bin"
  git archive "$rev" | tar -x -C "$root/src"
  ( cd "$root/src" && cargo build --release -p my-project )
  cp "$root/src/target/release/my-project" "$root/bin/$name"
done
cp "$root/bin/A" "$root/bin/AA"   # the A/A arm: same bytes, separate file
```

- `git archive` excludes untracked and ignored files, so the arm is exactly the
  commit; an uncommitted edit cannot leak into either arm.
- Use plain `release` (`lto = true`, `codegen-units = 1`; Cargo reference,
  "Profiles"), not `qa`: the `qa`
  profile's overflow checks and `debug_assert!` change the code being timed.
- Same toolchain for every arm (the template's devenv uses channel `stable`,
  fixed by the rust-overlay revision locked in `devenv.lock`); record it.
- `rm -rf "$root/src"` also removes `target/`, so every arm is a clean build:
  no arm can be a cached binary of another.
- **Append, never assign, `RUSTFLAGS`.** The devenv shell exports
  `RUSTFLAGS=-D warnings` (`nix/rust.nix`, `languages.rust.rustflags`); a
  bare `RUSTFLAGS='...'` drops it for that arm only. Write
  `RUSTFLAGS="$RUSTFLAGS <extra>"` and record the full value per arm.
- If hot code sits near the change, add the **same** placement flag to every
  arm: `RUSTFLAGS="$RUSTFLAGS -C llvm-args=-align-all-functions=6"` (64-byte function
  alignment; `-C llvm-args` is in the rustc book, "Codegen options", and
  passes the flag to LLVM unchecked). Without it, identical code can time
  differently across rebuilds; with it the arms differ only by what the change
  did.
- A second A/A variant -- **A rebuilt** at the same path -- is a
  reproducibility check: its text must be byte-identical to A's (section 2).
  If it is not, the build itself moves code and that difference is placement
  noise to control (flag above) before any A/B means anything. The copied
  A/A measures run noise only.

## 2. Compare text before timing

```sh
# macOS (Mach-O)
otool -X -s __TEXT __text "$root/bin/A" > "$root/A.text"
otool -X -s __TEXT __text "$root/bin/B" > "$root/B.text"
cmp "$root/A.text" "$root/B.text" && echo "byte-identical text: neutral, proven"
# Linux (ELF)
objcopy -O binary --only-section=.text "$root/bin/A" "$root/A.text"
```

- Byte-identical `__TEXT,__text` means the change cannot alter speed through
  code: record the row as `byte-identical -- neutral` and skip the timing.
- If the change touched tables or constants, compare their sections too
  (`__TEXT,__const` / `.rodata`): identical code reading different data is not
  neutral.
- Text that differs only by a shifted address is still a placement change:
  time it. (`otool` prints addresses, so a shift shows as a difference.)
- The reverse check matters as much: a ceiling, probe or B arm whose text is
  byte-identical to A **was not built as intended**. A mistyped value in
  `--cfg arm="..."` builds A silently -- check-cfg only checks the cfgs
  written in source, never the value passed on the command line (verified on
  the template's toolchain). Fix the arm; never record it "neutral".

## 3. Run: rotated rounds

- Arms `[A, B, AA]` (plus any ceiling arms). Round `r` runs every arm once,
  starting at arm `r mod k`, so no arm always runs first or last.
- **At least 4 rounds; drop round 1.** Round 1 pays page-in, file cache and
  frequency ramp for whichever arm it ran first. `bench` rotates and drops
  round 1 itself; it defaults to 7 rounds but accepts 3, so never pass
  `--rounds` below 4.
- Same host for every arm and every round. Nothing else heavy running.
- Record power state before and after: AC or battery, low-power mode
  (`pmset -g` on macOS), CPU governor on Linux
  (`/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor`). A change of
  power state mid-run voids the run. `bench` prints the power source in its
  header and refuses to run on battery unless `--battery` is passed (then
  say so in the row); low-power mode and the governor are still yours.
- Output of the program goes to a counting sink or `/dev/null`, never a
  terminal: the terminal is then part of the measurement.

## 4. Read the work counters first

Before looking at any rate, read the work counters on **every** arm:

- counters the program already exposes (the template's `Machine::refused()`
  `Refusals`; lines, counted from the emitted `Effect::Line`s or the output);
- counters you add for the run (`cfg(arm = ...)`-gated, so they cost nothing
  in shipped builds).

If the counters differ where the change was not supposed to change them, the
arms ran different workloads and the timing means nothing. Fix the arm first.

## 5. Ceiling and probe arms

- **Ceiling arm**: remove one named piece of work to bound what optimising it
  could ever give. **Probe-double arm**: do that work twice to measure its cost
  from the other side.
- A ceiling arm must remove **only** the named work. Prove it with an
  **invariant counter** -- a count the removal must not change -- equal on the
  ceiling arm and A. A stub that also removed neighbouring work reads faster for
  the wrong reason: the gain it shows bounds nothing, and only the invariant
  counter falling on that arm shows it.
- Declare arms as cfgs, not features (the template has no `cfg(arm)`
  declaration yet; add the one below with the first arm). Features must be additive, and a
  ceiling arm is by definition not: turned on together with the rest, arm
  features fail `--all-features` under `-D warnings` (in the template that is
  `test:clippy`).
  Declare the values once in the root manifest -- member crates set
  `[lints] workspace = true`, and the Cargo reference ("The `[lints]`
  section") does not allow crate-level overrides alongside it. check-cfg
  itself is specified in the rustc book, "Checking conditional
  configurations":

```toml
# Cargo.toml (root)
[workspace.lints.rust]
unexpected_cfgs = { level = "deny", check-cfg = ['cfg(arm, values("ceiling_classify"))'] }
```

```rust
# #[derive(Clone, Copy)]
# pub enum ByteClass { Newline, Space, Other }
# const fn build() -> [ByteClass; 256] { [ByteClass::Other; 256] }
# fn main() { let _ = ByteClass::of(b'a'); }
// crates/<name>-domain/src/class.rs -- a ceiling arm that is WRONG on purpose.
#[cfg(not(arm = "ceiling_classify"))]
static CLASS: [ByteClass; 256] = build();

impl ByteClass {
    /// The named work: one table load.
    #[cfg(not(arm = "ceiling_classify"))]
    #[must_use]
    #[inline]
    pub(crate) fn of(b: u8) -> Self {
        CLASS[usize::from(b)]
    }

    /// Ceiling: "classification is free".
    #[cfg(arm = "ceiling_classify")]
    #[inline]
    pub(crate) fn of(_b: u8) -> Self {
        Self::Other
    }
}
```

- **Arm builds run under the same workspace lints.** Whatever only the
  removed work used loses its last reader and fails `unused = "deny"`; here
  `build`, `reference` and the agreement test need the same
  `#[cfg(not(arm = "ceiling_classify"))]` as `CLASS` (verified: ungated, the
  arm fails with "`CLASS` is never used", "`build` ...", "`reference` ...").
  Gate them; do not add an `#[expect(dead_code, ...)]` -- domain and core
  suppression budgets are 0 (`cargo xtask budgets`).
- `cargo xtask budgets` parses the source with `syn` and skips only
  `#[cfg(test)]`/`#[test]` items; it does not evaluate other cfgs. Code that
  exists only in an arm is counted like shipped code: an arm that adds an `as`
  cast or an `#[expect]` fails the budget. Write arms without either.
- This arm also removes every line end and every word boundary: the invariant
  counter "lines emitted" drops to zero on it, so whatever rate it shows
  bounds nothing. The counter comparison is what makes an arm honest -- write
  it before reading the rate. A usable ceiling here keeps the classification
  result and removes only its cost (for example, classify from a precomputed
  per-recording array outside the timed region), and still shows "lines
  emitted" and "words counted" equal to A's.

Build the arm with `RUSTFLAGS="$RUSTFLAGS --cfg arm=\"ceiling_classify\""`;
every other flag must match the A arm's exactly. Then `cmp` its text against
A's (section 2): identical means the cfg did not take.

## 6. Verdict

Per metric, per arm: median and range (min..max) over the kept rounds.

| Situation | Verdict in the row | What the shipped tool prints |
|---|---|---|
| text byte-identical | `byte-identical -- neutral` (no timing needed) | `arms`: `byte-identical binaries -- neutral, proven`, or `identical machine code (text section) ...` |
| A and B ranges overlap | `no result` -- not "slightly faster" | `bench`: `overlap: no result` |
| disjoint, delta below the floor | `under floor -- reverted`; the row is still recorded | `bench`: `disjoint but under the <F> ms floor: revert` |
| disjoint, delta at or above the floor | `disjoint` -- a result | `bench`: `disjoint: faster by` / `slower by` |
| A and AA ranges disjoint | run is void: fix the environment, rerun | `bench --aa` prints the A/A line; reading it is yours |

`bench`'s default floor is 1 ms (`--floor-ms`); set the row's own floor.

- The delta of a disjoint result is stated together with the A/A spread.
- A result on one recording is a result on that recording. Say which.
- A result that contradicts the written falsifier is reported as the change
  being wrong, even if another metric improved.

## 7. Instrument self-checks

Before trusting a new harness (or after changing it), run these controls.
The template's commit `c0f4705` reports 1 and 2 for `bench` (a planted 30 ms
sleep read as disjoint, slower by 45 ms). 3 and 4 are not automated. `arms`
printing "byte-identical" for two equal revisions does not prove 3: `arms`
reuses one `CARGO_TARGET_DIR` for both revisions, so a skipped rebuild would
print the same thing (inferred from `measure.rs`, not run). Treat an `arms`
"byte-identical" for revisions that should differ as "not built as intended"
(section 2):

1. A vs A/A must report `no result`.
2. An arm with a planted `sleep(30 ms)` on the measured path must report
   `disjoint` and over a 20 ms floor.
3. Two arms from the same commit (A and A rebuilt) must report
   `byte-identical -- neutral`.
4. An arm with a deliberately mistyped `--cfg arm` value must be flagged
   "not built as intended", not reported neutral.

A harness that fails one of these measures itself, not the code.
