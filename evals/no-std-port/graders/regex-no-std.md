---
type: regex
pattern: '^\s*#!\[(cfg_attr\([^\n]*)?no_std\b'
flags: m
match: contains
target:
  source: file
  path: crates/rust-code-checksum/src/lib.rs
---
The checksum crate declares itself `no_std` (`#![no_std]`, or a `cfg_attr` that applies it when `std` is off).
