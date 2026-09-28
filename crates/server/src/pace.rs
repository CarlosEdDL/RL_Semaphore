//! The pure, clock-free deadline logic behind [`crate::Pace`] (R5.2), so it can be unit-tested
//! without sleeping (R9.4).

use std::time::{Duration, Instant};

use crate::config::Pace;

/// What the sim thread should do about its pending tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Decision {
    /// The pending tick is due: publish it now.
    Publish,
    /// Not yet due: wait until this instant, then poll again.
    Wait(Instant),
}

/// The absolute-deadline scheduler behind [`crate::Pace::RealTime`].
///
/// Tick `k`'s deadline is `start + k * period` as long as every previous tick was published no
/// more than one period late; that keeps the schedule drift-free (each deadline is the previous
/// one plus exactly one period, never recomputed from a fresh call to `Instant::now`). If a tick
/// is published more than one period late, the following deadline is `now + period` instead, so
/// a burst of already-missed ticks is never queued up and fired back to back.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Pacer {
    period: Duration,
    deadline: Instant,
}

impl Pacer {
    /// A pacer whose tick 0 is due exactly at `start`.
    pub(crate) const fn new(period: Duration, start: Instant) -> Self {
        Self {
            period,
            deadline: start,
        }
    }

    /// Checks the pending tick's deadline against `now`.
    pub(crate) fn poll(&mut self, now: Instant) -> Decision {
        if now < self.deadline {
            return Decision::Wait(self.deadline);
        }
        let late = now.saturating_duration_since(self.deadline);
        self.deadline = if late > self.period {
            now + self.period
        } else {
            self.deadline + self.period
        };
        Decision::Publish
    }
}

/// The pacing decision for a whole [`Pace`]: a [`Pacer`] for [`Pace::RealTime`], or an
/// always-publish decision for [`Pace::Unthrottled`] that never waits.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Ticker {
    /// Paced by wall-clock time.
    RealTime(Pacer),
    /// No pacing: always due.
    Unthrottled,
}

impl Ticker {
    /// Builds the ticker for `pace`, whose tick 0 (under [`Pace::RealTime`]) is due at `start`.
    pub(crate) fn new(pace: Pace, start: Instant) -> Self {
        match pace {
            Pace::RealTime { speed } => {
                // INVARIANT: `speed` was validated finite and > 0 by `ServerConfig::validate`
                // before the sim thread starts (R4.3), so this division is finite and positive.
                let period = Duration::from_secs_f64(1.0 / speed);
                Self::RealTime(Pacer::new(period, start))
            }
            Pace::Unthrottled => Self::Unthrottled,
        }
    }

    /// Checks the pending tick's deadline against `now`. Always [`Decision::Publish`] for
    /// [`Pace::Unthrottled`].
    pub(crate) fn poll(&mut self, now: Instant) -> Decision {
        match self {
            Self::RealTime(pacer) => pacer.poll(now),
            Self::Unthrottled => Decision::Publish,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{Decision, Pace, Pacer, Ticker};

    #[test]
    fn tick_zero_is_due_at_start() {
        let start = Instant::now();
        let mut pacer = Pacer::new(Duration::from_secs(1), start);
        assert_eq!(pacer.poll(start), Decision::Publish);
    }

    #[test]
    fn waits_until_the_deadline_when_early() {
        let start = Instant::now();
        let mut pacer = Pacer::new(Duration::from_secs(1), start);
        let early = start.checked_sub(Duration::from_millis(1)).unwrap();
        assert_eq!(pacer.poll(early), Decision::Wait(start));
    }

    #[test]
    fn deadlines_advance_by_exactly_one_period_with_no_drift() {
        let start = Instant::now();
        let period = Duration::from_millis(100);
        let mut pacer = Pacer::new(period, start);
        // Hitting each deadline exactly on time, `start + k * period`, always publishes: the
        // deadline never drifts away from that schedule.
        for k in 0..100u32 {
            let now = start + period * k;
            assert_eq!(pacer.poll(now), Decision::Publish, "tick {k}");
        }
    }

    #[test]
    fn a_small_lag_does_not_reset_the_reference() {
        let start = Instant::now();
        let period = Duration::from_millis(100);
        let mut pacer = Pacer::new(period, start);
        assert_eq!(pacer.poll(start), Decision::Publish);
        // Half a period late: still within one period, so the deadline advances from the old
        // one, not from `now`.
        let late = start + period + period / 2;
        assert_eq!(pacer.poll(late), Decision::Publish);
        assert_eq!(pacer.poll(start + period * 2), Decision::Publish);
    }

    #[test]
    fn more_than_one_period_late_resets_the_reference_to_now() {
        let start = Instant::now();
        let period = Duration::from_millis(100);
        let mut pacer = Pacer::new(period, start);
        assert_eq!(pacer.poll(start), Decision::Publish);
        // Three periods late.
        let very_late = start + period * 4;
        assert_eq!(pacer.poll(very_late), Decision::Publish);
        // The next deadline is `very_late + period`, not `start + 2 * period` (which has
        // already passed) or a queued-up `start + 5 * period`.
        let just_before = (very_late + period)
            .checked_sub(Duration::from_millis(1))
            .unwrap();
        assert_eq!(pacer.poll(just_before), Decision::Wait(very_late + period));
    }

    #[test]
    fn unthrottled_ticker_never_waits() {
        let start = Instant::now();
        let mut ticker = Ticker::new(Pace::Unthrottled, start);
        for offset in [0, 1, 1_000_000] {
            assert_eq!(
                ticker.poll(start + Duration::from_nanos(offset)),
                Decision::Publish
            );
        }
    }

    #[test]
    fn real_time_ticker_matches_a_bare_pacer() {
        let start = Instant::now();
        let mut ticker = Ticker::new(Pace::RealTime { speed: 2.0 }, start);
        let mut pacer = Pacer::new(Duration::from_millis(500), start);
        for k in 0..10u32 {
            let now = start + Duration::from_millis(500) * k;
            assert_eq!(ticker.poll(now), pacer.poll(now), "tick {k}");
        }
    }
}
