# Typestate and generative brands

Both techniques move a runtime check into a type. Typestate encodes a lifecycle. A brand encodes "this index was checked against that collection". Sources: the Rustonomicon chapters "PhantomData" and "Subtyping and Variance", and API Guidelines C-SEALED.

## 1. Sealed typestate: the template's `Line`

This excerpt comes from `crates/<name>-domain/src/line.rs`. It is abridged: the imports, `Pushed`, `starting_at` and `is_empty` are left out.

```rust
# use my_project_domain::{ByteClass, ByteLen, BytePos, LineRecord, Pushed, WordCount};
use core::marker::PhantomData;

mod sealed {
    /// Sealed, so no other crate can add a state.
    pub trait State {}
}

/// A line still taking bytes. Uninhabited: it exists only as a type.
#[derive(Debug)]
pub enum Building {}
/// A line that has ended. Uninhabited: it exists only as a type.
#[derive(Debug)]
pub enum Complete {}

impl sealed::State for Building {}
impl sealed::State for Complete {}

/// A line, in state `S`.
#[derive(Debug)]
pub struct Line<S: sealed::State> {
    start: BytePos,
    len: ByteLen,
    words: WordCount,
    prev: ByteClass,
    state: PhantomData<S>,
}

impl Line<Building> {
    /// Takes one non-newline byte's class.
    pub fn push(&mut self, class: ByteClass) -> Pushed {
        let Some(len) = self.len.checked_grow() else {
            return Pushed::Refused;
        };
        self.len = len;
        if class == ByteClass::Other && self.prev != ByteClass::Other {
            self.words = self.words.saturating_incr();
        }
        self.prev = class;
        Pushed::Taken
    }

    /// Ends the line. Consumes it: a completed line cannot take more bytes.
    #[must_use]
    pub const fn complete(self) -> Line<Complete> {
        Line {
            start: self.start,
            len: self.len,
            words: self.words,
            prev: self.prev,
            state: PhantomData,
        }
    }
}

impl Line<Complete> {
    /// The line as plain old data, for the host.
    #[must_use]
    pub const fn record(&self) -> LineRecord {
        LineRecord {
            start: self.start,
            len: self.len,
            words: self.words,
        }
    }
}
# fn main() {}
```

Each part has a job:

- **Uninhabited states (`enum Building {}`).** No value of the state can exist, so `PhantomData<S>` is zero-sized, and `Line<Building>` and `Line<Complete>` have the same size. The typestate costs nothing at run time.
  - The states still derive `Debug`, because the workspace denies `missing_debug_implementations`.
- **The sealed trait.** `sealed` is a private module holding a `pub trait`. Downstream crates can name the bound but cannot implement it, so the set of states is closed (C-SEALED).
  - Copy this pattern verbatim. It passes `unreachable_pub` as written.
- **Methods per state.** `push` exists only on `Line<Building>`. Calling it on a completed line is E0599, "no method named `push` found for struct `my_project_domain::Line<my_project_domain::Complete>`". Trybuild case `push_after_complete` pins that error.
- **Consuming transitions.** `complete(self)` moves the old state away, so nothing can push through a stale handle. `#[must_use]` makes a dropped transition a warning, and `unused` is denied.
- **Shared methods** go in `impl<S: sealed::State> Line<S>`.

### Typestate held in a struct field

A struct field has one type, so a machine that keeps a lifecycle across events keeps it in one state and swaps it out at the transition. Core's `Machine` does this:

```rust
# use core::mem;
# use my_project_domain::{Building, BytePos, Effect, Line};
# struct Machine { line: Line<Building>, pos: BytePos }
# impl Machine {
# fn emit(&mut self, _: Effect) {}
/// Ends the current line and starts the next at the current position.
fn finish_line(&mut self) {
    let done = mem::replace(&mut self.line, Line::starting_at(self.pos)).complete();
    self.emit(Effect::Line(done.record()));
}
# }
# fn main() {}
```

Some state varies with input and must persist across events in the same field: core's `Phase {Open, Ended}`, `Activity`, `Idle`. That is a private two-state enum, not typestate. Use typestate where the lifecycle runs inside one call chain or follows a value that moves. Use an enum field where the machine must remember the state.

### A fallible transition returns what it consumed

```rust
# use core::marker::PhantomData;
# pub enum Building {}
# pub enum Complete {}
# pub struct Line<S>(PhantomData<S>);
# impl Line<Building> {
#     pub const fn is_empty(&self) -> bool { true }
#     pub const fn complete(self) -> Line<Complete> { Line(PhantomData) }
# }
impl Line<Building> {
    /// Ends the line if it holds at least one byte.
    ///
    /// # Errors
    ///
    /// Returns the line unchanged when it is empty, so the caller keeps it.
    pub fn complete_nonempty(self) -> Result<Line<Complete>, Self> {
        if self.is_empty() { Err(self) } else { Ok(self.complete()) }
    }
}
# fn main() {}
```

Returning `Self` in the error follows BeautifulRust's "return the consumed argument in the error". Without it, a failed transition destroys the value. `# Errors` is required by `clippy::missing_errors_doc`, which is denied.

### Third states

A lifecycle with more than two steps, such as `Building → Complete → Sent`, adds one uninhabited enum and one `sealed::State` impl per state. Give each transition a verb (`complete`, `send`) and a `compile_fail` case for the most likely misuse.

## 2. Generative brands

A brand is a lifetime that no other value shares. A collection mints one per call, and every index made from that collection carries it. An index from collection A then fails to type-check against collection B. The technique is the one behind the `indexing` crate and the GhostCell paper.

Here is a generic example over the template's `LineRecord`:

```rust
# use my_project_domain::LineRecord;
# mod brand {
use core::marker::PhantomData;

use crate::LineRecord;

/// A lifetime no other value shares. `fn(&'id ()) -> &'id ()` is
/// contravariant in its argument and covariant in its result, so `'id` is
/// invariant: it can neither grow nor shrink, and two brands never unify.
/// An alias of `PhantomData`, so a field of this type is zero-sized and, like
/// `Line`'s `state`, is not reported by `dead_code` for never being read.
type Brand<'id> = PhantomData<fn(&'id ()) -> &'id ()>;

/// A read-only view of some line records, branded `'id`.
#[derive(Clone, Copy, Debug)]
pub struct Records<'id, 'a> {
    rows: &'a [LineRecord],
    brand: Brand<'id>,
}

/// A position checked against one `Records`, and valid in no other.
#[derive(Clone, Copy, Debug)]
pub struct Slot<'id> {
    at: usize,
    brand: Brand<'id>,
}

/// Mints a fresh brand for `rows` and lends the branded view to `f`.
pub fn with_records<'a, R>(
    rows: &'a [LineRecord],
    f: impl for<'id> FnOnce(Records<'id, 'a>) -> R,
) -> R {
    f(Records { rows, brand: PhantomData })
}

impl<'id, 'a> Records<'id, 'a> {
    /// Every slot of this view, in order: the only way to make a `Slot<'id>`.
    pub fn slots(self) -> impl Iterator<Item = Slot<'id>> {
        let brand = self.brand;
        (0..self.rows.len()).map(move |at| Slot { at, brand })
    }

    /// The record at `slot`. `slot` came from this view, so it is in range.
    #[must_use]
    pub fn record(self, slot: Slot<'id>) -> LineRecord {
        self.rows[slot.at]
    }
}
# }
# fn main() {
#     use my_project_domain::{ByteLen, BytePos, WordCount};
#     let rows = [LineRecord { start: BytePos::ZERO, len: ByteLen::ZERO, words: WordCount::ZERO }];
#     let n = brand::with_records(&rows, |r| r.slots().map(|s| r.record(s)).count());
#     assert_eq!(n, 1);
# }
```

Why it holds:

- **The higher-ranked bound.** `for<'id>` makes `f` work for every `'id`, so the caller can neither choose `'id` nor name it.
- **`R` cannot carry the brand.** `R` is chosen outside the closure and cannot mention `'id`, so no `Slot<'id>` escapes the call.
- **Invariance.** Two nested `with_records` calls produce two `'id`s that the compiler will not unify, so `outer.record(inner_slot)` is a lifetime error.
- **Pin the error.** Write that program as a `compile_fail` case and pin whatever rustc reports ([compile-fail](compile-fail.md)). Do not guess the error code.
  - When this review checked the snippet, rustc 1.98 reported E0521 "borrowed data escapes outside of closure" for `with_records(a, |outer| with_records(b, |inner| inner.slots().next().map(|s| outer.record(s))))`.
  - Re-pin it on your own toolchain.
- **Rust 2024 capture.** In Rust 2024, return-position `impl Trait` captures all in-scope lifetimes, so `slots` needs no `+ use<..>` bound.

### What a brand does not buy here

`record` still indexes with a bounds check. Without `unsafe`, which pillar 6 forbids outside a quarantine crate, a brand cannot call `get_unchecked`. Its value in this workspace is correctness: an index cannot be used against the wrong snapshot. Any speed claim needs a measurement (rust-measure).

### What a brand costs here

- **In a `domain` crate:** the scanner counts the mint's `impl for<'id> FnOnce(..)` parameter as `callback_param`. `FnOnce` is not one of its data traits. Domain is held to budget 0 on it, so a brand needs `callback_param = 1` in `budgets.toml`, raised in a commit of its own that says "brand mint, not an effect callback".
- **In a `core` crate:** `cargo xtask arch` rejects any callback in core's `pub` API with no budget to raise, because pillar 2 says effects are polled, not called back. Mint brands in the domain crate. Core can then use branded types it receives.
- **Ergonomics:** every user of the collection runs inside a closure, and error messages are lifetime errors.

### When a brand is worth it

All of these must hold:

1. **An index outlives the check that made it.** It is stored, or passed through several functions.
2. **Mixing up collections is a realistic bug.** There are several snapshots, generations or tables of the same element type alive at once.
3. **Code that ships uses it,** not only tests. Brands and typestate used only by tests pay the costs above for nothing.

Otherwise, use a narrowing constructor ([newtypes §5](newtypes.md#5-declared-operators-only-where-total)). Make the proof type only from the collection's own method, and accept that two collections with the same element type can still be confused.
