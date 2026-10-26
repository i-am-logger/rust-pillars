# Measurements

Current numbers only; superseded ones live in git history. Every row names
the commit that first carried it, the host, the power state, the statistic
and the repetitions, and its A/A spread. **If two ranges overlap there is no
result.** A change under its row's floor is reverted, and the row is still
recorded.

Produced by `cargo xtask bench` (pillar M, the `rust-measure` skill).

### wall ms on a 1 MiB input -- default `--chunk` 64 KiB (A) vs 4 KiB (B)

Measured. Run: commit none (the bench printed an empty commit: the working
copy is not a git repository; both arms are the same tree, differing only in
`default_value_t` of `--chunk` in `crates/rust-pillars/src/main.rs`, 64 * 1024
vs 4 * 1024); host Darwin 27.0.0 arm64 (Apple M5 Max, Mac17,6), as the bench
printed it; toolchain rustc 1.94.1 (e408947bf 2026-03-25), `--release` (lto,
codegen-units=1); power AC (as the bench printed it; battery 100%, charged,
pmset powermode 0, low power off); median [min-max] of 6 (7 rounds, first
dropped, order rotated), `cargo xtask bench --arm chunk64k=... --arm
chunk4k=... --aa --stdin <input> --floor-ms 0.5`, each arm on its own
default (no `--chunk` argument). Arms built one after the other at one path
(`target/release`), then copied aside; `--help` shows `default: 65536` and
`default: 4096`, and `otool -t -X` text differs between them, so timed (by
hand: `cargo xtask arms` needs git). Floor 0.5 ms, chosen before the run.
Falsifier, written before the run: "4 KiB is faster" is wrong if the 4 KiB
wall range overlaps the 64 KiB one, or is disjoint but under the floor, or
disjoint and slower.

Input: synthetic, `yes '<99-byte line>' | head -c 1048576` (1048576 bytes,
10485 newlines: 10486 lines, 100 bytes each except a final 76-byte line
without a newline), sha256 466a87be. Work counters come from one untimed
run of each binary on the same input under `RUST_LOG=warn` (lines from
`wc -l`; refused 0 = no `refused` warning), not from the bench.

| arm | median ms | range ms | work counters | verdict |
|---|---|---|---|---|
| A: chunk64k (64 KiB) | 3.15 | 2.91..3.38 | 10486 lines out, refused 0 | |
| AA: chunk64k (A/A) | 3.35 | 3.04..3.68 | 10486 lines out, refused 0 | A/A: overlap: no result (+0.20 ms) |
| B: chunk4k (4 KiB) | 3.26 | 3.21..3.39 | 10486 lines out, refused 0 | **no result** -- overlap: no result (+0.11 ms), floor 0.5 ms |

Verdict, as the bench printed it: `chunk4k against chunk64k: overlap: no
result (+0.11 ms)`. The 4 KiB range overlaps the 64 KiB range, and the
+0.11 ms median delta is smaller than the run's own A/A delta (+0.20 ms, A/A
range 3.04..3.68 ms against A 2.91..3.38 ms), so a 4 KiB default is not
shown to be faster on this input; the default stays 64 KiB.
Stages (same run, not the verdict): `first read` 0.13 [0.13-0.14] ms on A,
0.05 [0.05-0.06] on B, 0.19 [0.13-0.20] on A/A; `end of input` 1.44
[1.43-1.48] on A, 1.55 [1.52-1.78] on B, 1.58 [1.43-1.87] on A/A.
Outputs compared with `cmp`: byte-identical on both arms.

Work counters on short lines (measured, no timing): on a 1 MiB input of
44-byte lines (`yes 'the quick brown fox jumps over the lazy dog' | head -c
1048576`, sha256 d05bf128), both arms write all 23832 lines, log no
`refused` warning under `RUST_LOG=warn`, and their outputs are
byte-identical: the arms do the same work, so the 100-byte-line row above
compares like with like.
