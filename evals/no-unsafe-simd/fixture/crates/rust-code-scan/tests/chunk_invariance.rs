//! Pillar 2's property for a scan: where the first newline is does not depend
//! on how the input was split into reads.

use proptest::prelude::*;
use rust_code_scan::first_newline;

const ALPHABET: &[u8] = b"ab \n";

proptest! {
    #[test]
    fn chunk_invariance_first_newline(
        bytes in prop::collection::vec(prop::sample::select(ALPHABET), 0..16),
        at in 0usize..16,
    ) {
        let (head, tail) = bytes.split_at(at.min(bytes.len()));
        let split = first_newline(head).or_else(|| first_newline(tail).map(|i| head.len() + i));
        prop_assert_eq!(first_newline(&bytes), split);
    }
}
