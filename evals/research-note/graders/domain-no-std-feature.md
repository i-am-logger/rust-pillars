---
type: regex
pattern: '^[ \t]*std[ \t]*='
flags: m
match: not_contains
target:
  source: file
  path: crates/rust-pillars-domain/Cargo.toml
---
The task asks for a decision only: the domain crate's `[features]` gains no `std` feature.
