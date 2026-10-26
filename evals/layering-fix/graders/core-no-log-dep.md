---
type: regex
pattern: '^\s*log\b|dep:log'
flags: m
match: not_contains
target:
  source: file
  path: crates/rust-pillars-core/Cargo.toml
---
The core's manifest names no `log` dependency in any table (plain, optional, feature-gated `dep:log`, or `cfg`/target-specific).
