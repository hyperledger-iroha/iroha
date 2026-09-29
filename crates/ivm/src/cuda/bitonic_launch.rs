//! Prepaid paired bitonic buffers and atomic caller publication staging.

use iroha_accel::{
    HostOutput, PtxArtifact,
    cuda::{CudaDevice, CudaFailure, WorkRequest},
};
use std::ffi::c_void;

pub(super) struct Sorted {
    pub(super) hi: HostOutput<u64>,
    pub(super) lo: HostOutput<u64>,
}

pub(super) fn request(count: usize) -> Option<(u32, WorkRequest)> {
    if count < 2 {
        return None;
    }
    let padded = u32::try_from(count.checked_next_power_of_two()?).ok()?;
    let host = count.checked_mul(16)?;
    let native = (padded as usize).checked_mul(16)?;
    Some((
        padded,
        WorkRequest {
            host_bytes: host,
            pinned_bytes: native,
            device_bytes: native,
        },
    ))
}

/// Sort copies of the original pairs, retaining both charged native outputs.
///
/// # Safety
/// `artifact` must contain the qualified five-argument `bitonic_step` ABI.
pub(super) unsafe fn output(
    device: &CudaDevice<'_>,
    artifact: PtxArtifact,
    hi: &[u64],
    lo: &[u64],
) -> Result<Sorted, CudaFailure> {
    if hi.len() != lo.len() {
        return Err(CudaFailure::InvalidRequest);
    }
    let (mut padded, request) = request(hi.len()).ok_or(CudaFailure::InvalidRequest)?;
    let work = device.prepare(&[artifact], request)?;
    let mut high = work.buffer::<u64>(padded as usize)?;
    let mut low = work.buffer::<u64>(padded as usize)?;
    work.upload_generated(&mut high, |i| hi.get(i).copied().unwrap_or(u64::MAX))?;
    work.wait()?;
    work.upload_generated(&mut low, |i| lo.get(i).copied().unwrap_or(u64::MAX))?;
    work.wait()?;
    let mut high_pointer = high.device_pointer();
    let mut low_pointer = low.device_pointer();
    let mut k = 2u32;
    loop {
        let mut j = k >> 1;
        while j != 0 {
            let mut arguments = [
                (&raw mut high_pointer).cast::<c_void>(),
                (&raw mut low_pointer).cast::<c_void>(),
                (&raw mut padded).cast::<c_void>(),
                (&raw mut j).cast::<c_void>(),
                (&raw mut k).cast::<c_void>(),
            ];
            // SAFETY: the checked power-of-two count and j/k schedule bound
            // every compare/exchange within the two initialized arrays.
            unsafe {
                work.launch(
                    artifact,
                    c"bitonic_step",
                    [padded.div_ceil(256), 1, 1],
                    [256, 1, 1],
                    0,
                    &mut arguments,
                )?;
            }
            j >>= 1;
        }
        if k == padded {
            break;
        }
        k = k.checked_mul(2).ok_or(CudaFailure::InvalidRequest)?;
    }
    work.wait()?;
    // SAFETY: generated upload initialized both full arrays; all kernels only
    // swap initialized words, and only the original count is returned.
    let high_output = unsafe { work.download_prefix(&mut high, hi.len()) }?;
    // SAFETY: same completed initialized schedule as the high-word array.
    let low_output = unsafe { work.download_prefix(&mut low, lo.len()) }?;
    Ok(Sorted {
        hi: high_output,
        lo: low_output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn padded_arrays_and_unpadded_results_are_funded_separately() {
        let (padded, work) = request(257).unwrap();
        assert_eq!(padded, 512);
        assert_eq!(work.host_bytes, 257 * 16);
        assert_eq!(work.device_bytes, 512 * 16);
        assert_eq!(work.pinned_bytes, work.device_bytes);
    }
    #[test]
    fn overflow_and_non_native_work_are_rejected_before_admission() {
        assert!(request(0).is_none());
        assert!(request(1).is_none());
        assert!(request(usize::MAX).is_none());
        assert!(request(u32::MAX as usize).is_none());
        if usize::BITS > 32 {
            assert_eq!(request(1usize << 31).unwrap().0, 1u32 << 31);
        }
    }
}
