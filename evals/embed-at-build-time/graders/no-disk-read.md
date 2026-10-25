---
type: regex
pattern: 'read_to_string|File::open|fs::read'
match: not_contains
target:
  source: file
  path: crates/rust-code/src/stopwords.rs
---
The stopword module no longer reads the asset from disk at run time.
