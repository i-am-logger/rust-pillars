---
type: regex
pattern: 'names = \["get", "of", "from_host", "threshold_ms"\]'
match: contains
target:
  source: file
  path: budgets.toml
---
The list of named seams in budgets.toml is unchanged: no new seam name was added to let a primitive through.
