---
type: regex
pattern: '^\s*Retry\s*\(\s*Millis\s*\)\s*,?\s*$'
flags: m
match: contains
target:
  source: file
  path: crates/rust-code-domain/src/event.rs
---
The domain's `Effect` gains the variant `Retry(Millis)` the prompt named.
