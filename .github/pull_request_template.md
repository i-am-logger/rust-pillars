## What does this change and why?

<!-- The "why" matters more than the "what" — the diff already shows what changed. -->

## Checklist

- [ ] `dev-ci` passes locally (see [CONTRIBUTING.md](../CONTRIBUTING.md)) — it runs the same nix-defined suite as CI
- [ ] Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/), so release-plz computes the right version bump
- [ ] Any new check was added as a `tasks."test:*"` entry in `devenv.nix`, not as a workflow-only step
- [ ] No version change outside a `chore: release` PR — the workspace version in `Cargo.toml` and `.claude-plugin/plugin.json` move together there, with `CHANGELOG.md`
