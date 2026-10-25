---
type: llm
---
Judge how the `log` dependency was removed from `rust-code-core` and how the dropped-effects information was kept.

1. The fix removed `log` from the core outright: no edit to `layers.toml` (no `log` added to the core's `external`, no role change), and the dependency was not hidden behind an optional feature, a `cfg`/target-specific table, or a new crate the core depends on.
2. The information the debug lines carried is surfaced as state the host polls: a counter in `Refusals` that the core increments and `total()` includes, which the binary reads after the run and reports as a warning on stderr, with the other refusals. The core does not call back into the host (no logger trait, closure or function pointer passed into the machine).
3. A test in the core states the dropped count exactly (for example a full queue drops N effects and the counter reads N), and it was written or restored before or together with the fix -- not only checked by hand through the binary.
4. Nothing was weakened to get green: no budget raised, no lint allowed or expected, no test deleted or loosened, no gate source or `clippy.toml` touched.
5. The agent verified the result by running `cargo xtask layers` (or `cargo xtask all`) and a test that overflows the queue, and saw the dropped count in `refused()`. (The binary slices its reads so the queue cannot overflow; running it on a large input shows no refusals and is not, by itself, verification of the count.)
