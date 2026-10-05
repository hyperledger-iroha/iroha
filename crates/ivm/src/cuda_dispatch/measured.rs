//! Cost selection retains the original qualified physical and kernel owners.

use super::*;
use crate::{
    cuda_cost::{
        CalibrationFailure, CalibrationScheduler, CostProfile, ProfileKey, family,
        geometry_supported,
    },
    field_dispatch::{FieldArithmetic, field_impl},
};
use std::{any::TypeId, time::Instant};

static CALIBRATION: CalibrationScheduler = CalibrationScheduler::new();

/// Only selection may construct this original-owner token. It cannot be replaced
/// by a slot number, a same-shaped profile or a later independently chosen device.
pub(crate) struct Selected {
    active: ActiveDevice,
    cpu: TypeId,
}

impl Selected {
    pub(crate) fn valid(&self) -> bool {
        if !ENABLED.load(Ordering::Acquire) || field_impl().type_id() != self.cpu {
            return false;
        }
        let Some(cap) = DEVICE_CAP.try_lock() else {
            return false;
        };
        !cap.is_some_and(|limit| self.active.index >= limit)
            && self.active.device.usable()
            && self.active.policy.kernels[self.active.kernel as usize]
                .admitted(self.active.artifact)
    }

    pub(crate) fn run<T>(&self, call: impl FnOnce(&Self) -> T) -> Option<T> {
        if !self.valid() {
            return None;
        }
        Some(with_active(self.active.clone(), || call(self)))
    }
}

fn with_active<T>(active: ActiveDevice, call: impl FnOnce() -> T) -> T {
    struct Restore(Option<ActiveDevice>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE.with(|slot| *slot.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(ACTIVE.with(|slot| slot.replace(Some(active))));
    call()
}

pub(crate) fn select(
    kernel: Kernel,
    artifact: PtxArtifact,
    items: usize,
    cpu: &'static dyn FieldArithmetic,
    admit: impl Fn() -> bool,
    calibrate: impl Fn(Instant) -> Result<CostProfile, CalibrationFailure>,
) -> Option<Selected> {
    let family = family(kernel)?;
    if !geometry_supported(items)
        || !ENABLED.load(Ordering::Acquire)
        || cpu.type_id() != field_impl().type_id()
    {
        return None;
    }
    let process = physical_process()?;
    if matches!(process.discover(), Err(CudaFailure::Busy)) {
        return None;
    }
    let count = selection_count(process)?;
    if count == 0 {
        return None;
    }
    let pinned = ACTIVE
        .with(|slot| slot.borrow().as_ref().map(|active| active.index))
        .or_else(qualification_device);
    let now = Instant::now();
    let mut pass = CALIBRATION.try_pass(count, now);
    let start = pass.as_ref().map_or(0, |pass| pass.start);
    let key = ProfileKey {
        artifact,
        cpu: cpu.type_id(),
    };
    let mut best: Option<(u64, Selected)> = None;
    // Stable finite record capacity bounds this scan. Discovery and every lock
    // are nonblocking; only one new candidate consumes this call's public pass.
    for index in (start..count).chain(0..start) {
        if pinned.is_some_and(|pinned| pinned != index) {
            continue;
        }
        let Some(device) = process.device(index) else {
            continue;
        };
        let Some(policy) = policy_for(&device, index) else {
            continue;
        };
        if !device.usable() || !policy.kernels[kernel as usize].can_attempt(artifact) {
            continue;
        }
        let active = ActiveDevice {
            device,
            policy: policy.clone(),
            index,
            kernel,
            artifact,
            qualifying: false,
        };
        let estimate = with_active(active.clone(), || {
            policy.measured_costs[family].estimate(
                key,
                items,
                Instant::now(),
                pass.as_mut(),
                |deadline| {
                    // Admission is executed only within the finite calibration pass,
                    // and its nested dispatcher is pinned by this original ACTIVE.
                    if Instant::now() >= deadline {
                        return Err(CalibrationFailure::Deadline);
                    }
                    if !admit() {
                        return Err(CalibrationFailure::Deferred);
                    }
                    calibrate(deadline)
                },
            )
        });
        if let Some(ns) = estimate {
            let selected = Selected {
                active,
                cpu: key.cpu,
            };
            if selected.valid() && best.as_ref().is_none_or(|(old, _)| ns < *old) {
                best = Some((ns, selected));
            }
        }
    }
    best.map(|(_, selected)| selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_families_and_unmeasured_geometry_never_probe_or_calibrate() {
        let cpu = field_impl();
        let artifact = PtxArtifact::new(c"selection-does-not-execute");
        for (kernel, items) in [
            (Kernel::Add32, 64),
            (Kernel::Poseidon2, 63),
            (Kernel::Poseidon6, 4097),
        ] {
            assert!(
                select(
                    kernel,
                    artifact,
                    items,
                    cpu,
                    || panic!("no admission"),
                    |_| panic!("no calibration")
                )
                .is_none()
            );
        }
    }
}
