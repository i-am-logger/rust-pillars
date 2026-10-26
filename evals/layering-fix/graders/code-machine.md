---
type: llm
focus:
  source: file
  path: crates/rust-pillars-core/src/machine.rs
---
This is the core's state machine after the fix. Judge it from this file alone.

1. `Refusals` (what `Machine::refused()` returns) has a counter for effects dropped because the effect queue was full. Its name is free, but it is a counter the host can read (a `Tally`-like count), not a flag.
2. When pushing an effect onto the queue reports it was dropped (`Queued::Dropped`), the machine increments that counter. Nothing else is done with the drop.
3. `Refusals::total()` includes that counter along with the other refusals. The counter also appears in `Refusals`'s `Debug` output, whether `Debug` is derived or written by hand.
4. The file has no `log::` calls and no other way of calling back into the host. Nothing like a logger trait, closure, function pointer or callback is stored in or handed to the machine. The drop is only state that the host polls.
5. The file adds no `#[allow(...)]`, `#[expect(...)]`, `#![allow(...)]` or `#![expect(...)]` attribute.
6. The `#[cfg(test)]` tests in this file include one that overflows a small queue and asserts the dropped counter's exact value. For example, a `Machine<2>` fed three lines reports exactly 1 dropped, and `total()` agrees. The file keeps its tests of refusal after the end, of clearing the queue, of a last line with no newline, and of the idle threshold.
