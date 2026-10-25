---
type: regex
pattern: '^\s*unsafe_code\s*=\s*"forbid"'
flags: m
match: contains
target:
  source: file
  path: Cargo.toml
---
The workspace lints still forbid unsafe code (`[workspace.lints.rust] unsafe_code = "forbid"` was not loosened).
