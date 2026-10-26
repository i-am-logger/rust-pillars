---
type: regex
pattern: 'Retry\s*\('
match: contains
target:
  source: file
  path: crates/rust-pillars/src/main.rs
---
The binary handles the new `Effect::Retry` when it drains effects (prints it as it prints idle ones).
