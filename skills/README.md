# rust-pillars

rust-pillars is a Claude Code plugin that teaches Claude how to write Rust. It
follows the style of BeautifulRust (morganlinton, MIT, commit 6b3a96c), with
two differences. First, its rules are enforced by the gates of the companion
template rust-template (`devenv test`, the same tasks CI runs), not just
written down. The gates are on rust-template's `feat/pillars` branch, not its
master; the table below was checked against that branch's head at commit
`a076f93`, and the branch can move on. Second, it is split into six skills whose short SKILL.md files load
reference files only when a task needs them. Each rule names its reason and the
gate that holds it, or says "review" when no gate does. Where a rule leaves
BeautifulRust or a guideline, it says "DEPARTS from <source>: <why>". Every
fact carries a label: measured, by-reading or inferred.

## The six pillars

| # | Pillar | Skill | Gates in rust-template |
|---|---|---|---|
| 1 | A domain type system that refuses raw primitives | rust-domain | `cargo xtask budgets` (`primitive_api`, `seam`, `pub_newtype_field`, `bool_param_or_field`, `as_cast`); trybuild `tests/ui/{fail,pass}` in `test:unit`, twins checked by `cargo xtask docs` |
| 2 | Event-driven, sans-IO cores | rust-architecture (`sans-io.md`) | `cargo xtask arch` (no behaviour-trait bounds in core, a `chunk_invariance_` test); the crate-local `clippy.toml` bans `Instant`/`SystemTime`/`Thread`; budgets `callback_param` |
| 3 | Layered architecture with crate roles | rust-architecture (`layering.md`) | `cargo xtask layers` (`layers.toml`); `cargo xtask arch` (`[lints] workspace = true`) |
| 4 | Zero-copy, no leaks, cache-aware layout | rust-architecture (`zero-copy.md`, `caches.md`) | clippy `disallowed_methods` (leak bans under `std::`/`alloc::`/`core::`); `crates/<name>-core/tests/alloc.rs` (dhat); `const` size asserts; the layout itself is checked in review and with rust-measure |
| 5 | `no_std` | rust-architecture (`no-std.md`) | `cargo xtask arch`; `test:bare` (`x86_64-unknown-none`, `thumbv7em-none-eabihf`); `test:features` |
| 6 | No `unsafe` | rust-architecture (`no-unsafe.md`) | `unsafe_code = "forbid"` in `[workspace.lints.rust]`; budgets `unsafe_code`; a declared `quarantine` role is the only escape |

## The method skills

| Skill | Covers | Gates |
|---|---|---|
| rust-research | standard first, pinned sources, notes, decisions (D-n), claim labels, the judge protocol, records, the closed refuter brief and the one verdict schema | `cargo xtask sources` (`test:sources`), `cargo xtask docs` (`test:docs`) |
| rust-measure | A/B with an A/A arm, instruments, startup marks, embedding precomputed data in the binary, the two invariants, CI work counters | `cargo xtask arms`, `cargo xtask bench`, `crates/<name>-core/tests/alloc.rs`, `MEASUREMENTS.md` |
| rust-qa | test placement, independent models, gates that can fail, mutation testing, the code-side refuter rules R1-R8 | `cargo xtask tests`, `cargo xtask coverage`, `cargo xtask controls`, `test:mutants-diff`, `test:unit` under `[profile.qa]` |
| rust-idioms | everyday idiom adapted from BeautifulRust and scoped per crate role; errors, suppressions, naming, docs | `test:clippy` (`-D warnings`, `allow_attributes`), budgets `expect_attr`, `test:doc` |

Every skill stands alone: none needs a skill this plugin does not ship, and
none links outside its own folder. The `rust-pillars` skill fires on any Rust
work and routes to the one the task needs.

## Which skill owns what

Each rule lives in one skill, and the others point to it:
- the hand-back checklist and which skill to load: rust-pillars
- domain types: rust-domain
- layering, sans-IO, zero-copy, caches, `no_std` and `unsafe`: rust-architecture
- error types, lint suppression and general idiom: rust-idioms
- measurement and the performance invariants: rust-measure
- tests and the code-side refuter: rust-qa
- research, judges, records and the verdict schema: rust-research

A "planned" gate is in the rust-pillars plan but not yet in the template. Never
cite one as passing.
