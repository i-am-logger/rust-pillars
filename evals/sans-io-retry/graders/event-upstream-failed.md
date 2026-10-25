---
type: regex
pattern: '^\s*UpstreamFailed\s*,?\s*$'
flags: m
match: contains
target:
  source: file
  path: crates/rust-code-domain/src/event.rs
---
The domain's `Event` gains the payload-free variant `UpstreamFailed` the prompt named.
