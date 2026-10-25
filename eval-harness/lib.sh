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
  name=$1
  shift
  for a in "$@"; do
    case $a in
    "$name" | "$name"=*) return 0 ;;
    esac
  done
  return 1
}

# pe_opt_value NAME DEFAULT ARGS...: the value ARGS give option NAME, or
# DEFAULT. The last one wins, as in the runner.
pe_opt_value() {
  name=$1
  value=$2
  shift 2
  while [ $# -gt 0 ]; do
    case $1 in
    "$name"=*) value=${1#*=} ;;
    "$name") [ $# -gt 1 ] && value=$2 ;;
    esac
    shift
  done
  printf '%s\n' "$value"
}

# pe_pin_args ARGS...: the pins ARGS do not already set, one word per line,
# for `set -- "$@" $(pe_pin_args "$@")`. A caller's own --model or
# --judge-model wins.
pe_pin_args() {
  pe_has_opt --model "$@" || printf -- '--model\n%s\n' "$PE_MODEL"
  pe_has_opt --judge-model "$@" || printf -- '--judge-model\n%s\n' "$PE_JUDGE"
}
