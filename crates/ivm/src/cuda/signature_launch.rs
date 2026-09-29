//! Prepaid Ed25519 native staging with public batch geometry.

use iroha_accel::{
    HostOutput, PtxArtifact,
    cuda::{CudaDevice, CudaFailure, WorkRequest},
};
use std::ffi::c_void;

pub(super) fn request(count: usize) -> Option<(u32, WorkRequest)> {
    let count32 = u32::try_from(count).ok().filter(|&value| value != 0)?;
    let native_bytes = count.checked_mul(129)?;
    Some((
        count32,
        WorkRequest {
            host_bytes: count,
            pinned_bytes: native_bytes,
            device_bytes: native_bytes,
        },
    ))
}

/// Stage all input rows, execute, and retain the charged byte result.
///
/// # Safety
/// `artifact` must have the qualified five-argument `signature_kernel` ABI.
/// Each generated array is an initialized fixed-width kernel input.
pub(super) unsafe fn signature_output(
    device: &CudaDevice<'_>,
    artifact: PtxArtifact,
    count: usize,
    signature: impl FnMut(usize) -> [u8; 64],
    public_key: impl FnMut(usize) -> [u8; 32],
    hram: impl FnMut(usize) -> [u8; 32],
) -> Result<HostOutput<u8>, CudaFailure> {
    let (mut count32, request) = request(count).ok_or(CudaFailure::InvalidRequest)?;
    let work = device.prepare(&[artifact], request)?;
    let mut signatures = work.buffer::<[u8; 64]>(count)?;
    let mut keys = work.buffer::<[u8; 32]>(count)?;
    let mut hrams = work.buffer::<[u8; 32]>(count)?;
    let mut output = work.buffer::<u8>(count)?;
    work.upload_generated(&mut signatures, signature)?;
    work.wait()?;
    work.upload_generated(&mut keys, public_key)?;
    work.wait()?;
    work.upload_generated(&mut hrams, hram)?;
    work.wait()?;
    let mut signature_pointer = signatures.device_pointer();
    let mut key_pointer = keys.device_pointer();
    let mut hram_pointer = hrams.device_pointer();
    let mut output_pointer = output.device_pointer();
    let mut arguments = [
        (&raw mut signature_pointer).cast::<c_void>(),
        (&raw mut key_pointer).cast::<c_void>(),
        (&raw mut hram_pointer).cast::<c_void>(),
        (&raw mut count32).cast::<c_void>(),
        (&raw mut output_pointer).cast::<c_void>(),
    ];
    // SAFETY: arrays are contiguous, count was checked, and the qualified kernel
    // bounds every access by the exact public count.
    unsafe {
        work.launch(
            artifact,
            c"signature_kernel",
            [count32.div_ceil(128), 1, 1],
            [128, 1, 1],
            0,
            &mut arguments,
        )?;
    }
    work.wait()?;
    // SAFETY: the kernel initializes one byte for every row before completion.
    unsafe { work.download(&mut output) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_geometry_prepays_three_inputs_and_native_and_host_results() {
        let (count, work) = request(129).unwrap();
        assert_eq!(count, 129);
        assert_eq!(work.host_bytes, 129);
        assert_eq!(work.pinned_bytes, 129 * 129);
        assert_eq!(work.device_bytes, work.pinned_bytes);
    }
    #[test]
    fn unrepresentable_or_empty_geometry_is_refused_before_native_work() {
        assert!(request(0).is_none());
        assert!(request(usize::MAX).is_none());
        if usize::BITS > 32 {
            assert!(request(u32::MAX as usize + 1).is_none());
        }
    }
}
