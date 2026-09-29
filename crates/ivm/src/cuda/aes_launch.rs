//! Complete native AES attempt ownership without flattened or Vec output storage.

use iroha_accel::{
    HostOutput, PtxArtifact,
    cuda::{CudaDevice, CudaFailure, WorkRequest},
};
use std::{
    ffi::{CStr, c_void},
    mem::size_of,
};

fn request(blocks: usize, rounds: usize) -> Option<(u32, u32, WorkRequest)> {
    let count = u32::try_from(blocks).ok()?;
    let rounds = u32::try_from(rounds).ok()?;
    if count == 0 || rounds == 0 {
        return None;
    }
    let block_bytes = blocks.checked_mul(size_of::<[u8; 16]>())?;
    let key_bytes = (rounds as usize).checked_mul(size_of::<[u8; 16]>())?;
    let buffers = block_bytes.checked_mul(2)?.checked_add(key_bytes)?;
    Some((
        count,
        rounds,
        WorkRequest {
            host_bytes: block_bytes,
            pinned_bytes: buffers,
            device_bytes: buffers,
        },
    ))
}

/// Stage an AES round batch from admitted array storage; arrays have no padding.
///
/// # Safety
/// The artifact must be the qualified IVM AES artifact for these exact symbols.
/// `fused == false` requires exactly one supplied round key.
pub(super) unsafe fn aes_output(
    device: &CudaDevice<'_>,
    artifact: PtxArtifact,
    decrypt: bool,
    fused: bool,
    states: &[[u8; 16]],
    keys: &[[u8; 16]],
) -> Result<HostOutput<[u8; 16]>, CudaFailure> {
    if !fused && keys.len() != 1 {
        return Err(CudaFailure::InvalidRequest);
    }
    let (mut count, mut rounds, request) =
        request(states.len(), keys.len()).ok_or(CudaFailure::InvalidRequest)?;
    let work = device.prepare(&[artifact], request)?;
    let mut input = work.buffer::<[u8; 16]>(states.len())?;
    let mut round_keys = work.buffer::<[u8; 16]>(keys.len())?;
    let mut output = work.buffer::<[u8; 16]>(states.len())?;
    work.upload(&mut input, states)?;
    work.wait()?;
    work.upload(&mut round_keys, keys)?;
    work.wait()?;
    let mut input_pointer = input.device_pointer();
    let mut key_pointer = round_keys.device_pointer();
    let mut output_pointer = output.device_pointer();
    let symbol: &CStr = match (decrypt, fused) {
        (false, false) => c"aesenc_round_batch",
        (true, false) => c"aesdec_round_batch",
        (false, true) => c"aesenc_rounds_batch",
        (true, true) => c"aesdec_rounds_batch",
    };
    let mut plain_arguments = [
        (&raw mut input_pointer).cast::<c_void>(),
        (&raw mut key_pointer).cast::<c_void>(),
        (&raw mut output_pointer).cast::<c_void>(),
        (&raw mut count).cast::<c_void>(),
    ];
    let mut fused_arguments = [
        (&raw mut input_pointer).cast::<c_void>(),
        (&raw mut key_pointer).cast::<c_void>(),
        (&raw mut rounds).cast::<c_void>(),
        (&raw mut output_pointer).cast::<c_void>(),
        (&raw mut count).cast::<c_void>(),
    ];
    let arguments = if fused {
        &mut fused_arguments[..]
    } else {
        &mut plain_arguments[..]
    };
    // SAFETY: fixed kernel symbols receive contiguous arrays of bytes, exact
    // checked count/rounds, and their established four- or five-argument ABI.
    unsafe {
        work.launch(
            artifact,
            symbol,
            [count.div_ceil(256), 1, 1],
            [256, 1, 1],
            0,
            arguments,
        )?;
    }
    work.wait()?;
    // SAFETY: the qualified round kernel initializes every byte in each block.
    unsafe { work.download(&mut output) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_request_accounts_inputs_keys_output_and_escaping_host_copy() {
        let (count, rounds, work) = request(257, 9).unwrap();
        assert_eq!((count, rounds), (257, 9));
        assert_eq!(work.host_bytes, 257 * 16);
        assert_eq!(work.device_bytes, (257 * 2 + 9) * 16);
        assert_eq!(work.pinned_bytes, work.device_bytes);
    }

    #[test]
    fn malformed_geometry_is_refused_without_native_work() {
        assert!(request(0, 1).is_none());
        assert!(request(1, 0).is_none());
        assert!(request(usize::MAX, 1).is_none());
        assert!(request(1, usize::MAX).is_none());
    }
}
