# Security Policy

rust-code is a Claude Code plugin: skills, two slash commands, an agent and
two hooks, plus an eval harness for maintainers. What it can run on your
machine is the security surface worth reporting on.

## What runs, and when

**Without opting in**

- `hooks/rust-nudge.sh` runs on every prompt you submit. It reads the prompt
  and checks whether the project root has a `Cargo.toml`, and may print one
  line asking Claude to load the rust-code skill. It runs no other command and
  writes nothing. The `nudge` option turns it off.
- `hooks/gate-on-edit.sh` runs after every Edit or Write and exits at once
  unless you turned on `gate_on_edit`.
- The skills are text. A skill can suggest commands to Claude, and they are
  run, or not, under your own permission settings.

**With `gate_on_edit` turned on**

After an edit to a `.rs` file, `hooks/gate-on-edit.sh` walks upward from that
file to the nearest directory with a `layers.toml`, and if it also has an
`xtask/` directory, runs `cargo xtask budgets` and `cargo xtask arch` there.
That executes the project's own code -- its xtask binary, build scripts and
proc macros -- under the project's cargo configuration (which can redefine
the `xtask` alias or set a compiler wrapper) and toolchain file, and cargo
may fetch crates or a toolchain. Hooks run with no permission prompt, the walk
is not bounded by the project directory, and an install is user-scoped by
default, so the option applies to every project you open (inferred). Turn it
on only if you trust the repositories you open.

**When you run a command**

Each command's `allowed-tools` frontmatter pre-approves those tools for as
long as the command runs:

| Command | Pre-approved tools | Runs |
|---|---|---|
| `/rust-code:gate` | Bash, Read, Grep, Glob | `devenv test` or `cargo xtask`, or `cargo fmt`, `cargo clippy` and `cargo test` in a project without the gates |
| `/rust-code:adopt` | Bash, Read, Grep, Glob, Edit, Write | `cargo metadata`, `cargo xtask init` and `cargo xtask all`; it copies gate files into the workspace |

Bash is not scoped to those commands, so text in the project that the model
reads can steer what it runs while one of these is active. `adopt` asks before
each step that changes files, but that is an instruction to the model, not a
permission check. Both run the project's own code, as the hook does.

The `refuter` agent's `tools` list (Read, Grep, Glob, Bash) limits what it may
use; it grants nothing past your permission settings (inferred).

**The eval harness (maintainers only)**

Nothing here runs when you use the plugin. `eval-harness/` runs `claude -p`
with `--permission-mode bypassPermissions` in a scratch copy on the host, with
no sandbox, and runs each case's `scaffold.sh`: the session has your shell,
files, network and Claude login. Run it only on a tree you trust, ideally in a
VM or container. `eval-harness/plugin-eval.sh` uses `claude plugin eval`,
which runs each session in its sandbox with no network. Every eval run makes
real model calls on your account.

## Reporting a Vulnerability

Please report security issues privately rather than opening a public issue:
use [GitHub Security Advisories](https://github.com/i-am-logger/rust-code/security/advisories/new).
If that page is not available, open an issue that asks for a private channel
and leave the details out of it.

Include the plugin version (`.claude-plugin/plugin.json`) or the commit, the
output of `claude --version`, a minimal reproduction, and the impact as you
see it. There is no bug bounty -- this is a solo-maintained, source-available
project -- but reports are taken seriously and credited unless you ask
otherwise.

## Supported Versions

The latest tag and `master`. A fix lands on `master` and in the next tag;
there is no LTS branch and no backport.
