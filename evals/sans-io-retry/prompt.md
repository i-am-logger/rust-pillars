---
name: sans-io-retry
description: Add reconnect/backoff to the sans-IO core machine -- a failure event in, a polled retry effect out, time only through Tick -- with a split-invariance property test.
runs: 3
max_turns: 40
timeout_seconds: 900
---
I want the core machine to cope with a flaky upstream. Please add a reconnect-with-backoff behaviour to `rust-code-core`:

- A new input, `Event::UpstreamFailed` (no payload): the host tells the machine its upstream connection just failed.
- A new output, `Effect::Retry(Millis)`: the machine tells the host to try reconnecting now. The `Millis` is the time of the `Tick` on which the retry became due.
- The backoff starts at 100 ms and doubles with every consecutive failure, capped at 1600 ms. Hard-code those two for now; `Machine::new` keeps its current signature.
- A failure has no timestamp of its own: count its backoff from the most recent `Tick` the machine has seen. The retry fires on the first `Tick` at which at least the backoff has passed, and exactly once.
- Only one retry is outstanding at a time: a failure while one is still pending is just another consecutive failure and replaces it. A retry firing is not a success -- if the host's reconnect fails it sends `UpstreamFailed` again and the backoff keeps doubling.
- Any non-empty `Bytes` means the upstream is healthy again, so the next failure starts over at 100 ms.
- Time must only come in through the existing `Event::Tick`; the core doesn't get a clock.

Tests: unit tests for the doubling, the cap and the reset, plus a property test next to the existing `chunk_invariance_*` ones in `crates/rust-code-core/tests/` (name it `chunk_invariance_...` too) that feeds the same bytes, ticks and failures but with the bytes split into reads differently, and checks the effects come out the same.

The binary has no upstream yet, so it only needs to print retry effects the way it prints idle ones. Keep the workspace green as usual.
