//! AES Metal attempts retain native output until caller-owned publication.

use super::MetalBuffer;
use super::{
    MetalKernel, metal_dispatch, metal_input_buffer, metal_output_buffer, metal_runtime_allowed,
    with_metal_state_try,
};
use objc2::rc::autoreleasepool;
use objc2_foundation::NSUInteger;
use objc2_metal::MTLBuffer as _;

struct Output {
    buffer: MetalBuffer,
    blocks: usize,
}

impl Output {
    fn copy_into(self, destination: &mut [[u8; 16]]) -> bool {
        if self.blocks != destination.len() {
            return false;
        }
        // SAFETY: the completed kernel initialized exactly `blocks` contiguous
        // byte arrays. The retained native buffer outlives the entire copy.
        let completed = unsafe {
            std::slice::from_raw_parts(
                self.buffer.contents().as_ptr().cast::<[u8; 16]>(),
                self.blocks,
            )
        };
        destination.copy_from_slice(completed);
        true
    }
}

fn kernel(decrypt: bool, fused: bool) -> MetalKernel {
    match (decrypt, fused) {
        (false, false) => MetalKernel::AesEncBatch,
        (true, false) => MetalKernel::AesDecBatch,
        (false, true) => MetalKernel::AesEncRounds,
        (true, true) => MetalKernel::AesDecRounds,
    }
}

fn attempt(
    states: &[[u8; 16]],
    keys: &[[u8; 16]],
    decrypt: bool,
    fused: bool,
    receipt: Option<MetalKernel>,
) -> Option<Output> {
    if !metal_runtime_allowed()
        || states.is_empty()
        || keys.is_empty()
        || (!fused && keys.len() != 1)
    {
        return None;
    }
    let rounds = u32::try_from(keys.len()).ok()?;
    let blocks = NSUInteger::try_from(states.len()).ok()?;
    let output_bytes = states.len().checked_mul(16)?;
    autoreleasepool(|_| {
        with_metal_state_try(|ctx| {
            // Borrow the existing contiguous arrays; no flattened host copy or
            // uncharged Vec escapes from the native attempt.
            let input = metal_input_buffer(&ctx.device, states.as_flattened(), output_bytes)?;
            let keys = metal_input_buffer(
                &ctx.device,
                keys.as_flattened(),
                keys.len().checked_mul(16)?,
            )?;
            let output = metal_output_buffer(&ctx.device, output_bytes)?;
            let pipeline = match (decrypt, fused) {
                (false, false) => &ctx.aesenc_batch,
                (true, false) => &ctx.aesdec_batch,
                (false, true) => &ctx.aesenc_rounds,
                (true, true) => &ctx.aesdec_rounds,
            };
            if fused {
                let rounds =
                    metal_input_buffer(&ctx.device, &[rounds], std::mem::size_of::<u32>())?;
                metal_dispatch(
                    &ctx.queue,
                    pipeline,
                    &[&input, &keys, &output, &rounds],
                    blocks,
                    1,
                    "metal AES fused batch",
                    receipt,
                )?;
            } else {
                metal_dispatch(
                    &ctx.queue,
                    pipeline,
                    &[&input, &keys, &output],
                    blocks,
                    1,
                    "metal AES round batch",
                    receipt,
                )?;
            }
            Some(Output {
                buffer: output,
                blocks: states.len(),
            })
        })
    })
}

pub(super) fn with_receipt_into(
    states: &[[u8; 16]],
    keys: &[[u8; 16]],
    destination: &mut [[u8; 16]],
    decrypt: bool,
    fused: bool,
    receipt: Option<MetalKernel>,
) -> bool {
    if states.len() != destination.len() || (!fused && keys.len() != 1) {
        return false;
    }
    if states.is_empty() {
        return true;
    }
    if keys.is_empty() {
        destination.copy_from_slice(states);
        return true;
    }
    attempt(states, keys, decrypt, fused, receipt)
        .is_some_and(|output| output.copy_into(destination))
}

pub(crate) fn metal_aes_batch_in_place(
    states: &mut [[u8; 16]],
    keys: &[[u8; 16]],
    decrypt: bool,
    fused: bool,
) -> bool {
    if !fused && keys.len() != 1 {
        return false;
    }
    if states.is_empty() || keys.is_empty() {
        return true;
    }
    attempt(states, keys, decrypt, fused, Some(kernel(decrypt, fused)))
        .is_some_and(|output| output.copy_into(states))
}

/// Attempt one AESENC batch; refusal or failure preserves the caller destination.
pub fn metal_aesenc_batch_into(
    states: &[[u8; 16]],
    key: [u8; 16],
    destination: &mut [[u8; 16]],
) -> bool {
    with_receipt_into(
        states,
        &[key],
        destination,
        false,
        false,
        Some(MetalKernel::AesEncBatch),
    )
}
/// Attempt one AESDEC batch; refusal or failure preserves the caller destination.
pub fn metal_aesdec_batch_into(
    states: &[[u8; 16]],
    key: [u8; 16],
    destination: &mut [[u8; 16]],
) -> bool {
    with_receipt_into(
        states,
        &[key],
        destination,
        true,
        false,
        Some(MetalKernel::AesDecBatch),
    )
}
/// Attempt ordered AESENC rounds; refusal or failure preserves the caller destination.
pub fn metal_aesenc_rounds_batch_into(
    states: &[[u8; 16]],
    keys: &[[u8; 16]],
    destination: &mut [[u8; 16]],
) -> bool {
    with_receipt_into(
        states,
        keys,
        destination,
        false,
        true,
        Some(MetalKernel::AesEncRounds),
    )
}
/// Attempt ordered AESDEC rounds; refusal or failure preserves the caller destination.
pub fn metal_aesdec_rounds_batch_into(
    states: &[[u8; 16]],
    keys: &[[u8; 16]],
    destination: &mut [[u8; 16]],
) -> bool {
    with_receipt_into(
        states,
        keys,
        destination,
        true,
        true,
        Some(MetalKernel::AesDecRounds),
    )
}
