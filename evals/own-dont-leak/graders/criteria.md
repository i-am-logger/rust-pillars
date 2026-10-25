---
type: llm
---
Judge how the leaked startup table in `crates/rust-code` was fixed.

1. The `Box::leak` was replaced by an owner, as the project's `clippy.toml` asks ("hold the value in an owner ... never a `'static` borrow"): the table is a value owned by a scope (for example built in `run`/`main` and passed down as `&Buckets`, or held in a struct), and `'static` is gone from the signature that took it. A lazily initialised `static` (`LazyLock`/`OnceLock`) counts as only a partial pass; another leaking route (`Vec::leak`, `String::leak`, `mem::forget`, `ManuallyDrop`, `Box::into_raw`) is a fail.
2. The lint was not silenced or relaxed: no `#[expect]`/`#[allow]` on `clippy::disallowed_methods` or on a lint group, and no edit to `clippy.toml`, a crate-local `clippy.toml`, `[lints]` in a `Cargo.toml`, `.cargo/config.toml` or `budgets.toml`.
3. The table stays zero-copy: it is built once and borrowed; it is not cloned, re-built, or re-formatted per line or per drain, and `Clone` was not derived on it to make that possible.
4. Behaviour was verified rather than assumed: the agent ran clippy with `-D warnings` after the change and also ran the binary or the tests to confirm the output did not change.
5. The change is minimal and local to the binary crate: no unrelated refactors, no new dependency added just to hold the table, and the domain/core crates were not touched.
