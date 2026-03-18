#[cfg(feature = "cuda")]
#[test]
fn test_cuda_sha256_round() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }
    if ivm::GpuManager::shared().is_none() {
        eprintln!("Failed to init GpuManager; skipping test");
        return;
    }
    let block = [0u8; 64];
    let mut cpu_state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut gpu_state = cpu_state;
    ivm::sha256_compress(&mut cpu_state, &block);
    if ivm::sha256_compress_cuda(&mut gpu_state, &block) {
        assert_eq!(gpu_state, cpu_state);
    } else {
        eprintln!("CUDA SHA256 path unavailable; skipping");
    }
}
