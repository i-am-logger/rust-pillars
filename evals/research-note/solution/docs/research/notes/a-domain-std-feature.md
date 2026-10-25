# A. Should the domain crate get an optional `std` feature for its `std::error::Error` impls?

**Status: decided, 2026-10-24.** Nothing is built: the domain crate has no
error type yet, and when it gets one it implements `core::error::Error` with
no feature. Every number is measured unless it says inferred. The decision is
D1 in [decisions.md](../../decisions.md).

## The answer, in short

- **Allowed, but not needed. Decided: no `std` feature.** A `std` feature
  that is off by default and only *adds* items is CONFORMING to the Cargo Book
  (by-reading, [[source: cargo-features]] "Feature unification"), and this
  workspace's gates would accept it (by-reading, `xtask/src/arch.rs` `pure()`
  rejects only `alloc`/`std` in `default`).
- But the error impls need no feature. A `#![no_std]` crate can implement
  `core::error::Error`, and a host can then use that type with `?` into
  `Box<dyn std::error::Error>`. Measured on rustc 1.94.1, in a scratch crate
  (see "How it was checked").
- At this workspace's MSRV, 1.85, this is **inferred, unpinned**:
  `core::error::Error` was stabilised in Rust 1.81. That version is my memory
  of the release notes, which are not pinned here. No 1.85 toolchain was run.
- What is never allowed: a `no_std` feature (the Cargo Book says **do not**),
  or `std` in `default` (the `arch` gate fails it, pillar 5).
- The domain crate has no error types today (by-reading, with a positive
  control), so nothing is built now.

## The governing text

The Cargo Book, features chapter, pinned at cargo commit `0f95863`
([file](../sources/cargo-0f95863-features.md), sha256 `7dde8d4e...c9d2`). All
quotes are verbatim, except that Markdown link markup is dropped.

[[source: cargo-features]] "Feature unification", lines 284-291:

> A consequence of this is that features should be *additive*. That is, enabling
> a feature should not disable functionality, and it should usually be safe to
> enable any combination of features. A feature should not introduce a
> SemVer-incompatible change.
>
> For example, if you want to optionally support `no_std` environments, **do
> not** use a `no_std` feature. Instead, use a `std` feature that *enables*
> `std`.

It follows with the shape it means: `#![no_std]`, then
`#[cfg(feature = "std")] extern crate std;`, then items gated on
`#[cfg(feature = "std")]` (lines 293-302).

[[source: cargo-features]] "The `default` feature", the IMPORTANT box (lines 103-116), lines 107-110:

> Dependencies automatically enable default
> features unless `default-features = false` is specified. This can make it
> difficult to ensure that the default features are not enabled, especially
> for a dependency that appears multiple times in the dependency graph.

and lines 114-115:

> it can be a SemVer incompatible
> change to remove a feature from the default set

[[source: cargo-features]] "SemVer compatibility", lines 470-472. These should
usually **not** be done in a minor release: "Remove a feature or optional
dependency" and "Moving existing public code behind a feature".

[[source: cargo-features]] "Feature combinations", line 535: features "require
an exponential number of configurations and test cases to be 100% covered".

### The workspace's own rules (by-reading, this tree)

| rule | where | says |
|---|---|---|
| no_std | `crates/rust-code-domain/src/lib.rs` line 25 | `#![no_std]` |
| features | `crates/rust-code-domain/Cargo.toml` lines 12-15 | `default = []`, `alloc = []`; "it is never on by default, and `test:bare` builds with it off" |
| arch gate | `xtask/src/arch.rs` `pure()`, lines 105-130 | fails if `lib.rs` lacks `#![no_std]`, or if `default` names anything containing `alloc` or `std` |
| layering | `layers.toml` `[roles.domain]`, lines 21-24 | no internal dependency; the only external one is `zerocopy` |
| additive | `CLAUDE.md` pillar 3 | "Features are additive." |
| bare build | `devenv.nix` `test:bare` (line 274) | domain and core build with `--no-default-features` for a bare-metal target |
| each feature | `devenv.nix` `test:features` (line 289) | `cargo hack check --workspace --each-feature`, so every feature must build on its own |
| MSRV | `Cargo.toml` lines 12 and 16 | `edition = "2024"`, `rust-version = "1.85"` |

## Practice

Not surveyed: no other implementation is pinned under
`docs/research/sources/`, and this question does not need one. A common
pattern is a `std` feature that gates `impl std::error::Error`, often in
`default`. That is **inferred, unpinned**, from memory, and it carries no
weight here. If it is ever recorded, `std` in `default` should be marked
DEPARTS from this workspace's pillar 5. It does not depart from the Cargo
Book, which allows a default set but warns about it (lines 103-116).

## Options

| | option | vs Cargo Book | vs workspace | cost |
|---|---|---|---|---|
| 1 | Implement `core::error::Error` (and `core::fmt::Display`) on each domain error type, with **no feature** | **CONFORMING**: no feature, so nothing to unify or combine | **CONFORMING**: stays `#![no_std]`; no new dependency; nothing for `arch`, `test:bare` or `test:features` to see | Needs `core::error::Error` at the MSRV. Inferred to be stable since 1.81, and edition 2024 already forces at least 1.85 (measured, below). Hosts get `?` and `Box<dyn Error>` with no flag (measured on 1.94.1). |
| 2 | Optional `std` feature, `default = []`, `#[cfg(feature = "std")] extern crate std;`, `impl std::error::Error` behind it | **CONFORMING**: this is the shape on lines 289-302; it only adds impls | **CONFORMING** with the gates: `arch` checks only `default`; `test:bare` builds without it; `test:features` builds it alone | One more cfg arm to build and test, which is the exponential cost on line 535. With `std` on, the domain crate can name `std::` paths that its `clippy.toml` bans exist to catch. It buys nothing over option 1. Under SemVer it is a promise: once published, removing the feature is a break (lines 470-471). |
| 3 | A `no_std` feature that turns std *off* | **DEPARTS** from [[source: cargo-features]] "Feature unification", lines 289-290: "**do not** use a `no_std` feature". It is subtractive: Cargo builds a dependency with "the union of all features" (line 268), so one dependent turning it on turns std off for every other dependent (inferred from that line). | **DEPARTS**: the crate would no longer be `#![no_std]` unconditionally, which fails `arch` | Rejected. |
| 4 | Option 2, with `std` in `default` | CONFORMING to the text (a default set is allowed), against the IMPORTANT warning, lines 103-116 | **DEPARTS** from pillar 5: `arch` fails "`default` turns on `std`" | It fails a gate. Every dependent would need `default-features = false` to stay bare. Removing it later is SemVer-incompatible (lines 114-115). Rejected. |

**Decision: option 1.** Option 2 is allowed but redundant. Options 3 and 4
are not allowed. It reverses if a domain error needs something only `std` has
(a `std::io::Error` source, say). That would be I/O in the domain, which
pillar 2 forbids, so it would be a design question before it is a feature
question. It also reverses if the MSRV drops below the version that
stabilised `core::error::Error`, which edition 2024 rules out.

## How it was checked

- **Pinned source.** Read from `docs/research/sources/cargo-0f95863-features.md`.
  `shasum -a 256` gives
  `7dde8d4ec8faa1ab9fc06f185d88dada09bbfa5924a05f2fd303cd620a54c9d2`, the
  hash in MANIFEST.toml. It is plain Markdown, so it was read whole and
  searched with `grep -n`. Positive control: `grep -n '^## '` hit every
  section heading, including "Feature unification" at line 263.
- **No error types in the domain crate (by-reading).** Ran
  `grep -rnE 'Error|Display' crates/rust-code-domain/src`: 0 hits. Positive
  control, same method: `grep -rn ByteClass` hits `class.rs` 15 times and
  `line.rs` 11 times. So the miss is an absence, not a broken search.
- **The no-feature design works on a host (measured; rustc and cargo 1.94.1
  from the devenv shell; macOS host).** A scratch workspace outside the repo,
  edition 2024, `rust-version = "1.85"`:
  - crate `dom` is `#![no_std]`, `default = []`, and has
    `impl core::error::Error for Overflow {}`;
  - `cargo build -p dom --target x86_64-unknown-none` succeeds;
  - a host bin with `fn run() -> Result<u32, Box<dyn std::error::Error>>`
    uses `dom::check(n)?` and coerces `&dom::Overflow` to
    `&dyn std::error::Error`. It builds and runs: `run(99)` printed
    `Err("line too long")`.
- **The MSRV floor (measured).** In the same scratch workspace, edition 2024
  with `rust-version` 1.80 or 1.81 makes cargo refuse to load the manifest:
  "rust-version 1.80 is incompatible with the version (1.85.0) required by
  the specified edition (2024)". So this workspace cannot declare an MSRV
  below 1.85.
- **What did NOT verify the 1.81 claim (measured; a negative result).**
  `clippy::incompatible_msrv` was silent on the trait impl at
  `rust-version = "1.80"` (edition 2021). Positive control: the same lint
  fired on `<[u8]>::is_sorted` ("this item is stable since `1.82.0`"). So the
  lint does not see trait-impl paths, and its silence says nothing about
  `core::error::Error`. The 1.81 claim stays inferred.
- **Gates.** This note changes no code, manifest or gate config.
  `cargo xtask sources` was run after writing it and passes: its only
  citation key is `cargo-features`.

## Not read

- The Rust release notes and the `core::error::Error` docs. These are the
  source for "stable since 1.81". Unpinned; the claim is inferred.
- The Cargo Book's resolver chapter and its SemVer chapter. They are only
  linked from the pinned page (lines 271, 460) and are not pinned here.
- The Rust Reference on `no_std`: linked at line 305, not pinned.
- Practice in other crates: none pinned (see Practice).
- A real 1.85 toolchain: none installed (no rustup), so nothing ran at the
  MSRV itself.

## Open

- Which error types the domain will have. The first one gets
  `impl core::error::Error` and a `core::fmt::Display` impl, with no feature.
  Closes when that type lands with a test that turns it into a
  `Box<dyn std::error::Error>` on the host.
- The 1.81 stabilisation version. Closes by pinning the Rust 1.81 release
  notes, or by building the scratch crate on a 1.85 toolchain. Either can only
  confirm option 1, because edition 2024 already needs 1.85.
