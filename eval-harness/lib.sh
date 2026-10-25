#!/bin/sh
# Functions plugin-eval.sh is built from, kept apart so that
# eval-harness/lib-test.sh can check each one without an export or a model
# run (`cargo test --manifest-path eval-harness/Cargo.toml` runs it).
# POSIX sh; sourced, never run.

# The model and judge every run pins, so a model rollout is never read as a
# plugin change (docs: "Pin it in CI"). claude-opus-5-5 is what run 4 used by
# default (every trace's model field); haiku is the judge default
# (`claude plugin eval --help`, claude 2.1.303), an alias the runner resolves.
PE_MODEL=claude-opus-5-5
PE_JUDGE=haiku

# pe_tags NAME: the tags line the export gives case NAME: case-NAME, so any
# set of cases can be picked with --tag (one --case keeps only one glob, and
# a brace glob matches nothing: measured). A tier is a case's own tag, in its
# prompt.md (`tags: [smoke]`); the export only adds case-NAME to it.
pe_tags() {
  printf 'tags: [case-%s]\n' "$1"
}

# pe_tags_merge NAME LINE: a case's own inline tags line (`tags: [a, b]`)
# with pe_tags' tags added where missing, so a case that tags itself keeps
# its tags and can still be picked as case-NAME.
pe_tags_merge() {
  _pe_value=$(printf '%s\n' "$2" | sed -n 's/^tags:[[:space:]]*\[\(.*\)\][[:space:]]*$/\1/p')
  for _pe_a in $(pe_tags "$1" | sed 's/^tags: \[\(.*\)\]$/\1/' | tr ',' ' '); do
    case ", $_pe_value, " in
    *", $_pe_a, "*) ;;
    ", , ") _pe_value=$_pe_a ;;
    *) _pe_value="$_pe_value, $_pe_a" ;;
    esac
  done
  printf 'tags: [%s]\n' "$_pe_value"
}

# pe_changed_cases REPO REV: the cases to run after the changes REV..HEAD,
# one per line, or `all`. A case is picked when its own files under
# evals/<case>/ changed; any other change picks every case, since every
# other path in the export can reach a run: the scaffold copies the
# workspace, the scripts build it, and the arm with the plugin can read any
# file of the plugin root (measured: README.md and eval-harness/lib.sh).
pe_changed_cases() {
  git -C "$1" diff --name-only "$2" HEAD >"${TMPDIR:-/tmp}/pe-changed.$$" || return 1
  if grep -qv '^evals/[^/_][^/]*/' "${TMPDIR:-/tmp}/pe-changed.$$"; then
    echo all
  else
    sed -n 's|^evals/\([^/]*\)/.*|\1|p' "${TMPDIR:-/tmp}/pe-changed.$$" | sort -u
  fi
  rm -f "${TMPDIR:-/tmp}/pe-changed.$$"
}

# pe_has_opt NAME ARGS...: whether ARGS pass option NAME (as `NAME v` or
# `NAME=v`).
pe_has_opt() {
  _pe_name=$1
  shift
  for _pe_a in "$@"; do
    case $_pe_a in
    "$_pe_name" | "$_pe_name"=*) return 0 ;;
    esac
  done
  return 1
}

# pe_opt_value NAME DEFAULT ARGS...: the value ARGS give option NAME, or
# DEFAULT. The last one wins, as in the runner.
pe_opt_value() {
  _pe_name=$1
  _pe_value=$2
  shift 2
  while [ $# -gt 0 ]; do
    case $1 in
    "$_pe_name"=*) _pe_value=${1#*=} ;;
    "$_pe_name") [ $# -gt 1 ] && _pe_value=$2 ;;
    esac
    shift
  done
  printf '%s\n' "$_pe_value"
}

# pe_pin_args ARGS...: the pins ARGS do not already set, one word per line,
# for `set -- "$@" $(pe_pin_args "$@")`. A caller's own --model or
# --judge-model wins.
pe_pin_args() {
  pe_has_opt --model "$@" || printf -- '--model\n%s\n' "$PE_MODEL"
  pe_has_opt --judge-model "$@" || printf -- '--judge-model\n%s\n' "$PE_JUDGE"
}

# pe_jobs ARGS...: the concurrency ARGS give the runner (-j N, -jN, -j=N,
# --concurrency N, --concurrency=N; the last wins), else the runner's
# default, 1.
pe_jobs() {
  _pe_value=1
  while [ $# -gt 0 ]; do
    case $1 in
    -j | --concurrency) [ $# -gt 1 ] && _pe_value=$2 ;;
    -j=* | --concurrency=*) _pe_value=${1#*=} ;;
    -j[0-9]*) _pe_value=${1#-j} ;;
    esac
    shift
  done
  printf '%s\n' "$_pe_value"
}

# pe_ncpu: this machine's logical cores.
pe_ncpu() {
  sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null || getconf _NPROCESSORS_ONLN 2>/dev/null || echo 1
}

# pe_job_cap NCPU J: the cargo and test-thread cap of one session when J
# sessions share NCPU cores: max(1, NCPU / J). Every cargo, nextest and test
# binary otherwise takes all the cores, so J sessions oversubscribe J times.
pe_job_cap() {
  _pe_value=$(($1 / $2))
  [ "$_pe_value" -ge 1 ] || _pe_value=1
  printf '%s\n' "$_pe_value"
}

# pe_cwds: `p<pid>` and `n<cwd>` lines for this user's processes: from
# /proc where it names a cwd (Linux; a removed directory's " (deleted)" mark
# dropped, as lsof shows it), else from lsof (macOS).
pe_cwds() {
  if [ -n "$(readlink /proc/self/cwd 2>/dev/null)" ]; then
    for _pe_a in /proc/[0-9]*; do
      _pe_value=$(readlink "$_pe_a/cwd" 2>/dev/null) || continue
      printf 'p%s\nn%s\n' "${_pe_a#/proc/}" "${_pe_value% (deleted)}"
    done
  else
    lsof -a -d cwd -u "$(id -u)" -Fpn 2>/dev/null
  fi
}

# pe_reap RUNROOT MARKER OWNER: ends what timed-out sessions of the
# invocation OWNER (its PID) left running. The runner kills a session's
# claude process but not the commands it started: they live on with their
# working directory inside the run's directory RUNROOT/e-*/ (measured),
# taking cores from the runs that follow.
#
# For each run directory of this invocation -- newer than MARKER, or seen so
# by an earlier call (MARKER.seen) and since removed by the runner -- the
# processes whose cwd is inside it are grouped; their roots are those whose
# parent is not among them. A session's root is its claude process, whose
# parent is a runner. When a process's parent dies, the kernel hands it to
# an ancestor of that parent: PID 1 (launchd, init) or, on Linux, the
# nearest subreaper above it (systemd --user, tini, a task runner) -- so
# never to anything below OWNER, and never to another invocation's runner.
# A directory is dead when every root's parent is PID 1 or a strict ancestor
# of OWNER: no runner holds any of it. Only then is every process whose cwd
# is inside it sent SIGTERM (its TMPDIR is inside it too, so a cargo-mutants
# copy is found). A live session's root has a runner as its parent, which is
# no ancestor of OWNER, so no process of a live session -- of this
# invocation or a concurrent one -- is ever ended. Prints what it ended.
pe_reap() {
  pe_cwds |
    awk -v root="$1/e-" '
      /^p/ { pid = substr($0, 2) }
      /^n/ { path = substr($0, 2)
             if (index(path, root) == 1) {
               rest = substr(path, length(root) + 1); sub(/\/.*/, "", rest)
               print pid, root rest } }' >"$2.cwd"
  [ -s "$2.cwd" ] || {
    rm -f "$2.cwd"
    return 0
  }
  ps -A -o pid=,ppid= >"$2.ps"
  # OWNER's strict ancestors, and PID 1: the parents an orphan can have.
  awk -v owner="$3" '{ pp[$1] = $2 } END { print 1
         for (a = pp[owner]; a != "" && a != 0 && !(a in seen); a = pp[a]) { seen[a]; print a } }' \
    "$2.ps" >"$2.adopters"
  awk 'NR == FNR { dir[$1] = $2; next } ($1 in dir) { print $1, $2, dir[$1] }' "$2.cwd" "$2.ps" >"$2.procs"
  # One line per directory with a root: the directory, then "live" or "dead".
  awk 'NR == FNR { adopter[$1]; next }
       { d[$1] = $3; pp[$1] = $2 }
       END { for (p in d) if (!(pp[p] in d) || d[pp[p]] != d[p])
         print d[p], ((pp[p] in adopter) ? "dead" : "live") }' "$2.adopters" "$2.procs" | sort -u >"$2.roots"
  # This invocation's directories, remembered while they exist, so one the
  # runner removed is still known as ours (lsof keeps naming a removed cwd).
  cut -d' ' -f1 "$2.roots" | sort -u | while read -r dir; do
    [ -e "$dir" ] && [ "$dir" -nt "$2" ] && echo "$dir"
  done >>"$2.seen"
  cut -d' ' -f1 "$2.roots" | sort -u | while read -r dir; do
    if [ -e "$dir" ]; then
      [ "$dir" -nt "$2" ] || continue
    else
      grep -qxF "$dir" "$2.seen" || continue
    fi
    grep -qxF "$dir live" "$2.roots" && continue
    awk -v dir="$dir" '$3 == dir { print $1 }' "$2.procs" | while read -r pid; do
      kill -TERM "$pid" 2>/dev/null && echo "plugin-eval: ended $pid, left running in $dir"
    done
  done
  rm -f "$2.cwd" "$2.ps" "$2.adopters" "$2.procs" "$2.roots"
}

# pe_sha256: the SHA-256 of stdin, in hex.
pe_sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum | cut -d' ' -f1
  else
    shasum -a 256 | cut -d' ' -f1
  fi
}

# Raised whenever what plugin-eval.sh vendors, or how, changes.
PE_VENDOR_FORMAT=1

# pe_vendor_key REPO REV: the content key of the dependencies vendored for
# REV -- every Cargo.toml and Cargo.lock that the scaffold copies or that a
# case's fixture or solution brings, .cargo/config.toml, the solutions'
# .solve.sh and the scaffold itself. Not the commit: a commit that changes
# none of them reuses the vendoring.
pe_vendor_key() {
  _pe_tab=$(printf '\t')
  {
    echo "vendor-format $PE_VENDOR_FORMAT"
    git -C "$1" ls-tree -r "$2" |
      grep -E "$_pe_tab"'(\.cargo/config\.toml|evals/_scaffold/scaffold\.sh|evals/[^/]+/solution/\.solve\.sh|(.*/)?Cargo\.(toml|lock))$' |
      grep -v "${_pe_tab}eval-harness/"
  } | pe_sha256 | cut -c1-16
}
