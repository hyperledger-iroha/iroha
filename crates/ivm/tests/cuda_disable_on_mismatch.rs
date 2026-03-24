//! Acceptance: disable CUDA backend on golden-vector mismatch
//!
//! Forces the CUDA self-test to fail via an env var and verifies that:
//! - The backend is disabled (`cuda_available()` is false and `cuda_disabled()` is true)
//! - Fallback scalar path for SHA-256 still matches known vectors
//!
//! Skips when the `cuda` feature is not enabled.

#[cfg(feature = "cuda")]
#[test]
fn cuda_backend_disables_on_forced_selftest_failure_and_parity_holds() {
    ivm::reset_cuda_backend_for_tests();
    // Ensure self-test runs with the forced fail flag before any CUDA init.
    unsafe {
        std::env::set_var("IVM_FORCE_CUDA_SELFTEST_FAIL", "1");
        std::env::set_var("IVM_DISABLE_CUDA", "0");
    }

    let available = ivm::cuda_available();
    assert!(
        !available,
        "cuda_available should be false after forced self-test fail"
    );
    assert!(
        ivm::cuda_disabled(),
        "cuda_disabled should be true after forced fail"
    );

    // Verify SHA-256 known vector via fallback (CPU path)
    let mut state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut block = [0u8; 64];
    block[0] = b'a';
    block[1] = b'b';
    block[2] = b'c';
    block[3] = 0x80;
    block[63] = 24;
    ivm::sha256_compress(&mut state, &block);

    let mut digest = [0u8; 32];
    for (i, word) in state.iter().enumerate() {
        digest[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    let expected: [u8; 32] = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
    ];
    assert_eq!(digest, expected, "scalar fallback must match known digest");

    // Crypto helpers must fall back to CPU while CUDA is disabled.
    let poseidon_expected = ivm::poseidon2_simd(7, 9);
    assert_eq!(
        ivm::poseidon2(7, 9),
        poseidon_expected,
        "poseidon2 should fall back to the scalar path when CUDA is disabled"
    );
    assert!(
        ivm::poseidon2_cuda(7, 9).is_none(),
        "poseidon2_cuda should return None when CUDA fails self-tests"
    );

    let state = [0x42u8; 16];
    let rk = [0x24u8; 16];
    let aes_enc_cpu = ivm::aesenc_impl(state, rk);
    let aes_enc_cuda = ivm::aesenc_cuda(state, rk)
        .expect("aesenc_cuda should return Some even when falling back to CPU");
    assert_eq!(
        aes_enc_cuda, aes_enc_cpu,
        "aesenc_cuda should fall back to CPU output when CUDA is disabled"
    );
    let aes_dec_cpu = ivm::aesdec_impl(aes_enc_cpu, rk);
    let aes_dec_cuda = ivm::aesdec_cuda(aes_enc_cpu, rk)
        .expect("aesdec_cuda should return Some even when falling back to CPU");
    assert_eq!(
        aes_dec_cuda, aes_dec_cpu,
        "aesdec_cuda should fall back to CPU output when CUDA is disabled"
    );

    unsafe {
        std::env::remove_var("IVM_FORCE_CUDA_SELFTEST_FAIL");
        std::env::remove_var("IVM_DISABLE_CUDA");
    }
    ivm::reset_cuda_backend_for_tests();
}

#[cfg(feature = "cuda")]
#[test]
fn cuda_backend_respects_config_disable_and_falls_back() {
    ivm::reset_cuda_backend_for_tests();
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }

    // Sanity: GPU path should succeed before disabling.
    let mut gpu_state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut block = [0u8; 64];
    block[0] = b'a';
    block[1] = b'b';
    block[2] = b'c';
    block[3] = 0x80;
    block[63] = 24;
    assert!(
        ivm::sha256_compress_cuda(&mut gpu_state, &block),
        "CUDA path should succeed before disable"
    );

    ivm::set_acceleration_config(ivm::AccelerationConfig {
        enable_simd: true,
        enable_metal: true,
        enable_cuda: false,
        max_gpus: None,
        merkle_min_leaves_gpu: None,
        merkle_min_leaves_metal: None,
        merkle_min_leaves_cuda: None,
        prefer_cpu_sha2_max_leaves_aarch64: None,
        prefer_cpu_sha2_max_leaves_x86: None,
    });

    assert!(
        !ivm::cuda_available(),
        "cuda_available should be false after config disable"
    );
    assert!(
        ivm::cuda_disabled(),
        "cuda_disabled should reflect config gating"
    );

    let mut disabled_state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    assert!(
        !ivm::sha256_compress_cuda(&mut disabled_state, &block),
        "CUDA helper should report false when disabled"
    );
    ivm::sha256_compress(&mut disabled_state, &block);
    let mut digest = [0u8; 32];
    for (i, word) in disabled_state.iter().enumerate() {
        digest[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    let expected: [u8; 32] = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
    ];
    assert_eq!(digest, expected, "CPU fallback must match known digest");

    // poseidon2 falls back to the scalar path when CUDA is disabled via config.
    let poseidon_expected = ivm::poseidon2_simd(11, 13);
    assert_eq!(
        ivm::poseidon2(11, 13),
        poseidon_expected,
        "poseidon2 should fall back to CPU when CUDA is disabled in config"
    );
    assert!(
        ivm::poseidon2_cuda(11, 13).is_none(),
        "poseidon2_cuda should report None when CUDA is disabled in config"
    );

    // AES round helpers should still return the CPU output while disabled.
    let state = [0xAAu8; 16];
    let rk = [0xBBu8; 16];
    let aes_enc_cpu = ivm::aesenc_impl(state, rk);
    let aes_enc_cuda = ivm::aesenc_cuda(state, rk)
        .expect("aesenc_cuda returns Some with CPU output when CUDA is disabled");
    assert_eq!(aes_enc_cuda, aes_enc_cpu);
    let aes_dec_cpu = ivm::aesdec_impl(aes_enc_cpu, rk);
    let aes_dec_cuda = ivm::aesdec_cuda(aes_enc_cpu, rk)
        .expect("aesdec_cuda returns Some with CPU output when CUDA is disabled");
    assert_eq!(aes_dec_cuda, aes_dec_cpu);

    ivm::set_acceleration_config(ivm::AccelerationConfig {
        enable_simd: true,
        enable_metal: true,
        enable_cuda: true,
        max_gpus: None,
        merkle_min_leaves_gpu: None,
        merkle_min_leaves_metal: None,
        merkle_min_leaves_cuda: None,
        prefer_cpu_sha2_max_leaves_aarch64: None,
        prefer_cpu_sha2_max_leaves_x86: None,
    });
    ivm::reset_cuda_backend_for_tests();
}
