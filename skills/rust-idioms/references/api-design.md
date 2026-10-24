# API design: signatures, traits, constructors, visibility

Load this before adding or changing a public item. Rule ids such as C-NEWTYPE are from the
Rust API Guidelines checklist. "Gate" names what fails `devenv test`; "review" means nothing
enforces the rule yet.

## 1. Which rules apply

| Kind | API audience | What changes |
|---|---|---|
| Domain | the workspace's own core and adapters | No primitive in the public API (rust-domain). Derives follow meaning. `const fn` wherever possible. |
| Core | adapters and bins | Same as domain, plus no callbacks, no generic hot `impl`s, effects polled, and no allocation per event. |
| Adapter / bin | this workspace only | std types are fine (`Duration`, `PathBuf`, `NonZeroUsize`). Ergonomics come before micro-cost. |
| Library for others | strangers on crates.io | The full checklist: semver, `#[non_exhaustive]`, `C-STABLE`, MSRV policy. |

Inside one workspace a "breaking change" is a compile error you fix in the same commit. Rules
that exist only for semver (`#[non_exhaustive]`, "`const fn` only if it stays const") apply to
the last row only.

## 2. Parameters and returns

- Borrow the most general form: `&str` over `&String`, `&[T]` over `&Vec<T>`, and
  `impl AsRef<Path>` in adapters. Gate: clippy `ptr_arg`.
- Take `T` by value only when storing or consuming it. If you need ownership "sometimes", the
  caller should decide (C-CALLER-CONTROL). Adapters may accept `impl Into<String>` for a value
  that is stored.
- Return a tuple or a small named struct, never an out-parameter (C-NO-OUT). A caller-lent
  `&mut [T]` buffer on a hot path is not an out-parameter: it is the zero-copy contract
  (rust-architecture).
- Keep `Rc`, `Arc`, `Box` and `RefCell` out of public signatures. Expose the value or a
  borrow; how it is shared is the caller's business.
- Use a consistent parameter order across a module: subject first, then inputs, then settings.
- Four or more loose arguments mean a missing type. Group them into a struct with named
  fields or a builder. Gate: clippy `too_many_arguments` fires at 8; group earlier on review.
- Domain and core: no raw integer, float or `char` in a public signature or public field,
  except through a named seam (`budgets.toml [seams]`: `get`, `of`, `from_host`,
  `threshold_ms`). No `bool` parameter or field at all, private ones included: it becomes a
  two-variant enum that names what happened (`Pushed { Taken, Refused }`). A predicate's
  `bool` return stays (`Wire::is_empty`; `len_without_is_empty` asks for it).
  - DEPARTS from BeautifulRust, which bans only `bool` parameters, and generalises C-CUSTOM-TYPE.
  - Gate: `cargo xtask budgets` (`primitive_api`, `bool_param_or_field`, `seam`).
- Core takes no `impl Fn`, `dyn Fn` or `fn(..)` parameter, and no sink trait object: effects
  are queued and polled. Gate: `cargo xtask arch`, `callback_param` budget.

## 3. Standard traits

| Trait | Rule | Gate |
|---|---|---|
| `Debug` | Every public type has it (C-DEBUG). Write it by hand to redact secrets. | `missing_debug_implementations` |
| `Clone`, `Copy` | `Copy` only for small plain values; a large `Copy` is a hidden memcpy. | review |
| `PartialEq`/`Eq`/`Ord`/`Hash` | Derive what the meaning supports. Hand-written ones must agree. | clippy `derived_hash_with_manual_eq`, `derive_ord_xor_partial_ord` |
| `Default` | Only if one value is the obvious default, and it agrees with `new()`. | clippy `new_without_default` |
| `Display` | Only for user-facing text. Error messages are lowercase with no trailing period. | review |
| `From`/`TryFrom` | Implement `From`, never `Into`. Invariant types get only `TryFrom` or a fallible constructor. | clippy `from_over_into` |
| `FromStr`, `FromIterator`, `Extend` | Implement for collections and parsable values (C-COLLECT). | review |
| `IntoIterator` | For `T`, `&T` and `&mut T` where iteration makes sense (C-ITER). | review |
| `Deref` | Smart pointers only (C-DEREF); never "inheritance". | review |
| `std::ops::*` | Only with their mathematical meaning (C-OVERLOAD). In domain, only where the result cannot leave its range (rust-domain). | clippy `should_implement_trait` |
| `Send`/`Sync` | Public types and futures are `Send` where possible (C-SEND-SYNC). Assert it. | the const assertion below |

DEPARTS from BeautifulRust's "derive eagerly" and C-COMMON-TRAITS for domain and core (rust-domain owns the per-quantity rules). A derive is a promise.
`Default` on a quantity invents a value nobody chose, `From<u32>` reopens the door that
primitive refusal closed, and `Hash` on a length is never used. The template's quantities
(`crates/<name>-domain/src/quantity.rs`) derive per type, and explain each choice in the
module doc. For libraries for others, implement eagerly.

```rust
use std::fmt;

/// An API token. `Debug` is written by hand so it never prints the secret.
pub struct Token(String);

impl Token {
    /// The secret, for the one call that sends it.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(<redacted>)")
    }
}
```

Compile-time `Send`/`Sync` check, placed next to the type it guards. Excerpt: `Machine<EFFECTS>`
stands for your type, with any const or type parameters filled in.

```rust,ignore
const _: () = {
    const fn send_and_sync<T: Send + Sync>() {}
    send_and_sync::<Machine<EFFECTS>>();
};
```

## 4. `#[must_use]`

Put it on:
- outcome types whose loss is a bug. The template marks the enum itself
  (`Pushed`, `Queued`), so every function returning one is covered.
- pure functions and getters (the template's `get()`, `since()`, `saturating_incr()`).
- builders and `with_*` methods that return a new value.

It is not a gate (`must_use_candidate` is pedantic and off), so check it on review.

## 5. Constructors

- `new` is the primary constructor, a static inherent method (C-CTOR). Variants are
  `with_capacity`, `try_new` (fallible) and `from_<source>`.
- Domain entry points are named after where the value comes from (`Wire::of(bytes)`,
  `Millis::from_host(ms)`, `Elapsed::threshold_ms(ms)`). Each one is a counted seam, so a new
  way in is a reviewed change.
- Narrowing constructors return `Option` or `Result` (`ByteLen::checked_grow`). A
  "proof" type has no path that widens it again.
- A zero-argument `new` needs an agreeing `Default` (clippy `new_without_default`, a gate). The
  template's `Effects<N>` has both. Its quantities have neither: they expose `ZERO`, so the
  empty value is named at every use.
- One or two optional settings: use `with_*` methods. More: use a builder.

Builder (adapter code; std types allowed):

```rust
use std::error::Error;
use std::fmt;
use std::num::NonZeroUsize;
use std::time::Duration;

/// Read size used when none is set. A `const` is evaluated at compile time,
/// so a zero here fails the build, not a run.
const DEFAULT_CHUNK: NonZeroUsize = match NonZeroUsize::new(64 * 1024) {
    Some(n) => n,
    None => panic!("the default chunk is not zero"),
};

/// How the stdin reader is configured.
#[derive(Clone, Debug)]
pub struct ReaderConfig {
    chunk: NonZeroUsize,
    idle_after: Duration,
}

impl ReaderConfig {
    /// Bytes per read.
    #[must_use]
    pub const fn chunk(&self) -> NonZeroUsize {
        self.chunk
    }

    /// The quiet spell that is reported.
    #[must_use]
    pub const fn idle_after(&self) -> Duration {
        self.idle_after
    }
}

/// Builds a [`ReaderConfig`]; every setting is optional.
#[derive(Clone, Debug, Default)]
pub struct ReaderConfigBuilder {
    chunk: Option<NonZeroUsize>,
    idle_after: Option<Duration>,
}

/// Why a [`ReaderConfig`] was refused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuildConfigError {
    /// An idle threshold of zero would report idle after every read.
    ZeroIdle,
}

impl fmt::Display for BuildConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroIdle => f.write_str("idle threshold is zero"),
        }
    }
}

impl Error for BuildConfigError {}

impl ReaderConfigBuilder {
    /// Reads this many bytes at a time.
    pub fn chunk(&mut self, chunk: NonZeroUsize) -> &mut Self {
        self.chunk = Some(chunk);
        self
    }

    /// Reports a quiet spell after `after` without input.
    pub fn idle_after(&mut self, after: Duration) -> &mut Self {
        self.idle_after = Some(after);
        self
    }

    /// The configuration.
    ///
    /// # Errors
    ///
    /// [`BuildConfigError::ZeroIdle`] if the idle threshold was set to zero.
    pub fn build(&self) -> Result<ReaderConfig, BuildConfigError> {
        let idle_after = self.idle_after.unwrap_or(Duration::from_secs(1));
        if idle_after.is_zero() {
            return Err(BuildConfigError::ZeroIdle);
        }
        Ok(ReaderConfig {
            chunk: self.chunk.unwrap_or(DEFAULT_CHUNK),
            idle_after,
        })
    }
}
```

The setters take `&mut self` and return `&mut Self`, so the builder works both as a one-line
chain and when configured step by step in a loop. `build(&self)` lets the same builder be reused.

## 6. Visibility and evolution

- Keep items private by default, and use `pub(crate)` before `pub`. Gate: `unreachable_pub`
  (deny) flags a `pub` item that no outside crate can reach.
- Struct fields are private (C-STRUCT-PRIVATE). A domain newtype's field is `pub(crate)`, never
  `pub`, so the only way out is the named exit. Gate: `pub_newtype_field` budget (0).
- `#[non_exhaustive]` on public enums and structs applies to libraries for others only.
  - Inside the workspace it forces a `_ =>` arm in every consumer, which turns "you forgot the
    new variant" from a compile error into silence.
  - The template's `Event` and `Effect` are deliberately exhaustive.
- Sealed traits (C-SEALED) are for traits that others may name or bound on but not implement.
  The pattern, adapted from `crates/<name>-domain/src/line.rs` (`Phase` is added here):

```rust
mod sealed {
    /// Sealed, so no other crate can add a state.
    pub trait State {}
}

/// A line still taking bytes. Uninhabited: it exists only as a type.
#[derive(Debug)]
pub enum Building {}

impl sealed::State for Building {}

/// A line's lifecycle state. Other crates can name and bound on it, but
/// cannot implement it: its supertrait is unreachable from outside.
pub trait Phase: sealed::State {}

impl Phase for Building {}
```

The template bounds on the sealed trait directly (`pub struct Line<S: sealed::State>`), which
seals the set of states without a public trait.

- Iterator returns: `impl Iterator<Item = T> + 'a` inside the workspace (`Wire::classes`).
  Libraries for others name the types `Iter`, `IterMut` and `IntoIter` (C-ITER-TY), so callers
  can store them.
- Give each public item one path. Re-export by name, never with a glob (`pub use foo::*`).
  - A facade crate may re-export a dependency's vocabulary by name so hosts depend on one
    crate. The template's `my-project-core` re-exports `Effect`, `Event`, `Millis` and others
    from the domain.
  - Do not also expose the same item under a second path.
- Features are additive, with bare names (`alloc`, `serde`, never `use-serde` or `with-std`;
  C-FEATURE). serde support goes behind a `serde` feature (C-SERDE).
  - Measurement arms are `cfg(arm = "...")` with check-cfg, not features (planned: the
    template has no arm yet). An arm that turns work off is not additive.
  - Gate: `test:features` (`cargo hack check --each-feature`).
- `const fn`:
  - Domain and core: make constructors and tables `const` (`Machine::new`, `Effects::new` and
    the `ByteClass` table are), so a host can build them in a `static`.
  - Libraries for others: only commit to `const` if it will stay const, because removing it
    is breaking.

## 7. Traits, generics, macros

- Write a trait only for generic programming or for extension by others; never for "testability"
  of pure code. A sans-IO core is tested by feeding events, not by mocking.
- Prefer, in order: a concrete type, then generics, then `dyn Trait` wrapped in a struct you own.
  - Domain and core: keep hot `impl` blocks non-generic. Use generics for cold types only
    (rust-architecture).
  - Why: code placement moves hot-path speed, and monomorphising a hot `impl` emits a new
    copy of hot code at a new address (inferred; unmeasured here).
- No nested generics (`Foo<Bar<Baz<T>>>` in a public signature): name the composite type.
- Decide object safety when the trait is created (C-OBJECT); adding a generic method later
  breaks every `dyn` user.
- Keep required methods minimal, and provide the rest as default methods built on them.
- Use macros last: a function or generic first, then `macro_rules!`, then a proc macro. Hand-written
  newtypes beat a newtype macro, because each derives only what it means (`quantity.rs` module doc).
