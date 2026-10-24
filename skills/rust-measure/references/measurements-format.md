# MEASUREMENTS.md format

`MEASUREMENTS.md` holds numbers; `docs/record.md` holds the story (hypothesis,
falsifier, what was tried); `docs/decisions.md` holds what was decided because
of them. The template's header states the rules this file expands:

- current numbers only; superseded ones live in git history;
- every row names the commit that first carried it, the host, the power
  state, the statistic and the repetitions, and its A/A spread;
- if two ranges overlap there is no result;
- a change under its row's floor is reverted, and the row is still recorded.

`cargo xtask bench` (shipped) prints a header (host, power, commit, rounds)
and a `median [min-max] ms` table per metric and arm, plus one verdict line
per arm; copy its numbers into rows in this format, and fill the columns it
does not know (OS, toolchain, floor reasoning) by hand. Anything not from a
tool says "by hand" in the notes column.

## Columns

| Column | Content | Why |
|---|---|---|
| metric | what was measured, with unit: `throughput MB/s`, `cold wall-to-first-output ms` | a number without a unit and direction is ambiguous |
| input | recording name + short sha256, or `synthetic: <generator>` | results hold for the input they ran on |
| arm | `A`, `B`, `AA`, `A rebuilt`, `ceiling:<name>` | each arm is a row; the verdict compares them |
| median | median of kept rounds | robust to one bad round |
| range | min..max of kept rounds | the overlap test uses ranges, not medians |
| A/A range | the A/A arm's range in the same run | the noise floor of this run |
| floor | the smallest delta worth keeping for this metric | decides revert vs keep |
| verdict | `disjoint`, `no result`, `under floor -- reverted`, `byte-identical -- neutral` | one word a reader can act on |
| work counters | the counters read on this arm (`lines=..., refused=...`) | proves the arms did the same work |
| commit | the commit that first carried the number | ties the number to code |
| host | machine model and CPU | rates are per host |
| OS | OS and version | schedulers and caches change between versions |
| toolchain | `rustc --version` and the profile (`release`, flags such as `-align-all-functions=6`) | codegen is a variable |
| power | AC / battery, low-power mode, governor | frequency policy is a variable |
| cold/warm | for startup: `cold (moved: <caches>)` or `warm` | names exactly what was cold |
| statistic, reps | `median of 6 (7 rounds, first dropped)` | lets a reader judge the spread |
| label | `measured` (and nothing else may sit in this file) | estimates do not get rows |

Not every table needs every column: put the columns that are the same for a
whole run (host, OS, toolchain, power) in the run's heading line, and keep
the per-arm columns in the table.

## Template for one run

```markdown
### <metric> on <input> -- <short description of B>

Run: commit <A sha> vs <B sha>; host <model, CPU>; OS <version>;
toolchain <rustc version>, release, <flags>; power <AC, low-power off>;
median of <n-1> (<n> rounds rotated, first dropped). Falsifier: <written before
the run, copied from docs/record.md>.

| arm | median | range | work counters | verdict |
|---|---|---|---|---|
| A | <m> | <lo>..<hi> | <counters> | |
| AA | <m> | <lo>..<hi> | <counters> | A/A |
| B | <m> | <lo>..<hi> | <counters> | <verdict> (floor <f>) |
```

The angle-bracket fields are placeholders. A row is only ever filled from a
run's output; never type in a number nobody measured.

## Rules for rows

- **Reverted changes keep their row.** "Under floor -- reverted" and "no
  result" rows are evidence: the next person to try the same idea reads that
  it was tried, on which input, with what spread. A "things already tried that
  lost" list in `docs/record.md` points at them, and is itself re-measured
  before it is trusted on a new host or toolchain.
- **Byte-identical rows have no timing.** The text comparison is the
  measurement; say which section was compared.
- **Ceiling rows name the invariant counter** and its value on both arms.
- **A result on one input is a result on that input.** A second input is a
  second row.
- **Updating a number** replaces the row; the old one remains in git history,
  and any document that cited the old number is updated in the same commit.
- **Wrong rows** are struck through in place in `docs/record.md` with the
  correction and reason; `MEASUREMENTS.md` simply carries the corrected row.

## Floors

- Each metric has one floor, stated in its table and chosen before the run,
  from what the change is worth (complexity it adds) and from the A/A spread
  (a floor inside the A/A spread can never be met honestly).
