//! Test-only process-local deadlines whose clocks advance while the handset sleeps.
//!
//! These values are never serialized and are not trusted UTC, MiBank approval, or monetary
//! commit-time authority. They only bound a native-owned challenge. Apple uses the public
//! `mach_continuous_time` clock; Android/Linux use `CLOCK_BOOTTIME`. There is no wall-clock,
//! uptime-only or unsupported-platform fallback.
//!
//! Platform contracts: <https://developer.apple.com/documentation/kernel/mach> and
//! <https://man7.org/linux/man-pages/man2/clock_gettime.2.html>.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

const NANOS_PER_SECOND: u128 = 1_000_000_000;
const MAX_LIFETIME: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NativeDeadlineErrorV1 {
    Unavailable,
    Invalid,
    Expired,
}

type Result<T> = std::result::Result<T, NativeDeadlineErrorV1>;

/// Native-created same-process clock reading. No host timestamp constructor or wire codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NativeContinuousInstantV1 {
    process_id: u32,
    nanos: u128,
}

impl NativeContinuousInstantV1 {
    pub(super) fn now() -> Result<Self> {
        let process_id = std::process::id();
        let nanos = platform_nanos()?;
        if process_id == 0 || process_id != std::process::id() {
            return Err(NativeDeadlineErrorV1::Invalid);
        }
        Ok(Self { process_id, nanos })
    }

    pub(super) fn checked_duration_since(self, earlier: Self) -> Option<Duration> {
        if self.process_id != earlier.process_id {
            return None;
        }
        let nanos = self.nanos.checked_sub(earlier.nanos)?;
        Some(Duration::new(
            (nanos / NANOS_PER_SECOND).try_into().ok()?,
            (nanos % NANOS_PER_SECOND).try_into().ok()?,
        ))
    }
}

struct DeadlineState {
    started: NativeContinuousInstantV1,
    expires_nanos: u128,
    last_seen: Mutex<Option<u128>>,
}

/// Clones retain the original expiry and shared monotonic floor; cloning never renews a lease.
/// A forked process cannot reuse its parent's pending challenge even within the same boot.
#[derive(Clone)]
pub(super) struct NativeDeadlineV1(Arc<DeadlineState>);

impl NativeDeadlineV1 {
    pub(super) fn start(lifetime: Duration) -> Result<Self> {
        Self::from_reading(NativeContinuousInstantV1::now()?, lifetime)
    }

    fn from_reading(started: NativeContinuousInstantV1, lifetime: Duration) -> Result<Self> {
        if lifetime.is_zero() || lifetime > MAX_LIFETIME || started.process_id == 0 {
            return Err(NativeDeadlineErrorV1::Invalid);
        }
        let expires_nanos = started
            .nanos
            .checked_add(lifetime.as_nanos())
            .ok_or(NativeDeadlineErrorV1::Invalid)?;
        Ok(Self(Arc::new(DeadlineState {
            started,
            expires_nanos,
            last_seen: Mutex::new(Some(started.nanos)),
        })))
    }

    /// Check against the actual continuous clock, returning the exact successful observation.
    pub(super) fn check(&self) -> Result<NativeContinuousInstantV1> {
        let mut last_seen = self
            .0
            .last_seen
            .lock()
            .map_err(|_| NativeDeadlineErrorV1::Invalid)?;
        // Read while holding the shared clock lock. Concurrent clones cannot observe
        // increasing times and then accidentally publish them in reverse order.
        let now = match NativeContinuousInstantV1::now() {
            Ok(now) => now,
            Err(error) => {
                *last_seen = None;
                return Err(error);
            }
        };
        self.check_locked(&mut last_seen, now)
    }

    #[cfg(test)]
    fn check_reading(&self, now: NativeContinuousInstantV1) -> Result<NativeContinuousInstantV1> {
        let mut last_seen = self
            .0
            .last_seen
            .lock()
            .map_err(|_| NativeDeadlineErrorV1::Invalid)?;
        self.check_locked(&mut last_seen, now)
    }

    fn check_locked(
        &self,
        last_seen: &mut Option<u128>,
        now: NativeContinuousInstantV1,
    ) -> Result<NativeContinuousInstantV1> {
        if now.process_id != self.0.started.process_id
            || last_seen.is_none_or(|previous| now.nanos < previous)
        {
            // A faulty clock or process replay permanently invalidates this challenge.
            *last_seen = None;
            return Err(NativeDeadlineErrorV1::Invalid);
        }
        *last_seen = Some(now.nanos);
        if now.nanos >= self.0.expires_nanos {
            return Err(NativeDeadlineErrorV1::Expired);
        }
        Ok(now)
    }

    #[cfg(test)]
    pub(super) fn expired_for_test() -> Self {
        let now = NativeContinuousInstantV1::now().unwrap();
        Self(Arc::new(DeadlineState {
            started: now,
            expires_nanos: now.nanos,
            last_seen: Mutex::new(Some(now.nanos)),
        }))
    }
}

#[cfg(target_vendor = "apple")]
fn platform_nanos() -> Result<u128> {
    // Verified against the current public SDK's mach/mach_time.h. The conversion ratio
    // is immutable for this boot; rejection is cached too, never replaced by a weak clock.
    #[repr(C)]
    struct MachTimebase {
        numer: u32,
        denom: u32,
    }
    unsafe extern "C" {
        fn mach_continuous_time() -> u64;
        fn mach_timebase_info(info: *mut MachTimebase) -> i32;
    }
    static TIMEBASE: std::sync::OnceLock<Result<(u32, u32)>> = std::sync::OnceLock::new();
    let (numer, denom) = *TIMEBASE
        .get_or_init(|| {
            let mut info = MachTimebase { numer: 0, denom: 0 };
            // SAFETY: writable correctly laid-out struct lives for the complete C call.
            if unsafe { mach_timebase_info(&mut info) } != 0 || info.numer == 0 || info.denom == 0 {
                Err(NativeDeadlineErrorV1::Unavailable)
            } else {
                Ok((info.numer, info.denom))
            }
        })
        .as_ref()
        .map_err(|error| *error)?;
    // SAFETY: the SDK function takes no pointers and is available on all supported iPhones.
    mach_ticks_to_nanos(unsafe { mach_continuous_time() }, numer, denom)
}

#[cfg(any(target_vendor = "apple", test))]
fn mach_ticks_to_nanos(ticks: u64, numer: u32, denom: u32) -> Result<u128> {
    if numer == 0 || denom == 0 {
        return Err(NativeDeadlineErrorV1::Unavailable);
    }
    u128::from(ticks)
        .checked_mul(u128::from(numer))
        .and_then(|value| value.checked_div(u128::from(denom)))
        .ok_or(NativeDeadlineErrorV1::Invalid)
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn platform_nanos() -> Result<u128> {
    let mut time: libc::timespec = unsafe { std::mem::zeroed() };
    // SAFETY: native libc supplies the target's exact timespec layout, including 32-bit ABIs.
    // A denied/absent BOOTTIME clock is unavailable, never replaced with MONOTONIC or REALTIME.
    if unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut time) } != 0 {
        return Err(NativeDeadlineErrorV1::Unavailable);
    }
    timespec_to_nanos(i128::from(time.tv_sec), i128::from(time.tv_nsec))
}

#[cfg(any(target_os = "android", target_os = "linux", test))]
fn timespec_to_nanos(seconds: i128, nanos: i128) -> Result<u128> {
    if seconds < 0 || !(0..1_000_000_000).contains(&nanos) {
        return Err(NativeDeadlineErrorV1::Invalid);
    }
    (seconds as u128)
        .checked_mul(NANOS_PER_SECOND)
        .and_then(|value| value.checked_add(nanos as u128))
        .ok_or(NativeDeadlineErrorV1::Invalid)
}

#[cfg(not(any(target_vendor = "apple", target_os = "android", target_os = "linux")))]
fn platform_nanos() -> Result<u128> {
    Err(NativeDeadlineErrorV1::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick(nanos: u128) -> NativeContinuousInstantV1 {
        NativeContinuousInstantV1 {
            process_id: 7,
            nanos,
        }
    }

    #[test]
    fn exact_expiry_is_rejected_and_cloning_preserves_the_original_deadline() {
        let deadline = NativeDeadlineV1::from_reading(tick(100), Duration::from_nanos(50)).unwrap();
        assert_eq!(deadline.check_reading(tick(149)), Ok(tick(149)));
        let copy = deadline.clone();
        assert!(Arc::ptr_eq(&deadline.0, &copy.0));
        assert_eq!(
            copy.check_reading(tick(150)),
            Err(NativeDeadlineErrorV1::Expired)
        );
        assert_eq!(
            deadline.check_reading(tick(150)),
            Err(NativeDeadlineErrorV1::Expired)
        );
    }

    #[test]
    fn elapsed_suspension_expires_a_pending_challenge_without_intermediate_polls() {
        let deadline = NativeDeadlineV1::from_reading(tick(0), MAX_LIFETIME).unwrap();
        // A continuous clock includes time asleep even though application code never ran.
        assert_eq!(
            deadline.check_reading(tick(121 * NANOS_PER_SECOND)),
            Err(NativeDeadlineErrorV1::Expired)
        );
    }

    #[test]
    fn backwards_reading_and_parent_process_replay_reject() {
        let deadline = NativeDeadlineV1::from_reading(tick(100), Duration::from_nanos(50)).unwrap();
        deadline.check_reading(tick(120)).unwrap();
        assert_eq!(
            deadline.clone().check_reading(tick(119)),
            Err(NativeDeadlineErrorV1::Invalid)
        );
        assert_eq!(
            deadline.check_reading(tick(121)),
            Err(NativeDeadlineErrorV1::Invalid)
        );
        let deadline = NativeDeadlineV1::from_reading(tick(100), Duration::from_nanos(50)).unwrap();
        assert_eq!(
            deadline.check_reading(NativeContinuousInstantV1 {
                process_id: 8,
                nanos: 121
            }),
            Err(NativeDeadlineErrorV1::Invalid)
        );
    }

    #[test]
    fn duration_comparison_requires_one_process_and_non_decreasing_time() {
        assert_eq!(
            tick(150).checked_duration_since(tick(100)),
            Some(Duration::from_nanos(50))
        );
        assert_eq!(tick(99).checked_duration_since(tick(100)), None);
        assert_eq!(
            NativeContinuousInstantV1 {
                process_id: 8,
                nanos: 150
            }
            .checked_duration_since(tick(100)),
            None
        );
        assert_eq!(tick(u128::MAX).checked_duration_since(tick(0)), None);
    }

    #[test]
    fn concurrent_clones_share_clock_order_without_false_rollback() {
        let deadline = NativeDeadlineV1::start(Duration::from_secs(30)).unwrap();
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let deadline = deadline.clone();
                std::thread::spawn(move || {
                    for _ in 0..1000 {
                        deadline.check().unwrap();
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        deadline.check().unwrap();
    }

    #[test]
    fn invalid_lifetime_and_arithmetic_overflow_have_no_weak_clock_fallback() {
        for duration in [Duration::ZERO, Duration::from_secs(121)] {
            assert!(matches!(
                NativeDeadlineV1::from_reading(tick(1), duration),
                Err(NativeDeadlineErrorV1::Invalid)
            ));
        }
        assert!(matches!(
            NativeDeadlineV1::from_reading(tick(u128::MAX), Duration::from_nanos(1)),
            Err(NativeDeadlineErrorV1::Invalid)
        ));
    }

    #[test]
    fn mach_timebase_conversion_is_checked_and_retains_subsecond_precision() {
        assert_eq!(mach_ticks_to_nanos(99, 125, 3), Ok(4125));
        assert_eq!(
            mach_ticks_to_nanos(u64::MAX, u32::MAX, 1),
            Ok(u128::from(u64::MAX) * u128::from(u32::MAX))
        );
        assert_eq!(
            mach_ticks_to_nanos(1, 1, 0),
            Err(NativeDeadlineErrorV1::Unavailable)
        );
        assert_eq!(
            mach_ticks_to_nanos(1, 0, 1),
            Err(NativeDeadlineErrorV1::Unavailable)
        );
    }

    #[test]
    fn kernel_timespec_rejects_negative_noncanonical_and_overflowing_values() {
        assert_eq!(timespec_to_nanos(2, 17), Ok(2_000_000_017));
        for (seconds, nanos) in [(-1, 0), (0, -1), (0, 1_000_000_000), (i128::MAX, 0)] {
            assert_eq!(
                timespec_to_nanos(seconds, nanos),
                Err(NativeDeadlineErrorV1::Invalid)
            );
        }
    }

    #[test]
    #[cfg(any(target_vendor = "apple", target_os = "android", target_os = "linux"))]
    fn actual_platform_continuous_clock_supports_a_native_owned_deadline() {
        let deadline = NativeDeadlineV1::start(Duration::from_secs(1)).unwrap();
        let first = deadline.check().unwrap();
        let second = deadline.check().unwrap();
        assert_eq!(first.process_id, std::process::id());
        assert!(second.nanos >= first.nanos);
        assert_eq!(
            NativeDeadlineV1::expired_for_test().check(),
            Err(NativeDeadlineErrorV1::Expired)
        );
    }
}
