#[cfg(feature = "cuda")]
use ivm::{AccelerationConfig, GpuManager, IVM};

#[cfg(feature = "cuda")]
struct AccelGuard(AccelerationConfig);

#[cfg(feature = "cuda")]
impl Drop for AccelGuard {
    fn drop(&mut self) {
        ivm::set_acceleration_config(self.0);
    }
}

#[cfg(feature = "cuda")]
#[test]
fn disable_cuda_via_env() {
    let baseline = ivm::acceleration_config();
    let _guard = AccelGuard(baseline);
    unsafe {
        std::env::set_var("IVM_DISABLE_CUDA", "1");
    }
    let vm = IVM::new(1_000);
    assert!(
        !vm.uses_cuda(),
        "VM should not enable CUDA when IVM_DISABLE_CUDA is set"
    );
    unsafe {
        std::env::remove_var("IVM_DISABLE_CUDA");
    }
}

#[cfg(feature = "cuda")]
#[test]
fn limit_gpu_count_respects_config() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }
    let baseline = ivm::acceleration_config();
    let _guard = AccelGuard(baseline);
    let mut cfg = baseline;
    cfg.enable_cuda = true;
    cfg.max_gpus = Some(1);
    ivm::set_acceleration_config(cfg);

    let mgr = match GpuManager::shared() {
        Some(m) => m,
        None => {
            eprintln!("Failed to init GpuManager");
            return;
        }
    };
    assert!(mgr.device_count() <= 1);
}

#[cfg(feature = "cuda")]
#[test]
fn disable_cuda_via_config() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }
    let baseline = ivm::acceleration_config();
    let _guard = AccelGuard(baseline);
    let mut cfg = baseline;
    cfg.enable_cuda = false;
    ivm::set_acceleration_config(cfg);

    let result = std::panic::catch_unwind(|| {
        assert!(ivm::GpuManager::init().is_none());
        ivm::GpuManager::shared()
    });

    match result {
        Ok(shared) => assert!(shared.is_none(), "manager should not initialize GPUs"),
        Err(_) => panic!("disable flag should not panic"),
    }
}
