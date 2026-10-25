---
type: regex
pattern: 'include_(str|bytes)!|percent\.rs|CARGO_MANIFEST_DIR|read_to_string|fs::read|File::open|file!\('
match: not_contains
target:
  source: file
  path: crates/rust-code-domain/src/percent.rs
---
No test in percent.rs reads the source text (embedded or from disk), which would kill every mutant without checking behaviour.
