//! The state machine.

use core::mem;

use rust_pillars_domain::{
    Building, ByteClass, BytePos, Effect, Elapsed, Event, Line, Millis, Pushed, ReadLen, Tally,
    Wire,
};

use crate::effects::{Effects, Queued};

/// What the machine refused, by kind. A refusal is always counted, never
/// silent: the host reads these to know what it lost.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Refusals {
    /// Bytes beyond a line's maximum length.
    pub bytes: Tally,
    /// Effects that found the queue full.
    pub effects: Tally,
    /// Events that arrived after [`Event::End`].
    pub after_end: Tally,
}

impl Refusals {
    const NONE: Self = Self {
        bytes: Tally::ZERO,
        effects: Tally::ZERO,
        after_end: Tally::ZERO,
    };

    /// Every refusal of every kind.
    #[must_use]
    pub const fn total(&self) -> Tally {
        self.bytes
            .saturating_add(self.effects)
            .saturating_add(self.after_end)
    }
}

/// Whether input has ended.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Open,
    Ended,
}

/// Whether bytes arrived since the last tick.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Activity {
    Seen,
    Quiet,
}

/// Whether the current quiet spell has been reported.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Idle {
    Pending,
    Reported,
}

/// Whether a reconnect is owed to the host.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Retry {
    /// None is owed: no failure since the last retry fired.
    Clear,
    /// One is owed on the first tick at least `wait` after `from`.
    Owed { from: Millis, wait: Elapsed },
}

/// The wait after the first failure, and after the first failure following
/// healthy bytes.
const FIRST_BACKOFF: Elapsed = Elapsed::threshold_ms(100);

/// The longest wait: consecutive failures double the backoff up to here.
const MAX_BACKOFF: Elapsed = Elapsed::threshold_ms(1600);

/// Counts lines and words in a byte stream, reports when the stream goes
/// quiet, and paces reconnects to a failing upstream with a doubling backoff.
/// Holds at most `N` unread effects.
#[derive(Debug)]
pub struct Machine<const N: usize> {
    pos: BytePos,
    line: Line<Building>,
    phase: Phase,
    activity: Activity,
    idle: Idle,
    last_input: Millis,
    idle_after: Elapsed,
    /// The most recent tick's time: a failure has none of its own.
    last_tick: Millis,
    /// The wait the next failure will be given.
    backoff: Elapsed,
    retry: Retry,
    effects: Effects<N>,
    refused: Refusals,
}

impl<const N: usize> Machine<N> {
    /// A machine at the start of a stream, reporting a quiet spell of
    /// `idle_after` or longer.
    #[must_use]
    pub const fn new(idle_after: Elapsed) -> Self {
        Self {
            pos: BytePos::ZERO,
            line: Line::starting_at(BytePos::ZERO),
            phase: Phase::Open,
            activity: Activity::Quiet,
            idle: Idle::Pending,
            last_input: Millis::ZERO,
            idle_after,
            last_tick: Millis::ZERO,
            backoff: FIRST_BACKOFF,
            retry: Retry::Clear,
            effects: Effects::new(),
            refused: Refusals::NONE,
        }
    }

    /// Takes one event. Never blocks, never allocates, never fails: what it
    /// cannot keep it refuses and counts in [`Machine::refused`].
    pub fn handle(&mut self, event: Event<'_>) {
        if self.phase == Phase::Ended {
            self.refused.after_end = self.refused.after_end.saturating_incr();
            return;
        }
        match event {
            Event::Bytes(wire) => self.bytes(wire),
            Event::Tick(now) => self.tick(now),
            Event::End => self.end(),
            Event::UpstreamFailed => self.upstream_failed(),
        }
    }

    /// The effects owed since the last [`Machine::clear_effects`], oldest
    /// first. Borrowed: the host reads them where they sit.
    #[must_use]
    pub fn effects(&self) -> &[Effect] {
        self.effects.as_slice()
    }

    /// Forgets the effects the host has read.
    pub fn clear_effects(&mut self) {
        self.effects.clear();
    }

    /// Everything refused so far.
    #[must_use]
    pub const fn refused(&self) -> Refusals {
        self.refused
    }

    fn bytes(&mut self, wire: Wire<'_>) {
        if !wire.is_empty() {
            self.activity = Activity::Seen;
            // The upstream delivered: the next failure starts over.
            self.backoff = FIRST_BACKOFF;
        }
        for class in wire.classes() {
            self.pos = self.pos.saturating_advance(ReadLen::ONE);
            match class {
                ByteClass::Newline => self.finish_line(),
                other => {
                    if self.line.push(other) == Pushed::Refused {
                        self.refused.bytes = self.refused.bytes.saturating_incr();
                    }
                }
            }
        }
    }

    fn tick(&mut self, now: Millis) {
        self.last_tick = now;
        match self.activity {
            Activity::Seen => {
                self.last_input = now;
                self.activity = Activity::Quiet;
                self.idle = Idle::Pending;
            }
            Activity::Quiet => {
                if self.idle == Idle::Pending && now.since(self.last_input) >= self.idle_after {
                    self.idle = Idle::Reported;
                    self.emit(Effect::Idle(now));
                }
            }
        }
        if let Retry::Owed { from, wait } = self.retry {
            if now.since(from) >= wait {
                self.retry = Retry::Clear;
                self.emit(Effect::Retry(now));
            }
        }
    }

    /// One more consecutive failure: owe a retry after the current backoff,
    /// counted from the last tick, replacing any retry still owed; the next
    /// failure waits twice as long, up to the cap. A retry firing is not a
    /// success, so only healthy bytes reset the backoff.
    fn upstream_failed(&mut self) {
        self.retry = Retry::Owed {
            from: self.last_tick,
            wait: self.backoff,
        };
        self.backoff = self.backoff.saturating_double().min(MAX_BACKOFF);
    }

    fn end(&mut self) {
        if !self.line.is_empty() {
            self.finish_line();
        }
        self.phase = Phase::Ended;
    }

    /// Ends the current line and starts the next at the current position.
    fn finish_line(&mut self) {
        let done = mem::replace(&mut self.line, Line::starting_at(self.pos)).complete();
        self.emit(Effect::Line(done.record()));
    }

    fn emit(&mut self, e: Effect) {
        if self.effects.push(e) == Queued::Dropped {
            self.refused.effects = self.refused.effects.saturating_incr();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines<const N: usize>(m: &Machine<N>) -> usize {
        m.effects()
            .iter()
            .filter(|e| matches!(e, Effect::Line(_)))
            .count()
    }

    #[test]
    fn a_full_queue_drops_and_counts() {
        let mut m: Machine<2> = Machine::new(Elapsed::threshold_ms(10));
        m.handle(Event::Bytes(Wire::of(b"a\nb\nc\n")));
        assert_eq!(lines(&m), 2);
        assert_eq!(m.refused().effects.get(), 1);
        assert_eq!(m.refused().total().get(), 1);
    }

    #[test]
    fn clearing_forgets_what_was_read_and_the_queue_refills_from_empty() {
        let mut m: Machine<2> = Machine::new(Elapsed::threshold_ms(10));
        m.handle(Event::Bytes(Wire::of(b"a\nb\n")));
        assert_eq!(lines(&m), 2);
        m.clear_effects();
        assert!(m.effects().is_empty());
        m.handle(Event::Bytes(Wire::of(b"c\nd\n")));
        assert_eq!(lines(&m), 2);
        assert_eq!(
            m.refused().effects.get(),
            0,
            "a cleared queue has room again"
        );
    }

    #[test]
    fn an_empty_read_is_not_activity() {
        let mut m: Machine<4> = Machine::new(Elapsed::threshold_ms(10));
        m.handle(Event::Bytes(Wire::of(b"x")));
        m.handle(Event::Tick(Millis::from_host(100)));
        m.handle(Event::Bytes(Wire::of(b"")));
        m.handle(Event::Tick(Millis::from_host(200)));
        assert_eq!(m.effects(), &[Effect::Idle(Millis::from_host(200))]);
    }

    #[test]
    fn events_after_the_end_are_refused_and_counted() {
        let mut m: Machine<4> = Machine::new(Elapsed::threshold_ms(10));
        m.handle(Event::End);
        m.handle(Event::Bytes(Wire::of(b"late\n")));
        assert_eq!(lines(&m), 0);
        assert_eq!(m.refused().after_end.get(), 1);
    }

    #[test]
    fn the_last_line_needs_no_newline() {
        let mut m: Machine<4> = Machine::new(Elapsed::threshold_ms(10));
        m.handle(Event::Bytes(Wire::of(b"one two")));
        assert_eq!(lines(&m), 0);
        m.handle(Event::End);
        assert_eq!(lines(&m), 1);
    }

    #[test]
    fn a_quiet_spell_is_reported_once_and_only_after_the_threshold() {
        let mut m: Machine<4> = Machine::new(Elapsed::threshold_ms(10));
        m.handle(Event::Bytes(Wire::of(b"x")));
        m.handle(Event::Tick(Millis::from_host(100)));
        m.handle(Event::Tick(Millis::from_host(109)));
        assert!(m.effects().is_empty(), "9 ms is under the threshold");
        m.handle(Event::Tick(Millis::from_host(110)));
        m.handle(Event::Tick(Millis::from_host(500)));
        assert_eq!(m.effects(), &[Effect::Idle(Millis::from_host(110))]);
    }

    /// A machine whose idle report never fires, so only retries and lines
    /// show up in these tests.
    fn never_idle() -> Machine<8> {
        Machine::new(Elapsed::threshold_ms(u64::MAX))
    }

    fn tick<const N: usize>(m: &mut Machine<N>, ms: u64) {
        m.handle(Event::Tick(Millis::from_host(ms)));
    }

    fn retry_at(ms: u64) -> Effect {
        Effect::Retry(Millis::from_host(ms))
    }

    #[test]
    fn a_failure_owes_one_retry_after_100_ms_from_the_last_tick() {
        let mut m = never_idle();
        tick(&mut m, 1000);
        m.handle(Event::UpstreamFailed);
        assert!(
            m.effects().is_empty(),
            "a failure owes nothing until a tick"
        );
        tick(&mut m, 1099);
        assert!(m.effects().is_empty(), "99 ms is under the backoff");
        tick(&mut m, 1100);
        tick(&mut m, 1200);
        tick(&mut m, 9000);
        assert_eq!(m.effects(), &[retry_at(1100)], "exactly once");
    }

    #[test]
    fn consecutive_failures_double_the_backoff_up_to_the_cap() {
        let mut m = never_idle();
        let mut t = 0;
        tick(&mut m, t);
        for wait in [100, 200, 400, 800, 1600, 1600, 1600] {
            m.handle(Event::UpstreamFailed);
            tick(&mut m, t + wait - 1);
            assert!(m.effects().is_empty(), "early after a wait of {wait}");
            tick(&mut m, t + wait);
            assert_eq!(m.effects(), &[retry_at(t + wait)], "a wait of {wait}");
            m.clear_effects();
            t += wait;
        }
    }

    #[test]
    fn healthy_bytes_reset_the_backoff_to_100_ms() {
        let mut m = never_idle();
        tick(&mut m, 0);
        m.handle(Event::UpstreamFailed);
        tick(&mut m, 100);
        m.handle(Event::UpstreamFailed);
        tick(&mut m, 300);
        assert_eq!(m.effects(), &[retry_at(100), retry_at(300)]);
        m.clear_effects();
        m.handle(Event::Bytes(Wire::of(b"back")));
        m.handle(Event::UpstreamFailed);
        tick(&mut m, 399);
        assert!(m.effects().is_empty());
        tick(&mut m, 400);
        assert_eq!(m.effects(), &[retry_at(400)], "100 ms again, not 400");
    }

    #[test]
    fn an_empty_read_is_not_a_healthy_upstream() {
        let mut m = never_idle();
        tick(&mut m, 0);
        m.handle(Event::UpstreamFailed);
        tick(&mut m, 100);
        m.handle(Event::Bytes(Wire::of(b"")));
        m.handle(Event::UpstreamFailed);
        tick(&mut m, 200);
        tick(&mut m, 300);
        assert_eq!(m.effects(), &[retry_at(100), retry_at(300)]);
    }

    #[test]
    fn a_failure_while_a_retry_is_owed_replaces_it_and_doubles() {
        let mut m = never_idle();
        tick(&mut m, 0);
        m.handle(Event::UpstreamFailed);
        tick(&mut m, 50);
        m.handle(Event::UpstreamFailed);
        tick(&mut m, 100);
        tick(&mut m, 249);
        assert!(m.effects().is_empty(), "the first retry was replaced");
        tick(&mut m, 250);
        tick(&mut m, 5000);
        assert_eq!(m.effects(), &[retry_at(250)], "one retry, 200 ms after 50");
    }

    #[test]
    fn a_failure_after_the_end_is_refused_and_owes_nothing() {
        let mut m = never_idle();
        m.handle(Event::End);
        m.handle(Event::UpstreamFailed);
        tick(&mut m, 5000);
        assert!(m.effects().is_empty());
        assert_eq!(m.refused().after_end.get(), 2);
    }
}
