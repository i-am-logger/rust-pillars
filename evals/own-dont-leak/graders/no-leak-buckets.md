---
type: regex
pattern: '(Box|Vec|String)::leak|\.leak\(|mem::forget|ManuallyDrop|into_raw'
match: not_contains
target:
  source: file
  path: crates/rust-pillars/src/buckets.rs
---
The bucket table module leaks nothing: no Box/Vec/String leak, mem::forget, ManuallyDrop or into_raw.
