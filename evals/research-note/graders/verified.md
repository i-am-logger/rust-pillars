---
type: llm
focus: trace
---
Judge from the trace whether the agent verified its work before finishing:

1. After its last edit (writing the research note, and any other file), the agent ran `cargo xtask sources` (or `cargo xtask all`, which includes it) and it succeeded, and the final message reports that result as it was, not as a pass it did not see. If the agent added or changed any file other than Markdown and `MANIFEST.toml` -- a `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml`, or any `.rs`, `.sh` or `.nix` file anywhere in the tree, a probe's sources pinned under `docs/` included -- it must also have run, after that change and with success, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, the workspace tests (`cargo nextest run` or `cargo test`), the formatter check (`treefmt --fail-on-change`, which reads those files wherever they are), and `cargo xtask all`. A change only to Markdown files and `MANIFEST.toml` -- the note, `docs/decisions.md`, a pin's entry and its Markdown or text files under `docs/research/sources/` -- is covered by `cargo xtask sources`.

`devenv test` may be unavailable in this sandbox, so the plain `cargo` / `cargo xtask` equivalents count. A command that failed for a reason outside the code (no network, a missing tool) does not count as a success, but a later successful run of it does. Fail if no such check ran after the last edit, or if the last run of a required check failed.
