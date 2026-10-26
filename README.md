<div align="center">

# rust-pillars

[![CI](https://img.shields.io/github/actions/workflow/status/i-am-logger/rust-pillars/ci.yml?branch=master&label=CI&logo=githubactions&logoColor=white)](https://github.com/i-am-logger/rust-pillars/actions/workflows/ci.yml)
[![Nix](https://img.shields.io/badge/Nix-2b2b2b?logo=nixos&logoColor=white)](https://nixos.org)
[![devenv](https://img.shields.io/badge/devenv-2b2b2b?logo=nixos&logoColor=white)](https://devenv.sh)
[![License: CC BY-NC-SA 4.0](https://img.shields.io/badge/CC%20BY--NC--SA%204.0-2b2b2b?logo=creativecommons&logoColor=white)](LICENSE)

</div>

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
claude plugin marketplace add i-am-logger/rust-pillars
claude plugin install rust-pillars@rust-pillars
# or, inside a session:
/plugin marketplace add i-am-logger/rust-pillars
/plugin install rust-pillars@rust-pillars
```

From a local checkout: `claude --plugin-dir /path/to/rust-pillars`.

**Loading the skills.** A skill's description leaves loading to the model,
and on small tasks it skips it: with Sonnet, one run per condition (evidence
of the direction, not a rate), writing a whole crate loaded no skill. So the
plugin adds one line to any prompt about Rust (it mentions Rust, cargo, a
crate, a `.rs` file, the borrow checker, lifetimes, clippy or rustfmt, or the
project has a `Cargo.toml`) telling Claude to load the entry skill; with it,
the same task loaded the entry skill first. A one-line question it can answer
outright may still be answered without the skills. The hook reads the
prompt and checks for a `Cargo.toml`; it runs nothing in your project. Turn
it off with `--config nudge=false`.

**Opt-in gates while you work.** Install with `--config gate_on_edit=true`
(or set the option later with `claude plugin configure rust-pillars`), and after
every edit to a `.rs` file inside the open project the plugin looks upward
from that file, no higher than the project's root, for a `layers.toml`
beside an `xtask/` directory, runs `cargo xtask budgets` and
`cargo xtask arch` there, and shows Claude what fails, so a violation is
fixed in the same turn. A file outside the project, once links and `..` are
resolved, runs nothing. The option is off by default because the hook
executes the project's own code -- its xtask, build scripts, proc macros and
cargo configuration (`.cargo/config.toml`, `rust-toolchain.toml`), which can
run anything -- with no permission prompt. An install is user-scoped by
default, so once the option is on it applies in every project you open
(inferred): do not turn it on if you open repositories you do not trust.

**claude.ai**: upload the whole plugin as a zip at Customize > Plugins, or
any one skill as a zip at Customize > Skills. `devenv tasks run dist:package`
builds them: `target/dist/rust-pillars-plugin.zip` and one zip per skill under
`target/dist/skills/`. Every skill stands alone: no link leaves its folder,
and its frontmatter uses only the keys claude.ai accepts (`test:plugin` holds
both).

## Use

The `rust-pillars` skill fires on any Rust work and routes to the specialist the
task needs; each specialist loads its references only when it is used. Call
any of them directly:

| Invoke | Fires on its own when |
|---|---|
| `/rust-pillars:rust-pillars` | any Rust work: writing, reviewing, debugging, testing, explaining |
| `/rust-pillars:rust-domain` | modelling quantities, ids, lifecycles, events and effects as types |
| `/rust-pillars:rust-architecture` | crates and dependencies, a core state machine, ownership, `no_std`, `unsafe`, SIMD, caches |
| `/rust-pillars:rust-idioms` | naming, signatures, traits, errors, docs, concurrency, refactoring |
| `/rust-pillars:rust-measure` | a claim about speed, size or startup; a benchmark; embedding data |
| `/rust-pillars:rust-qa` | tests, properties, oracles, mutation testing, gates |
| `/rust-pillars:rust-research` | choosing against a spec; citing a source; a decision record |

| Command or agent | Does |
|---|---|
| `/rust-pillars:gate [gate]` | runs every gate (or one) and reports what fails, by pillar |
| `/rust-pillars:adopt [--ratchet]` | brings the gates into an existing workspace: roles, then `cargo xtask init` |
| `@agent-rust-pillars:refuter` | a closed-brief refuter: tries to show a change is wrong, returns its verdict as JSON |

Both commands run the open project's own code: `/rust-pillars:gate` its build,
xtask and tests (`devenv test`, `cargo xtask`), `/rust-pillars:adopt`
`cargo metadata` and `cargo xtask`. Each pre-approves only a few scoped shell
commands while it runs, and every edit or other command prompts;
[SECURITY.md](SECURITY.md) lists each grant. `/rust-pillars:adopt` copies files
from rust-template, which is licensed CC BY-NC-SA 4.0 (NonCommercial,
ShareAlike): it says so and asks before it copies anything.

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

| | BeautifulRust | rust-pillars |
|---|---|---|
| Fires on any Rust work | one skill | the `rust-pillars` entry skill, which routes to six specialists |
| Install from a marketplace, invoke by slash command | yes | yes |
| claude.ai upload | a zipped skill folder | the whole plugin, or any one skill on its own (`dist:package`) |
| Evals under `claude plugin eval` | 3 cases, graded by a model | 10 cases with scaffolds and transcript graders, **plus** executable checks that build, lint, test and gate what was written, each case proven to fail on an untouched project and pass with a reference solution |
| Rules enforced | none: prose | every rule is a gate, here and on rust-template's `feat/pillars` branch, and every gate is seen failing for its reason |
| Getting loaded | its description only | its descriptions, plus a nudge on any Rust prompt (one run per condition: it turned a crate-writing task that loaded no skill into one that loaded the entry skill first) |
| In-session enforcement | none | opt-in hook runs the gates after each `.rs` edit |
| Commands, agents | none | `/rust-pillars:gate`, `/rust-pillars:adopt`, a refuter agent |
| Code in the skills | examples | every Rust block compiles and runs (`cargo xtask snippets`) |
| Context cost | ~160 tokens always-on, ~7.3k every time it fires (unmeasured: bytes / 4 over the pinned SKILL.md's 650-byte frontmatter and 504-line body) | ~1,034 always-on; ~1.6k for the entry skill, plus only the specialist a task needs (~3.6k-7k) (the tool's estimates: `claude --plugin-dir . plugin details rust-pillars`, claude 2.1.303) |

It costs more context up front -- one description per skill and command --
and less when it works, because no task loads all of it. The idioms are
adapted from BeautifulRust under its MIT license (`NOTICE.md`), with every
departure written down: performance-first design for domain and core,
payload-free errors in a `no_std` core, all primitives refused rather than
only `bool`, `unsafe` forbidden rather than justified. The full comparison,
with sources pinned by hash, is
[the evidence note](docs/research/notes/a-rust-pillars-evidence.md).

## Evals

Ten cases in `evals/`, scored two ways. `claude plugin eval`, run through
`eval-harness/plugin-eval.sh` in the runner's sandbox on an offline export of
HEAD, grades
whether the expected skill fired and whether the code is right. The Rust
harness in `eval-harness/` runs each case headless with and without the
plugin and scores what was left on disk by building, linting, testing and
gating it. From the devenv shell, `devenv tasks run eval:smoke` is for
iterating (the smoke cases, one run, never reported) and
`devenv tasks run eval:full` gives the numbers (every case, three runs, both
arms). Every run makes real model calls on your Claude account. The Rust
harness (`devenv tasks run eval:run`) runs `claude` with
`--permission-mode bypassPermissions` and no sandbox, so it refuses to start
unless `RUST_PILLARS_EVAL_UNSANDBOXED=1` is set: run it only on a tree you
trust. Runner options, reuse and what was measured are in
[eval-harness/README.md](eval-harness/README.md).

## Development

```bash
direnv allow      # or: devenv shell
dev-ci            # every gate, exactly as CI runs it (= devenv test)
```

## Releases

One version for the repository: the workspace version in `Cargo.toml`, which
`.claude-plugin/plugin.json` must match (a check fails when they differ).
[release-plz](https://release-plz.dev) opens a `chore: release` PR from
[Conventional Commits](https://www.conventionalcommits.org/) that bumps the
version and writes [CHANGELOG.md](CHANGELOG.md); merging it tags `vX.Y.Z` from
the `rust-pillars` package and creates a GitHub release. Nothing is published to crates.io -- the plugin
ships as this repository. [CONTRIBUTING.md](CONTRIBUTING.md#releases) has the
details.

## Funding

Solo-maintained, and sponsorship buys maintenance time rather than obligations.
See [FUNDING.md](FUNDING.md) — including how to arrange a commercial licence,
which the NonCommercial clause below makes a separate conversation.

## License

Creative Commons Attribution-NonCommercial-ShareAlike (CC BY-NC-SA) 4.0 International

See [LICENSE](LICENSE) for details.
