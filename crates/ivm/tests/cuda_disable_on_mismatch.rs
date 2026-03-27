//! Acceptance: disable CUDA backend on golden-vector mismatch.
//!
//! Verifies that:
//! - the backend disables itself cleanly on forced self-test failure or config disable;
//! - documented CPU fallbacks still match known outputs; and
//! - explicit CUDA entry points fail closed instead of returning drifted output.

#![cfg(feature = "cuda")]

use std::sync::{Mutex, MutexGuard, OnceLock};

const SHA256_ABC_DIGEST: [u8; 32] = [
    0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22, 0x23,
    0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad,
];

fn cuda_disable_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn restore_env_var(name: &str, value: &Option<String>) {
    unsafe {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
}

struct CudaDisableTestGuard {
    _lock: MutexGuard<'static, ()>,
    original_accel: ivm::AccelerationConfig,
    original_force_fail: Option<String>,
    original_disable_cuda: Option<String>,
}

impl CudaDisableTestGuard {
    fn new() -> Self {
        let lock = cuda_disable_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        Self {
            _lock: lock,
            original_accel: ivm::acceleration_config(),
            original_force_fail: std::env::var("IVM_FORCE_CUDA_SELFTEST_FAIL").ok(),
            original_disable_cuda: std::env::var("IVM_DISABLE_CUDA").ok(),
        }
    }
}

impl Drop for CudaDisableTestGuard {
    fn drop(&mut self) {
        restore_env_var("IVM_FORCE_CUDA_SELFTEST_FAIL", &self.original_force_fail);
        restore_env_var("IVM_DISABLE_CUDA", &self.original_disable_cuda);
        ivm::reset_cuda_backend_for_tests();
        ivm::set_acceleration_config(self.original_accel);
    }
}

fn sha256_single_block_abc() -> [u8; 64] {
    let mut block = [0u8; 64];
    block[0] = b'a';
    block[1] = b'b';
    block[2] = b'c';
    block[3] = 0x80;
    block[63] = 24;
    block
}

fn digest_bytes(state: [u32; 8]) -> [u8; 32] {
    let mut digest = [0u8; 32];
    for (index, word) in state.iter().enumerate() {
        digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

fn assert_cuda_disabled_surface_behaves() {
    let initial_state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    let block = sha256_single_block_abc();

    let mut cuda_state = initial_state;
    assert!(
        !ivm::sha256_compress_cuda(&mut cuda_state, &block),
        "explicit CUDA SHA-256 helper should fail closed while CUDA is disabled"
    );
    assert_eq!(
        cuda_state, initial_state,
        "failed CUDA SHA-256 helper must not mutate the digest state"
    );

    let mut cpu_state = initial_state;
    ivm::sha256_compress(&mut cpu_state, &block);
    assert_eq!(
        digest_bytes(cpu_state),
        SHA256_ABC_DIGEST,
        "scalar fallback must match the known SHA-256 digest"
    );

    let poseidon_expected = ivm::poseidon2_simd(7, 9);
    assert_eq!(
        ivm::poseidon2(7, 9),
        poseidon_expected,
        "poseidon2 should fall back to the scalar path when CUDA is disabled"
    );
    assert!(
        ivm::poseidon2_cuda(7, 9).is_none(),
        "poseidon2_cuda should return None when CUDA is disabled"
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

    let states = [
        [
            0x00u8, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ],
        [
            0xffu8, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x99, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22,
            0x11, 0x00,
        ],
    ];
    let round_keys = [
        [
            0x0f, 0x15, 0x71, 0xc9, 0x47, 0xd9, 0xe8, 0x59, 0x0c, 0xb7, 0xad, 0xd6, 0xaf, 0x7f,
            0x67, 0x98,
        ],
        [
            0xa5, 0x40, 0x76, 0x28, 0x10, 0x4f, 0xdc, 0xe6, 0x43, 0xdd, 0x27, 0x0f, 0x6c, 0xa7,
            0x63, 0x6f,
        ],
        [
            0x2c, 0x5e, 0x2f, 0x88, 0x6a, 0x84, 0xd2, 0x57, 0x8b, 0x3f, 0x8c, 0x9c, 0x4f, 0x11,
            0x64, 0x15,
        ],
    ];
    let expected_enc: Vec<[u8; 16]> = states
        .iter()
        .copied()
        .map(|block| {
            round_keys
                .iter()
                .copied()
                .fold(block, |value, round_key| ivm::aesenc_impl(value, round_key))
        })
        .collect();
    let expected_dec: Vec<[u8; 16]> = states
        .iter()
        .copied()
        .map(|block| {
            round_keys
                .iter()
                .copied()
                .fold(block, |value, round_key| ivm::aesdec_impl(value, round_key))
        })
        .collect();
    let expected_single_round_enc: Vec<[u8; 16]> = states
        .iter()
        .copied()
        .map(|block| ivm::aesenc_impl(block, rk))
        .collect();
    let expected_single_round_dec: Vec<[u8; 16]> = states
        .iter()
        .copied()
        .map(|block| ivm::aesdec_impl(block, rk))
        .collect();
    assert_eq!(
        ivm::aesenc_batch_cuda(&states, rk),
        Some(expected_single_round_enc),
        "aesenc_batch_cuda should return CPU-computed output when CUDA is disabled"
    );
    assert_eq!(
        ivm::aesdec_batch_cuda(&states, rk),
        Some(expected_single_round_dec),
        "aesdec_batch_cuda should return CPU-computed output when CUDA is disabled"
    );
    assert_eq!(
        ivm::aesenc_rounds_batch_cuda(&states, &round_keys),
        Some(expected_enc.clone()),
        "aesenc_rounds_batch_cuda should return CPU-computed output when CUDA is disabled"
    );
    assert_eq!(
        ivm::aesdec_rounds_batch_cuda(&states, &round_keys),
        Some(expected_dec.clone()),
        "aesdec_rounds_batch_cuda should return CPU-computed output when CUDA is disabled"
    );
    assert_eq!(
        ivm::aesenc_n_rounds_many(&states, &round_keys),
        expected_enc,
        "aesenc_n_rounds_many should preserve deterministic CPU output when CUDA is disabled"
    );
    assert_eq!(
        ivm::aesdec_n_rounds_many(&states, &round_keys),
        expected_dec,
        "aesdec_n_rounds_many should preserve deterministic CPU output when CUDA is disabled"
    );

    let lhs = [1.0f32, -2.5, 3.25, 4.5];
    let rhs = [2.0f32, 0.5, -1.25, 3.5];
    assert!(
        ivm::vector_add_f32(&lhs, &rhs).is_none(),
        "explicit CUDA vector helper should fail closed while CUDA is disabled"
    );
    assert!(ivm::vadd32_cuda(&[1u32, 2], &[3u32, 4]).is_none());
    assert!(ivm::vadd64_cuda(&[1u64, 2], &[3u64, 4]).is_none());
    assert!(ivm::vand_cuda(&[1u32, 2], &[3u32, 4]).is_none());
    assert!(ivm::vxor_cuda(&[1u32, 2], &[3u32, 4]).is_none());
    assert!(ivm::vor_cuda(&[1u32, 2], &[3u32, 4]).is_none());

    let leaf_blocks = [block];
    assert!(
        ivm::sha256_leaves_cuda(&leaf_blocks).is_none(),
        "explicit CUDA SHA-256 leaves helper should fail closed while CUDA is disabled"
    );
    assert!(
        ivm::sha256_pairs_reduce_cuda(&[[0u8; 32], [1u8; 32]]).is_none(),
        "explicit CUDA SHA-256 pair-reduce helper should fail closed while CUDA is disabled"
    );

    let original_keccak_state = core::array::from_fn(|index| index as u64);
    let mut keccak_state = original_keccak_state;
    assert!(
        !ivm::keccak_f1600_cuda(&mut keccak_state),
        "explicit CUDA keccak helper should fail closed while CUDA is disabled"
    );
    assert_eq!(
        keccak_state, original_keccak_state,
        "failed CUDA keccak helper must not mutate the state"
    );

    let bn254_lhs = [1u64, 2, 3, 4];
    let bn254_rhs = [5u64, 6, 7, 8];
    assert!(ivm::bn254_add_cuda(bn254_lhs, bn254_rhs).is_none());
    assert!(ivm::bn254_sub_cuda(bn254_lhs, bn254_rhs).is_none());
    assert!(ivm::bn254_mul_cuda(bn254_lhs, bn254_rhs).is_none());

    let msg = b"cuda disable regression";
    let sig = [0u8; 64];
    let pk = [0u8; 32];
    assert!(
        ivm::ed25519_verify_cuda(msg, &sig, &pk).is_none(),
        "explicit CUDA ed25519 helper should fail closed while CUDA is disabled"
    );
    assert!(
        ivm::ed25519_verify_batch_cuda(&[sig], &[pk], &[[0u8; 32]]).is_none(),
        "explicit CUDA ed25519 batch helper should fail closed while CUDA is disabled"
    );
}

#[test]
fn cuda_backend_disables_on_forced_selftest_failure_and_parity_holds() {
    let _guard = CudaDisableTestGuard::new();
    ivm::reset_cuda_backend_for_tests();
    unsafe {
        std::env::set_var("IVM_FORCE_CUDA_SELFTEST_FAIL", "1");
        std::env::set_var("IVM_DISABLE_CUDA", "0");
    }

    assert!(
        !ivm::cuda_available(),
        "cuda_available should be false after forced self-test fail"
    );
    assert!(
        ivm::cuda_disabled(),
        "cuda_disabled should be true after forced self-test fail"
    );
    assert!(
        ivm::cuda_last_error_message()
            .as_deref()
            .is_some_and(|message| message.contains("IVM_FORCE_CUDA_SELFTEST_FAIL")),
        "forced self-test failure should report a diagnostic message"
    );

    assert_cuda_disabled_surface_behaves();
}

#[test]
fn cuda_backend_respects_config_disable_and_falls_back() {
    let guard = CudaDisableTestGuard::new();
    ivm::reset_cuda_backend_for_tests();
    ivm::set_acceleration_config(ivm::AccelerationConfig {
        enable_cuda: true,
        ..guard.original_accel
    });

    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }

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
    let block = sha256_single_block_abc();
    assert!(
        ivm::sha256_compress_cuda(&mut gpu_state, &block),
        "CUDA path should succeed before config disable"
    );

    ivm::set_acceleration_config(ivm::AccelerationConfig {
        enable_cuda: false,
        ..guard.original_accel
    });

    assert!(
        !ivm::cuda_available(),
        "cuda_available should be false after config disable"
    );
    assert!(
        ivm::cuda_disabled(),
        "cuda_disabled should reflect config gating"
    );
    assert_eq!(
        ivm::cuda_last_error_message().as_deref(),
        Some("disabled by configuration"),
        "config disable should surface the canonical diagnostic message"
    );

    assert_cuda_disabled_surface_behaves();
}
