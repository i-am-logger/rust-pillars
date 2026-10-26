//! Pillar 4's hot-path guarantee: handling an event allocates nothing, and a
//! long run does not grow. `dhat` is a safe global allocator, so counting
//! costs no `unsafe` here (pillar 6).
//!
//! No libtest harness (`harness = false` in Cargo.toml). dhat counts every
//! thread in the process, and libtest's main thread allocates the first time
//! it blocks waiting for the test thread (std's channel context and waker
//! entry, `library/std/src/sync/mpmc/context.rs` and `waker.rs` at 1.94.1);
//! on a slow runner that lands inside the measured window and fails the test
//! with blocks the hot path never made. Here the test runs on the binary's
//! only thread, so every block counted is the hot path's, a one-time lazy
//! allocation included. `main` answers just enough of libtest's command line
//! for `cargo test` and nextest: `--list`, `--ignored`, and a name filter.
//!
//! One test per binary: dhat allows one profiler per process.

use rust_pillars_core::{Elapsed, Event, Machine, Millis, Wire};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn handling_events_allocates_nothing_and_nothing_grows() {
    let input = b"the quick brown fox\njumps over\n\tthe lazy dog\n".repeat(64);
    // Built before the profiler starts: setup may allocate, the hot path may not.
    let mut m: Box<Machine<64>> = Box::new(Machine::new(Elapsed::threshold_ms(5)));

    let _profiler = dhat::Profiler::builder().testing().build();
    let before = dhat::HeapStats::get();
    for round in 0..1_000_u64 {
        m.handle(Event::Tick(Millis::from_host(round)));
        for chunk in input.chunks(7) {
            m.handle(Event::Bytes(Wire::of(chunk)));
            m.clear_effects();
        }
    }
    let after = dhat::HeapStats::get();

    dhat::assert_eq!(after.total_blocks, before.total_blocks);
    dhat::assert_eq!(after.curr_bytes, before.curr_bytes);
}

const NAME: &str = "handling_events_allocates_nothing_and_nothing_grows";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    if flag("--list") {
        // nextest lists with `--list --format terse`, then `--ignored` for
        // the ignored set, which this binary does not have.
        if !flag("--ignored") {
            println!("{NAME}: test");
        }
        return;
    }
    // Name filters are the positional arguments; a value given to one of
    // libtest's value flags as the next argument (`--test-threads 1`) is not
    // a filter, or it would match nothing and the run would pass empty.
    const VALUED: [&str; 6] = [
        "--test-threads",
        "--format",
        "--color",
        "--logfile",
        "--skip",
        "-Z",
    ];
    let mut filters = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if VALUED.contains(&a.as_str()) {
            it.next();
        } else if !a.starts_with('-') {
            filters.push(a.as_str());
        }
    }
    let exact = flag("--exact");
    let wanted = filters
        .iter()
        .all(|f| if exact { *f == NAME } else { NAME.contains(f) });
    if flag("--ignored") || !wanted {
        return;
    }
    handling_events_allocates_nothing_and_nothing_grows();
    println!("test {NAME} ... ok");
}
