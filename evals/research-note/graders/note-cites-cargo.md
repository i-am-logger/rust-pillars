---
type: regex
pattern: '\[\[source: [a-z0-9-]+\]\]'
match: contains
target:
  source: file
  path: docs/research/notes/a-domain-std-feature.md
---
The note cites a pinned source in the project's `[[source: key]]` form.
