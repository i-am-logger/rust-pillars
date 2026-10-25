#!/bin/sh
# `claude plugin eval` on a clean export of the committed tree.
#
# Never run the runner at the repository root:
# - it treats every directory named `evals` at any depth as an eval root, and
#   `cargo xtask controls` leaves a copy of the cases under target/controls/;
# - it refuses a case whose files have a second name (a hard link), which
#   build output under the root can give them.
#
# The runner's sandbox has no network and no writable /tmp, so cargo could
# neither fetch a dependency nor run devenv. This script exports HEAD with
# `git archive`, vendors every dependency any case can need -- the starting
# state and the reference solution of each -- and sets cargo offline, so the
# agent can build, lint and test exactly as it would at home. The export is
# kept per commit, so the vendoring is paid once.
#
#   sh eval-harness/plugin-eval.sh [claude plugin eval options...]
#   sh eval-harness/plugin-eval.sh --graders
#
# --graders checks the graders, not the plugin: in a suite generated beside
# the export, a haiku agent told to do nothing works once on the reference
# solution (every file grader must pass) and once on the untouched start
# (every grader that reads a file the task changes must fail). Cheap: two
# short turns per case. Read its report per grader; a trace grader fails on
# both, since nothing was done.
set -eu

repo=$(cd "$(dirname "$0")/.." && pwd)
# shellcheck disable=SC1091 # lib.sh is checked as a file of its own
. "$repo/eval-harness/lib.sh"
cd "$repo"
sha=$(git rev-parse --short HEAD)
if [ -n "$(git status --porcelain -- evals skills commands agents hooks .claude-plugin crates xtask)" ]; then
  echo "plugin-eval: uncommitted changes; the run uses HEAD ($sha), not the working tree" >&2
fi
mkdir -p "${TMPDIR:-/tmp}/rust-code-plugin-eval"
# The physical path: cargo and the sandbox see /private/tmp, not /tmp.
base=$(cd "${TMPDIR:-/tmp}/rust-code-plugin-eval" && pwd -P)
out=$base/$sha
export_dir=$out/root
# The vendored dependencies, keyed on the Cargo files rather than the commit
# (lib.sh, pe_vendor_key), and kept outside every export: the scaffold copies
# the export into each working copy, and 77 MB of vendor/ went with it.
vendor=$base/vendor-$(pe_vendor_key "$repo" HEAD)

# share_dir NAME BUILD: makes $base/NAME by running BUILD with a fresh
# directory as $1, and publishes it with one `ln -s`, which either wins or
# finds another export's copy already there -- so exports of several commits
# can build at once and none sees a half-made one.
share_dir() {
  [ -e "$base/$1" ] && return 0
  tmp=$(mktemp -d "$base/.$1.XXXXXX")
  "$2" "$tmp"
  ln -s "$(basename "$tmp")" "$base/$1" 2>/dev/null || rm -rf "$tmp"
}

# vendor_into DIR: every dependency any case can need -- the starting state
# and the reference solution of each -- vendored into DIR.
vendor_into() {
  work=$(mktemp -d "$base/.vendor-work.XXXXXX")
  syncs=""
  for c in "$export_dir"/evals/*/; do
    n=$(basename "$c")
    [ "$n" = _scaffold ] && continue
    for slot in start solution; do
      d=$work/$n-$slot
      mkdir -p "$d" "$work/home"
      (cd "$d" && HOME=$work/home sh "$c/scaffold.sh" >/dev/null)
      [ "$slot" = solution ] && cp -R "$c/solution/." "$d/"
      syncs="$syncs -s $d/Cargo.toml"
    done
  done
  # shellcheck disable=SC2086 # one word per -s argument
  (cd "$export_dir" && cargo vendor --quiet --versioned-dirs "$1" $syncs >/dev/null)
  rm -rf "$work"
}

# A ready export whose vendoring was removed is built again.
[ -f "$out/.ready" ] && [ ! -d "$vendor" ] && rm -rf "$out"
if [ ! -f "$out/.ready" ]; then
  rm -rf "$export_dir"
  mkdir -p "$export_dir/.cargo"
  git archive HEAD | tar -x -C "$export_dir"
  share_dir "$(basename "$vendor")" vendor_into
  cat >>"$export_dir/.cargo/config.toml" <<EOF

# Eval runs only: the sandbox has no network, so every dependency the cases
# can need is vendored, outside the working copy, and cargo never asks the
# registry.
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "$vendor"

[net]
offline = true
EOF
  # The agent's shell starts without the project's toolchain on PATH and
  # cannot enter devenv (nix must read /private/tmp), so it hunted /nix/store for any
  # rustc -- and a newer one words a compile_fail note differently. A case
  # may not set PATH, and the shell rebuilds it anyway, so capture what
  # devenv exports for this project -- PATH, RUSTFLAGS, the SDK, CC -- from an
  # empty environment (no operator variable leaks in), with the repository's
  # path left as __ROOT__; the scaffold loads it into the run's ~/.zshenv.
  : "${DEVENV_PROFILE:?run inside devenv shell}"
  env -i HOME="$HOME" USER="$USER" PATH=/usr/bin:/bin "$(command -v devenv)" shell -- \
    sh -c 'unset HOME USER SHELL PWD OLDPWD SHLVL TMPDIR TMP TEMP TERM LOGNAME DEVENV_RUNTIME _; export -p' \
    2>/dev/null | sed -n '/^export /,$p' | sed "s|$repo|__ROOT__|g" >"$export_dir/evals/_scaffold/devenv.env"
  grep -q '^export RUSTFLAGS=' "$export_dir/evals/_scaffold/devenv.env" ||
    {
      echo "plugin-eval: could not capture the devenv environment" >&2
      exit 1
    }
  touch "$out/.ready"
fi

if [ "${1:-}" = --graders ]; then
  shift
  for state in solution start; do
    suite=$export_dir/graders-$state
    [ -d "$suite" ] && continue
    for c in "$export_dir"/evals/*/; do
      n=$(basename "$c")
      [ "$n" = _scaffold ] && continue
      d=$suite/$n
      mkdir -p "$d/graders"
      for g in "$c"/graders/*.md; do
        # Skill-loading graders measure the plugin, not the work.
        grep -q '^type: tool_used' "$g" || cp "$g" "$d/graders/"
      done
      printf -- '---\nname: %s\nruns: 1\nmax_turns: 2\ntimeout_seconds: 300\n---\n\nReply with the single word done. Do not use any tool.\n' "$n" >"$d/prompt.md"
      printf 'schema_version: "1.1"\nname: %s\ncontext:\n  scaffold_script: scaffold.sh\n' "$n" >"$d/case.yaml"
      if [ "$state" = solution ]; then
        overlay="cp -R \"\$here/../../evals/$n/solution/.\" . && if [ -f .solve.sh ]; then sh .solve.sh && rm -f .solve.sh; fi"
      else
        overlay=":"
      fi
      # shellcheck disable=SC2016 # $here and $0 belong to the generated script
      printf '#!/bin/sh\nset -e\nhere=$(cd "$(dirname "$0")" && pwd)\nsh "$here/../../evals/%s/scaffold.sh"\n%s\n' "$n" "$overlay" >"$d/scaffold.sh"
    done
  done
  cd "$export_dir"
  for state in solution start; do
    echo "== graders on the $state state"
    # shellcheck disable=SC2046 # the pins are single words
    claude plugin eval . --eval-dir "graders-$state" --scaffold --trust-plugin \
      --ablation none --model haiku --no-publish --threshold 0 \
      --json "$out/graders-$state.json" --report "$out/graders-$state.html" "$@" \
      $(pe_pin_args --model haiku "$@") || true
  done
  echo "reports: $out/graders-solution.html $out/graders-start.html"
  exit 0
fi

# Both models pinned unless the caller pins them (lib.sh says why).
# shellcheck disable=SC2046 # the pins are single words
set -- "$@" $(pe_pin_args "$@")
cd "$export_dir"
exec claude plugin eval . --scaffold --trust-plugin --allow-tools Bash Edit Write "$@"
