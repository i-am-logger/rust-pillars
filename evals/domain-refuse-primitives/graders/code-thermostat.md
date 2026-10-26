---
type: llm
focus:
  source: file
  path: crates/rust-pillars-domain/src/thermostat.rs
---
This file is the domain crate's new thermostat module. Judge it from its contents alone. If the file is missing or empty, every item fails.

1. It defines a reading type, a setpoint type distinct from it, a public `enum Heat` with two variants (heat / don't heat), and a public decision function or method that takes a reading and a setpoint and returns `Heat`. The decision heats exactly when the reading is below the setpoint, and does not heat at or above it. It compares temperatures as signed values, not as raw bytes.
2. No raw signed 16-bit value (or any other integer, or a `[u8; 2]` array) appears in a new public signature. That rules out a `pub fn`/method such as `new(i16)`, `of(i16)`, `get() -> i16` or `from_le_bytes([u8; 2])`, an `impl From<i16>` / `TryFrom<i16>` (or from any integer) for these types, and a `pub` field. The value enters only through a derive-only route such as `zerocopy::FromBytes`, which adds no hand-written primitive-carrying API. These are allowed and are not violations: a `pub(crate)` or private field, including one holding zerocopy's `I16`; methods the derives supply (`read_from_bytes(&[u8])`, `ref_from_bytes`); calling `.get()` on zerocopy's `I16` inside the module; and helper functions inside `#[cfg(test)]` that take `i16`.
3. There is no `bool` parameter, no `bool` field, and the decision does not return `bool`. Booleans that come from comparisons or assertions in code or tests are fine.
4. The file has no `#[allow(...)]`, `#![allow(...)]`, `#[expect(...)]` or `#![expect(...)]` suppression.
5. Its tests pin the decision. Each of these cases is checked: a reading below the setpoint (heat), a reading exactly at the setpoint (don't heat), a reading above the setpoint (don't heat), and at least one negative reading.
