# Naming, imports and documentation

Load this when naming a public item, writing a doc comment, or reviewing a module's surface.
Rule ids are from the Rust API Guidelines checklist.

## 1. Case and words

- Follow RFC 430 casing (C-CASE):
  - `UpperCamelCase` for types, traits and enum variants
  - `snake_case` for functions, methods, modules, locals and fields
  - `SCREAMING_SNAKE_CASE` for consts and statics
  - Acronyms are one word: `Uuid`, not `UUID`.
  - Rustc's `non_camel_case_types`, `non_snake_case` and `non_upper_case_globals` warn, and
    `test:clippy` runs with `-D warnings`.
- Name types after what they are, never after a role in a pattern. No `Manager`, `Service`,
  `Factory`, `Helper` or `Util`.
  - A `LineCounter` counts lines.
  - A `LineManager` says nothing, and grows until it does everything.
- Use consistent word order (C-WORD-ORDER). Error types are verb-object-`Error`:
  `ParseIntError`, `LoadConfigError`.
- Name a module after its primary item, and keep that item's name free of the module's name:
  `quantity::ByteLen`, not `quantity::QuantityByteLen`.
- Name magic numbers as consts with a doc comment that says where the value comes from
  (`const EFFECTS: usize = 1024;` in `crates/<name>/src/main.rs` says what it bounds).

## 2. Method names that promise a cost

| Prefix | Cost | Receiver | Example |
|---|---|---|---|
| `as_` | free, borrowed view | `&self` / `&mut self` | `Effects::as_slice` |
| `to_` | expensive, or a copy of a `Copy` value | `&self` (or `self` for `Copy`) | `str::to_owned` |
| `into_` | consumes, usually cheap | `self` | `String::into_bytes` |

- These follow C-CONV.
- clippy `wrong_self_convention` checks the receiver. Under clippy's default
  `avoid-breaking-exported-api = true` it skips exported items, so on public API this is
  review, not a gate (checked with clippy 1.94).

Other names:
- Getters have no `get_` prefix: `fn len(&self)`, not `fn get_len` (C-GETTER).
  - The template's `get()` is not a getter of a field. It is the single named exit from a
    newtype to its raw integer, as in `NonZeroU32::get`, and is counted as a seam
    (`budgets.toml`).
- Constructors are `new`, `with_capacity`, `try_new` and `from_<source>`. Domain entry seams
  are named for their source: `of`, `from_host`, `threshold_ms`.
- Iteration methods are `iter`, `iter_mut` and `into_iter` (C-ITER), with types named `Iter`,
  `IterMut` and `IntoIter` in libraries for others (C-ITER-TY).
- A public `len()` comes with `is_empty()`. Gate: clippy `len_without_is_empty`.
- Name the behaviour at the edge of the range: `saturating_incr`, `checked_grow`,
  `saturating_advance`. A plain `add` hides saturation (rust-domain).
- Two-state enums name what happened, not true/false: `Pushed::Taken`/`Refused`,
  `Queued::Taken`/`Dropped`.
- Features have bare names: `alloc`, `serde`, `std`. Never `use-serde`, `with-alloc` or
  `no-std` (C-FEATURE): a feature turns something on.
- Test names are the property, as a sentence: `a_full_queue_drops_and_counts`,
  `a_clock_that_steps_back_reads_as_no_time`. A failing test name should read as the bug report.

## 3. Imports

- Import types unqualified (`use my_project_domain::{Effect, Event};`).
- Call free functions through their module (`use core::mem;` then `mem::replace(..)`), so the
  call site says where the function lives.
- Globs:
  - only `use super::*;` inside a `#[cfg(test)] mod tests`
  - `use proptest::prelude::*;` where a test framework documents it
  - never `pub use foo::*`
- Group imports as std/core/alloc, then external crates, then `crate::`, with a blank line
  between groups. `cargo fmt` (via `treefmt`, gate `test:fmt`) sorts within each group.

## 4. Documentation

### What gets docs

- `//!` on every crate root and module.
  - Say what the module is for, which pillar it serves, and what it refuses.
  - The template's `crates/<name>-domain/src/lib.rs` lists each technique it uses, and
    says each is enforced by the compiler or a gate.
- `///` on every public item.
  - The template has no `missing_docs` gate today.
  - A workspace that publishes a library for others adds `missing_docs = "deny"` under
    `[workspace.lints.rust]`, not `#![deny(missing_docs)]` in source.
- The first sentence is one line, about 15 words, saying what the item is or does. rustdoc uses
  it as the summary in module listings.

### Sections

| Section | When | Gate |
|---|---|---|
| `# Examples` | every public fn of a library for others; key entry points elsewhere | review (C-EXAMPLE) |
| `# Errors` | every public fn returning `Result`: each variant and when | clippy `missing_errors_doc` |
| `# Panics` | every public fn that can panic: the condition | clippy `missing_panics_doc` |
| `# Safety` | only in a declared quarantine crate, on each `unsafe fn` | `unsafe_code = "forbid"` elsewhere |

Examples use `?`, with the `main` hidden (C-QUESTION-MARK), so readers copy error handling and not
`unwrap` (a doc comment, not a standalone item):

````rust,ignore
/// Reads the big-endian length field at the start of `bytes`.
///
/// # Examples
///
/// ```
/// use my_proto::{Bytes, PayloadLen};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let _len = PayloadLen::parse(Bytes::of(&[0x00, 0x03]))?;
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// [`ParseHeaderError::Short`] if fewer than two bytes arrived.
````

- Doctests run in `test:unit` (`cargo test --workspace --all-features --doc --profile qa`), so an example
  that stops compiling or panics fails the suite.

### Links

- Link with intra-doc links: [`Machine::effects`], [`crate::Pushed::Refused`]. Never use a URL
  to your own docs.
- `test:doc` runs `cargo doc` with `RUSTDOCFLAGS='-D warnings'`, so a broken intra-doc link
  (`rustdoc::broken_intra_doc_links`) fails the build.

### Comments

- A comment says why: the invariant, the measurement, or the standard clause. The code
  already says what.
  - Good, from `quantity.rs`: "`usize` is at most 64 bits on every target this builds for,
    so the widening is exact".
  - Bad: "increment the counter".
- No design journals in comments ("first we tried X, then Y").
  - History goes to `docs/record.md`, decisions (D-n, with "Reverses if") to
    `docs/decisions.md`, and numbers to `MEASUREMENTS.md`.
  - A comment may cite them.
- Cite a standard as `[[source: <key>]]` plus a clause. `cargo xtask sources` resolves every
  citation against `docs/research/sources/MANIFEST.toml` (rust-research).
- Docs are claims about the tree. `cargo xtask docs` checks that every backticked repository
  path and every relative Markdown link in the docs exists, so do not name a file that is not
  there.
- `CLAUDE.md` holds rules only, and is held to an exact byte budget that may only shrink
  (`budgets.toml [docs] claude_md_bytes`, `cargo xtask docs`). Put reasoning in a skill or a
  record, not there.
