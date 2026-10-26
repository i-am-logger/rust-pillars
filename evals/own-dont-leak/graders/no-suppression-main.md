---
type: regex
pattern: '#!?\[\s*(expect|allow)\s*\('
match: not_contains
target:
  source: file
  path: crates/rust-pillars/src/main.rs
---
No `#[allow]`/`#[expect]` (item or crate level) silences a lint in the driver.
