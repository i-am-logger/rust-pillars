---
name: no-std-port
description: A new Adler-32 crate was given the domain role but written against std (io::Read, Vec, String errors); port it to a proper no_std domain crate while the binary's --checksum keeps working.
runs: 3
max_turns: 40
timeout_seconds: 900
---
I added a small crate, `crates/rust-code-checksum`, that does Adler-32 checksums, and wired it into the binary as `rust-code --checksum` (prints the checksum of stdin as eight hex digits). I registered it as a domain crate in `layers.toml` and declared its seams in `budgets.toml`, but I wrote it quickly against std -- it reads through `std::io::Read`, buffers into a `Vec`, and its errors are `String`s -- and now the gates are unhappy with it.

Please turn it into a proper domain crate like `rust-code-domain`: it has to build for bare-metal targets with no std, the same as the other domain and core crates, and it shouldn't do any I/O itself -- reading stdin is the binary's job. Leave `layers.toml` and `budgets.toml` as they are.

`rust-code --checksum` has to keep printing exactly what it prints now, e.g.

```
printf Wikipedia | cargo run -q -- --checksum
11e60398
```

and it should still get its checksums from the crate. While you're in there, I'd like a property test, or an exhaustive check over every byte value, that shows feeding the input in pieces gives the same checksum as feeding it whole -- the current tests are just a few hand-picked examples.
