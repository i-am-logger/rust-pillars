---
type: llm
focus:
  source: file
  path: crates/rust-pillars-checksum/tests/adler.rs
---
This is the crate's existing test file `crates/rust-pillars-checksum/tests/adler.rs` after the Adler-32 crate was ported to no_std and its reading moved into the binary. Judge it from its contents alone.

1. No existing check was deleted or loosened: it still asserts the known values (the empty input is `0x0000_0001`, `a` is `0x0062_0062`, `abc` is `0x024d_0127`, `Wikipedia` is `0x11e6_0398`), that feeding `Wiki`, an empty piece and `pedia` equals feeding `Wikipedia` whole, that 100 000 bytes of `0xff` give `0x149a_302c`, that a checksum prints as and parses from `11e60398`, and that `11e6039`, `+1e60398` and `11e6039g` are refused. Refusals may be asserted more strictly (a specific error variant) but not more loosely; no test is `#[ignore]`d.
2. It tests only the crate's I/O-free API: it calls no reading function of the crate (such as `read_all`) and no `std::io::Read` through the crate. The old reader test may be gone, since reading is now the binary's job.

Pass only if both items hold.
