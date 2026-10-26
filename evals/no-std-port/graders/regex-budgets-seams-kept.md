---
type: regex
pattern: '^\[crates\.rust-pillars-checksum\]\nseam = 3$'
flags: m
match: contains
target:
  source: file
  path: budgets.toml
---
`budgets.toml` still holds `rust-pillars-checksum` to its three declared seams.
