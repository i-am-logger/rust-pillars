---
type: regex
pattern: '#!\[no_std\]'
match: contains
target:
  source: file
  path: crates/rust-code-domain/src/lib.rs
---
The domain crate is still `#![no_std]` (pillar 5 was not worked around).
