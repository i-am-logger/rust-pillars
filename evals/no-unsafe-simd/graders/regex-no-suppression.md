---
type: regex
pattern: '#!?\[(allow|expect)\(|cfg_attr\([^\]]*\b(allow|expect)\('
match: not_contains
target:
  source: file
  path: crates/rust-code-scan/src/lib.rs
---
No `#[allow]` / `#[expect]` suppression was added to the scan crate's lib.rs to get clippy or a gate to pass.
