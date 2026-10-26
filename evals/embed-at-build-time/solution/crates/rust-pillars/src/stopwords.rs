//! The stopword list: words the counter treats as noise.
//!
//! The list is `crates/rust-pillars/assets/stopwords.txt`, one word per line;
//! blank lines and lines starting with `#` are skipped. It is embedded in the
//! binary at build time, so the binary needs no file at run time and works
//! from any directory. The asset stays the one copy people edit: the compiler
//! records it as an input of this crate, so an edit to it rebuilds the crate
//! and the next binary carries the new list.

/// The asset's text, embedded at build time. A `static`, not a `const`: one
/// copy in the binary, one address.
static TEXT: &str = include_str!("../assets/stopwords.txt");

/// The parsed list.
#[derive(Debug)]
pub(crate) struct Stopwords {
    words: Vec<&'static str>,
}

impl Stopwords {
    /// Parses the embedded list. Nothing is read at run time, so this cannot
    /// fail.
    pub(crate) fn load() -> Self {
        Self::parse(TEXT)
    }

    /// Parses a list in the asset's format.
    fn parse(text: &'static str) -> Self {
        let words = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        Self { words }
    }

    /// How many words the list holds.
    pub(crate) fn len(&self) -> usize {
        self.words.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_list_is_the_asset() {
        let stop = Stopwords::load();
        // Words the asset has held since it was written; an embed of the
        // wrong file, or of nothing, loses them.
        for w in ["a", "and", "the", "with"] {
            assert!(
                stop.words.contains(&w),
                "{w:?} missing from the embedded list"
            );
        }
        assert!(!stop.words.iter().any(|w| w.starts_with('#')));
    }

    #[test]
    fn comments_and_blank_lines_are_skipped_and_words_trimmed() {
        let stop = Stopwords::parse("# note\n\n  the \n\t\nof\n#of\n");
        assert_eq!(stop.words, ["the", "of"]);
    }
}
