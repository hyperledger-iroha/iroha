//! Fixed, owner-resident public cost profiles and nonblocking calibration scheduling.
//!
//! A profile is subordinate to an independently qualified device/kernel/artifact.
//! These timings never qualify a kernel or depend on production operand contents.

use iroha_accel::PtxArtifact;
use parking_lot::{Mutex, MutexGuard};
use std::{
    any::TypeId,
    time::{Duration, Instant},
};

pub(crate) use crate::bn254_vec::BATCH_SIZES as SIZES;
pub(crate) const TRIALS: usize = 3;
const PASS_BUDGET: Duration = Duration::from_secs(8);
const RETRY_DELAY: Duration = Duration::from_secs(30);

pub(crate) fn geometry_supported(items: usize) -> bool {
    (SIZES[0]..=SIZES[SIZES.len() - 1]).contains(&items)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProfileKey {
    pub(crate) artifact: PtxArtifact,
    pub(crate) cpu: TypeId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CalibrationFailure {
    /// Local capacity, scheduling, policy changes or insufficient stable samples.
    Deferred,
    /// The bounded public pass expired between native attempts.
    Deadline,
    /// Actual native failure; the native adapter retains its quarantine decision.
    Backend,
    /// Completed native output disagreed with the independent field relation.
    Parity,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TrialSample {
    pub(crate) cpu_ns: [u64; TRIALS],
    pub(crate) gpu_ns: [u64; TRIALS],
}

#[derive(Clone, Copy, Debug)]
struct Sample {
    items: usize,
    cpu_ns: u64,
    gpu_ns: u64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CostProfile([Sample; SIZES.len()]);

impl CostProfile {
    pub(crate) fn from_trials(trials: [TrialSample; SIZES.len()]) -> Option<Self> {
        let mut samples = [Sample {
            items: 0,
            cpu_ns: 0,
            gpu_ns: 0,
        }; SIZES.len()];
        for (index, trial) in trials.into_iter().enumerate() {
            let bounds = |times: [u64; TRIALS]| {
                let low = *times.iter().min()?;
                let high = *times.iter().max()?;
                // Zero or highly unstable timing provides no selection authority.
                (low != 0 && u128::from(high) <= u128::from(low) * 4).then_some((low, high))
            };
            let (cpu_low, _) = bounds(trial.cpu_ns)?;
            let (_, gpu_high) = bounds(trial.gpu_ns)?;
            samples[index] = Sample {
                items: SIZES[index],
                cpu_ns: cpu_low,
                gpu_ns: gpu_high,
            };
        }
        Some(Self(samples))
    }

    pub(crate) fn estimate(self, items: usize) -> Option<u64> {
        let (cpu, gpu) = crate::acceleration_cost::interpolate(
            &self.0,
            items,
            |sample| sample.items,
            |sample| (sample.cpu_ns, sample.gpu_ns),
        )?;
        // Compare conservative whole-operation bounds with a ten-percent margin.
        (u128::from(gpu) * 110 < u128::from(cpu) * 100).then_some(gpu)
    }
}

#[derive(Debug, Default)]
struct CachedProfile {
    key: Option<ProfileKey>,
    profile: Option<CostProfile>,
    retry_after: Option<Instant>,
}

/// Inline backing is charged with the original DevicePolicy allocation.
#[derive(Debug, Default)]
pub(crate) struct ProfileCell(Mutex<CachedProfile>);

impl ProfileCell {
    pub(crate) fn estimate(
        &self,
        key: ProfileKey,
        items: usize,
        now: Instant,
        pass: Option<&mut CalibrationPass<'_>>,
        calibrate: impl FnOnce(Instant) -> Result<CostProfile, CalibrationFailure>,
    ) -> Option<u64> {
        if !geometry_supported(items) {
            return None;
        }
        let mut state = self.0.try_lock()?;
        if state.key == Some(key) {
            if let Some(profile) = state.profile {
                return profile.estimate(items);
            }
            if state.retry_after.is_some_and(|after| now < after) {
                return None;
            }
        }
        let deadline = pass?.begin(now)?;
        // Publish the cooldown before calling foreign/native code: panic unwinds
        // neither promote a partial profile nor immediately repeat expensive work.
        *state = CachedProfile {
            key: Some(key),
            profile: None,
            retry_after: now.checked_add(RETRY_DELAY),
        };
        if let Ok(profile) = calibrate(deadline) {
            state.profile = Some(profile);
            state.retry_after = None;
        }
        state.profile?.estimate(items)
    }
}

/// One public synthetic calibration at a time, at most one candidate per call.
#[derive(Debug)]
pub(crate) struct CalibrationScheduler(Mutex<usize>);

impl CalibrationScheduler {
    pub(crate) const fn new() -> Self {
        Self(Mutex::new(0))
    }

    pub(crate) fn try_pass(&self, count: usize, now: Instant) -> Option<CalibrationPass<'_>> {
        if count == 0 {
            return None;
        }
        let mut cursor = self.0.try_lock()?;
        let start = *cursor % count;
        *cursor = (start + 1) % count;
        Some(CalibrationPass {
            _cursor: cursor,
            start,
            deadline: now.checked_add(PASS_BUDGET)?,
            unused: true,
        })
    }
}

pub(crate) struct CalibrationPass<'a> {
    _cursor: MutexGuard<'a, usize>,
    pub(crate) start: usize,
    deadline: Instant,
    unused: bool,
}

impl CalibrationPass<'_> {
    fn begin(&mut self, now: Instant) -> Option<Instant> {
        if !self.unused || now >= self.deadline {
            return None;
        }
        self.unused = false;
        Some(self.deadline)
    }
}

#[cfg(test)]
#[path = "bn254_cost_tests.rs"]
mod tests;
