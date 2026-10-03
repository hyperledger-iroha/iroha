//! Exact AES geometry, measured CPU identity and bounded physical-owner costs.

use super::{AesCpuBaseline, CalibrationFailure, MetalBatchWork};
use std::time::{Duration, Instant};

mod sample;
#[cfg(test)]
mod scheduler_tests;
#[cfg(test)]
mod tests;

const MIN_BLOCKS: usize = 32;
const MAX_BLOCKS: usize = 2_048;
const MAX_ROUNDS: usize = 64;
const TRIALS: usize = 3;
const PATTERNS: usize = 2;
const MAX_CALIBRATION: Duration = Duration::from_secs(8);
const RETRY_AFTER: Duration = Duration::from_secs(30);

/// Exact public AES operation; input block and key bytes never enter this key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::vector) struct Geometry {
    work: MetalBatchWork,
    blocks: usize,
}

impl Geometry {
    pub(in crate::vector) fn new(work: MetalBatchWork, blocks: usize) -> Option<Self> {
        ((MIN_BLOCKS..=MAX_BLOCKS).contains(&blocks) && (1..=MAX_ROUNDS).contains(&work.rounds()))
            .then_some(Self { work, blocks })
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::vector) struct Profile {
    geometry: Geometry,
    baseline: AesCpuBaseline,
    cpu_ns: u64,
    metal_ns: u64,
}

impl Profile {
    fn from_trials(
        geometry: Geometry,
        baseline: AesCpuBaseline,
        cpu: [[u64; TRIALS]; PATTERNS],
        metal: [[u64; TRIALS]; PATTERNS],
    ) -> Result<Self, CalibrationFailure> {
        if geometry.work != baseline.work() {
            return Err(CalibrationFailure::CpuChanged);
        }
        let bounded = |trials: [u64; TRIALS]| {
            let minimum = trials.into_iter().min().expect("fixed trial count");
            let maximum = trials.into_iter().max().expect("fixed trial count");
            (minimum > 0 && maximum <= minimum.saturating_mul(4)).then_some((minimum, maximum))
        };
        let mut cpu_ns = u64::MAX;
        let mut metal_ns = 0;
        for pattern in 0..PATTERNS {
            cpu_ns = cpu_ns.min(
                bounded(cpu[pattern])
                    .ok_or(CalibrationFailure::Overloaded)?
                    .0,
            );
            metal_ns = metal_ns.max(
                bounded(metal[pattern])
                    .ok_or(CalibrationFailure::Overloaded)?
                    .1,
            );
        }
        Ok(Self {
            geometry,
            baseline,
            cpu_ns,
            metal_ns,
        })
    }

    fn cost(self) -> Option<u64> {
        (u128::from(self.metal_ns) * 10 < u128::from(self.cpu_ns) * 9).then_some(self.metal_ns)
    }
}

/// One inline slot in each of the four original physical-owner family caches.
/// Sample buffers never enter the cache; eviction releases no live sample credit.
#[derive(Default)]
pub(in crate::vector) struct CostCache {
    profile: Option<Profile>,
    retry_after: Option<Instant>,
    quarantined: bool,
}

impl CostCache {
    pub(in crate::vector) fn qualified_cost(
        &mut self,
        now: Instant,
        geometry: Geometry,
        baseline: AesCpuBaseline,
        begin: impl FnOnce() -> Option<Instant>,
        run: impl FnOnce(Instant) -> Result<Profile, CalibrationFailure>,
    ) -> Option<u64> {
        // Quarantine takes precedence even over an otherwise exact cache hit.
        if self.quarantined || geometry.work != baseline.work() || !baseline.is_current() {
            return None;
        }
        if let Some(profile) = self
            .profile
            .filter(|profile| profile.geometry == geometry && profile.baseline == baseline)
        {
            return profile.cost();
        }
        if self.retry_after.is_some_and(|deadline| now < deadline) {
            return None;
        }
        // A busy scheduler or exhausted shared deadline is not an attempted
        // sample and must not grant this device a new cooldown penalty.
        let started = begin()?;
        // Publish the cooldown before entering a callback that may unwind.
        // CPU policy toggles cannot erase an original admitted attempt's bound.
        self.retry_after = Some(now + RETRY_AFTER);
        match run(started) {
            Ok(profile) if profile.geometry == geometry && profile.baseline == baseline => {
                if !baseline.is_current() {
                    return None;
                }
                let cost = profile.cost();
                self.profile = Some(profile);
                cost
            }
            // The comparison's CPU identity may have changed. Preserve the
            // existing profile and cooldown without quarantining a healthy GPU.
            Ok(profile) if profile.geometry == geometry => None,
            Ok(_) | Err(CalibrationFailure::ParityMismatch) => {
                self.quarantined = true;
                None
            }
            Err(
                CalibrationFailure::Deadline
                | CalibrationFailure::Allocation
                | CalibrationFailure::BackendUnavailable
                | CalibrationFailure::Overloaded
                | CalibrationFailure::CpuChanged,
            ) => None,
        }
    }
}

pub(in crate::vector) use sample::calibrate;

#[cfg(all(test, feature = "metal-hardware-tests"))]
mod qualification;
