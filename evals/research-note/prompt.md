---
name: research-note
description: Decide, from the pinned Cargo Book text and the workspace's own rules, whether an optional `std` feature on the domain crate (for std::error::Error impls) is allowed, and record it as a research note with a status and a decision.
runs: 3
max_turns: 40
timeout_seconds: 900
---

We want an optional `std` feature on the domain crate (`rust-code-domain`) that pulls in `std::error::Error` impls for its error types, so callers on a host can use them with `?` and `Box<dyn Error>`. Before anyone writes it: is that allowed by Cargo's feature rules, and by the rules this workspace holds the domain crate to?

Write it up as a research note, `docs/research/notes/a-domain-std-feature.md`, citing the Cargo Book (we already have the features chapter pinned under `docs/research/sources/`), and say what you decide. Leave the repo's checks as green as they are now.
