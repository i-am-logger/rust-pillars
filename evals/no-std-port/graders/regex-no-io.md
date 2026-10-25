---
type: regex
pattern: '^(?![ \t]*//).*(std::io|io::(Read|Stdin|stdin)|\bread_all\b)'
flags: m
match: not_contains
target:
  source: file
  path: crates/rust-code-checksum/src/lib.rs
---
The checksum crate's code (comment lines aside) does no I/O: no `std::io`, no `Read`/`Stdin`, no `read_all`, not even behind a feature.
