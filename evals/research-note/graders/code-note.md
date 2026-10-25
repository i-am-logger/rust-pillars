---
type: llm
focus:
  source: file
  path: docs/research/notes/a-domain-std-feature.md
---
Judge the research note on whether an optional `std` feature on the domain crate is allowed. Pass only if all hold:

1. It cites the pinned Cargo Book features chapter as `[[source: <key>]]` with the clause or section it relies on (for example "Feature unification", the `default` feature warning, or SemVer compatibility), not only a URL.
2. It has a filled-in status line (for example `Status: decided` with a date), not a placeholder.
3. It weighs the options against both authorities, the Cargo Book and this workspace's own rules (the domain crate is `no_std`, and a gate checks that the `default` features do not turn on `std`), marking each option CONFORMING or DEPARTS against each, with the reason.
4. Its reasoning is right on the facts: an additive feature that is off by default is allowed by Cargo's rules; a subtractive `no_std` feature is against them; putting `std` in `default` would break the workspace's `no_std` rule. It may decide for the feature or for an alternative (for example implementing `core::error::Error` with no feature), as long as the decision is stated explicitly and follows from what it found.
5. It says which claims were read in a source, which were measured (a build or command run), and which are inferred, and does not present an inference as measured.
