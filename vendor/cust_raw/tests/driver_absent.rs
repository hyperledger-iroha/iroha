//! Explicit qualification of CUDA-enabled programs on hosts without a driver.

use cust_raw::{CUdevice, CUresult, cuDeviceGet, cuDeviceGetCount, cuDriverGetVersion, cuInit};

// The wrappers preserve the upstream function-pointer ABI, as well as call syntax.
const _: unsafe extern "C" fn(u32) -> CUresult = cuInit;

#[test]
#[ignore = "requires a host with no installed CUDA driver; run explicitly in CPU CI"]
fn public_bindings_start_without_a_driver_and_preserve_outputs() {
    let mut version = -1;
    let mut count = -1;
    let mut device: CUdevice = -1;
    // SAFETY: all output pointers refer to live objects of the declared type.
    unsafe {
        assert_eq!(cuInit(0), CUresult::CUDA_ERROR_NOT_INITIALIZED);
        assert_eq!(
            cuDriverGetVersion(&mut version),
            CUresult::CUDA_ERROR_NOT_INITIALIZED
        );
        assert_eq!(
            cuDeviceGetCount(&mut count),
            CUresult::CUDA_ERROR_NOT_INITIALIZED
        );
        assert_eq!(
            cuDeviceGet(&mut device, 0),
            CUresult::CUDA_ERROR_NOT_INITIALIZED
        );
    }
    assert_eq!((version, count, device), (-1, -1, -1));
    assert_eq!(
        cust::init(cust::CudaFlags::empty()),
        Err(cust::error::CudaError::NotInitialized)
    );
    assert_eq!(
        cust::device::Device::num_devices(),
        Err(cust::error::CudaError::NotInitialized)
    );
    assert!(
        cust::error::CudaError::NotInitialized
            .to_string()
            .contains("library is unavailable")
    );
    assert!(
        cust::error::CudaError::NotSupported
            .to_string()
            .contains("entry point is unavailable")
    );
}
