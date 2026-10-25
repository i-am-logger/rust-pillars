---
type: regex
pattern: '\{\s*path\s*=\s*"(std|alloc|core)::'
match: count:10
target:
  source: file
  path: clippy.toml
---
The workspace clippy.toml still lists all ten disallowed leak methods (the ban was not loosened).
