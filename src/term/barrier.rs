//! A host's opt-in output barrier: it learns when the pty bytes it counted have reached the parser.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// How far the pty's output has come, shared by the tap that reads it, the listener that hears the
/// grid, and the view.
///
/// The tap, alacritty's parser and the listener's wakeups run on one thread, and a wakeup comes after
/// a turn's last `advance`. So the count the listener commits at a wakeup is what the parser has
/// taken, never the tap's running read count. A sequence the prompt scanner holds back is counted
/// when read. Bytes inside a synchronized update count too, though the grid shows them only at its
/// end or timeout; until then it holds the last complete screen, which is safe to show.
#[derive(Default)]
pub(super) struct Progress {
    /// Raw bytes read from the pty so far; only the tap writes it.
    read: AtomicU64,
    /// `read` as it stood at the last wakeup.
    done: AtomicU64,
    /// A host asked to hear the count at each wakeup.
    on: AtomicBool,
}

impl Progress {
    /// The tap's running total of raw bytes read: a relaxed store per read, no read-modify-write.
    /// The tap keeps the sum in its own `u64`, so this is the only shared write it makes.
    pub(super) fn read(&self, total: u64) {
        self.read.store(total, Ordering::Relaxed);
    }

    /// The parser caught up with everything read: commits the count, and returns it if a host listens.
    pub(super) fn woke(&self) -> Option<u64> {
        let bytes = self.read.load(Ordering::Relaxed);
        self.done.store(bytes, Ordering::SeqCst);
        self.on.load(Ordering::SeqCst).then_some(bytes)
    }

    /// The count the parser has certainly reached.
    pub(super) fn done(&self) -> u64 {
        self.done.load(Ordering::SeqCst)
    }

    /// Starts the host's events; a wakeup after this tells its count, one before is in `done`.
    pub(super) fn listen(&self) -> u64 {
        self.on.store(true, Ordering::SeqCst);
        self.done()
    }

    /// Ends the host's events: wakeups commit the count but tell nobody.
    pub(super) fn stop_listening(&self) {
        self.on.store(false, Ordering::SeqCst);
    }
}

/// One target count the host waits for, and the highest count the parser is known to have reached.
#[derive(Default)]
pub(super) struct Barrier {
    reached: u64,
    target: Option<u64>,
}

impl Barrier {
    /// Waits for `target`, replacing an earlier wait; `reached` is the count already known.
    /// Some at once if it is there, as zero always is.
    pub(super) fn arm(&mut self, target: u64, reached: u64) -> Option<u64> {
        self.reached = self.reached.max(reached);
        self.target = Some(target);
        self.check()
    }

    /// The parser reached `bytes` in all; some once the target is there.
    pub(super) fn advance(&mut self, bytes: u64) -> Option<u64> {
        self.reached = self.reached.max(bytes);
        self.check()
    }

    fn check(&mut self) -> Option<u64> {
        let target = self.target.filter(|&target| target <= self.reached)?;
        self.target = None;
        Some(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_already_reached_fires_at_once_and_zero_always() {
        let mut barrier = Barrier::default();
        assert_eq!(barrier.arm(0, 0), Some(0));
        assert_eq!(barrier.arm(10, 12), Some(10));
        assert_eq!(barrier.advance(20), None, "once");
    }

    #[test]
    fn a_target_ahead_waits_for_the_count_and_fires_once() {
        let mut barrier = Barrier::default();
        assert_eq!(barrier.arm(100, 40), None);
        assert_eq!(barrier.advance(99), None);
        assert_eq!(barrier.advance(150), Some(100));
        assert_eq!(barrier.advance(200), None);
    }

    #[test]
    fn a_new_target_replaces_the_old_and_the_count_never_falls() {
        let mut barrier = Barrier::default();
        assert_eq!(barrier.arm(100, 0), None);
        assert_eq!(barrier.arm(50, 0), None);
        assert_eq!(barrier.advance(60), Some(50));
        assert_eq!(barrier.arm(55, 0), Some(55), "60 is still reached");
    }

    #[test]
    fn read_bytes_are_processed_only_at_a_wakeup_and_told_only_to_a_listener() {
        let progress = Progress::default();
        progress.read(5);
        assert_eq!(progress.woke(), None, "nobody listens");
        assert_eq!(progress.done(), 5, "yet the parser's count is kept");
        assert_eq!(progress.listen(), 5);
        progress.read(12);
        assert_eq!(progress.done(), 5, "read is not processed");
        assert_eq!(progress.woke(), Some(12));
        progress.read(14);
        assert_eq!(progress.done(), 12, "14 was read, no wakeup yet");
        assert_eq!(progress.woke(), Some(14));
        assert_eq!(progress.done(), 14);
    }

    #[test]
    fn a_stopped_listener_hears_no_more_but_the_count_goes_on() {
        let progress = Progress::default();
        progress.listen();
        progress.read(3);
        assert_eq!(progress.woke(), Some(3));
        progress.stop_listening();
        progress.read(8);
        assert_eq!(progress.woke(), None);
        assert_eq!(progress.done(), 8);
        assert_eq!(progress.listen(), 8, "listening again starts from the count");
    }
}
