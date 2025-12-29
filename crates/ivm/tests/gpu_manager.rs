#[cfg(feature = "cuda")]
#[test]
fn test_gpu_manager_init() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }
    let mgr = match ivm::GpuManager::init() {
        Some(m) => m,
        None => {
            eprintln!("Failed to init GpuManager");
            return;
        }
    };
    assert!(mgr.device_count() >= 1);
    let idx = mgr.gpu_for_task(42);
    assert!(idx < mgr.device_count());
}
