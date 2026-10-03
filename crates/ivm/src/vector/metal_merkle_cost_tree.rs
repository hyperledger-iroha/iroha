//! Exact tree-construction geometry and actual parallel CPU baseline custody.
//!
//! Root-only and retained-tree rehash operations have different allocation and
//! traversal costs. Neither can consume this complete BuildTree profile.

use super::sha256_cpu::context::{Sha256Baseline, Sha256Context};
use std::time::{Duration, Instant};

mod sample;
#[cfg(test)]
mod tests;

const TRIALS: usize = 3;
const MAX_CALIBRATION: Duration = Duration::from_secs(8);
const RETRY_AFTER: Duration = Duration::from_secs(30);

/// Exact ordered public byte extent, leaf width and final partial leaf.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Geometry {
    byte_len: usize,
    chunk: usize,
    leaves: usize,
}
impl Geometry {
    pub(super) fn new(byte_len: usize, chunk: usize) -> Option<Self> {
        if !(1..=32).contains(&chunk) {
            return None;
        }
        let leaves = byte_len.div_ceil(chunk).max(1);
        (8_192..=65_536).contains(&leaves).then_some(Self {
            byte_len,
            chunk,
            leaves,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Failure {
    Deadline,
    Allocation,
    Backend,
    CpuChanged,
    Overloaded,
    Parity,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Profile {
    geometry: Geometry,
    baseline: Sha256Baseline,
    cpu_ns: u64,
    metal_ns: u64,
}
impl Profile {
    fn from_trials(
        geometry: Geometry,
        baseline: Sha256Baseline,
        cpu: [[u64; TRIALS]; 2],
        metal: [[u64; TRIALS]; 2],
    ) -> Result<Self, Failure> {
        let bounded = |trials: [u64; TRIALS]| {
            let min = trials.into_iter().min().expect("fixed trial count");
            let max = trials.into_iter().max().expect("fixed trial count");
            (min > 0 && max <= min.saturating_mul(4)).then_some((min, max))
        };
        let mut cpu_ns = u64::MAX;
        let mut metal_ns = 0;
        for pattern in 0..2 {
            cpu_ns = cpu_ns.min(bounded(cpu[pattern]).ok_or(Failure::Overloaded)?.0);
            metal_ns = metal_ns.max(bounded(metal[pattern]).ok_or(Failure::Overloaded)?.1);
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

/// Inline, bounded state belongs to one original charged physical Metal owner.
#[derive(Default)]
pub(super) struct CostCache {
    profile: Option<Profile>,
    retry_after: Option<Instant>,
    quarantined: bool,
}
impl CostCache {
    pub(super) fn qualified_cost(
        &mut self,
        now: Instant,
        geometry: Geometry,
        baseline: Sha256Baseline,
        context: Sha256Context,
        begin: impl FnOnce() -> Option<Instant>,
        run: impl FnOnce(Instant) -> Result<Profile, Failure>,
    ) -> Option<u64> {
        if self.quarantined || !baseline.is_current(context) {
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
        let started = begin()?;
        self.retry_after = Some(now + RETRY_AFTER);
        match run(started) {
            Ok(profile) if profile.geometry == geometry && profile.baseline == baseline => {
                if !baseline.is_current(context) {
                    return None;
                }
                let cost = profile.cost();
                self.profile = Some(profile);
                cost
            }
            Ok(_) | Err(Failure::Parity) => {
                self.quarantined = true;
                None
            }
            Err(
                Failure::Deadline
                | Failure::Allocation
                | Failure::Backend
                | Failure::CpuChanged
                | Failure::Overloaded,
            ) => None,
        }
    }
}

pub(super) use sample::calibrate;

#[cfg(all(test, feature = "metal-hardware-tests"))]
mod qualification;
