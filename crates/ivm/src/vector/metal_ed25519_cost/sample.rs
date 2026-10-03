//! Synthetic inputs and caller-owned outputs funded before construction.

use super::*;
use crate::signature::Ed25519BatchItem;
use ed25519_dalek::{Signer as _, SigningKey};
use iroha_accel::{HostOutput, ProcessResources};
use std::hint::black_box;

fn check_deadline(started: Instant) -> Result<(), Failure> {
    if started.elapsed() >= MAX_CALIBRATION {
        Err(Failure::Deadline)
    } else {
        Ok(())
    }
}
fn elapsed(started: Instant) -> Result<u64, Failure> {
    u64::try_from(started.elapsed().as_nanos())
        .ok()
        .filter(|&value| value > 0)
        .ok_or(Failure::Overloaded)
}

fn allocate<T: Copy + Default>(
    owner: &ProcessResources,
    len: usize,
) -> Result<HostOutput<T>, Failure> {
    owner.try_host_output(len).map_err(|_| Failure::Allocation)
}

/// Messages, borrowed views, results and profile remain covered by the original
/// process resource owner. Generation reads only lengths, never caller contents.
pub(super) fn with_inputs<T>(
    geometry: MessageGeometry<'_, '_>,
    started: Instant,
    call: impl FnOnce(&[Ed25519BatchItem<'_>], &[bool], &mut [bool]) -> Result<T, Failure>,
) -> Result<T, Failure> {
    check_deadline(started)?;
    let owner = ProcessResources::get().ok_or(Failure::Allocation)?;
    let mut messages = allocate::<u8>(owner, geometry.total_bytes())?;
    let mut inputs = allocate::<Ed25519BatchItem<'_>>(owner, geometry.len())?;
    let mut expected = allocate::<bool>(owner, geometry.len())?;
    let mut actual = allocate::<bool>(owner, geometry.len())?;
    let signing = SigningKey::from_bytes(&[0x43; 32]);
    let valid_key = signing.verifying_key().to_bytes();
    let invalid_key = SigningKey::from_bytes(&[0x71; 32])
        .verifying_key()
        .to_bytes();
    for (index, byte) in messages.iter_mut().enumerate() {
        *byte = (index as u8)
            .wrapping_mul(31)
            .wrapping_add((index >> 8) as u8)
            .wrapping_add(7);
    }
    let mut offset = 0;
    for index in 0..geometry.len() {
        check_deadline(started)?;
        let end = offset + geometry.message_len(index);
        let message = &messages[offset..end];
        let valid = index % 4 != 3;
        inputs[index] = Ed25519BatchItem {
            message,
            signature: signing.sign(message).to_bytes(),
            public_key: if valid { valid_key } else { invalid_key },
        };
        expected[index] = valid;
        offset = end;
    }
    call(&inputs, &expected, &mut actual)
}

pub(in crate::vector) fn calibrate(
    geometry: MessageGeometry<'_, '_>,
    started: Instant,
) -> Result<Profile, Failure> {
    with_inputs(geometry, started, |inputs, expected, actual| {
        let mut cpu_ns = [0; TRIALS];
        let mut metal_ns = [0; TRIALS];
        for trial in 0..TRIALS {
            check_deadline(started)?;
            let cpu_start = Instant::now();
            crate::signature::cpu_batch_into(black_box(inputs), black_box(&mut *actual));
            cpu_ns[trial] = elapsed(cpu_start)?;
            if actual != expected {
                return Err(Failure::CpuMismatch);
            }
            check_deadline(started)?;
            let (completed, time) = super::super::metal_receipts::with_synthetic(|| {
                let gpu_start = Instant::now();
                let completed = super::super::metal_signature::metal_ed25519_items_into(
                    black_box(inputs),
                    black_box(&mut *actual),
                );
                (completed, elapsed(gpu_start))
            });
            if !completed {
                return Err(Failure::Backend);
            }
            metal_ns[trial] = time?;
            if actual != expected {
                super::super::record_metal_disable(
                    "synthetic exact-geometry Ed25519 parity mismatch",
                );
                return Err(Failure::Parity);
            }
            check_deadline(started)?;
        }
        Profile::from_trials(geometry, cpu_ns, metal_ns)
    })
}

#[cfg(test)]
mod tests;
