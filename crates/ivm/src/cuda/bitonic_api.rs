//! Bitonic qualification and all-or-nothing publication to original caller buffers.

use super::policy::{Kernel, public_workload_task_id};
use iroha_accel::cuda::CudaFailure;
#[path = "bitonic_launch.rs"]
mod launch;

fn stage(hi: &[u64], lo: &[u64]) -> Result<launch::Sorted, CudaFailure> {
    let artifact = crate::cuda_artifact::artifact(Kernel::Bitonic)?;
    match crate::cuda_dispatch::with_selected(Kernel::Bitonic, artifact, |device| {
        // SAFETY: this module owns the exact artifact and checked paired arrays.
        unsafe { launch::output(device, artifact, hi, lo) }
    }) {
        Ok(result) if result.hi.len() == hi.len() && result.lo.len() == lo.len() => {
            super::imp::record_completed_cuda_dispatch(Kernel::Bitonic, artifact);
            Ok(result)
        }
        Ok(_) => {
            crate::cuda_dispatch::quarantine_current_kernel();
            Err(CudaFailure::Quarantined)
        }
        Err(error) => {
            if !matches!(
                error,
                CudaFailure::Capacity | CudaFailure::Busy | CudaFailure::Unavailable
            ) {
                crate::cuda_dispatch::quarantine_current_kernel();
            }
            Err(error)
        }
    }
}

pub(super) fn admit() -> bool {
    let Ok(artifact) = crate::cuda_artifact::artifact(Kernel::Bitonic) else {
        return false;
    };
    crate::cuda_dispatch::admit_kernel(Kernel::Bitonic, artifact, || {
        let Some(_guard) = super::imp::SelftestRunningGuard::enter() else {
            return Err(CudaFailure::Busy);
        };
        let hi = [u64::MAX, 2, 1, 2, 0];
        let lo = [u64::MAX, 7, 9, 3, u64::MAX];
        stage(&hi, &lo).map(|result| {
            result.hi.as_slice() == [0, 1, 2, 2, u64::MAX]
                && result.lo.as_slice() == [u64::MAX, 9, 3, 7, u64::MAX]
        })
    })
}

/// Attempt lexicographic sorting through the qualified native kernel. Both
/// caller buffers remain unchanged unless the complete native result succeeds.
/// Empty and singleton pairs need no kernel and succeed without native work.
pub fn bitonic_sort_pairs(hi: &mut [u64], lo: &mut [u64]) -> Option<()> {
    if hi.len() != lo.len() {
        return None;
    }
    if hi.len() < 2 {
        return Some(());
    }
    let (padded, _) = launch::request(hi.len())?;
    let task =
        public_workload_task_id(0x0f0f_0f0f_0000_0002, &[hi.len() as u64, u64::from(padded)]);
    crate::cuda_dispatch::with_task_scope(task, || {
        super::imp::record_cuda_attempt();
        if !super::imp::ensure_cuda_kernel(Kernel::Bitonic) {
            return None;
        }
        let result = stage(hi, lo).ok()?;
        hi.copy_from_slice(&result.hi);
        lo.copy_from_slice(&result.lo);
        Some(())
    })
}
