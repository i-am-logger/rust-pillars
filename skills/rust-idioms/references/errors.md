# Errors and panics

Load this before writing an error type, a `Result`-returning public fn, an `expect`, or a
teardown path. Error shape depends on the kind of code. One rule set for everything is how
`String` errors end up in hot loops and `Box<dyn Error>` ends up in `no_std`.

## 1. The decision: absence, failure, bug

| Situation | Use | Why |
|---|---|---|
| A value may normally be missing (lookup, `checked_*`) | `Option<T>` | Absence is not a failure; there is nothing to explain. |
| The caller can act on the failure (retry, report, choose another path) | `Result<T, E>` | The type forces a decision at the call site. |
| An invariant the code itself guarantees is broken | panic (`expect`, `assert!`, `unreachable!`) | A bug cannot be handled by the caller; continuing corrupts state. |
| Input the core cannot keep (full queue, over-long line, event after end) | refuse and count | The host stays fast and still learns what it lost. |

Panics are not exceptions. Never use `catch_unwind` for control flow, and never panic on input
that a caller can produce.

## 2. Per kind

### Core (`no_std`, hot): refuse and count first

The template's core never returns an error from its event entry point:
- `Machine::handle(&mut self, Event)` returns `()`.
- What it cannot keep becomes an outcome enum: `Pushed { Taken, Refused }` in the domain,
  `Queued { Taken, Dropped }` in core. Both are `#[must_use]` on the type, so the caller cannot
  ignore them.
- The caller turns an outcome into a saturating `Tally` in `Refusals`, which the host reads
  with `refused()` (`crates/<name>-core/src/machine.rs`).

Why: an error return on a per-byte path costs a branch and a decision at every caller, and
there is usually nothing to do except count. Counting keeps the hot path straight-line and makes
loss visible: the bin logs `refused()` when its `total()` is not zero.

When a real `Result` is needed (a fallible narrowing at a seam), use a `Copy` enum with no
payload:

```rust
#![no_std]

use core::fmt;

/// One read's bytes, borrowed. The entry seam is `of`.
#[derive(Clone, Copy, Debug)]
pub struct Bytes<'a>(&'a [u8]);

impl<'a> Bytes<'a> {
    /// A host's bytes.
    #[must_use]
    pub const fn of(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }
}

/// The largest payload the frame buffer holds.
const MAX_PAYLOAD: u16 = 1024;

/// A payload length already checked against the frame buffer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PayloadLen(u16);

/// Why a header was refused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParseHeaderError {
    /// Fewer bytes than the length field needs.
    Short,
    /// A length larger than the frame buffer.
    TooLong,
}

impl fmt::Display for ParseHeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Short => "header shorter than its length field",
            Self::TooLong => "payload length exceeds the frame buffer",
        })
    }
}

impl core::error::Error for ParseHeaderError {}

impl PayloadLen {
    /// Reads the big-endian length field at the start of `bytes`.
    ///
    /// # Errors
    ///
    /// [`ParseHeaderError::Short`] if fewer than two bytes arrived, and
    /// [`ParseHeaderError::TooLong`] if the length exceeds the frame buffer.
    pub fn parse(bytes: Bytes<'_>) -> Result<Self, ParseHeaderError> {
        let &[hi, lo, ..] = bytes.0 else {
            return Err(ParseHeaderError::Short);
        };
        match u16::from_be_bytes([hi, lo]) {
            n if n > MAX_PAYLOAD => Err(ParseHeaderError::TooLong),
            n => Ok(Self(n)),
        }
    }
}
```

What this enum does and why:
- It is `Copy` and has no payload. Without `alloc` there is no `String` or `Box` to carry,
  and a one-byte enum returns in a register.
  - Context such as "where" belongs to the caller, which knows the position.
  - Do not add `#[non_exhaustive]` inside the workspace (api-design.md §6).
- It implements `core::error::Error` (stable since Rust 1.81; the template's MSRV is 1.85), so an
  adapter can wrap it as a `#[source]` with no conversion shim.
- It does not use `thiserror`.
  - The reason is the layering rule, not `no_std`: thiserror 2 is `#![no_std]` with an
    optional, default-on `std` feature.
  - `layers.toml` gives core `external = []` and domain only `zerocopy`, so the core's whole
    dependency surface is reviewed in one place. Gate: `cargo xtask layers`.

DEPARTS from BeautifulRust's "library errors implement `Error + Send + Sync + 'static` with
`source()` and a `From<Upstream>` per cause". That rule assumes std and payloads. In a no_std
core, a payload-free `Copy` enum gives up nothing a hot caller can use. BeautifulRust's rules
hold for adapters, bins and libraries for others.

### Domain

- Narrowing that can only fail one way returns `Option`: `ByteLen::checked_grow() -> Option<Self>`.
- Saturating and clamping behaviour is named in the method (`saturating_incr`,
  `saturating_advance`), never hidden in an operator (rust-domain).
- Use a `Copy` error enum like the one above when there is more than one reason to refuse.

### Adapter and bin (std, I/O)

- Use one `thiserror` enum per binary or adapter boundary, with one variant per thing that
  failed and the upstream error as `#[source]`. Excerpt from `crates/<name>/src/main.rs`:

```rust,ignore
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("reading stdin")]
    Read(#[source] io::Error),
    #[error("writing stdout")]
    Write(#[source] io::Error),
}
```

- Messages are lowercase and have no trailing punctuation. They say what was being done
  ("reading stdin"), not that it failed, because they are chained as
  `outer: inner: innermost` when printed.
- Do not repeat the source's message in your own. Print the chain once, at the top
  (`main` logs `{e}` and then its `source()`).
- Map errors explicitly at the call (`.map_err(Error::Read)?`) when two variants share a
  source type. A blanket `#[from] io::Error` would make reads and writes indistinguishable.
- `main` returns `ExitCode` and logs. It does not `unwrap` a `Result` into a panic message.
- Tools (`xtask`, role `tool`) use `anyhow::Result` with `.context("...")`/`.with_context(|| ...)`.
  Nobody matches on a tool's error; a person reads it.
- Use `anyhow` or `eyre` in a bin only if nothing branches on the error. The template's bin uses
  a two-variant `thiserror` enum whose variants name what was being done.

### Library for others

- The error type implements `std::error::Error + Send + Sync + 'static`, with `source()`
  returning the cause (C-GOOD-ERR).
  - Never use `String`, `()` or `Box<dyn Error>` as the public error type. Callers cannot
    match on them.
- Either an enum per cause (`#[non_exhaustive]`), or a struct with a private kind and a
  `kind()` accessor when the representation must stay free to change:

```rust
use std::error::Error;
use std::fmt;
use std::io;

/// What went wrong loading a config file.
#[derive(Debug)]
pub struct LoadConfigError {
    kind: LoadConfigErrorKind,
    source: Option<io::Error>,
}

/// The kinds of [`LoadConfigError`], for callers that branch on the cause.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum LoadConfigErrorKind {
    /// The file could not be read.
    Unreadable,
    /// The file was read but is not a config.
    Malformed,
}

impl LoadConfigError {
    /// Which kind of failure this is.
    #[must_use]
    pub fn kind(&self) -> LoadConfigErrorKind {
        self.kind
    }
}

impl fmt::Display for LoadConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            LoadConfigErrorKind::Unreadable => "reading the config file",
            LoadConfigErrorKind::Malformed => "parsing the config file",
        })
    }
}

impl Error for LoadConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.source {
            Some(e) => Some(e),
            None => None,
        }
    }
}
```

- Write `source()` with a `match`, not `.map(|e| e as _)`. Why: the coercion needs no `as`,
  and every `as` counts against the `as_cast` budget.
- Implement `From<Upstream>` only when exactly one variant can come from that upstream type.
- If an operation consumed an argument and failed, give the argument back in the error.
  std does this with `String::from_utf8` (`FromUtf8Error::into_bytes`) and `mpsc::SendError<T>`.
- Never put `anyhow` in a public API. It erases the type the caller would match on.

## 3. Panics

- No `unwrap()` outside tests. Gate: clippy `unwrap_used`; `clippy.toml` sets
  `allow-unwrap-in-tests` and `allow-expect-in-tests`.
- Use `expect("...")` with the reason the value is present ("counter lock poisoned",
  "the default chunk is not zero"), not a restatement ("failed to unwrap").
- Prefer `let ... else` to `match` with an early return: `let Some(len) = self.len.checked_grow() else { return Pushed::Refused; };`.
- Treat lock poisoning as fatal: `m.lock().expect("... lock poisoned")`. A poisoned lock means
  another thread panicked mid-update; the data is suspect.
- Prefer compile-time failure to a runtime panic:
  - `const _: () = assert!(size_of::<LineRecord>() == 16);`
  - `const NONZERO: () = assert!(N > 0, "...");` evaluated in a `const fn new`, as in
    `crates/<name>-core/src/effects.rs`.
  - a `const` initialised with `match ... { None => panic!(..) }`, which fails the build.
- Arithmetic overflow is a bug, and the suite runs with overflow checks and `debug_assert!` on:
  `[profile.qa]` in `Cargo.toml`, used by `test:unit`. Name the intended behaviour
  (`checked_`, `saturating_`, `wrapping_`) wherever overflow can really happen.
- Every public fn that can panic has a `# Panics` section. Gate: clippy `missing_panics_doc`.

## 4. Documenting errors

- Every public fn returning `Result` has an `# Errors` section listing each variant and when it
  happens. Gate: clippy `missing_errors_doc`.
- Link variants with intra-doc links (``[`ParseHeaderError::Short`]``). Broken links fail `test:doc`.

## 5. Teardown

- Destructors never fail and never panic (C-DTOR-FAIL). If closing can fail (flush, fsync,
  network close), expose `finish(self) -> Result<(), E>` or `close(self) -> Result<..>`, and
  make `Drop` a best-effort fallback that ignores errors.
- A destructor that may block needs a non-blocking alternative (C-DTOR-BLOCK).
- Do not reach for `mem::forget` or `ManuallyDrop::new` to skip a destructor. Both are banned
  (clippy `disallowed_methods`, `clippy.toml`); restructure ownership instead (ownership.md).
