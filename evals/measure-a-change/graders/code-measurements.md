---
type: llm
focus:
  source: file
  path: MEASUREMENTS.md
---
Judge only the new record this file adds for the default `--chunk` change (64 KiB against 4 KiB on a 1 MiB input); the file's opening prose (the rules about rows, A/A spread and overlap) was there before and earns nothing. Pass only if every point holds:

1. The record says the two defaults were timed with the repo's own `cargo xtask bench` with an A/A arm (`--aa`), every arm fed the same 1 MiB input (e.g. `--stdin <file>`), not with an ad-hoc `time` loop or a single run.
2. The record says how the two arms were made and that they differ only in the default chunk size (e.g. two release builds of the bin differing only in that default, each run on its own default, or identical wrappers on both arms; never a wrapper on one arm only). The difference is presented as measured, not inferred from reading the code.
3. The record states a wall-time verdict for the 4 KiB arm against the 64 KiB arm in the bench's terms, and it is consistent with the numbers recorded: overlapping ranges are "no result", not faster or slower; a disjoint difference under the floor is not a win. Stage timings, if given, are not presented as the verdict. Every number is presented as measured; none is marked as an estimate, a guess or an expectation.
4. The record names the host, the power state, the statistic and the number of repetitions, and the A/A spread, and names the commit or says there is none.
5. The record says what happens to the default: it moves to 4 KiB only if the verdict is disjoint, faster and above the floor; otherwise the record says the default stays at 64 KiB.
