//! GPU-accelerated Merkle roots must match the CPU tree.
#[cfg(feature = "cuda")]
#[test]
fn merkle_root_accel_matches_cpu_for_large_input() {
    // Build a large buffer to exceed the GPU threshold (8192 leaves * 32 bytes = 262,144 bytes)
    let chunk = 32usize;
    let leaves = 10_000usize;
    let mut data = vec![0u8; leaves * chunk];
    for (i, byte) in data.iter_mut().enumerate() {
        *byte = (i as u8).wrapping_mul(31).wrapping_add(7);
    }
    if !ivm::cuda_available() {
        eprintln!("CUDA not available; skipping merkle accel test");
        return;
    }
    let t_cpu = ivm::ByteMerkleTree::from_bytes_parallel(&data, chunk).unwrap();
    let root_gpu = ivm::ByteMerkleTree::root_from_bytes_accel(&data, chunk).unwrap();
    assert_eq!(t_cpu.root(), root_gpu);
}
#[cfg(all(target_os = "macos", feature = "metal"))]
#[test]
fn merkle_root_accel_metal_matches_cpu_for_large_input() {
    if !ivm::metal_available() {
        eprintln!("Metal not available; skipping merkle accel test");
        return;
    }
    let chunk = 32usize;
    let leaves = 10_000usize;
    let mut data = vec![0u8; leaves * chunk];
    for (i, byte) in data.iter_mut().enumerate() {
        *byte = (i as u8).wrapping_mul(17).wrapping_add(3);
    }
    let t_cpu = ivm::ByteMerkleTree::from_bytes_parallel(&data, chunk).unwrap();
    let root_gpu = ivm::ByteMerkleTree::root_from_bytes_accel(&data, chunk).unwrap();
    assert_eq!(t_cpu.root(), root_gpu);
}
