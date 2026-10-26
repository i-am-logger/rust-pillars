# Newtypes: worked examples

The examples come from rust-template's `crates/<name>-domain/src/quantity.rs`, a line and word counter vocabulary. Section 5 adds a generic example: the template declares no operators.

## 1. Anatomy of one quantity

```rust
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

/// The length of a line, in bytes, newline excluded. Stored as `u32`: a line
/// longer than 4 GiB is refused and counted, never wrapped.
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, FromBytes, IntoBytes, Immutable, KnownLayout,
)]
#[repr(transparent)]
pub struct ByteLen(pub(crate) u32);

impl ByteLen {
    /// An empty line.
    pub const ZERO: Self = Self(0);

    /// One byte longer, or `None` at the top of the range.
    #[must_use]
    pub const fn checked_grow(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(n) => Some(Self(n)),
            None => None,
        }
    }

    /// The raw length, for interop. The only way out.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}
```

What each part does:

| Part | Why |
|---|---|
| `pub(crate) u32` | Other crates cannot build or read it; the domain crate's own modules can (`line.rs` builds `ByteLen`s). The gate `pub_newtype_field` stays at 0. |
| `repr(transparent)` | `ByteLen` is a field of `LineRecord`, which a host writes as bytes, so its layout is part of the contract. |
| zerocopy derives | Needed for the derives on `LineRecord`. An all-zero `ByteLen` is `ZERO`, so a zeroed buffer is valid. |
| No `Default`, `Hash` or `From<u32>` | Nothing asked for them. `Default` would invent a value; `From<u32>` would make every integer a length (trybuild case `integer_into_quantity`, E0277). |
| `ZERO` const, not `Default` | The empty value has a name at the call site. |
| `checked_grow` in a `match` | `?` and `Option::map` are not usable in a `const fn` on stable; a `match` is. |
| `get` | The one exit. Its name is in `[seams]`, so it counts as `seam`, not as `primitive_api`. |

## 2. The template's quantities

Every type below derives `Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug`. None derives `Hash` or `Default`, and none implements `From<integer>`.

| Type | Inner | Layout | Entries | Exit | Why this shape |
|---|---|---|---|---|---|
| `BytePos` | `u64` | transparent + POD | `ZERO`, `saturating_advance(ReadLen)` | `get` | A stream position. It is stored in `LineRecord`, and a 2^64-byte stream saturates by name. |
| `ReadLen` | `usize` | none | `of(&[u8])`, `ONE` | none | One read is any slice length, so its type is wide; it is never stored. |
| `ByteLen` | `u32` | transparent + POD | `ZERO`, `checked_grow` | `get` | Stored narrow. At the top of its range it refuses (`Pushed::Refused`). |
| `WordCount` | `u32` | transparent + POD | `ZERO`, `saturating_incr` | `get` | A count that the host reads. |
| `Millis` | `u64` | none | `ZERO`, `from_host(u64)` | none | Host clock time, entering as an event (pillar 2). Only the host makes one. |
| `Elapsed` | `u64` | none | `Millis::since`, `threshold_ms(u64)` | none | A proof type: it comes from a subtraction or a named setting. Core only compares it (`>=`). |
| `Tally` | `u64` | none | `ZERO`, `saturating_incr`, `saturating_add` | `get` | A refusal counter (`Refusals`). |

A "none" in the Layout column is a gap against pillar 1, which puts `repr(transparent)` on every quantity. No gate checks it, so add the attribute when you touch the type. A layout guarantee costs nothing, and a later move into a POD record then changes nothing.

## 3. Count the seams yourself

`budgets.toml` holds `seam = 8` for the domain. Each item below is a `pub fn` whose name is in `[seams] names` and whose signature mentions a primitive:

1. `BytePos::get`
2. `ByteLen::get`
3. `WordCount::get`
4. `Tally::get`
5. `ReadLen::of(&[u8])`: `&[u8]` contains `u8`.
6. `Wire::of(&'a [u8])`
7. `Millis::from_host(u64)`
8. `Elapsed::threshold_ms(u64)`

`ByteClass::of(u8)` is `pub(crate)` and not counted: the scanner counts `pub` items only, though `bool` parameters are counted on every fn.

A seam named like this is visible in review. Naming a primitive-returning fn `get` to dodge `primitive_api` gains nothing: it moves the hit to `seam`, whose budget then breaks.

## 4. Store narrow, compute wide: `ReadLen` vs `ByteLen`

```rust,ignore
impl BytePos {
    /// The position `by` bytes further on. Saturates at the end of the range,
    /// and says so in its name: a stream of 2^64 bytes is refused, not wrapped.
    #[must_use]
    pub const fn saturating_advance(self, by: ReadLen) -> Self {
        // `usize` is at most 64 bits on every target this builds for, so the
        // widening is exact; the one `as` here is counted by `xtask budgets`.
        Self(self.0.saturating_add(by.0 as u64))
    }
}
```

Excerpt of `crates/<name>-domain/src/quantity.rs`, compiled there: it reads the private fields of `BytePos` and `ReadLen`, so it does not compile outside that crate.

- **Two types, two ranges.**
  - A read is as long as the host's buffer (`usize`); a line we keep is `u32`.
  - A single `Len` type would force one of two outcomes: every stored record pays for `usize`, or every read is truncated to `u32`.
- **Why this `as` stays.**
  - `u64: From<usize>` does not exist, because the standard library leaves room for a wider `usize`.
  - `u64::try_from` is not `const`, and it is fallible where the operation cannot fail.
  - So the cast is counted (`as_cast = 2`: this one, plus the class table's `i as u8` under `i < 256`).
  - To back the comment with a check, add `const _: () = assert!(usize::BITS <= 64);` next to it.
- **The other direction.** `usize::from(b)` (from `u8`) in `ByteClass::of` is lossless and has no cast to count. Prefer `From` wherever it exists.
- **Arithmetic width.** A type used for indexing or repeated adding is `usize`, so every use runs at full width without a cast. A `u16` cursor adds a zero-extension or a mask to each operation.

## 5. Declared operators, only where total

Saturating and checked steps get names (§6). An operator is for the case where the result provably fits. Here is a generic grid example; no type has a `pub` field, and there is no cast:

```rust
use core::ops::Add;

// `Col + Room` is total only if two `u16`-bounded values fit in a `usize`.
const _: () = assert!(usize::BITS >= 32, "Col + Room needs a 32-bit usize");

/// A row's width. Stored narrow: no row is wider than `u16::MAX`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(transparent)]
pub struct Cols(pub(crate) u16);

/// A column. Wide (`usize`) for arithmetic, but never above `u16::MAX`:
/// only `Cols` makes one.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(transparent)]
pub struct Col(pub(crate) usize);

/// Columns from a column to the end of its row. Only `Cols::room_at` makes
/// one, and `at_most` can only shrink it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(transparent)]
pub struct Room(pub(crate) usize);

/// One past the last column of a span. It has no arithmetic, so a sum
/// cannot be summed again and the bound cannot compound.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(transparent)]
pub struct End(pub(crate) usize);

impl Cols {
    /// The first column, or `None` for a zero-width row.
    #[must_use]
    pub const fn first(self) -> Option<Col> {
        if self.0 == 0 { None } else { Some(Col(0)) }
    }

    /// The column after `col`, or `None` at the end of the row.
    #[must_use]
    pub fn after(self, col: Col) -> Option<Col> {
        // `col.0 <= u16::MAX`, so `+ 1` cannot overflow a 32-bit `usize`.
        let next = col.0 + 1;
        if next < usize::from(self.0) { Some(Col(next)) } else { None }
    }

    /// The room left from `col` to the end of the row; none past it.
    #[must_use]
    pub fn room_at(self, col: Col) -> Room {
        Room(usize::from(self.0).saturating_sub(col.0))
    }
}

impl Room {
    /// At most `cap` columns of this room. It can only shrink, which keeps
    /// the proof "this fits" intact.
    #[must_use]
    pub const fn at_most(self, cap: Self) -> Self {
        if cap.0 < self.0 { cap } else { self }
    }
}

/// Total: `Col` and `Room` are each at most `u16::MAX`, and their sum fits the
/// asserted 32-bit `usize`. The result is an `End`, not a `Col`.
impl Add<Room> for Col {
    type Output = End;

    fn add(self, room: Room) -> End {
        End(self.0 + room.0)
    }
}
```

What makes the operator total:

- **Bounded constructors.** Every way to make a `Col` or a `Room` keeps it at or below `u16::MAX`.
- **A wide representation.** The const assert pins the platform assumption: `thumbv7em-none-eabihf`, one of the `test:bare` targets, has a 32-bit `usize`.
- **A non-compounding output.** `End` has no `Add`, so the bound cannot be stacked.

Without all three, write `saturating_add` or `checked_add` instead. An operator that can overflow behaves differently by profile: under `[profile.qa]` (`overflow-checks = true`, used by `test:unit`) it panics, and under plain `release` it wraps. An operator also hides what the arithmetic does: `a + b` does not say that it saturates, and it does not name the quantity.

Note what the operator does not prove. `col_b + room_from_col_a` still type-checks, and the result can pass the end of the row; it cannot overflow. To tie a `Room` to the column it was measured from, use a brand ([typestate-and-brands](typestate-and-brands.md)), or have `room_at` return the `End` directly.

## 6. Name the failure mode

| Method shape | Use when | Template example |
|---|---|---|
| `checked_x(self) -> Option<Self>` | The caller must decide, usually by refusing and counting. | `ByteLen::checked_grow`, which `Line::push` turns into `Pushed::Refused` |
| `saturating_x(self) -> Self` | The top of the range is the right answer, and any loss is counted or harmless. | `WordCount::saturating_incr`, `Tally::saturating_add`, `BytePos::saturating_advance` |
| `at_most` / `clamped_to(self, bound)` | Another value bounds this one. | `Room::at_most` (§5) |
| `since(self, earlier) -> Elapsed` | A subtraction whose sign is a domain fact. A clock that stepped back reads as no time. | `Millis::since` (saturating) |
| `wrapping_x` | Only for quantities that are modular by definition: sequence numbers, hashes. Never as a shortcut. | none |

Put `#[must_use]` on all of them. `Option` is not `#[must_use]` by itself, and dropping a `checked_` result loses the refusal.

## 7. Adding a quantity: checklist

1. **Name it by its meaning**, not its unit or representation. `WordCount`, not `U32Count`.
2. **Write down the range and the uses.** The range decides the stored width. Uses for indexing or repeated adding decide the arithmetic width. If the two differ, make two types and one named conversion between them.
3. **List every source.** Bytes from a wire or a store enter through the derived `FromBytes::read_from_bytes`, which counts nothing. A host value goes through an existing seam. A new `pub fn` that takes a raw integer is a new seam, whatever its name: it raises the count, so it goes in a budget commit of its own that says why no derive or existing seam works, and never in the change that needs it.
4. **Add an exit only if a caller outside the domain needs the integer**: a host, a format, an index into foreign data.
5. **Choose derives** with section 2's table as the baseline. Add `Hash` only for keys, `Default` only for the empty value, never `From<int>`.
6. **Add `repr(transparent)`.** If the type is stored, also add the zerocopy derives, and check that the record holding it has no padding.
7. **Test at the extremes** in `#[cfg(test)] mod tests` (test code is not counted). `quantity.rs` checks `u32::MAX` and `u64::MAX`, and a clock stepping backwards.
8. **Add a `compile_fail` case and its twin** for the misuse the type exists to stop ([compile-fail](compile-fail.md)).
9. **Run `cargo xtask budgets --print`.** A new type that enters by a derive leaves every count where it was. If a count rose, remove what raised it. A raise is never the fix for a red gate: it is a commit of its own, holding only the counted construct and its budget, that says why.

## 8. What the gates catch

| Written | Counted as |
|---|---|
| `pub struct Id(pub u32);` | `pub_newtype_field` |
| `impl From<u32> for ByteLen` | `primitive_api` (a trait generic over a primitive) |
| `impl Deref for ByteLen { type Target = u32; .. }` | `primitive_api` (an associated type that is a primitive) |
| `pub fn len(&self) -> u32` | `primitive_api`, unless the name is a seam |
| `pub struct Stats { pub lines: u64 }` | `primitive_api` (a `pub` named field) |
| `fn push(&mut self, last: bool)`, even private | `bool_param_or_field` |
| `struct State { done: bool }`, even private | `bool_param_or_field` |
| `n as u32` | `as_cast` |
| `#[expect(clippy::x, reason = "..")]` in domain or core | `expect_attr` |

In a trait impl, the methods' primitives, seams and callbacks are not counted, because the trait fixes those signatures. Their `bool` parameters and any `unsafe fn` are still counted. The impl's own generic arguments and its associated types are counted too.

Since rust-template `b4d5da6` the scanner also counts `pub const`, `pub static`, a free `pub type`, and a primitive inside `impl Trait`, `dyn` or `fn(..)` (`-> impl Iterator<Item = u8>`), all as `primitive_api`. A `bool` nested in another type (`Option<bool>`) is still review's.
