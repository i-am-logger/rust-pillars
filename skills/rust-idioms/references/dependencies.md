# Dependency hygiene

Load this before you add, bump or remove a dependency, or change the edition or MSRV. Which
role may depend on which crate is rust-architecture's subject (`layering.md`, checked against
`layers.toml`). This file covers how a dependency is declared and audited once it is allowed.

## 1. Every dependency is a cost

A dependency is code that you did not write, did not measure and do not review on each
release. Its `build.rs` and its proc macros also run on every developer's machine and in CI at
build time. Before you add one, check these:
- **Does std or core already do it?** A twenty-line function is cheaper than a crate that you
  have to keep auditing.
- **Is it allowed for this role?** In domain and core the answer is almost always no:
  `[roles.core] external = []` (`cargo xtask layers`). Every dependency of the core is code on
  the hot path that rust-measure has not measured.
- **What does it pull in?** Run `cargo tree -p <crate> -e normal,build` before you commit to
  it. Declare it with `default-features = false`, and enable only the features you use (the
  Cargo Book, "Features"). Why: default features are the most common source of an unexpected
  `std` or a duplicate crate.
- **Who publishes it, and is it maintained?** `qa:advisories` reports unmaintained crates that
  have RustSec notices. Otherwise this is review.

## 2. Version requirements

- **Use caret ranges at the lowest version you actually need** (`"1"`, `"1.4"`). Why: Cargo
  can then unify one copy for everyone in the graph, and the lower bound records what you
  depend on (the Cargo Book, "Specifying Dependencies").
- **Never use `"*"`.** Why: it accepts a future major version that breaks your build. Gate:
  `deny.toml` `[bans] wildcards = "deny"` (`test:deny`). crates.io also rejects wildcard
  requirements when you publish.
- **Do not pin with `"="` unless two crates release in lockstep** (a proc-macro crate and its
  runtime crate). Why: an exact pin blocks unification and stops security fixes from reaching
  you. Gate: review.
- **Declare each version once, in `[workspace.dependencies]`.** Members write
  `foo = { workspace = true }`, and may add `features = [...]`. Why: one place to bump, and no
  two members drift onto different versions. The template already declares its own crates
  there (`Cargo.toml`). Gate: review.

## 3. The lock file

- **Commit `Cargo.lock` for anything that builds a binary.** Why: the build that CI tested is
  then the build that ships, and a dependency's new release cannot change it overnight. The
  template commits it, and its `.gitignore` says so.
- **For a library for others, the Cargo Book leaves the lock file to you** (its FAQ, "Why have
  Cargo.lock in version control?"). The template's `.gitignore` comment says to ignore it for a
  library. If you commit it anyway, also test against the newest versions (`cargo update`, then
  build) so that you see the breakage your users will see.

## 4. Audit: `cargo deny` and `cargo tree`

`deny.toml` says what may enter the graph. Two tasks check it.

| Task | Checks | Network | In `devenv test` |
|---|---|---|---|
| `test:deny` | `licenses` (allow list), `bans` (wildcards, duplicate versions), `sources` (crates.io only; unknown registries and git sources denied) | no | yes |
| `qa:advisories` | `advisories`: the RustSec database, covering vulnerabilities, unmaintained crates and yanked versions | yes | no; run it by hand or on a schedule |

- **A new licence is a reviewed change to `[licenses] allow`.** Add it in its own commit, which
  names the crate that needs it. Why: a licence decision should be visible in history.
- **A git or path dependency from outside the workspace fails `sources`.** Publish the crate,
  or vendor it as a workspace member. Why: a git source bypasses the registry and its checksums,
  so it can be force-pushed or deleted. crates.io accepts one only alongside a `version`.
- **Fix an advisory by upgrading.** If no fixed version exists, add an `[advisories] ignore`
  entry with the advisory id and a reason, and record the decision (rust-research). Never ignore
  one silently.
- **Find duplicates with `cargo tree --duplicates` (`-d`).** Then run `cargo tree --invert foo@<old version>`
  to see who pulls in the old copy, and bump it or ask its maintainer. Why: two versions of a
  crate double its compile time and binary size, and their types do not match each other.
  Gate: `multiple-versions = "warn"`, and `test:deny` runs at `--log-level error`, so
  duplicates are printed but do not fail the gate. Reading them is review.
- **Remove unused dependencies.** Run `cargo machete` (stable; it reads the source) before you
  hand back a change that removed code. `cargo udeps` is more precise but needs a nightly
  toolchain, which the template does not install. rustc's `unused_crate_dependencies` lint is
  allow by default, and is noisy across test targets. Gate: review. None of these is in the
  template.

## 5. Edition and MSRV

- **Use the newest stable edition for a new crate, and set it once in `[workspace.package]`.**
  The template sets `edition = "2024"`. Why: one edition across the workspace means one set of
  language rules. Gate: review.
- **Set `rust-version` deliberately, to the newest compiler version that anything in the tree
  needs.** Measure it with `cargo msrv find`, or check it with `cargo hack check --rust-version`.
  Why: with resolver 3 (the default under edition 2024, and set explicitly in the template),
  Cargo prefers dependency versions whose own `rust-version` fits yours (the Cargo Book,
  "The `rust-version` field" and "Resolver versions"). So a `rust-version` that is too low
  silently holds dependencies back, and one that is too high breaks users with older compilers.
  The comment on `rust-version` in the template's `Cargo.toml` explains the same thing.
- **Raise the MSRV in its own commit, and say why.** For a library for others, an MSRV raise is
  a semver-relevant change for many users. Gate: review.

## 6. Two things never to depend on

- **No code before `main`.** Do not use `ctor`, `inventory`-style registration, or any crate
  that runs code from a link section at load time. Why: the order is unspecified, a panic there
  cannot be caught or reported, and the code runs in every test binary and every library user.
  It is also hidden startup cost that rust-measure's startup marks cannot see. Gate: review.
  To make it a gate, add `deny = [{ crate = "ctor", reason = "..." }]` under `[bans]` in
  `deny.toml` (accepted by the template's cargo-deny 0.19).
- **No nightly-only features on the stable toolchain.** Never write `#![feature(...)]`, and never set
  `RUSTC_BOOTSTRAP` to get one. Why: a nightly feature can change or disappear in any release,
  and stable users cannot build your code. Gate: the shell's toolchain is
  `channel = "stable"` (`nix/rust.nix`), where `#![feature]` is error E0554. A dependency that
  needs nightly fails the same way.
