# Eval harness

How the plugin's evals are run, what each runner mode does, and what was
measured about running them. The [README](../README.md#evals) has the short
version.

Every run here makes real model calls on your Claude account, except the two
modes under [Proving every case can grade](#proving-every-case-can-grade).

## Two layers

Both run the same ten cases in `evals/`.

- **`claude plugin eval`**: did the expected skill fire, did the entry
  skill, did the agent verify its change (graded from the trace), and is the
  code right (graded file by file: the judge sees one file whole, while a
  long trace has its middle elided). Run it through
  `eval-harness/plugin-eval.sh`, not at the repository root: the runner loads
  every directory named `evals` at any depth (the controls gate leaves a copy
  under `target/`), and its sandbox has no network, so the script runs it on a
  clean export of HEAD with every dependency vendored and cargo offline.
  `devenv tasks run eval:graders` checks the graders themselves: each must
  pass on the reference solution and fail on the untouched start.
- **The Rust harness** (`devenv tasks run eval:run`, the crate in this
  directory) runs each case headless with and without the plugin and scores
  what was left on disk with `checks.toml`: it builds, lints, tests and gates
  the code. Each arm runs twice, and a difference counts only if it is larger
  than the arm's own run-to-run spread.

**The Rust harness is not sandboxed.** It runs `claude -p` with
`--permission-mode bypassPermissions` in a scratch copy on the host, and runs
each case's `scaffold.sh`: the session has your shell, your files, the network
and your Claude login. Run it only on a tree you trust -- not on a branch
whose `evals/` you have not read -- and preferably in a VM or container.

## Running `claude plugin eval`

From the devenv shell:

| what | command |
|---|---|
| full tier: every case, 3 runs, both arms | `devenv tasks run eval:full` |
| smoke tier: the cases tagged `smoke` in their `prompt.md`, 1 run, both arms, `-j 8` | `devenv tasks run eval:smoke` |
| only what changed since a commit | `sh eval-harness/plugin-eval.sh --changed-since <rev> -j 4 --no-publish` |
| plugin side only, against a stored baseline | `sh eval-harness/plugin-eval.sh --reuse-baseline <base.json> <out.json> -j 4 --no-publish` |
| keep each run's full trace | `sh eval-harness/plugin-eval.sh --keep-state …` |
| one fixed command in a haiku session | `sh eval-harness/plugin-eval.sh --probe <name> <cmdfile>` |
| free what older commits left under `$TMPDIR/rust-code-plugin-eval` | `sh eval-harness/plugin-eval.sh --prune` |

Smoke numbers are for iterating, never reported. Every run pins
`--model claude-opus-5-5 --judge-model haiku` unless you pass your own, and
writes `<result>.keys` beside its JSON: a key per case over everything the
arm without the plugin depends on.

`--reuse-baseline` runs the arm with the plugin alone and takes a two-arm
baseline's other arm for every case whose key is unchanged. Any change to the
workspace, the scripts, the toolchain, the claude version or any runner
option that can change a run (the model, the judge, `--runs`, the tools,
`--max-cost-usd`, `--mocks`, `--tag`, `--case`, `-j`: `lib.sh`,
`pe_key_args`) refuses it, so a with-arm run must repeat the baseline's
options except `--json`, `--output-dir`, `--report` and the publishing flags.
After a change to the runner, the next full run must be a two-arm one.

`--changed-since` picks the cases whose `evals/<case>/` changed, or all of
them after any other change.

`--keep-state` keeps every run's full trace and start delay in the export's
`state/`. The runner seals what it keeps (about 1.4 GB a run, measured; see
the header of `src/state.rs`), so the
run directories stay, listed in `kept-dirs.txt`, unless `PE_UNSEAL=1` lets the
script unseal, diff and remove exactly those (off by default). The diff never
follows a link the session left (it keeps the link's target as text) and
never opens a FIFO, socket or device (listed in `skipped.txt`).

## What makes a run fast

Each item with the probe that measured it (`--probe`, haiku, on an 18-core
machine under other load):

- the vendored dependencies are in the nix store, the one path every session
  reads in both arms, so no working copy holds a `vendor/`, and a
  prebuilt `target/` of the base workspace is cloned into each working copy:
  a fixed build, clippy, nextest, `xtask all` command took 24.5 and 31.6 s
  warm against 61.2 to 122.9 s cold;
- every cargo, nextest and test binary of a session is capped at
  max(1, cores / `-j`) (`EVAL_JOB_CAP`; `PE_JOB_CAP` overrides it). Against
  no cap, 8 sessions of a fixed command at `-j 8` took 130, 82 and 116 s
  capped and 73, 99 and 83 s uncapped: no result. The full tier stays at
  `-j 4` (97, 122 and 124 s for the same 8 sessions), as `-j 8` showed no
  gain its spread could separate; these sessions are almost all tool time,
  where a real one is about half model time;
- ~~the eval-only environment builds with `line-tables-only` debug info and
  no incremental cache (12.1 and 12.5 s against 14.9 and 15.5 s, measured
  outside the sandbox; inside it the runs overlapped)~~ withdrawn: the
  outside numbers had no A/A arm and inside the sandbox the runs overlapped,
  so it was never shown faster; the eval environment builds with the
  project's own dev profile;
- what a timed-out session leaves running is ended while the suite runs:
  the runner kills the session, not the commands it started;
- the export's vendoring and prebuilt target are keyed on content, so a
  commit that changes only skills reuses both (an export then takes seconds).

## Durations

A run's `durationSeconds` counts from when the runner gives it a slot, not
from its session's start, and no session ran past its `timeout_seconds`. In
the full run on commit 28c0e7a (2026-10-25, claude 2.1.303; its result is
[fixtures/run-28c0e7a.json](fixtures/run-28c0e7a.json)), three runs waited
122, 835 and 878 s before their first trace event, for a reason not found, so
a 900 s case reported up to 1,770 s. `--keep-state` records each run's start
delay in `runs.tsv`.

## What a session can read

Both runners build every working copy with the same script, outside the
repository, ~~so no run can read the checks it is graded by~~. That holds
for the Rust harness; under `claude plugin eval` the export is the plugin
root, which the arm with the plugin can read by path (measured). So the
export holds no reference solution, `checks.toml` or `.solve.sh`: they sit
beside it, where `--graders` takes them, and so do the runner's own results
in every mode (by default it writes them under the root, with the graders'
evidence: in `--graders`, the solution's files). Each case's prompt and
graders stay in it, as the runner needs them, and a session could read
those; the script warns when a result names a path under the root's `evals/`
(the visible trace of the 28c0e7a run had none).

## Proving every case can grade

Two modes run no model: `devenv tasks run eval:control` (every task check
fails on the untouched project) and the Rust harness's `--solution` (every
check passes with the case's reference solution). Every other run is a real
model call on your account, so `eval:run` is manual.
