//! A host's opt-in resize settling: the time rules for how long the last whole screen stays drawn.

use std::time::{Duration, Instant};

/// The least a screen stays after a resize, so a program's redraw and its bridge get to answer.
pub(super) const HOLD: Duration = Duration::from_millis(500);

/// The most, from the latest size, so output that never rests still shows.
pub(super) const CAP: Duration = Duration::from_secs(1);

/// One resize's wait: it ends once the output has been quiet for `quiet` and `HOLD` has passed,
/// and never later than `CAP` after the latest size.
#[derive(Clone, Copy, Debug)]
pub(super) struct Settling {
    quiet: Duration,
    started: Instant,
    not_before: Instant,
    drawn: Instant,
}

impl Settling {
    pub(super) fn begin(now: Instant, quiet: Duration) -> Self {
        Self {
            quiet,
            started: now,
            not_before: now + HOLD,
            drawn: now,
        }
    }

    /// The size changed again: the hold and the cap count from this size.
    pub(super) fn resized(&mut self, now: Instant) {
        self.started = now;
        self.not_before = now + HOLD;
    }

    /// The program's output reached the grid at `now`.
    pub(super) fn output(&mut self, now: Instant) {
        self.drawn = now;
    }

    /// When the screen shows the grid again.
    pub(super) fn due(&self) -> Instant {
        (self.drawn + self.quiet)
            .max(self.not_before)
            .min(self.started + CAP)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUIET: Duration = Duration::from_millis(120);

    fn after(from: Instant, millis: u64) -> Instant {
        from + Duration::from_millis(millis)
    }

    #[test]
    fn a_resize_without_output_waits_the_hold() {
        let now = Instant::now();
        let settling = Settling::begin(now, QUIET);
        assert_eq!(settling.due(), after(now, 500));
    }

    #[test]
    fn output_after_the_hold_waits_for_a_quiet_stretch() {
        let now = Instant::now();
        let mut settling = Settling::begin(now, QUIET);
        settling.output(after(now, 450));
        assert_eq!(settling.due(), after(now, 570));
        settling.output(after(now, 560));
        assert_eq!(settling.due(), after(now, 680));
    }

    #[test]
    fn output_before_the_hold_does_not_shorten_it() {
        let now = Instant::now();
        let mut settling = Settling::begin(now, QUIET);
        settling.output(after(now, 100));
        assert_eq!(settling.due(), after(now, 500));
    }

    #[test]
    fn output_that_never_rests_ends_at_the_cap() {
        let now = Instant::now();
        let mut settling = Settling::begin(now, QUIET);
        for millis in (0..2000).step_by(50) {
            settling.output(after(now, millis));
        }
        assert_eq!(settling.due(), after(now, 1000));
    }

    #[test]
    fn a_new_size_counts_hold_and_cap_from_itself() {
        let now = Instant::now();
        let mut settling = Settling::begin(now, QUIET);
        settling.output(after(now, 300));
        settling.resized(after(now, 800));
        assert_eq!(settling.due(), after(now, 1300));
        settling.output(after(now, 1250));
        assert_eq!(settling.due(), after(now, 1370));
        settling.output(after(now, 1790));
        assert_eq!(settling.due(), after(now, 1800), "the cap from the latest size");
    }
}
