# no_std: domain and core build without std

Load this when you create or port a domain or core crate, add `alloc`, add a
dependency to a pure crate, write a clippy ban, or `test:bare` fails.

Evidence labels: **measured** (someone measured it), **by-reading** (the code
or notes say it), **measured here** (this skill's own compile checks on rustc
1.94.1, aarch64-apple-darwin, against a `git archive` copy of rust-template).

## The rule

- **Domain and core are `#![no_std]`.** `alloc` is optional, behind an
  off-by-default feature named `alloc`.
  - Write `#[cfg(feature = "alloc")] extern crate alloc;` in the same commit as
    the first `alloc::` use, not before.
  - An unused `extern crate alloc` fails the build under the workspace's
    `unused = deny` (`unused_extern_crates`, measured here). That is why the
    template's `lib.rs` files have only `#![no_std]`.

  ```toml
  [features]
  default = []
  alloc = []          # in core: alloc = ["my-project-domain/alloc"]
  ```

- **Why no_std is a pillar.**
  - It makes fs, net, threads, process and the clock *unreachable*, so the
    sans-IO rule is enforced by the compiler, not by review.
  - It keeps the core usable on bare metal, in a kernel, in WASM, and inside
    any host.
- **Gates.**
  - `cargo xtask arch`: `#![no_std]` must be in `src/lib.rs`, and `default`
    must not turn on `alloc`/`std`.
  - `test:bare`: `cargo build --no-default-features --target
    x86_64-unknown-none` and `--target thumbv7em-none-eabihf` for domain and
    core. A `use std` anywhere fails the build.
  - `test:features`: `alloc` builds on its own (`cargo hack --each-feature`).
- **Dependencies of pure crates.** Every dependency must say
  `default-features = false`. The template does this for
  `zerocopy = { version = "0.8", default-features = false, features = ["derive"] }`,
  because a default `std` feature would pull std into a no_std crate through
  the back door. `cargo xtask layers` already limits *which* crates a pure
  crate may use.

## The clippy path trap

- A clippy `disallowed-methods`/`disallowed-types` path under `std::` resolves
  to nothing in a no_std crate, and clippy then bans nothing. It is silent: no
  warning, no error.
- So every ban is listed under the crate that defines the item. The root
  `clippy.toml` lists `alloc::boxed::Box::leak`, `alloc::vec::Vec::leak`,
  `alloc::string::String::leak`, `core::mem::forget` and
  `core::mem::ManuallyDrop::new` alongside their `std::` paths.
- The template's planted-violation run found this hole. A leak planted in a
  no_std crate passed while only `std::` paths were listed (measured).
- `std::time::Instant`/`SystemTime` exist only in std, so there is no `core::`
  path to list. Those bans catch an `extern crate std` slip, and `#![no_std]`
  does the rest.
- **Clippy reads the `clippy.toml` nearest the crate's manifest.** A crate-local
  file *replaces* the root one; it does not merge with it. So domain and core
  carry a copy of every root ban plus the sans-IO bans, and `cargo xtask arch`
  checks that each copy is a superset.
- **Check a new ban with a plant.** Add the banned call in a scratch copy, run
  clippy, and see it go red. Only then trust the ban (positive control).

## Errors

- **Errors in a no_std crate are `Copy` enums with no payload**: no `String`,
  no `Box<dyn Error>`, because there is no allocator to carry one. They
  implement `core::error::Error` and `core::fmt::Display`, both in `core`.
- The rule, its worked snippet, why core also skips `thiserror` (layering,
  not capability) and its DEPARTS from BeautifulRust are rust-idioms'
  (`references/errors.md`, "Core").
- On the core's hot entry, prefer refuse-and-count over `Result`
  ([sans-io.md](sans-io.md)). Use these enums for fallible constructors and
  parsers.

## Tests in a no_std crate

- Integration tests in `tests/` are separate crates and have `std`. The
  template's model test and `alloc.rs` live there.
- Unit tests (`#[cfg(test)] mod tests`) are built with the test harness. To use
  `std` there, add `#[cfg(test)] extern crate std;`. Better still, keep
  std-using tests in `tests/`.
- Bare targets are built, not run. Run the tests on the host.

## Porting a std crate to no_std

| std thing | Replacement in domain/core |
|---|---|
| `use std::{fmt, mem, ...}` | `use core::{fmt, mem, ...}` |
| `Vec<T>` on the hot path | a caller-lent `&'b mut [T]` or a fixed `[T; N]` with a length newtype ([zero-copy.md](zero-copy.md)) |
| `Vec`/`Box` for setup, really needed | `alloc::` behind the `alloc` feature, off by default |
| `HashMap` | a fixed, power-of-two open-addressing table (see the interning notes in [caches.md](caches.md)), or `alloc::collections::BTreeMap` behind `alloc` |
| `String` errors | `Copy` error enums (above) |
| `Instant::now()` / `SystemTime` | an `Event::Tick(Millis)` from the host ([sans-io.md](sans-io.md)) |
| `println!` / `log::` | an effect the host prints |
| `std::io::Read`/`Write` | bytes in as `Event::Bytes(Wire)`, bytes out as effects |
| `thread::spawn`, locks | the host's job; the core is single-threaded and `Send` data |
| `rand` | a seed event plus PRNG state as plain fields |

Steps:
1. Add `#![no_std]` and fix every error with `core::`/`alloc::` paths.
2. Move each I/O, clock and RNG use out to an event.
3. Copy the core's `clippy.toml`.
4. Give the crate its role in `layers.toml`. `test:bare` reads the domain and
   core crates from there (`cargo xtask crates --role domain,core`).
5. Run `devenv test`.

## Evidence

- The `std::`-only clippy ban hole was found by rust-template's own
  planted-violation run (measured).
