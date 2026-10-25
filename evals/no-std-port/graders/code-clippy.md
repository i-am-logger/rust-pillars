---
type: llm
focus:
  source: file
  path: crates/rust-code-checksum/clippy.toml
---
This should be the crate-local `crates/rust-code-checksum/clippy.toml` of a domain crate. Clippy reads the clippy.toml nearest a crate's manifest, so a domain crate's copy must keep every ban of the workspace `clippy.toml` and add the sans-IO ones. Judge it from its contents alone; if the file is missing or empty, fail.

1. It keeps the workspace settings and bans: `allow-unwrap-in-tests = true`, `allow-expect-in-tests = true`, `too-many-lines-threshold = 80` (not raised), and `disallowed-methods` entries for all ten of `std::boxed::Box::leak`, `std::vec::Vec::leak`, `std::string::String::leak`, `alloc::boxed::Box::leak`, `alloc::vec::Vec::leak`, `alloc::string::String::leak`, `std::mem::forget`, `std::mem::ManuallyDrop::new`, `core::mem::forget` and `core::mem::ManuallyDrop::new`.
2. It adds the sans-IO bans the other domain crates carry: no thread spawning (`std::thread::spawn` in `disallowed-methods`), no clock (`std::time::Instant` and `std::time::SystemTime` in `disallowed-types`), and ideally no ambient randomness and no `std::thread::Thread`. More bans than these are fine.

Pass only if both items hold.
