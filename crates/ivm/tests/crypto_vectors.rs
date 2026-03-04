//! Cross-backend crypto/vector conformance checks.

use std::sync::{Mutex, MutexGuard, OnceLock};

use ivm::{AccelerationConfig, aesdec, aesdec_impl, aesenc, aesenc_impl, sha256_compress, vadd32};
use sha2::{Digest, Sha256};

const SHA256_IV: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

fn sha256_single_block(message: &[u8]) -> [u8; 64] {
    assert!(message.len() < 56, "message must fit in one SHA-256 block");
    let mut block = [0u8; 64];
    block[..message.len()].copy_from_slice(message);
    block[message.len()] = 0x80;
    let bit_len = (message.len() as u64) * 8;
    block[56..].copy_from_slice(&bit_len.to_be_bytes());
    block
}

fn state_to_bytes(state: [u32; 8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (idx, word) in state.iter().enumerate() {
        out[idx * 4..idx * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

struct AccelConfigGuard {
    _lock: MutexGuard<'static, ()>,
    original: AccelerationConfig,
}

impl AccelConfigGuard {
    fn new() -> Self {
        fn accel_test_lock() -> &'static Mutex<()> {
            static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
            LOCK.get_or_init(|| Mutex::new(()))
        }
        let lock = accel_test_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        Self {
            _lock: lock,
            original: ivm::acceleration_config(),
        }
    }
}

impl Drop for AccelConfigGuard {
    fn drop(&mut self) {
        ivm::set_acceleration_config(self.original);
    }
}

#[test]
fn sha256_compress_matches_sha2_digest() {
    let block = sha256_single_block(b"abc");
    let mut state = SHA256_IV;
    sha256_compress(&mut state, &block);
    let digest = Sha256::digest(b"abc");
    let mut expected = [0u8; 32];
    expected.copy_from_slice(&digest);
    assert_eq!(state_to_bytes(state), expected);
}

#[test]
fn aes_rounds_match_scalar_impl() {
    let state = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ];
    let round_key = [
        0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf, 0x4f,
        0x3c,
    ];
    assert_eq!(aesenc(state, round_key), aesenc_impl(state, round_key));
    assert_eq!(aesdec(state, round_key), aesdec_impl(state, round_key));
}

#[test]
fn vadd32_matches_scalar_when_simd_enabled() {
    let guard = AccelConfigGuard::new();
    let a = [0xffff_ff00, 0x0, 0x7fff_fffe, 0x8000_0000];
    let b = [0x10, 0x20, 0x1, 0x7fff_ffff];

    ivm::set_acceleration_config(AccelerationConfig {
        enable_simd: false,
        enable_metal: false,
        enable_cuda: false,
        ..guard.original
    });
    let scalar = vadd32(a, b);

    ivm::set_acceleration_config(AccelerationConfig {
        enable_simd: true,
        enable_metal: false,
        enable_cuda: false,
        ..guard.original
    });
    let simd = vadd32(a, b);

    assert_eq!(scalar, simd);
}

#[test]
fn aesenc_matches_across_acceleration_configs() {
    let guard = AccelConfigGuard::new();
    let state = [
        0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96, 0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93, 0x17,
        0x2a,
    ];
    let round_key = [
        0xae, 0x2d, 0x8a, 0x57, 0x1e, 0x03, 0xac, 0x9c, 0x9e, 0xb7, 0x6f, 0xac, 0x45, 0xaf, 0x8e,
        0x51,
    ];

    ivm::set_acceleration_config(AccelerationConfig {
        enable_simd: false,
        enable_metal: false,
        enable_cuda: false,
        ..guard.original
    });
    let cpu = aesenc(state, round_key);

    ivm::set_acceleration_config(AccelerationConfig {
        enable_simd: true,
        enable_metal: true,
        enable_cuda: true,
        ..guard.original
    });
    let accel = aesenc(state, round_key);

    assert_eq!(cpu, accel);
}
