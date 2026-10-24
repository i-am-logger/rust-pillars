# No unsafe: forbid everywhere, one declared quarantine

Load this when something seems to need `unsafe` (FFI, intrinsics, a custom
allocator, `transmute`, an OS call), when you add a quarantine crate, or when a
dependency's macro expands into your crate.

Evidence labels: **measured** (someone measured it), **by-reading** (the code
or notes say it), **measured here** (this skill's own compile checks on rustc
1.94.1, aarch64-apple-darwin, against a `git archive` copy of rust-template).

## The policy

- `[workspace.lints.rust] unsafe_code = "forbid"`, and every crate sets
  `[lints] workspace = true`, which `cargo xtask arch` checks.
- `forbid` cannot be lowered: an inner `#[allow]` or `#[expect]` is a compile
  error (E0453).
- **No crate the template generates contains `unsafe`.**
- **DEPARTS from BeautifulRust**, which allows `unsafe` for three reasons
  (novel abstractions, benchmarked performance, FFI) with `SAFETY` comments.
  - WHY: a program is only as sound as its weakest `unsafe` block, and a
    reviewer cannot hold every invariant scattered across a codebase.
  - With every block in one leaf crate, the audit is finite, Miri and
    sanitizers have one target, and the count is a gate.
- Edition 2024 also makes these unsafe:
  - `#[no_mangle]` and `#[export_name]`, which must be written as
    `#[unsafe(no_mangle)]`;
  - `extern` blocks, which must be written as `unsafe extern`.
- Both trip `forbid`. A `#[unsafe(no_mangle)]` added to see a symbol in a
  disassembly failed under `-F unsafe-code` (measured here). Use
  `#[inline(never)]` and the mangled name instead.
- The exception is a symbol C calls by name: that export, every `unsafe
  extern` block and every foreign handle live in a quarantine, as
  [ffi.md](ffi.md) describes.

## Before reaching for the quarantine (in this order)

1. **A safe `core`/`std` API.** Examples:
   - `slice::get` / `split_at` / `chunks_exact`;
   - `u32::from_le_bytes`;
   - `<[T; N]>::try_from(&[T])`;
   - `NonZero*`.
   - `<[T]>::align_to` is *not* safe; zerocopy (below) is its safe replacement.
2. **A safe crate that already holds the `unsafe`, audited and tested.**
   - zerocopy derives and `transmute!`/byte views. With its `simd` feature,
     zerocopy also implements the arch vector types on aarch64, so
     `zerocopy::transmute!(*block)` turns a `[u8; 16]` into a `uint8x16_t`
     with no `unsafe` (measured here).
   - `dhat` counts allocations in tests with no `unsafe impl GlobalAlloc`
     (template `crates/<name>-core/tests/alloc.rs`).
   - Safe platform binding crates, for OS calls.
3. **Safe `#[target_feature]` functions.**
   - Inside `#[target_feature(enable = "neon")]`, the non-pointer intrinsics
     are safe to call (measured here).
   - *Entering* such a function from code without the attribute still needs
     `unsafe`, even when the build enables the feature globally. rustc's
     error E0133 says so: "the neon target feature being enabled in the build
     configuration does not remove the requirement to list it in
     `#[target_feature]`" (measured here).
   - So explicit intrinsics need one `unsafe` entry, and that entry goes in the
     quarantine ([simd.md](simd.md)).
   - These features arrived after the template's `rust-version = "1.85"`.
     Safe `#[target_feature]` functions are believed to have come in 1.86 and
     safe intrinsics in 1.87. That is UNVERIFIED here: they were checked only
     on 1.94.1. Raise `rust-version` to what `cargo msrv find` measures.
4. **A quarantine crate**, if none of the above works.

## The quarantine crate

Declare it in `layers.toml`. A quarantine is off by default, so adding one is a
reviewed change:

```toml
[crates]
my-project-quarantine = "quarantine"
# [roles.quarantine] already exists: may_depend_on = ["domain", "quarantine"], external = ["*"]
```

Requirements, with today's enforcement:

| Requirement | Enforced by |
|---|---|
| Depends only on the domain and other quarantine crates; the core, adapters and the bin use its safe API | `cargo xtask layers` |
| The only crate not on `[lints] workspace = true`; its own `[lints]` repeat the workspace set with `unsafe_code = "deny"` (not forbid) | `cargo xtask arch` (exempts only quarantine) |
| Every `unsafe` block carries `#[expect(unsafe_code, reason = "...")]`, so each site is visible twice | rustc (`deny`, then `unfulfilled_lint_expectations` if stale) |
| Every block has a `// SAFETY:` comment naming the contract and how it is discharged | clippy `undocumented_unsafe_blocks` |
| One unsafe operation per block, so each SAFETY comment covers one contract | clippy `multiple_unsafe_ops_per_block` (add it to the quarantine's `[lints.clippy]`) |
| `unsafe fn` bodies still mark each unsafe op | `unsafe_op_in_unsafe_fn = "deny"` |
| The count of `unsafe` blocks, fns, impls and traits equals the budget, and so does the count of `#[expect]` | `cargo xtask budgets` (`unsafe_code`, `expect_attr`) |
| A public API that is safe functions only, with domain-typed I/O where a domain type exists | review |
| A written soundness argument per `unsafe impl` (which invariant makes `Send`/`Sync` hold), in its doc comment, referring to the Rustonomicon section it relies on | review |
| Miri and sanitizers in QA (`qa:miri`, `qa:sanitize`) | review (planned task) |
| A gate that checks `allow(unsafe_code)` appears only in quarantine crates, and that the quarantine's lint set is complete | review (planned gate; today, `forbid` plus `cargo xtask arch` plus `budgets` cover the placement) |
| `#![no_std]`, if the core may use it | review: `test:bare` builds only `cargo xtask crates --role domain,core`, so a quarantine is not built bare until that role list is extended (planned) |

Its manifest lints repeat the workspace set with `unsafe_code` lowered to
`deny`, plus `multiple_unsafe_ops_per_block`. The pedantic `missing_errors_doc`,
`missing_panics_doc` and `too_many_lines` are not in `all`, so they are listed
by hand. This passed clippy `-D warnings` with the example below (measured
here):

```toml
[lints.rust]
unsafe_code = "deny"
unsafe_op_in_unsafe_fn = "deny"
missing_debug_implementations = "deny"
unreachable_pub = "deny"
unused = { level = "deny", priority = -1 }

[lints.clippy]
all = { level = "deny", priority = -1 }
allow_attributes = "deny"
allow_attributes_without_reason = "deny"
undocumented_unsafe_blocks = "deny"
multiple_unsafe_ops_per_block = "deny"
unwrap_used = "deny"
missing_errors_doc = "deny"
missing_panics_doc = "deny"
too_many_lines = "deny"
disallowed_methods = "deny"
disallowed_types = "deny"
```

A minimal site, compiled and tested here (measured here). The whole crate holds
one `unsafe` block, the entry, and `cargo xtask budgets --print` counts
`unsafe_code = 1` and `expect_attr = 1` for it. Its manifest has
`my-project-domain` and `zerocopy = { workspace = true, features = ["simd"] }`
as dependencies. The domain type `Newlines` is in [simd.md](simd.md#compiled-example).
The snippet gate does not compile it: this project has no quarantine crate, and
its domain has no `Newlines`.

```rust,ignore
//! The only home of `unsafe` (role: quarantine). Exposes safe functions only.

#![no_std]

/// Newlines in a 16-byte block, by NEON. Safe to call: the one `unsafe` is
/// inside, and the target discharges its contract.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[must_use]
pub fn newlines(block: &[u8; 16]) -> my_project_domain::Newlines {
    #[expect(unsafe_code, reason = "entering a #[target_feature] fn")]
    // SAFETY: this item is compiled only when `target_feature = "neon"` is set
    // for the whole build, so every CPU this binary runs on has NEON.
    let n = unsafe { neon::newlines(block) };
    my_project_domain::Newlines::from_backend(n)
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
mod neon {
    use core::arch::aarch64::{uint8x16_t, vaddvq_u8, vandq_u8, vceqq_u8, vdupq_n_u8};

    /// No `unsafe` inside: the load is a zerocopy transmute, and the
    /// arithmetic intrinsics are safe in a `neon` context.
    #[target_feature(enable = "neon")]
    pub(super) fn newlines(block: &[u8; 16]) -> u8 {
        let v: uint8x16_t = zerocopy::transmute!(*block);
        let eq = vceqq_u8(v, vdupq_n_u8(b'\n'));
        vaddvq_u8(vandq_u8(eq, vdupq_n_u8(1)))
    }
}
```

- An earlier draft loaded the bytes with `vld1q_u8(block.as_ptr())`, which
  needs a second `unsafe` because pointer loads stay unsafe.
- zerocopy's `simd` feature removed that block. Use the escapes above in order
  before keeping any `unsafe`.

## Macro-expanded unsafe

- `forbid(unsafe_code)` does not report `unsafe` blocks that come from another
  crate's `macro_rules!` expansion. `self_cell::self_cell!` expands to `unsafe`
  blocks, and it compiled in an adapter under the workspace `forbid` (measured
  here).
- `cargo xtask budgets` scans source text, not expansions, so it does not see
  these blocks either.
- Rule: a dependency whose macros expand `unsafe` into your crate is an
  `unsafe` decision. Name it in review, and prefer crates that say they are
  Miri-tested. self_cell's docs say it is.
- The same macro trips clippy `disallowed_methods`, because it calls
  `core::mem::forget`. See [zero-copy.md](zero-copy.md#own-dont-leak) for the
  counted `#[expect]`.

## Why gates, not prose

- A policy held by per-crate attributes and prose has gaps that nothing
  reports:
  - one crate without the crate-level deny;
  - a doc naming "the one module that needs unsafe" while others allow it;
  - an `#[allow]` without a reason, which cannot rot visibly.
- The template's `forbid`, its counted `#[expect]`, and `allow_attributes =
  "deny"` close all three gaps.
