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
    ivm::sha3::keccak_f1600_impl(&mut st_cpu);
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
    use rand_core::OsRng;
    let mut rng = OsRng;
    let keypair = SigningKey::generate(&mut rng);
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
    use rand_core::OsRng;
    let mut rng = OsRng;
    let key1 = SigningKey::generate(&mut rng);
    let key2 = SigningKey::generate(&mut rng);

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
