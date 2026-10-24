# Startup and embedding at build time

Startup is a metric like throughput: it gets marks, cold and warm arms, a
floor and rows. The cheapest startup work is work done at build time.

## Stage marks (shipped: `crates/<name>/src/marks.rs`)

The template's bin prints one stderr line per stage, `startup <ms> <stage>`
(milliseconds since `Marks::start`, three decimals), only when
`MY_PROJECT_STARTUP=1`. `cargo xtask bench` sets that variable, parses those
lines and reports each stage as its own metric beside `wall`, so a row can say
which stage moved. Keep the format; change the stages.

- Marks live in the **bin or adapter**, never in domain or core: core has no
  clock (domain/core `clippy.toml` bans `std::time::Instant`, gate
  `test:clippy`). If a core stage needs timing, the bin marks before and after
  the call.
- The template names stages with `&str` (`marks.mark("first read")`). An
  enum is stricter -- a typo cannot create a stage, and `unused` (denied)
  catches a stage nothing marks -- as long as it prints the same line.
- The switch is read once, at startup, from the environment. Never toggle it
  in-process from a test: `std::env::set_var` is an `unsafe fn` in edition
  2024 (Edition Guide, Rust 2024, standard-library changes), and
  `unsafe_code` is forbidden.
- Marks are offsets from the first line of `main`; time before `main`
  (exec, dynamic loading) is only visible from outside the process, so
  `bench` times the spawn itself (`wall`) and reads the marks from stderr.

```rust
//! Startup stage marks (role: bin), in the line format `cargo xtask bench`
//! parses: `startup <ms> <stage>` on stderr, only under `MY_PROJECT_STARTUP=1`.

use std::time::Instant;

/// Whether marks are printed. An enum, not a `bool`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Print {
    On,
    Off,
}

/// A named point in startup. Every variant is marked by `main`.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Stage {
    ArgsParsed,
    MachineBuilt,
    FirstEffectWritten,
}

#[derive(Debug)]
pub(crate) struct Marks {
    start: Instant,
    print: Print,
}

impl Marks {
    /// Starts the clock. Call first in `main`.
    pub(crate) fn start() -> Self {
        let print = if std::env::var_os("MY_PROJECT_STARTUP").is_some_and(|v| v == "1") {
            Print::On
        } else {
            Print::Off
        };
        Self { start: Instant::now(), print }
    }

    /// Prints `startup <ms> <stage>`.
    pub(crate) fn mark(&self, stage: Stage) {
        if self.print == Print::On {
            let ms = self.start.elapsed().as_secs_f64() * 1e3;
            eprintln!("startup {ms:.3} {stage:?}");
        }
    }
}
```

No `as` cast and no suppression: `budgets.toml` holds the bin crate at zero
of each, and `as_secs_f64` needs no integer conversion.

## Cold and warm

- **Warm**: run the arm once untimed, then the rounds. Caches populated.
- **Cold**: before each round, **move aside** every cache the program or the
  OS keeps for it -- the app's own cache directory, OS shader caches, font
  caches -- by renaming, and restore them after the run. Never delete (you lose
  the user's state) and never assume a cache is cold.
- Name each cache moved in the row. A cold row that does not say which caches
  were cold is not reproducible.
- The OS file cache is a separate variable from application caches; say
  whether it was cleared, and how.
- `cargo xtask bench --cold <dir>` (repeatable) renames each directory
  aside (a `.cold-aside` name) before every launch and restores it after
  (shipped).

## Floors for startup

Startup rows carry a floor like every other row, chosen before the run and
outside the A/A spread; changes under it are reverted and the row is still
recorded.

## Embed at build time

**Precompute anything deterministic** so the shipped binary starts with the
answer instead of computing it: lookup tables, shader binaries, font subsets,
parsed default configuration, generated protocol tables.

### Choose the shape

| Artefact | Shape | Drift check |
|---|---|---|
| a table a `const fn` can compute | `static T: [X; N] = build();` | exhaustive agreement test (shipped: `class.rs`) |
| a blob from an external tool (shader compiler, font subsetter) | committed file + `static B: &[u8] = include_bytes!(...)` | regenerate and diff, plus pinned sha256 |
| structured data (default config) | generated `.rs` with `const`/`static` items, committed | regenerate and diff |

- A `const fn`-built `static` lives in read-only data and needs **no
  initialiser at startup**; `LazyLock`/`OnceLock` tables pay at first use and
  add a branch to every access. In `class.rs`, `static CLASS: [ByteClass; 256]
  = build();` is filled at compile time and a classification is one load.
- **Bind a blob to one `static`, never a `const`.** A `const` is inlined at
  every use, and each use may get its own copy of the bytes in the binary; a
  `static` is one address (Rust Reference, "Constant items" and "Static
  items").

Not compiled here: `generated/shaders.bin` belongs to the embedding crate
and does not exist in this repository.

```rust,ignore
/// The precompiled artefact, produced by the generator and committed.
/// A `static`, not a `const`: one copy in the binary, one address.
static SHADERS: &[u8] = include_bytes!("../generated/shaders.bin");

/// The one reader (`unused` is denied: an unread static fails the build).
pub(crate) fn shaders() -> &'static [u8] {
    SHADERS
}
```

- If a consumer parses the blob in place (zerocopy), it needs alignment that
  `include_bytes!` does not promise (`[u8; N]` has alignment 1). Wrap it:

```rust,ignore
#[repr(C, align(16))]
struct Aligned<T: ?Sized>(T);

static TABLE: &Aligned<[u8]> = &Aligned(*include_bytes!("../generated/table.bin"));

/// The bytes, 16-byte aligned, for an in-place `zerocopy` parse.
pub(crate) fn table() -> &'static [u8] {
    &TABLE.0
}
```

Not compiled here either: `generated/table.bin` is the embedding crate's
file and does not exist in this repository.

### Who generates

- Generators are **tool-role** code (planned `cargo xtask gen`), run by a
  developer, with output committed. Domain and core get no `build.rs` with
  dependencies: `cargo xtask layers` checks build-dependency edges against
  `layers.toml` as well as normal ones, and a build script that shells out to
  an external compiler makes the build depend on a tool the gates do not pin.
- Output must be deterministic: same inputs, same bytes (sort, fix
  timestamps, pin the tool version). A generator that is not deterministic
  makes every drift check red.

### Drift tests and pins

- `const fn` tables: the readable definition stays in the source and a test
  checks **every** input against it, asserting the count
  (`assert_eq!(checked, 256)` in `class.rs`), so a sweep that ran short fails.
- Blobs: a test re-hashes the embedded bytes against the pin, so a hand edit or
  a stale file fails:

Not compiled here: it needs `sha2`, which the skills' snippet gate does not link; it would sit in the generator's own crate.

```rust,ignore
#[cfg(test)]
mod tests {
    // Domain/core are `#![no_std]`: `format!` is not in scope until the test
    // names std. In a std crate (adapter, bin) delete these two lines --
    // there `extern crate std` fails `unused` (denied workspace-wide).
    extern crate std;
    use std::format;

    use super::*;
    use sha2::{Digest, Sha256};

    /// The pinned digest; changes only in the commit that regenerates the
    /// file. Test-only, so a non-test build has no unused constant.
    const SHADERS_SHA256: &str = "<sha256 of generated/shaders.bin>";

    #[test]
    fn the_embedded_shaders_are_the_pinned_ones() {
        let got = format!("{:x}", Sha256::digest(SHADERS));
        assert_eq!(got, SHADERS_SHA256, "regenerate with the generator and repin in the same commit");
    }
}
```

- `sha2` is already a workspace dependency; add it as a dev-dependency of the
  embedding crate (`layers.toml` does not check dev-dependencies, so a domain
  crate may). Verified: both forms pass `test:clippy` and run green in a copy
  of the template.
- The sha catches edits to the file; it does not catch the **source** moving
  on while the file stays. That needs regenerate-and-diff (planned
  `cargo xtask gen --check`, in a `qa` task where the external tool exists).
- Planned: each artefact's sha also goes in a MANIFEST checked by a gate, like
  `docs/research/sources/MANIFEST.toml` does for research sources.

### Prove it pays

- Each embedded artefact gets a **cold and warm startup row** with and
  without the embedding, and a **size row** (binary and `__TEXT` growth).
  Embedding trades load time for page-in and size; the rows say which won.
- Costs the embedding exposes are recorded as **open items**, not hidden by
  the headline.
