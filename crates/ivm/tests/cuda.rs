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
    assert_eq!(ivm::vadd32(a, b), [5, 5, 5, 5]);

    let a64 = [1u32, 0, 2, 0];
    let b64 = [5u32, 0, 6, 0];
    assert_eq!(ivm::vadd64(a64, b64), [6, 0, 8, 0]);

    assert_eq!(ivm::vand(a, b), [0u32, 2 & 3, 3 & 2, 4 & 1]);
    assert_eq!(ivm::vxor(a, b), [1 ^ 4, 2 ^ 3, 3 ^ 2, 4 ^ 1]);
    assert_eq!(ivm::vor(a, b), [1 | 4, 2 | 3, 3 | 2, 4 | 1]);
}
