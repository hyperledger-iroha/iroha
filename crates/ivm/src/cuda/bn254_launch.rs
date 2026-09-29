//! BN254 launch ownership and checked geometry over the shared physical owner.

use iroha_accel::{
    HostOutput, PtxArtifact,
    cuda::{CudaDevice, CudaFailure, WorkRequest},
};
use std::{ffi::c_void, mem::size_of};

use super::Kernel;

pub(super) fn request(elements: usize) -> Option<(u32, WorkRequest)> {
    let count = u32::try_from(elements).ok()?;
    if count == 0 {
        return None;
    }
    let bytes = elements.checked_mul(size_of::<[u64; 4]>())?;
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

/// Stage one complete BN254 batch without modifying the original operands.
///
/// # Safety
/// `artifact` must be the immutable admitted BN254 artifact with the existing
/// five-argument kernel ABI: three limb pointers, a u32 count and a u32 stride.
pub(super) unsafe fn output(
    device: &CudaDevice<'_>,
    artifact: PtxArtifact,
    kernel: Kernel,
    left: &[[u64; 4]],
    right: &[[u64; 4]],
) -> Result<HostOutput<[u64; 4]>, CudaFailure> {
    let symbol = match kernel {
        Kernel::BnAdd => c"bn254_add_kernel",
        Kernel::BnSub => c"bn254_sub_kernel",
        Kernel::BnMul => c"bn254_mul_kernel",
        _ => return Err(CudaFailure::InvalidRequest),
    };
    if !crate::bn254_vec::valid_batch(left, right, left.len()) {
        return Err(CudaFailure::InvalidRequest);
    }
    let (mut count, request) = request(left.len()).ok_or(CudaFailure::InvalidRequest)?;
    let work = device.prepare(&[artifact], request)?;
    // Typed arrays have the exact contiguous limb layout consumed by the kernel;
    // no flattening allocation or uncharged temporary output is constructed.
    let mut lhs = work.buffer::<[u64; 4]>(left.len())?;
    let mut rhs = work.buffer::<[u64; 4]>(right.len())?;
    let mut result = work.buffer::<[u64; 4]>(left.len())?;
    work.upload(&mut lhs, left)?;
    work.wait()?;
    work.upload(&mut rhs, right)?;
    work.wait()?;
    let mut lhs_pointer = lhs.device_pointer();
    let mut rhs_pointer = rhs.device_pointer();
    let mut result_pointer = result.device_pointer();
    let mut stride = 4u32;
    let mut arguments = [
        (&raw mut lhs_pointer).cast::<c_void>(),
        (&raw mut rhs_pointer).cast::<c_void>(),
        (&raw mut result_pointer).cast::<c_void>(),
        (&raw mut count).cast::<c_void>(),
        (&raw mut stride).cast::<c_void>(),
    ];
    // SAFETY: fixed symbols, typed contiguous arrays and checked count supply the
    // qualified ABI. All DMA backing belongs to the prepaid exact-stream work.
    unsafe {
        work.launch(
            artifact,
            symbol,
            [count.div_ceil(128), 1, 1],
            [128, 1, 1],
            0,
            &mut arguments,
        )?;
    }
    work.wait()?;
    // SAFETY: each admitted kernel initializes four u64 limbs per output element;
    // the lower owner observes exact-stream completion before typed publication.
    unsafe { work.download(&mut result) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_funds_typed_inputs_staging_and_escaping_output() {
        for len in [1, 127, 128, 129, 257] {
            let (count, request) = request(len).unwrap();
            assert_eq!(count as usize, len);
            assert_eq!(request.host_bytes, len * 32);
            assert_eq!(request.pinned_bytes, len * 96);
            assert_eq!(request.device_bytes, len * 96);
            assert_eq!(count.div_ceil(128) as usize, len.div_ceil(128));
        }
    }

    #[test]
    fn invalid_or_empty_geometry_declines_before_driver_access() {
        assert!(request(0).is_none());
        if let Some(overwide) = (u32::MAX as usize).checked_add(1) {
            assert!(request(overwide).is_none());
        }
        assert!(request(usize::MAX).is_none());
    }
}
