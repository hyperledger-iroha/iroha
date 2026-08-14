//! Source-level CUDA contracts that must hold even on hosts without a CUDA toolkit.
#[test]
fn poseidon_status_is_initialized_by_the_host_only() {
    let kernel = include_str!("../cuda/poseidon.cu");
    let host = include_str!("../src/cuda.rs");
    assert!(
        !kernel.contains("status_out[0].code = STATUS_OK"),
        "a device-side status reset can erase an error reported by another CUDA block"
    );
    assert!(
        !kernel.contains("status_out[0].detail = 0"),
        "a device-side detail reset can race with the first error reporter"
    );
    assert_eq!(
        kernel
            .matches("The host initializes status_out before launch.")
            .count(),
        2,
        "both Poseidon entry points must document the host-owned status lifecycle"
    );
    assert!(
        host.contains("let mut status = [KernelStatus::default(); 1];"),
        "the host must initialize the shared Poseidon status before upload"
    );
    assert!(
        host.contains("cuda_buffer_from_slice_async(&status, stream, \"poseidon status upload\")"),
        "the initialized status must be uploaded before the Poseidon kernel launch"
    );
}
