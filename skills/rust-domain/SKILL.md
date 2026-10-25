---
name: rust-domain
description: "Use when modelling values in a domain or core crate: a quantity, id, count, length, offset, timestamp or flag; an integer width, operator or conversion; a lifecycle (typestate); events and effects as types; replacing a bool; a compile_fail test; or a `cargo xtask budgets` primitive finding."
license: CC-BY-NC-SA-4.0
---

# rust-domain: refuse primitives

Pillar 1 applies to crates with role `domain` or `core` in `layers.toml`. There, every value that means something has its own type, the wrong program does not compile, and `cargo xtask budgets` counts every place a raw integer still crosses the API. Adapters and bins are held only on `as_cast`, `expect_attr` and `unsafe_code`; there, the rest of this skill is advice, not a gate.

Load a reference only when the task needs it:
[newtypes](references/newtypes.md) (quantities, widths, operators, checklist) ·
[typestate-and-brands](references/typestate-and-brands.md) ·
[vocabulary](references/vocabulary.md) (events, effects, wire enums, bitsets) ·
[compile-fail](references/compile-fail.md) (trybuild twins, pinned stderr).

## How a value gets in: derive first, never a budget raise

- **Bytes enter by a derive, not a constructor.** Give the type its wire or stored layout (`repr(transparent)` over a zerocopy byte-order type such as `big_endian::U32`, or a `repr(C)` record of such types) and derive `FromBytes`. Callers use the derived `T::read_from_bytes`, `T::ref_from_prefix` or `<[T]>::ref_from_bytes`. That is no hand-written fn, so nothing is counted, and a value that is also stored is read back through the same derive.
- **Otherwise call a seam that already exists** (`Millis::from_host`, `ReadLen::of`). Adding a fn does not count as using an existing seam, even under a seam name.
- **Any `pub fn` that takes a raw primitive is a new seam**, whatever its name (`of`, `from_raw`, `from_le_bytes`). A `[seams]` name counts as `seam`, any other name as `primitive_api`, and the budget equals the count, so either one fails the gate. Measured with `cargo xtask budgets --print` on rust-code's domain while `budgets.toml` held `seam = 8` (re-measured 2026-10-25): one added constructor under a seam name took `seam` to 9, one under another name took `primitive_api` to 1, and a derive-only type moved neither.
- **Tests build values the same way**, or through a helper inside `#[cfg(test)]` that takes a plain integer. Test code is not counted, so tests are no reason for a public constructor.
- **Never raise a budget to make a gate pass.** CLAUDE.md: "a budget moves only in a commit of its own that says why". Keep that commit apart from the change that needs it, say in it why no derive or existing seam works, and name it in your hand-back.

```rust
use zerocopy::big_endian::{U16, U32};
use zerocopy::{FromBytes, Immutable, KnownLayout, Unaligned};

/// A channel number as the wire carries it: unsigned 16-bit, big-endian.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[derive(FromBytes, Immutable, KnownLayout, Unaligned)]
#[repr(transparent)]
pub struct Channel(pub(crate) U16);

/// A payload length as the wire carries it: unsigned 32-bit, big-endian.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[derive(FromBytes, Immutable, KnownLayout, Unaligned)]
#[repr(transparent)]
pub struct PayloadLen(pub(crate) U32);

/// A frame header, borrowed in place from the bytes that follow it.
#[derive(FromBytes, Immutable, KnownLayout, Unaligned)]
#[repr(C)]
pub struct Header {
    pub(crate) channel: Channel,
    pub(crate) len: PayloadLen,
}

let wire = [0x00, 0x07, 0x00, 0x00, 0x01, 0x00, b'h', b'i'];
let (header, rest) = Header::ref_from_prefix(&wire).expect("a six-byte header");
assert_eq!(header.channel, Channel::read_from_bytes(&[0x00, 0x07]).expect("two bytes"));
assert_eq!(header.len, PayloadLen::read_from_bytes(&[0, 0, 1, 0]).expect("four bytes")); // 256
assert_eq!(rest, b"hi"); // borrowed, not copied
assert!(Header::ref_from_prefix(&wire[..5]).is_err());
```

## Opaque newtypes

- **Give every quantity its own type.** `BytePos`, `ByteLen` and `WordCount` are separate types even though two of them are `u32`. Mixing them is error E0308, not a review comment. Source: API Guidelines C-NEWTYPE. Gates: `primitive_api` = 0, and the trybuild case `mixed_quantities`. DEPARTS from BeautifulRust's narrower "no `bool` parameters": a raw `u32` says no more than a `bool` does, so every primitive is refused outside a named seam.
- **Make the field `pub(crate)`, never `pub`.** A `pub` field is a forgeable constructor and an exit nobody counts. Sources: C-NEWTYPE-HIDE, C-STRUCT-PRIVATE. Gate: `pub_newtype_field` = 0.
- **Hand-write each newtype; no newtype macro.** That way each derive is a decision about that type. A macro hands every type the same set.
- **Derive by meaning.**
  - `Ord` only if the values are ordered; `Hash` only if the type is a key.
  - `Default` only when zero is the empty value and someone asked for it.
  - Never `From<int>`.
  - Why: each derive is a capability. `Default` invents a value, and `From<u32>` reopens the door the newtype closed.
  - DEPARTS from BeautifulRust's "derive eagerly" and from C-COMMON-TRAITS: those serve interop between published libraries, while a domain crate serves one workspace, where an unused capability is only a hole.
  - Gate: `impl From<u32>` counts as `primitive_api`; trybuild case `integer_into_quantity`.
- **Put `repr(transparent)` on every single-field quantity** (pillar 1). It guarantees the inner type's layout (Rust Reference, "Type layout") at no cost, and it is mandatory where layout is a contract: a POD record, bytes on a wire, FFI. Not gated: review holds it. Every quantity in the template has it since `ea24734`.
- **Give each source of a value its own named entry.** Bytes take the derive (above). The template's host-value seams are `Millis::from_host(u64)`, `ReadLen::of(&[u8])` and `Elapsed::threshold_ms(u64)`: each name records where the integer came from, and each is counted.
- **Add an exit only when interop needs one, and name it `get()`.** The pillar also allows `index()`, but `index` is not in the template's `[seams]`, so using it is a seam commit. `Elapsed` has no exit; core compares it with `Ord`. Every exit is a hole in the type, so add one for a caller, not "for completeness".
- **Declare the seams, and let the gate count them.** `[seams] names` in `budgets.toml` lists `get`, `of`, `from_host` and `threshold_ms`. A `pub fn` with one of those names whose signature carries a primitive counts as `seam`; under any other name it counts as `primitive_api`. The template domain has `seam = 8`. A new seam is a budget raise, so it takes a commit of its own (above).
- **Convert with `T::from` / `T::try_from`, not `as`** (the general rule: rust-idioms). `usize::from(u8)` is lossless and not counted; every `as` counts toward `as_cast`. `From<usize> for u64` does not exist, and a `const fn` cannot call `TryFrom`, so there a `usize → u64` stays one counted `as` with a comment proving it exact (`BytePos::saturating_advance`); elsewhere write `u64::try_from(n)` with a named fallback. Gate: `as_cast` must equal the count.
- **Don't suppress lints in domain or core.** `expect_attr` is counted in every crate and its budget is 0 unless `budgets.toml` says otherwise, so fix the code; a suppression is a budget raise. How to suppress when unavoidable (`#[expect(lint, reason)]`, never `#[allow]`): rust-idioms.

## Width and arithmetic

- **Store narrow, compute wide.**
  - Store at the value's real range: `ByteLen(u32)` sits in a record.
  - Compute in `usize` for types used to index and add (`ReadLen(usize)`).
  - Why: a narrow arithmetic type pays a widening at every use, and it truncates silently when a wider source is routed through it.
- **Split types whose ranges differ.** A read can be any slice length, so it is `ReadLen`; a line is `ByteLen`. One type for both truncates: a read longer than the narrow type's maximum is cut, silently if the type saturates.
- **Implement an operator only when it is total.**
  - Total means every value the constructors can produce stays in range, and the result type has no operator of its own, so the bound cannot compound. Otherwise use a named method.
  - Why: `[profile.qa]` sets `overflow-checks = true`, so an overflowing `+` panics under `test:unit` but wraps in `release`. An operator also hides the saturation it does.
  - Source: C-OVERLOAD. See [newtypes §5](references/newtypes.md#5-declared-operators-only-where-total).
- **Name the failure mode in the method:** `saturating_incr`, `checked_grow -> Option<Self>`, `at_most`. Put `#[must_use]` on each such method and on result enums (`Pushed`, `Queued`), so an ignored refusal warns. `unused` is `deny`.
- **At the top of a range, refuse; never wrap.** Return `None` or `Pushed::Refused`, and have the caller count it (`Refusals` in core).

## Proofs carried by types

- **Narrow in the constructor, with no path back to wider.** Only the operation that checked the invariant may make the type. `Elapsed` comes only from `Millis::since` or `threshold_ms`, and nothing widens it. Trybuild case: `forge_an_interval` (E0423).
- **Model a one-way lifecycle as sealed typestate.**
  - Write it as `Line<S: sealed::State>`; each state is an uninhabited enum that derives `Debug`.
  - Transitions take `self` by value and are `#[must_use]`.
  - A method exists only in the impl block of the state where it is valid.
  - Sources: C-SEALED. Trybuild case: `push_after_complete` (E0599).
- **A fallible transition returns its input on failure:** `Result<Line<Complete>, Line<Building>>`, so the caller keeps what it gave up.
- **Use generative brands only when an index outlives its check and code that ships uses it.**
  - The pattern is an HRTB plus an invariant lifetime.
  - Cost in domain: the mint (`impl for<'id> FnOnce`) counts as `callback_param`, which needs a budget commit.
  - Cost in core: the mint fails `cargo xtask arch`.
  - With no unsafe, a brand buys correctness, not bounds-check removal.

## No `bool`

- **No `bool` parameter or field anywhere in domain or core, private ones included.** Use a two-variant enum that names both states: `Pushed {Taken, Refused}`, or core's private `Phase {Open, Ended}`.
  - Gate: `bool_param_or_field` = 0. The scanner counts a bare `bool` in a parameter of any fn, a field or a variant payload. A `bool` inside `Option`, `&` or a tuple slips past it, so review catches those.
  - DEPARTS from BeautifulRust and C-CUSTOM-TYPE (public signatures only): a private `bool` field is a state that a `match` cannot name.
- **Predicates may return `bool`** (`is_empty`, `Modes::has`). The gate does not count returns. DEPARTS from the pillar text ("`bool` in pub signatures"): a predicate answers a yes/no question, and an enum for it would only rename `true`.
- **Bound bitsets by their enum.** Use `Modes(u32)` over `enum Mode`. An exhaustive `const fn bit` match holds the bound: a new variant fails to compile until it has a bit, and a bit past 32 fails `overflowing_literals`, which is deny by default.
  - DEPARTS from the pillar text, which bounds bitsets with `variant_count`: `core::mem::variant_count` is nightly-only, and the template's toolchain is stable.
  - See [vocabulary §5](references/vocabulary.md#5-bitsets).

## Closed vocabulary

- **Use closed enums for input and output:** `Event<'a> {Bytes(Wire<'a>), Tick(Millis), End}` and `Effect {Line(LineRecord), Idle(Millis)}`. Their payloads are already-narrowed types. A primitive in a `pub` enum payload counts as `primitive_api`.
- **Parse once, in one place.** Bytes become meanings only in `Wire::classes` / `ByteClass::of`. Mark the parse `#[inline]` so it can fold into the caller's `match`.
- **Unknown input gets a variant that carries a newtype.** The core refuses it and counts it; it is never silently dropped.
- **Size buffers from the protocol's own maximum, and `const`-assert the size.** A size guessed by eye is not a size.
- **No `#[non_exhaustive]` on `Event`, `Effect` or any workspace-only enum:** a new variant must break every core `match`. The rule and its DEPARTS from BeautifulRust: rust-idioms (visibility and evolution).

## Stored layout

- **Make zero the empty value.**
  - Give the type a `ZERO` const. zerocopy `FromBytes` then makes all-zero bytes a valid empty store: one zeroed allocation, no initialisation loop, no `unsafe`.
  - An id that can be absent reserves 0 as a named `NONE` and returns `Option` from its accessor.
  - DEPARTS from BeautifulRust's "Option not sentinels", for storage only: a zero-filled store must be valid with no per-field tag and no initialisation loop.
- **Derive `FromBytes, IntoBytes, Immutable, KnownLayout` on stored POD** (`repr(C)` or `repr(transparent)`), and pin the size: `const _: () = assert!(size_of::<LineRecord>() == 16);`. `IntoBytes` rejects padding, so order fields so there is none.

## Prove it with `compile_fail`

- **Give every rule a `compile_fail` case, every case a passing twin, and every case a pinned stderr.**
  - Files: `crates/<name>-domain/tests/ui/fail/X.rs` and `X.stderr`, plus `crates/<name>-domain/tests/ui/pass/X.rs`.
  - `cargo xtask docs` checks that the twin and the `.stderr` exist; trybuild runs in `test:unit`.
  - "The twin differs only in the illegal line" is a review rule, not a gate.
- **Read the stderr before you pin it.** It must name your rule's error (E0308, E0599, E0423, E0277), not a typo's (E0425 unresolved name, E0432 unresolved import). A `compile_fail` case that calls a function that does not exist passes, for the wrong reason. See [compile-fail](references/compile-fail.md).

## How `cargo xtask budgets` holds it

| kind | counts (non-test code under `src/`, parsed with `syn`) | domain/core |
|---|---|---|
| `primitive_api` | raw `u8…u128`, `usize`, `i*`, `f32`/`f64`, `char` in a `pub fn` signature, a `pub` named field of a `pub` struct, a `pub` enum payload, `impl Trait<prim>`, an associated `type X = prim` in a trait impl | 0 |
| `seam` | a `pub fn` named in `[seams]` that carries a primitive | = count |
| `pub_newtype_field` | a tuple struct with a `pub` field | 0 |
| `bool_param_or_field` | a `bool` parameter on any fn; a `bool` field or variant payload | 0 |
| `as_cast` | every `as` | = count |
| `expect_attr` · `callback_param` · `unsafe_code` | an `#[expect(..)]` · an `Fn*`/sink parameter in a `pub` signature (pillar 2) · any `unsafe` (pillar 6) | 0 |

- **A budget equals the count.**
  - Over budget fails: something was added.
  - Under budget fails too ("fell to N: lower its budget"), because a budget above the count measures nothing.
  - Lower a budget in the same commit that lowers the count.
  - Raise one only in a commit of its own: the counted construct and its budget, nothing else, with the reason (`budgets.toml`'s "a commit of its own that says why"). The two travel together because the budget must equal the count at every commit; the feature that uses it comes in a later commit. Never raise one to turn a red gate green.
  - `cargo xtask budgets --print` prints the counts, ready to paste.
  - A positive-control fixture runs first; if it miscounts, the gate errors out instead of passing.
- **The scanner also counts** a `pub const`, `pub static` or free `pub type` of a primitive, and a primitive inside `impl Trait`, `dyn` or `fn(..)` (`-> impl Iterator<Item = u8>`, `impl Into<u32>`), all as `primitive_api` (rust-template `ea24734`). Make the item `pub(crate)` or give it a newtype.
- **Review what it still cannot see:** a `bool` nested in another type (`Option<bool>`).
