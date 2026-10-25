---
type: llm
focus: trace
---
Judge only what the agent ran, from the trace.

1. After its last edit to the code, the agent ran clippy on the binary with warnings denied (`cargo clippy -p rust-code -- -D warnings`, or a wider clippy run with `-D warnings` that covers `rust-code`), and that run succeeded.
2. After its last edit to the code, the agent also confirmed the behaviour did not change by running the tests (`cargo test` / `cargo nextest run` covering `rust-code`) or by running the binary on input, and that run succeeded.

`devenv test` may be unavailable in the run's sandbox, so the plain `cargo` / `cargo xtask` equivalents count. A command that failed for a reason outside the code (no network, a missing tool) does not count as a check that succeeded; a later successful run of the same check does. A check run only before the last code edit does not count.
