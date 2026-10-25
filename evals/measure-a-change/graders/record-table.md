---
type: regex
pattern: '^\|.*\|'
flags: m
match: contains
target:
  source: file
  path: MEASUREMENTS.md
---
MEASUREMENTS.md carries a results table (the pristine file has none).
