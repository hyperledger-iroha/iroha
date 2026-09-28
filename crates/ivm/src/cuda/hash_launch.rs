//! Native fixed-state hash attempts owned by the shared process registry.
//!
//! Original state and block remain in caller custody; only charged completed host
//! staging escapes. The caller publishes after checked native cleanup completes.

use iroha_accel::{
    HostOutput, PtxArtifact,
    cuda::{CudaDevice, CudaFailure, WorkRequest},
};
use std::{ffi::c_void, mem::size_of_val};

/// Stage one SHA-256 compression using the exact admitted two-pointer kernel ABI.
///
/// # Safety
/// The supplied artifact must be the qualified SHA-256 compression artifact.
pub(super) unsafe fn sha256_output(
    device: &CudaDevice<'_>,
    artifact: PtxArtifact,
    state: &[u32; 8],
    block: &[u8; 64],
) -> Result<HostOutput<u32>, CudaFailure> {
    let bytes = size_of_val(state) + size_of_val(block);
    let work = device.prepare(
        &[artifact],
        WorkRequest {
            host_bytes: size_of_val(state),
            pinned_bytes: bytes,
            device_bytes: bytes,
        },
    )?;
    let mut device_state = work.buffer::<u32>(state.len())?;
    let mut device_block = work.buffer::<u8>(block.len())?;
    work.upload(&mut device_state, state)?;
    work.wait()?;
    work.upload(&mut device_block, block)?;
    work.wait()?;
    let mut state_pointer = device_state.device_pointer();
    let mut block_pointer = device_block.device_pointer();
    let mut arguments = [
        (&raw mut state_pointer).cast::<c_void>(),
        (&raw mut block_pointer).cast::<c_void>(),
    ];
    // SAFETY: fixed ABI and one-thread geometry address the exact admitted buffers.
    unsafe {
        work.launch(
            artifact,
            c"sha256_compress",
            [1; 3],
            [1; 3],
            0,
            &mut arguments,
        )?;
    }
    work.wait()?;
    // SAFETY: the qualified compression kernel initializes all eight u32 words.
    unsafe { work.download(&mut device_state) }
}

/// Stage Keccak-f1600 using the exact admitted single-pointer kernel ABI.
///
/// # Safety
/// The supplied artifact must be the qualified Keccak-f1600 artifact.
pub(super) unsafe fn keccak_output(
    device: &CudaDevice<'_>,
    artifact: PtxArtifact,
    state: &[u64; 25],
) -> Result<HostOutput<u64>, CudaFailure> {
    let bytes = size_of_val(state);
    let work = device.prepare(
        &[artifact],
        WorkRequest {
            host_bytes: bytes,
            pinned_bytes: bytes,
            device_bytes: bytes,
        },
    )?;
    let mut device_state = work.buffer::<u64>(state.len())?;
    work.upload(&mut device_state, state)?;
    work.wait()?;
    let mut state_pointer = device_state.device_pointer();
    let mut arguments = [(&raw mut state_pointer).cast::<c_void>()];
    // SAFETY: fixed ABI and one-thread geometry address exactly twenty-five words.
    unsafe {
        work.launch(
            artifact,
            c"keccak_f1600_cuda",
            [1; 3],
            [1; 3],
            0,
            &mut arguments,
        )?;
    }
    work.wait()?;
    // SAFETY: Keccak overwrites the complete state with initialized u64 words.
    unsafe { work.download(&mut device_state) }
}
