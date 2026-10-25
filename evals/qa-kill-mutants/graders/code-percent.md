---
type: llm
focus:
  source: file
  path: crates/rust-code-domain/src/percent.rs
---
Judge this file, `crates/rust-code-domain/src/percent.rs`, from its contents alone. Before the task, everything above `#[cfg(test)]` read exactly as below, and the test module held a single test, that 200 of 100 is `Percent::FULL`:

```rust
impl Percent {
    pub const ZERO: Self = Self(0);
    pub const FULL: Self = Self(100);

    #[must_use]
    pub fn through(done: BytePos, total: BytePos) -> Self {
        if done.0 >= total.0 {
            return Self::FULL;
        }
        let whole = u128::from(done.0) * 100 / u128::from(total.0);
        Self(u8::try_from(whole).unwrap_or(100))
    }
}
```

(doc comments and the two `//` comment lines omitted here.)

1. The code above the test module is unchanged: the same `ZERO` and `FULL` constants, the same `>=` clamp, the same `u128` arithmetic `done * 100 / total`, the same `u8::try_from(..).unwrap_or(100)`. No attribute such as `#[mutants::skip]`, `#[cfg(mutants)]` or a `cfg_attr` naming mutants was added anywhere in the file, and no test is marked `#[ignore]`.
2. The tests now reach the arithmetic below 100, not only the clamp: they include positions strictly short of the end with a non-trivial result (for example 50 of 100 is 50, 99 of 100 is 99, 1 of 3 is 33), so that replacing `*` or `/` with another operator, or `100` with another constant, changes an asserted result.
3. The tests pin the boundary of the clamp: a position exactly at the end is `FULL` (so `>=` turned into `>` is caught), and the start of a non-empty stream is `ZERO` or 0.
4. The assertions compare against exact expected values (`assert_eq!` against a specific `Percent`, or against an independently computed expected value such as a counting loop or other definition that does not call `through` itself), not weak properties such as "is at most 100" or "is not zero" alone.
5. No test reads the source text or otherwise detects mutation instead of checking behaviour: no `include_str!`/`include_bytes!`, no reading `percent.rs` or any file from disk, no checksum or comparison of the code.
