//! Validate that the public CUDA status helpers report disabled on non-CUDA builds.

#![cfg(not(feature = "cuda"))]

#[test]
fn cuda_status_disabled_without_feature() {
    assert!(
        !ivm::cuda_available(),
        "cuda_available must return false without the cuda feature"
    );
    assert!(
        !ivm::cuda_disabled(),
        "cuda_disabled must be false (no backend to disable) without the cuda feature"
    );
    assert!(
        ivm::cuda_last_error_message().is_none(),
        "no error messages should be recorded when CUDA is absent"
    );
}
