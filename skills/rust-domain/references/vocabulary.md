# Vocabulary: events, effects, wire enums, bitsets

The domain defines every word the core understands, and every word it can say back. Each is a closed enum with narrowed payloads, parsed once at the edge. The sans-IO mechanics (polling, bounded queues, chunk invariance) belong to `rust-architecture`. This file covers the types.

## 1. Events in, effects out (template `event.rs`)

```rust
# use my_project_domain::{LineRecord, Millis, Wire};
/// Everything that can happen to the core. The host turns the outside world --
/// a read, the clock, the end of input -- into one of these (pillar 2).
#[derive(Clone, Copy, Debug)]
pub enum Event<'a> {
    /// A read's bytes, borrowed.
    Bytes(Wire<'a>),
    /// The host's clock, read by the host.
    Tick(Millis),
    /// No more input will come.
    End,
}

/// Everything the core can owe the host. Queued, and polled by the host --
/// never a callback.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effect {
    /// A line ended.
    Line(LineRecord),
    /// No input arrived for the configured threshold, as of this time.
    Idle(Millis),
}
```

- **Payloads are already narrowed.**
  - `Tick` carries `Millis`, not `u64`, so a tick cannot be confused with a length.
  - `Bytes` carries `Wire<'a>`, borrowed bytes. The only public view of their content is `classes()`, never the `u8`s; `len()` returns a `ReadLen`.
  - Gate: a primitive in a `pub` enum payload counts as `primitive_api`; so does a `bool` payload, under `bool_param_or_field`.
- **Time, randomness and I/O are variants, not calls.** `Tick(Millis)` is the clock. The crate-local `clippy.toml` bans `std::time::Instant` and `SystemTime` in domain and core (`disallowed_types`).
- **`Event` borrows and `Effect` owns.** An event lives for one `handle` call. An effect waits in a queue until the host polls it, so its payload is `Copy` POD (`LineRecord`), with no borrow of the input.
- **Exhaustive by design.** Core matches `Event` with no wildcard arm, so adding a variant breaks every match that must handle it.
  - No `#[non_exhaustive]` on it: inside one workspace the compile error is the feature. The rule and its DEPARTS from BeautifulRust: rust-idioms.

## 2. Parse once, in one place (template `class.rs`, `wire.rs`)

```rust,ignore
/// The table. Indexed only through [`ByteClass::of`].
static CLASS: [ByteClass; 256] = build();

impl ByteClass {
    /// The class of one byte: one table load.
    #[must_use]
    #[inline]
    pub(crate) fn of(b: u8) -> Self {
        CLASS[usize::from(b)]
    }
}

impl<'a> Wire<'a> {
    /// The bytes' classes, in order. No allocation, no copy.
    pub fn classes(&self) -> impl Iterator<Item = ByteClass> + 'a {
        self.0.iter().map(|&b| ByteClass::of(b))
    }
}
```

Excerpt of `crates/<name>-domain/src/class.rs` and `wire.rs`, compiled there: inherent impls on `ByteClass` and `Wire`, and `Wire`'s private field, exist only inside that crate.

- **One function turns a raw byte into a meaning** (`ByteClass::of`). Every later stage matches on the enum and never re-inspects bytes. Re-parsing in a later stage is how two stages come to disagree.
- **Mark the parse `#[inline]`.** Then the parse can fold into the caller's `match` on the result. It suits a large message enum as well as a byte class. Claim no speed for it without a measurement (rust-measure).
- **Build a finite map as a table at compile time:** a `const fn` filling a `static` in read-only data, with no initialiser at startup.
  - Test it exhaustively against the readable definition, and assert the count: `assert_eq!(checked, 256, ..)`. Otherwise a loop that silently ran short passes.
  - The template's `i as u8` (under `i < 256`) inside `build()` is one of the domain's two counted `as_cast`s.

## 3. A wire protocol: closed opcodes, narrowed payloads

A generic example uses RFC 6455 (WebSocket) framing: §5.2 defines the opcodes, and §5.5 limits control-frame payloads to 125 bytes.

```rust
/// The low four bits of a frame's first byte (RFC 6455 §5.2). Closed: every
/// opcode the protocol defines, and one variant for the reserved ones, which
/// the core refuses and counts rather than dropping.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Opcode {
    /// Continues a fragmented message.
    Continuation,
    /// A text data frame.
    Text,
    /// A binary data frame.
    Binary,
    /// A close control frame.
    Close,
    /// A ping control frame.
    Ping,
    /// A pong control frame.
    Pong,
    /// 0x3-0x7 and 0xB-0xF, reserved by the protocol.
    Reserved(ReservedOpcode),
}

/// A reserved opcode's value, kept so a refusal can say which one it was.
/// Only [`Opcode::parse`] makes one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ReservedOpcode(u8);

impl ReservedOpcode {
    /// The raw opcode, for the host's log. A seam: counted by `xtask budgets`.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl Opcode {
    /// The one place a header byte becomes an opcode. Called by the frame
    /// parser; `#[inline]` so this match folds into the caller's.
    #[inline]
    #[must_use]
    pub(crate) const fn parse(first: u8) -> Self {
        match first & 0x0F {
            0x0 => Self::Continuation,
            0x1 => Self::Text,
            0x2 => Self::Binary,
            0x8 => Self::Close,
            0x9 => Self::Ping,
            0xA => Self::Pong,
            other => Self::Reserved(ReservedOpcode(other)),
        }
    }
}

/// A control frame's payload limit, from the protocol (RFC 6455 §5.5).
/// `pub(crate)`: a `pub const` of a raw `usize` is a primitive in the API,
/// and `xtask budgets` counts it as one.
pub(crate) const MAX_CONTROL_PAYLOAD: usize = 125;
/// A server-to-client control frame: 2 header bytes (no mask, 7-bit length).
pub(crate) const MAX_CONTROL_FRAME: usize = 2 + MAX_CONTROL_PAYLOAD;

/// A control frame's payload, borrowed and proven to be within the limit.
#[derive(Clone, Copy, Debug)]
pub struct ControlBody<'a>(&'a [u8]);

impl<'a> ControlBody<'a> {
    /// The payload, if the protocol allows its length. Called by the parser.
    #[must_use]
    pub(crate) const fn narrow(bytes: &'a [u8]) -> Option<Self> {
        if bytes.len() <= MAX_CONTROL_PAYLOAD { Some(Self(bytes)) } else { None }
    }

    /// The bytes, at most [`MAX_CONTROL_PAYLOAD`] of them, for the in-crate
    /// copy into the outbox. `pub(crate)`: not a seam, and never counted.
    #[must_use]
    pub(crate) const fn bytes(self) -> &'a [u8] {
        self.0
    }
}

/// Outgoing bytes the host drains. Sized so one control frame always fits.
const OUTBOX: usize = 128;
const _: () = assert!(OUTBOX >= MAX_CONTROL_FRAME, "a control frame must fit the outbox");
```

- **Closed means that unknown input is a variant too.**
  - `Reserved(ReservedOpcode)` keeps the raw value inside a newtype that only the parser can build.
  - The core refuses that frame and counts it (`Tally`), so the input is never silently dropped.
  - Never hide unknowns behind `_ => return`.
  - Give the payload an exit only if a host needs the value. `ReservedOpcode::get` is one more `seam` to budget.
- **Narrow the payload while parsing.**
  - A `ControlBody` exists only if the length is within the limit, so every later stage, such as copying it into a pong, is total.
  - Later stages in the crate take `ControlBody<'_>`, not `&[u8]`. Only `bytes()` opens it, and only the copy into the outbox calls that.
- **Take capacities from the standard, not from taste.**
  - `MAX_CONTROL_PAYLOAD` is the protocol's own maximum, cited by clause.
  - The buffer's size is `const`-asserted against it, so shrinking the buffer, or a protocol revision that grows the maximum, fails the build instead of truncating.
  - The citation belongs in the source manifest (`rust-research`).
- **Use one closed enum per family of messages,** not one enum for everything. For a protocol, that is one enum per message family. A family's variants share one parse and one payload shape, so the `match` that handles them is one the compiler can check for exhaustiveness.

## 4. Refusals are vocabulary too

- **Outcomes are small `Copy` enums:** `Pushed {Taken, Refused}` from the domain's `Line::push`, and core's private `Queued {Taken, Dropped}`.
  - Mark them `#[must_use]` on the type, so every call site handles them.
  - The caller adds each refusal to a `Tally` in `Refusals`, which the host reads.
- **In a `no_std` core, an error is a `Copy` enum with no payload** (no allocator to carry one; the host owns the context). The rule, its snippet and its DEPARTS from BeautifulRust: rust-idioms (`references/errors.md`).
- **A predicate returns `bool`;** an outcome does not. `Line::is_empty() -> bool` asks a question. `push` reports what happened, so it returns `Pushed`. The gate counts `bool` parameters and fields, not returns.

## 5. Bitsets

A set of flags becomes a newtype over an integer, indexed by an enum. It is never a struct of `bool`s, which `bool_param_or_field` counts.

```rust
/// One option of a stream.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Echo input back.
    Echo,
    /// Wrap at the end of a line.
    Wrap,
    /// Insert rather than overwrite.
    Insert,
}

impl Mode {
    /// Every mode, in bit order.
    pub const ALL: [Self; 3] = [Self::Echo, Self::Wrap, Self::Insert];

    /// This mode's bit. Exhaustive: a new variant does not compile until it
    /// has a bit, and a 33rd bit does not fit a `u32` literal.
    const fn bit(self) -> u32 {
        match self {
            Self::Echo => 0b001,
            Self::Wrap => 0b010,
            Self::Insert => 0b100,
        }
    }
}

/// A set of modes. Zero is the empty set, so `Default` means "none" here.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Modes(u32);

impl Modes {
    /// No mode set.
    pub const NONE: Self = Self(0);

    /// This set with `m` on.
    #[must_use]
    pub const fn with(self, m: Mode) -> Self {
        Self(self.0 | m.bit())
    }

    /// This set with `m` off.
    #[must_use]
    pub const fn without(self, m: Mode) -> Self {
        Self(self.0 & !m.bit())
    }

    /// Whether `m` is on. A predicate: `bool` return is fine.
    #[must_use]
    pub const fn has(self, m: Mode) -> bool {
        self.0 & m.bit() != 0
    }
}
```

- **The width bound.** DEPARTS from the pillar text, which says "bounded by `variant_count`". The stable form below gives the same compile-time failure.
  - The direct form is `const _: () = assert!(variant_count::<Mode>() <= 32)`, but `core::mem::variant_count` is nightly-only (feature `variant_count`), so the template cannot use it on its stable toolchain.
  - On stable, the bound is the exhaustive `bit` match: a variant without a bit is a non-exhaustive match error, and a bit past 32 is an `overflowing_literals` error (deny by default).
- **`ALL` must list every variant.**
  - Add a variant to `ALL` in the same edit that adds its bit.
  - A unit test that `Mode::ALL[i].bit() == 1 << i` catches a duplicated or misordered bit. It does not catch a variant missing from `ALL`; review does.
- **Use `Default` only where zero is the empty value.** For a set, it is; the derive matches the "zero = empty" rule for stored types.
- **The `bitflags` crate (BeautifulRust's suggestion)** is not on the domain's dependency allowlist: `layers.toml` lists only `zerocopy` for `roles.domain`. Adding it is a `layers.toml` change with a reason.
