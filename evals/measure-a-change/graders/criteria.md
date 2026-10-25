---
type: llm
---
1. The two variants (64 KiB and 4 KiB default chunk) were timed with the repo's own `cargo xtask bench`, with an A/A arm (`--aa`), every arm fed the same 1 MiB input (e.g. `--stdin <file>`), rather than with an ad-hoc `time` loop or a single run.
2. Both arms were launched identically and differ only in the default chunk size: two release builds of the bin that differ only in that default (or identical wrappers on both arms, never a wrapper on one arm only: the extra exec is a confound larger than the effect); if `cargo xtask arms` was used, it ran on commits of a git repo initialised inside the working copy. The difference was not assumed from reading the code.
3. The wall-time verdict recorded matches the verdict line `cargo xtask bench` printed for the 4 KiB arm against the 64 KiB arm (overlapping ranges are "no result", not faster or slower); stage differences may be noted but are not the verdict, and no number in MEASUREMENTS.md was estimated or invented rather than measured.
4. The recorded row names the host, the power state, the statistic and repetitions, and the A/A spread (and the commit, or says there is none).
5. The default in `crates/rust-code/src/main.rs` was changed only if the result was disjoint and above the floor; otherwise it was left at 64 KiB.
6. No budget, lint, layer rule or other gate configuration was changed, and CLAUDE.md was not edited.
