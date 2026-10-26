---
type: regex
pattern: '#!?\[(allow|expect)\('
match: not_contains
target:
  source: file
  path: crates/rust-pillars/src/stopwords.rs
---
No `#[allow]`/`#[expect]` was added to the stopword module to silence a lint.
