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
