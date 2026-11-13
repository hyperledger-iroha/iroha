//! Crypto helper regression tests across acceleration configurations.
use ivm::{
    AccelerationConfig, ByteMerkleTree, SimdChoice, acceleration_runtime_status, poseidon2,
    poseidon6, vector_supported,
};
use std::sync::{Mutex, MutexGuard, OnceLock};
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
fn poseidon_helpers_match_across_acceleration_configs() {
    let guard = AccelConfigGuard::new();
    let inputs2 = (11u64, 17u64);
    let inputs6 = [1u64, 2, 3, 4, 5, 6];
    ivm::set_acceleration_config(AccelerationConfig {
        enable_simd: true,
        enable_cuda: false,
        enable_metal: false,
        ..guard.original
    });
    let cpu_poseidon2 = poseidon2(inputs2.0, inputs2.1);
    let cpu_poseidon6 = poseidon6(inputs6);
    let status_cpu = acceleration_runtime_status();
    assert!(
        !status_cpu.cuda.configured,
        "CUDA should be marked disabled in CPU-only config"
    );
    assert!(
        !status_cpu.metal.configured,
        "Metal should be marked disabled in CPU-only config"
    );
    ivm::set_acceleration_config(AccelerationConfig {
        enable_cuda: true,
        enable_metal: true,
        ..guard.original
    });
    let accel_poseidon2 = poseidon2(inputs2.0, inputs2.1);
    let accel_poseidon6 = poseidon6(inputs6);
    let status_accel = acceleration_runtime_status();
    assert!(
        status_accel.cuda.configured,
        "CUDA should be marked enabled when acceleration is allowed"
    );
    assert!(
        status_accel.metal.configured,
        "Metal should be marked enabled when acceleration is allowed"
    );
    assert_eq!(accel_poseidon2, cpu_poseidon2);
    assert_eq!(accel_poseidon6, cpu_poseidon6);
    #[cfg(feature = "cuda")]
    {
        if ivm::cuda_available() {
            if let Some(cuda_poseidon2) = ivm::poseidon2_cuda(inputs2.0, inputs2.1) {
                assert_eq!(cuda_poseidon2, accel_poseidon2);
            }
            if let Some(cuda_poseidon6) = ivm::poseidon6_cuda(inputs6) {
                assert_eq!(cuda_poseidon6, accel_poseidon6);
            }
        }
    }
}
#[test]
fn simd_disable_forces_scalar_without_affecting_outputs() {
    let guard = AccelConfigGuard::new();
    let inputs = (3u64, 9u64);
    ivm::set_acceleration_config(AccelerationConfig {
        enable_simd: false,
        ..guard.original
    });
    let scalar_status = acceleration_runtime_status();
    assert!(
        !scalar_status.simd.configured,
        "SIMD backend should report disabled when enable_simd = false"
    );
    let scalar_poseidon = poseidon2(inputs.0, inputs.1);
    ivm::set_acceleration_config(AccelerationConfig {
        enable_simd: true,
        ..guard.original
    });
    let simd_status = acceleration_runtime_status();
    assert!(
        simd_status.simd.configured,
        "SIMD backend should report configured when re-enabled"
    );
    let simd_poseidon = poseidon2(inputs.0, inputs.1);
    assert_eq!(
        scalar_poseidon, simd_poseidon,
        "Disabling SIMD must not change cryptographic outputs"
    );
}
#[test]
fn merkle_roots_match_across_simd_modes() {
    let guard = AccelConfigGuard::new();
    let payload: Vec<u8> = (0..(32 * 128))
        .map(|i| (i as u8).wrapping_mul(13).wrapping_add(7))
        .collect();
    ivm::set_acceleration_config(AccelerationConfig {
        enable_simd: true,
        enable_cuda: false,
        enable_metal: false,
        ..guard.original
    });
    let simd_status = acceleration_runtime_status();
    assert!(simd_status.simd.configured);
    assert_eq!(simd_status.simd.available, vector_supported());
    let simd_root = ByteMerkleTree::from_bytes_accel(&payload, 32)
        .unwrap()
        .root();
    ivm::set_acceleration_config(AccelerationConfig {
        enable_simd: false,
        enable_cuda: false,
        enable_metal: false,
        ..guard.original
    });
    let scalar_status = acceleration_runtime_status();
    assert!(!scalar_status.simd.configured);
    assert_eq!(ivm::simd_choice(), SimdChoice::Scalar);
    assert!(!scalar_status.simd.available);
    let scalar_root = ByteMerkleTree::from_bytes_accel(&payload, 32)
        .unwrap()
        .root();
    assert_eq!(scalar_root, simd_root);
}
