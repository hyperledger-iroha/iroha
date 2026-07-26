//! Acceleration config/regression coverage for SIMD policy toggling.

use ivm::{
    AccelerationConfig, SimdChoice, acceleration_config, acceleration_runtime_errors,
    acceleration_runtime_status, clear_forced_simd, forced_simd_test_lock, set_acceleration_config,
    set_forced_simd,
};

/// Restores the acceleration config at drop to avoid cross-test interference.
struct ResetConfig(AccelerationConfig);

impl Drop for ResetConfig {
    fn drop(&mut self) {
        set_acceleration_config(self.0);
    }
}

/// Restores the process-wide SIMD override on drop.
struct SimdOverrideGuard(Option<SimdChoice>);

impl Drop for SimdOverrideGuard {
    fn drop(&mut self) {
        set_forced_simd(self.0);
    }
}

#[test]
fn disabling_simd_forces_scalar_and_preserves_outputs() {
    let _lock = forced_simd_test_lock();
    let original_cfg = acceleration_config();
    let _guard = ResetConfig(original_cfg);

    // Clear any existing overrides so policy toggles are authoritative.
    let prev_override = set_forced_simd(None);
    let baseline_choice = ivm::simd_choice();
    let baseline_sum = ivm::vadd32([1, 2, 3, 4], [5, 6, 7, 8]);

    // Disable SIMD via config and assert we force scalar with a clear reason.
    set_acceleration_config(AccelerationConfig {
        enable_simd: false,
        ..original_cfg
    });
    assert_eq!(ivm::simd_choice(), SimdChoice::Scalar);
    let disabled_status = acceleration_runtime_status();
    assert!(!disabled_status.simd.configured);
    assert!(!disabled_status.simd.available);
    assert_eq!(
        acceleration_runtime_errors().simd.as_deref(),
        Some("disabled by config")
    );
    let disabled_sum = ivm::vadd32([1, 2, 3, 4], [5, 6, 7, 8]);
    assert_eq!(disabled_sum, baseline_sum);

    // Re-enable SIMD and restore any prior override.
    set_acceleration_config(AccelerationConfig {
        enable_simd: true,
        ..original_cfg
    });
    match prev_override {
        Some(choice) => {
            set_forced_simd(Some(choice));
        }
        None => clear_forced_simd(),
    }
    let restored_status = acceleration_runtime_status();
    assert_eq!(restored_status.simd.configured, original_cfg.enable_simd);
    if baseline_choice != SimdChoice::Scalar {
        assert_eq!(ivm::simd_choice(), baseline_choice);
    }
    assert_eq!(ivm::vadd32([1, 2, 3, 4], [5, 6, 7, 8]), baseline_sum);
}

#[test]
fn forced_scalar_override_reports_error_and_clears() {
    let _lock = forced_simd_test_lock();
    let original_cfg = acceleration_config();
    let _config_guard = ResetConfig(original_cfg);
    let original_override = set_forced_simd(None);
    let _override_guard = SimdOverrideGuard(original_override);

    set_acceleration_config(AccelerationConfig {
        enable_simd: true,
        ..original_cfg
    });

    set_forced_simd(Some(SimdChoice::Scalar));
    let forced_status = acceleration_runtime_status();
    assert!(forced_status.simd.configured);
    assert!(!forced_status.simd.available);
    assert_eq!(
        acceleration_runtime_errors().simd.as_deref(),
        Some("forced scalar override")
    );

    clear_forced_simd();
    let cleared_status = acceleration_runtime_status();
    let cleared_errors = acceleration_runtime_errors();
    if cleared_status.simd.supported {
        assert!(
            cleared_errors.simd.is_none(),
            "simd error should clear once override is removed on supported hardware"
        );
    } else {
        assert_eq!(
            cleared_errors.simd.as_deref(),
            Some("simd unsupported on hardware")
        );
    }
}
