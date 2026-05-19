//! Extra CUDA public-helper parity and adversarial coverage.

#[cfg(feature = "cuda")]
#[test]
fn test_cuda_poseidon2() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }
    if ivm::GpuManager::shared().is_none() {
        eprintln!("Failed to init GpuManager; skipping test");
        return;
    }
    let a = 1u64;
    let b = 2u64;
    let cpu = ivm::poseidon2_simd(a, b);
    if let Some(gpu) = ivm::poseidon2_cuda(a, b) {
        assert_eq!(gpu, cpu);
    } else {
        eprintln!("CUDA Poseidon2 path unavailable; skipping");
    }
}

#[cfg(feature = "cuda")]
#[test]
fn test_cuda_keccak() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }
    if ivm::GpuManager::shared().is_none() {
        eprintln!("Failed to init GpuManager; skipping test");
        return;
    }
    let mut st_cpu = [0u64; 25];
    let mut st_gpu = [0u64; 25];
    ivm::keccak_f1600(&mut st_cpu);
    if ivm::keccak_f1600_cuda(&mut st_gpu) {
        assert_eq!(st_gpu, st_cpu);
    } else {
        eprintln!("CUDA Keccak path unavailable; skipping");
    }
}

#[cfg(feature = "cuda")]
#[test]
fn test_cuda_aesenc() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }
    if ivm::GpuManager::shared().is_none() {
        eprintln!("Failed to init GpuManager; skipping test");
        return;
    }
    let state = [0u8; 16];
    let rk = [1u8; 16];
    let cpu = ivm::aesenc_impl(state, rk);
    if let Some(gpu) = ivm::aesenc_cuda(state, rk) {
        assert_eq!(gpu, cpu);
    } else {
        eprintln!("CUDA AESENC path unavailable; skipping");
    }
}

#[cfg(feature = "cuda")]
#[test]
fn test_cuda_bn254_add() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }
    if ivm::GpuManager::shared().is_none() {
        eprintln!("Failed to init GpuManager; skipping test");
        return;
    }
    let a = ivm::bn254_vec::FieldElem::from_u64(3);
    let b = ivm::bn254_vec::FieldElem::from_u64(4);
    let cpu = ivm::bn254_vec::add_scalar(a, b);
    if let Some(gpu) = ivm::bn254_add_cuda(a.0, b.0) {
        assert_eq!(gpu, cpu.0);
    } else {
        eprintln!("CUDA BN254 add path unavailable; skipping");
    }
}
#[cfg(feature = "cuda")]
#[test]
fn test_cuda_aesdec() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }
    if ivm::GpuManager::shared().is_none() {
        eprintln!("Failed to init GpuManager; skipping test");
        return;
    }
    let state = [0u8; 16];
    let rk = [1u8; 16];
    let enc = ivm::aesenc_impl(state, rk);
    let cpu = ivm::aesdec_impl(enc, rk);
    if let Some(gpu) = ivm::aesdec_cuda(enc, rk) {
        assert_eq!(gpu, cpu);
    } else {
        eprintln!("CUDA AESDEC path unavailable; skipping");
    }
}

#[cfg(feature = "cuda")]
#[test]
fn test_cuda_bn254_sub() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }
    if ivm::GpuManager::shared().is_none() {
        eprintln!("Failed to init GpuManager; skipping test");
        return;
    }
    let a = ivm::bn254_vec::FieldElem::from_u64(5);
    let b = ivm::bn254_vec::FieldElem::from_u64(2);
    let cpu = ivm::bn254_vec::sub_scalar(a, b);
    if let Some(gpu) = ivm::bn254_sub_cuda(a.0, b.0) {
        assert_eq!(gpu, cpu.0);
    } else {
        eprintln!("CUDA BN254 sub path unavailable; skipping");
    }
}

#[cfg(feature = "cuda")]
#[test]
fn test_cuda_bn254_mul() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }
    if ivm::GpuManager::shared().is_none() {
        eprintln!("Failed to init GpuManager; skipping test");
        return;
    }
    let a = ivm::bn254_vec::FieldElem::from_u64(3);
    let b = ivm::bn254_vec::FieldElem::from_u64(5);
    let cpu = ivm::bn254_vec::mul_scalar(a, b);
    if let Some(gpu) = ivm::bn254_mul_cuda(a.0, b.0) {
        assert_eq!(gpu, cpu.0);
    } else {
        eprintln!("CUDA BN254 mul path unavailable; skipping");
    }
}

#[cfg(feature = "cuda")]
#[test]
fn test_cuda_poseidon6() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }
    if ivm::GpuManager::shared().is_none() {
        eprintln!("Failed to init GpuManager; skipping test");
        return;
    }
    let inputs = [1u64, 2, 3, 4, 5, 6];
    let cpu = ivm::poseidon6_simd(inputs);
    if let Some(gpu) = ivm::poseidon6_cuda(inputs) {
        assert_eq!(gpu, cpu);
    } else {
        eprintln!("CUDA Poseidon6 path unavailable; skipping");
    }
}

#[cfg(feature = "cuda")]
#[test]
fn test_cuda_ed25519_verify() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }
    if ivm::GpuManager::shared().is_none() {
        eprintln!("Failed to init GpuManager; skipping test");
        return;
    }
    use ed25519_dalek::{Signer, SigningKey};
    let keypair = SigningKey::from_bytes(&[0x11; 32]);
    let msg = b"cuda ed25519";
    let sig = keypair.sign(msg);
    let pk_bytes = keypair.verifying_key().to_bytes();
    let cpu = keypair.verifying_key().verify_strict(msg, &sig).is_ok();
    if let Some(gpu) = ivm::ed25519_verify_cuda(msg, &sig.to_bytes(), &pk_bytes) {
        assert_eq!(gpu, cpu);
        let mut bad = sig.to_bytes();
        bad[0] ^= 0x42;
        if let Some(gpu_bad) = ivm::ed25519_verify_cuda(msg, &bad, &pk_bytes) {
            assert!(!gpu_bad);
        }
    } else {
        eprintln!("CUDA ed25519 verify path unavailable; skipping");
    }
}

#[cfg(feature = "cuda")]
fn compute_hram(sig: &[u8; 64], pk: &[u8; 32], msg: &[u8]) -> [u8; 32] {
    use curve25519_dalek::scalar::Scalar;
    use sha2::Digest;
    let mut hasher = sha2::Sha512::new();
    hasher.update(&sig[..32]);
    hasher.update(pk);
    hasher.update(msg);
    Scalar::from_hash(hasher).to_bytes()
}

#[cfg(feature = "cuda")]
#[test]
fn cuda_public_helpers_reject_adversarial_shape_mismatches() {
    assert!(ivm::vector_add_f32(&[1.0, 2.0, 3.0], &[4.0, 5.0]).is_none());
    assert!(ivm::vadd32_cuda(&[1u32, 2, 3], &[4u32, 5]).is_none());
    assert!(ivm::vadd64_cuda(&[1u64, 2, 3], &[4u64, 5]).is_none());
    assert!(ivm::vand_cuda(&[1u32, 2, 3], &[4u32, 5]).is_none());
    assert!(ivm::vxor_cuda(&[1u32, 2, 3], &[4u32, 5]).is_none());
    assert!(ivm::vor_cuda(&[1u32, 2, 3], &[4u32, 5]).is_none());

    let lhs = [[1u64, 2, 3, 4], [5, 6, 7, 8]];
    let rhs = [[9u64, 10, 11, 12]];
    assert!(ivm::bn254_add_batch_cuda(&lhs, &rhs).is_none());
    assert!(ivm::bn254_sub_batch_cuda(&lhs, &rhs).is_none());
    assert!(ivm::bn254_mul_batch_cuda(&lhs, &rhs).is_none());

    let signatures = [[0x11u8; 64], [0x22; 64]];
    let public_keys = [[0x33u8; 32], [0x44; 32]];
    let hrams = [[0x55u8; 32], [0x66; 32]];
    assert!(ivm::ed25519_verify_batch_cuda(&signatures, &public_keys[..1], &hrams).is_none());
    assert!(ivm::ed25519_verify_batch_cuda(&signatures, &public_keys, &hrams[..1]).is_none());
}

#[cfg(feature = "cuda")]
#[test]
fn cuda_adversarial_rejections_preserve_caller_buffers() {
    let original_hi = [9u64, 1, 9, 0];
    let original_lo = [2u64, 3, 1, 4];
    let mut hi = original_hi;
    let mut lo = original_lo;

    {
        let lo_short = &mut lo[..3];
        assert!(
            ivm::bitonic_sort_pairs(&mut hi, lo_short).is_none(),
            "mismatched bitonic-sort buffers must be rejected"
        );
    }

    assert_eq!(
        hi, original_hi,
        "rejected bitonic sort must not mutate the high-word buffer"
    );
    assert_eq!(
        lo, original_lo,
        "rejected bitonic sort must not mutate the low-word buffer"
    );
}

#[cfg(feature = "cuda")]
#[test]
fn cuda_empty_vector_boundaries_short_circuit_without_device_work() {
    assert_eq!(ivm::vector_add_f32(&[], &[]), Some(Vec::<f32>::new()));
    assert_eq!(ivm::vadd32_cuda(&[], &[]), Some(Vec::<u32>::new()));
    assert_eq!(ivm::vadd64_cuda(&[], &[]), Some(Vec::<u64>::new()));
    assert_eq!(ivm::vand_cuda(&[], &[]), Some(Vec::<u32>::new()));
    assert_eq!(ivm::vxor_cuda(&[], &[]), Some(Vec::<u32>::new()));
    assert_eq!(ivm::vor_cuda(&[], &[]), Some(Vec::<u32>::new()));

    assert!(ivm::vector_add_f32(&[], &[1.0]).is_none());
    assert!(ivm::vadd32_cuda(&[], &[1]).is_none());
    assert!(ivm::vadd64_cuda(&[], &[1]).is_none());
    assert!(ivm::vand_cuda(&[], &[1]).is_none());
    assert!(ivm::vxor_cuda(&[], &[1]).is_none());
    assert!(ivm::vor_cuda(&[], &[1]).is_none());
}

#[cfg(feature = "cuda")]
#[test]
fn cuda_empty_and_singleton_boundaries_short_circuit_without_device_work() {
    let mut empty_hi: [u64; 0] = [];
    let mut empty_lo: [u64; 0] = [];
    assert_eq!(
        ivm::bitonic_sort_pairs(&mut empty_hi, &mut empty_lo),
        Some(())
    );
    assert_eq!(ivm::sha256_leaves_cuda(&[]), Some(Vec::new()));
    assert_eq!(
        ivm::ed25519_verify_batch_cuda(&[], &[], &[]),
        Some(Vec::new())
    );
    assert_eq!(ivm::poseidon2_cuda_many(&[]), Some(Vec::new()));
    assert_eq!(ivm::poseidon6_cuda_many(&[]), Some(Vec::new()));
    assert_eq!(ivm::aesenc_batch_cuda(&[], [0u8; 16]), Some(Vec::new()));
    assert_eq!(ivm::aesdec_batch_cuda(&[], [0u8; 16]), Some(Vec::new()));
    assert_eq!(
        ivm::aesenc_rounds_batch_cuda(&[[0x42u8; 16]], &[]),
        Some(vec![[0x42u8; 16]])
    );
    assert_eq!(
        ivm::aesdec_rounds_batch_cuda(&[[0x24u8; 16]], &[]),
        Some(vec![[0x24u8; 16]])
    );

    let digest = [0xa5u8; 32];
    assert_eq!(ivm::sha256_pairs_reduce_cuda(&[]), None);
    assert_eq!(ivm::sha256_pairs_reduce_cuda(&[digest]), Some(digest));
}

#[cfg(feature = "cuda")]
#[test]
fn cuda_ed25519_does_not_accept_adversarial_public_key_bytes() {
    use ed25519_dalek::{Signer, SigningKey};

    let keypair = SigningKey::from_bytes(&[0x44; 32]);
    let msg = b"cuda invalid public key";
    let sig = keypair.sign(msg).to_bytes();
    let adversarial_pk = [0xffu8; 32];

    assert_ne!(
        ivm::ed25519_verify_cuda(msg, &sig, &adversarial_pk),
        Some(true),
        "adversarial Ed25519 public-key bytes must not verify successfully"
    );
}

#[cfg(feature = "cuda")]
#[test]
fn test_cuda_ed25519_verify_batch() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }
    if ivm::GpuManager::shared().is_none() {
        eprintln!("Failed to init GpuManager; skipping test");
        return;
    }
    use ed25519_dalek::{Signer, SigningKey};
    let key1 = SigningKey::from_bytes(&[0x22; 32]);
    let key2 = SigningKey::from_bytes(&[0x33; 32]);

    let msg1 = b"cuda batch one";
    let msg2 = b"cuda batch two";
    let sig1 = key1.sign(msg1);
    let sig2 = key2.sign(msg2);

    let mut bad_sig2 = sig2.to_bytes();
    bad_sig2[0] ^= 0x11;

    let pks = vec![
        key1.verifying_key().to_bytes(),
        key2.verifying_key().to_bytes(),
    ];
    let sigs = vec![sig1.to_bytes(), bad_sig2];
    let hrams = vec![
        compute_hram(&sigs[0], &pks[0], msg1),
        compute_hram(&sigs[1], &pks[1], msg2),
    ];

    if let Some(gpu_results) = ivm::ed25519_verify_batch_cuda(&sigs, &pks, &hrams) {
        assert_eq!(gpu_results, vec![true, false]);
    } else {
        eprintln!("CUDA ed25519 batch verify path unavailable; skipping");
    }
}

#[cfg(feature = "cuda")]
#[test]
fn test_cuda_ed25519_verify_batch_rejects_adversarial_hram() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }
    if ivm::GpuManager::shared().is_none() {
        eprintln!("Failed to init GpuManager; skipping test");
        return;
    }
    use ed25519_dalek::{Signer, SigningKey};
    let key1 = SigningKey::from_bytes(&[0x55; 32]);
    let key2 = SigningKey::from_bytes(&[0x66; 32]);

    let msg1 = b"cuda adversarial hram one";
    let msg2 = b"cuda adversarial hram two";
    let sig1 = key1.sign(msg1).to_bytes();
    let sig2 = key2.sign(msg2).to_bytes();
    let pk1 = key1.verifying_key().to_bytes();
    let pk2 = key2.verifying_key().to_bytes();

    let sigs = vec![sig1, sig2];
    let pks = vec![pk1, pk2];
    let mut hrams = vec![
        compute_hram(&sigs[0], &pks[0], msg1),
        compute_hram(&sigs[1], &pks[1], msg2),
    ];
    hrams[1][0] ^= 0x80;

    if let Some(gpu_results) = ivm::ed25519_verify_batch_cuda(&sigs, &pks, &hrams) {
        assert_eq!(
            gpu_results,
            vec![true, false],
            "tampered Ed25519 challenge scalar must be rejected by the CUDA batch verifier"
        );
    } else {
        eprintln!("CUDA ed25519 batch verify path unavailable; skipping");
    }
}
