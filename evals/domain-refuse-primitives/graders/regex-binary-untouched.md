---
type: regex
pattern: '\bHeat\b'
match: not_contains
target:
  source: file
  path: crates/rust-code/src/main.rs
---
The binary's main.rs does not mention `Heat`: the thermostat was not wired into the binary.
