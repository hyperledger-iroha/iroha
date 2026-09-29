# Runtime-loaded CUDA driver bindings

Based on `cust_raw` 0.11.3 from [Rust-CUDA](https://github.com/Rust-GPU/Rust-CUDA).
The crates.io archive SHA-256 is `fbf40d6ade12cb9828bbc844b9875c7b93d25e67a3c9bf61c7aa3ae09e402bf8`.
The upstream package is MIT OR Apache-2.0; this distribution uses Apache-2.0.

`src/cuda.rs` preserves the upstream bindgen declarations and layout tests. Its
353 `extern "C"` declaration blocks are replaced with `cuda_driver_functions!`
invocations. The macro preserves each argument, return type and C calling
convention, resolving and caching symbols from the runtime driver. The upstream
build script and CUDA linker dependency are removed. Layout checks use Rust
`offset_of!` instead of forming references through null pointers. No generated
PTX, toolkit stubs, or CUDA runtime library are included here. The result enum
uses the same declared values through `cuda_result_enum!`; native status returns
are read as C integers and unknown future values become `CUDA_ERROR_UNKNOWN`
instead of forming invalid Rust enum discriminants.

Linux loads the driver soname `libcuda.so.1` through the system loader, with the
WSL system path as a fallback. Windows restricts `nvcuda.dll` to System32. Other
platforms report an unavailable driver. No application environment switch or
CUDA toolkit discovery changes this policy. Missing libraries return
`CUDA_ERROR_NOT_INITIALIZED`; missing functions return `CUDA_ERROR_NOT_SUPPORTED`.
The library and function pointers remain alive for the process lifetime. Error
name/string queries supply static diagnostics for the two loader-owned errors
when the driver is unavailable, so upstream `cust::CudaError` remains printable.

Run `cargo test -p cust_raw` for layout, typed symbol and missing-symbol tests.
Run `cargo test -p cust_raw --test driver_absent -- --ignored` on a driverless host
to prove raw and upstream `cust` calls start without a driver, report printable
errors, and preserve output buffers on failed operations. This verifies
the driver boundary, not IVM kernel correctness or CUDA hardware qualification.
