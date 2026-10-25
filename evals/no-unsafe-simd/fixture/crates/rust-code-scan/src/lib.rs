//! Fast byte scans for the reader (role: `core`).
//!
//! The reader spends most of its time looking for the end of the current
//! line, so [`first_newline`] compares 16 bytes at a time with the platform's
//! vector instructions (SSE2 on `x86_64`, NEON on `aarch64`) and falls back to
//! a byte loop for the tail and on every other target.

#![no_std]

/// The index of the first `\n` in `haystack`, or `None` if it has none.
#[must_use]
pub fn first_newline(haystack: &[u8]) -> Option<usize> {
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    {
        sse2::first_newline(haystack)
    }
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        neon::first_newline(haystack)
    }
    #[cfg(not(any(
        all(target_arch = "x86_64", target_feature = "sse2"),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        scalar(haystack, 0)
    }
}

/// The byte loop: the tail after the last full block, and every target
/// without a vector unit.
fn scalar(haystack: &[u8], from: usize) -> Option<usize> {
    haystack
        .get(from..)?
        .iter()
        .position(|&b| b == b'\n')
        .map(|i| from + i)
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
mod sse2 {
    use core::arch::x86_64::{
        __m128i, _mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8, _mm_set1_epi8,
    };

    pub(crate) fn first_newline(haystack: &[u8]) -> Option<usize> {
        let mut at = 0;
        while at + 16 <= haystack.len() {
            // SAFETY: `at + 16 <= len`, so the 16 bytes read are in bounds;
            // `loadu` has no alignment requirement, and SSE2 is enabled.
            let mask = unsafe {
                let block = _mm_loadu_si128(haystack.as_ptr().add(at).cast::<__m128i>());
                _mm_movemask_epi8(_mm_cmpeq_epi8(block, _mm_set1_epi8(0x0a)))
            };
            if mask != 0 {
                return usize::try_from(mask.trailing_zeros()).ok().map(|i| at + i);
            }
            at += 16;
        }
        super::scalar(haystack, at)
    }
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
mod neon {
    use core::arch::aarch64::{vceqq_u8, vdupq_n_u8, vld1q_u8, vmaxvq_u8};

    pub(crate) fn first_newline(haystack: &[u8]) -> Option<usize> {
        let mut at = 0;
        while at + 16 <= haystack.len() {
            // SAFETY: `at + 16 <= len`, so the 16 bytes read are in bounds,
            // and NEON is enabled.
            let any = unsafe {
                let block = vld1q_u8(haystack.as_ptr().add(at));
                vmaxvq_u8(vceqq_u8(block, vdupq_n_u8(b'\n')))
            };
            if any != 0 {
                return super::scalar(&haystack[..at + 16], at);
            }
            at += 16;
        }
        super::scalar(haystack, at)
    }
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
}
