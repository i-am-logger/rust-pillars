# What rust-idioms took from BeautifulRust, and what it changed

BeautifulRust (morganlinton, MIT, commit 6b3a96c) is a Claude Code plugin with one skill,
`rust-best-practices`. It has a single, always-loaded SKILL.md of about 500 lines and five
paraphrased reference digests. rust-idioms keeps most of its rules.

It changes where those rules live and how they are held:
- **Short entry, depth on demand.** SKILL.md is short, and depth lives in these references.
  An always-loaded 500-line file costs about a third more tokens on every task.
- **Gates, not advice.** Each rule is tied to a gate in rust-template where one exists. A
  prose rule with no gate is marked "review".
- **Scoped by code kind.** Each rule is scoped to the kind of code (domain, core, adapter/bin,
  library for others), so performance-critical code is not held to rules written for
  application code.

Labels: **KEPT** (same rule), **CHANGED** (same intent, a different rule or scope),
**DROPPED** (not carried over), **ADDED** (not in BeautifulRust).

## Kept

| Rule | Where now | Gate in rust-template |
|---|---|---|
| Write Rust-shaped code: no Manager/Factory, no `Deref`-to-base | docs-and-naming.md §1, api-design.md §3 | review |
| Make invalid states unrepresentable | rust-domain | budgets, trybuild |
| Before handing back: fmt, clippy `-D warnings`, test, doc | SKILL.md | `devenv test` |
| Fix lints, don't silence; `#[expect(lint, reason)]` on the narrowest item | SKILL.md | clippy `allow_attributes`, `allow_attributes_without_reason` |
| Never `#![deny(warnings)]` in source; lints in `[workspace.lints]` | SKILL.md | review; `cargo xtask arch` checks `[lints] workspace = true` |
| Don't clone to quiet the borrow checker; the ladder | ownership.md §2 | review; `clone_on_copy` |
| Most general borrowed parameter; own only to store | api-design.md §2 | clippy `ptr_arg` |
| Tuples, not out-parameters | api-design.md §2 | review |
| No `Rc`/`Arc`/`Box`/`RefCell` in public signatures | api-design.md §2 | review |
| RFC 430 case; no `get_`; `as_`/`to_`/`into_`; `iter`/`IntoIter`; verb-object-Error | docs-and-naming.md | rustc case lints; `len_without_is_empty` |
| `Debug` everywhere, redacting secrets | api-design.md §3 | `missing_debug_implementations` |
| `From`/`TryFrom`, never `Into`; `Deref` for smart pointers only; operators keep their math meaning | api-design.md §3 | `from_over_into`, `should_implement_trait` |
| Hand-written `Ord`/`Eq` agree with `PartialEq`/`Hash` | api-design.md §3 | `derived_hash_with_manual_eq`, `derive_ord_xor_partial_ord` |
| `#[must_use]` | api-design.md §4 | review |
| `with_*` for few options, builder otherwise; group loose args | api-design.md §5 | `too_many_arguments` |
| Private by default, `pub(crate)` before `pub` | api-design.md §6 | `unreachable_pub`, `pub_newtype_field` budget |
| Sealed traits | api-design.md §6 | review |
| One path per public item; re-export individually, never by glob | api-design.md §6 | review |
| Additive, bare-named features; serde behind a feature | api-design.md §6 | `test:features` |
| concrete > generics > `dyn`; no nested generics; object safety early; macros last | api-design.md §7 | `callback_param` (core) |
| Option / Result / panic split; panics are not exceptions | errors.md §1 | — |
| `expect` with a reason, `let ... else`, lock poisoning fatal | errors.md §3 | `unwrap_used` |
| Library errors: `Error + Send + Sync + 'static`, `source()`, no `String`/`()`/`Box<dyn Error>`, lowercase Display, return the consumed argument, no `anyhow` in a public API | errors.md §2 (library for others) | review |
| Destructors never fail | errors.md §5 | review |
| `# Errors`, `# Panics`, `# Examples` with `?`, intra-doc links; comments say why | docs-and-naming.md §4 | `missing_errors_doc`, `missing_panics_doc`, `test:doc` |
| Iterators for short pipelines, `for` not `for_each`, typed `collect`/`parse`/`into` | SKILL.md Idioms | review |
| Named drop guards; rebind before `move` closures | ownership.md §3 | rustc `let_underscore_lock` |
| Imports: types unqualified, functions via module, globs only `super::*` in tests | docs-and-naming.md §3 | review |
| `Send` public types and futures, asserted at compile time; an `Rc` or `RefCell` borrow held over `.await` loses `Send` | concurrency.md §3 | `await_holding_refcell_ref`; a `Send` test |
| Channels over shared state; linked invariants under one lock; no user closures under a lock; short locks; no returned guard | concurrency.md §1, §4 | `await_holding_lock`; otherwise review |
| `async fn` by default; `-> impl Future` in traits or for eager setup | concurrency.md §2 | clippy `manual_async_fn`, rustc `async_fn_in_trait` |
| Long CPU work yields or moves to `spawn_blocking` | concurrency.md §5 | review |
| Libraries don't spawn or assume a runtime | concurrency.md §9 (only the bin starts one) | review; `std::thread::spawn` banned in domain and core (crate-local `clippy.toml`) |
| Refactoring: pin behaviour with tests first, fix the smells the request is about, keep the public surface (error variants, derives, constructor arguments), panic-to-`Err` welcome, keep and flag quirks, show signature diffs | refactoring.md | rust-qa (red first); `ptr_arg`, `unnecessary_unwrap` for two of the smells |
| Dependencies cost compile time, size and supply-chain exposure, `build.rs` and proc macros included | dependencies.md §1 | `cargo xtask layers` (per-role `external`) |
| Version ranges, never `*`, no needless `=`; `[workspace.dependencies]` | dependencies.md §2 | `deny.toml` `wildcards = "deny"` (`test:deny`) |
| Commit `Cargo.lock` for binaries | dependencies.md §3 | review (the template commits it) |
| Audit with `cargo tree --duplicates` and `cargo deny` | dependencies.md §4 | `test:deny` (licenses, bans, sources); `qa:advisories` |
| Newest stable edition, MSRV set deliberately | dependencies.md §5 | review; `rust-version` in `Cargo.toml` |
| No code before `main`; no unstable features on stable | dependencies.md §6 | review; the stable channel in `nix/rust.nix` (E0554) |

## Changed

| BeautifulRust | rust-idioms | Why |
|---|---|---|
| "Design for usability first, optimize after measuring" | **DEPARTS for domain and core.** Layout and ownership are designed for performance from the start; every change is then measured. Kept for adapters and bins. | A hot struct's layout is costly to change later, and layout itself moves performance (rust-measure: placement is a variable). |
| No `bool` parameters in public signatures | **Generalised:** in domain and core, no raw integer, float or `char` in a public signature or field outside a named, counted seam, and no `bool` parameter or field at all (private ones too). A predicate's `bool` return stays. | A `u32` says no more than a `bool` does. C-CUSTOM-TYPE and C-NEWTYPE applied to every quantity. Gate: `cargo xtask budgets`. Detail in rust-domain. |
| Library-error rules everywhere; apps use `anyhow`/`eyre` | **Per kind.** Core refuses and counts. When a `Result` is needed it uses a `Copy` enum with no payload that implements `core::error::Error`, with no thiserror. Adapters and bins use `thiserror` with `#[source]`; tools use `anyhow` + context; libraries for others follow BeautifulRust. | No allocator means no payload to carry, and core's dependency allowlist is empty (`layers.toml`). thiserror 2 is itself `#![no_std]` with an optional `std` feature, so the reason is layering, not capability. |
| `unsafe` allowed for a novel abstraction, a benchmarked speed-up, or FFI, with SAFETY comments | **DEPARTS:** `unsafe_code = "forbid"` workspace-wide. The only escape is a declared quarantine crate in `layers.toml`, with an `unsafe` budget equal to its count. | `forbid` cannot be lowered by an inner `allow`. One audited leaf beats scattered justified blocks. See rust-architecture. |
| Derive common traits eagerly | **Per type in domain and core;** eager in libraries for others. | A derive is a promise: `Default` invents a value, `From<int>` reopens primitive entry. |
| `new()` alongside an agreeing `Default` | Kept where a zero-argument `new` exists (clippy `new_without_default`). Domain quantities expose a named `ZERO` and have neither. | The empty value is named at each use. |
| `#[non_exhaustive]` on public enums and structs | **Library for others only.** Workspace enums (`Event`, `Effect`) stay exhaustive. | Inside one workspace, the exhaustive match is the check that every consumer handles a new variant. |
| No foreign `pub use` | A facade crate may re-export a dependency's vocabulary by name (`my-project-core` re-exports domain types), still one path per item. | Hosts depend on one crate. Duplicate paths, not re-exports, are what confuse readers. |
| Log via `tracing`/`log` | **Adapters and bins only.** Core counts (`Refusals`), and the host logs the counts. | Logging is I/O. A no_std sans-IO core has no logger and must not branch on log levels in hot code. |
| `const fn` only if it stays const | **Libraries for others only.** In domain and core, make constructors and tables `const`. | No external semver to break inside the workspace; `const` lets hosts build machines and tables in `static`s. |
| Library takes `impl Read` / `impl AsyncRead`, an executor or a spawn function from the caller | **DEPARTS for core:** a core takes none of these; bytes arrive as events, and only adapters read. Adapters and bins: kept, and only the bin starts a runtime (concurrency.md §1, §9). | Sans-IO: the core is then the same under every runtime and under none, and needs no mocks. |
| Audit with `cargo udeps` and `cargo audit` | `cargo machete` for unused dependencies (review); `cargo deny check advisories` as `qa:advisories` (dependencies.md §4). | `cargo udeps` needs a nightly toolchain, which the template does not install. cargo-deny already covers the RustSec database, so a second tool would duplicate it. |
| Lints in `[lints]`/`[workspace.lints]` | Kept, plus `#[expect]` is counted (`expect_attr`, budget 0 by default) and every `as` is counted (`as_cast`). | A suppression or a cast is a reviewed change, raised only in its own commit. |
| Baseline lints | Stronger: `unsafe_code` forbid, `unreachable_pub`, `unused` deny, clippy `all` deny, `unwrap_used`, `missing_errors_doc`, `missing_panics_doc`, `too_many_lines`, `disallowed_methods`/`types`. | Rules that fail the build instead of being remembered. |

## Dropped

| BeautifulRust | Why not |
|---|---|
| "Don't define a prelude" as a hard rule | Not adopted. It is opinionated, and BeautifulRust gives no reason for it. "One path per item" and "no glob re-exports" carry the real concern. The one sanctioned exception to one-path-per-item is a prelude in a library for others whose users import many traits, re-exported by name and never by glob. |
| Its `unsafe` mechanics section (SAFETY comments, Miri, FFI `repr(C)`/`CString`, `-sys` crates) | Out of scope here because `unsafe` is forbidden outside a quarantine crate. The quarantine rules are in rust-architecture's `references/no-unsafe.md`, and the FFI mechanics in its `references/ffi.md`. |
| Its test checklist (`cfg(test)`, `tests/`, fuzz, criterion/divan, Miri) | Replaced by rust-qa (independent models, chunk invariance, trybuild twins, mutation testing). Benchmarks are covered by rust-measure, which uses interleaved A/B and A/A arms rather than criterion, deliberately. |
| Its performance section (`with_capacity`/`collect`, caller-owned buffers, `Box<[T]>`/`shrink_to_fit`, fast hashers, batching under a lock, mimalloc, no `Copy` on large types) | Moved out of rust-idioms. Each tip is now a section of rust-architecture's `references/allocation.md`, scoped to code outside the hot core. A tip is adopted only once an A/B run confirms it, following rust-measure's `references/ab-protocol.md`. In core, allocation per event is a gate (`crates/<name>-core/tests/alloc.rs`), not a tip. |
| Mockable I/O, time and randomness via traits | Replaced by sans-IO. Time, I/O and randomness enter the core as events, so tests feed events and need no mocks. `Instant`/`SystemTime` are banned in domain and core (crate-local `clippy.toml`). |
| Self-graded LLM evals (three cases) | The plugin's evaluation is the template's gates plus planted violations that must turn each gate red. |

## Added (not in BeautifulRust)

- Four code kinds, with each rule scoped to them (SKILL.md table).
- The refuse-and-count error model for hot cores (errors.md §2).
- `as` replaced by `From`/`TryFrom`, with casts counted.
- Rung 6 of the ownership ladder (one borrow per batch; ownership.md) and the no-leak owner order
  (rust-architecture, zero-copy).
- The fact that a `std::` path in `disallowed-methods` bans nothing in a `no_std` crate, so the
  `alloc::`/`core::` paths are listed too (`clippy.toml`).
- The scope of clippy `wrong_self_convention`: it skips exported items by default, so
  `as_`/`to_`/`into_` on public API is review (docs-and-naming.md §2).
- Docs as tested claims (`cargo xtask docs`) and a byte-budgeted `CLAUDE.md`.
- Concurrency meets sans-IO: one core value per task, or one owner task fed by a channel, before
  any lock (concurrency.md §1).
- Bounded channels, with backpressure or refuse-and-count when full (concurrency.md §6).
- Cancellation safety in `select!`, and a `# Cancel safety` doc section (concurrency.md §7).
- Structured concurrency: `thread::scope` and `JoinSet`, never a detached task (concurrency.md §8).
- Refactoring: quirks pinned by a named `quirk_` test; `cargo semver-checks` for a library for
  others (refactoring.md §3, §4).
- Dependency hygiene tied to the template's gates: `deny.toml`, `test:deny`, `qa:advisories`,
  duplicates that only warn, and MSRV-aware resolution under resolver 3 (dependencies.md).

## ATTRIBUTION

- **BeautifulRust**
  - Author morganlinton, MIT licence, commit 6b3a96c.
  - The rule list adapted here comes from its `rust-best-practices` skill. Wording is
    rewritten, and every change is listed above.
  - Its MIT licence requires the copyright notice and permission notice to accompany
    substantial portions. The full licence text ships beside this skill as
    [LICENSE-BeautifulRust.txt](../LICENSE-BeautifulRust.txt). Keep that file and this section
    whenever the skill is redistributed.
- **Rust API Guidelines** (rust-lang). Item ids such as C-NEWTYPE, C-CONV and C-GOOD-ERR are
  cited by id; nothing is copied.
- **Other guides BeautifulRust digests** (Rust Design Patterns, Microsoft Pragmatic Rust
  Guidelines, the Google Rust style guide) are named as BeautifulRust's sources only. Nothing
  here is taken from them directly.
- **Effective Rust** (David Drysdale) is CC BY-NC-ND. It is mentioned by name only: nothing
  from it is quoted, digested or adapted here.
