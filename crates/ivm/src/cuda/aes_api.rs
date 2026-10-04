//! AES kernel qualification and caller-owned batch publication.

use super::policy::{Kernel, public_workload_task_id};
use iroha_accel::{HostOutput, cuda::CudaFailure};

#[path = "aes_launch.rs"]
mod launch;

fn stage(
    kernel: Kernel,
    states: &[[u8; 16]],
    keys: &[[u8; 16]],
) -> Result<HostOutput<[u8; 16]>, CudaFailure> {
    let (decrypt, fused) = match kernel {
        Kernel::AesEnc => (false, false),
        Kernel::AesDec => (true, false),
        Kernel::AesEncFused => (false, true),
        Kernel::AesDecFused => (true, true),
        _ => return Err(CudaFailure::InvalidRequest),
    };
    let artifact = crate::cuda_artifact::artifact(kernel)?;
    match crate::cuda_dispatch::with_selected(kernel, artifact, |device| {
        // SAFETY: the exact artifact, fixed symbols and public checked geometry
        // are owned here. Input and key arrays stay unchanged through completion.
        unsafe { launch::aes_output(device, artifact, decrypt, fused, states, keys) }
    }) {
        Ok(output) if output.len() == states.len() && !states.is_empty() => {
            super::imp::record_completed_cuda_dispatch(kernel, artifact);
            Ok(output)
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

pub(super) fn admit(kernel: Kernel) -> bool {
    let Ok(artifact) = crate::cuda_artifact::artifact(kernel) else {
        return false;
    };
    crate::cuda_dispatch::admit_kernel(kernel, artifact, || {
        let Some(_guard) = super::imp::SelftestRunningGuard::enter() else {
            return Err(CudaFailure::Busy);
        };
        let states = [[0; 16], [0xff; 16]];
        let keys = [[0x3c; 16], [0xc3; 16]];
        let (decrypt, rounds) = match kernel {
            Kernel::AesEnc => (false, 1),
            Kernel::AesDec => (true, 1),
            Kernel::AesEncFused => (false, 2),
            Kernel::AesDecFused => (true, 2),
            _ => return Ok(false),
        };
        let mut expected = states;
        for key in &keys[..rounds] {
            for block in &mut expected {
                *block = if decrypt {
                    crate::aes::aesdec_impl(*block, *key)
                } else {
                    crate::aes::aesenc_impl(*block, *key)
                };
            }
        }
        stage(kernel, &states, &keys[..rounds]).map(|output| output.as_slice() == expected)
    })
}

/// A charged native result for an already funded caller destination. Internal
/// consumers may keep this owner through an in-place publish without aliasing
/// the original input borrow and mutable caller destination.
pub(crate) fn attempt(
    kernel: Kernel,
    states: &[[u8; 16]],
    keys: &[[u8; 16]],
) -> Option<HostOutput<[u8; 16]>> {
    let task = public_workload_task_id(
        0x0f0f_0f0f_0000_0050,
        &[kernel as u64, states.len() as u64, keys.len() as u64],
    );
    crate::cuda_dispatch::with_task_scope(task, || {
        super::imp::record_cuda_attempt();
        if !super::imp::ensure_cuda_kernel(kernel) {
            return None;
        }
        stage(kernel, states, keys).ok()
    })
}

fn into(
    kernel: Kernel,
    states: &[[u8; 16]],
    keys: &[[u8; 16]],
    destination: &mut [[u8; 16]],
) -> bool {
    if states.len() != destination.len() {
        return false;
    }
    if destination.is_empty() {
        return true;
    }
    if keys.is_empty() {
        destination.copy_from_slice(states);
        return true;
    }
    let Some(output) = attempt(kernel, states, keys) else {
        return false;
    };
    if output.len() != destination.len() {
        return false;
    }
    destination.copy_from_slice(output.as_slice());
    true
}

/// Attempt one AESENC round for each block; failure preserves the destination.
pub fn aesenc_batch_cuda_into(
    states: &[[u8; 16]],
    key: [u8; 16],
    destination: &mut [[u8; 16]],
) -> bool {
    into(Kernel::AesEnc, states, &[key], destination)
}
/// Attempt one AESDEC round for each block; failure preserves the destination.
pub fn aesdec_batch_cuda_into(
    states: &[[u8; 16]],
    key: [u8; 16],
    destination: &mut [[u8; 16]],
) -> bool {
    into(Kernel::AesDec, states, &[key], destination)
}
/// Attempt ordered AESENC rounds; failure preserves the destination.
pub fn aesenc_rounds_batch_cuda_into(
    states: &[[u8; 16]],
    keys: &[[u8; 16]],
    destination: &mut [[u8; 16]],
) -> bool {
    into(Kernel::AesEncFused, states, keys, destination)
}
/// Attempt ordered AESDEC rounds; failure preserves the destination.
pub fn aesdec_rounds_batch_cuda_into(
    states: &[[u8; 16]],
    keys: &[[u8; 16]],
    destination: &mut [[u8; 16]],
) -> bool {
    into(Kernel::AesDecFused, states, keys, destination)
}
/// Attempt a single AESENC round; returns only completed native output.
pub fn aesenc_cuda(state: [u8; 16], key: [u8; 16]) -> Option<[u8; 16]> {
    let mut output = [[0; 16]];
    aesenc_batch_cuda_into(&[state], key, &mut output).then_some(output[0])
}
/// Attempt a single AESDEC round; returns only completed native output.
pub fn aesdec_cuda(state: [u8; 16], key: [u8; 16]) -> Option<[u8; 16]> {
    let mut output = [[0; 16]];
    aesdec_batch_cuda_into(&[state], key, &mut output).then_some(output[0])
}
