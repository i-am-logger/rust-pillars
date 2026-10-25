#!/bin/sh
# After Claude edits a .rs file, run the project's fast source gates and show
# Claude what they found, so a violation is fixed in the same turn instead of
# at `devenv test`. Off unless the user turned it on (the plugin's
# `gate_on_edit` option), and a no-op outside a project with rust-template's
# gates. Exit 2 shows stderr to Claude; it cannot undo the edit.
#
# `cargo xtask` runs the project's own code (its xtask, build scripts and
# cargo config), so the hook acts only on a file inside the open project
# ($CLAUDE_PROJECT_DIR), after resolving links and `..`, and looks for the
# gates no higher than the project's root. Checked by test-gate-on-edit.sh.
set -u

[ "${CLAUDE_PLUGIN_OPTION_GATE_ON_EDIT:-false}" = "true" ] || exit 0
[ -n "${CLAUDE_PROJECT_DIR:-}" ] || exit 0
project=$(cd "$CLAUDE_PROJECT_DIR" 2>/dev/null && pwd -P) || exit 0

input=$(cat)
if command -v jq >/dev/null 2>&1; then
  file=$(printf '%s' "$input" | jq -r '.tool_input.file_path // empty' 2>/dev/null) || exit 0
else
  # Without jq: the first "file_path" string, split on commas, so a path
  # holding a comma or an escaped quote is skipped rather than misread.
  file=$(printf '%s' "$input" | tr ',' '\n' | sed -n 's/.*"file_path" *: *"\([^"\\]*\)".*/\1/p' | head -n 1)
fi
case $file in
/*.rs) ;;
*) exit 0 ;;
esac

# The file's directory exists once Edit or Write has run; resolved, it must
# be the project or below it.
dir=$(cd "$(dirname "$file")" 2>/dev/null && pwd -P) || exit 0
case $dir/ in
"$project"/*) ;;
*) exit 0 ;;
esac

while [ "$dir" != "$project" ] && [ "$dir" != "/" ] && [ ! -f "$dir/layers.toml" ]; do
  dir=$(dirname "$dir")
done
[ -f "$dir/layers.toml" ] && [ -d "$dir/xtask" ] || exit 0

cd "$dir" || exit 0
out=$(
  cargo xtask budgets 2>&1
  cargo xtask arch 2>&1
)
if printf '%s' "$out" | grep -q 'finding(s)'; then
  printf 'rust-code gates after editing %s:\n%s\n' "$file" "$(printf '%s' "$out" | grep -E '^\[|finding')" >&2
  exit 2
fi
exit 0
