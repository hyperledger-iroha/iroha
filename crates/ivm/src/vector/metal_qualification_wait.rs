//! Bounded observations for required tests of the nonblocking Metal owner.
//!
//! Only absence may be retried. A supplied identity is returned immediately for
//! exact comparison by the caller; this helper cannot substitute an expected
//! owner, skip a device, reset quarantine or credit a native completion.

use std::time::{Duration, Instant};

pub(super) fn observe_until<T>(
    deadline: Instant,
    mut observe: impl FnMut() -> Option<T>,
) -> Option<T> {
    loop {
        let now = Instant::now();
        if now >= deadline {
            return None;
        }
        let observed = observe();
        if Instant::now() >= deadline {
            return None;
        }
        if let Some(value) = observed {
            return Some(value);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        std::thread::sleep(remaining.min(Duration::from_millis(1)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_observed_wrong_owner_is_returned_without_retry_or_substitution() {
        let mut calls = 0;
        let observed = observe_until(Instant::now() + Duration::from_secs(1), || {
            calls += 1;
            Some(17)
        });
        assert_eq!(observed, Some(17));
        assert_ne!(observed, Some(23));
        assert_eq!(calls, 1);
    }

    #[test]
    fn permanent_absence_exhausts_the_deadline_without_a_success() {
        let deadline = Instant::now() + Duration::from_millis(3);
        let observed = observe_until(deadline, || None::<usize>);
        assert_eq!(observed, None);
        assert!(Instant::now() >= deadline);
        assert_eq!(
            observe_until(Instant::now(), || panic!("expired")),
            None::<usize>
        );
    }

    #[test]
    fn delayed_identity_cannot_satisfy_an_expired_observation() {
        let deadline = Instant::now() + Duration::from_millis(3);
        assert_eq!(
            observe_until(deadline, || {
                std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
                Some(17)
            }),
            None
        );
    }
}
