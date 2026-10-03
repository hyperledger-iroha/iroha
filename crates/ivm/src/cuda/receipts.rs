//! Bounded completion evidence owned by the original device and kernel admission.

use super::policy::Kernel;
use iroha_accel::DeviceIdentity;
#[cfg(any(feature = "cuda", test))]
use std::sync::atomic::{AtomicU64, Ordering};

/// One immutable observation of an IVM device owner's completed kernel batches.
///
/// Counts exclude admission probes, failed or invalid outputs, and CPU fallbacks.
/// They establish native completion only, not parity, performance, or release
/// qualification. They never enter gas, state, events, or commitments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaCompletionSnapshot {
    device: DeviceIdentity,
    counts: [u64; Kernel::ALL.len()],
}

impl CudaCompletionSnapshot {
    #[cfg(any(feature = "cuda", test))]
    pub(crate) fn new(device: DeviceIdentity, counts: [u64; Kernel::ALL.len()]) -> Self {
        Self { device, counts }
    }

    /// Original driver-observed UUID and driver version for this retained owner.
    pub fn device(self) -> DeviceIdentity {
        self.device
    }

    /// Successfully validated production batches for exactly this kernel.
    pub fn completed(self, kernel: Kernel) -> u64 {
        self.counts[kernel as usize]
    }
}

/// A local observation refusal, never a zero completion count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CudaCompletionError {
    /// The nonblocking policy-owner registry is currently borrowed.
    Busy,
}

#[cfg(any(feature = "cuda", test))]
#[derive(Debug, Default)]
pub(crate) struct CompletionCounter(AtomicU64);

#[cfg(any(feature = "cuda", test))]
impl CompletionCounter {
    pub(crate) fn get(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }

    pub(crate) fn record(&self) {
        // Telemetry must never overflow or affect operation acceptance.
        let _ = self
            .0
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                Some(count.saturating_add(1))
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immutable_snapshots_preserve_every_kernel_and_original_driver_identity() {
        let device = DeviceIdentity {
            uuid: [7; 16],
            driver_version: 12000,
        };
        let counts = std::array::from_fn(|index| index as u64 + 1);
        let snapshot = CudaCompletionSnapshot::new(device, counts);
        assert_eq!(snapshot.device(), device);
        for (index, kernel) in Kernel::ALL.into_iter().enumerate() {
            assert_eq!(snapshot.completed(kernel), index as u64 + 1);
        }
        assert_ne!(
            snapshot,
            CudaCompletionSnapshot::new(
                DeviceIdentity {
                    uuid: [8; 16],
                    ..device
                },
                counts,
            )
        );
        assert_ne!(
            snapshot,
            CudaCompletionSnapshot::new(
                DeviceIdentity {
                    driver_version: 13000,
                    ..device
                },
                counts,
            )
        );
    }

    #[test]
    fn invalid_native_outputs_cannot_credit_the_original_kernel_owner() {
        let entry = CompletionCounter::default();
        let record = || entry.record();
        assert!(!crate::cuda::output_validation::bn254(
            &[crate::bn254_vec::MODULUS],
            1,
            record
        ));
        assert!(!crate::cuda::output_validation::poseidon(
            [1, 0],
            Some(&[0; 12]),
            3,
            1,
            record
        ));
        assert_eq!(entry.get(), 0);
        assert!(crate::cuda::output_validation::bn254(&[[0; 4]], 1, record));
        assert_eq!(entry.get(), 1);
    }

    #[test]
    fn concurrent_completion_counts_are_original_owner_bound_and_saturating() {
        let original = CompletionCounter::default();
        let other = CompletionCounter::default();
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    for _ in 0..100 {
                        original.record();
                    }
                });
            }
        });
        assert_eq!(original.get(), 400);
        assert_eq!(other.get(), 0);
        original.0.store(u64::MAX - 1, Ordering::Relaxed);
        original.record();
        original.record();
        assert_eq!(original.get(), u64::MAX);
    }
}
