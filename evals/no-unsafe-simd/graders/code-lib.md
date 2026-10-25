---
type: llm
focus:
  source: file
  path: crates/rust-code-scan/src/lib.rs
---
Judge this file, the scan crate's `lib.rs`, on its contents alone.

1. `first_newline` keeps the signature `pub fn first_newline(haystack: &[u8]) -> Option<usize>` (a `const fn` is fine) and is still a fast scan: it tests several bytes per step in safe code -- for example word-at-a-time SWAR over `u64`/`usize`/`u128` chunks, or a safe bulk API -- and uses a plain byte-by-byte loop only for a short tail. A body that is just `iter().position(..)` or a `for`/`while` loop over every byte fails. If this file only re-exports `first_newline` from a module of this same crate (`mod x; pub use x::first_newline;`), the body is not visible here: then judge only item 2.
2. The file has no `unsafe` block, function, impl or trait, and no SSE2/NEON intrinsics (`core::arch` / `std::arch`); the vector code was replaced, not hidden behind a call into another crate (a new quarantine crate or a dependency) that does the unsafe work.
