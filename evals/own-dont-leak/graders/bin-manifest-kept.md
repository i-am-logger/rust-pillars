---
type: regex
pattern: '\[dependencies\]\s*\nrust-code-core\.workspace = true\nclap\.workspace = true\nlog\.workspace = true\nenv_logger\.workspace = true\nthiserror\.workspace = true\s*\n\s*\[lints\]\s*\nworkspace = true\s*$'
match: contains
target:
  source: file
  path: crates/rust-code/Cargo.toml
---
The binary's manifest is as it was: no new dependency added to hold the table, and its lints still come from the workspace with no local override.
