//! `first_newline` agrees with a plain byte-by-byte search: the search is the
//! specification, and the word-at-a-time scan is an optimisation of it.

use proptest::prelude::*;
use rust_code_scan::first_newline;

/// The specification: obviously right, and slow.
fn byte_by_byte(haystack: &[u8]) -> Option<usize> {
    haystack.iter().position(|&b| b == b'\n')
}

/// Every length up to four 16-byte blocks, so newlines land in the first and
/// later blocks, on both sides of each block boundary, and in the byte tail;
/// every newline position, and no newline at all; and, around it, every one
/// of the 255 other byte values in every position.
#[test]
fn agrees_with_a_byte_search_on_every_length_position_and_byte() {
    let others: Vec<u8> = (0..=u8::MAX).filter(|&b| b != b'\n').collect();
    let mut checked = 0_usize;
    for len in 0..=64 {
        // `newline == len` is the haystack with no newline in it.
        for newline in 0..=len {
            for shift in 0..others.len() {
                let mut haystack: Vec<u8> = (0..len)
                    .map(|i| others[(i + shift) % others.len()])
                    .collect();
                if let Some(b) = haystack.get_mut(newline) {
                    *b = b'\n';
                }
                assert_eq!(
                    first_newline(&haystack),
                    byte_by_byte(&haystack),
                    "{haystack:02x?}"
                );
                checked += 1;
            }
        }
    }
    // 255 fillers for each of the 1 + 2 + ... + 65 (length, position) pairs.
    assert_eq!(
        checked,
        255 * 65 * 66 / 2,
        "the sweep did not cover every input"
    );
}

/// Bytes a word-at-a-time scan can confuse with `\n`: `\n` itself, its
/// neighbours, the byte with only the high bit flipped, zero, and `\r`.
const NEAR_MISSES: &[u8] = b"\n\n\x0b\x09\x8a\x00\r\x80\xffa";

proptest! {
    #[test]
    fn agrees_with_a_byte_search_on_arbitrary_bytes(
        bytes in prop::collection::vec(any::<u8>(), 0..300),
        start in 0usize..16,
    ) {
        // Start at every offset, so no alignment is assumed.
        let haystack = bytes.get(start..).unwrap_or_default();
        prop_assert_eq!(first_newline(haystack), byte_by_byte(haystack));
    }

    #[test]
    fn agrees_with_a_byte_search_among_near_misses(
        bytes in prop::collection::vec(prop::sample::select(NEAR_MISSES), 0..100),
    ) {
        prop_assert_eq!(first_newline(&bytes), byte_by_byte(&bytes));
    }
}
