---
type: regex
pattern: 'exclude|examine|skip'
match: not_contains
target:
  source: file
  path: .cargo/mutants.toml
---
The cargo-mutants config was not loosened to exclude or skip any mutant.
