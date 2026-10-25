# Contributing

## Setup

This project uses [devenv](https://devenv.sh) to pin the exact Rust toolchain
and tool versions CI uses:

```bash
direnv allow   # or: devenv shell
```

Everything below assumes you are inside that shell (or run each command as
`devenv shell -- <command>`).

## Before opening a PR

Run the full local pipeline:

```bash
dev-ci
```

This is not an approximation of CI — `.github/workflows/ci.yml` runs
`devenv test`, which runs the same `tasks."test:*"` set defined in
`devenv.nix`. A green `dev-ci` locally means a green PR check, because both
execute the same nix-defined suite. If you want to run one piece while
iterating, `enterShell` lists the individual scripts (`dev-fmt`, `dev-lint`,
`dev-check`, `dev-test`).

**Add a check by adding a task in `devenv.nix`, never by adding a step to the
workflow.** A step that exists only in CI is a step you cannot run locally, and
that is exactly the divergence this setup exists to prevent.

## Evals

The evals are not part of `dev-ci`: every run makes real model calls on your
Claude account. The Rust harness (`devenv tasks run eval:run`) runs `claude`
with `--permission-mode bypassPermissions` and no sandbox, so run it only on a
tree you trust -- not on a branch whose `evals/` you have not read. See
[eval-harness/README.md](eval-harness/README.md).

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org/). release-plz reads
them to compute the next version and write `CHANGELOG.md`. Its rules
([next_version](https://github.com/release-plz/release-plz/blob/f126c0e32b6c431e1e57eb000204537364e3fac1/crates/next_version/src/lib.rs)),
while the version is `0.x`:

- `feat!:` / `BREAKING CHANGE:` in the body → minor (major from 1.0)
- `feat:` → patch (minor from 1.0)
- `fix:` and any other commit → patch

`chore: release` commits are skipped (`release-plz.toml`).

## Releases

There is one version: the workspace version in `Cargo.toml`.
`.claude-plugin/plugin.json` must carry the same version, and a check fails
when the two differ, so the release PR changes both. release-plz opens that
`chore: release` PR: it bumps the version and writes `CHANGELOG.md`. Merging
it tags `vX.Y.Z` from the `rust-code` package and creates a GitHub release.
Nothing is published to crates.io: the plugin ships as this repository. Do
not change the version outside a release PR.

## Code of Conduct

This project follows the [Code of Conduct](CODE_OF_CONDUCT.md).
