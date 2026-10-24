# Instruments

What each instrument counts, how to find out before trusting it, and which
inputs a number may come from.

## Rule zero: identify the instrument with control programs

An instrument's documentation tells you what it intends to count; a control
program tells you what it counts on this host, this OS version, this binary.
Before using an unfamiliar profiler or counter, run it on programs whose
answer you know:

| Control | Known answer | Tells you |
|---|---|---|
| `sleeps`: blocks in `read`/`sleep` for 1 s | ~0 on-CPU time, ~1 s wall | whether blocked time is counted |
| `spins`: busy loop for 1 s | ~1 s on-CPU | the on-CPU baseline |
| `fixed_ops`: a loop of exactly N iterations of a known body, `black_box`ed | instructions ~ k*N | which column is instructions, and its overhead |
| `strided`: walks an array larger than L2 at line stride | misses ~ len / line | which column is cache misses |

Keep the control programs in the repo (a tool-role crate or `examples/`), so
the identification is repeatable after an OS or tool update.

## Sampling vs on-CPU profilers

- A **sampling** profiler that samples thread stacks on a timer (macOS
  `sample`, and similar wall-clock samplers) records a thread **blocked** in a
  syscall as much as one running. It answers "where is the wall time".
- An **on-CPU** profiler (`dtrace -n 'profile-997 ...'`, and CPU-cycle
  sampling in general) only records threads that are running. It answers
  "where is the CPU time".
- On a program that spends its time blocked in `read`, the two disagree about
  the same run, and both are right: they count different things. A conclusion from one
  without knowing which kind it is is not a conclusion.
- On Linux, `perf record` defaults to on-CPU cycle sampling; off-CPU time
  needs a separate mode. Confirm with the `sleeps` control rather than from
  memory of a flag.

## Hardware counters

- Use them only after control programs have identified each column (macOS:
  `xctrace record --template 'CPU Counters'`; Linux: `perf stat -e ...`).
  Counter names and event mappings differ between CPU generations and tool
  versions.
- **Never assign a counter from its magnitude** ("this column is about the
  size I expect cache misses to be, so it is cache misses"). An
  identification from magnitude alone is a guess: a control program that
  exercises the event decides what a column counts.
- Counters are per-run evidence, like timings: same host, both arms, A/A arm.
- For CI, take instruction counts from an instruction-counting simulator,
  not from the PMU (ci-counters.md): a simulator needs no hardware counter
  access and does not vary with frequency or neighbours (inferred).

## Inputs: real recordings and ceiling arms only

- **Never derive a number from an edited recording.** Deleting the "expensive"
  part of an input changes its distribution (run lengths, branch patterns,
  cache footprint), so the remainder no longer runs like the original. The
  number describes an input nobody sends.
- To ask "what would this cost if X were free", keep the real recording and
  build a **ceiling arm** that removes X (ab-protocol.md section 5), checked
  by an invariant counter.
- Recordings are real program output, captured once, committed with a README
  (what produced it, how, when) and a sha256 that a test or gate re-hashes.
- Synthetic inputs are fine for unit tests and counters; a timing row on a
  synthetic input says "synthetic" in the row.

## Placement noise

- Function and loop alignment move with unrelated edits: adding cold code,
  renaming a path, changing a string. A hot loop that straddles a fetch or
  cache-line boundary differently can move several percent.
- Bytes that nothing reads still move what follows them: adding them can
  shift a hot function and its timing.
- Controls: build every arm with the same
  `-C llvm-args=-align-all-functions=6`; check that an "A rebuilt" arm is
  byte-identical to A (if not, the build itself moves code); when a cold-only
  change moves a hot timing, suspect placement before believing it.
- The same lesson applies to generics: monomorphising a hot `impl` over a new
  type parameter emits a new copy of hot code at a new address. Reserve
  generics for cold types, or measure (rust-architecture covers the shapes).

## Prefetch and other "hints"

- Software prefetch is one more load, competing with the demand loads, not
  free advice. Treat any "hint" (prefetch, `#[cold]`,
  `likely`-style branches, alignment attributes) as a change to measure.

## Wording claims

Every number in a note, comment, commit message or PR carries its label:

| Label | Means |
|---|---|
| measured | an A/B row or counter output exists; cite the row |
| inferred | follows from measured facts by stated reasoning; not itself measured |
| by-reading | read from code or docs; nothing was run |
| unmeasured | not known; do not estimate |

- Write **"unmeasured"** instead of an estimate. An estimate in a document is
  read later as a measurement.
- Sweep for expired claims: a claim labelled unmeasured is out of date once a
  row measures it -- update it in the same commit as the row.
- Example of labelling: "owned storage is speed-neutral if hot structs stay
  byte-identical" is **inferred** until an arm measures it, while a placement
  cost read from an A/B row is **measured**. Keep the two apart.
