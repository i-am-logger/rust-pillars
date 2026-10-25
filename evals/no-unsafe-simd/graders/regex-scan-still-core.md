---
type: regex
pattern: '^\s*rust-code-scan\s*=\s*"core"'
flags: m
match: contains
target:
  source: file
  path: layers.toml
---
layers.toml still gives the scan crate the `core` role.
