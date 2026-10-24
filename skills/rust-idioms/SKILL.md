---
name: rust-idioms
description: Use when writing, reviewing or refactoring any Rust in the owner's projects and the question is everyday idiom rather than architecture. Covers naming, function signatures and borrowing, standard trait impls, constructors and builders, visibility and API evolution, error types and panics, lint suppressions, docs, imports and basic concurrency. Load it before writing a new public item, an error type, a `#[expect]`, a doc comment or a refactor of someone else's code, and whenever a reviewer asks "is this idiomatic?". It scopes each rule to the kind of code involved (domain, no_std core, std adapter/bin, library for others). It defers to rust-domain for quantity types and primitive refusal, to rust-architecture for layering, zero-copy and unsafe, and to rust-qa for tests.
---

# rust-idioms

Everyday Rust idiom for every project built from rust-template. Most rules are enforced by
`devenv test`, which runs the same gates as CI. A rule that names no gate, or says "review",
is enforced by review only.

Adapted from BeautifulRust by morganlinton (MIT, commit 6b3a96c). What was kept, changed and
dropped is in [references/beautifulrust-delta.md](references/beautifulrust-delta.md).

## First: what kind of code is this?

Every crate has a role in `layers.toml`. The role decides which version of a rule applies.

| Kind | Role | Optimise for | Errors | Allocation |
|---|---|---|---|---|
| Domain | `domain` | invariants that cannot be forged | `Option`, or a `Copy` enum with no payload | none (`no_std`) |
| Core (hot) | `core` | performance designed in, then measured | refuse and count; `Copy` enum if a `Result` is needed | none per event |
| Adapter / bin | `adapter`, `bin`, `tool` | clarity first, optimise after measuring | `thiserror` enum (adapter/bin), `anyhow` + context (tool) | free, outside loops |
| Library for others | any crate published for other people | semver stability, the full Rust API Guidelines | `Error + Send + Sync + 'static` | caller decides |

The "Optimise for" column DEPARTS from BeautifulRust's "usability first, optimise after
measuring" for domain and core; the reason is rust-architecture's ("Performance is a design
requirement"), the measuring rust-measure's.

## Before handing back

- Run `devenv test` before handing back. Why: CI runs exactly this; nothing else counts as green. While iterating, `test:fmt`, `test:clippy`, `test:unit` and `test:doc` are the fast subset, never the hand-back.
- Fix the lint; do not silence it. Why: a silenced lint hides the next instance too. Gate: `clippy::all` is deny, with `-D warnings`.
- To suppress, use `#[expect(lint, reason = "...")]` on the narrowest item, never `#[allow]`. Why: an `expect` fails once the lint stops firing (`unfulfilled_lint_expectations`). Gate: clippy `allow_attributes` and `allow_attributes_without_reason`.
- Treat every `#[expect]` as a budget change. Why: `expect_attr` is counted in every crate, and its budget is 0 unless `budgets.toml` sets one. Gate: `cargo xtask budgets`; a raise goes in its own commit that says why.
- Name only lints that are enabled and firing in an `#[expect]`. Why: expecting a pedantic lint that is off fails the build as unfulfilled.
- Never write `#![deny(warnings)]` in source (review). Lints live in `[workspace.lints]`, and every crate sets `[lints] workspace = true`. Why: a new compiler's new warning should not break downstream builds, and one table is one policy. Gate: `cargo xtask arch` checks `[lints] workspace = true`.
- Replace `as` with `From`/`TryFrom` (`usize::from(b)`, `u64::try_from(x).unwrap_or(u64::MAX)`). Why: `as` truncates silently. The exception is a `const fn`, where trait conversions are not callable: there, one `as` with a comment proving it exact (`BytePos::saturating_advance`). Gate: `as_cast` budget, `cargo xtask budgets`.

## Signatures and ownership — [ownership.md](references/ownership.md)

- Accept the most general borrowed form: `&str`, `&[T]`, a borrowed domain wrapper (`Wire<'a>`). Why: every caller can pass one without converting or allocating. Gate: clippy `ptr_arg` (`&Vec`, `&String`).
- Take ownership only to store or consume. Why: it lets the caller decide where data lives (C-CALLER-CONTROL).
- Return values and tuples, not out-parameters (C-NO-OUT). Why: the signature then says what comes out. The exception is a caller-lent buffer on a hot path (rust-architecture).
- No `Rc`/`Arc`/`Box`/`RefCell` in a public signature. Why: it fixes the caller's ownership model. Gate: review.
- Do not clone to satisfy the borrow checker; climb the ladder in ownership.md first. Why: the clone hides a design question and costs a copy on every call. Gate: clippy `clone_on_copy`; otherwise review.
- Give lifetimes descriptive names (`'src`, `'buf`) when there are two or more, and elide them otherwise. Why: an error then names which borrow is wrong. Gate: clippy `needless_lifetimes`.
- No leaks (`Box::leak`, `mem::forget`, ...): hold the value in an owner. Gate: clippy `disallowed_methods`. The owner order and why: rust-architecture (zero-copy).

## Types and traits — [api-design.md](references/api-design.md)

- In domain and core, no raw primitive in a public signature outside a named seam, and no `bool` parameter or field at all; a predicate still returns `bool`. Gate: `cargo xtask budgets`. The rules and their DEPARTS: rust-domain.
- Derive `Debug` on every public type, and write it by hand to redact secrets. Why: an undebuggable type blocks every caller's `Debug`. Gate: `missing_debug_implementations`.
- Derive the other common traits per type, by meaning: a derive is a promise. For domain quantities (no `Default`, `Hash` or `From<int>` unless the meaning needs one, and the DEPARTS from "derive eagerly"): rust-domain.
- `Copy` only for small types that are plain values. Why: copying a large type is a hidden memcpy at every use.
- Implement `From`/`TryFrom`, never `Into`. Why: `From` gives `Into` for free. An invariant-carrying type gets only `TryFrom` or a fallible constructor. Gate: clippy `from_over_into`.
- Implement `Deref` only for smart pointers (C-DEREF), and operators only where they keep their mathematical meaning (C-OVERLOAD). Why: both are invisible at the call site. Gate: clippy `should_implement_trait` for look-alike method names.
- Hand-written `PartialEq`/`Ord` must agree with `Hash`/`PartialOrd`. Why: maps and sorts silently misbehave otherwise. Gate: clippy `derived_hash_with_manual_eq`, `derive_ord_xor_partial_ord`.
- Put `#[must_use]` on outcome types (`Pushed`, `Queued`), on builders and on pure getters. Why: a dropped refusal is a silent loss. Gate: an ignored `#[must_use]` value is `unused_must_use`, denied through `unused`; adding the attribute is review.
- Choose types in this order: concrete, then generics, then `dyn` wrapped in your own struct. In domain and core keep hot `impl`s non-generic. Why: code placement measurably moves hot-path speed, and monomorphising hot code moves it (rust-architecture). Core takes no callbacks. Gate: `callback_param`, `cargo xtask arch`.
- Seal traits that outside crates may name but must not implement (C-SEALED; `mod sealed`, as in `line.rs`). Why: you can add methods later without a breaking change.

## Constructors

- `new` is the primary constructor. Add `with_capacity`, `try_new` and `from_<source>` as needed (C-CTOR). Why: readers find constructors by name. Domain entry seams are named per source (`of`, `from_host`, `threshold_ms`) and counted (`seam`).
- A zero-argument `new` comes with an agreeing `Default`. Why: generic code reaches for `Default`. Gate: clippy `new_without_default`. Domain quantities have neither and expose a named `ZERO`, so the empty value is named at each use.
- Use `with_*` for one or two optional settings. Use a `FooBuilder` with `&mut self` setters and `build() -> Result` otherwise (C-BUILDER). Why: one validation point, and no telescoping `new`.
- Group four or more loose arguments into a type. Why: positional arguments of one type swap silently. Gate: clippy `too_many_arguments` (more than 7); below that, review.

## Visibility and evolution

- Keep items private by default, and use `pub(crate)` before `pub`. Struct fields stay private (C-STRUCT-PRIVATE); a newtype's field is `pub(crate)`. Why: what is not public can change freely. Gate: `unreachable_pub`, and the `pub_newtype_field` budget.
- Use `#[non_exhaustive]` only in a library for others. DEPARTS from BeautifulRust for workspace enums (`Event`, `Effect`): the exhaustive `match` is what forces every consumer to handle a new variant.
- Give each public item one path, and re-export individually, never by glob. Why: two paths make readers ask whether the items differ. A facade may re-export a dependency's vocabulary by name (`my-project-core` re-exports `Event`, `Effect`, ...). The one sanctioned second path is a prelude in a library for others, re-exported by name.
- Keep features additive and bare-named (`alloc`, `serde`), with serde behind a feature (C-FEATURE, C-SERDE). Why: Cargo unifies features across the graph, so a subtractive feature breaks someone else's build. Measurement arms are `cfg(arm)`, never features (rust-measure). Gate: `test:features` (`cargo hack --each-feature`).
- No prelude is required, and none is banned. DEPARTS from BeautifulRust's "don't define a prelude": not adopted as a hard rule, because it gives no reason. See the one-path rule above for the exception.

## Errors and panics — [errors.md](references/errors.md)

- Use `Option` for normal absence, `Result` for failures the caller can act on, and a panic for bugs. Why: the type says which one the caller faces. Panics are not exceptions.
- No `unwrap` outside tests; use `expect("why this holds")` or `let ... else`. Why: the message records the invariant. Gate: clippy `unwrap_used` (tests allowed by `clippy.toml`).
- Every public fallible fn has `# Errors`, and every panicking one has `# Panics`. Why: callers handle what they are told about. Gate: clippy `missing_errors_doc`, `missing_panics_doc`.
- In core, refuse and count before failing: `handle` never fails, and refusals are `Tally` counters the host reads. DEPARTS from BeautifulRust's error rules: core errors are `Copy` enums with no payload that implement `core::error::Error`. Gate: `layers.toml` `[roles.core] external = []`, checked by `cargo xtask layers`.
- Adapter and bin errors use a `thiserror` enum with `#[source]`, with lowercase messages and no trailing punctuation. Why: messages chain as `outer: inner`. Tools use `anyhow` with `.context()`. Neither appears in a public library API.
- Destructors never fail (C-DTOR-FAIL). Expose `finish() -> Result` for fallible teardown. Why: `Drop` cannot report an error.
- Lock poisoning is fatal: `.expect("... lock poisoned")`. Why: the holder panicked mid-update, so the data is suspect.

## Idioms

- Use iterators for short pipelines and `for` loops for side effects; never `for_each`. Why: `?`, `break` and `continue` work in a loop. Collect into `Result<Vec<_>, _>` to stop at the first error.
- Annotate the type at `into()`, `parse()` and `collect()`. Why: inference that flows from far away breaks on an unrelated edit.
- Name drop guards (`let _guard = lock()`). Why: `let _ =` drops at once. Gate: rustc `let_underscore_lock` (deny by default).
- Rebind before a `move` closure (`let total = Arc::clone(total);`). Why: the closure then takes only what it needs.
- Import types unqualified and call functions through their module (`mem::replace`). Use globs only as `use super::*` in tests. Why: the call site then says where a function lives.
- Name magic numbers as consts (`const EFFECTS: usize = 1024;`). Why: the doc comment says where the number came from.
- Log only in adapters and bins (`log`/`tracing`). DEPARTS from BeautifulRust's "log via `tracing`/`log`": core does not log, because logging is I/O. It counts, and the host logs the counts (`main.rs` warns with `refused()`).

## Concurrency (adapters and bins only; core is single-threaded by design)

- Prefer channels to shared state. Keep lock scopes short, never return a `MutexGuard`, and never hold one across `.await`. Why: each of these turns contention into deadlock. Gate: clippy `await_holding_lock`.
- Public futures and shared types are `Send`. For a nameable type, assert it at compile time: `const _: () = { const fn s<T: Send>() {} s::<Foo>(); };`. For an `async fn`'s future, call `fn assert_send<T: Send>(_: &T) {}` on the future value in a test. Why: a lost `Send` breaks callers far away.
- Move CPU-bound work to `spawn_blocking` or a thread. A library never spawns tasks and never assumes a runtime. Why: blocking an executor thread stalls every task on it, and the runtime is the application's choice.

## Docs and naming — [docs-and-naming.md](references/docs-and-naming.md)

- Follow RFC 430 casing (C-CASE). Getters have no `get_` prefix (C-GETTER). Use `as_`/`to_`/`into_` by cost (C-CONV), `iter`/`iter_mut`/`into_iter` (C-ITER), and verb-object-`Error` names (C-WORD-ORDER). Why: a name then predicts cost and shape.
- `len()` implies `is_empty()`. Gate: clippy `len_without_is_empty`. No `Manager`/`Service`/`Factory`/`Helper`/`Util` names. Why: they name no responsibility, so they grow without bound.
- Put `//!` on every crate and module, and `///` on every public item, starting with a one-line summary (review: no `missing_docs` gate). Examples use `?`. Use intra-doc links. Gate: `test:doc` (`RUSTDOCFLAGS='-D warnings'`) fails a broken link, and doctests run in `test:unit`.
- Comments say why, not what. Design history goes in `docs/record.md` and `docs/decisions.md`, not in comments. Why: the code already says what, and history in comments goes stale unseen.

## Refactoring someone else's code

- Pin current behaviour with tests first; they must be red on a planted change (rust-qa). Why: without them, "kept behaviour" is a claim.
- Fix every smell you touch, but keep behaviour; turning a panic into an `Err` is welcome. Show signature diffs. Preserve quirks you do not understand and flag them, because a quirk may be a contract.

## Out of scope here

- Quantity types, seams and typestate: rust-domain.
- Layers, sans-IO, zero-copy and leaks, hot/cold layout, `no_std` and the `unsafe` quarantine (`forbid` workspace-wide, and its DEPARTS from BeautifulRust): rust-architecture.
- Tests and QA: rust-qa. Measurement: rust-measure. Sources and decisions: rust-research.
