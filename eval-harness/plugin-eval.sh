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
# kept per commit; the vendoring and a prebuilt target are kept per content
# key, so a commit that changes neither reuses both.
#
#   sh eval-harness/plugin-eval.sh [--keep-state] [--changed-since REV]
#       [--reuse-baseline BASELINE.json OUT.json] [claude plugin eval options...]
#   sh eval-harness/plugin-eval.sh --graders
#   sh eval-harness/plugin-eval.sh --probe NAME CMDFILE [options...]
#   sh eval-harness/plugin-eval.sh --prune
#
# --prune removes what older commits left under $TMPDIR/rust-code-plugin-eval
# (exports, prebuilt targets of about 1.1 GB each, vendorings and their GC
# roots) that HEAD's export does not use; run it when no suite is running.
#
# The suite-run options are described where they are read, below;
# eval-harness/README.md says how to run each tier.
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
# Output paths the caller gives relative to where it stands: made absolute,
# since the runner runs in the export and the keys go beside the result.
n=$#
prev=""
for a in "$@"; do
  case $prev in
  --json | --output-dir | --report)
    case $a in
    /* | -*) ;;
    *) a=$PWD/$a ;;
    esac
    ;;
  esac
  set -- "$@" "$a"
  prev=$a
done
shift "$n"
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
# PE_JOB_CAP overrides it (to measure the cap against none).
EVAL_JOB_CAP=${PE_JOB_CAP:-$(pe_job_cap "$(pe_ncpu)" "$(pe_jobs "$@")")}
export EVAL_JOB_CAP

mkdir -p "${TMPDIR:-/tmp}/rust-code-plugin-eval"
# The physical path: cargo and the sandbox see /private/tmp, not /tmp.
base=$(cd "${TMPDIR:-/tmp}/rust-code-plugin-eval" && pwd -P)
out=$base/$sha
export_dir=$out/root
# The cases' reference solutions, beside the export rather than in it: the
# export is the plugin root, which the arm with the plugin can read by path
# (measured), so it holds no solution/, checks.toml or .solve.sh. --graders
# and the vendoring take them from here; the scaffold that applies them
# runs outside the sandbox.
solutions=$out/solutions
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

# --prune: removes, from $base only, the entries this script makes -- <sha>
# exports, target-<key>, vendor-<key>, vendor-<key>.store and
# vendor-<key>.gcroot -- that HEAD does not use: its own export, its
# vendoring and the target its export names (all targets are kept when HEAD
# has no export, since only the export knows its target's key). A link to a
# `.NAME.XXXXXX` directory beside it (share_dir) goes with that directory; any
# other link, a GC root into the nix store among them, is removed as a link
# and never followed, so the store is never touched (`nix-store --gc` frees
# the path later). A read-only directory inside a removed entry -- a copy out
# of the store, or one sealed -- is opened (u+rwx) as find reaches it; find
# does not follow links, so only directories under $base are. Prints each
# removal and its size.
if [ "${1:-}" = --prune ]; then
  keep_target=""
  if [ -f "$export_dir/evals/_scaffold/target-dir" ]; then
    keep_target=$(basename "$(dirname "$(cat "$export_dir/evals/_scaffold/target-dir")")")
  else
    echo "plugin-eval: HEAD ($sha) has no export, so the target it uses is unknown: every target-* is kept"
  fi
  kb() { du -sk "$1" 2>/dev/null | cut -f1; }
  size() { if [ "$1" -ge 1024 ]; then echo "$(($1 / 1024)) MB"; else echo "$1 KB"; fi; }
  # remove_tree DIR: DIR (a real directory under $base) and all in it.
  remove_tree() {
    find "$1" -type d ! -perm -u+rwx -exec chmod u+rwx {} \;
    rm -rf "$1"
  }
  total=0
  for e in "$base"/*; do
    [ -e "$e" ] || [ -L "$e" ] || continue
    name=${e##*/}
    printf '%s\n' "$name" | grep -Eq '^([0-9a-f]{7,40}|target-[0-9a-f]{16}|vendor-[0-9a-f]{16}(\.store|\.gcroot)?)$' || continue
    case $name in
    "$sha" | "vendor-$vkey" | "vendor-$vkey.store" | "vendor-$vkey.gcroot") continue ;;
    target-*) [ -n "$keep_target" ] && [ "$name" != "$keep_target" ] || continue ;;
    esac
    if [ -L "$e" ]; then
      to=$(readlink "$e")
      case $to in
      ."$name".*)
        if [ -d "$base/$to" ] && [ ! -L "$base/$to" ]; then
          k=$(kb "$base/$to")
          remove_tree "$base/$to"
          rm -f "$e"
          echo "plugin-eval: removed $e and $base/$to ($(size "$k"))"
          total=$((total + k))
          continue
        fi
        ;;
      esac
      rm -f "$e"
      echo "plugin-eval: removed the link $e (to $to, left as it is)"
    elif [ -d "$e" ]; then
      k=$(kb "$e")
      remove_tree "$e"
      echo "plugin-eval: removed $e ($(size "$k"))"
      total=$((total + k))
    else
      rm -f "$e"
      echo "plugin-eval: removed $e"
    fi
  done
  echo "plugin-eval: pruned $(size "$total") from $base; store paths whose GC root went are freed by nix-store --gc"
  exit 0
fi

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
      [ "$slot" = solution ] && [ -d "$solutions/$n" ] && cp -R "$solutions/$n/." "$d/"
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
# shellcheck disable=SC2329 # called by prebuild_into, through share_dir
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
# dependency is fresh in a working copy that clones it (measured in the
# sandbox: build 61 fresh of 62, clippy 96 of 96). A failing command is
# logged, not fatal: what it built stays.
# shellcheck disable=SC2329 # called through share_dir
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
# An export made before the solutions moved out of it is made again.
if [ -f "$out/.ready" ] && [ -n "$(ls -d "$export_dir"/evals/*/solution "$export_dir"/evals/*/checks.toml 2>/dev/null)" ]; then
  rm -rf "$out"
fi
if [ ! -f "$out/.ready" ]; then
  rm -rf "$export_dir"
  mkdir -p "$export_dir/.cargo"
  git archive HEAD | tar -x -C "$export_dir"
  # The export is the plugin root, which the arm with the plugin can read
  # by path (measured): the harness's recorded results, which name graders,
  # stay out of it.
  rm -rf "$export_dir/eval-harness/fixtures"
  # Nor do the answers: each case's solution/ goes beside the export, and
  # its checks.toml (eval-harness's own graders) and any .solve.sh go.
  rm -rf "$solutions"
  mkdir -p "$solutions"
  for c in "$export_dir"/evals/*/; do
    n=$(basename "$c")
    [ -d "$c/solution" ] && mv "$c/solution" "$solutions/$n"
    rm -f "$c/checks.toml"
  done
  find "$export_dir" -name .solve.sh -type f -exec rm -f {} +
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
  env -i HOME="$HOME" USER="${USER:-$(id -un)}" PATH=/usr/bin:/bin "$(command -v devenv)" shell -- \
    sh -c 'unset HOME USER SHELL PWD OLDPWD SHLVL TMPDIR TMP TEMP TERM LOGNAME DEVENV_RUNTIME _; export -p' \
    2>/dev/null | sed -n '/^export /,$p' | sed "s|$repo|__ROOT__|g" >"$export_dir/evals/_scaffold/devenv.env"
  grep -q '^export RUSTFLAGS=' "$export_dir/evals/_scaffold/devenv.env" ||
    {
      echo "plugin-eval: could not capture the devenv environment" >&2
      exit 1
    }
  # Eval runs only: the per-session job cap the invocation hands every
  # session as EVAL_JOB_CAP (lib.sh, pe_job_cap). The dev profile stays the
  # project's own: a lighter one (line-tables-only, no incremental cache) was
  # not shown faster in the sandbox, where its runs overlapped.
  cat >>"$export_dir/evals/_scaffold/devenv.env" <<'EOF'
if [ -n "${EVAL_JOB_CAP:-}" ]; then
  export CARGO_BUILD_JOBS="$EVAL_JOB_CAP" NEXTEST_TEST_THREADS="$EVAL_JOB_CAP" RUST_TEST_THREADS="$EVAL_JOB_CAP"
fi
EOF
  # The base workspace's target/, keyed on what it is built from: the
  # working copy's files and the environment above.
  kw=$(mktemp -d "$base/.key-work.XXXXXX")
  base_workspace "$kw"
  content_key "$kw/home/cwd" >"$out/base-key"
  rm -rf "$kw"
  tkey=$( (cat "$out/base-key" && pe_sha256 <"$export_dir/evals/_scaffold/devenv.env") | pe_sha256 | cut -c1-16)
  share_dir "target-$tkey" prebuild_into
  echo "$base/target-$tkey/target" >"$export_dir/evals/_scaffold/target-dir"
  # Tags in the export's copy of each case (lib.sh, pe_tags), so --tag can
  # pick a tier or any set of cases. A case's own inline tags are kept, ours
  # added (pe_tags_merge); a block list is left as it is.
  for c in "$export_dir"/evals/*/; do
    n=$(basename "$c")
    p=$c/prompt.md
    [ -f "$p" ] && [ "$(sed -n '1p' "$p")" = --- ] || continue
    own=$(sed -n '2,/^---$/p' "$p" | grep '^tags:' || true)
    if [ -z "$own" ]; then
      { sed -n '1p' "$p" && pe_tags "$n" && sed '1d' "$p"; } >"$p.new"
    elif printf '%s\n' "$own" | grep -q '\]'; then
      new=$(pe_tags_merge "$n" "$own")
      awk -v new="$new" 'NR > 1 && !done && /^tags:/ { print new; done = 1; next } { print }' "$p" >"$p.new"
    else
      echo "plugin-eval: $n lists its tags as a block; --tag case-$n will not pick it" >&2
      continue
    fi
    mv "$p.new" "$p"
  done
  touch "$out/.ready"
fi
# The runner writes each result into <eval dir>/results/ under the plugin
# root unless told otherwise -- even with --json -- and a result holds the
# graders' evidence (under graders-solution/, the solution's own files),
# which the arm with the plugin could read there. Every mode now passes an
# --output-dir beside the root; what earlier runs left in it is moved there.
for r in "$export_dir"/*/results; do
  [ -d "$r" ] || continue
  mkdir -p "$out/moved-results"
  mv "$r" "$out/moved-results/$(basename "$(dirname "$r")")-$$"
done

# run_suite ARGS...: the runner on the export with ARGS, ending what timed-out
# sessions leave running (lib.sh, pe_reap) every 15 s while it runs and once
# after. Returns the runner's exit code.
runroot=${PE_RUN_ROOT:-/private/tmp}
run_suite() {
  marker=$(mktemp "$base/.started.XXXXXX")
  # On stderr: a background job holding stdout would keep a caller's pipe
  # open until its last sleep ends.
  (
    s=""
    trap 'kill "$s" 2>/dev/null; exit 0' TERM
    while kill -0 "$$" 2>/dev/null; do
      sleep 15 &
      s=$!
      wait "$s"
      pe_reap "$runroot" "$marker" "$$"
    done
  ) >&2 &
  reaper=$!
  set +e
  (cd "$export_dir" && claude plugin eval . "$@")
  code=$?
  set -e
  kill "$reaper" 2>/dev/null || true
  pe_reap "$runroot" "$marker" "$$"
  rm -f "$marker" "$marker.seen"
  return "$code"
}

# write_keys FILE ARGS...: FILE holds `<case> <key>` per case: a key over
# everything a run of that case without the plugin depends on -- the base
# working copy and the vendored dependencies (base-key), the captured
# environment, the scripts that build the run, the case's own files, the
# claude version, the job cap, and every runner argument that can change a
# run (lib.sh, pe_key_args): the model, the judge, --runs, the tools, the
# cost ceiling, the mocks, the case selection and the concurrency (a
# timed-out run is still graded, and contention decides timeouts: in the
# full run at commit 28c0e7a (2026-10-25), the only timed-out sans-io-retry
# run without the plugin scored 0.91, its two others 1.0). merge.rs reuses a without arm only under an equal key.
write_keys() {
  f=$1
  shift
  for k in "$out/base-key" "$export_dir/evals/_scaffold/devenv.env"; do
    [ -f "$k" ] || {
      echo "plugin-eval: $k is missing; delete $out and run again" >&2
      exit 1
    }
  done
  common=$(
    {
      cat "$out/base-key"
      pe_sha256 <"$export_dir/evals/_scaffold/devenv.env"
      git -C "$repo" rev-parse HEAD:evals/_scaffold HEAD:eval-harness/plugin-eval.sh HEAD:eval-harness/lib.sh
      echo "claude $(claude --version 2>/dev/null | head -1) cap $EVAL_JOB_CAP"
      pe_key_args "$@"
    } | pe_sha256
  )
  for c in "$export_dir"/evals/*/; do
    n=$(basename "$c")
    [ "$n" = _scaffold ] && continue
    printf '%s %s\n' "$n" "$(printf '%s %s' "$common" "$(git -C "$repo" rev-parse "HEAD:evals/$n")" | pe_sha256 | cut -c1-16)"
  done >"$f"
}

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
        overlay="cp -R '$solutions/$n/.' . && if [ -f .solve.sh ]; then sh .solve.sh && rm -f .solve.sh; fi"
      else
        overlay=":"
      fi
      # shellcheck disable=SC2016 # $here and $0 belong to the generated script
      printf '#!/bin/sh\nset -e\nhere=$(cd "$(dirname "$0")" && pwd)\nsh "$here/../../evals/%s/scaffold.sh"\n%s\n' "$n" "$overlay" >"$d/scaffold.sh"
    done
  done
  for state in solution start; do
    echo "== graders on the $state state"
    # shellcheck disable=SC2046 # the pins are single words
    run_suite --eval-dir "graders-$state" --scaffold --trust-plugin \
      --ablation none --model haiku --no-publish --threshold 0 \
      --json "$out/graders-$state.json" --report "$out/graders-$state.html" \
      --output-dir "$out/graders-$state-results" "$@" \
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
  # shellcheck disable=SC2046 # the pins are single words
  run_suite --eval-dir "probe-$name" --scaffold --trust-plugin --allow-tools Bash \
    --ablation "${PE_PROBE_ABLATION:-none}" --model haiku --no-publish --threshold 0 --json "$json" \
    --output-dir "$out/probe-$name-results" "$@" \
    $(pe_pin_args --model haiku "$@") || true
  # From the repository: the export's cargo config points at the vendored
  # dependencies, which hold the cases' crates, not the harness's.
  (cd "$repo" && cargo run --quiet --manifest-path eval-harness/Cargo.toml -- --evidence "$json" out)
  exit 0
fi

# A suite run. Its own options, taken out before the runner's:
#   --keep-state            keep each run's trace (state.rs); with PE_UNSEAL=1
#                           (off by default) also its changes, then remove the
#                           run directories the result names
#   --changed-since REV     only the cases REV..HEAD changed (lib.sh,
#                           pe_changed_cases); all of them after any other change
#   --reuse-baseline B.json OUT.json
#                           run the arm with the plugin only and merge it with
#                           B's arm without (merge.rs), where every case's key
#                           matches; write OUT.json
usage() {
  echo "usage: plugin-eval.sh [--keep-state] [--changed-since REV] [--reuse-baseline BASELINE.json OUT.json] [runner options]" >&2
  exit 2
}
keep_state=""
baseline=""
merged=""
while :; do
  case ${1:-} in
  --keep-state)
    keep_state=yes
    shift
    ;;
  --changed-since)
    [ $# -ge 2 ] || usage
    picked=$(pe_changed_cases "$repo" "$2")
    shift 2
    if [ -z "$picked" ]; then
      echo "plugin-eval: no case changed; nothing to run"
      exit 0
    fi
    echo "plugin-eval: cases: $(echo "$picked" | tr '\n' ' ')"
    if [ "$picked" != all ]; then
      for n in $picked; do
        [ -d "$export_dir/evals/$n" ] && set -- "$@" --tag "case-$n"
      done
    fi
    ;;
  --reuse-baseline)
    [ $# -ge 3 ] || usage
    baseline=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
    merged=$(cd "$(dirname "$3")" && pwd)/$(basename "$3")
    shift 3
    [ -f "${baseline%.json}.keys" ] || {
      echo "plugin-eval: ${baseline%.json}.keys is missing: only a run of this script can be a baseline" >&2
      exit 2
    }
    ;;
  *) break ;;
  esac
done
if [ -n "$baseline" ] && { pe_has_opt --json "$@" || pe_has_opt --ablation "$@"; }; then
  echo "plugin-eval: --reuse-baseline sets --json and --ablation itself" >&2
  exit 2
fi

# Both models pinned unless the caller pins them (lib.sh says why).
# shellcheck disable=SC2046 # the pins are single words
set -- "$@" $(pe_pin_args "$@")

# Where the result goes, so its keys (and any kept state) can sit beside it.
stamp=$(date -u +%Y-%m-%dT%H-%M-%SZ)
if [ -n "$baseline" ]; then
  result=$out/results/$stamp-with.json
  mkdir -p "$out/results"
  set -- "$@" --ablation none --json "$result"
elif pe_has_opt --json "$@"; then
  result=$(pe_opt_value --json "" "$@")
elif pe_has_opt --output-dir "$@"; then
  result=$(pe_opt_value --output-dir "" "$@")/aggregate-result.json
else
  set -- "$@" --output-dir "$out/results/$stamp"
  result=$out/results/$stamp/aggregate-result.json
fi
pe_has_opt --output-dir "$@" || set -- "$@" --output-dir "$out/results/$stamp"
[ -n "$keep_state" ] && set -- "$@" --keep-temp
mkdir -p "$(dirname "$result")"
write_keys "${result%.json}.keys" "$@"

# An interrupted runner still writes its partial result (docs: exit 130,
# 143); wait for it, so kept state is collected either way.
trap ':' INT TERM
code=0
run_suite --scaffold --trust-plugin --allow-tools Bash Edit Write "$@" || code=$?
trap - INT TERM
# What is left in the plugin root under evals/ -- the prompts and graders the
# runner needs -- the arm with the plugin can still read by path. Say so
# when a run's visible trace names it.
if [ -f "$result" ] && grep -qF -e "$export_dir/evals/" -e 'CLAUDE_PLUGIN_ROOT}/evals' -e 'CLAUDE_PLUGIN_ROOT/evals' "$result"; then
  echo "plugin-eval: warning: $result names a path under the plugin root's evals/ (prompts, graders): a session may have read them" >&2
fi
if [ -n "$keep_state" ] && [ -f "$result" ]; then
  (cd "$repo" && PE_RUN_ROOT=$runroot cargo run --quiet --manifest-path eval-harness/Cargo.toml -- \
    --harvest "$result" "$out/state/$stamp" "$export_dir") || code=1
fi
if [ -n "$baseline" ]; then
  (cd "$repo" && cargo run --quiet --manifest-path eval-harness/Cargo.toml -- \
    --merge "$result" "$baseline" "$merged") || code=1
  cp "${result%.json}.keys" "${merged%.json}.keys"
fi
echo "plugin-eval: result $result (keys ${result%.json}.keys)"
exit "$code"
