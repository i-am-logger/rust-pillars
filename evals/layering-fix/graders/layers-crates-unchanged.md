---
type: regex
pattern: '\[crates\]\nrust-code-domain = "domain"\nrust-code-core = "core"\nrust-code = "bin"\nxtask = "tool"\n\n'
match: contains
target:
  source: file
  path: layers.toml
---
`layers.toml` still gives the core the `core` role, and no crate (a log wrapper, say) was added to the roles table.
