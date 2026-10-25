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
trap '[ -n "${KEEP:-}" ] || rm -rf "$scratch"' EXIT

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

# In an export, the shared vendored dependencies are cloned into the copy.
mkdir -p "$scratch/shared/serde-1.0.0" "$r/.cargo" "$r/evals/_scaffold" "$scratch/work2" "$scratch/home2"
touch "$scratch/shared/serde-1.0.0/lib.rs"
echo "$scratch/shared" >"$r/evals/_scaffold/vendor-dir"
printf '[source.vendored-sources]\ndirectory = "__VENDOR__"\n' >"$r/.cargo/config.toml"
(cd "$scratch/work2" && HOME=$scratch/home2 sh "$repo/evals/_scaffold/scaffold.sh" "$r/evals/c")
w2=$(cd "$scratch/work2" && pwd)
same "the scaffold clones the vendored dependencies" yes \
  "$([ -f "$w2/vendor/serde-1.0.0/lib.rs" ] && echo yes)"
same "and points cargo at the clone" "directory = \"$w2/vendor\"" \
  "$(grep '^directory' "$w2/.cargo/config.toml")"
rm "$r/evals/_scaffold/vendor-dir" "$r/.cargo/config.toml"

# --- plugin-eval.sh, building exports ---------------------------------------

# A repository with one case, the real scripts, and stand-ins for the tools
# an export runs: `cargo vendor` logs its call and makes the directory,
# `devenv shell` prints an environment, `claude` does nothing.
f=$scratch/fake
mkdir -p "$f/.claude-plugin" "$f/eval-harness" "$f/evals/_scaffold" "$f/evals/c/fixture" \
  "$f/evals/c/solution" "$f/docs/research/sources" "$f/docs/research/notes" "$f/skills/s" \
  "$f/crates/a" "$scratch/fakebin"
cp "$here/plugin-eval.sh" "$here/lib.sh" "$f/eval-harness/"
cp "$repo/evals/_scaffold/scaffold.sh" "$f/evals/_scaffold/"
cp "$repo/evals/own-dont-leak/scaffold.sh" "$f/evals/c/"
echo '{"name": "p"}' >"$f/.claude-plugin/plugin.json"
printf -- '---\nname: c\n---\nDo it.\n' >"$f/evals/c/prompt.md"
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
chmod +x "$scratch/fakebin/cargo" "$scratch/fakebin/devenv" "$scratch/fakebin/claude"
g2() { git -C "$f" "$@"; }
fcommit() { g2 add -A && g2 -c user.name=t -c user.email=t@t commit -qm "$1"; }
g2 init -q && fcommit base
calls=$scratch/calls
: >"$calls"
fakerun() {
  CALLS=$calls DEVENV_PROFILE=/x TMPDIR=$scratch/ftmp PATH="$scratch/fakebin:$PATH" \
    sh "$f/eval-harness/plugin-eval.sh" --case c >>"$scratch/fakerun.log" 2>&1
}
mkdir -p "$scratch/ftmp"
fakerun
fbase=$(cd "$scratch/ftmp" && pwd -P)/rust-code-plugin-eval
fsha=$(g2 rev-parse --short HEAD)
same "the first export vendors once" 1 "$(grep -c . "$calls")"
same "the vendored dependencies live outside the export" "no" \
  "$([ -e "$fbase/$fsha/root/vendor" ] && echo yes || echo no)"
same "the export names them for the scaffold to clone" \
  "$(cd "$fbase/vendor-$(pe_vendor_key "$f" HEAD)" && pwd -P)" \
  "$(cat "$fbase/$fsha/root/evals/_scaffold/vendor-dir" 2>/dev/null)"
same "and leaves the clone's path to the scaffold" 'directory = "__VENDOR__"' \
  "$(grep '^directory = ' "$fbase/$fsha/root/.cargo/config.toml" 2>/dev/null)"
echo two >"$f/skills/s/SKILL.md" && fcommit skills
fakerun
same "a skills-only commit reuses the vendoring" 1 "$(grep -c . "$calls")"
same "and still gets its own export" yes \
  "$([ -f "$fbase/$(g2 rev-parse --short HEAD)/.ready" ] && echo yes)"
echo '[package] # a dependency' >"$f/crates/a/Cargo.toml" && fcommit deps
fakerun
same "a Cargo.toml change vendors again" 2 "$(grep -c . "$calls")"

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
mkdir -p "$scratch/tmp/rust-code-plugin-eval/vendor-$(pe_vendor_key "$repo" HEAD)"
touch "$scratch/tmp/rust-code-plugin-eval/vendor-$(pe_vendor_key "$repo" HEAD)/.ready"

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
