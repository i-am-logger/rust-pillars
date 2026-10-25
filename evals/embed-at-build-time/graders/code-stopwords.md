---
type: llm
focus:
  source: file
  path: crates/rust-code/src/stopwords.rs
---
This is the stopword module of the `rust-code` binary. The task was to stop reading `crates/rust-code/assets/stopwords.txt` from disk at run time and embed it at build time instead, so the asset stays the one copy people edit and an edit to it reaches the next build. Judge from this file alone.

1. The module does not read the list from disk at run time: no `fs::read_to_string`, `File::open`, `fs::read` or similar on the load path, whether by a relative path or an absolute one built at run time. A read inside a `#[cfg(test)]` test that compares the embed with the asset is fine.
2. The list's text comes from a build-time embed of the asset file itself. Any of these counts: `include_str!`/`include_bytes!` of `assets/stopwords.txt` (a relative path or one built with `concat!(env!("CARGO_MANIFEST_DIR"), ...)`), an `include!` of a build-script output in `OUT_DIR`, or text handed in by the caller (for example from `main.rs`), where this module embeds nothing itself. It FAILS if the words are pasted into Rust source as a hand-maintained array, string or `const` copy that would not change when the asset is edited.
3. If the file holds a test of the embedded list, that test could fail for its reason: it checks the list's actual contents, such as known words from the asset, the exact count, or that comments are skipped. An assertion that holds whatever the file contains (such as `len() >= 0` or that loading does not panic) does not count. If the file adds no test, this item passes.
