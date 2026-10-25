//! Adler-32 checksums, as RFC 1950 (zlib) defines them (role: `domain`).
//!
//! An [`Adler32`] is a running checksum: feed it [`Bytes`] as they arrive and
//! [`finish`](Adler32::finish) it into a [`Checksum`]. It does no I/O: the host
//! reads (stdin, a file, a socket) in whatever pieces it likes and lends each
//! piece here, and how the input was split does not change the result.
//!
//! `no_std`, no allocator, and no dependency (`layers.toml`).

#![no_std]

use core::fmt;

/// The largest prime below 2^16.
const MOD: u32 = 65_521;

/// The most bytes that can be summed before `b` must be reduced: the largest
/// n with 255 n (n + 1) / 2 + (n + 1) (MOD - 1) below 2^32 (zlib's `NMAX`).
const NMAX: usize = 5552;

/// A running Adler-32 checksum.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Adler32 {
    a: u32,
    b: u32,
}

impl Default for Adler32 {
    fn default() -> Self {
        Self::new()
    }
}

impl Adler32 {
    /// The checksum of no bytes yet.
    #[must_use]
    pub const fn new() -> Self {
        Self { a: 1, b: 0 }
    }

    /// Feeds `bytes`. Feeding a stream in any number of pieces gives the same
    /// checksum as feeding it whole.
    pub fn update(&mut self, bytes: Bytes<'_>) {
        for block in bytes.0.chunks(NMAX) {
            for &byte in block {
                self.a += u32::from(byte);
                self.b += self.a;
            }
            self.a %= MOD;
            self.b %= MOD;
        }
    }

    /// The checksum of everything fed so far.
    #[must_use]
    pub const fn finish(&self) -> Checksum {
        Checksum((self.b << 16) | self.a)
    }
}

/// Bytes to checksum, borrowed from the host.
#[derive(Clone, Copy, Debug)]
pub struct Bytes<'a>(&'a [u8]);

impl<'a> Bytes<'a> {
    /// The seam in: a host's bytes.
    #[must_use]
    pub const fn of(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }
}

/// An Adler-32 value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Checksum(u32);

/// Why text was refused as a [`Checksum`]. The caller has the text, so the
/// error does not carry it (there is no allocator to carry it in).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParseChecksumError {
    /// Not exactly eight bytes long.
    Length,
    /// Eight bytes, but not all of them hex digits.
    NotHex,
}

impl fmt::Display for ParseChecksumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Length => "not eight hex digits: wrong length",
            Self::NotHex => "not eight hex digits: a non-hex character",
        })
    }
}

impl core::error::Error for ParseChecksumError {}

impl Checksum {
    /// The checksum of `bytes`, all at once.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        let mut sum = Adler32::new();
        sum.update(Bytes::of(bytes));
        sum.finish()
    }

    /// The seam out: the value as RFC 1950 writes it.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Parses eight hex digits, as [`Display`](fmt::Display) prints them.
    ///
    /// # Errors
    ///
    /// [`ParseChecksumError::Length`] if `text` is not eight bytes long, and
    /// [`ParseChecksumError::NotHex`] if one of them is not a hex digit (a
    /// sign included).
    pub fn parse(text: &str) -> Result<Self, ParseChecksumError> {
        let digits: &[u8; 8] = text
            .as_bytes()
            .try_into()
            .map_err(|_| ParseChecksumError::Length)?;
        let mut value = 0_u32;
        for &digit in digits {
            let nibble = char::from(digit)
                .to_digit(16)
                .ok_or(ParseChecksumError::NotHex)?;
            value = (value << 4) | nibble;
        }
        Ok(Self(value))
    }
}

impl fmt::Display for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08x}", self.0)
    }
}
