#!/bin/sh
# After Claude edits a .rs file, run the project's fast source gates and show
# Claude what they found, so a violation is fixed in the same turn instead of
# at `devenv test`. Off unless the user turned it on (the plugin's
# `gate_on_edit` option), and a no-op outside a project with rust-template's
# gates. Exit 2 shows stderr to Claude; it cannot undo the edit.
set -u

[ "${CLAUDE_PLUGIN_OPTION_GATE_ON_EDIT:-false}" = "true" ] || exit 0

input=$(cat)
file=$(printf '%s' "$input" | tr ',' '\n' | sed -n 's/.*"file_path" *: *"\([^"]*\)".*/\1/p' | head -n 1)
case $file in
*.rs) ;;
*) exit 0 ;;
esac

dir=$(dirname "$file")
while [ "$dir" != "/" ] && [ ! -f "$dir/layers.toml" ]; do
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
