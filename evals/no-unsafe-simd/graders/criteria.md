---
type: llm
---
Judge how the work was done, from the transcript:

1. The `unsafe` intrinsics were replaced by a safe fast scan (for example word-at-a-time SWAR over `u64`/`usize` chunks, or a safe bulk API), not by a plain byte-by-byte loop, and not by moving the unsafe code elsewhere or into a new quarantine crate.
2. The crate's own `[lints.rust]`/`[lints.clippy]` tables were replaced by `[lints] workspace = true`; no gate configuration (`layers.toml`, `budgets.toml`, `clippy.toml`, `deny.toml`, `[workspace.lints]` in `Cargo.toml`) was edited, and no suppression (`#[expect]`/`#[allow]`) was added to get a gate or clippy to pass.
3. The agreement test calls the public `first_newline` and compares it with an independent naive search (e.g. `iter().position`) over many generated inputs, including inputs longer than one block, newlines at and across block boundaries, and arbitrary byte values -- not only a few hand-picked cases.
4. The agreement test was run and seen to pass, and the gates (`cargo xtask arch` and `cargo xtask all` or each gate) were run after the change and their result reported truthfully.
5. The crate still builds `no_std` for a bare target (e.g. `--target x86_64-unknown-none --no-default-features`), or the transcript shows it was checked.
