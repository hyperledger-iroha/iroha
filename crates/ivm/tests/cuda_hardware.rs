//! Required CUDA qualification: every kernel family must execute on every usable device.
//!
//! This dedicated process owns its backend policy. A missing GPU, scalar fallback,
//! self-test-only execution, or nonmatching output fails qualification.

use ed25519_dalek::{Signer as _, SigningKey};
use ivm::bn254_vec::{FieldElem, add_scalar, mul_scalar, sub_scalar};
use sha2::{Digest as _, Sha256};

fn require_dispatch<T: std::fmt::Debug + PartialEq>(
    label: &str,
    expected: T,
    operation: impl FnOnce() -> T,
) {
    let before = ivm::cuda_completed_dispatches();
    assert_eq!(operation(), expected, "{label}: scalar parity");
    let completed = ivm::cuda_completed_dispatches().saturating_sub(before);
    assert!(completed > 0, "{label}: no completed CUDA kernel batch");
    assert!(
        !ivm::cuda_disabled(),
        "{label}: backend disabled during work"
    );
    println!("IVM_CUDA_RECEIPT operation={label} completed_batches={completed}");
}

#[test]
fn required_cuda_hardware_covers_every_kernel_family_on_every_device() {
    // Build reference outputs before CUDA is enabled, including helpers with
    // their own optional dispatch. Hardware calibration never changes inputs.
    ivm::set_acceleration_config(ivm::AccelerationConfig {
        enable_simd: false,
        enable_metal: false,
        enable_cuda: false,
        ..Default::default()
    });
    let initial_sha = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut block = [0u8; 64];
    block[..3].copy_from_slice(b"abc");
    block[3] = 0x80;
    block[63] = 24;
    let mut expected_sha = initial_sha;
    ivm::sha256_compress(&mut expected_sha, &block);
    let leaf: [u8; 32] = Sha256::digest(b"abc").into();
    let mut pair_block = [0u8; 64];
    pair_block[..32].copy_from_slice(&leaf);
    pair_block[32..].copy_from_slice(&leaf);
    let expected_pair: [u8; 32] = Sha256::digest(pair_block).into();
    let mut expected_keccak = [0u64; 25];
    ivm::keccak_f1600(&mut expected_keccak);
    let poseidon2 = ivm::poseidon2_simd(7, 11);
    let poseidon6 = ivm::poseidon6_simd([1, 2, 3, 4, 5, 6]);
    let state = [0x53; 16];
    let round_key = [0xCA; 16];
    let aes_enc = ivm::aesenc_impl(state, round_key);
    let aes_dec = ivm::aesdec_impl(state, round_key);
    let round_keys = [round_key, [0x35; 16]];
    let aes_enc_rounds = ivm::aesenc_impl(aes_enc, round_keys[1]);
    let aes_dec_rounds = ivm::aesdec_impl(aes_dec, round_keys[1]);
    let a = FieldElem::from_u64(17);
    let b = FieldElem::from_u64(29);
    let signing_key = SigningKey::from_bytes(&[0x53; 32]);
    let message = b"ivm required CUDA hardware qualification";
    let signature = signing_key.sign(message).to_bytes();
    let public_key = signing_key.verifying_key().to_bytes();

    ivm::set_acceleration_config(ivm::AccelerationConfig {
        enable_simd: false,
        enable_metal: false,
        enable_cuda: true,
        ..Default::default()
    });
    assert!(
        ivm::cuda_available(),
        "required CUDA backend unavailable: {:?}",
        ivm::cuda_last_error_message()
    );
    let device_slots = ivm::cuda_device_slots();
    assert!(device_slots > 0, "required CUDA device missing");
    assert!(ivm::with_cuda_device_for_qualification(device_slots, || ()).is_none());
    assert_eq!(ivm::cuda_qualification_device(), None);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ivm::with_cuda_device_for_qualification(0, || {
            assert_eq!(ivm::cuda_qualification_device(), Some(0));
            panic!("qualification unwind control")
        })
    }));
    assert!(unwind.is_err());
    assert_eq!(ivm::cuda_qualification_device(), None);
    for device in 0..device_slots {
        println!("IVM_CUDA_DEVICE index={device}");
        ivm::with_cuda_device_for_qualification(device, || {
            require_dispatch("vector_add32", (true, [0, 5, 7]), || {
                let mut output = [0; 3];
                let completed = ivm::vadd32_cuda_into(&[u32::MAX, 2, 3], &[1, 3, 4], &mut output);
                (completed, output)
            });
            require_dispatch("vector_add64", (true, [0, 5]), || {
                let mut output = [0; 2];
                let completed = ivm::vadd64_cuda_into(&[u64::MAX, 2], &[1, 3], &mut output);
                (completed, output)
            });
            require_dispatch("vector_and", (true, [0, 2]), || {
                let mut output = [0; 2];
                let completed = ivm::vand_cuda_into(&[1, 2], &[2, 3], &mut output);
                (completed, output)
            });
            require_dispatch("vector_xor", (true, [3, 1]), || {
                let mut output = [0; 2];
                let completed = ivm::vxor_cuda_into(&[1, 2], &[2, 3], &mut output);
                (completed, output)
            });
            require_dispatch("vector_or", (true, [3, 3]), || {
                let mut output = [0; 2];
                let completed = ivm::vor_cuda_into(&[1, 2], &[2, 3], &mut output);
                (completed, output)
            });
            require_dispatch("sha256", (true, expected_sha), || {
                let mut actual = initial_sha;
                let executed = ivm::sha256_compress_cuda(&mut actual, &block);
                (executed, actual)
            });
            require_dispatch("sha256_leaves", (true, [leaf, leaf]), || {
                let mut output = [[0; 32]; 2];
                let completed = ivm::sha256_leaves_cuda_into(&[block, block], &mut output);
                (completed, output)
            });
            require_dispatch("sha256_pairs_reduce", Some(expected_pair), || {
                ivm::sha256_pairs_reduce_cuda(&[leaf, leaf])
            });
            require_dispatch("keccak", (true, expected_keccak), || {
                let mut actual = [0u64; 25];
                let executed = ivm::keccak_f1600_cuda(&mut actual);
                (executed, actual)
            });
            require_dispatch("poseidon2", Some(poseidon2), || ivm::poseidon2_cuda(7, 11));
            require_dispatch("poseidon6", Some(poseidon6), || {
                ivm::poseidon6_cuda([1, 2, 3, 4, 5, 6])
            });
            require_dispatch("aes_encrypt_batch", (true, [aes_enc]), || {
                let mut output = [[0; 16]];
                let completed = ivm::aesenc_batch_cuda_into(&[state], round_key, &mut output);
                (completed, output)
            });
            require_dispatch("aes_decrypt_batch", (true, [aes_dec]), || {
                let mut output = [[0; 16]];
                let completed = ivm::aesdec_batch_cuda_into(&[state], round_key, &mut output);
                (completed, output)
            });
            require_dispatch("aes_encrypt_fused", (true, [aes_enc_rounds]), || {
                let mut output = [[0; 16]];
                let completed =
                    ivm::aesenc_rounds_batch_cuda_into(&[state], &round_keys, &mut output);
                (completed, output)
            });
            require_dispatch("aes_decrypt_fused", (true, [aes_dec_rounds]), || {
                let mut output = [[0; 16]];
                let completed =
                    ivm::aesdec_rounds_batch_cuda_into(&[state], &round_keys, &mut output);
                (completed, output)
            });
            require_dispatch("bn254_add", Some(add_scalar(a, b).0), || {
                ivm::bn254_add_cuda(a.0, b.0)
            });
            require_dispatch("bn254_sub", Some(sub_scalar(a, b).0), || {
                ivm::bn254_sub_cuda(a.0, b.0)
            });
            require_dispatch("bn254_mul", Some(mul_scalar(a, b).0), || {
                ivm::bn254_mul_cuda(a.0, b.0)
            });
            require_dispatch("ed25519", Some(true), || {
                ivm::ed25519_verify_cuda(message, &signature, &public_key)
            });
            require_dispatch("bitonic_sort", (Some(()), [1, 2, 2], [9, 3, 7]), || {
                let mut hi = [2, 1, 2];
                let mut lo = [7, 9, 3];
                let executed = ivm::bitonic_sort_pairs(&mut hi, &mut lo);
                (executed, hi, lo)
            });
        })
        .expect("enumerated CUDA device remains available");
    }
}

#[test]
#[should_panic(expected = "no completed CUDA kernel batch")]
fn qualification_rejects_matching_cpu_output_without_gpu_work() {
    require_dispatch("CPU fallback negative control", Some(42), || Some(42));
}
