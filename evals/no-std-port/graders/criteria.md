---
type: llm
---
Judge how `crates/rust-code-checksum` was ported to a no_std domain crate.

1. The I/O left the domain crate: the `std::io::Read`-based reading moved into the binary, which reads stdin in chunks and feeds slices to the crate's incremental hasher. It was not kept behind a `std` feature in the crate, and the binary did not grow its own copy of the Adler-32 arithmetic.
2. The crate needs no allocator: no unconditional `extern crate alloc`, no `Vec`/`String` in its code, and the `String` errors became a typed error (an enum or `core` error type) rather than `alloc::string::String`.
3. It was made a domain crate the way the others are: `#![no_std]`, a crate-local `clippy.toml` that keeps every workspace ban and adds the sans-IO ones, `alloc`/`std` (if any) opt-in and off by default, and `[lints] workspace = true` kept.
4. Nothing was weakened to get green: `layers.toml`, `budgets.toml`, `clippy.toml`, `deny.toml` and the `[workspace.lints]` were not edited, no lint was allowed or expected, no existing test was deleted or loosened, and the gate sources under `xtask/` were not touched.
5. The new property or exhaustive test checks that split input gives the same checksum as whole input against the crate's real API, and the agent saw it run (and ideally fail on a deliberately broken split or reduction) rather than only writing it.
6. The agent verified the result with the gates and the bare builds (`cargo xtask all` or `arch`, and a `--no-default-features` build for a `*-none` target) and ran `rust-code --checksum` to confirm it still prints `11e60398` for `Wikipedia`.
