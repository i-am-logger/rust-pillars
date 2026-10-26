# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - Unreleased

First public release.

### Added

- Seven skills: the `rust-pillars` entry skill, which routes to six specialists
  (`rust-domain`, `rust-architecture`, `rust-idioms`, `rust-measure`,
  `rust-qa`, `rust-research`).
- Two commands, `/rust-pillars:gate` and `/rust-pillars:adopt`, and the `refuter`
  agent.
- Two hooks: a one-line nudge to load the skills on a Rust prompt (on by
  default), and the opt-in `gate_on_edit` hook, which runs the project's
  gates after each `.rs` edit.
- Ten eval cases, run under `claude plugin eval` and by the Rust harness in
  `eval-harness/`.
- The gates this repository holds itself to, in `xtask/`.
