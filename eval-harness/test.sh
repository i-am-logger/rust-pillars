#!/bin/sh
# Checks eval-harness/lib.sh function by function, and plugin-eval.sh end to
# end against a stand-in `claude` that prints its arguments, with no export
# built and no model called. Run by `cargo test --manifest-path
# eval-harness/Cargo.toml`; on its own: sh eval-harness/test.sh
set -u
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/.." && pwd)
# shellcheck disable=SC1091 # lib.sh is checked as a file of its own
. "$here/lib.sh"

failed=0
# same WHAT EXPECTED ACTUAL
same() {
  if [ "$2" = "$3" ]; then
    echo "ok    $1"
  else
    printf 'FAIL  %s\n  expected: %s\n  actual:   %s\n' "$1" "$2" "$3"
    failed=$((failed + 1))
  fi
}
# shellcheck disable=SC2048,SC2086 # split on purpose: one word per line in
words() { printf '%s ' $* | sed 's/ $//'; }

scratch=$(mktemp -d "${TMPDIR:-/tmp}/plugin-eval-test.XXXXXX")
trap 'rm -rf "$scratch"' EXIT

# --- lib.sh ---------------------------------------------------------------

same "pins both when the caller sets neither" \
  "--model claude-opus-5-5 --judge-model haiku" "$(words "$(pe_pin_args -j 4 --no-publish)")"
same "the caller's --model wins" \
  "--judge-model haiku" "$(words "$(pe_pin_args --model haiku --case x)")"
same "the caller's --judge-model=... wins" \
  "--model claude-opus-5-5" "$(words "$(pe_pin_args --judge-model=sonnet)")"
same "an option value is not an option" \
  "--model claude-opus-5-5 --judge-model haiku" "$(words "$(pe_pin_args --case --model-like)")"
same "-j's value, the last one" "8" "$(pe_opt_value -j 1 -j 4 --runs 1 -j 8)"
same "-j's default" "1" "$(pe_opt_value -j 1 --runs 1)"
same "--opt=value" "3" "$(pe_opt_value --runs 1 --runs=3)"

# --- plugin-eval.sh, end to end -------------------------------------------

# A stand-in claude: prints its working directory's name and its arguments.
mkdir -p "$scratch/bin"
cat >"$scratch/bin/claude" <<'EOF'
#!/bin/sh
printf 'cwd=%s\n' "$(basename "$PWD")"
printf 'arg=%s\n' "$@"
EOF
chmod +x "$scratch/bin/claude"

# An export that is already built, so the script goes straight to the run.
sha=$(git -C "$repo" rev-parse --short HEAD)
mkdir -p "$scratch/tmp/rust-code-plugin-eval/$sha/root"
touch "$scratch/tmp/rust-code-plugin-eval/$sha/.ready"

# run ARGS...: plugin-eval.sh's runner arguments for ARGS, space-separated.
run() {
  TMPDIR=$scratch/tmp PATH="$scratch/bin:$PATH" sh "$here/plugin-eval.sh" "$@" 2>/dev/null |
    sed -n 's/^arg=//p' | tr '\n' ' ' | sed 's/ $//'
}

same "a suite run pins the model and the judge" \
  "plugin eval . --scaffold --trust-plugin --allow-tools Bash Edit Write -j 4 --no-publish --model claude-opus-5-5 --judge-model haiku" \
  "$(run -j 4 --no-publish)"
same "a suite run keeps the caller's model" \
  "plugin eval . --scaffold --trust-plugin --allow-tools Bash Edit Write --model haiku --judge-model haiku" \
  "$(run --model haiku)"

if [ "$failed" -ne 0 ]; then
  echo "$failed check(s) failed"
  exit 1
fi
echo "all checks passed"
