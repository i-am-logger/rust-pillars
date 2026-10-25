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

After an edit to a `.rs` file, `hooks/gate-on-edit.sh` acts only if
`CLAUDE_PROJECT_DIR` is set and the file, once links and `..` are resolved,
is inside that directory. It then walks upward from the file to the nearest
directory with a `layers.toml`, stopping at the project's root, and if that
directory also has an `xtask/` directory, runs `cargo xtask budgets` and
`cargo xtask arch` there. That executes the project's own code -- its xtask
binary, build scripts and proc macros -- under the project's cargo
configuration (which can redefine the `xtask` alias or set a compiler
wrapper) and toolchain file, and cargo may fetch crates or a toolchain. Hooks
run with no permission prompt, and an install is user-scoped by default, so
the option applies to every project you open (inferred). Turn it on only if
you trust the repositories you open. `eval-harness/test-gate-on-edit.sh`
checks the inside-the-project rule, and `test:plugin` runs it.

**When you run a command**

Each command's `allowed-tools` frontmatter pre-approves those tools for as
long as the command runs:

| Command | Pre-approved tools | Runs |
|---|---|---|
| `/rust-code:gate` | `Bash(devenv test:*)`, `Bash(cargo xtask:*)`, `Bash(cargo fmt --check:*)`, `Bash(cargo clippy:*)`, `Bash(cargo test:*)`, Read, Grep, Glob | `devenv test` or `cargo xtask`, or `cargo fmt`, `cargo clippy` and `cargo test` in a project without the gates |
| `/rust-code:adopt` | `Bash(cargo metadata:*)`, `Bash(cargo xtask all:*)`, Read, Grep, Glob | `cargo metadata` and `cargo xtask all` without asking; `cargo xtask init`, copying gate files into the workspace and every edit prompt |

No command pre-approves a bare Bash, Edit or Write, so any other command and
every file change asks under your permission settings (`test:plugin` fails
if a bare grant comes back). The scoped grants are still wide:
`cargo xtask:*` covers `cargo xtask init`, which writes files, and
`cargo clippy:*` covers `--fix`. Both commands run the project's own code, as
the hook does. `adopt` copies files licensed CC BY-NC-SA 4.0 from
rust-template, and says so before it copies anything.

The `refuter` agent's `tools` list (Read, Grep, Glob, Bash) limits what it may
use; it grants nothing past your permission settings (inferred).

**The eval harness (maintainers only)**

Nothing here runs when you use the plugin. `eval-harness/` runs `claude -p`
with `--permission-mode bypassPermissions` in a scratch copy on the host, with
no sandbox, and runs each case's `scaffold.sh`: the session has your shell,
files, network and Claude login. A model run refuses to start unless
`RUST_CODE_EVAL_UNSANDBOXED=1` is set; set it only on a tree you trust,
ideally in a VM or container. `eval-harness/plugin-eval.sh` uses `claude plugin eval`,
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
