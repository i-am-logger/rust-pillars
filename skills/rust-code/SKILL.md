---
name: rust-code
description: "Write robust, fast Rust on six enforced pillars. Use this skill whenever you write, review, refactor, debug, test or explain Rust code: new crates, modules, functions, types, traits, errors, async, unsafe or FFI, Cargo.toml, tests or docs. Trigger on any mention of Rust, cargo, crates, .rs files, the borrow checker, lifetimes, clippy or rustfmt, even when nobody asks for best practice."
---

# rust-code: Rust on six enforced pillars

Every rule here is a gate in projects made from rust-template: the wrong
program fails `devenv test`, not a review. In other projects the same rules
hold as advice until the gates are adopted (`/rust-code:adopt`).

## The pillars

| # | Pillar | In one line | Gate |
|---|---|---|---|
| 1 | Type system | No raw primitive in a domain or core API outside a named seam; no `bool` parameter or field; illegal programs do not compile | `cargo xtask budgets`, trybuild cases with twins |
| 2 | Event-driven | The core takes events and queues effects for the host to poll: no callbacks, no clock, no I/O | `cargo xtask arch`, a chunk-invariance property |
| 3 | Layering | Every crate has a role (`layers.toml`) that says what it may use; features are additive | `cargo xtask layers`, `cargo hack --each-feature` |
| 4 | Zero-copy | Borrow input; no allocation per event; own, don't leak | clippy bans on `Box::leak`/`mem::forget`; an allocation test |
| 5 | `no_std` | Domain and core build with no std | `cargo xtask arch`, bare-metal builds |
| 6 | No `unsafe` | Forbidden; a declared quarantine crate is the only escape | the `forbid` lint |

## Which skill to load

| The task is about | Load |
|---|---|
| a quantity, id, length, timestamp, flag; integer widths; a lifecycle; events and effects as types; a `compile_fail` test | `rust-domain` |
| crates and dependencies, a core state machine, who owns a buffer, `no_std`, `unsafe`, SIMD, caches, GPU hand-off | `rust-architecture` |
| naming, signatures, traits, errors, panics, docs, lint suppressions, concurrency basics -- everyday idiom | `rust-idioms` |
| a claim that something is faster, smaller or starts quicker; a benchmark; embedding data in the binary | `rust-measure` |
| writing or reviewing tests, properties, oracles, gates; a refuter's review | `rust-qa` |
| choosing between designs against a spec; citing a source; a research note or a decision record | `rust-research` |

Load only what the task needs; each specialist loads its references on
demand.

## Four kinds of code

- **Domain and core** -- performance and ownership designed from the start,
  `no_std`, no primitives in the API, payload-free `Copy` errors.
- **Adapters and bins** -- the I/O: std, real error types (`thiserror`), an
  application error with context, still no `unsafe`.
- **A quarantine crate** -- the one place `unsafe` lives, behind safe
  wrappers, with a written soundness argument and Miri.
- **A library for others** -- the Rust API Guidelines in full: `#[non_exhaustive]`,
  sealed traits, semver.

## Before you hand back code

1. In a rust-template project, run `devenv test` -- every gate, as CI runs it
   (or `cargo xtask all` for the source gates alone). Elsewhere:
   `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`,
   `cargo test --all-features`, `cargo doc --no-deps`.
2. A failing gate is fixed in the code. A budget moves only in a commit of
   its own that says why; never raise one to make a gate pass.
3. Every new test was red before the change that made it green; a test
   that cannot fail is not a test.
4. A claim about speed or size has a measurement with an A/A control
   (`cargo xtask arms`, then `cargo xtask bench --aa`); otherwise write
   "unmeasured".
5. A suppression is `#[expect(lint, reason = "...")]` on the narrowest item,
   never `#[allow]`, never `#![deny(warnings)]` in source.
6. No `unwrap()` outside tests; an `expect` says which invariant failed.
7. Every public item has a doc comment; a Rust block in a document compiles
   (`cargo xtask snippets`).
8. Say what was measured, what was only read, and what is inferred.

## Evidence

Each rule names its source or its measured case. The reasoning, the pinned
sources and what this repository's own gates found are in the plugin's
evidence note (`docs/research/notes/a-rust-code-evidence.md` in the
rust-code repository).
