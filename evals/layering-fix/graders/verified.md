---
type: llm
focus: trace
---
Judge only what the agent ran after its last code edit, and whether it succeeded.

1. After its last edit to code or manifests, the agent ran `cargo xtask layers` (or `cargo xtask all`, which includes it) and it passed.
2. After that last edit it also ran clippy with warnings denied (`cargo clippy --workspace --all-targets --all-features -- -D warnings` or equivalent) and the workspace tests (`cargo test` or `cargo nextest run`), and both passed. The tests run include a core test that overflows the effect queue and checks the dropped count reported through `refused()`. Running the binary on a large input does not count as that check: the binary slices its reads so the queue never overflows.

`devenv test` may be unavailable in this sandbox, so the plain `cargo` / `cargo xtask` equivalents count. A command that failed for a reason outside the code (no network, a missing tool) does not count as a pass, but a later successful run of the same check does.
