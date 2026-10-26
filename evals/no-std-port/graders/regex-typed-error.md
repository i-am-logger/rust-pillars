---
type: regex
pattern: '->\s*Result<[^\n]*\bString\b'
match: not_contains
target:
  source: file
  path: crates/rust-pillars-checksum/src/lib.rs
---
No function of the checksum crate returns a `String` error: its errors are typed.
