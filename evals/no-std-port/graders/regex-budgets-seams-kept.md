---
type: regex
pattern: '^\[crates\.rust-code-checksum\]\nseam = 3$'
flags: m
match: contains
target:
  source: file
  path: budgets.toml
---
`budgets.toml` still holds `rust-code-checksum` to its three declared seams.
