//! Shared-owner SHA leaves and complete Merkle reduction publication.

use super::policy::{Kernel, public_workload_task_id};
use iroha_accel::{HostOutput, PtxArtifact, cuda::CudaFailure};
use std::ffi::CStr;

#[path = "merkle_launch.rs"]
mod launch;

static LEAVES: PtxArtifact = PtxArtifact::new(
    match CStr::from_bytes_with_nul(
        concat!(
            include_str!(concat!(env!("OUT_DIR"), "/sha256_leaves.ptx")),
            "\0"
        )
        .as_bytes(),
    ) {
        Ok(bytes) => bytes,
        Err(_) => panic!("embedded SHA leaves artifact must have exactly one terminal NUL"),
    },
);
static PAIRS: PtxArtifact = PtxArtifact::new(
    match CStr::from_bytes_with_nul(
        concat!(
            include_str!(concat!(env!("OUT_DIR"), "/sha256_pairs_reduce.ptx")),
            "\0"
        )
        .as_bytes(),
    ) {
        Ok(bytes) => bytes,
        Err(_) => panic!("embedded SHA pairs artifact must have exactly one terminal NUL"),
    },
);

fn completed(
    result: Result<HostOutput<[u8; 32]>, CudaFailure>,
) -> Result<HostOutput<[u8; 32]>, CudaFailure> {
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

fn leaf_stage(blocks: &[[u8; 64]]) -> Result<HostOutput<[u8; 32]>, CudaFailure> {
    completed(crate::cuda_dispatch::with_selected(
        Kernel::ShaLeaves,
        LEAVES,
        |device| {
            // SAFETY: exact embedded artifact, fixed symbol and checked public geometry.
            unsafe { launch::leaves_output(device, LEAVES, blocks) }
        },
    ))
}
fn pair_stage(digests: &[[u8; 32]]) -> Result<HostOutput<[u8; 32]>, CudaFailure> {
    completed(crate::cuda_dispatch::with_selected(
        Kernel::ShaPairs,
        PAIRS,
        |device| {
            // SAFETY: exact embedded artifact, fixed symbol and checked public geometry.
            unsafe { launch::pairs_output(device, PAIRS, digests) }
        },
    ))
}

pub(super) fn admit(kernel: Kernel) -> bool {
    let artifact = match kernel {
        Kernel::ShaLeaves => LEAVES,
        Kernel::ShaPairs => PAIRS,
        _ => return false,
    };
    crate::cuda_dispatch::admit_kernel(kernel, artifact, || {
        use sha2::{Digest as _, Sha256};
        let Some(_guard) = super::imp::SelftestRunningGuard::enter() else {
            return Err(CudaFailure::Busy);
        };
        match kernel {
            Kernel::ShaLeaves => {
                let messages: [&[u8]; 2] = [b"abc", b"norito"];
                let blocks: [[u8; 64]; 2] = messages.map(|message| {
                    let mut block = [0; 64];
                    block[..message.len()].copy_from_slice(message);
                    block[message.len()] = 0x80;
                    block[56..].copy_from_slice(&((message.len() as u64) * 8).to_be_bytes());
                    block
                });
                let expected: [[u8; 32]; 2] =
                    messages.map(|message| Sha256::digest(message).into());
                leaf_stage(&blocks).map(|output| output.as_slice() == expected)
            }
            Kernel::ShaPairs => {
                let input = [[0; 32], [0xff; 32], [0xa5; 32]];
                let pair = |left: [u8; 32], right: [u8; 32]| -> [u8; 32] {
                    let mut hash = Sha256::new();
                    hash.update(left);
                    hash.update(right);
                    hash.finalize().into()
                };
                let expected = pair(pair(input[0], input[1]), input[2]);
                pair_stage(&input).map(|output| output.as_slice() == [expected])
            }
            _ => Ok(false),
        }
    })
}

/// Keep native output charged until an internal caller copies into its original destination.
pub(crate) fn sha256_leaves_cuda_attempt(blocks: &[[u8; 64]]) -> Option<HostOutput<[u8; 32]>> {
    if blocks.is_empty() {
        return None;
    }
    let task = public_workload_task_id(0x0f0f_0f0f_0000_0021, &[blocks.len() as u64]);
    crate::cuda_dispatch::with_task_scope(task, || {
        super::imp::record_cuda_attempt();
        if !super::imp::ensure_cuda_kernel(Kernel::ShaLeaves) {
            return None;
        }
        leaf_stage(blocks).ok()
    })
}

fn chunk_count(data_len: usize, chunk: usize, count: usize) -> Option<()> {
    ((1..=32).contains(&chunk) && count >= data_len.div_ceil(chunk).max(1)).then_some(())
}

fn padded_chunk(data: &[u8], chunk: usize, index: usize) -> [u8; 64] {
    let mut block = [0; 64];
    // Out-of-data leaves are canonical zero chunks; avoid overflow for bounded
    // synthetic/retained shapes that contain more leaves than populated bytes.
    if let Some(start) = index.checked_mul(chunk).filter(|start| *start < data.len()) {
        let available = chunk.min(data.len() - start);
        block[..available].copy_from_slice(&data[start..start + available]);
    }
    block[chunk] = 0x80;
    block[56..].copy_from_slice(&((chunk as u64) * 8).to_be_bytes());
    block
}

/// Hash canonical fixed chunks without allocating a flattened host input table.
/// The returned native owner stays charged until the existing destination is filled.
pub(crate) fn sha256_leaf_chunks_cuda_attempt(
    data: &[u8],
    chunk: usize,
    count: usize,
) -> Option<HostOutput<[u8; 32]>> {
    chunk_count(data.len(), chunk, count)?;
    let task = public_workload_task_id(0x0f0f_0f0f_0000_0021, &[count as u64, chunk as u64]);
    crate::cuda_dispatch::with_task_scope(task, || {
        super::imp::record_cuda_attempt();
        if !super::imp::ensure_cuda_kernel(Kernel::ShaLeaves) {
            return None;
        }
        completed(crate::cuda_dispatch::with_selected(
            Kernel::ShaLeaves,
            LEAVES,
            |device| {
                // SAFETY: fixed canonical padded chunks, exact artifact and checked geometry.
                unsafe {
                    launch::leaves_generated_output(device, LEAVES, count, |index| {
                        padded_chunk(data, chunk, index)
                    })
                }
            },
        ))
        .ok()
    })
}

/// Hash already padded single-block SHA-256 messages into caller storage.
/// Refusal, failure, or a length mismatch leaves the destination unchanged.
pub fn sha256_leaves_cuda_into(blocks: &[[u8; 64]], destination: &mut [[u8; 32]]) -> bool {
    if blocks.len() != destination.len() {
        return false;
    }
    if blocks.is_empty() {
        return true;
    }
    let Some(output) = sha256_leaves_cuda_attempt(blocks) else {
        return false;
    };
    if output.len() != destination.len() {
        return false;
    }
    destination.copy_from_slice(output.as_slice());
    true
}

/// Reduce digest pairs with left promotion, returning only a completely initialized root.
pub fn sha256_pairs_reduce_cuda(digests: &[[u8; 32]]) -> Option<[u8; 32]> {
    match digests {
        [] => return None,
        [single] => return Some(*single),
        _ => {}
    }
    let task = public_workload_task_id(0x0f0f_0f0f_0000_0022, &[digests.len() as u64]);
    crate::cuda_dispatch::with_task_scope(task, || {
        super::imp::record_cuda_attempt();
        if !super::imp::ensure_cuda_kernel(Kernel::ShaPairs) {
            return None;
        }
        let output = pair_stage(digests).ok()?;
        (output.len() == 1).then(|| output.as_slice()[0])
    })
}

/// Hash leaves and every Merkle level within one complete process reservation.
pub(crate) fn sha256_merkle_root_cuda(data: &[u8], chunk: usize) -> Option<[u8; 32]> {
    if !(1..=32).contains(&chunk) {
        return None;
    }
    let count = data.len().div_ceil(chunk).max(1);
    if count == 1 {
        let output = sha256_leaf_chunks_cuda_attempt(data, chunk, count)?;
        return (output.len() == 1).then(|| output.as_slice()[0]);
    }
    let task = public_workload_task_id(
        0x0f0f_0f0f_0000_0022,
        &[count as u64, 0x6d65_726b_6c65_726f],
    );
    crate::cuda_dispatch::with_task_scope(task, || {
        super::imp::record_cuda_attempt();
        if !super::imp::ensure_cuda_kernel(Kernel::ShaLeaves)
            || !super::imp::ensure_cuda_kernel(Kernel::ShaPairs)
        {
            return None;
        }
        let output = completed(crate::cuda_dispatch::with_selected(
            Kernel::ShaPairs,
            PAIRS,
            |device| {
                if !crate::cuda_dispatch::current_is_admitted(Kernel::ShaLeaves, LEAVES) {
                    return Err(CudaFailure::Quarantined);
                }
                // SAFETY: both artifacts are independently admitted on this pinned device;
                // one request prepays all inputs, level buffers and final host output.
                let result = unsafe {
                    launch::root_output(device, LEAVES, PAIRS, count, |index| {
                        padded_chunk(data, chunk, index)
                    })
                };
                if !crate::cuda_dispatch::current_is_admitted(Kernel::ShaLeaves, LEAVES) {
                    return Err(CudaFailure::Quarantined);
                }
                result
            },
        ))
        .ok()?;
        (output.len() == 1).then(|| output.as_slice()[0])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_chunks_preserve_zero_padding_length_and_absent_leaves() {
        assert!(chunk_count(3, 2, 1).is_none());
        assert!(chunk_count(3, 2, 2).is_some());
        assert!(chunk_count(0, 0, 1).is_none());
        assert!(chunk_count(0, 33, 1).is_none());
        let first = padded_chunk(&[1, 2, 3], 2, 0);
        assert_eq!(&first[..4], &[1, 2, 0x80, 0]);
        let last = padded_chunk(&[1, 2, 3], 2, 1);
        assert_eq!(&last[..4], &[3, 0, 0x80, 0]);
        assert_eq!(&last[56..], &16u64.to_be_bytes());
        assert_eq!(padded_chunk(&[], 2, usize::MAX), padded_chunk(&[], 2, 0));
    }
}
