//! Ensure CUDA-gated helpers degrade to scalar paths when the `cuda` feature is
//! disabled. This guards the optional backend so non-GPU builds remain stable.

#![cfg(not(feature = "cuda"))]

#[test]
fn cuda_helpers_fall_back_when_disabled() {
    // Poseidon helpers return None without CUDA.
    assert!(ivm::poseidon2_cuda(0, 0).is_none());
    assert!(ivm::poseidon2_cuda_many(&[]).is_none());
    assert!(ivm::poseidon6_cuda([0; 6]).is_none());
    assert!(ivm::poseidon6_cuda_many(&[]).is_none());

    // Hashing helpers return false/None without CUDA.
    let mut keccak_state = [0u64; 25];
    assert!(!ivm::keccak_f1600_cuda(&mut keccak_state));
    let mut sha_state = [0u32; 8];
    assert!(!ivm::sha256_compress_cuda(&mut sha_state, &[0u8; 64]));
    assert!(ivm::sha256_leaves_cuda(&[[0u8; 64]]).is_none());
    assert!(ivm::sha256_pairs_reduce_cuda(&[[0u8; 32], [1u8; 32]]).is_none());

    // AES helpers return None without CUDA.
    assert!(ivm::aesenc_cuda([0u8; 16], [0u8; 16]).is_none());
    assert!(ivm::aesdec_cuda([0u8; 16], [0u8; 16]).is_none());
    assert!(ivm::aesenc_batch_cuda(&[[0u8; 16]], [0u8; 16]).is_none());
    assert!(ivm::aesdec_batch_cuda(&[[0u8; 16]], [0u8; 16]).is_none());
    assert!(ivm::aesenc_rounds_batch_cuda(&[[0u8; 16]], &[[0u8; 16]]).is_none());
    assert!(ivm::aesdec_rounds_batch_cuda(&[[0u8; 16]], &[[0u8; 16]]).is_none());

    // Sorting helper returns None without CUDA.
    let mut hi = [5u64, 3, 5, 3, 3];
    let mut lo = [7u64, 9, 1, 2, 1];
    assert!(ivm::bitonic_sort_pairs(&mut hi, &mut lo).is_none());
    assert_eq!(hi, [5u64, 3, 5, 3, 3]);
    assert_eq!(lo, [7u64, 9, 1, 2, 1]);

    // Vector CUDA helper entry points return None without CUDA.
    assert!(ivm::vector_add_f32(&[1.0, 2.0], &[3.0, 4.0]).is_none());
    assert!(ivm::vadd32_cuda(&[1u32, 2], &[3u32, 4]).is_none());
    assert!(ivm::vadd64_cuda(&[1u64, 2], &[3u64, 4]).is_none());
    assert!(ivm::vand_cuda(&[1u32, 2], &[3u32, 4]).is_none());
    assert!(ivm::vxor_cuda(&[1u32, 2], &[3u32, 4]).is_none());
    assert!(ivm::vor_cuda(&[1u32, 2], &[3u32, 4]).is_none());

    // BN254 and Ed25519 helpers fall back gracefully.
    assert!(ivm::bn254_add_cuda([0; 4], [0; 4]).is_none());
    assert!(ivm::bn254_sub_cuda([0; 4], [0; 4]).is_none());
    assert!(ivm::bn254_mul_cuda([0; 4], [0; 4]).is_none());
    assert!(ivm::ed25519_verify_cuda(&[], &[0; 64], &[0; 32]).is_none());
    assert!(ivm::ed25519_verify_batch_cuda(&[], &[], &[]).is_none());
}
