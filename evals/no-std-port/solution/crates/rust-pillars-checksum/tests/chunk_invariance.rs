//! Feeding the input in pieces gives the same checksum as feeding it whole,
//! and both agree with RFC 1950's definition.
//!
//! The model below is the definition itself, reduced after every byte in
//! `u64`, so it shares neither the hasher's deferred (`NMAX`) reduction nor
//! its block boundaries: a defect in either cannot pass both.

use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use rust_pillars_checksum::{Adler32, Bytes, Checksum};

/// RFC 1950's modulus, written out again so the model does not borrow it.
const MOD: u64 = 65_521;

/// More than three of the hasher's 5552-byte blocks, so splits land inside,
/// on, and across the boundaries where it reduces.
const LONGEST: usize = 20_000;

/// Adler-32 by its definition: A = 1 + sum of bytes, B = sum of the A's,
/// both mod 65521, reduced at every step.
fn model(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1_u64, 0_u64);
    for &byte in bytes {
        a = (a + u64::from(byte)) % MOD;
        b = (b + a) % MOD;
    }
    u32::try_from((b << 16) | a).expect("both halves are below 2^16")
}

/// Feeds `pieces` one at a time to one hasher.
fn in_pieces(pieces: &[&[u8]]) -> Checksum {
    let mut sum = Adler32::new();
    for p in pieces {
        sum.update(Bytes::of(p));
    }
    sum.finish()
}

/// `bytes` cut at `at` (sorted, each at most `bytes.len()`).
fn cut<'a>(bytes: &'a [u8], at: &[usize]) -> Vec<&'a [u8]> {
    let mut pieces = Vec::with_capacity(at.len() + 1);
    let mut prev = 0;
    for &c in at {
        pieces.push(&bytes[prev..c]);
        prev = c;
    }
    pieces.push(&bytes[prev..]);
    pieces
}

/// Every byte value, alone and split out of the middle of a run of 0xff long
/// enough to need reducing: the finite domain swept whole, and the sweep
/// counted so one that silently ran short is not a pass.
#[test]
fn chunk_invariance_every_byte_value() {
    let run = [0xff_u8; 6000];
    let mut checked = 0;
    for value in 0..=u8::MAX {
        assert_eq!(
            Checksum::of(&[value]).get(),
            model(&[value]),
            "{value:#04x}"
        );

        let mut whole = run.to_vec();
        whole.insert(run.len() / 2, value);
        let (before, rest) = whole.split_at(run.len() / 2);
        let (byte, after) = rest.split_at(1);
        let expected = model(&whole);
        assert_eq!(Checksum::of(&whole).get(), expected, "{value:#04x} whole");
        assert_eq!(
            in_pieces(&[before, byte, after]).get(),
            expected,
            "{value:#04x} in three pieces"
        );
        checked += 1;
    }
    assert_eq!(checked, 256);
}

/// Any bytes around a run of 0xff, the value that overflows an unreduced sum
/// soonest, long enough to cross several of the hasher's blocks. Built from
/// parts so a 20 000-byte case costs three draws, not 20 000.
fn input() -> impl Strategy<Value = Vec<u8>> {
    let edge = || prop::collection::vec(any::<u8>(), 0..256);
    (edge(), 0..LONGEST, edge()).prop_map(|(mut bytes, run, after)| {
        bytes.resize(bytes.len() + run, 0xff);
        bytes.extend(after);
        bytes
    })
}

proptest! {
    // Failures persist beside this file (`chunk_invariance.regressions`), to be
    // committed: an integration test has no lib.rs for proptest's default
    // location.
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::WithSource("regressions"))),
        ..ProptestConfig::default()
    })]

    #[test]
    fn chunk_invariance_any_split_gives_the_same_checksum(
        bytes in input(),
        cuts in prop::collection::vec(any::<prop::sample::Index>(), 0..16),
    ) {
        let mut at: Vec<usize> = cuts.iter().map(|i| i.index(bytes.len() + 1)).collect();
        at.sort_unstable();
        let whole = Checksum::of(&bytes);
        prop_assert_eq!(whole.get(), model(&bytes));
        prop_assert_eq!(in_pieces(&cut(&bytes, &at)), whole);
    }

    #[test]
    fn chunk_invariance_one_byte_at_a_time(bytes in input()) {
        let singles: Vec<&[u8]> = bytes.chunks(1).collect();
        prop_assert_eq!(in_pieces(&singles).get(), model(&bytes));
    }
}
