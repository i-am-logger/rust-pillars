---
name: embed-at-build-time
description: Startup reads a data file from a cwd-relative path on every launch; embed it in the binary at build time so it runs from any directory, without letting the embedded copy drift from the asset.
tags: [smoke]
runs: 3
max_turns: 40
timeout_seconds: 900
---

Every time `rust-code` starts it reads the stopword list from `crates/rust-code/assets/stopwords.txt` on disk, with a path relative to the current directory. So it only works when you launch it from the repo root: run `rust-code stopwords` (or just pipe text into it) from anywhere else and it dies with "loading the stopword list".

Embed the list in the binary at build time instead, so the built binary works from any directory and doesn't need the file at run time. Keep the list where it is, in `crates/rust-code/assets/stopwords.txt` -- that's the file people edit -- and make sure the copy in the binary can't silently drift from it: when someone edits the asset, the next build has to pick the change up. `rust-code stopwords` should keep printing how many words are in the list.
