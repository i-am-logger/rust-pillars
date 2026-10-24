# Layering: crate roles and the dependency direction

Load this when you add a crate, add a dependency, move code between crates, add
a feature, or a `cargo xtask layers` or `cargo xtask arch` gate fails.

Evidence labels: **measured** (a number someone measured), **by-reading** (the
code or notes say it), **inferred** (a prediction nobody has measured).

## Roles (`layers.toml`)

Every workspace member has exactly one role, in `[crates]`. The role decides
what the crate may depend on and which gates apply to it.

| Role | Is | May depend on (`may_depend_on`) | External crates (`external`) |
|---|---|---|---|
| `domain` | the type system: no_std, no I/O, no primitives in its API | nothing | `["zerocopy"]` (derive-only, no default features) |
| `core` | the machine: no_std, sans-IO | `domain`, `quarantine` (its safe wrappers) | `[]` |
| `adapter` | I/O, platform, GPU; turns the outside world into events | `domain`, `core`, `adapter`, `quarantine` | `["*"]` |
| `bin` | a binary: wires adapters to the core | `domain`, `core`, `adapter`, `quarantine` | `["*"]` |
| `quarantine` | the only home of `unsafe`; exposes safe wrappers | `domain`, `quarantine` | `["*"]` |
| `tool` | build and gate tooling (`xtask`) | nothing | `["*"]` |
| `harness` | benchmarks and differential tests | every role | `["*"]` |

The direction is **domain → core → adapter → bin**. A quarantine sits beside
the core: the core, adapters and bins call its safe wrappers, and it may
depend on the domain and on other quarantine crates (a per-arch backend on its
trait crate). A tool or a harness is never a dependency of shipped code.
Why the core may reach a quarantine: a core calling a per-arch backend's safe
wrappers is the intended SIMD pattern, and a rule letting the core use only
the domain would forbid it. Benchmarks and differential tests use everything
and are used by nothing, so no shipped role fits them: hence `harness`.

- The core reaches zerocopy only through the domain's types. It does not list
  zerocopy itself.
- Changing an allowlist is a reviewed change: edit `layers.toml` in a commit of
  its own that says why. Two cases where this is needed:
  - letting the core reach a quarantine SIMD backend
    ([simd.md](simd.md#where-the-backend-lives))
  - adding any crate to a pure role's `external`. Never `self_cell`: its
    macro expands `unsafe` that `forbid` and `budgets` cannot see
    ([no-unsafe.md](no-unsafe.md#macro-expanded-unsafe)), so it stays in
    adapters and bins ([zero-copy.md](zero-copy.md#own-dont-leak)).

## What each gate checks

- **`cargo xtask layers`**
  - Reads `cargo metadata`.
  - Checks every normal and build dependency edge against `may_depend_on`.
  - Checks every external crate against `external`.
  - Dev-dependencies are not checked, because a test may use what the code may
    not. The template's model test uses `std` and proptest.
- **`cargo xtask arch`**
  - Every member has a role, and every crate named in `layers.toml` is a
    member.
  - Every crate except a quarantine sets `[lints] workspace = true`.
  - Domain and core:
    - `#![no_std]` in `src/lib.rs`
    - no `alloc`/`std` in `default`
    - a crate-local `clippy.toml` that contains every root ban plus
      `std::time::Instant` and `std::time::SystemTime`
  - Core:
    - no callback in a public signature in `src/` (`fn(..)`, or a behaviour
      trait in `impl`/`dyn`/a method generic)
    - a `chunk_invariance_*` test in `tests/`
- **`cargo xtask coverage`**: every pub fn of domain and core is called by
  some test.
- **`cargo xtask tests`**: no skipped tests, exact floors, every `#[ignore]`
  listed in `ignores.toml`.
- **`cargo xtask budgets`**
  - Domain and core are held to all eight counted kinds: `primitive_api`,
    `seam`, `pub_newtype_field`, `bool_param_or_field`, `as_cast`,
    `expect_attr`, `unsafe_code` and `callback_param`.
  - Every other crate is held to `as_cast`, `expect_attr` and `unsafe_code`.
  - A budget is the count exactly: over fails, and under fails too.
  - `cargo xtask budgets --print` prints the current counts.
- **`test:features`** (`cargo hack check --workspace --each-feature
  --no-dev-deps`): every feature builds on its own.
- Each xtask gate runs a positive control first: it proves its detector fires on
  known-bad input before it trusts a pass.

## Rules

1. **The domain is its own crate.**
   - WHY: privacy is enforced at the crate boundary. A `pub(crate)` newtype
     field is sealed only from other crates, so inside one crate a module
     boundary is advisory.
   - Gate: the crate edge itself (`cargo xtask layers` checks it points one
     way); `cargo xtask budgets` (`pub_newtype_field`) keeps the fields sealed.
2. **The core depends on the domain and nothing else.**
   - WHY: every dependency of the core is code on the hot path that you did not
     measure, a possible `std` or `unsafe` import, and a compile-time cost on
     every build.
   - Gate: `cargo xtask layers`.
3. **Adapters turn the world into events. The bin wires them together.**
   - Clocks, files, sockets, GPU devices, OS geometry probes and async runtimes
     live here.
   - Adapters and the bin are still under `unsafe_code = "forbid"`. Platform
     APIs go through safe binding crates or the quarantine
     ([no-unsafe.md](no-unsafe.md)).
4. **The bin is the composition root.**
   - It wires adapters to the core. The template has no adapter yet, so its
     `main.rs` reads the clock and stdin itself; once an adapter exists, the
     I/O moves there.
   - It holds the single named process owner, if one exists
     ([zero-copy.md](zero-copy.md#own-dont-leak)).
   - WHY: decisions that live in the core can be tested without a terminal, a
     clock or a file.
5. **Features are additive.**
   - Turning on a feature never removes an item or changes behaviour that
     another feature relies on. The Cargo reference ("Features": feature
     unification) explains why: features unify across the graph, so a
     subtractive feature breaks some crate you never built together with it.
   - `alloc`/`std` features are off by default in pure roles
     (`cargo xtask arch`).
   - Gate: `test:features`.
6. **Measurement arms are not features.**
   - Ablation arms (ceiling, probe-double) are `cfg(arm = "...")`, declared with
     check-cfg in `[workspace.lints.rust]`:
     `unexpected_cfgs = { level = "deny", check-cfg = ['cfg(arm, values("ceiling"))'] }`.
     A crate-level `[lints.rust]` beside `[lints] workspace = true` is a Cargo
     error ("cannot override `workspace.lints`", measured here). With the
     workspace entry, a misspelt `cfg(arm = "cieling")` fails the build
     (measured here).
   - WHY: ceiling and probe arms are not additive by definition, so as Cargo
     features they make `--all-features` fail under `-D warnings`. That makes
     `cargo clippy --all-features` unusable as a gate.
   - Review: this is a planned gate (not in the template yet).
7. **Per-architecture code is a leaf.**
   - Put each ISA behind a trait, with one leaf crate per architecture.
   - In the template, an ISA backend that needs `unsafe` is a quarantine crate,
     and portable code stays in the core ([simd.md](simd.md)).
8. **One layout function serves every allocator.**
   - One function decides row pitch for GPU, mmap and heap allocations alike,
     so no two allocators disagree about a row.
   - Put such a function in the domain or core as a `const fn` over domain
     types. The template-style `row_pitch` is in [caches.md](caches.md).

## Adding a crate

1. Create `crates/<name>/` with `[lints] workspace = true` and `*.workspace = true`
   package fields (`edition = "2024"`).
2. Add `<name> = "<role>"` under `[crates]` in `layers.toml`.
3. If the crate is domain or core:
   - add `#![no_std]`
   - copy the core's `clippy.toml`
   - keep `default = []`
   - nothing to add to `test:bare` or `test:mutants-diff`: both take their
     crate list from `cargo xtask crates --role domain,core`, which reads
     `layers.toml`
4. Add `[crates.<name>]` to `budgets.toml`. For a domain or core crate,
   `primitive_api`, `pub_newtype_field` and `bool_param_or_field` stay 0
   (pillar 1): fix the code, never paste those counts from
   `cargo xtask budgets --print`. Other kinds take the printed count; raise a
   budget only in a commit that says why.
5. Run `devenv test`. Every gate must be green.

## Fixing a layering violation

- **Core imports from an adapter or the bin:**
  - Move the type the core needs down into the domain.
  - Or invert the flow: make the adapter send an event, and have the core queue
    an effect.
  - Never add the edge.
- **Domain or core needs an external crate:**
  - First ask whether `core::` already has it.
  - Then whether the type can be your own in the domain.
  - Only then propose an allowlist entry, with what it costs: `unsafe` inside
    it, `std` features, its size.
- **A `use std::` slips into a pure crate:** `test:bare` fails at the build.
  Replace it with `core::`/`alloc::` ([no-std.md](no-std.md)).
- **A layering written in docs but not checked drifts.** `cargo xtask layers`
  is the check, and `[workspace.lints]` with `allow_attributes = "deny"` and
  the counted `expect_attr` keep suppressions from piling up unseen.
