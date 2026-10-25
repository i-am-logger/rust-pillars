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
}
