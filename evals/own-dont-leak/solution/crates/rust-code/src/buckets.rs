//! Length buckets: the label printed after each line's counts.
//!
//! The labels are formatted once at startup into a table indexed by bucket,
//! so printing a line is one index and no formatting.

/// How many bytes wide each bucket is.
const WIDTH: u32 = 8;

/// How many bounded buckets there are; longer lines share one open bucket.
const BOUNDED: u32 = 16;

/// One label per bucket, built once.
#[derive(Debug)]
pub(crate) struct Buckets {
    labels: Vec<String>,
}

impl Buckets {
    /// Formats every label once. The caller owns the table and lends it to
    /// whatever prints a line.
    pub(crate) fn new() -> Self {
        let mut labels: Vec<String> = (0..BOUNDED)
            .map(|b| format!("{}-{}", b * WIDTH, (b + 1) * WIDTH - 1))
            .collect();
        labels.push(format!("{}+", BOUNDED * WIDTH));
        Self { labels }
    }

    /// The label of the bucket a line of `len` bytes falls in.
    pub(crate) fn label(&self, len: u32) -> &str {
        let bucket = usize::try_from((len / WIDTH).min(BOUNDED)).unwrap_or(usize::MAX);
        self.labels.get(bucket).map_or("?", String::as_str)
    }
}
