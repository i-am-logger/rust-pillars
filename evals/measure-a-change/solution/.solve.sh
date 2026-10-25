#!/bin/sh
# The reference solution's measurement, performed rather than committed: two
# release builds of the bin at one path, differing only in the default of
# `--chunk`, timed by the repo's own bench with an A/A arm on a 1 MiB input.
# `cargo xtask bench --aa` leaves the A/A copy in target/bench/chunk64k-aa,
# which a check looks for; the log beside it is the run MEASUREMENTS.md
# records. POSIX sh: the harness runs `sh .solve.sh`, with output discarded.
set -eu

src=crates/rust-code/src/main.rs
arms=$PWD/target/arms
out=$PWD/target/bench
mkdir -p "$arms" "$out"
log=$out/chunk.log
: >"$log"

# The 4 KiB arm is the base with one line changed; the base comes back on any
# exit, and is proven to have come back.
cp "$src" "$arms/main.rs.base"
restore() { cp "$arms/main.rs.base" "$src"; }
trap restore EXIT
test "$(grep -c 'default_value_t = 64 \* 1024' "$src")" = 1

cargo build -q --release -p rust-code
cp target/release/rust-code "$arms/chunk64k"
sed 's/default_value_t = 64 \* 1024/default_value_t = 4 * 1024/' "$arms/main.rs.base" >"$arms/main.rs.4k"
test "$(diff "$arms/main.rs.base" "$arms/main.rs.4k" | grep -c '^[<>]')" = 2
cp "$arms/main.rs.4k" "$src"
cargo build -q --release -p rust-code
cp target/release/rust-code "$arms/chunk4k"
restore
cmp "$src" "$EVAL_ROOT/$src"

# The arms really differ, and only where intended.
"$arms/chunk64k" --help | grep -q 'default: 65536'
"$arms/chunk4k" --help | grep -q 'default: 4096'
# (`! cmp` would not stop the script: `set -e` ignores a negated command.)
if cmp -s "$arms/chunk64k" "$arms/chunk4k"; then exit 1; fi
if command -v otool >/dev/null; then
  otool -t -X "$arms/chunk64k" >"$arms/chunk64k.text"
  otool -t -X "$arms/chunk4k" >"$arms/chunk4k.text"
  if cmp -s "$arms/chunk64k.text" "$arms/chunk4k.text"; then
    echo "machine code: text identical" >>"$log"
  else
    echo "machine code: text differs" >>"$log"
  fi
fi

# The input: 1 MiB of 100-byte lines, and 1 MiB of 44-byte lines for the work
# counters (more lines than the core's 1024-effect queue per 64 KiB read).
yes 'the input for the chunk bench: one hundred bytes per line, ninety-nine of text and a newline ......' |
  head -c 1048576 >"$out/input-100"
yes 'the quick brown fox jumps over the lazy dog' | head -c 1048576 >"$out/input-44"
for f in input-100 input-44; do
  echo "$f: $(wc -c <"$out/$f") bytes, $(wc -l <"$out/$f") newlines, $(shasum -a 256 "$out/$f" | cut -c1-8)" >>"$log"
done

# Work counters, untimed: lines out and refused effects per arm per input.
for f in input-100 input-44; do
  for a in chunk64k chunk4k; do
    RUST_LOG=warn "$arms/$a" <"$out/$f" >"$arms/$a.$f.out" 2>"$arms/$a.$f.err"
    echo "$a on $f: $(wc -l <"$arms/$a.$f.out") lines out, refused lines on stderr: $(grep -c 'refused' "$arms/$a.$f.err" || true)" >>"$log"
  done
  if cmp -s "$arms/chunk64k.$f.out" "$arms/chunk4k.$f.out"; then
    echo "$f: outputs byte-identical" >>"$log"
  else
    echo "$f: outputs differ" >>"$log"
  fi
done

{
  pmset -g batt 2>/dev/null | head -2
  pmset -g 2>/dev/null | grep -E 'powermode|lowpowermode'
} >>"$log" || true
rustc -V >>"$log"

# The measurement. Each arm on its own default: no `--chunk` argument.
cargo xtask bench --arm "chunk64k=$arms/chunk64k" --arm "chunk4k=$arms/chunk4k" \
  --aa --stdin "$out/input-100" --floor-ms 0.5 >"$arms/bench.out"
cat "$arms/bench.out" >>"$log"
test -x "$out/chunk64k-aa"
