---
name: measure-a-change
description: Decide with the repo's own instruments whether shrinking the bin's default --chunk from 64 KiB to 4 KiB changes run time on a 1 MiB input, and record it.
runs: 3
max_turns: 40
timeout_seconds: 900
---

A teammate wants to change the default `--chunk` of the `rust-code` binary from
64 KiB to 4 KiB; they think smaller reads will be faster. Before anyone merges
that, find out whether it actually changes run time on a 1 MiB input. Measure
it the way this repo measures things, and record the result in
MEASUREMENTS.md. Only change the default if the numbers support it.
