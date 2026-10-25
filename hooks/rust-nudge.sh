#!/bin/sh
# When a prompt is about Rust -- it mentions Rust, cargo, a crate, a .rs file,
# the borrow checker, lifetimes, clippy or rustfmt, or the project has a
# Cargo.toml -- add one line telling Claude to load the rust-code skill. A
# skill's description alone leaves loading to the model, which skips it on
# small tasks (measured: Sonnet wrote a whole crate without loading any skill).
# Reads nothing but the prompt and the project's root; runs nothing.
set -u
[ "${CLAUDE_PLUGIN_OPTION_NUDGE:-true}" = "false" ] && exit 0

prompt=$(cat)
root=${CLAUDE_PROJECT_DIR:-$PWD}
if printf '%s' "$prompt" | grep -qiE '\brust\b|\bcargo\b|\bcrates?\b|\.rs\b|borrow checker|lifetimes?\b|clippy|rustfmt' ||
  [ -f "$root/Cargo.toml" ]; then
  echo "This is Rust work: load the rust-code skill (rust-code:rust-code) before writing, reviewing or debugging Rust, and the specialist it names for the task."
fi
exit 0
