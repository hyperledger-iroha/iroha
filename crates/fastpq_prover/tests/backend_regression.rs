//! Final-V1 proof backend admission remains closed until GPU integration exists.
#[cfg(feature = "fastpq-gpu")]
use fastpq_prover::{Error, ExecutionMode, Prover, preflight_native_v1_gpu_backend};

#[cfg(feature = "fastpq-gpu")]
#[test]
fn native_v1_gpu_proofs_are_unavailable_despite_kernel_feature() {
    // Compiled scalar kernels and a discoverable device do not demonstrate a
    // complete six-lane proof dispatch. No hardware availability skip applies.
    assert!(!preflight_native_v1_gpu_backend());
    assert!(matches!(
        Prover::canonical_with_execution_mode(
            "fastpq-state-transition-stark-v1",
            ExecutionMode::Gpu,
        ),
        Err(Error::NativeV1GpuUnavailable)
    ));
    // TODO: require actual CPU/Metal/CUDA dispatch receipts and complete proof
    // byte parity when the native-V1 GPU implementation is added.
}
