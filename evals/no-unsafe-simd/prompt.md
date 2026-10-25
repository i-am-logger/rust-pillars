---
name: no-unsafe-simd
description: A core crate opts out of the workspace lints to use unsafe SIMD intrinsics; make it safe, keep it fast, and prove it against a naive scan.
runs: 3
max_turns: 40
timeout_seconds: 900
---
I added `crates/rust-code-scan` for the reader: `first_newline` finds the first `\n` in a buffer, 16 bytes at a time with SSE2/NEON intrinsics. To get the intrinsics past the build I gave it its own `[lints]` table instead of `workspace = true`, and now `cargo xtask arch` fails on it.

Please make it fit the workspace rules properly: no `unsafe` anywhere, the crate inherits the workspace lints like every other crate, and every gate passes. It still has to be fast -- this runs on every byte we read, so don't just swap in a byte-by-byte loop. Keep the signature `pub fn first_newline(haystack: &[u8]) -> Option<usize>`; the binary is going to call it next.

Also add a test that checks `first_newline` agrees with a plain byte-by-byte search over lots of different inputs, so we know the fast version is right.
