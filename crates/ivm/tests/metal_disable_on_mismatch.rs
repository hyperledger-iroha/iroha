//! Acceptance: disable Metal backend on golden-vector mismatch
//!
//! This test forces the Metal self-test to fail via an env var and verifies that:
//! - The backend is disabled (reported via `metal_available()` / `metal_disabled()`), and
//! - Execution falls back to the scalar path and still matches known vectors
//!
//! Skips on non-macOS or when the `metal` feature is not enabled.

#[cfg(target_os = "macos")]
#[test]
fn metal_backend_disables_on_forced_selftest_failure_and_parity_holds() {
    // If the metal feature is not enabled or no device, skip.
    if !cfg!(feature = "metal") {
        eprintln!("metal feature not enabled; skipping test");
        return;
    }

    // Safety: Ensure we start with a fresh state so init path runs.
    ivm::release_metal_state();

    // Force the self-test to fail
    #[cfg_attr(not(feature = "metal"), allow(unused_unsafe))]
    unsafe {
        std::env::set_var("IVM_FORCE_METAL_SELFTEST_FAIL", "1");
        // Also make sure it isn't re-enabled
        std::env::set_var("IVM_DISABLE_METAL", "0");
    }

    // Trigger Metal init path (this will run self-test and disable on mismatch)
    let _ = ivm::vadd32([1, 2, 3, 4], [4, 3, 2, 1]);
    let was_available = ivm::metal_available();
    // After forced failure, it must be false (disabled)
    assert!(
        !was_available,
        "metal_available should be false after forced self-test fail"
    );
    assert!(
        ivm::metal_disabled(),
        "metal_disabled should be true after forced fail"
    );

    // Verify fallback parity using the known "abc" single-block digest
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

    #[cfg_attr(not(feature = "metal"), allow(unused_unsafe))]
    unsafe {
        std::env::remove_var("IVM_FORCE_METAL_SELFTEST_FAIL");
        std::env::remove_var("IVM_DISABLE_METAL");
    }
    ivm::reset_metal_backend_for_tests();
}

#[cfg(all(target_os = "macos", feature = "metal"))]
#[test]
fn metal_backend_respects_config_disable_and_falls_back() {
    ivm::reset_metal_backend_for_tests();
    if !ivm::metal_available() {
        eprintln!("No Metal GPU available; skipping test");
        return;
    }

    ivm::release_metal_state();
    let pre_compiles = ivm::bit_pipe_compile_count();
    ivm::set_acceleration_config(ivm::AccelerationConfig {
        enable_simd: true,
        enable_metal: false,
        enable_cuda: true,
        max_gpus: None,
        merkle_min_leaves_gpu: None,
        merkle_min_leaves_metal: None,
        merkle_min_leaves_cuda: None,
        prefer_cpu_sha2_max_leaves_aarch64: None,
        prefer_cpu_sha2_max_leaves_x86: None,
    });

    assert!(
        !ivm::metal_available(),
        "metal_available should report false after config disable"
    );
    assert!(
        ivm::metal_disabled(),
        "metal_disabled should reflect config gating"
    );

    let result = ivm::vadd32([1, 2, 3, 4], [4, 3, 2, 1]);
    assert_eq!(result, [5, 5, 5, 5]);
    assert_eq!(
        ivm::bit_pipe_compile_count(),
        pre_compiles,
        "Disabling Metal must prevent pipeline compilation"
    );

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
    ivm::reset_metal_backend_for_tests();
}
