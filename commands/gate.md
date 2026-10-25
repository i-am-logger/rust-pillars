---
description: Run every rust-code gate on this project and report what fails, by pillar
argument-hint: "[gate]  e.g. budgets, arch, layers, tests, coverage, snippets"
allowed-tools: Bash, Read, Grep, Glob
---

Run the project's gates and report the result by pillar.

1. If the project has `devenv.nix` with `test:*` tasks and `$ARGUMENTS` is
   empty, run `devenv test`: every gate, as CI runs it. Otherwise run
   `cargo xtask $ARGUMENTS`, or `cargo xtask all` when no gate is named.
2. If the project has no `xtask` gates (it was not made from rust-template),
   say so, run `cargo fmt --check`, `cargo clippy --all-targets --all-features
   -- -D warnings` and `cargo test --all-features`, and suggest
   `/rust-code:adopt`.
3. Report each failing gate with its findings, grouped by pillar (type system,
   event-driven, layering, zero-copy, no_std, no unsafe, tests, docs), and for
   each the smallest fix that keeps the rule. Never propose raising a budget or
   loosening a lint to make a gate pass; a budget moves only in its own commit,
   with the reason, when the count itself went down.
4. Do not change any file unless asked to fix what was found.
