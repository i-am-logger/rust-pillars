---
type: llm
focus:
  source: file
  path: crates/rust-code/src/main.rs
---
Judge how the driver (`crates/rust-code/src/main.rs`) holds and uses the bucket-label table.

1. The table has an owner in a scope: it is a value built once (for example in `run`, or in a struct that `run` owns) and lent down by reference to whatever prints a line (`pump`, `drain`, or their equivalents); or, if `buckets.rs` turned the labels into compile-time constants, the driver uses those directly and builds nothing. No parameter, field or variable in this file is typed `&'static` for the table, and the file does not leak it (`Box::leak`, `Vec::leak`, `mem::forget`, `ManuallyDrop`, `Box::into_raw`) or read it from a lazily initialised `static`.
2. The table stays zero-copy: it is built once per run (the test may build its own once), not cloned, re-built or re-formatted per read, per line or per drain; no `.clone()`/`.to_vec()` of it appears.
3. The change to this file is local to the table: apart from how the table is created, passed and looked up, the driver (argument parsing, the read/drain loop, the output format string, the test's assertions) is as it was, with no unrelated refactor.
