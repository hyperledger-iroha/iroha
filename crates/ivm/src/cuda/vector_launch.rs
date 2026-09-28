//! Complete vector launch ownership migrated to the shared physical registry.
//!
//! This projected consumer unit replaces the two launch_u32/launch_u64 bodies;
//! it does not coexist with their old native allocation/stream implementation.
//! IVM operation self-tests, counters, task selection and artifact qualification
//! remain in the calling IVM policy adapter. Completed staging stays charged until
//! the full result is copied into the caller's initialized destination.

use cust::memory::DeviceCopy;
use iroha_accel::{
    HostOutput, PtxArtifact,
    cuda::{CudaDevice, CudaFailure, WorkRequest},
};
use std::{
    ffi::{CStr, c_void},
    mem::size_of,
};

/// Public geometry and exact requested backing before creating native owners.
fn vector_request<T>(len: usize) -> Option<(u32, WorkRequest)> {
    let count = u32::try_from(len).ok()?;
    if count == 0 {
        return None;
    }
    let bytes = len.checked_mul(size_of::<T>())?;
    let all = bytes.checked_mul(3)?;
    Some((
        count,
        WorkRequest {
            host_bytes: bytes,
            pinned_bytes: all,
            device_bytes: all,
        },
    ))
}

/// Stage a complete u32 operation while retaining its original output charge.
///
/// # Safety
/// `artifact` must be the admitted immutable IVM vector artifact providing the
/// listed symbols with the existing exact vector ABI. Selection alone is not
/// artifact qualification. The public safe IVM entrypoints retain that check.
pub(super) unsafe fn launch_u32_output(
    device: &CudaDevice<'_>,
    artifact: PtxArtifact,
    name: &str,
    a: &[u32],
    b: &[u32],
) -> Result<HostOutput<u32>, CudaFailure> {
    let symbol = match name {
        "vadd32" => c"vadd32",
        "vand" => c"vand",
        "vxor" => c"vxor",
        "vor" => c"vor",
        _ => return Err(CudaFailure::InvalidRequest),
    };
    // SAFETY: caller supplies the qualified exact artifact; this fixed symbol map
    // binds u32 elements to the existing arithmetic operation's argument types.
    unsafe { launch(device, artifact, symbol, a, b) }
}

/// Stage full-width u64 addition while retaining its original output charge.
///
/// # Safety
/// `artifact` must be the admitted immutable IVM vector artifact with its existing
/// four-argument vadd64 ABI, including full u64 wrapping arithmetic.
pub(super) unsafe fn launch_u64_output(
    device: &CudaDevice<'_>,
    artifact: PtxArtifact,
    a: &[u64],
    b: &[u64],
) -> Result<HostOutput<u64>, CudaFailure> {
    // SAFETY: caller supplies the qualified artifact and this wrapper fixes the
    // sole u64 symbol; all lengths and backing demands are checked below.
    unsafe { launch(device, artifact, c"vadd64", a, b) }
}

unsafe fn launch<T: DeviceCopy + Copy + Default>(
    device: &CudaDevice<'_>,
    artifact: PtxArtifact,
    symbol: &CStr,
    a: &[T],
    b: &[T],
) -> Result<HostOutput<T>, CudaFailure> {
    if a.len() != b.len() {
        return Err(CudaFailure::InvalidRequest);
    }
    let (mut count, request) = vector_request::<T>(a.len()).ok_or(CudaFailure::InvalidRequest)?;
    let work = device.prepare(&[artifact], request)?;
    // Every stable backing allocation is constructed from the complete original
    // request before the first asynchronous command. Caller input stays unchanged.
    let mut left = work.buffer::<T>(a.len())?;
    let mut right = work.buffer::<T>(b.len())?;
    let mut output = work.buffer::<T>(a.len())?;
    work.upload(&mut left, a)?;
    work.wait()?;
    work.upload(&mut right, b)?;
    work.wait()?;
    let mut left_pointer = left.device_pointer();
    let mut right_pointer = right.device_pointer();
    let mut output_pointer = output.device_pointer();
    let mut arguments = [
        (&raw mut left_pointer).cast::<c_void>(),
        (&raw mut right_pointer).cast::<c_void>(),
        (&raw mut output_pointer).cast::<c_void>(),
        (&raw mut count).cast::<c_void>(),
    ];
    // SAFETY: the admitted fixed kernel uses exactly these typed buffers and the
    // existing 256-thread geometry. The scalar/pointer argument values are copied
    // synchronously; all actual DMA targets belong to stable admitted allocations.
    unsafe {
        work.launch(
            artifact,
            symbol,
            [count.div_ceil(256), 1, 1],
            [256, 1, 1],
            0,
            &mut arguments,
        )?;
    }
    work.wait()?;
    // SAFETY: these exact vector kernels initialize every output element to valid
    // u32/u64 bits. Exact-stream completion was observed before any typed read.
    unsafe { work.download(&mut output) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_funds_all_inputs_staging_and_escaping_output() {
        let (count, request) = vector_request::<u64>(257).unwrap();
        assert_eq!(count, 257);
        assert_eq!(request.host_bytes, 257 * 8);
        assert_eq!(request.pinned_bytes, 3 * 257 * 8);
        assert_eq!(request.device_bytes, 3 * 257 * 8);
        assert_eq!(count.div_ceil(256), 2);
    }

    #[test]
    fn invalid_or_empty_geometry_declines_before_any_driver_operation() {
        assert!(vector_request::<u32>(0).is_none());
        if let Some(overwide) = (u32::MAX as usize).checked_add(1) {
            assert!(vector_request::<u32>(overwide).is_none());
        }
        assert!(vector_request::<u64>(usize::MAX).is_none());
    }
}
