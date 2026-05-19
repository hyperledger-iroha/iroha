//! CUDA environment and config gate regressions.

#[cfg(feature = "cuda")]
use ivm::{AccelerationConfig, GpuManager, IVM};
#[cfg(feature = "cuda")]
use std::sync::{Mutex, MutexGuard, OnceLock};

#[cfg(feature = "cuda")]
struct AccelGuard {
    _lock: MutexGuard<'static, ()>,
    original: AccelerationConfig,
    original_disable_cuda: Option<String>,
    original_force_selftest_fail: Option<String>,
}

#[cfg(feature = "cuda")]
impl AccelGuard {
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
            original_disable_cuda: std::env::var("IVM_DISABLE_CUDA").ok(),
            original_force_selftest_fail: std::env::var("IVM_FORCE_CUDA_SELFTEST_FAIL").ok(),
        }
    }
}

#[cfg(feature = "cuda")]
fn restore_env_var(name: &str, value: &Option<String>) {
    unsafe {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
}

#[cfg(feature = "cuda")]
impl Drop for AccelGuard {
    fn drop(&mut self) {
        restore_env_var("IVM_DISABLE_CUDA", &self.original_disable_cuda);
        restore_env_var(
            "IVM_FORCE_CUDA_SELFTEST_FAIL",
            &self.original_force_selftest_fail,
        );
        ivm::reset_cuda_backend_for_tests();
        ivm::set_acceleration_config(self.original);
    }
}

#[cfg(feature = "cuda")]
#[test]
fn disable_cuda_via_env() {
    let _guard = AccelGuard::new();
    unsafe {
        std::env::set_var("IVM_DISABLE_CUDA", "1");
    }
    let vm = IVM::new(1_000);
    assert!(
        !vm.uses_cuda(),
        "VM should not enable CUDA when IVM_DISABLE_CUDA is set"
    );
}

#[cfg(feature = "cuda")]
#[test]
fn disable_cuda_env_present_values_fail_closed_for_vm_policy() {
    let _guard = AccelGuard::new();
    for value in ["", "0", "false", "not-a-bool"] {
        unsafe {
            std::env::set_var("IVM_DISABLE_CUDA", value);
        }
        let vm = IVM::new(1_000);
        assert!(
            !vm.uses_cuda(),
            "VM policy should fail closed for present IVM_DISABLE_CUDA={value:?}"
        );
    }
}

#[cfg(feature = "cuda")]
#[test]
fn limit_gpu_count_respects_config() {
    if !ivm::cuda_available() {
        eprintln!("No CUDA GPU available; skipping test");
        return;
    }
    let _guard = AccelGuard::new();
    let mut cfg = ivm::acceleration_config();
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
    let _guard = AccelGuard::new();
    let mut cfg = ivm::acceleration_config();
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

#[cfg(feature = "cuda")]
#[test]
fn config_disable_marks_cuda_unavailable_without_gpu_probe() {
    let _guard = AccelGuard::new();
    ivm::reset_cuda_backend_for_tests();
    unsafe {
        std::env::remove_var("IVM_DISABLE_CUDA");
        std::env::remove_var("IVM_FORCE_CUDA_SELFTEST_FAIL");
    }

    let mut cfg = ivm::acceleration_config();
    cfg.enable_cuda = false;
    ivm::set_acceleration_config(cfg);

    assert!(ivm::cuda_disabled());
    assert_eq!(
        ivm::cuda_last_error_message().as_deref(),
        Some("disabled by configuration")
    );
    assert!(!ivm::cuda_available());
    assert!(
        GpuManager::init().is_none(),
        "disabled CUDA config should reject direct manager init"
    );
    assert!(
        GpuManager::shared().is_none(),
        "disabled CUDA config should reject cached manager init"
    );
}

#[cfg(feature = "cuda")]
#[test]
fn config_reenable_clears_previous_cuda_disable_status() {
    let _guard = AccelGuard::new();
    ivm::reset_cuda_backend_for_tests();

    let mut cfg = ivm::acceleration_config();
    cfg.enable_cuda = false;
    ivm::set_acceleration_config(cfg);
    assert!(ivm::cuda_disabled());

    cfg.enable_cuda = true;
    ivm::set_acceleration_config(cfg);
    assert!(!ivm::cuda_disabled());
    assert_eq!(ivm::cuda_last_error_message(), None);
}

#[cfg(feature = "cuda")]
#[test]
fn disable_cuda_env_reports_backend_disable_before_gpu_probe() {
    let _guard = AccelGuard::new();
    ivm::reset_cuda_backend_for_tests();
    unsafe {
        std::env::set_var("IVM_DISABLE_CUDA", "1");
        std::env::remove_var("IVM_FORCE_CUDA_SELFTEST_FAIL");
    }
    let mut cfg = ivm::acceleration_config();
    cfg.enable_cuda = true;
    ivm::set_acceleration_config(cfg);

    assert!(!ivm::cuda_available());
    assert!(ivm::cuda_disabled());
    assert!(
        ivm::cuda_last_error_message()
            .as_deref()
            .is_some_and(|message| message.contains("IVM_DISABLE_CUDA")),
        "backend should report the CUDA disable environment override"
    );
}

#[cfg(feature = "cuda")]
#[test]
fn forced_cuda_selftest_failure_reports_status_without_gpu_probe() {
    let _guard = AccelGuard::new();
    ivm::reset_cuda_backend_for_tests();
    unsafe {
        std::env::set_var("IVM_DISABLE_CUDA", "0");
        std::env::set_var("IVM_FORCE_CUDA_SELFTEST_FAIL", "1");
    }
    let mut cfg = ivm::acceleration_config();
    cfg.enable_cuda = true;
    ivm::set_acceleration_config(cfg);

    assert!(!ivm::cuda_available());
    assert!(ivm::cuda_disabled());
    assert!(
        ivm::cuda_last_error_message()
            .as_deref()
            .is_some_and(|message| message.contains("IVM_FORCE_CUDA_SELFTEST_FAIL")),
        "forced CUDA self-test failure should report a diagnostic message"
    );
}
