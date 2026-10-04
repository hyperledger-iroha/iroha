//! Exact public message geometry and whole-operation Ed25519 cost custody.
//!
//! One inline profile belongs to its charged physical Metal state. The CPU
//! baseline is the fixed compiled strict dalek/SHA-512 traversal, whose internal
//! ISA dispatch is process-lived and does not use the IVM vector SIMD override.

use crate::signature::ed25519_geometry::{GeometryKey, MessageGeometry};
use std::time::{Duration, Instant};

mod sample;
#[cfg(test)]
mod tests;

const TRIALS: usize = 3;
const MAX_CALIBRATION: Duration = Duration::from_secs(8);
const RETRY_AFTER: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Failure {
    Deadline,
    Allocation,
    Backend,
    Overloaded,
    CpuMismatch,
    Parity,
}

#[derive(Debug)]
pub(super) struct Profile {
    geometry: GeometryKey,
    cpu_ns: u64,
    metal_ns: u64,
}
impl Profile {
    fn from_trials(
        geometry: MessageGeometry<'_, '_>,
        cpu: [u64; TRIALS],
        metal: [u64; TRIALS],
    ) -> Result<Self, Failure> {
        let bounded = |values: [u64; TRIALS]| {
            let min = values.into_iter().min().expect("fixed trial count");
            let max = values.into_iter().max().expect("fixed trial count");
            (min > 0 && max <= min.saturating_mul(4)).then_some((min, max))
        };
        let (cpu_ns, _) = bounded(cpu).ok_or(Failure::Overloaded)?;
        let (_, metal_ns) = bounded(metal).ok_or(Failure::Overloaded)?;
        Ok(Self {
            geometry: GeometryKey::capture(geometry),
            cpu_ns,
            metal_ns,
        })
    }
    fn cost(&self) -> Option<u64> {
        (u128::from(self.metal_ns) * 10 < u128::from(self.cpu_ns) * 9).then_some(self.metal_ns)
    }
}

#[derive(Default)]
pub(super) struct CostCache {
    profile: Option<Profile>,
    retry_after: Option<Instant>,
    quarantined: bool,
}
impl CostCache {
    /// Lookup borrows caller lengths and never copies message contents. Record
    /// a cooldown before entering a callback so unwind cannot erase the attempt.
    pub(super) fn qualified_cost(
        &mut self,
        now: Instant,
        geometry: MessageGeometry<'_, '_>,
        begin: impl FnOnce() -> Option<Instant>,
        run: impl FnOnce(Instant) -> Result<Profile, Failure>,
    ) -> Option<u64> {
        if self.quarantined {
            return None;
        }
        if let Some(profile) = self
            .profile
            .as_ref()
            .filter(|profile| profile.geometry.matches(geometry))
        {
            return profile.cost();
        }
        if self.retry_after.is_some_and(|deadline| now < deadline) {
            return None;
        }
        let started = begin()?;
        self.retry_after = Some(now + RETRY_AFTER);
        match run(started) {
            Ok(profile) if profile.geometry.matches(geometry) => {
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
                | Failure::Overloaded
                | Failure::CpuMismatch,
            ) => None,
        }
    }
}

pub(super) use sample::calibrate;

#[cfg(all(test, feature = "metal-hardware-tests"))]
mod qualification;
