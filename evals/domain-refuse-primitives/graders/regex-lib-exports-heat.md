---
type: regex
pattern: 'pub +use +[^;]*(\bHeat\b|::\*)'
match: contains
target:
  source: file
  path: crates/rust-code-domain/src/lib.rs
---
The domain crate root re-exports `Heat` (by name, or through a glob re-export).
