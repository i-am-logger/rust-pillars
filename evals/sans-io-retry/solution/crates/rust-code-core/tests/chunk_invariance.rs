//! Pillar 2's property: the effects of an input do not depend on how the host
//! split it into reads, and they agree with an independent model -- for the
//! bytes alone, and for a session that interleaves them with ticks and
//! upstream failures, where the retries agree with a model of the backoff.
//!
//! `cargo xtask arch` requires a test named `chunk_invariance_*` in every core
//! crate. The model below is written against `std`'s own line and word
//! splitting, not against the machine, so a defect shared by the machine and
//! its test cannot pass both.

use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use rust_code_core::{Effect, Elapsed, Event, LineRecord, Machine, Millis, Wire};

/// Large enough that no generated input fills it: a dropped effect would make
/// the comparison vacuous, and `refused()` is asserted empty below.
const CAP: usize = 4096;

/// The bytes the generator draws from. `0x0b` is left out because `std`'s
/// `split_ascii_whitespace` does not treat it as a separator while the
/// machine does; that difference is tested by hand in the domain.
const ALPHABET: &[u8] = b"ab \t\r\n";

fn run(chunks: &[&[u8]]) -> Vec<LineRecord> {
    let mut m: Box<Machine<CAP>> = Box::new(Machine::new(Elapsed::threshold_ms(1)));
    for c in chunks {
        m.handle(Event::Bytes(Wire::of(c)));
    }
    m.handle(Event::End);
    let r = m.refused();
    assert_eq!(
        (r.bytes.get(), r.effects.get()),
        (0, 0),
        "nothing may be refused"
    );
    m.effects()
        .iter()
        .map(|e| match e {
            Effect::Line(l) => *l,
            Effect::Idle(_) | Effect::Retry(_) => {
                panic!("no tick or failure was sent, so no idle or retry can be owed")
            }
        })
        .collect()
}

/// The independent model: (start, len, words) per line, from `std`.
fn model(input: &[u8]) -> Vec<(u64, u32, u32)> {
    let mut out = Vec::new();
    let mut start = 0_u64;
    let pieces: Vec<&[u8]> = input.split(|&b| b == b'\n').collect();
    let last = pieces.len() - 1;
    for (i, line) in pieces.iter().enumerate() {
        // A final piece with no newline after it is a line only if non-empty.
        if i == last && line.is_empty() {
            break;
        }
        let text = core::str::from_utf8(line).expect("the alphabet is ASCII");
        let words = text.split_ascii_whitespace().count();
        out.push((
            start,
            u32::try_from(line.len()).expect("short"),
            u32::try_from(words).expect("short"),
        ));
        start += u64::try_from(line.len()).expect("short") + 1;
    }
    out
}

fn triples(v: &[LineRecord]) -> Vec<(u64, u32, u32)> {
    v.iter()
        .map(|l| (l.start.get(), l.len.get(), l.words.get()))
        .collect()
}

fn input() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(prop::sample::select(ALPHABET), 0..512)
}

/// `bytes` cut into reads at the given points, in order; empty reads included.
fn split<'a>(bytes: &'a [u8], cuts: &[prop::sample::Index]) -> Vec<&'a [u8]> {
    let mut at: Vec<usize> = cuts.iter().map(|i| i.index(bytes.len() + 1)).collect();
    at.sort_unstable();
    let mut chunks = Vec::new();
    let mut prev = 0;
    for &c in &at {
        chunks.push(&bytes[prev..c]);
        prev = c;
    }
    chunks.push(&bytes[prev..]);
    chunks
}

/// One step of a session with a flaky upstream.
#[derive(Clone, Debug)]
enum Step {
    /// Bytes from the host, and where a split run cuts them into reads.
    Bytes(Vec<u8>, Vec<prop::sample::Index>),
    /// The clock moves on this many milliseconds, and the host ticks.
    Tick(u64),
    /// The upstream fails.
    Fail,
}

/// How a session's bytes are handed to the machine.
#[derive(Clone, Copy, Debug)]
enum Reads {
    /// Each step's bytes as one read.
    Whole,
    /// Each step's bytes cut where its step says.
    Cut,
    /// One byte per read.
    Singles,
}

/// An idle threshold the generated ticks cross, so idle reports are part of
/// what is compared alongside lines and retries.
const SESSION_IDLE_MS: u64 = 50;

fn session(steps: &[Step], reads: Reads) -> Vec<Effect> {
    let mut m: Box<Machine<CAP>> = Box::new(Machine::new(Elapsed::threshold_ms(SESSION_IDLE_MS)));
    let mut now = 0_u64;
    for step in steps {
        match step {
            Step::Bytes(bytes, cuts) => {
                let chunks = match reads {
                    Reads::Whole => vec![bytes.as_slice()],
                    Reads::Cut => split(bytes, cuts),
                    Reads::Singles => bytes.chunks(1).collect(),
                };
                for c in chunks {
                    m.handle(Event::Bytes(Wire::of(c)));
                }
            }
            Step::Tick(ms) => {
                now += ms;
                m.handle(Event::Tick(Millis::from_host(now)));
            }
            Step::Fail => m.handle(Event::UpstreamFailed),
        }
    }
    m.handle(Event::End);
    let r = m.refused();
    assert_eq!(
        (r.bytes.get(), r.effects.get(), r.after_end.get()),
        (0, 0, 0),
        "nothing may be refused"
    );
    m.effects().to_vec()
}

/// The independent model of the backoff: the times retries fire. Written from
/// the stated contract -- the n-th consecutive failure waits 100 * 2^(n-1) ms,
/// at most 1600, from the last tick; a retry fires on the first tick at or
/// past that deadline; a later failure moves the deadline; non-empty bytes end
/// the streak -- with an absolute deadline and a shift, where the machine keeps
/// an interval that it doubles.
fn retry_model(steps: &[Step]) -> Vec<Millis> {
    let mut now = 0_u64;
    let mut streak = 0_u32;
    let mut deadline = None;
    let mut out = Vec::new();
    for step in steps {
        match step {
            Step::Bytes(bytes, _) if !bytes.is_empty() => streak = 0,
            Step::Bytes(..) => {}
            Step::Tick(ms) => {
                now += ms;
                if deadline.is_some_and(|d| now >= d) {
                    out.push(Millis::from_host(now));
                    deadline = None;
                }
            }
            Step::Fail => {
                streak += 1;
                deadline = Some(now + (100_u64 << (streak - 1).min(4)));
            }
        }
    }
    out
}

fn retries(effects: &[Effect]) -> Vec<Millis> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Retry(at) => Some(*at),
            Effect::Line(_) | Effect::Idle(_) => None,
        })
        .collect()
}

/// Weighted so a script often holds a streak of failures long enough to reach
/// the cap, with ticks far enough apart to tell the capped wait from a longer
/// one; bytes come rarely enough not to end every streak.
fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        1 => (
            prop::collection::vec(prop::sample::select(ALPHABET), 0..48),
            prop::collection::vec(any::<prop::sample::Index>(), 0..4),
        )
            .prop_map(|(bytes, cuts)| Step::Bytes(bytes, cuts)),
        3 => (0_u64..2000).prop_map(Step::Tick),
        2 => Just(Step::Fail),
    ]
}

fn script() -> impl Strategy<Value = Vec<Step>> {
    prop::collection::vec(step(), 0..96)
}

proptest! {
    // Failures persist beside this file (`chunk_invariance.regressions`), to be
    // committed: an integration test has no lib.rs for proptest's default
    // location, and a case worth finding once is worth keeping.
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::WithSource("regressions"))),
        ..ProptestConfig::default()
    })]

    #[test]
    fn chunk_invariance_any_split_gives_the_same_effects(
        bytes in input(),
        cuts in prop::collection::vec(any::<prop::sample::Index>(), 0..8),
    ) {
        let whole = run(&[&bytes]);
        prop_assert_eq!(run(&split(&bytes, &cuts)), whole);
    }

    #[test]
    fn chunk_invariance_one_byte_reads_agree_with_the_model(bytes in input()) {
        let singles: Vec<&[u8]> = bytes.chunks(1).collect();
        prop_assert_eq!(triples(&run(&singles)), model(&bytes));
    }

    #[test]
    fn chunk_invariance_ticks_and_upstream_failures_split_any_way_give_the_same_effects(
        steps in script(),
    ) {
        let whole = session(&steps, Reads::Whole);
        prop_assert_eq!(&session(&steps, Reads::Cut), &whole);
        let singles = session(&steps, Reads::Singles);
        prop_assert_eq!(&singles, &whole);
        prop_assert_eq!(retries(&singles), retry_model(&steps));
    }
}
