//! Native wall observations for local idle Archive scheduling, never monetary time authority.
//!
//! The first observation is stored with durable receipt intake. A fresh wall observation
//! includes time across restart and reboot, but clock adjustments may advance or delay this
//! local gate. These values never enter proofs, Requests, quotas, leases or accepted-time
//! floors. Design §7 continues to use its separate authenticated monotonic clock.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

const IDLE_AGE_MILLIS: u64 = 86_400_000;
const MIN_RECHECK_MILLIS: u64 = 60_000;
const MAX_RECHECK_MILLIS: u64 = 15 * MIN_RECHECK_MILLIS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WallObservation {
    pub(super) unix_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ClockUnavailable {
    BeforeEpoch,
    OutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Age {
    Due,
    NotDue,
    Unavailable,
    Regression,
}

fn from_wall_time(now: SystemTime) -> Result<WallObservation, ClockUnavailable> {
    let elapsed = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ClockUnavailable::BeforeEpoch)?;
    from_elapsed(elapsed)
}

fn from_elapsed(elapsed: Duration) -> Result<WallObservation, ClockUnavailable> {
    let unix_ms = u64::try_from(elapsed.as_millis()).map_err(|_| ClockUnavailable::OutOfRange)?;
    Ok(WallObservation { unix_ms })
}

/// Sample the Native process clock; foreign callers cannot supply a scheduling timestamp.
pub(super) fn observe() -> Result<WallObservation, ClockUnavailable> {
    from_wall_time(SystemTime::now())
}

/// Re-evaluate only this local age alternative. No high-water mark preserves eligibility
/// after a clock adjustment; a missing first observation is never replaced on retry.
pub(super) fn age(first: Option<u64>, current: Result<WallObservation, ClockUnavailable>) -> Age {
    let (Some(first), Ok(current)) = (first, current) else {
        return Age::Unavailable;
    };
    match current.unix_ms.checked_sub(first) {
        Some(elapsed) if elapsed >= IDLE_AGE_MILLIS => Age::Due,
        Some(_) => Age::NotDue,
        None => Age::Regression,
    }
}

/// Relative housekeeping delay only. Never convert elapsed sleeps into a wall-age verdict.
pub(super) fn wake_hint(
    first: Option<u64>,
    current: Result<WallObservation, ClockUnavailable>,
) -> super::DeferredArchiveWakeHintV1 {
    use super::DeferredArchiveWakeHintV1 as Hint;
    let Some(first) = first else {
        return Hint::NoAgeWake;
    };
    let elapsed = current.ok().and_then(|now| now.unix_ms.checked_sub(first));
    match elapsed {
        Some(elapsed) if elapsed >= IDLE_AGE_MILLIS => Hint::Due,
        Some(elapsed) => Hint::RecheckAfter(Duration::from_millis(
            (IDLE_AGE_MILLIS - elapsed).clamp(MIN_RECHECK_MILLIS, MAX_RECHECK_MILLIS),
        )),
        None => Hint::RecheckAfter(Duration::from_millis(MAX_RECHECK_MILLIS)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wake_hint_threshold_and_bounded_delays_never_change_age_admission() {
        use super::super::DeferredArchiveWakeHintV1 as Hint;
        let first = 123;
        for (elapsed, expected) in [
            (0, Hint::RecheckAfter(Duration::from_mins(15))),
            (
                IDLE_AGE_MILLIS - 120_000,
                Hint::RecheckAfter(Duration::from_secs(120)),
            ),
            (
                IDLE_AGE_MILLIS - 1,
                Hint::RecheckAfter(Duration::from_secs(60)),
            ),
            (IDLE_AGE_MILLIS, Hint::Due),
            (IDLE_AGE_MILLIS + 1, Hint::Due),
        ] {
            let now = Ok(WallObservation {
                unix_ms: first + elapsed,
            });
            assert_eq!(wake_hint(Some(first), now), expected);
            assert_eq!(
                age(Some(first), now) == Age::Due,
                elapsed >= IDLE_AGE_MILLIS
            );
        }
    }

    #[test]
    fn wake_hint_missing_regressed_unavailable_and_adjusted_clocks_never_latch() {
        use super::super::DeferredArchiveWakeHintV1 as Hint;
        let first = 123;
        let due = Ok(WallObservation {
            unix_ms: first + IDLE_AGE_MILLIS,
        });
        assert_eq!(wake_hint(None, due), Hint::NoAgeWake);
        assert_eq!(
            wake_hint(None, Err(ClockUnavailable::BeforeEpoch)),
            Hint::NoAgeWake
        );
        assert_eq!(wake_hint(Some(first), due), Hint::Due);
        for now in [
            Ok(WallObservation { unix_ms: first - 1 }),
            Err(ClockUnavailable::BeforeEpoch),
            Err(ClockUnavailable::OutOfRange),
        ] {
            assert_eq!(
                wake_hint(Some(first), now),
                Hint::RecheckAfter(Duration::from_mins(15))
            );
            assert_ne!(age(Some(first), now), Age::Due);
        }
        // A frozen near-threshold clock repeats a positive delay, never elapsed-time credit.
        for _ in 0..3 {
            let now = Ok(WallObservation {
                unix_ms: first + IDLE_AGE_MILLIS - 1,
            });
            assert_eq!(
                wake_hint(Some(first), now),
                Hint::RecheckAfter(Duration::from_secs(60))
            );
            assert_eq!(age(Some(first), now), Age::NotDue);
        }
        assert_eq!(
            wake_hint(Some(u64::MAX), Ok(WallObservation { unix_ms: 0 })),
            Hint::RecheckAfter(Duration::from_mins(15))
        );
        assert_eq!(
            wake_hint(
                Some(u64::MAX - IDLE_AGE_MILLIS),
                Ok(WallObservation { unix_ms: u64::MAX })
            ),
            Hint::Due
        );
    }

    #[test]
    fn wall_conversion_preserves_epoch_and_truncates_only_submillisecond_precision() {
        assert_eq!(
            from_wall_time(UNIX_EPOCH),
            Ok(WallObservation { unix_ms: 0 })
        );
        assert_eq!(
            from_wall_time(UNIX_EPOCH + Duration::from_nanos(1_999_999)),
            Ok(WallObservation { unix_ms: 1 })
        );
        assert_eq!(
            from_wall_time(UNIX_EPOCH - Duration::from_nanos(1)),
            Err(ClockUnavailable::BeforeEpoch)
        );
    }

    #[test]
    fn wall_conversion_rejects_millisecond_overflow_without_wrapping() {
        // Exercise integer conversion without requiring this platform's SystemTime
        // representation to reach hundreds of millions of years after the epoch.
        let last = Duration::from_millis(u64::MAX);
        assert_eq!(
            from_elapsed(last),
            Ok(WallObservation { unix_ms: u64::MAX })
        );
        assert_eq!(
            from_elapsed(last + Duration::from_millis(1)),
            Err(ClockUnavailable::OutOfRange)
        );
    }

    #[test]
    fn age_requires_the_complete_twenty_four_hour_threshold() {
        let first = 12_345;
        for (elapsed, expected) in [
            (0, Age::NotDue),
            (IDLE_AGE_MILLIS - 1, Age::NotDue),
            (IDLE_AGE_MILLIS, Age::Due),
            (IDLE_AGE_MILLIS + 1, Age::Due),
        ] {
            assert_eq!(
                age(
                    Some(first),
                    Ok(WallObservation {
                        unix_ms: first + elapsed
                    })
                ),
                expected
            );
        }
    }

    #[test]
    fn missing_or_unavailable_observations_never_fabricate_age() {
        for current in [
            Ok(WallObservation { unix_ms: u64::MAX }),
            Err(ClockUnavailable::BeforeEpoch),
            Err(ClockUnavailable::OutOfRange),
        ] {
            assert_eq!(age(None, current), Age::Unavailable);
        }
        for unavailable in [ClockUnavailable::BeforeEpoch, ClockUnavailable::OutOfRange] {
            assert_eq!(age(Some(0), Err(unavailable)), Age::Unavailable);
        }
    }

    #[test]
    fn adjusted_wall_time_is_rechecked_without_latching_prior_eligibility() {
        let first = 123;
        let held = Some(first);
        assert_eq!(
            age(
                held,
                Ok(WallObservation {
                    unix_ms: first + IDLE_AGE_MILLIS
                })
            ),
            Age::Due
        );
        assert_eq!(
            age(held, Ok(WallObservation { unix_ms: first + 1 })),
            Age::NotDue
        );
        assert_eq!(
            age(held, Ok(WallObservation { unix_ms: first - 1 })),
            Age::Regression
        );
        assert_eq!(
            age(held, Err(ClockUnavailable::BeforeEpoch)),
            Age::Unavailable
        );
        assert_eq!(
            age(
                held,
                Ok(WallObservation {
                    unix_ms: first + IDLE_AGE_MILLIS + 1
                })
            ),
            Age::Due
        );
        assert_eq!(
            held,
            Some(first),
            "observations never reset the stored origin"
        );
    }

    #[test]
    fn age_near_integer_limits_does_not_overflow_or_wrap_regression() {
        assert_eq!(
            age(
                Some(u64::MAX - IDLE_AGE_MILLIS),
                Ok(WallObservation { unix_ms: u64::MAX })
            ),
            Age::Due
        );
        assert_eq!(
            age(
                Some(u64::MAX - IDLE_AGE_MILLIS + 1),
                Ok(WallObservation { unix_ms: u64::MAX })
            ),
            Age::NotDue
        );
        assert_eq!(
            age(Some(u64::MAX), Ok(WallObservation { unix_ms: 0 })),
            Age::Regression
        );
    }
}
