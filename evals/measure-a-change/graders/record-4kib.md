---
type: regex
pattern: '(^|[^0-9])(4 ?(KiB|KB|K)|4096)([^0-9]|$)'
flags: i
match: contains
target:
  source: file
  path: MEASUREMENTS.md
---
MEASUREMENTS.md names the 4 KiB arm (the pristine file does not).
