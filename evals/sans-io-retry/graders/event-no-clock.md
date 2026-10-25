---
type: regex
pattern: 'std::time|\b(Instant|SystemTime)\b|\bsleep\b|\bdyn\s+Fn|\bimpl\s+Fn|\bFn(Mut|Once)?\s*\(|\bfn\s*\('
match: not_contains
target:
  source: file
  path: crates/rust-code-domain/src/event.rs
---
The domain's events and effects carry no clock, timer or callback: the retry is data the host polls.
