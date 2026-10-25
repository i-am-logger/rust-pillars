---
type: regex
pattern: '#!\[(cfg_attr\([^\]]*,\s*)?no_std\s*\)?\s*\]'
match: contains
target:
  source: file
  path: crates/rust-code-scan/src/lib.rs
---
The scan crate's lib.rs is still `no_std`, so it keeps building for bare targets.
