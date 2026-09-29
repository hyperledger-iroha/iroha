//! Complete leaf/reduction requests and charged Merkle native results.

use iroha_accel::{
    HostOutput, PtxArtifact,
    cuda::{CudaBuffer, CudaDevice, CudaFailure, CudaWork, WorkRequest},
};
use std::ffi::c_void;

fn geometry(count: usize, leaves: bool, reduce: bool) -> Option<WorkRequest> {
    let count32 = u32::try_from(count).ok()?;
    if count32 == 0 {
        return None;
    }
    let digests = count.checked_mul(32)?;
    let scratch = if reduce {
        count.div_ceil(2).checked_mul(32)?
    } else {
        0
    };
    let blocks = if leaves { count.checked_mul(64)? } else { 0 };
    let buffers = digests.checked_add(scratch)?.checked_add(blocks)?;
    Some(WorkRequest {
        host_bytes: if reduce { 32 } else { digests },
        pinned_bytes: buffers,
        device_bytes: buffers,
    })
}

unsafe fn leaves(
    work: &CudaWork,
    artifact: PtxArtifact,
    input: &CudaBuffer<'_, [u8; 64]>,
    output: &CudaBuffer<'_, [u8; 32]>,
    count: u32,
) -> Result<(), CudaFailure> {
    let mut input_pointer = input.device_pointer();
    let mut output_pointer = output.device_pointer();
    let mut count = count;
    let mut arguments = [
        (&raw mut input_pointer).cast::<c_void>(),
        (&raw mut output_pointer).cast::<c_void>(),
        (&raw mut count).cast::<c_void>(),
    ];
    // SAFETY: fixed SHA leaf ABI with exactly count contiguous blocks and digests.
    unsafe {
        work.launch(
            artifact,
            c"sha256_leaves",
            [count.div_ceil(256), 1, 1],
            [256, 1, 1],
            0,
            &mut arguments,
        )
    }
}

unsafe fn reduce<'work>(
    work: &CudaWork,
    artifact: PtxArtifact,
    initial: &mut CudaBuffer<'work, [u8; 32]>,
    scratch: &mut CudaBuffer<'work, [u8; 32]>,
    mut count: u32,
) -> Result<HostOutput<[u8; 32]>, CudaFailure> {
    let mut initial_is_current = true;
    while count > 1 {
        let next_count = count.div_ceil(2);
        let (mut input, mut output) = if initial_is_current {
            (initial.device_pointer(), scratch.device_pointer())
        } else {
            (scratch.device_pointer(), initial.device_pointer())
        };
        let mut arguments = [
            (&raw mut input).cast::<c_void>(),
            (&raw mut output).cast::<c_void>(),
            (&raw mut count).cast::<c_void>(),
        ];
        // SAFETY: each level writes ceil(count/2) initialized digests into a
        // separate buffer with sufficient capacity. Commands share one stream.
        unsafe {
            work.launch(
                artifact,
                c"sha256_pairs_reduce",
                [next_count.div_ceil(256), 1, 1],
                [256, 1, 1],
                0,
                &mut arguments,
            )?;
        }
        count = next_count;
        initial_is_current = !initial_is_current;
    }
    work.wait()?;
    let final_buffer = if initial_is_current { initial } else { scratch };
    // SAFETY: reduction initialized the first digest; tails need not be initialized.
    unsafe { work.download_prefix(final_buffer, 1) }
}

/// Attempt a complete padded-leaf hash into charged private output.
/// # Safety
/// The artifact must be the exact admitted SHA leaf kernel for this fixed ABI.
pub(super) unsafe fn leaves_output(
    device: &CudaDevice<'_>,
    artifact: PtxArtifact,
    blocks: &[[u8; 64]],
) -> Result<HostOutput<[u8; 32]>, CudaFailure> {
    // SAFETY: the same artifact and initialized input contract is forwarded.
    unsafe { leaves_generated_output(device, artifact, blocks.len(), |index| blocks[index]) }
}

/// Hash generated padded blocks directly in already reserved pinned custody.
/// # Safety
/// The exact artifact must be admitted; each generated array is one fully padded
/// single-block message using the same canonical SHA-256 encoding as the CPU.
pub(super) unsafe fn leaves_generated_output(
    device: &CudaDevice<'_>,
    artifact: PtxArtifact,
    count: usize,
    input_block: impl FnMut(usize) -> [u8; 64],
) -> Result<HostOutput<[u8; 32]>, CudaFailure> {
    let request = geometry(count, true, false).ok_or(CudaFailure::InvalidRequest)?;
    let work = device.prepare(&[artifact], request)?;
    let mut input = work.buffer::<[u8; 64]>(count)?;
    let mut output = work.buffer::<[u8; 32]>(count)?;
    work.upload_generated(&mut input, input_block)?;
    work.wait()?;
    unsafe {
        leaves(&work, artifact, &input, &output, count as u32)?;
    }
    work.wait()?;
    // SAFETY: the admitted leaf kernel initialized all digests.
    unsafe { work.download(&mut output) }
}

/// Attempt complete pair reduction, preserving the original input digests.
/// # Safety
/// The artifact must be the exact admitted SHA pair kernel for this fixed ABI.
pub(super) unsafe fn pairs_output(
    device: &CudaDevice<'_>,
    artifact: PtxArtifact,
    digests: &[[u8; 32]],
) -> Result<HostOutput<[u8; 32]>, CudaFailure> {
    let request = geometry(digests.len(), false, true).ok_or(CudaFailure::InvalidRequest)?;
    let work = device.prepare(&[artifact], request)?;
    let mut initial = work.buffer::<[u8; 32]>(digests.len())?;
    let mut scratch = work.buffer::<[u8; 32]>(digests.len().div_ceil(2))?;
    work.upload(&mut initial, digests)?;
    work.wait()?;
    unsafe {
        reduce(
            &work,
            artifact,
            &mut initial,
            &mut scratch,
            digests.len() as u32,
        )
    }
}

/// Hash leaves and reduce all levels through one complete prepaid attempt.
/// # Safety
/// Both exact artifacts and their fixed ABIs must be admitted on this device.
pub(super) unsafe fn root_output(
    device: &CudaDevice<'_>,
    leaves_artifact: PtxArtifact,
    pairs_artifact: PtxArtifact,
    count: usize,
    input_block: impl FnMut(usize) -> [u8; 64],
) -> Result<HostOutput<[u8; 32]>, CudaFailure> {
    let request = geometry(count, true, true).ok_or(CudaFailure::InvalidRequest)?;
    let work = device.prepare(&[leaves_artifact, pairs_artifact], request)?;
    let mut input = work.buffer::<[u8; 64]>(count)?;
    let mut initial = work.buffer::<[u8; 32]>(count)?;
    let mut scratch = work.buffer::<[u8; 32]>(count.div_ceil(2))?;
    work.upload_generated(&mut input, input_block)?;
    work.wait()?;
    unsafe {
        leaves(&work, leaves_artifact, &input, &initial, count as u32)?;
    }
    unsafe {
        reduce(
            &work,
            pairs_artifact,
            &mut initial,
            &mut scratch,
            count as u32,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn odd_tree_has_two_bounded_device_levels_and_only_one_host_root() {
        let work = geometry(257, true, true).unwrap();
        assert_eq!(work.host_bytes, 32);
        assert_eq!(work.device_bytes, 257 * (64 + 32) + 129 * 32);
        assert_eq!(work.pinned_bytes, work.device_bytes);
        assert_eq!(geometry(257, true, false).unwrap().host_bytes, 257 * 32);
        assert_eq!(
            geometry(257, false, true).unwrap().device_bytes,
            (257 + 129) * 32
        );
    }
    #[test]
    fn empty_and_overwide_geometry_never_reach_native_allocation() {
        assert!(geometry(0, true, true).is_none());
        assert!(geometry(usize::MAX, true, true).is_none());
        assert_eq!(geometry(1, true, true).unwrap().host_bytes, 32);
    }
}
