//! Native Ed25519 batch publication without flattened host or returned vectors.

use super::*;
use crate::signature::{BatchInput, Ed25519BatchItem};
use std::mem::size_of;

#[cfg(test)]
pub(crate) fn metal_ed25519_verify_batch_into(
    signatures: &[[u8; 64]],
    public_keys: &[[u8; 32]],
    hrams: &[[u8; 32]],
    destination: &mut [bool],
) -> bool {
    into(
        BatchInput::Prepared {
            signatures,
            public_keys,
            hrams,
        },
        destination,
        Some(MetalKernel::Ed25519),
    )
}

pub(crate) fn metal_ed25519_items_into(
    items: &[Ed25519BatchItem<'_>],
    destination: &mut [bool],
) -> bool {
    into(
        BatchInput::Items(items),
        destination,
        Some(MetalKernel::Ed25519),
    )
}

fn into(input: BatchInput<'_, '_>, destination: &mut [bool], receipt: Option<MetalKernel>) -> bool {
    if input.checked_len() != Some(destination.len()) {
        return false;
    }
    let count = destination.len();
    if count == 0 {
        return true;
    }
    let Ok(count32) = u32::try_from(count) else {
        return false;
    };
    let Some(signature_bytes) = count.checked_mul(64) else {
        return false;
    };
    let Some(scalar_bytes) = count.checked_mul(32) else {
        return false;
    };
    if !metal_runtime_allowed() {
        return false;
    }
    objc2::rc::autoreleasepool(|_| {
        with_metal_state_try(|ctx| {
            let pipeline = ctx.ed25519_signature.as_ref()?;
            // No Rust flatten/index/result allocation is made here. Each native
            // buffer stays owned through the completed command and final copy.
            let signatures = metal_output_buffer(&ctx.device, signature_bytes)?;
            let keys = metal_output_buffer(&ctx.device, scalar_bytes)?;
            let hrams = metal_output_buffer(&ctx.device, scalar_bytes)?;
            let output = metal_output_buffer(&ctx.device, count)?;
            let count_buffer = metal_input_buffer(&ctx.device, &[count32], size_of::<u32>())?;
            // SAFETY: newly allocated shared buffers are uniquely owned, have
            // the exact checked byte lengths, and no command has been enqueued.
            // Fixed byte arrays have alignment one and contain no padding.
            unsafe {
                let signature_pointer = signatures.contents().as_ptr().cast::<[u8; 64]>();
                let key_pointer = keys.contents().as_ptr().cast::<[u8; 32]>();
                let hram_pointer = hrams.contents().as_ptr().cast::<[u8; 32]>();
                for index in 0..count {
                    signature_pointer.add(index).write(input.signature(index));
                    key_pointer.add(index).write(input.public_key(index));
                    hram_pointer.add(index).write(input.hram(index));
                }
            }
            metal_dispatch(
                &ctx.queue,
                pipeline,
                &[&signatures, &keys, &hrams, &count_buffer, &output],
                count as NSUInteger,
                pipeline.threadExecutionWidth().max(1),
                "metal ed25519 batch verify",
                receipt,
            )?;
            // SAFETY: the completed qualified kernel initializes every result byte.
            let native = unsafe {
                std::slice::from_raw_parts(output.contents().as_ptr().cast::<u8>(), count)
            };
            if native.iter().any(|&byte| byte > 1) {
                record_metal_disable("noncanonical Ed25519 kernel result");
                return None;
            }
            if !output.usable() {
                return None;
            }
            input.publish(native, destination).then_some(())
        })
    })
    .is_some()
}
