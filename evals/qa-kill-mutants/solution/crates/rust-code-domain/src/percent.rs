//! Progress as a whole percentage, for a host that draws a bar.

use crate::BytePos;

/// A whole percentage, from 0 to 100 inclusive, rounded down.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Percent(pub(crate) u8);

impl Percent {
    /// Nothing done yet.
    pub const ZERO: Self = Self(0);

    /// All of it.
    pub const FULL: Self = Self(100);

    /// How far `done` is through a stream of `total` bytes, rounded down.
    /// Clamped at [`Percent::FULL`]: a position past the end, or any position
    /// in an empty stream, is complete.
    #[must_use]
    pub fn through(done: BytePos, total: BytePos) -> Self {
        if done.0 >= total.0 {
            return Self::FULL;
        }
        // `done < total` here, so the quotient is below 100 and `total` is
        // not zero; `u128` keeps `done * 100` from overflowing.
        let whole = u128::from(done.0) * 100 / u128::from(total.0);
        Self(u8::try_from(whole).unwrap_or(100))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_position_past_the_end_is_complete() {
        assert_eq!(Percent::through(BytePos(200), BytePos(100)), Percent::FULL);
    }

    #[test]
    fn the_end_of_the_stream_is_complete() {
        assert_eq!(Percent::through(BytePos(100), BytePos(100)), Percent::FULL);
    }

    #[test]
    fn an_empty_stream_is_complete() {
        assert_eq!(Percent::through(BytePos(0), BytePos(0)), Percent::FULL);
        assert_eq!(Percent::through(BytePos(7), BytePos(0)), Percent::FULL);
    }

    #[test]
    fn the_start_of_a_stream_is_zero() {
        assert_eq!(Percent::through(BytePos(0), BytePos(100)), Percent::ZERO);
    }

    #[test]
    fn a_position_short_of_the_end_is_its_share_rounded_down() {
        assert_eq!(Percent::through(BytePos(50), BytePos(100)), Percent(50));
        assert_eq!(Percent::through(BytePos(99), BytePos(100)), Percent(99));
        assert_eq!(Percent::through(BytePos(1), BytePos(3)), Percent(33));
        assert_eq!(Percent::through(BytePos(2), BytePos(3)), Percent(66));
        assert_eq!(Percent::through(BytePos(1), BytePos(200)), Percent(0));
        assert_eq!(Percent::through(BytePos(3), BytePos(4)), Percent(75));
    }

    #[test]
    fn positions_near_the_top_of_the_range_do_not_overflow() {
        assert_eq!(
            Percent::through(BytePos(u64::MAX - 1), BytePos(u64::MAX)),
            Percent(99)
        );
        assert_eq!(
            Percent::through(BytePos(u64::MAX / 2), BytePos(u64::MAX)),
            Percent(49)
        );
    }

    /// Every `done` from 0 to past the end of every `total` up to 120, checked
    /// against the definition of a whole percentage rounded down: the largest
    /// `p` with `p * total <= done * 100`, found by counting up, not dividing.
    #[test]
    fn every_small_stream_agrees_with_the_definition() {
        let mut checked = 0_u32;
        for total in 0..=120_u64 {
            for done in 0..=total + 2 {
                let expected = if done >= total {
                    100
                } else {
                    let mut p = 0_u8;
                    while (u64::from(p) + 1) * total <= done * 100 {
                        p += 1;
                    }
                    p
                };
                assert_eq!(
                    Percent::through(BytePos(done), BytePos(total)),
                    Percent(expected),
                    "{done} of {total}"
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 7623);
    }
}
