---
type: regex
pattern: '^\s*default\s*=\s*\[[^\]]*"(std|alloc)"'
flags: m
match: not_contains
target:
  source: file
  path: crates/rust-pillars-checksum/Cargo.toml
---
`std` and `alloc`, if the crate has them, are opt-in: neither is a default feature.
