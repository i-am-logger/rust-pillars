---
description: Adopt the rust-code gates in an existing Rust workspace, strictly or as a ratchet
argument-hint: "[--ratchet]"
allowed-tools: Bash, Read, Grep, Glob, Edit, Write
---

Adopt rust-template's gates in this workspace. Ask before each step that
changes files, and say what it will change.

1. Read the workspace (`cargo metadata --no-deps`) and propose a role for
   every crate -- domain, core, adapter, bin, tool, harness or quarantine --
   with one line of evidence each (no_std? does I/O? has `unsafe`? benchmarks
   or difftests?). Wait for the user to confirm or change the roles.
2. Bring in the gates from rust-template: `xtask/`, `.cargo/config.toml`'s
   `xtask` alias, `clippy.toml` and the crate-local ones for domain and core,
   `[workspace.lints]`, `deny.toml`, and the devenv tasks. Show the diff.
3. Run `cargo xtask init` (every budget 0, so every violation fails a gate --
   what adopting the pillars means) or, with `--ratchet` in `$ARGUMENTS`,
   `cargo xtask init --ratchet` (today's counts, which may only shrink).
   Then write the confirmed roles into `layers.toml`.
4. Run `cargo xtask all` and summarise the findings by gate and pillar, with
   the order to fix them in: no unsafe and layering first, then the type
   system, then tests and docs. Fix nothing yet.
