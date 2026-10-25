//! The stopword list: words the counter treats as noise.
//!
//! Loaded once at startup from `assets/stopwords.txt`, one word per line;
//! blank lines and lines starting with `#` are skipped.

use std::fs;
use std::io;

/// Where the list lives.
pub(crate) const PATH: &str = "crates/rust-code/assets/stopwords.txt";

/// The loaded list.
#[derive(Debug)]
pub(crate) struct Stopwords {
    words: Vec<String>,
}

impl Stopwords {
    /// Reads the list from [`PATH`].
    pub(crate) fn load() -> io::Result<Self> {
        let text = fs::read_to_string(PATH)?;
        let words = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(String::from)
            .collect();
        Ok(Self { words })
    }

    /// How many words the list holds.
    pub(crate) fn len(&self) -> usize {
        self.words.len()
    }
}
