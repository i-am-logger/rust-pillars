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
mkdir -p "$scratch/systmp" "$scratch/pids"
# A temp dir of this run's own, so two runs at once never share a file.
TMPDIR=$scratch/systmp
export TMPDIR
# Every process the test plants writes its PID to pids/NAME; the test looks
# at and ends exactly those, never a process found by its command line (a
# second run, or the user's own `sleep 300`, would match). The cleanup runs
# on every exit, a signal's too.
cleanup() {
  for p in "$scratch"/pids/*; do
    [ -f "$p" ] || continue
    pid=$(cat "$p")
    case $(ps -o args= -p "$pid" 2>/dev/null) in
    sleep\ *) kill "$pid" 2>/dev/null ;;
    esac
  done
  [ -n "${KEEP:-}" ] || {
    chmod -R u+w "$scratch" 2>/dev/null
    rm -rf "$scratch"
  }
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
# plant DIR NAME SECONDS: an orphan `sleep SECONDS` with its cwd in DIR (its
# parent exits at once, as a timed-out session's shell does), its PID in
# pids/NAME.
plant() {
  # shellcheck disable=SC2016 # expanded by the inner shell
  (cd "$1" && sh -c 'sleep "$1" </dev/null >/dev/null 2>&1 & echo $! >"$2"' sh "$3" "$scratch/pids/$2")
}
# alive NAME: whether the process planted as NAME still runs (a zombie is
# gone).
alive() {
  case $(ps -o stat= -p "$(cat "$scratch/pids/$1" 2>/dev/null)" 2>/dev/null) in
  "" | Z*) echo gone ;;
  *) echo alive ;;
  esac
}

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
same "-j N" 4 "$(pe_jobs --runs 1 -j 4)"
same "-jN, --concurrency=N, last wins" 6 "$(pe_jobs -j8 --concurrency=6)"
same "--concurrency N" 2 "$(pe_jobs --concurrency 2 --no-publish)"
same "the runner's default concurrency" 1 "$(pe_jobs --runs 3)"
same "18 cores, -j 4" 4 "$(pe_job_cap 18 4)"
same "18 cores, -j 8" 2 "$(pe_job_cap 18 8)"
same "never below one" 1 "$(pe_job_cap 4 8)"
# The reaper: an orphan in one run directory is ended; a live session's
# process in another, and anything in a run older than the marker, is not.
runs=$scratch/runs
mkdir -p "$runs/e-old/home/cwd" "$runs/e-gone/home/cwd" "$runs/e-othergone/home/cwd"
plant "$runs/e-gone/home/cwd" gone 300
plant "$runs/e-othergone/home/cwd" othergone 300
sleep 1
touch "$scratch/marker"
sleep 1
mkdir -p "$runs/e-dead/home/cwd" "$runs/e-live/home/cwd"
plant "$runs/e-old/home/cwd" old 300
plant "$runs/e-dead/home/cwd" dead 300
# A live session: its root's parent is this shell, outside the directory.
(cd "$runs/e-live/home/cwd" && exec sleep 300 >/dev/null 2>&1) &
echo $! >"$scratch/pids/live"
rr=$(cd "$runs" && pwd -P)
# An earlier poll saw e-gone as this invocation's; e-othergone it never saw.
echo "$rr/e-gone" >"$scratch/marker.seen"
rm -rf "$runs/e-gone" "$runs/e-othergone"
sleep 1
same "an orphan exists before the reap" alive "$(alive dead)"
pe_reap "$rr" "$scratch/marker" "$$" >/dev/null
sleep 1
same "the reaper ends an orphan of a dead session" gone "$(alive dead)"
same "and one of its own runs the runner already removed" gone "$(alive gone)"
same "but not one of a removed run it never saw" alive "$(alive othergone)"
same "and leaves a live session's process" alive "$(alive live)"
same "and a run older than this invocation" alive "$(alive old)"
same "and leaves no file of its own behind" "" \
  "$(cd "$scratch" && ls -d marker.cwd marker.ps marker.adopters marker.procs marker.roots 2>/dev/null)"
pe_reap "$scratch/nothing" "$scratch/marker" "$$" >/dev/null
same "nor when no process is in a run directory" "" "$(cd "$scratch" && ls -d marker.cwd 2>/dev/null)"
# Where the parent of an orphan's dead parent goes: PID 1 on macOS, but on
# Linux the nearest subreaper above it (systemd --user, tini, a task runner),
# which is an ancestor of the invocation, not PID 1. Here the invocation is
# a shell one below this one, so this shell stands for that subreaper.
runs3=$scratch/runs3
mkdir -p "$runs3"
touch "$scratch/marker3"
sleep 1
mkdir -p "$runs3/e-adopted/home/cwd" "$runs3/e-other/home/cwd"
(cd "$runs3/e-adopted/home/cwd" && exec sleep 300 >/dev/null 2>&1) &
echo $! >"$scratch/pids/adopted"
# A concurrent invocation's live session: its parent is that invocation's
# runner, which is no ancestor of this one.
# shellcheck disable=SC2016 # expanded by the inner shell
sh -c '(cd "$1" && exec sleep 300 >/dev/null 2>&1) & echo $! >"$2"; wait' sh \
  "$runs3/e-other/home/cwd" "$scratch/pids/other" &
sleep 1
rr3=$(cd "$runs3" && pwd -P)
# shellcheck disable=SC2016 # expanded by the inner shell
sh -c '. "$1"; pe_reap "$2" "$3" "$$"' sh "$here/lib.sh" "$rr3" "$scratch/marker3" >/dev/null
sleep 1
same "an orphan a subreaper above the invocation adopted is ended" gone "$(alive adopted)"
same "a concurrent invocation's live session is left" alive "$(alive other)"
name=mine value=mine a=mine tab=mine
pe_opt_value --json x --json y >/dev/null
pe_has_opt --json --json y && pe_vendor_key "$repo" HEAD >/dev/null
same "the functions leave the caller's variables alone" "mine mine mine mine" "$name $value $a $tab"

# The vendoring's content key, over a repository of its own.
g=$scratch/keyrepo
mkdir -p "$g/skills/s" "$g/crates/a" "$g/evals/c/fixture/crates/b" "$g/eval-harness"
for f in Cargo.toml Cargo.lock crates/a/Cargo.toml evals/c/fixture/crates/b/Cargo.toml \
  eval-harness/Cargo.toml skills/s/SKILL.md README.md; do
  echo one >"$g/$f"
done
commit() { git -C "$g" add -A && git -C "$g" -c user.name=t -c user.email=t@t commit -qm "$1"; }
git -C "$g" init -q && commit base
k0=$(pe_vendor_key "$g" HEAD)
echo two >"$g/skills/s/SKILL.md" && echo two >"$g/README.md" && commit skills
same "a skills-only commit keeps the vendor key" "$k0" "$(pe_vendor_key "$g" HEAD)"
echo two >"$g/eval-harness/Cargo.toml" && commit harness
same "the harness's own lockfile is not vendored" "$k0" "$(pe_vendor_key "$g" HEAD)"
echo two >"$g/evals/c/fixture/crates/b/Cargo.toml" && commit fixture
k1=$(pe_vendor_key "$g" HEAD)
[ "$k1" != "$k0" ] && k1=changed
same "a fixture's Cargo.toml changes the vendor key" changed "$k1"
echo two >"$g/Cargo.lock" && commit lock
k2=$(pe_vendor_key "$g" HEAD)
[ "$k2" != "$(pe_vendor_key "$g" HEAD~1)" ] && k2=changed
same "the lockfile changes the vendor key" changed "$k2"

# Tiers and the changed-cases selector.
same "the export adds only case-<name>; a tier is the case's own tag" \
  "tags: [case-layering-fix]" "$(pe_tags layering-fix)"
same "a case that tags itself smoke keeps it" "tags: [smoke, case-own-dont-leak]" \
  "$(pe_tags_merge own-dont-leak 'tags: [smoke]')"
same "a case's own tags are kept and ours added" "tags: [slow, case-no-std-port]" \
  "$(pe_tags_merge no-std-port 'tags: [slow]')"
same "and not repeated" "tags: [smoke, case-own-dont-leak]" \
  "$(pe_tags_merge own-dont-leak 'tags: [smoke, case-own-dont-leak]')"
same "an empty list" "tags: [case-x]" "$(pe_tags_merge x 'tags: []')"
mkdir -p "$g/evals/d" && echo one >"$g/evals/d/prompt.md" && commit addcase
k=$(git -C "$g" rev-parse HEAD)
echo four >"$g/evals/c/fixture/crates/b/Cargo.toml" && echo two >"$g/evals/d/prompt.md" && commit cases
same "changes inside case directories pick those cases" "c d" "$(words "$(pe_changed_cases "$g" "$k")")"
echo three >"$g/skills/s/SKILL.md" && commit skill
same "a skill change picks every case" all "$(pe_changed_cases "$g" "$k")"
k=$(git -C "$g" rev-parse HEAD)
mkdir -p "$g/evals/_scaffold" && echo x >"$g/evals/_scaffold/scaffold.sh" && commit scaffold
same "a scaffold change picks every case" all "$(pe_changed_cases "$g" "$k")"
same "nothing changed picks nothing" "" "$(pe_changed_cases "$g" HEAD)"

# --- scaffold.sh -----------------------------------------------------------

# A root with what scaffold.sh needs, plus what it must leave out.
r=$scratch/root
mkdir -p "$r/.claude-plugin" "$r/evals/c" "$r/crates/a" "$r/docs/research/sources" \
  "$r/docs/research/notes" "$r/vendor/dep" "$r/probe-env/env" "$r/graders-start/c" "$r/skills/s"
touch "$r/.claude-plugin/plugin.json" "$r/evals/c/prompt.md" "$r/crates/a/lib.rs" \
  "$r/docs/README.md" "$r/vendor/dep/lib.rs" "$r/probe-env/env/prompt.md" \
  "$r/graders-start/c/prompt.md" "$r/skills/s/SKILL.md"
echo docs >"$r/docs/README.md"
mkdir "$scratch/work" "$scratch/home"
(cd "$scratch/work" && HOME=$scratch/home sh "$repo/evals/_scaffold/scaffold.sh" "$r/evals/c")
same "the scaffold runs to the end" 0 "$?"
same "the scaffold copies the workspace" yes "$([ -f "$scratch/work/crates/a/lib.rs" ] && echo yes)"
same "the scaffold leaves out vendor/, probe-*/, graders-*/ and the plugin" "" \
  "$(cd "$scratch/work" && ls -d vendor probe-env graders-start skills evals 2>/dev/null)"

mtime() { perl -e 'print +(stat shift)[9] // ""' "$1"; }

# Base files keep their mtimes (a prebuilt target was built from them, later);
# a fixture's files are new, so whatever the case changes rebuilds.
mkdir -p "$r/evals/c/fixture/crates/a" "$scratch/work2" "$scratch/home2"
echo changed >"$r/evals/c/fixture/crates/a/lib.rs"
touch -t 200001010000 "$r/crates/a/lib.rs" "$r/evals/c/fixture/crates/a/lib.rs" "$r/layers.toml"
(cd "$scratch/work2" && HOME=$scratch/home2 sh "$repo/evals/_scaffold/scaffold.sh" "$r/evals/c")
same "a base file keeps its mtime" "$(mtime "$r/layers.toml")" "$(mtime "$scratch/work2/layers.toml")"
same "a fixture file is copied new" yes \
  "$([ "$(mtime "$scratch/work2/crates/a/lib.rs")" -gt "$(mtime "$r/crates/a/lib.rs")" ] && echo yes)"
rm -r "$r/evals/c/fixture"

# The base workspace alone (the scaffold's own directory as the case), with
# a prebuilt target cloned in, its mtimes kept.
mkdir -p "$scratch/prebuilt/target/debug" "$r/evals/_scaffold" "$scratch/work3" "$scratch/home3"
touch -t 200001010000 "$scratch/prebuilt/target/debug/dep.rlib"
echo "$scratch/prebuilt/target" >"$r/evals/_scaffold/target-dir"
(cd "$scratch/work3" && HOME=$scratch/home3 sh "$repo/evals/_scaffold/scaffold.sh" "$r/evals/_scaffold")
same "the scaffold builds the base workspace" yes "$([ -f "$scratch/work3/crates/a/lib.rs" ] && echo yes)"
same "the scaffold clones the prebuilt target, mtimes kept" \
  "$(mtime "$scratch/prebuilt/target/debug/dep.rlib")" "$(mtime "$scratch/work3/target/debug/dep.rlib")"
same "no vendored tree in the working copy or its home" "" \
  "$(ls -d "$scratch/work3/vendor" "$scratch/home3/vendor" 2>/dev/null)"
rm "$r/evals/_scaffold/target-dir"

# --- plugin-eval.sh, building exports ---------------------------------------

# A repository with one case, the real scripts, and stand-ins for the tools
# an export runs: `cargo vendor` logs its call and makes the directory,
# `devenv shell` prints an environment, `claude` does nothing.
f=$scratch/fake
mkdir -p "$f/.claude-plugin" "$f/eval-harness" "$f/evals/_scaffold" "$f/evals/c/fixture" \
  "$f/evals/c/solution" "$f/docs/research/sources" "$f/docs/research/notes" "$f/skills/s" \
  "$f/crates/a" "$scratch/fakebin"
cp "$here/plugin-eval.sh" "$here/lib.sh" "$f/eval-harness/"
mkdir -p "$f/eval-harness/fixtures"
echo '{"name": "some-grader"}' >"$f/eval-harness/fixtures/run.json"
cp "$repo/evals/_scaffold/scaffold.sh" "$f/evals/_scaffold/"
cp "$repo/evals/own-dont-leak/scaffold.sh" "$f/evals/c/"
echo '{"name": "p"}' >"$f/.claude-plugin/plugin.json"
printf -- '---\nname: c\n---\nDo it.\n' >"$f/evals/c/prompt.md"
# A case that puts itself in the smoke tier, as evals/own-dont-leak does.
mkdir -p "$f/evals/s/solution"
cp "$repo/evals/own-dont-leak/scaffold.sh" "$f/evals/s/"
printf -- '---\nname: s\ndescription: d\ntags: [smoke]\nruns: 3\n---\nDo it.\n' >"$f/evals/s/prompt.md"
touch "$f/evals/s/solution/a.txt"
printf '[workspace]\nmembers = ["crates/a"]\n' >"$f/Cargo.toml"
echo '[package]' >"$f/crates/a/Cargo.toml"
echo one >"$f/skills/s/SKILL.md"
touch "$f/docs/research/notes/TEMPLATE.md" "$f/docs/research/sources/MANIFEST.toml" "$f/evals/c/fixture/a.txt" "$f/evals/c/solution/a.txt"
echo docs >"$f/docs/README.md"
cat >"$scratch/fakebin/cargo" <<'EOF'
#!/bin/sh
[ "$1" = vendor ] || exit 0
echo "vendor $PWD" >>"$CALLS"
shift
for a in "$@"; do case $a in -*) ;; *) mkdir -p "$a"; touch "$a/dep"; break ;; esac; done
EOF
cat >"$scratch/fakebin/devenv" <<'EOF'
#!/bin/sh
printf 'declare\nexport PATH="/nix/x/bin"\nexport RUSTFLAGS="-D warnings"\n'
EOF
printf '#!/bin/sh\n' >"$scratch/fakebin/claude"
# A stand-in store: --add copies DIR in and prints its path; --add-root is a no-op.
cat >"$scratch/fakebin/nix-store" <<'EOF'
#!/bin/sh
[ "$1" = --add ] || exit 0
mkdir -p "$FAKESTORE"
cp -R "$2" "$FAKESTORE/"
echo "$FAKESTORE/$(basename "$2")"
EOF
chmod +x "$scratch/fakebin/cargo" "$scratch/fakebin/devenv" "$scratch/fakebin/claude" "$scratch/fakebin/nix-store"
g2() { git -C "$f" "$@"; }
fcommit() { g2 add -A && g2 -c user.name=t -c user.email=t@t commit -qm "$1"; }
g2 init -q && fcommit base
calls=$scratch/calls
: >"$calls"
fakerun() {
  CALLS=$calls FAKESTORE=$scratch/store DEVENV_PROFILE=/x TMPDIR=$scratch/ftmp PATH="$scratch/fakebin:$PATH" \
    sh "$f/eval-harness/plugin-eval.sh" --case c >>"$scratch/fakerun.log" 2>&1
}
mkdir -p "$scratch/ftmp"
fakerun
fbase=$(cd "$scratch/ftmp" && pwd -P)/rust-code-plugin-eval
fsha=$(g2 rev-parse --short HEAD)
same "the first export vendors once" 1 "$(grep -c . "$calls")"
same "the harness's recorded results stay out of the export" "no" \
  "$([ -e "$fbase/$fsha/root/eval-harness/fixtures" ] && echo yes || echo no)"
same "the vendored dependencies live outside the export" "no" \
  "$([ -e "$fbase/$fsha/root/vendor" ] && echo yes || echo no)"
same "they go to the store, and cargo is pointed at the store path" \
  "directory = \"$scratch/store/vendored-crates-$(pe_vendor_key "$f" HEAD)\"" \
  "$(grep '^directory = ' "$fbase/$fsha/root/.cargo/config.toml" 2>/dev/null)"
same "the store holds what cargo vendor made" yes \
  "$([ -f "$scratch/store/vendored-crates-$(pe_vendor_key "$f" HEAD)/dep" ] && echo yes)"
fenv=$fbase/$fsha/root/evals/_scaffold/devenv.env
same "the export tags its copy of each case" "tags: [case-c]" \
  "$(sed -n '2p' "$fbase/$fsha/root/evals/c/prompt.md" 2>/dev/null)"
same "and leaves the case's own files alone" "name: c" "$(sed -n '2p' "$f/evals/c/prompt.md")"
same "a case's own smoke tag gets case-<name> beside it, once" "tags: [smoke, case-s]" \
  "$(grep '^tags:' "$fbase/$fsha/root/evals/s/prompt.md" 2>/dev/null)"
# No profile of its own: a lighter one was never shown faster in the sandbox.
same "the export's environment keeps the project's dev profile" "" \
  "$(grep -E 'CARGO_PROFILE_|CARGO_INCREMENTAL' "$fenv" 2>/dev/null)"
# shellcheck disable=SC2016 # expanded by the inner shells
same "and derives the job caps from the invocation's EVAL_JOB_CAP" "3 3 3" \
  "$(EVAL_JOB_CAP=3 sh -c '. "$1"; echo "$CARGO_BUILD_JOBS $NEXTEST_TEST_THREADS $RUST_TEST_THREADS"' sh "$fenv" 2>/dev/null)"
# shellcheck disable=SC2016 # expanded by the inner shell
same "and sets no cap without one" "/" \
  "$(env -u EVAL_JOB_CAP sh -c '. "$1"; echo "${CARGO_BUILD_JOBS:-}/"' sh "$fenv" 2>/dev/null)"
same "the export names a prebuilt target for the scaffold to clone" yes \
  "$(d=$(cat "$fbase/$fsha/root/evals/_scaffold/target-dir" 2>/dev/null) && [ -d "$d" ] && echo yes)"
echo two >"$f/skills/s/SKILL.md" && fcommit skills
fakerun
same "a skills-only commit reuses the vendoring" 1 "$(grep -c . "$calls")"
same "and still gets its own export" yes \
  "$([ -f "$fbase/$(g2 rev-parse --short HEAD)/.ready" ] && echo yes)"
echo '[package] # a dependency' >"$f/crates/a/Cargo.toml" && fcommit deps
fakerun
same "a Cargo.toml change vendors again" 2 "$(grep -c . "$calls")"

# --- plugin-eval.sh, end to end -------------------------------------------

# A stand-in claude: prints its working directory's name, the job cap and
# its arguments; for a suite run it also leaves a command running in a run
# directory, as a timed-out session does.
mkdir -p "$scratch/bin" "$scratch/runs2"
cat >"$scratch/bin/claude" <<'EOF'
#!/bin/sh
[ "$1" = --version ] && { echo "9.9.9 (Claude Code)"; exit 0; }
printf 'cwd=%s\n' "$(basename "$PWD")"
printf 'cap=%s\n' "${EVAL_JOB_CAP:-}"
printf 'arg=%s\n' "$@"
if [ -n "${LEAVE_ORPHAN:-}" ]; then
  mkdir -p "$PE_RUN_ROOT/e-run1/home/cwd"
  # shellcheck disable=SC2016 # expanded by the inner shell
  (cd "$PE_RUN_ROOT/e-run1/home/cwd" && sh -c 'sleep 300 </dev/null >/dev/null 2>&1 & echo $! >"$1"' sh "$ORPHAN_PID")
fi
EOF
chmod +x "$scratch/bin/claude"

# An export that is already built, so the script goes straight to the run.
sha=$(git -C "$repo" rev-parse --short HEAD)
mkdir -p "$scratch/tmp/rust-code-plugin-eval/$sha/root/evals/own-dont-leak" \
  "$scratch/tmp/rust-code-plugin-eval/$sha/root/evals/layering-fix" \
  "$scratch/tmp/rust-code-plugin-eval/$sha/root/evals/_scaffold"
touch "$scratch/tmp/rust-code-plugin-eval/$sha/.ready" "$scratch/tmp/rust-code-plugin-eval/$sha/root/evals/_scaffold/devenv.env"
echo basekey >"$scratch/tmp/rust-code-plugin-eval/$sha/base-key"
mkdir -p "$scratch/vendored"
echo "$scratch/vendored" >"$scratch/tmp/rust-code-plugin-eval/vendor-$(pe_vendor_key "$repo" HEAD).store"

# pe ARGS...: plugin-eval.sh against that export, the stand-in claude and a
# run root of the test's own.
pe() {
  TMPDIR=$scratch/tmp PE_RUN_ROOT=$(cd "$scratch/runs2" && pwd -P) PATH="$scratch/bin:$PATH" \
    sh "$here/plugin-eval.sh" "$@" 2>/dev/null
}
# run ARGS...: the runner's arguments for ARGS, space-separated, with the
# result directory the script picks shown as DIR.
run() {
  pe "$@" | sed -n 's/^arg=//p' | tr '\n' ' ' | sed -e 's/ $//' -e 's|--output-dir [^ ]*|--output-dir DIR|'
}

same "a suite run pins the model and the judge" \
  "plugin eval . --scaffold --trust-plugin --allow-tools Bash Edit Write -j 4 --no-publish --model claude-opus-5-5 --judge-model haiku --output-dir DIR" \
  "$(run -j 4 --no-publish)"
same "a suite run keeps the caller's model" \
  "plugin eval . --scaffold --trust-plugin --allow-tools Bash Edit Write --model haiku --judge-model haiku --output-dir DIR" \
  "$(run --model haiku)"
same "a suite run hands its sessions the job cap for its -j" "$(pe_job_cap "$(pe_ncpu)" 4)" \
  "$(pe -j 4 | sed -n 's/^cap=//p')"

# Keys beside the result, for a later --reuse-baseline.
pe -j 4 --json "$scratch/r1.json" >/dev/null
pe -j 4 --json "$scratch/r2.json" >/dev/null
pe -j 8 --json "$scratch/r3.json" >/dev/null
(cd "$scratch" && pe -j 4 --json rel.json >/dev/null)
same "a relative --json is the caller's, and its keys sit beside it" yes \
  "$([ -s "$scratch/rel.keys" ] && echo yes)"
same "and the runner is given it absolute" yes \
  "$(cd "$scratch" && pe --json rel.json | grep -qx "arg=$(cd "$scratch" && pwd)/rel.json" && echo yes)"
same "a run writes a key per case beside its result" "layering-fix own-dont-leak" \
  "$(cut -d' ' -f1 "$scratch/r1.keys" 2>/dev/null | sort | tr '\n' ' ' | sed 's/ $//')"
same "the same run gets the same keys" "$(cat "$scratch/r1.keys" 2>/dev/null)" "$(cat "$scratch/r2.keys" 2>/dev/null)"
same "another -j gets other keys" yes \
  "$([ -s "$scratch/r3.keys" ] && ! cmp -s "$scratch/r1.keys" "$scratch/r3.keys" && echo yes)"
# keyed ARGS...: the keys a run with ARGS writes.
keyed() {
  rm -f "$scratch/k.keys"
  pe "$@" --json "$scratch/k.json" >/dev/null
  cat "$scratch/k.keys" 2>/dev/null
}
kbase=$(keyed -j 4 --runs 3)
for opt in "--runs 1" "--allow-tools Read" "--disallow-tools Agent" "--max-cost-usd 5" \
  "--mocks off" "--allow-real-servers" "--tag smoke" "--threshold 0.5" "--model sonnet"; do
  # shellcheck disable=SC2086 # one word per option and value
  same "$opt changes the keys" yes \
    "$(k=$(keyed -j 4 --runs 3 $opt) && [ -n "$k" ] && [ "$k" != "$kbase" ] && echo yes)"
done
same "--runs=3 is --runs 3, and -j4 is -j 4" "$kbase" "$(keyed -j4 --runs=3)"
same "a result or report path and --no-publish leave the keys" "$kbase" \
  "$(keyed -j 4 --runs 3 --no-publish --report "$scratch/r.html" --json "$scratch/other.json")"
same "-j 1 is the default concurrency" "$(keyed --runs 3)" "$(keyed -j 1 --runs 3)"
pe --reuse-baseline "$scratch/r1.json" "$scratch/m.json" -j 4 >/dev/null
same "a with-arm run of --reuse-baseline keys as the two-arm run it reuses" \
  "$(cat "$scratch/r1.keys")" "$(cat "$scratch/m.keys" 2>/dev/null)"
same "--reuse-baseline runs the arm with the plugin only" yes \
  "$(run --reuse-baseline "$scratch/r1.json" "$scratch/m.json" -j 4 | grep -q -- '--ablation none --json .*-with.json' && echo yes)"
same "--reuse-baseline needs a baseline with keys" "" \
  "$(run --reuse-baseline "$scratch/nokeys.json" "$scratch/m.json")"
same "--keep-state keeps the run directories for the harvest" yes \
  "$(run --keep-state -j 2 | grep -q -- '--keep-temp' && echo yes)"

# What a timed-out session leaves running is ended when the suite is over.
LEAVE_ORPHAN=1 ORPHAN_PID=$scratch/pids/suite pe -j 1 >/dev/null
sleep 1
same "the stand-in left a process behind" yes "$([ -s "$scratch/pids/suite" ] && echo yes)"
same "a suite run ends what a dead session left running" gone "$(alive suite)"

if [ "$failed" -ne 0 ]; then
  echo "$failed check(s) failed"
  exit 1
fi
echo "all checks passed"
