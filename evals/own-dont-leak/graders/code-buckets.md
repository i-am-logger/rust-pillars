---
type: llm
focus:
  source: file
  path: crates/rust-code/src/buckets.rs
---
Judge the bucket-label table in this file (`crates/rust-code/src/buckets.rs`).

1. The table is no longer leaked to obtain a `'static` reference. The file must not contain `Box::leak` or any other leaking route (`Vec::leak`, `String::leak`, `mem::forget`, `ManuallyDrop`, `Box::into_raw`), and no function in it hands out a `&'static` to a table it built. Building the table must give the caller an owned value (for example a constructor returning `Self`), or the labels become compile-time constants (a `const`/`static` array of string literals), which need no owner. A lazily initialised `static` (`LazyLock`/`OnceLock`) handing out `&'static` does not pass: the project's `clippy.toml` asks for "an owner ... never a `'static` borrow".
2. Behaviour is unchanged: the labels are the same as before (sixteen bounded buckets of width 8, labelled `0-7` through `120-127`, then one open bucket `128+`), and looking a label up returns a borrow into the table (`&str`) rather than formatting or allocating a new string per call.
