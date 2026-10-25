# Decisions

Each decision is numbered, states its claim, and carries three parts:
**Evidence** (measured, or by reading, said which), **Cost** (what it gives
up), and **Reverses if** (the observation that would undo it). A decision
that departs from a governing text says so and names the text.

A decision the evidence does not settle goes to the judge protocol (the
`rust-research` skill): two independent asks with the options reordered,
0.9 confidence both times, otherwise one re-ask with more context, then the
owner, who holds a veto.

<!-- ### D1. <claim>
**Evidence:** ...
**Cost:** ...
**Reverses if:** ... -->

### D1. The domain crate implements `core::error::Error` with no feature; it gets no `std` feature
**Evidence:** by-reading and measured, in
[note A](research/notes/a-domain-std-feature.md). By-reading: the Cargo Book
allows an additive `std` feature that is off by default, and forbids a
`no_std` feature ([[source: cargo-features]] "Feature unification", lines
284-291). `xtask/src/arch.rs` `pure()` fails `std` in `default`. Measured on
rustc 1.94.1: a `#![no_std]` crate's `impl core::error::Error` builds for
`x86_64-unknown-none`, and a host uses it with `?` into
`Box<dyn std::error::Error>`. Inferred, unpinned: `core::error::Error` is
stable since 1.81, below the 1.85 MSRV.
**Cost:** none known today. The domain crate has no error type yet. Hosts
that want `std::error::Error` get it through `core::error::Error`, which the
std trait is inferred to re-export (unpinned).
**Reverses if:** a domain error needs a source type only `std` has (which
would be I/O in the domain, against pillar 2), or the MSRV drops below the
release that stabilised `core::error::Error`.
