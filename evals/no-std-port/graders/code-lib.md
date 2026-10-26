---
type: llm
focus:
  source: file
  path: crates/rust-pillars-checksum/src/lib.rs
---
This is `crates/rust-pillars-checksum/src/lib.rs` after porting the Adler-32 crate to a no_std, sans-IO domain crate. Judge it from its contents alone.

1. Its input is borrowed bytes fed to an incremental hasher: an `update`-style method taking a slice (or a newtype over one) that can be called any number of times, and a way to finish it into a checksum. There is no function that pulls bytes from a source, and no unconditional `extern crate alloc` or `extern crate std` (a `#[cfg(test)]` one is fine).
2. The crate needs no allocator: its non-test code uses no `Vec`, `String`, `format!`, `Box` or `to_string` (a `#[cfg(test)]` module may use `std` and `Vec`).
3. The Adler-32 arithmetic is still here and still correct: modulus 65521, `a` starting at 1 and `b` at 0, both reduced often enough that the `u32` sums cannot overflow on a long run of `0xff`, and the result `(b << 16) | a`.
4. Nothing is silenced: no `#[allow(...)]` or `#[expect(...)]` attribute anywhere in the file.

Pass only if every item holds.
