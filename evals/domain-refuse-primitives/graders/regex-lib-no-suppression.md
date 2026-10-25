---
type: regex
pattern: '#!?\[(allow|expect)\('
match: not_contains
target:
  source: file
  path: crates/rust-code-domain/src/lib.rs
---
The domain crate root has no `allow`/`expect` lint suppression.
