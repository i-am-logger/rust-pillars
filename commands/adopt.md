---
description: Adopt the rust-code gates (CC BY-NC-SA 4.0 files from rust-template) in an existing Rust workspace, strictly or as a ratchet
argument-hint: "[--ratchet]"
allowed-tools: Bash(cargo metadata:*), Bash(cargo xtask all:*), Read, Grep, Glob
---

Adopt rust-template's gates in this workspace. Ask before each step that
changes files, and say what it will change. Only `cargo metadata`,
`cargo xtask all` and reading run without the user's approval; every edit,
file written and other command asks.

1. Read the workspace (`cargo metadata --no-deps`) and propose a role for
   every crate -- domain, core, adapter, bin, tool, harness or quarantine --
   with one line of evidence each (no_std? does I/O? has `unsafe`? benchmarks
   or difftests?). Wait for the user to confirm or change the roles.
2. Before copying anything, tell the user the licence of what step 3 brings
   in, and ask whether to go on. The files come from rust-template
   (<https://github.com/i-am-logger/rust-template>), licensed Creative Commons
   Attribution-NonCommercial-ShareAlike 4.0 International (CC BY-NC-SA 4.0,
   <https://creativecommons.org/licenses/by-nc-sa/4.0/>): they may be used
   and adapted only for non-commercial purposes; whoever shares them, changed
   or not, must credit rust-template, keep its licence notice and say whether
   they changed them; and shared adaptations must carry the same licence. Say
   that this is a summary of the licence, not legal advice. If the user does
   not agree, stop here: copy nothing.
3. Bring in the gates from rust-template: `xtask/`, `.cargo/config.toml`'s
   `xtask` alias, `clippy.toml` and the crate-local ones for domain and core,
   `[workspace.lints]`, `deny.toml`, and the devenv tasks, with rust-template's
   licence notice beside them. Show the diff.
4. Run `cargo xtask init` (every budget 0, so every violation fails a gate --
   what adopting the pillars means) or, with `--ratchet` in `$ARGUMENTS`,
   `cargo xtask init --ratchet` (today's counts, which may only shrink).
   Then write the confirmed roles into `layers.toml`.
5. Run `cargo xtask all` and summarise the findings by gate and pillar, with
   the order to fix them in: no unsafe and layering first, then the type
   system, then tests and docs. Fix nothing yet.
