---
type: regex
pattern: '&\s*''static\s+(buckets::)?Buckets'
match: not_contains
target:
  source: file
  path: crates/rust-code/src/main.rs
---
No signature or binding in the driver takes the table as a `&'static Buckets`.
