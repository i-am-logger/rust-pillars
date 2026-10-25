---
type: regex
pattern: 'pub fn through\(done: BytePos, total: BytePos\) -> Self \{\s*if done\.0 >= total\.0 \{\s*return Self::FULL;\s*\}\s*(//[^\n]*\s*)*let whole = u128::from\(done\.0\) \* 100 / u128::from\(total\.0\);\s*Self\(u8::try_from\(whole\)\.unwrap_or\(100\)\)\s*\}'
match: contains
target:
  source: file
  path: crates/rust-code-domain/src/percent.rs
---
The body of `Percent::through` still has its original clamp and arithmetic, statement for statement.
