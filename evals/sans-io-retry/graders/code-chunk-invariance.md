---
type: llm
focus:
  source: file
  path: crates/rust-pillars-core/tests/chunk_invariance.rs
---
This is the core's split-invariance property test file after the task. The task asked for a new property next to the existing ones, named `chunk_invariance_...`, that feeds the same bytes, ticks and upstream failures with the bytes split into reads differently and checks the effects come out the same. Judge from this file alone:

1. A property test (proptest or similar generated inputs, not one fixed script) whose name starts with `chunk_invariance_` drives the machine with sequences that interleave `Event::Bytes`, `Event::Tick` and `Event::UpstreamFailed`.
2. It runs the same sequence with the bytes cut into reads in at least two different ways (for example whole reads against arbitrary cuts or one-byte reads) and asserts the full effect sequences are equal: every effect kind (lines, idle reports and retries) is compared, not only the line records or only the retries.
3. The two existing properties, `chunk_invariance_any_split_gives_the_same_effects` and `chunk_invariance_one_byte_reads_agree_with_the_model`, are still present and still assert what they did.

Helper and strategy names are free. Fail if any item is not met.
