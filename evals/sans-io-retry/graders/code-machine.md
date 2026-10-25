---
type: llm
focus:
  source: file
  path: crates/rust-code-core/src/machine.rs
---
This is the core state machine after the task (add reconnect-with-backoff: `Event::UpstreamFailed` in, `Effect::Retry(Millis)` out). Judge from this file alone:

1. The machine handles `Event::UpstreamFailed`, and a due retry is delivered as an `Effect::Retry` queued on the machine's effect queue (the same polled path `Effect::Idle` uses) on a `Tick`, carrying that tick's time. It is never delivered by calling the host: no stored callback, closure, trait object or function the machine invokes, and no clock read; the only time the backoff sees is from `Event::Tick`.
2. The backoff follows the contract: 100 ms for the first failure, doubling with each consecutive failure, capped at 1600 ms; counted from the most recent tick seen; fires once, on the first tick at least the backoff later; a failure while a retry is still owed replaces it (one outstanding at a time) and keeps doubling; a retry firing does not reset the backoff; only non-empty bytes reset it to 100 ms (an empty read does not). `Machine::new` keeps its signature (one idle-threshold argument).
3. The backoff state is typed: the wait and the times it counts from are held as domain types (such as `Millis`, `Elapsed` or a domain newtype), not bare `u64`; whether a retry is owed is an enum with named states (or an `Option` of typed data), not a `bool`. No `bool` field or parameter is added, and no public function or field added here takes or exposes a raw integer or `bool`.
4. The doubling cannot overflow: it uses a saturating or checked operation (or a domain method that does), or caps before multiplying; there is no plain `* 2` or `<<` on a raw integer that could wrap.

The names of private items (fields, enums, constants, helper methods) are free; judge the properties, not the spelling. Fail if any item is clearly not met.
