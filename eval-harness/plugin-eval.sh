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
cd "$repo"
sha=$(git rev-parse --short HEAD)
if [ -n "$(git status --porcelain -- evals skills commands agents hooks .claude-plugin crates xtask)" ]; then
  echo "plugin-eval: uncommitted changes; the run uses HEAD ($sha), not the working tree" >&2
fi
out=${TMPDIR:-/tmp}/rust-code-plugin-eval/$sha
export_dir=$out/root

if [ ! -f "$out/.ready" ]; then
  mkdir -p "$export_dir" "$out/vendor-work"
  git archive HEAD | tar -x -C "$export_dir"
  syncs=""
  for c in "$export_dir"/evals/*/; do
    n=$(basename "$c")
    [ "$n" = _scaffold ] && continue
    for slot in start solution; do
      d=$out/vendor-work/$n-$slot
      mkdir -p "$d"
      (cd "$d" && sh "$c/scaffold.sh" >/dev/null)
      [ "$slot" = solution ] && cp -R "$c/solution/." "$d/"
      syncs="$syncs -s $d/Cargo.toml"
    done
  done
  # shellcheck disable=SC2086 # one word per -s argument
  (cd "$export_dir" && cargo vendor --quiet --versioned-dirs vendor $syncs >/dev/null)
  cat >>"$export_dir/.cargo/config.toml" <<'EOF'

# Eval runs only: the sandbox has no network, so every dependency the cases
# can need is vendored here and cargo never asks the registry.
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"

[net]
offline = true
EOF
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
      printf '#!/bin/sh\nset -e\nhere=$(cd "$(dirname "$0")" && pwd)\nsh "$here/../../evals/%s/scaffold.sh"\n%s\n' "$n" "$overlay" >"$d/scaffold.sh"
    done
  done
  cd "$export_dir"
  for state in solution start; do
    echo "== graders on the $state state"
    claude plugin eval . --eval-dir "graders-$state" --scaffold --trust-plugin \
      --ablation none --model haiku --no-publish --threshold 0 \
      --json "$out/graders-$state.json" --report "$out/graders-$state.html" "$@" || true
  done
  echo "reports: $out/graders-solution.html $out/graders-start.html"
  exit 0
fi

cd "$export_dir"
exec claude plugin eval . --scaffold --trust-plugin --allow-tools Bash Edit Write "$@"
