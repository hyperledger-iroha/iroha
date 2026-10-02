//! Caller-owned vector destinations over the single physical CUDA owner.
//!
//! Failed attempts leave the destination untouched. Ordinary vector callers then
//! recompute every lane from their original inputs. Dynamic destinations must be
//! obtained from the caller's complete execution lease before invoking this API.

use super::policy::{Kernel, public_workload_task_id};
use iroha_accel::{HostOutput, PtxArtifact, cuda::CudaFailure};
use std::ffi::CStr;

#[path = "vector_launch.rs"]
mod launch;

static ARTIFACT: PtxArtifact = PtxArtifact::new(
    match CStr::from_bytes_with_nul(
        concat!(include_str!(concat!(env!("OUT_DIR"), "/vector.ptx")), "\0").as_bytes(),
    ) {
        Ok(bytes) => bytes,
        Err(_) => panic!("embedded vector PTX must have exactly one terminal NUL"),
    },
);

fn staged32(
    kernel: Kernel,
    name: &str,
    left: &[u32],
    right: &[u32],
) -> Result<HostOutput<u32>, CudaFailure> {
    let result = crate::cuda_dispatch::with_selected(kernel, ARTIFACT, |device| {
        // SAFETY: this module binds the embedded artifact to the existing vector
        // ABI; launch validates lengths and selects only its fixed u32 symbols.
        unsafe { launch::launch_u32_output(device, ARTIFACT, name, left, right) }
    });
    complete_attempt(result)
}

fn staged64(left: &[u64], right: &[u64]) -> Result<HostOutput<u64>, CudaFailure> {
    let result = crate::cuda_dispatch::with_selected(Kernel::Add64, ARTIFACT, |device| {
        // SAFETY: the exact embedded artifact supplies the fixed vadd64 ABI.
        unsafe { launch::launch_u64_output(device, ARTIFACT, left, right) }
    });
    complete_attempt(result)
}

fn complete_attempt<T>(
    result: Result<HostOutput<T>, CudaFailure>,
) -> Result<HostOutput<T>, CudaFailure> {
    match result {
        Ok(output) => {
            super::imp::record_completed_cuda_dispatch();
            Ok(output)
        }
        Err(error) => {
            if failure_quarantines(error) {
                crate::cuda_dispatch::quarantine_current_kernel();
            }
            Err(error)
        }
    }
}

fn failure_quarantines(error: CudaFailure) -> bool {
    match error {
        CudaFailure::Capacity | CudaFailure::Busy | CudaFailure::Unavailable => false,
        CudaFailure::Quarantined
        | CudaFailure::InvalidRequest
        | CudaFailure::Driver(_)
        | CudaFailure::Timeout => true,
    }
}

/// Run actual bounded public vectors on each candidate, preserving exact artifact
/// and device identity. Scalar fallback never establishes kernel admission.
pub(super) fn admit(kernel: Kernel) -> bool {
    crate::cuda_dispatch::admit_kernel(kernel, ARTIFACT, || {
        let Some(_guard) = super::imp::SelftestRunningGuard::enter() else {
            return Err(CudaFailure::Busy);
        };
        if kernel == Kernel::Add64 {
            return staged64(&[u64::MAX, 1 << 48, 1 << 63], &[2, 1 << 48, 1 << 63])
                .map(|out| out.as_slice() == [1, 1 << 49, 0]);
        }
        let (name, expected) = match kernel {
            Kernel::Add32 => ("vadd32", [0, 5, 7]),
            Kernel::And => ("vand", [1, 2, 0]),
            Kernel::Xor => ("vxor", [u32::MAX - 1, 1, 7]),
            Kernel::Or => ("vor", [u32::MAX, 3, 7]),
            _ => return Ok(false),
        };
        staged32(kernel, name, &[u32::MAX, 2, 3], &[1, 3, 4]).map(|out| out.as_slice() == expected)
    })
}

fn into32(
    kernel: Kernel,
    name: &str,
    left: &[u32],
    right: &[u32],
    destination: &mut [u32],
) -> bool {
    if left.len() != right.len() || left.len() != destination.len() {
        return false;
    }
    if destination.is_empty() {
        return true;
    }
    let task = public_workload_task_id(0x0f0f_0f0f_0000_0011, &[kernel as u64, left.len() as u64]);
    crate::cuda_dispatch::with_task_scope(task, || {
        super::imp::record_cuda_attempt();
        if !super::imp::ensure_cuda_kernel(kernel) {
            return false;
        }
        let Ok(output) = staged32(kernel, name, left, right) else {
            return false;
        };
        if output.len() != destination.len() {
            crate::cuda_dispatch::quarantine_current_kernel();
            return false;
        }
        // The complete host output retains its original charge through this copy.
        destination.copy_from_slice(output.as_slice());
        true
    })
}

/// Attempt lane-wise wrapping u32 addition into initialized caller-owned storage.
pub fn vadd32_cuda_into(left: &[u32], right: &[u32], destination: &mut [u32]) -> bool {
    into32(Kernel::Add32, "vadd32", left, right, destination)
}
/// Attempt bitwise AND into initialized caller-owned storage.
pub fn vand_cuda_into(left: &[u32], right: &[u32], destination: &mut [u32]) -> bool {
    into32(Kernel::And, "vand", left, right, destination)
}
/// Attempt bitwise XOR into initialized caller-owned storage.
pub fn vxor_cuda_into(left: &[u32], right: &[u32], destination: &mut [u32]) -> bool {
    into32(Kernel::Xor, "vxor", left, right, destination)
}
/// Attempt bitwise OR into initialized caller-owned storage.
pub fn vor_cuda_into(left: &[u32], right: &[u32], destination: &mut [u32]) -> bool {
    into32(Kernel::Or, "vor", left, right, destination)
}
/// Attempt full-width wrapping u64 addition into initialized caller-owned storage.
pub fn vadd64_cuda_into(left: &[u64], right: &[u64], destination: &mut [u64]) -> bool {
    if left.len() != right.len() || left.len() != destination.len() {
        return false;
    }
    if destination.is_empty() {
        return true;
    }
    let task = public_workload_task_id(0x0f0f_0f0f_0000_0012, &[left.len() as u64]);
    crate::cuda_dispatch::with_task_scope(task, || {
        super::imp::record_cuda_attempt();
        if !super::imp::ensure_cuda_kernel(Kernel::Add64) {
            return false;
        }
        let Ok(output) = staged64(left, right) else {
            return false;
        };
        if output.len() != destination.len() {
            crate::cuda_dispatch::quarantine_current_kernel();
            return false;
        }
        destination.copy_from_slice(output.as_slice());
        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_failure_quarantines_but_resource_pressure_only_falls_back() {
        for failure in [
            CudaFailure::Driver(2),
            CudaFailure::Timeout,
            CudaFailure::Quarantined,
            CudaFailure::InvalidRequest,
        ] {
            assert!(failure_quarantines(failure));
        }
        for refusal in [
            CudaFailure::Capacity,
            CudaFailure::Busy,
            CudaFailure::Unavailable,
        ] {
            assert!(!failure_quarantines(refusal));
        }
    }
}
