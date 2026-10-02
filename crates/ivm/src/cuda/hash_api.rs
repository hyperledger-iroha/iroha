//! Fixed-state hash publication through the shared physical CUDA owner.

use super::policy::Kernel;
use iroha_accel::{HostOutput, PtxArtifact, cuda::CudaFailure};
use std::ffi::CStr;

#[path = "hash_launch.rs"]
mod launch;

static SHA256: PtxArtifact = PtxArtifact::new(
    match CStr::from_bytes_with_nul(
        concat!(include_str!(concat!(env!("OUT_DIR"), "/sha256.ptx")), "\0").as_bytes(),
    ) {
        Ok(bytes) => bytes,
        Err(_) => panic!("embedded SHA-256 artifact must contain exactly one terminal NUL"),
    },
);
static KECCAK: PtxArtifact = PtxArtifact::new(
    match CStr::from_bytes_with_nul(
        concat!(include_str!(concat!(env!("OUT_DIR"), "/sha3.ptx")), "\0").as_bytes(),
    ) {
        Ok(bytes) => bytes,
        Err(_) => panic!("embedded Keccak artifact must contain exactly one terminal NUL"),
    },
);

fn complete<T>(result: Result<HostOutput<T>, CudaFailure>) -> Result<HostOutput<T>, CudaFailure> {
    match result {
        Ok(output) => {
            super::imp::record_completed_cuda_dispatch();
            Ok(output)
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

fn sha256_staging(state: &[u32; 8], block: &[u8; 64]) -> Result<HostOutput<u32>, CudaFailure> {
    complete(crate::cuda_dispatch::with_selected(
        Kernel::Sha256,
        SHA256,
        |device| {
            // SAFETY: this module supplies the exact immutable artifact and fixed ABI.
            unsafe { launch::sha256_output(device, SHA256, state, block) }
        },
    ))
}
fn keccak_staging(state: &[u64; 25]) -> Result<HostOutput<u64>, CudaFailure> {
    complete(crate::cuda_dispatch::with_selected(
        Kernel::Keccak,
        KECCAK,
        |device| {
            // SAFETY: this module supplies the exact immutable artifact and fixed ABI.
            unsafe { launch::keccak_output(device, KECCAK, state) }
        },
    ))
}

pub(super) fn admit(kernel: Kernel) -> bool {
    let artifact = match kernel {
        Kernel::Sha256 => SHA256,
        Kernel::Keccak => KECCAK,
        _ => return false,
    };
    crate::cuda_dispatch::admit_kernel(kernel, artifact, || {
        let Some(_guard) = super::imp::SelftestRunningGuard::enter() else {
            return Err(CudaFailure::Busy);
        };
        match kernel {
            Kernel::Sha256 => {
                let initial = [
                    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c,
                    0x1f83d9ab, 0x5be0cd19,
                ];
                let mut block = [0u8; 64];
                block[..3].copy_from_slice(b"abc");
                block[3] = 0x80;
                block[63] = 24;
                let mut expected = initial;
                crate::sha256_ref::sha256_compress_scalar_ref(&mut expected, &block);
                sha256_staging(&initial, &block).map(|output| output.as_slice() == expected)
            }
            Kernel::Keccak => {
                let initial = std::array::from_fn(|index| index as u64 * 0x0101_0101_0101_0101);
                let mut expected = initial;
                crate::sha3::keccak_f1600_impl(&mut expected);
                keccak_staging(&initial).map(|output| output.as_slice() == expected)
            }
            _ => Ok(false),
        }
    })
}

/// Attempt one compression round; failure leaves the complete original state intact.
pub fn sha256_compress_cuda(state: &mut [u32; 8], block: &[u8; 64]) -> bool {
    crate::cuda_dispatch::with_task_scope(0x0f0f_0f0f_0000_0020, || {
        super::imp::record_cuda_attempt();
        if !super::imp::ensure_cuda_kernel(Kernel::Sha256) {
            return false;
        }
        let Ok(output) = sha256_staging(state, block) else {
            return false;
        };
        if output.len() != state.len() {
            crate::cuda_dispatch::quarantine_current_kernel();
            return false;
        }
        state.copy_from_slice(output.as_slice());
        true
    })
}

/// Attempt one permutation; failure leaves the complete original state intact.
pub fn keccak_f1600_cuda(state: &mut [u64; 25]) -> bool {
    crate::cuda_dispatch::with_task_scope(0x0f0f_0f0f_0000_0040, || {
        super::imp::record_cuda_attempt();
        if !super::imp::ensure_cuda_kernel(Kernel::Keccak) {
            return false;
        }
        let Ok(output) = keccak_staging(state) else {
            return false;
        };
        if output.len() != state.len() {
            crate::cuda_dispatch::quarantine_current_kernel();
            return false;
        }
        state.copy_from_slice(output.as_slice());
        true
    })
}
