---
type: regex
pattern: '(Box|Vec|String)::leak|\.leak\(|mem::forget|ManuallyDrop|into_raw'
match: not_contains
target:
  source: file
  path: crates/rust-code/src/main.rs
---
The driver did not move the leak into main.rs: no Box/Vec/String leak, mem::forget, ManuallyDrop or into_raw.
