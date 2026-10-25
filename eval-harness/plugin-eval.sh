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
# Each session's cargo and test threads, capped so the -j sessions of this
# invocation share the cores (lib.sh, pe_job_cap). Handed over as an EVAL_
# variable, which the runner passes to every session (docs; measured), and
# turned into CARGO_BUILD_JOBS and the rest by the export's devenv.env: an
# export is shared by every invocation on its commit, whatever their -j.
EVAL_JOB_CAP=$(pe_job_cap "$(pe_ncpu)" "$(pe_jobs "$@")")
export EVAL_JOB_CAP

mkdir -p "${TMPDIR:-/tmp}/rust-code-plugin-eval"
# The physical path: cargo and the sandbox see /private/tmp, not /tmp.
base=$(cd "${TMPDIR:-/tmp}/rust-code-plugin-eval" && pwd -P)
out=$base/$sha
export_dir=$out/root
# The vendored dependencies, keyed on the Cargo files rather than the commit
# (lib.sh, pe_vendor_key), and kept out of the export: the scaffold copies the
# export into each working copy, and 77 MB of vendor/ went with it. They live
# in the nix store, the one place found that every session reads in both arms
# at the same path (measured: the sandbox refuses the shared directory under
# /private/tmp, and the plugin root in the arm without the plugin), and the
# same path is what keeps prebuilt dependencies fresh: cargo's fingerprint
# holds the source path, and a clone per working copy left every one dirty
# ("the path to the source changed").
vkey=$(pe_vendor_key "$repo" HEAD)
vstore_file=$base/vendor-$vkey.store

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

# vendor_store: the store path of every dependency any case can need -- the
# starting state and the reference solution of each -- vendoring them and
# adding them to the store (with a GC root beside the exports) if no export
# did yet. Content-addressed, so exports racing on one key get one path.
vendor_store() {
  if [ -f "$vstore_file" ] && [ -d "$(cat "$vstore_file")" ]; then
    cat "$vstore_file"
    return 0
  fi
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
  (cd "$export_dir" && cargo vendor --quiet --versioned-dirs "$work/vendored-crates-$vkey" $syncs >/dev/null)
  p=$(nix-store --add "$work/vendored-crates-$vkey")
  nix-store --add-root "$base/vendor-$vkey.gcroot" --indirect --realise "$p" >/dev/null
  echo "$p" >"$vstore_file.$$"
  mv "$vstore_file.$$" "$vstore_file"
  rm -rf "$work"
  echo "$p"
}

# in_agent_env DIR CMD...: CMD in DIR/home/cwd (a scaffolded working copy)
# with the environment an agent's shell gets there: nothing of the caller's,
# the captured devenv.env the scaffold wrote, then the system PATH.
in_agent_env() {
  _home=$1
  shift
  # shellcheck disable=SC2016 # expanded by the inner shell
  env -i HOME="$_home/home" PATH=/usr/bin:/bin TERM=dumb sh -c \
    '. "$HOME/.devenv.env"; export PATH="$PATH:/usr/bin:/bin"; cd "$HOME/cwd" && "$@"' sh "$@"
}

# base_workspace DIR: the base workspace (no case) scaffolded in DIR/home/cwd.
base_workspace() {
  mkdir -p "$1/home/cwd"
  (cd "$1/home/cwd" && HOME=$1/home sh "$export_dir/evals/_scaffold/scaffold.sh" "$export_dir/evals/_scaffold" >/dev/null)
}

# content_key DIR: a key over every file of DIR but target/: names and
# contents (cargo's config among them, which names the vendored store path).
content_key() {
  (cd "$1" && find . -type f ! -path './target/*' | LC_ALL=C sort | while IFS= read -r f; do
    printf '%s %s\n' "$f" "$(pe_sha256 <"$f")"
  done) | pe_sha256 | cut -c1-16
}

# prebuild_into DIR: DIR/target, the base workspace built with the commands
# the gates and the agents run, in the agent's environment -- the same
# toolchain, RUSTFLAGS and profile, so cargo's fingerprints match and every
# dependency is fresh in a working copy that clones it (measured: 58 of 62
# units fresh). A failing command is logged, not fatal: what it built stays.
prebuild_into() {
  w=$(mktemp -d "$base/.prebuild-work.XXXXXX")
  base_workspace "$w"
  crates=$(in_agent_env "$w" cargo xtask crates --role domain,core 2>/dev/null) || crates=""
  {
    in_agent_env "$w" cargo build --workspace
    in_agent_env "$w" cargo clippy --workspace --all-targets --all-features -- -D warnings
    in_agent_env "$w" cargo nextest run --workspace
    in_agent_env "$w" cargo nextest run --workspace --all-features --cargo-profile qa --no-run
    for t in x86_64-unknown-none thumbv7em-none-eabihf; do
      # shellcheck disable=SC2086 # one word per -p argument
      in_agent_env "$w" cargo build --no-default-features --target "$t" $crates
    done
    in_agent_env "$w" cargo hack check --workspace --each-feature --no-dev-deps
    in_agent_env "$w" cargo xtask all
  } >"$1/prebuild.log" 2>&1 || true
  mkdir -p "$w/home/cwd/target"
  mv "$w/home/cwd/target" "$1/target"
  rm -rf "$w"
}

# A ready export whose vendoring or prebuilt target was removed is built
# again.
if [ -f "$out/.ready" ] && { [ ! -f "$vstore_file" ] || [ ! -d "$(cat "$vstore_file")" ]; }; then
  rm -rf "$out"
fi
if [ -f "$out/.ready" ] && [ -f "$export_dir/evals/_scaffold/target-dir" ] &&
  [ ! -d "$(cat "$export_dir/evals/_scaffold/target-dir")" ]; then
  rm -rf "$out"
fi
if [ ! -f "$out/.ready" ]; then
  rm -rf "$export_dir"
  mkdir -p "$export_dir/.cargo"
  git archive HEAD | tar -x -C "$export_dir"
  vendor=$(vendor_store)
  cat >>"$export_dir/.cargo/config.toml" <<EOF

# Eval runs only: the sandbox has no network, so every dependency the cases
# can need is vendored, and cargo never asks the registry.
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
  # Eval runs only, never the project: a lighter dev profile (measured
  # faster: 12.1 / 12.5 s against 14.9 / 15.5 s for build, nextest, clippy
  # and two edit loops on prebuilt dependencies), and the per-session job cap
  # the invocation hands every session as EVAL_JOB_CAP (lib.sh, pe_job_cap).
  cat >>"$export_dir/evals/_scaffold/devenv.env" <<'EOF'
export CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0
if [ -n "${EVAL_JOB_CAP:-}" ]; then
  export CARGO_BUILD_JOBS="$EVAL_JOB_CAP" NEXTEST_TEST_THREADS="$EVAL_JOB_CAP" RUST_TEST_THREADS="$EVAL_JOB_CAP"
fi
EOF
  # The base workspace's target/, keyed on what it is built from: the
  # working copy's files and the environment above.
  kw=$(mktemp -d "$base/.key-work.XXXXXX")
  base_workspace "$kw"
  tkey=$( (content_key "$kw/home/cwd" && pe_sha256 <"$export_dir/evals/_scaffold/devenv.env") | pe_sha256 | cut -c1-16)
  rm -rf "$kw"
  share_dir "target-$tkey" prebuild_into
  echo "$base/target-$tkey/target" >"$export_dir/evals/_scaffold/target-dir"
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

if [ "${1:-}" = --probe ]; then
  # --probe NAME CMDFILE [runner options]: one fixed command, the same in
  # every run, in a working copy of $PE_PROBE_CASE (default own-dont-leak),
  # run by a haiku session told to run it and nothing else. The command's
  # own timing and output land in probe.out, which a file grader keeps
  # whole in the result; `--keep-temp` keeps the trace too.
  [ $# -ge 3 ] || {
    echo "usage: plugin-eval.sh --probe NAME CMDFILE [runner options]" >&2
    exit 2
  }
  name=$2
  cmdfile=$(cd "$(dirname "$3")" && pwd)/$(basename "$3")
  shift 3
  suite=$export_dir/probe-$name
  d=$suite/$name
  rm -rf "$suite"
  mkdir -p "$d/graders"
  cp "$cmdfile" "$d/probe.cmd"
  printf -- '---\nname: %s\nruns: 1\nmax_turns: 4\ntimeout_seconds: %s\n---\n\nRun exactly this one Bash command, with a 900000 ms timeout, and then reply with the single word done:\n\nsh probe.sh >probe.out 2>&1\n' \
    "$name" "${PE_PROBE_TIMEOUT:-900}" >"$d/prompt.md"
  printf 'schema_version: "1.1"\nname: %s\ncontext:\n  scaffold_script: scaffold.sh\n' "$name" >"$d/case.yaml"
  printf -- '---\ntype: llm\nfocus:\n  source: file\n  path: probe.out\n---\n\nPASS if the file is not empty.\n' >"$d/graders/out.md"
  cat >"$d/probe.sh" <<'EOF'
now() { perl -MTime::HiRes=time -e 'printf "%.1f", time'; }
t0=$(now)
(. ./probe.cmd)
code=$?
echo "PROBE_EXIT=$code PROBE_SECONDS=$(perl -e "printf q(%.1f), $(now) - $t0")"
EOF
  # shellcheck disable=SC2016 # $here and $0 belong to the generated script
  printf '#!/bin/sh\nset -e\nhere=$(cd "$(dirname "$0")" && pwd)\nsh "$here/../../evals/%s/scaffold.sh"\ncp "$here/probe.cmd" "$here/probe.sh" .\n' \
    "${PE_PROBE_CASE:-own-dont-leak}" >"$d/scaffold.sh"
  json=$out/probe-$name.json
  pe_has_opt --json "$@" && json=$(pe_opt_value --json "$json" "$@")
  cd "$export_dir"
  # shellcheck disable=SC2046 # the pins are single words
  claude plugin eval . --eval-dir "probe-$name" --scaffold --trust-plugin --allow-tools Bash \
    --ablation "${PE_PROBE_ABLATION:-none}" --model haiku --no-publish --threshold 0 --json "$json" "$@" \
    $(pe_pin_args --model haiku "$@") || true
  # From the repository: the export's cargo config points at the vendored
  # dependencies, which hold the cases' crates, not the harness's.
  (cd "$repo" && cargo run --quiet --manifest-path eval-harness/Cargo.toml -- --evidence "$json" out)
  exit 0
fi

# Both models pinned unless the caller pins them (lib.sh says why).
# shellcheck disable=SC2046 # the pins are single words
set -- "$@" $(pe_pin_args "$@")
cd "$export_dir"
exec claude plugin eval . --scaffold --trust-plugin --allow-tools Bash Edit Write "$@"
