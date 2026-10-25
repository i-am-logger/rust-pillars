//! The core's whole vocabulary: events in, effects out.

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use crate::{ByteLen, BytePos, Millis, Wire, WordCount};

/// Everything that can happen to the core. The host turns the outside world --
/// a read, the clock, the end of input, a
/// failed upstream -- into one of these (pillar 2).
#[derive(Clone, Copy, Debug)]
pub enum Event<'a> {
    /// A read's bytes, borrowed.
    Bytes(Wire<'a>),
    /// The host's clock, read by the host.
    Tick(Millis),
    /// No more input will come.
    End,
    /// The host's upstream connection just failed. It carries no time of its
    /// own: the core counts the backoff from the last [`Event::Tick`].
    UpstreamFailed,
}

/// Everything the core can owe the host. Queued, and polled by the host --
/// never a callback.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effect {
    /// A line ended.
    Line(LineRecord),
    /// No input arrived for the configured threshold, as of this time.
    Idle(Millis),
    /// Try reconnecting the upstream now: the backoff after the last failure
    /// ran out on the tick at this time.
    Retry(Millis),
}

/// One line's summary, as plain old data: 16 bytes, no padding, valid for
/// every bit pattern, so a host can write it anywhere as bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, FromBytes, IntoBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct LineRecord {
    /// Where the line starts in the stream.
    pub start: BytePos,
    /// Its length in bytes, newline excluded.
    pub len: ByteLen,
    /// How many words it holds.
    pub words: WordCount,
}

// The layout is part of the contract with hosts that read it as bytes.
const _: () = assert!(core::mem::size_of::<LineRecord>() == 16);
