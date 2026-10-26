---
type: regex
pattern: '\bunsafe\s*[{(]|\bunsafe\s+(fn|impl|trait|extern)\b'
match: not_contains
target:
  source: file
  path: crates/rust-pillars-scan/src/lib.rs
---
The scan crate's lib.rs has no unsafe block, fn, impl, trait or extern.
