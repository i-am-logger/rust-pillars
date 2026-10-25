# A. Where the rust-code rules come from

**Status: research, 2026-10-24.** The rules are written as the plugin's six
skills (`skills/`) and enforced by rust-template's gates; this note is their
evidence. Every number below is labelled: **measured** (by whoever is named,
on their machine), **by reading** (code or text read, nothing run), or
**inferred** (an argument, not an observation).

## The answer, in short

BeautifulRust covers idiom and API hygiene and little else, so rust-code
keeps what it has, adds what it lacks, and enforces both:

- From **BeautifulRust** (a Claude Code plugin whose one skill is idiom and
  API hygiene drawn from five guides, [[source: beautifulrust-skill]]): the
  idioms -- naming, borrowing, standard traits, errors, visibility, docs --
  kept almost whole in `rust-idioms`, with the departures below stated.
- **Added by rust-code:** the architecture -- a domain type system that
  refuses primitives, a sans-IO core, layering, zero-copy, caches, build-time
  tables -- and the method: standard first, pinned sources, measurement with
  an A/A control, gates with positive controls, records with retractions.
- **Enforced:** every rule is a gate in rust-template, every gate is seen
  failing for its planted reason (`cargo xtask controls`), and the plugin's
  evals check the generated code by building and gating it, not only by
  asking a model.

## The governing texts

| Rule | Text |
|---|---|
| Newtypes for static distinctions | [[source: rust-api-guidelines-checklist]] C-NEWTYPE; [[source: rust-api-guidelines-type-safety]] C-NEWTYPE |
| Arguments convey meaning through types, not `bool` | [[source: rust-api-guidelines-checklist]] C-CUSTOM-TYPE |
| Newtypes encapsulate their inside | [[source: rust-api-guidelines-checklist]] C-NEWTYPE-HIDE |
| Features are additive | [[source: cargo-features]] "Feature unification" |
| Lints configured in the manifest | [[source: cargo-manifest]] "The `[lints]` section" |
| Banned methods and types | [[source: clippy-lint-configuration]] `disallowed-methods`, `disallowed-types` |

## BeautifulRust, rule by rule

**CONFORMING (kept in `rust-idioms`):** write Rust-shaped code; invalid
states unrepresentable; `#[expect(lint, reason)]` over `#[allow]`; lints in
`[lints]`; never `#![deny(warnings)]` in source; borrow the most general
type; don't clone to quiet the borrow checker; naming conventions; derive
the standard traits; builders; private by default; `#[non_exhaustive]`;
sealed traits; one path per item, no glob re-exports; library errors that
implement `Error` and never `String`; `# Errors`/`# Panics`; tests of
properties, not tautologies.

**DEPARTS:**

- *"Design for usability first and optimize after measuring."* For domain and
  core, layout and ownership are designed for performance from the start and
  every change is then measured: a hot struct's layout is costly to change
  later, and conforming to the governing text comes first, with speed
  achieved inside it. Usability-first still holds for adapters and bins.
- *Errors.* A `no_std` core without an allocator cannot box an error;
  payload-free `Copy` error enums there, BeautifulRust's rules everywhere
  with `std`.
- *No `bool` parameters* becomes the general rule: no raw primitive in a
  domain or core public API outside a named seam.
- *`unsafe` for three reasons* becomes `forbid`, with one declared
  quarantine crate as the only escape.
- *"Don't define a prelude"* is not adopted as a rule: it is an opinion the
  source does not argue.

**What BeautifulRust lacks** (by reading [[source: beautifulrust-skill]]):
no shipped lint table, config or CI; one always-loaded ~500-line file; three
evaluation cases graded by a model and tuned against themselves; nothing on
`no_std`, workspace layering, caches, measurement or research method.

## Three layering and test rules, and why

- **A core may depend on a quarantine crate, and a quarantine crate on
  another.** That is how a core reaches SIMD through safe wrappers, and how
  a per-architecture backend depends on its trait crate. A rule that let a
  core use only the domain would forbid the intended SIMD pattern.
- **Benchmarks and differential tests have their own role, `harness`.**
  They use everything and are used by nothing, so no shipped role fits them.
- **The exact-floor rule flags only literals of 2 or more.** `n > 0` says
  "not empty", a property rather than a count; `n >= 50` over a fixed set of
  60 fixtures is a count that lets 10 vanish unnoticed.

## What this repository's own gates found

**Measured** here, while building the gates (rust-template, 2026-10-24):

- A `disallowed-methods` entry under `std::` bans nothing in a `no_std`
  crate: a planted `alloc::boxed::Box::leak` in the domain passed clippy until
  the ban was listed under `alloc::` too. The planted-violation run found it.
- The first mutation run left 10 mutants alive. One test compared a byte
  table with the very definition it was compiled from, so deleting an arm of
  the definition changed both sides; it now checks against an independent
  definition. After the fixes, 0 of 33 viable mutants survive.
- A planted off-by-one in the core passed the split-equality property, whose
  two sides shared the bug, and failed the property against an independent
  model.
- Two early "red" results were link errors from a broken environment, not
  the gate firing; since then a red counts only if the output names the
  planted reason.
- Writing a reference solution for the measurement eval found a data-loss
  bug in the template's own driver: it handed the core a whole 64 KiB read and
  drained afterwards, so on 44-byte lines the read owed more effects than the
  1024-slot queue holds and 7447 of 23832 lines were missing from the output
  (refused and counted, so visible -- but lost). The driver now feeds each read
  in queue-sized slices; a test is red on the old loop and green on the new
  (rust-template `a076f93`). Refuse-and-count was right in the core; the
  driver's job was not to overflow it.
- **A description is not enough to get a skill loaded.** Measured headless
  (`claude -p`, Sonnet, one run each): asked to write a small semver crate,
  the model wrote it without loading any of the plugin's skills, with the
  entry skill's description phrased either way; with a `UserPromptSubmit`
  hook adding one line ("This is Rust work: load the rust-code skill") the
  same task loaded `rust-code:rust-code` first. A one-line "why does this not
  compile" question was answered without any skill, with or without the
  hook. One run per condition: evidence of the direction, not a rate; the
  eval graders `skill-fired` and `entry-fired` measure the rate.
- Every Rust block in the skills now runs under `cargo xtask snippets`: 29 of
  them in 14 files failed at first, and two more that passed only because
  `rustdoc` compiles `#[cfg(test)]` to nothing were caught once the gate set
  `--cfg test`.

## Not read

Effective Rust (CC BY-NC-ND: cited by name only). The Rustonomicon and the
Rust Reference sections the skills mention by name are not yet pinned.
BeautifulRust's five reference digests were not re-read against their
originals.

## Open

- Settle crate-relative paths in docs: the docs gate reads every backticked
  path from the repository root, so a test path meant relative to one crate
  is reported as missing.
