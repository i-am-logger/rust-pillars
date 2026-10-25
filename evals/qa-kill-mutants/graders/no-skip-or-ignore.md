---
type: regex
pattern: 'mutants::skip|cfg_attr\([^)]*mutants|cfg\(mutants|#\[ignore'
match: not_contains
target:
  source: file
  path: crates/rust-code-domain/src/percent.rs
---
percent.rs hides no mutant behind a skip attribute or `cfg(mutants)`, and ignores no test.
