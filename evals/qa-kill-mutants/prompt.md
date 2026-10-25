---
name: qa-kill-mutants
description: Mutation testing finds survivors in a small domain function; strengthen its tests until none survive, leaving the function and every gate as they are.
runs: 3
max_turns: 40
timeout_seconds: 900
---
I added `Percent::through` in `crates/rust-code-domain/src/percent.rs` for the progress bar, and mutation testing isn't happy with it:

    cargo mutants -p rust-code-domain --file crates/rust-code-domain/src/percent.rs --test-workspace=true

reports a bunch of missed mutants. The function itself is what I want -- please don't change its behaviour or its code. Strengthen the tests until no mutant survives, and leave the repo's checks as green as they are now.
