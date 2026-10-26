---
type: regex
pattern: '#!?\[(allow|expect)\(|#\[ignore'
match: not_contains
target:
  source: file
  path: crates/rust-pillars-domain/src/quantity.rs
---
No `#[allow]`, `#[expect]` or `#[ignore]` was added to crates/rust-pillars-domain/src/quantity.rs to silence a lint or a test.
