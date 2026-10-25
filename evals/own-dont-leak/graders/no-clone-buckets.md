---
type: regex
pattern: '\bClone\b'
match: not_contains
target:
  source: file
  path: crates/rust-code/src/buckets.rs
---
`Clone` was not derived or implemented on the bucket table to allow copying it per use.
