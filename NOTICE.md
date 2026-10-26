# Notice

Copyright (c) 2026 i-am-logger

Every path and link in this file is relative to the root of the rust-pillars
repository, <https://github.com/i-am-logger/rust-pillars>. A skill zip from
`dist:package` carries this file and [LICENSE](LICENSE) in the skill's
folder, but not the other files named here; in the `rust-idioms` zip,
BeautifulRust's licence is `LICENSE-BeautifulRust.txt` beside `SKILL.md`.

rust-pillars is licensed under the Creative Commons
Attribution-NonCommercial-ShareAlike 4.0 International License
(CC-BY-NC-SA-4.0). [LICENSE](LICENSE) is that licence's legal code, verbatim
from <https://creativecommons.org/licenses/by-nc-sa/4.0/legalcode.txt>.

The `rust-idioms` skill adapts rules from
[BeautifulRust](https://github.com/morganlinton/BeautifulRust) (commit
`6b3a96c`), used under the MIT License. Its text ships with that skill in
[skills/rust-idioms/LICENSE-BeautifulRust.txt](skills/rust-idioms/LICENSE-BeautifulRust.txt),
so it travels with the skill when the skill is packaged or uploaded on its
own.

## Third-party material

The material below comes from other projects. Most of it is unmodified
copies of their files, kept as pinned research sources, each with its
sha256 in
[docs/research/sources/MANIFEST.toml](docs/research/sources/MANIFEST.toml).
Two items differ: the research-note eval's fixture copy of the Cargo
features chapter is pinned in that fixture's own
[MANIFEST.toml](evals/research-note/fixture/docs/research/sources/MANIFEST.toml),
and the eval's model solution holds an attributed excerpt of that chapter,
not a copy, with no sha256. None of it is under CC-BY-NC-SA-4.0: each item
stays under its own licence. The licence texts, from each upstream at the
same commit, are in [docs/research/sources](docs/research/sources/README.md)
and cover every copy and quotation named here, wherever in this repository
it sits; the fixture also carries cargo's two beside its copy.

- [rust-lang/api-guidelines](https://github.com/rust-lang/api-guidelines) at
  `97a0969cb07fe4cabb0eed8a56234053f47d83dc`, under MIT OR Apache-2.0,
  "Copyright (c) 2017 The Rust Project Developers":
  [checklist](docs/research/sources/rust-api-guidelines-97a0969-checklist.md)
  (`src/checklist.md`) and
  [type safety](docs/research/sources/rust-api-guidelines-97a0969-type-safety.md)
  (`src/type-safety.md`). Licences:
  [LICENSE-MIT](docs/research/sources/rust-api-guidelines-97a0969-LICENSE-MIT),
  [LICENSE-APACHE](docs/research/sources/rust-api-guidelines-97a0969-LICENSE-APACHE).
- [rust-lang/cargo](https://github.com/rust-lang/cargo) at
  `0f958632a72dbd5505cd9815ad1d615b0eab4af9`, under MIT OR Apache-2.0 (its
  LICENSE-MIT carries no copyright line): the Cargo Book's
  [features](docs/research/sources/cargo-0f95863-features.md) and
  [manifest](docs/research/sources/cargo-0f95863-manifest.md) chapters
  (`doc/book/src/reference/features.md`, `manifest.md`). The features
  chapter is also copied, identical, into the research-note eval's fixture,
  [evals/research-note/fixture/docs/research/sources/cargo-0f95863-features.md](evals/research-note/fixture/docs/research/sources/cargo-0f95863-features.md),
  and the eval's model solution,
  [a-domain-std-feature.md](evals/research-note/solution/docs/research/notes/a-domain-std-feature.md),
  quotes it with attribution. Licences:
  [LICENSE-MIT](docs/research/sources/cargo-0f95863-LICENSE-MIT),
  [LICENSE-APACHE](docs/research/sources/cargo-0f95863-LICENSE-APACHE).
- [rust-lang/rust-clippy](https://github.com/rust-lang/rust-clippy) at
  `43c0efca149bb15b9cd2bbf2ef34771cefa70a7e`, under MIT OR Apache-2.0,
  "Copyright (c) The Rust Project Contributors":
  [lint configuration](docs/research/sources/clippy-43c0efc-lint-configuration.md)
  (`book/src/lint_configuration.md`). Licences:
  [COPYRIGHT](docs/research/sources/clippy-43c0efc-COPYRIGHT),
  [LICENSE-MIT](docs/research/sources/clippy-43c0efc-LICENSE-MIT),
  [LICENSE-APACHE](docs/research/sources/clippy-43c0efc-LICENSE-APACHE).
- [morganlinton/BeautifulRust](https://github.com/morganlinton/BeautifulRust)
  at `6b3a96c`, under MIT:
  [SKILL.md](docs/research/sources/beautifulrust-6b3a96c-SKILL.md)
  (`skills/rust-best-practices/SKILL.md`). Licence:
  [LICENSE](docs/research/sources/beautifulrust-6b3a96c-LICENSE).
