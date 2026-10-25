#!/bin/sh
# Builds one eval case's working copy in the current, empty directory: the
# rust-code workspace as a project made from rust-template has it -- crates,
# gates, config, the docs skeleton -- without the plugin, its evals or its
# research (whose pinned sources include BeautifulRust's skill text), then the
# case's fixture on top. Every case's scaffold.sh calls this, and so does
# eval-harness, so `claude plugin eval --scaffold` and the harness grade the
# same starting point.
#
# usage: scaffold.sh <case dir>     (run from the empty working directory)
set -eu

case_dir=$(cd "$1" && pwd)
root=$(cd "$case_dir/../.." && pwd)
if [ ! -f "$root/.claude-plugin/plugin.json" ] || [ ! -f "$case_dir/prompt.md" ]; then
  echo "scaffold: $case_dir is not a case of a rust-code checkout" >&2
  exit 1
fi
if [ -n "$(ls -A .)" ]; then
  echo "scaffold: the working directory is not empty" >&2
  exit 1
fi

for entry in "$root"/* "$root"/.[!.]*; do
  [ -e "$entry" ] || continue
  name=$(basename "$entry")
  case $name in
  # The plugin's own parts stay out, or the no-plugin arm could read them;
  # graders-* and probe-* are the grader self-check and probe suites, and
  # vendor/ an older export's dependencies (eval-harness/plugin-eval.sh
  # keeps them outside the export now, and points cargo at them).
  target | .git | skills | commands | agents | hooks | evals | graders-* | probe-* | vendor | eval-harness | .claude-plugin | .claude | .direnv | .devenv* | .pre-commit-config.yaml | README.md | NOTICE.md) continue ;;
  esac
  cp -R "$entry" .
done

# The research skeleton stays; the plugin's own research goes.
find docs/research/sources -type f -exec rm {} +
printf '# Pinned research sources.\n' >docs/research/sources/MANIFEST.toml
find docs/research/notes -type f ! -name TEMPLATE.md -exec rm {} +
grep -v 'a-rust-code-evidence' docs/README.md >docs/README.md.new
mv docs/README.md.new docs/README.md

if [ -d "$case_dir/fixture" ]; then
  cp -R "$case_dir/fixture/." .
fi

# Only in an eval export (eval-harness/plugin-eval.sh writes the file): HOME
# is the run's own temp home, and devenv cannot run in the sandbox (nix must
# read /private/tmp), so every shell the agent starts loads the environment
# devenv gives this project, as direnv would -- for both arms alike.
env_file=$root/evals/_scaffold/devenv.env
if [ -f "$env_file" ] && [ ! -e "$HOME/.zshenv" ]; then
  sed "s|__ROOT__|$(pwd)|g" "$env_file" >"$HOME/.devenv.env"
  # shellcheck disable=SC2016 # expanded by the agent's shell, not here
  printf '__p=$PATH\n. "$HOME/.devenv.env"\nexport PATH="$PATH:$__p"\nunset __p\n' >"$HOME/.zshenv"
fi

# clone FROM TO: TO as a copy of directory FROM, sharing its blocks where the
# file system can (APFS clonefile, then a reflink), else a plain copy; mtimes
# kept, so cargo sees prebuilt output as fresh.
clone() {
  cp -c -pR "$1/." "$2" 2>/dev/null && return 0
  rm -rf "$2"
  cp --reflink=auto -pR "$1/." "$2" 2>/dev/null && return 0
  rm -rf "$2"
  cp -pR "$1/." "$2"
}

# Only in an eval export: the vendored dependencies (eval-harness/plugin-eval.sh
# names their shared directory in vendor-dir). The sandbox cannot read that
# directory, so each working copy gets its own clone, and cargo is pointed
# at it.
vendor_file=$root/evals/_scaffold/vendor-dir
if [ -f "$vendor_file" ]; then
  case $(cat "$root/evals/_scaffold/vendor-place" 2>/dev/null || echo work) in
  home) vendor=$HOME/vendor ;;
  *) vendor=$(pwd)/vendor ;;
  esac
  clone "$(cat "$vendor_file")" "$vendor"
  sed "s|__VENDOR__|$vendor|" .cargo/config.toml >.cargo/config.toml.new
  mv .cargo/config.toml.new .cargo/config.toml
fi
