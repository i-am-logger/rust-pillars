# CI gates: deterministic work counters

Shared CI cannot time reliably: the host changes between runs, neighbours
share the CPU, power and frequency policy are not yours, and there is no A/A
arm on the same machine in the same minute. A timing gate on CI is either so
loose it catches nothing or so tight it fails at random -- and a gate that
fails at random gets ignored. So CI gates on **counts of work** --
allocation, byte and size counts are the same on every host; instruction
counts need a pinned runner class (below) -- and timings stay local under the
A/B protocol.

## The counters

| Counter | What it catches | Status in rust-template |
|---|---|---|
| allocations per event | a hidden `Vec` growth, a `format!`, a clone on the hot path | **shipped**: `crates/<name>-core/tests/alloc.rs` |
| bytes written per byte in | write amplification (one of the two invariants) | planned |
| instructions per op | algorithmic regressions too small to time | planned, optional profile |
| binary and `__TEXT` size | code bloat, monomorphisation blow-up, an accidental dependency | planned ratchet |
| embedded artefact sizes | a regenerated blob that grew | planned ratchet |

Each is ratcheted like `cargo xtask budgets`: **budget = count**. A count above
budget is a regression; a count below budget fails too, because a budget above
the count measures nothing. Lower the budget in the commit that lowers the
count; raise it only in a commit of its own that says why.

## Allocations per event (shipped)

`alloc.rs` installs `dhat::Alloc` as the global allocator (safe, so no
`unsafe impl GlobalAlloc` -- pillar 6), builds the machine **before** the
profiler starts (setup may allocate, the hot path may not), drives 1000 rounds
of ticks and 7-byte chunks, and asserts `total_blocks` and `curr_bytes` did not
change. Zero allocations is the budget, and also "nothing grows" over a long
run. dhat allows one profiler per process, so each such test is its own
integration-test binary.

## Bytes written per byte in (planned; shape)

Count what the output path writes for a pinned input, exactly. In the
template's bin, `drain` takes any `impl Write`, so a counting sink measures it
without touching the core:

Not compiled here: it is a test for the bin crate (planned), and `use super::*` means `crates/<name>/src/main.rs`, where it would compile.

```rust,ignore
// crates/<name>/src/main.rs
#[cfg(test)]
mod tests {
    use super::*;

    /// Counts what it is given and keeps nothing.
    #[derive(Debug, Default)]
    struct Counting {
        bytes: u64,
    }

    impl Write for Counting {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.bytes += u64::try_from(buf.len()).unwrap_or(u64::MAX);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// A work counter, not a timing: five 2-byte lines in, exactly 30 bytes out
    /// (`"<start>\t1\t1\n"`, 6 bytes each, for starts 0, 2, 4, 6, 8).
    #[test]
    fn bytes_out_for_the_pinned_input_are_the_budget() {
        let input = b"a\n".repeat(5);
        let mut m: Machine<EFFECTS> = Machine::new(Elapsed::threshold_ms(1000));
        let mut sink = Counting::default();
        m.handle(Event::Bytes(Wire::of(&input)));
        m.handle(Event::End);
        drain(&mut m, &mut sink).expect("a counting sink cannot fail");
        assert_eq!((input.len(), sink.bytes), (10, 30));
    }
}
```

- The expected count is exact, not a bound: `assert!(out <= 3 * input)` would
  pass a regression from 2.0 to 2.9 bytes per byte.
- For internal write amplification (bytes the core stores per byte ingested),
  the core exposes a counter of its own -- for example a `Tally` of bytes or
  cells written, read by the host like `Refusals` -- and a test pins its value
  on a pinned recording. A counter in core is a value, not a clock, so it
  passes `test:clippy`.

## Instructions per op (planned, optional profile)

- Count instructions under an instruction-counting simulator (Valgrind's
  cachegrind, or a harness built on it), not the hardware PMU: many CI VMs
  expose no PMU, and the simulated count does not move with frequency or
  neighbours. It is stable for a pinned toolchain, runner image and CPU
  class -- code that picks a path by runtime CPU-feature detection counts
  differently on another CPU, so pin the class or the target features
  (inferred; measure the A/A spread of the count before gating on it).
- Pin the toolchain (the template does, via devenv) -- a compiler update
  changes the count; land the new budget in its own commit that says so.
- Run it as an optional profile (Linux only, slower), not in every `devenv
  test`; it still fails on any change from the budget.

## Binary and `__TEXT` size (planned ratchet)

- Measure the release binary and its text section (`size -m` on macOS,
  `size -A` on Linux) per target, and ratchet both.
- A size jump without a code change of that size is usually an accidental
  dependency or a monomorphised hot impl -- both are architecture findings,
  not noise.
- Embedded artefacts (startup-and-embedding.md) get their own size rows, so a
  bigger blob is a reviewed change, not a surprise in the binary total.

## Every counter gate has a positive control

A counter that silently measures nothing (a profiler not installed, a sink
not wired, a test filtered out) looks exactly like a clean pass. Before
trusting a counter gate:

- plant the regression it exists for -- an allocation in `handle`, an extra
  byte per line, a doubled loop -- and see the gate go red for that reason;
- remove the plant and see it green.

The template's gates were checked this way: a planted-violation run turned
28 of 28 gates red for the intended reason (after fixing the `std::` path
hole in `clippy.toml`, where a `std::` path names nothing in a no_std crate);
commit `67e068d` reports 33 of 33.
`cargo xtask controls` (`test:controls`, last in `devenv test`) now repeats
that on every run: one plant per gate in a scratch copy, and a red only
counts if the output names the planted reason. A new counter gate gets a case
there. It has none for `alloc.rs` yet; until it does, plant by hand: core
has no `extern crate alloc`, so add `#[cfg(feature = "alloc")] extern crate
alloc;` to its `lib.rs` and `core::hint::black_box(alloc::vec![0_u8; 1]);`
(same cfg) at the top of `Machine::handle`. `test:unit` runs with
`--all-features`, so `alloc.rs` goes red on `total_blocks` (verified).
Remove the plant.

## What stays local

Rates, latencies and startup times are taken on a controlled host under
ab-protocol.md and recorded in MEASUREMENTS.md. CI may **run** a bench as a
smoke test (it builds, it finishes), but never fails on its timing.
