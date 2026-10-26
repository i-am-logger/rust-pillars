//! Adler-32 checksums, as RFC 1950 (zlib) defines them (role: `domain`).
//!
//! An [`Adler32`] is a running checksum: feed it [`Bytes`] as they arrive and
//! [`finish`](Adler32::finish) it into a [`Checksum`]. [`read_all`] does the
//! whole thing for anything that implements [`Read`].

use std::fmt;
use std::io::Read;

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
    pub fn new() -> Self {
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
    pub fn finish(&self) -> Checksum {
        Checksum((self.b << 16) | self.a)
    }
}

/// Bytes to checksum, borrowed from the host.
#[derive(Clone, Copy, Debug)]
pub struct Bytes<'a>(&'a [u8]);

impl<'a> Bytes<'a> {
    /// The seam in: a host's bytes.
    #[must_use]
    pub fn of(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }
}

/// An Adler-32 value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Checksum(u32);

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
    pub fn get(self) -> u32 {
        self.0
    }

    /// Parses eight hex digits, as [`Display`](fmt::Display) prints them.
    ///
    /// # Errors
    ///
    /// A message saying why `text` is not a checksum.
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() != 8 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("`{text}` is not eight hex digits"));
        }
        u32::from_str_radix(text, 16)
            .map(Self)
            .map_err(|e| format!("`{text}`: {e}"))
    }
}

impl fmt::Display for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08x}", self.0)
    }
}

/// Checksums everything `reader` yields, to its end.
///
/// # Errors
///
/// The reader's error, as text.
pub fn read_all<R: Read>(mut reader: R) -> Result<Checksum, String> {
    let mut all = Vec::new();
    reader
        .read_to_end(&mut all)
        .map_err(|e| format!("reading: {e}"))?;
    Ok(Checksum::of(&all))
}
