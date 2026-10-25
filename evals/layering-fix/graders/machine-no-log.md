---
type: regex
pattern: '\blog::'
match: not_contains
target:
  source: file
  path: crates/rust-code-core/src/machine.rs
---
The core's machine makes no `log::` calls.
