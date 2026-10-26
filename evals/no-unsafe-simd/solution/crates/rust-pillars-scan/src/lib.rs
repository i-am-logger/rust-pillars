//! Fast byte scans for the reader (role: `core`).
//!
//! The reader spends most of its time looking for the end of the current
//! line, so [`first_newline`] tests 16 bytes at a time in one `u128` word
//! (SWAR: SIMD within a register) and falls back to a byte loop only for the
//! tail of fewer than 16 bytes. It is plain integer arithmetic: no `unsafe`,
//! no intrinsics, the same code on every target, `no_std`.

#![no_std]

/// A 16-byte block, read as one little-endian word.
type Block = u128;

/// `0x01` in every byte of a block.
const LOW_BITS: Block = Block::from_ne_bytes([0x01; 16]);
/// `0x80` in every byte of a block.
const HIGH_BITS: Block = Block::from_ne_bytes([0x80; 16]);
/// `\n` in every byte of a block.
const NEWLINES: Block = Block::from_ne_bytes([b'\n'; 16]);

/// The index of the first `\n` in `haystack`, or `None` if it has none.
#[must_use]
pub fn first_newline(haystack: &[u8]) -> Option<usize> {
    let mut rest = haystack;
    let mut at = 0;
    while let Some((block, tail)) = rest.split_first_chunk::<16>() {
        let marks = newline_marks(Block::from_le_bytes(*block));
        if marks != 0 {
            return lowest_marked_byte(marks).map(|i| at + i);
        }
        rest = tail;
        at += 16;
    }
    rest.iter().position(|&b| b == b'\n').map(|i| at + i)
}

/// The high bit of every byte of `word` that may be a `\n`.
///
/// XOR turns each `\n` into a zero byte, and `(x - 0x01..) & !x & 0x80..`
/// sets the high bit of every zero byte. A byte without its high bit set is
/// never a newline, and the lowest marked byte always is one: the borrow
/// that can mark a non-newline byte only travels upwards from a byte that
/// was a newline, so a false mark always sits above a true one. With the
/// word read little-endian, "lowest" is "first in the buffer".
const fn newline_marks(word: Block) -> Block {
    let x = word ^ NEWLINES;
    x.wrapping_sub(LOW_BITS) & !x & HIGH_BITS
}

/// The index, within its block, of the lowest byte whose high bit `marks`
/// sets.
fn lowest_marked_byte(marks: Block) -> Option<usize> {
    usize::try_from(marks.trailing_zeros() / 8).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_first_newline_of_a_short_line() {
        assert_eq!(first_newline(b"ab\ncd\n"), Some(2));
    }

    #[test]
    fn a_line_without_a_newline_has_none() {
        assert_eq!(first_newline(b"abc"), None);
        assert_eq!(first_newline(b""), None);
    }

    #[test]
    fn a_false_mark_above_a_newline_does_not_move_it() {
        // 0x0b ^ 0x0a is 0x01: the borrow out of the newline below it marks
        // it too, and the newline must still win.
        let mut haystack = [b'a'; 32];
        haystack[20] = b'\n';
        haystack[21] = 0x0b;
        assert_eq!(first_newline(&haystack), Some(20));
    }
}
