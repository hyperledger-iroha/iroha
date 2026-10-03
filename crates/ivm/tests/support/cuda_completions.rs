//! Exact owner/kernel completion observations for native parity controls.

use ivm::{CudaCompletionSnapshot, CudaKernel};

/// Read every observed stable slot without interpreting local contention as zero.
pub fn capture() -> Vec<Option<CudaCompletionSnapshot>> {
    (0..ivm::cuda_device_slots())
        .map(|slot| {
            ivm::cuda_completion_snapshot(slot).expect("completion registry must be readable")
        })
        .collect()
}

/// Require increased completion credit for this kernel on the same original owner.
pub fn increased(before: &[Option<CudaCompletionSnapshot>], kernel: CudaKernel) -> bool {
    before.iter().enumerate().any(|(slot, previous)| {
        let Some(after) =
            ivm::cuda_completion_snapshot(slot).expect("completion registry must be readable")
        else {
            return false;
        };
        if let Some(previous) = previous {
            assert_eq!(
                previous.device(),
                after.device(),
                "physical owner cannot be replaced"
            );
        }
        after.completed(kernel) > previous.map_or(0, |value| value.completed(kernel))
    })
}

/// Verify no kernel gained credit, including policy owners initialized after capture.
pub fn unchanged(before: &[Option<CudaCompletionSnapshot>]) -> bool {
    let after = capture();
    before.len() == after.len()
        && before
            .iter()
            .zip(after)
            .all(|(previous, current)| match (previous, current) {
                (Some(previous), Some(current)) => previous == &current,
                (None, Some(current)) => CudaKernel::ALL
                    .into_iter()
                    .all(|kernel| current.completed(kernel) == 0),
                (None, None) => true,
                (Some(_), None) => false,
            })
}
