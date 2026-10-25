---
type: regex
pattern: '^\s*(\[lints\]\s*\n(\s*#.*\n)*\s*workspace\s*=\s*true|lints\.workspace\s*=\s*true|lints\s*=\s*\{\s*workspace\s*=\s*true)'
flags: m
match: contains
target:
  source: file
  path: crates/rust-code-scan/Cargo.toml
---
The scan crate inherits the workspace lints (`[lints] workspace = true`).
