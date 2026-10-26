---
type: regex
pattern: '^rust-pillars-checksum = "domain"$'
flags: m
match: contains
target:
  source: file
  path: layers.toml
---
`layers.toml` still gives `rust-pillars-checksum` the role `domain`.
