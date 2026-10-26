---
type: regex
pattern: '^(?![ \t]*//).*(rust_pillars_checksum::read_all|65_?521|0x[fF]{3}1\b)'
flags: m
match: not_contains
target:
  source: file
  path: crates/rust-pillars/src/main.rs
---
The binary's code neither calls the crate's old reader (`rust_pillars_checksum::read_all`) nor carries its own copy of the Adler-32 modulus.
