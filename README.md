# rust-code

[![CI](https://img.shields.io/github/actions/workflow/status/i-am-logger/rust-code/ci.yml?branch=master&label=CI&logo=githubactions&logoColor=white)](https://github.com/i-am-logger/rust-code/actions/workflows/ci.yml)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/i-am-logger/rust-code/badge)](https://scorecard.dev/viewer/?uri=github.com/i-am-logger/rust-code)
[![Nix](https://img.shields.io/badge/Nix-2b2b2b?logo=nixos&logoColor=white)](https://nixos.org)
[![devenv](https://img.shields.io/badge/devenv-2b2b2b?logo=nixos&logoColor=white)](https://devenv.sh)
[![License: CC BY-NC-SA 4.0](https://img.shields.io/badge/CC%20BY--NC--SA%204.0-2b2b2b?logo=creativecommons&logoColor=white)](LICENSE)

> A Claude Code plugin that teaches Claude to write Rust on six enforced
> pillars -- a domain type system that refuses primitives, event-driven
> sans-IO cores, layered crates, zero-copy, `no_std`, no `unsafe` -- and how
> to research, measure and test that code. Every rule is also a gate that
> fails the build: the gates run in this repository, and a new project gets
> them from the
> [`feat/pillars` branch of rust-template](https://github.com/i-am-logger/rust-template/tree/feat/pillars).
> The idioms are adapted from
> [BeautifulRust](https://github.com/morganlinton/BeautifulRust) under its MIT
> license.

## What it is

Two halves, versioned separately:

- **This plugin** -- an entry skill and six specialist skills, each a short
  `SKILL.md` with references loaded on demand, two commands, a refuter
  agent, and evals that score the skills by building and gating the code they
  produce.
- **rust-template** -- the enforcement: workspace lints, `cargo xtask`
  gates, devenv tasks that are CI. The gates are on its
  [`feat/pillars` branch](https://github.com/i-am-logger/rust-template/tree/feat/pillars)
  today, not on its `master`. A project made from that branch enables this
  plugin for everyone in it through its Claude Code project settings. This
  repository carries the same gates (`xtask/`), and every gate applies here
  too.

## Install

**Claude Code**, from the marketplace this repository is:

```bash
claude plugin marketplace add i-am-logger/rust-code
claude plugin install rust-code@rust-code
# or, inside a session:
/plugin marketplace add i-am-logger/rust-code
/plugin install rust-code@rust-code
```

From a local checkout: `claude --plugin-dir /path/to/rust-code`.

**Loading the skills.** A skill's description leaves loading to the model,
and on small tasks it skips it: with Sonnet, one run per condition (evidence
of the direction, not a rate), writing a whole crate loaded no skill. So the
plugin adds one line to any prompt about Rust (it mentions Rust, cargo, a
crate, a `.rs` file, the borrow checker, lifetimes, clippy or rustfmt, or the
project has a `Cargo.toml`) telling Claude to load the entry skill; with it,
the same task loaded `rust-code:rust-code` first. A one-line question it can
answer outright may still be answered without the skills. The hook reads the
prompt and checks for a `Cargo.toml`; it runs nothing in your project. Turn
it off with `--config nudge=false`.

**Opt-in gates while you work.** Install with `--config gate_on_edit=true`
(or set the option later with `claude plugin configure rust-code`), and after
every edit to a `.rs` file the plugin looks upward from that file for a
`layers.toml` beside an `xtask/` directory, runs `cargo xtask budgets` and
`cargo xtask arch` there, and shows Claude what fails, so a violation is
fixed in the same turn. It is off by default because the hook executes the
open project's own code -- its xtask, build scripts, proc macros and cargo
configuration -- with no permission prompt. An install is user-scoped by
default, so once the option is on it applies in every project you open
(inferred): do not turn it on if you open repositories you do not trust.

**claude.ai**: upload the whole plugin as a zip at Customize > Plugins, or
any one skill as a zip at Customize > Skills. `devenv tasks run dist:package`
builds them: `target/dist/rust-code-plugin.zip` and one zip per skill under
`target/dist/skills/`. Every skill stands alone: no link leaves its folder,
and its frontmatter uses only the keys claude.ai accepts (`test:plugin` holds
both).

## Use

The `rust-code` skill fires on any Rust work and routes to the specialist the
task needs; each specialist loads its references only when it is used. Call
any of them directly:

| Invoke | Fires on its own when |
|---|---|
| `/rust-code:rust-code` | any Rust work: writing, reviewing, debugging, testing, explaining |
| `/rust-code:rust-domain` | modelling quantities, ids, lifecycles, events and effects as types |
| `/rust-code:rust-architecture` | crates and dependencies, a core state machine, ownership, `no_std`, `unsafe`, SIMD, caches |
| `/rust-code:rust-idioms` | naming, signatures, traits, errors, docs, concurrency, refactoring |
| `/rust-code:rust-measure` | a claim about speed, size or startup; a benchmark; embedding data |
| `/rust-code:rust-qa` | tests, properties, oracles, mutation testing, gates |
| `/rust-code:rust-research` | choosing against a spec; citing a source; a decision record |

| Command or agent | Does |
|---|---|
| `/rust-code:gate [gate]` | runs every gate (or one) and reports what fails, by pillar |
| `/rust-code:adopt [--ratchet]` | brings the gates into an existing workspace: roles, then `cargo xtask init` |
| `@agent-rust-code:refuter` | a closed-brief refuter: tries to show a change is wrong, returns its verdict as JSON |

Both commands run the open project's code (`devenv test`, `cargo xtask`) and
pre-approve shell commands while they run; [SECURITY.md](SECURITY.md) lists
each grant.

## The pillars and their gates

The gates are `cargo xtask` commands in this repository and on rust-template's
[`feat/pillars` branch](https://github.com/i-am-logger/rust-template/tree/feat/pillars).

| # | Pillar | Gate |
|---|---|---|
| 1 | Type system: no raw primitive in a domain or core API outside a named seam | `cargo xtask budgets`, trybuild cases with twins |
| 2 | Event-driven: events in, effects polled, no callbacks, no clock | `cargo xtask arch`, the `chunk_invariance_*` property |
| 3 | Layering: every crate has a role and may depend only on what its role allows | `cargo xtask layers`, per-feature builds |
| 4 | Zero-copy: borrowed input, no allocation per event, own don't leak | clippy `disallowed_methods`, the allocation test |
| 5 | `no_std` for domain and core | `cargo xtask arch`, bare-metal builds |
| 6 | No `unsafe`: forbidden, a declared quarantine crate the only escape | the `forbid` lint, `cargo xtask arch` |

Every gate is seen failing for its reason: `cargo xtask controls` plants one
violation per gate in a scratch copy, and the suite passes only if each gate
names the planted fault.

## Compared with BeautifulRust

Both are Claude Code plugins for writing Rust. Side by side, against the
BeautifulRust copy pinned at commit 6b3a96c:

| | BeautifulRust | rust-code |
|---|---|---|
| Fires on any Rust work | one skill | the `rust-code` entry skill, which routes to six specialists |
| Install from a marketplace, invoke by slash command | yes | yes |
| claude.ai upload | a zipped skill folder | the whole plugin, or any one skill on its own (`dist:package`) |
| Evals under `claude plugin eval` | 3 cases, graded by a model | 10 cases with scaffolds and transcript graders, **plus** executable checks that build, lint, test and gate what was written, each case proven to fail on an untouched project and pass with a reference solution |
| Rules enforced | none: prose | every rule is a gate, here and on rust-template's `feat/pillars` branch, and every gate is seen failing for its reason |
| Getting loaded | its description only | its descriptions, plus a nudge on any Rust prompt (one run per condition: it turned a crate-writing task that loaded no skill into one that loaded the entry skill first) |
| In-session enforcement | none | opt-in hook runs the gates after each `.rs` edit |
| Commands, agents | none | `/rust-code:gate`, `/rust-code:adopt`, a refuter agent |
| Code in the skills | examples | every Rust block compiles and runs (`cargo xtask snippets`) |
| Context cost | ~160 tokens always-on, ~7.3k every time it fires (unmeasured: bytes / 4 over the pinned SKILL.md's 650-byte frontmatter and 504-line body) | ~980 always-on; ~1.6k for the entry skill, plus only the specialist a task needs (~3.9k-7.1k) (`claude plugin details` estimates, claude 2.1.303) |

It costs more context up front -- one description per skill and command --
and less when it works, because no task loads all of it. The idioms are
adapted from BeautifulRust under its MIT license (`NOTICE.md`), with every
departure written down: performance-first design for domain and core,
payload-free errors in a `no_std` core, all primitives refused rather than
only `bool`, `unsafe` forbidden rather than justified. The full comparison,
with sources pinned by hash, is
[the evidence note](docs/research/notes/a-rust-code-evidence.md).

## Evals

Two layers, over the same ten cases in `evals/`:

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
- **`eval-harness/`** runs each case headless with and without the plugin and
  scores what was left on disk with `checks.toml`: it builds, lints, tests and
  gates the code. Each arm runs twice, and a difference counts only if it is
  larger than the arm's own run-to-run spread.

Running `claude plugin eval`, from the devenv shell (all real model calls):

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
arm without the plugin depends on. `--reuse-baseline` runs the arm with the
plugin alone and takes a two-arm baseline's other arm for every case whose
key is unchanged; any change to the workspace, the scripts, the toolchain,
the claude version or any runner option that can change a run (the model,
the judge, `--runs`, the tools, `--max-cost-usd`, `--mocks`, `--tag`,
`--case`, `-j`: lib.sh, `pe_key_args`) refuses it, so a with-arm run must
repeat the baseline's options but `--json`, `--output-dir`, `--report` and
the publishing flags. So after the
runner changes of this release, the next full run must be a two-arm one.
`--changed-since` picks the cases whose `evals/<case>/` changed, or all of
them after any other change. `--keep-state` keeps every run's full trace and
start delay in the export's `state/`; the runner seals what it keeps (about
1.4 GB a run), so the run directories stay, listed in `kept-dirs.txt`, unless
`PE_UNSEAL=1` lets the script unseal, diff and remove exactly those (off by
default). The diff never follows a link the session left (it keeps the
link's target as text) and never opens a FIFO, socket or device (listed in
`skipped.txt`).

What makes a run fast, each with the probe that measured it
(`--probe`, haiku, on an 18-core machine under other load):

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

A run's `durationSeconds` counts from when the runner gives it a slot, not
from its session's start, and no session ran past its `timeout_seconds`: in
run 4 three runs waited 122, 835 and 878 s before their first trace event,
for a reason not found, so a 900 s case reported up to 1,770 s.
`--keep-state` records each run's start delay in `runs.tsv`.

Both runners build every working copy with the same script, outside the
repository, ~~so no run can read the checks it is graded by~~. That holds
for `eval-harness/`; under `claude plugin eval` the export is the plugin
root, which the arm with the plugin can read by path (measured). So the
export holds no reference solution, `checks.toml` or `.solve.sh`: they sit
beside it, where `--graders` takes them, and so do the runner's own results
in every mode (by default it writes them under the root, with the graders'
evidence: in `--graders`, the solution's files). Each case's prompt and graders stay
in it, as the runner needs them, and a session could read those; the script
warns when a result names a path under the root's `evals/` (run 4's visible
trace had none). Two modes run no
model and prove every case can grade: `devenv tasks run eval:control` (every
task check fails on the untouched project) and the harness's `--solution`
(every check passes with the case's reference solution). Every other run is
a real model call on your account, so `eval:run` is manual.

## Development

```bash
direnv allow      # or: devenv shell
dev-ci            # every gate, exactly as CI runs it (= devenv test)
```

## Releases

Automated with [release-plz](https://release-plz.dev) from
[Conventional Commits](https://www.conventionalcommits.org/): merging the
`chore: release` PR tags the version and writes `CHANGELOG.md`. Nothing is
published to crates.io -- the plugin ships as this repository. Bump
`.claude-plugin/plugin.json`'s version with the release.

## Funding

Solo-maintained, and sponsorship buys maintenance time rather than obligations.
See [FUNDING.md](FUNDING.md) — including how to arrange a commercial licence,
which the NonCommercial clause below makes a separate conversation.

## License

Creative Commons Attribution-NonCommercial-ShareAlike (CC BY-NC-SA) 4.0 International

See [LICENSE](LICENSE) for details.
