---
name: own-dont-leak
description: Clippy rejects a Box::leak'd startup lookup table in the binary; fix it properly, keeping output identical and the table zero-copy.
tags: [smoke]
runs: 3
max_turns: 40
timeout_seconds: 900
---
`cargo clippy -p rust-code -- -D warnings` fails on the startup table I added in the binary (the bucket labels in `crates/rust-code/src/buckets.rs`). Can you fix it properly? Keep the behaviour exactly as it is (same output for the same input), and keep it zero-copy: the table is built once, so no cloning it per use.
