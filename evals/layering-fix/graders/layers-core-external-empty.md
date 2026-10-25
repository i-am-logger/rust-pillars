---
type: regex
pattern: '\[roles\.core\]\n(?:#.*\n)*may_depend_on = \["domain", "quarantine"\]\nexternal = \[\]\n'
match: contains
target:
  source: file
  path: layers.toml
---
`layers.toml` was not loosened: the core role still may depend on the domain and quarantine only, with no external crates allowed.
