---
type: regex
pattern: '^\[lints\]\s*\nworkspace\s*=\s*true\s*$'
flags: m
match: contains
target:
  source: file
  path: crates/rust-pillars-checksum/Cargo.toml
---
The checksum crate's manifest keeps `[lints] workspace = true`.
