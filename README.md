# rust-code

[![CI](https://img.shields.io/github/actions/workflow/status/i-am-logger/rust-code/ci.yml?branch=master&label=CI&logo=githubactions&logoColor=white)](https://github.com/i-am-logger/rust-code/actions/workflows/ci.yml)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/i-am-logger/rust-code/badge)](https://scorecard.dev/viewer/?uri=github.com/i-am-logger/rust-code)
[![Nix](https://img.shields.io/badge/Nix-2b2b2b?logo=nixos&logoColor=white)](https://nixos.org)
[![devenv](https://img.shields.io/badge/devenv-2b2b2b?logo=nixos&logoColor=white)](https://devenv.sh)
[![License: CC BY-NC-SA 4.0](https://img.shields.io/badge/CC%20BY--NC--SA%204.0-2b2b2b?logo=creativecommons&logoColor=white)](LICENSE)

> A Claude Code plugin that teaches Claude to write Rust the way these
> projects are built -- a domain type system that refuses primitives,
> event-driven sans-IO cores, layered crates, zero-copy, `no_std`, no
> `unsafe` -- and how to research, measure and test it. In the spirit of
> [BeautifulRust](https://github.com/morganlinton/BeautifulRust), with one
> difference that matters: every rule here is also a gate in
> [rust-template](https://github.com/i-am-logger/rust-template) that fails
> the build.

## What it is

Two halves, versioned separately:

- **This plugin** -- six skills, each a short `SKILL.md` with references
  loaded on demand, plus evals that score the skills by building and gating
  the code they produce.
- **rust-template** -- the enforcement: workspace lints, `cargo xtask`
  gates, devenv tasks that are CI. Projects are generated from it and enable
  this plugin; this repository is itself one of them, and every gate applies
  here too.

## Install

Once the repository is published:

```text
/plugin marketplace add i-am-logger/rust-code
/plugin install rust-code@rust-code
```

A project made from rust-template enables it for everyone working in the
project through its Claude Code project settings. Locally, before publishing:
`claude --plugin-dir ~/Code/rust-code`.

## The skills

| Skill | Load it when | Covers |
|---|---|---|
| `rust-domain` | modelling quantities, IDs, protocol vocabulary, lifecycles | refusing primitives, seams, proof types, typestate, brands, `compile_fail` twins |
| `rust-architecture` | designing crates, a core, ownership, layout, `no_std`, SIMD, GPU hand-off | roles and layering, sans-IO, zero-copy and own-don't-leak, caches, no unsafe |
| `rust-idioms` | everyday API, error, naming and doc decisions | BeautifulRust's idioms, kept or departed from, with reasons |
| `rust-research` | deciding against a standard, citing sources, recording decisions | standard first, pinned sources, notes, the judge protocol, retractions |
| `rust-measure` | claiming anything about speed, memory or startup | A/A arms, one build path, instruments, embedding at build time, CI counters |
| `rust-qa` | writing or reviewing tests and gates | properties against a model, never-skip, exact floors, mutation testing, refuters |

## The pillars and their gates

| # | Pillar | Gate in rust-template |
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

## Against BeautifulRust

Kept, almost whole: the idioms (`rust-idioms`, adapted under BeautifulRust's
MIT license with attribution). Departed from, with the reason written down:
performance-first design for domain and core, payload-free errors in a
`no_std` core, primitives refused rather than only `bool`, `unsafe`
forbidden rather than justified. Added: the architecture and the method,
and enforcement. The full comparison, with sources pinned by hash, is
[the evidence note](docs/research/notes/a-rust-code-evidence.md).

## Evals

Two layers. `claude plugin eval` grades transcripts (did a skill fire, how
was the work done). `evals/harness` runs each case in `evals/` headless
with and without the plugin, then builds, lints, tests and gates what was
left on disk; each arm runs twice and a difference counts only if it is
larger than the arm's own run-to-run spread. `devenv tasks run eval:control`
runs no model: it proves every task-specific check fails on the untouched
base, so no case can grade nothing. Every other run is a paid model call,
so evals are a manual task (`eval:run`). The working copies live outside the
repository, and the arm with the plugin gets a copy of the plugin alone, so
no run can read the checks it is graded by.

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
