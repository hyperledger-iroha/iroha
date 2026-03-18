#[cfg(feature = "cuda")]
#[test]
fn test_cuda_vector_add() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }
    if ivm::GpuManager::shared().is_none() {
        eprintln!("Failed to init GpuManager; skipping test");
        return;
    }
    let a = [1.0f32, 2.0, 3.0, 4.0];
    let b = [4.0f32, 3.0, 2.0, 1.0];
    if let Some(res) = ivm::vector_add_f32(&a, &b) {
        assert_eq!(res, vec![5.0, 5.0, 5.0, 5.0]);
    } else {
        panic!("CUDA addition failed");
    }
}

#[cfg(feature = "cuda")]
#[test]
fn test_cuda_vector_int_ops() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }
    if ivm::GpuManager::shared().is_none() {
        eprintln!("Failed to init GpuManager; skipping test");
        return;
    }
    let a = [1u32, 2, 3, 4];
    let b = [4u32, 3, 2, 1];
    if let Some(res) = ivm::vadd32_cuda(&a, &b) {
        assert_eq!(res, vec![5, 5, 5, 5]);
    } else {
        panic!("vadd32_cuda failed");
    }

    let a64 = [1u64, 2u64];
    let b64 = [5u64, 6u64];
    if let Some(res) = ivm::vadd64_cuda(&a64, &b64) {
        assert_eq!(res, vec![6u64, 8u64]);
    } else {
        panic!("vadd64_cuda failed");
    }

    if let Some(res) = ivm::vand_cuda(&a, &b) {
        assert_eq!(res, vec![0u32, 2 & 3, 3 & 2, 4 & 1]);
    }
    if let Some(res) = ivm::vxor_cuda(&a, &b) {
        assert_eq!(res, vec![1 ^ 4, 2 ^ 3, 3 ^ 2, 4 ^ 1]);
    }
    if let Some(res) = ivm::vor_cuda(&a, &b) {
        assert_eq!(res, vec![1 | 4, 2 | 3, 3 | 2, 4 | 1]);
    }
}
