#!/bin/sh
# Checks gate-on-edit.sh against a stand-in `cargo` that records where it ran:
# the hook gates a .rs file inside the project, and runs nothing for a file
# outside it, a relative path, a `..` or link escape, or a project nested under
# another one's layers.toml. Each run is cut off after 3 s, so a walk that
# never ends fails instead of hanging. On its own: sh hooks/test-gate-on-edit.sh
set -u
here=$(cd "$(dirname "$0")" && pwd)
hook=$here/gate-on-edit.sh
shell=$(command -v sh)

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

scratch=$(mktemp -d "${TMPDIR:-/tmp}/gate-on-edit-test.XXXXXX")
scratch=$(cd "$scratch" && pwd -P)
pid=
cleanup() {
  [ -n "$pid" ] && kill "$pid" 2>/dev/null
  rm -rf "$scratch"
}
trap cleanup EXIT
trap 'exit 1' INT TERM HUP

# The stand-in `cargo`: appends the directory it ran in to ran.log and
# reports a finding, as a failing gate does.
mkdir -p "$scratch/bin" "$scratch/empty"
cat >"$scratch/bin/cargo" <<EOF
#!/bin/sh
pwd -P >>"$scratch/ran.log"
echo "[budgets] 1 finding(s)"
EOF
chmod +x "$scratch/bin/cargo"

# A PATH without jq, for the fallback parse: only the tools the hook calls.
mkdir -p "$scratch/nojq"
cp "$scratch/bin/cargo" "$scratch/nojq/cargo"
for t in cat tr sed head dirname grep; do
  ln -s "$(command -v "$t")" "$scratch/nojq/$t"
done

# gates DIR: a workspace with rust-template's gates at DIR.
gates() {
  mkdir -p "$1/xtask" "$1/src"
  : >"$1/layers.toml"
  : >"$1/src/lib.rs"
}
proj=$scratch/proj
outside=$scratch/outside
gates "$proj"
gates "$outside"
ln -s "$outside" "$proj/link"
mkdir -p "$proj/a,b" "$outside/nested/src"
: >"$proj/a,b/lib.rs"
: >"$outside/nested/src/lib.rs"

# run PATHVAR PROJECT FILE: runs the hook from $scratch/empty on an Edit of
# FILE; prints its exit status (`hung` if it ran past 3 s) and where cargo ran
# (`-` if nowhere), with $scratch shown as `@`.
run() {
  rm -f "$scratch/ran.log"
  printf '{"session_id":"s","tool_name":"Edit","tool_input":{"file_path":"%s","old_string":"a","new_string":"b"}}' "$3" >"$scratch/input.json"
  (cd "$scratch/empty" && PATH=$1 CLAUDE_PROJECT_DIR=$2 CLAUDE_PLUGIN_OPTION_GATE_ON_EDIT=true \
    exec "$shell" "$hook" <"$scratch/input.json" >/dev/null 2>&1) &
  pid=$!
  n=0
  while kill -0 "$pid" 2>/dev/null && [ "$n" -lt 30 ]; do
    sleep 0.1
    n=$((n + 1))
  done
  if kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null
    wait "$pid" 2>/dev/null
    status=hung
  else
    wait "$pid"
    status=$?
  fi
  pid=
  where=-
  [ -f "$scratch/ran.log" ] && where=$(sed "s|^$scratch|@|" "$scratch/ran.log" | sort -u | tr '\n' ' ' | sed 's/ $//')
  echo "$status $where"
}

with_jq=$scratch/bin:$PATH
no_jq=$scratch/nojq

for p in "$with_jq" "$no_jq"; do
  [ "$p" = "$no_jq" ] && arm="no jq" || arm="jq"
  same "$arm: a file inside the project is gated there" "2 @/proj" "$(run "$p" "$proj" "$proj/src/lib.rs")"
  same "$arm: a file outside the project runs nothing" "0 -" "$(run "$p" "$proj" "$outside/src/lib.rs")"
  same "$arm: a relative path runs nothing and ends" "0 -" "$(run "$p" "$proj" "src/lib.rs")"
  same "$arm: a .. escape from the project runs nothing" "0 -" "$(run "$p" "$proj" "$proj/../outside/src/lib.rs")"
  same "$arm: a link out of the project runs nothing" "0 -" "$(run "$p" "$proj" "$proj/link/src/lib.rs")"
  same "$arm: the walk stops at the project, not a layers.toml above it" "0 -" \
    "$(run "$p" "$outside/nested" "$outside/nested/src/lib.rs")"
  same "$arm: no CLAUDE_PROJECT_DIR runs nothing" "0 -" "$(run "$p" "" "$proj/src/lib.rs")"
  same "$arm: a file that is not .rs runs nothing" "0 -" "$(run "$p" "$proj" "$proj/layers.toml")"
done
# jq reads the field whole; the fallback splits on commas and skips the file.
same "jq: a path with a comma is gated" "2 @/proj" "$(run "$with_jq" "$proj" "$proj/a,b/lib.rs")"
same "no jq: a path with a comma runs nothing" "0 -" "$(run "$no_jq" "$proj" "$proj/a,b/lib.rs")"

# Off unless the user turned it on.
rm -f "$scratch/ran.log"
printf '{"tool_input":{"file_path":"%s"}}' "$proj/src/lib.rs" |
  (cd "$scratch/empty" && PATH=$with_jq CLAUDE_PROJECT_DIR=$proj sh "$hook" >/dev/null 2>&1)
same "off by default: runs nothing" "0 -" "$? $([ -f "$scratch/ran.log" ] && echo ran || echo -)"

[ "$failed" -eq 0 ] || {
  echo "$failed failed"
  exit 1
}
echo "all passed"
