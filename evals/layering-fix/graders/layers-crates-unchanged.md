---
type: regex
pattern: '\[crates\]\nrust-pillars-domain = "domain"\nrust-pillars-core = "core"\nrust-pillars = "bin"\nxtask = "tool"\n\n'
match: contains
target:
  source: file
  path: layers.toml
---
`layers.toml` still gives the core the `core` role, and no crate (a log wrapper, say) was added to the roles table.
