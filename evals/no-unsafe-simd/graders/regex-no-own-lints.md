---
type: regex
pattern: '^\s*\[lints\.'
flags: m
match: not_contains
target:
  source: file
  path: crates/rust-code-scan/Cargo.toml
---
The scan crate no longer carries its own `[lints.rust]` / `[lints.clippy]` tables.
