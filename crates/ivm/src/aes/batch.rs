//! Caller-owned AES batches with untouched native attempt inputs.

use super::{add_round_key, aesdec, aesdec_last_impl, aesenc, aesenc_last_impl};

fn rounds_in_place(states: &mut [[u8; 16]], keys: &[[u8; 16]], decrypt: bool, fused: bool) {
    if states.is_empty() || keys.is_empty() {
        return;
    }
    #[cfg(all(target_os = "macos", feature = "metal"))]
    {
        use crate::vector::MetalBatchWork;
        let work = match (decrypt, fused) {
            (false, false) => MetalBatchWork::AesEnc,
            (true, false) => MetalBatchWork::AesDec,
            (false, true) => MetalBatchWork::AesEncRounds(keys.len()),
            (true, true) => MetalBatchWork::AesDecRounds(keys.len()),
        };
        if crate::vector::select_metal_batch(work, states.len())
            .and_then(|selected| {
                selected
                    .run(|| crate::vector::metal_aes_batch_in_place(states, keys, decrypt, fused))
            })
            .unwrap_or(false)
        {
            return;
        }
    }
    #[cfg(feature = "cuda")]
    {
        use crate::cuda::policy::Kernel;
        let transfer_bytes = states
            .len()
            .saturating_mul(32)
            .saturating_add(keys.len().saturating_mul(16));
        let kernel = match (decrypt, fused) {
            (false, false) => Kernel::AesEnc,
            (true, false) => Kernel::AesDec,
            (false, true) => Kernel::AesEncFused,
            (true, true) => Kernel::AesDecFused,
        };
        if crate::vector::gpu_launch_eligible(transfer_bytes)
            && let Some(output) = crate::cuda::aes_batch_attempt(kernel, states, keys)
            && output.len() == states.len()
        {
            // The native owner retains its charged backing through the complete
            // copy into the original caller-funded destination.
            states.copy_from_slice(output.as_slice());
            return;
        }
    }
    #[cfg(not(any(feature = "cuda", all(target_os = "macos", feature = "metal"))))]
    let _ = fused;
    let round = if decrypt { aesdec } else { aesenc };
    // A refused or failed native attempt leaves all input blocks untouched.
    // Each scalar/SIMD fold begins from the original phase input.
    for block in states {
        *block = keys.iter().copied().fold(*block, round);
    }
}

fn rounds_into(
    states: &[[u8; 16]],
    keys: &[[u8; 16]],
    destination: &mut [[u8; 16]],
    decrypt: bool,
    fused: bool,
) -> bool {
    if states.len() != destination.len() {
        return false;
    }
    destination.copy_from_slice(states);
    rounds_in_place(destination, keys, decrypt, fused);
    true
}

/// Apply one AESENC round into caller-owned storage. A length mismatch leaves it unchanged.
pub fn aesenc_many_into(states: &[[u8; 16]], key: [u8; 16], destination: &mut [[u8; 16]]) -> bool {
    rounds_into(states, &[key], destination, false, false)
}
/// Apply one AESDEC round into caller-owned storage. A length mismatch leaves it unchanged.
pub fn aesdec_many_into(states: &[[u8; 16]], key: [u8; 16], destination: &mut [[u8; 16]]) -> bool {
    rounds_into(states, &[key], destination, true, false)
}
/// Apply ordered AESENC rounds into caller-owned storage, without the initial or last round.
/// An empty key sequence copies the input; a length mismatch preserves the destination.
pub fn aesenc_n_rounds_many_into(
    states: &[[u8; 16]],
    keys: &[[u8; 16]],
    destination: &mut [[u8; 16]],
) -> bool {
    rounds_into(states, keys, destination, false, true)
}
/// Apply ordered AESDEC rounds into caller-owned storage, without the initial or last round.
/// An empty key sequence copies the input; a length mismatch preserves the destination.
pub fn aesdec_n_rounds_many_into(
    states: &[[u8; 16]],
    keys: &[[u8; 16]],
    destination: &mut [[u8; 16]],
) -> bool {
    rounds_into(states, keys, destination, true, true)
}
/// Encrypt AES-128 blocks using the eleven keys returned by `aes128_expand_key`.
/// A length mismatch preserves the caller-owned destination.
pub fn aes128_encrypt_many_into(
    states: &[[u8; 16]],
    keys: &[[u8; 16]; 11],
    destination: &mut [[u8; 16]],
) -> bool {
    if states.len() != destination.len() {
        return false;
    }
    for (source, output) in states.iter().zip(destination.iter_mut()) {
        *output = *source;
        add_round_key(output, &keys[0]);
    }
    rounds_in_place(destination, &keys[1..10], false, true);
    for block in destination {
        *block = aesenc_last_impl(*block, keys[10]);
    }
    true
}
/// Decrypt AES-128 blocks using the same eleven-key encryption schedule.
/// A length mismatch preserves the caller-owned destination.
pub fn aes128_decrypt_many_into(
    states: &[[u8; 16]],
    keys: &[[u8; 16]; 11],
    destination: &mut [[u8; 16]],
) -> bool {
    if states.len() != destination.len() {
        return false;
    }
    for (source, output) in states.iter().zip(destination.iter_mut()) {
        *output = aesdec_last_impl(*source, keys[10]);
    }
    let inverse_keys: [[u8; 16]; 9] = std::array::from_fn(|index| keys[9 - index]);
    rounds_in_place(destination, &inverse_keys, true, true);
    for block in destination {
        add_round_key(block, &keys[0]);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_destinations_never_publish_and_empty_rounds_copy() {
        let input = [[7; 16], [8; 16]];
        let mut short = [[0xa5; 16]];
        let keys = super::super::aes128_expand_key([1; 16]);
        assert!(!aesenc_many_into(&input, keys[0], &mut short));
        assert!(!aesdec_many_into(&input, keys[0], &mut short));
        assert!(!aesenc_n_rounds_many_into(&input, &keys, &mut short));
        assert!(!aesdec_n_rounds_many_into(&input, &keys, &mut short));
        assert!(!aes128_encrypt_many_into(&input, &keys, &mut short));
        assert!(!aes128_decrypt_many_into(&input, &keys, &mut short));
        assert_eq!(short, [[0xa5; 16]]);
        let mut output = [[0; 16]; 2];
        assert!(aesenc_n_rounds_many_into(&input, &[], &mut output));
        assert_eq!(output, input);
        assert!(aesdec_n_rounds_many_into(&input, &[], &mut output));
        assert_eq!(output, input);
        assert!(aesenc_many_into(&[], keys[0], &mut []));
        assert!(aesdec_many_into(&[], keys[0], &mut []));
        assert!(aes128_encrypt_many_into(&[], &keys, &mut []));
        assert!(aes128_decrypt_many_into(&[], &keys, &mut []));
    }
}
