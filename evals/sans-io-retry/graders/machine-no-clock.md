---
type: regex
pattern: 'std::time|\b(Instant|SystemTime)\b|\bsleep\b|\bdyn\s+Fn|\bimpl\s+Fn|\bFn(Mut|Once)?\s*\('
match: not_contains
target:
  source: file
  path: crates/rust-pillars-core/src/machine.rs
---
The core machine gets no clock, sleep or callback: time only comes in through `Event::Tick`.
