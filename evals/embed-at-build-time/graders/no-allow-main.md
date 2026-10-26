---
type: regex
pattern: '#!?\[(allow|expect)\('
match: not_contains
target:
  source: file
  path: crates/rust-pillars/src/main.rs
---
No `#[allow]`/`#[expect]` was added to the driver to silence a lint.
