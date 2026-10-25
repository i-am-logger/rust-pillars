---
type: regex
pattern: '^\s*[\w-]+\s*=\s*"quarantine"'
flags: m
match: not_contains
target:
  source: file
  path: layers.toml
---
No quarantine crate was declared in layers.toml to hold the unsafe code.
