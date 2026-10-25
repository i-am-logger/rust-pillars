---
type: regex
pattern: '^\[sources\.'
flags: m
match: count:1
target:
  source: file
  path: docs/research/sources/MANIFEST.toml
---
MANIFEST.toml still pins exactly one source: no new source was added to the manifest.
