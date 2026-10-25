# Sources

Each file here is pinned in [MANIFEST.toml](MANIFEST.toml) by sha256 at the
upstream commit it was read from (`cargo xtask sources` re-hashes every one).
Two kinds sit side by side:

- **Cited sources**: the primary texts a note or a skill cites with
  `[[source: <key>]]`. Each is under MIT, or MIT OR Apache-2.0.
- **Licence texts**: each upstream's `LICENSE`, `LICENSE-MIT`,
  `LICENSE-APACHE` or `COPYRIGHT` file, from the same commit as the copies
  it covers. They are here because committing a copy is distributing it, and
  the copy's licence must go with it; nothing cites them. Each
  `*-LICENSE-APACHE` file is the Apache-2.0 text alone, and its MANIFEST
  entry says `Apache-2.0`.

[NOTICE.md](../../../NOTICE.md) names every copy with its upstream, commit and
licence.

Effective Rust (CC BY-NC-ND) is cited by name and never committed or
paraphrased here.
