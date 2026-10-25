---
type: regex
pattern: '^rust-code-checksum = "domain"$'
flags: m
match: contains
target:
  source: file
  path: layers.toml
---
`layers.toml` still gives `rust-code-checksum` the role `domain`.
