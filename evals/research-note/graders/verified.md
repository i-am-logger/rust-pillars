---
type: llm
focus: trace
---
Judge from the trace whether the agent verified its work before finishing:

1. After its last edit (writing the research note, and any other file), the agent ran `cargo xtask sources` (or `cargo xtask all`, which includes it) and it succeeded, and the final message reports that result as it was, not as a pass it did not see. If the agent changed anything cargo builds -- a `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml`, or Rust code under `crates/` or `xtask/` -- it must also have run, after that change and with success, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, the workspace tests (`cargo nextest run` or `cargo test`), and `cargo xtask all`. Files under `docs/` are not such a change, whatever they hold: the note, `docs/decisions.md`, and the research sources directory `docs/research/sources/` -- its `MANIFEST.toml` and any file pinned there, such as a probe's output -- are read by `cargo xtask sources`, not by cargo, so that check covers them.

`devenv test` may be unavailable in this sandbox, so the plain `cargo` / `cargo xtask` equivalents count. A command that failed for a reason outside the code (no network, a missing tool) does not count as a success, but a later successful run of it does. Fail if no such check ran after the last edit, or if the last run of a required check failed.
