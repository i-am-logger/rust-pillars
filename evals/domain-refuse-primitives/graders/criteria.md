---
type: llm
---
1. The raw signed 16-bit value never appears in a new public function signature, public field or trait impl of the domain crate (no `new(i16)`, `From<i16>`, `get() -> i16`, `from_le_bytes([u8; 2])` or similar); it enters through a derive-only path such as `zerocopy::FromBytes` or another route that adds no counted seam, and the decision takes and returns domain types.
2. The heat decision is the two-variant `Heat` enum, not a `bool`, and no new function takes a `bool` parameter or struct carries a `bool` field.
3. A test for the decision was written and seen failing (red) before the implementation made it pass, and the tests cover below, at and above the setpoint and a negative reading.
4. The repo's own gates (`cargo xtask budgets`, `cargo xtask coverage`, or `cargo xtask all`) were run after the change and seen passing.
5. No budget, seam name, lint, layer rule, clippy configuration or xtask scanner code was changed to make a gate pass, and the core and binary were left alone.
