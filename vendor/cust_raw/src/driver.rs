//! Process-lifetime ownership of the optional native CUDA driver.

use std::sync::OnceLock;

use libloading::Library;

use crate::CUresult;

struct Driver {
    library: Library,
}

impl Driver {
    fn load() -> Result<Self, CUresult> {
        #[cfg(target_os = "linux")]
        {
            for name in ["libcuda.so.1", "/usr/lib/wsl/lib/libcuda.so.1"] {
                // SAFETY: these are NVIDIA's system driver locations. Driver
                // initialization is explicit through cuInit, as with native linking.
                if let Ok(library) = unsafe { Library::new(name) } {
                    return Ok(Self { library });
                }
            }
        }
        #[cfg(target_os = "windows")]
        {
            use libloading::os::windows::{
                LOAD_LIBRARY_SEARCH_SYSTEM32, Library as WindowsLibrary,
            };

            // SAFETY: restrict lookup to the system driver installation, avoiding
            // current-directory DLL lookup. The owned library stays alive below.
            if let Ok(library) = unsafe {
                WindowsLibrary::load_with_flags("nvcuda.dll", LOAD_LIBRARY_SEARCH_SYSTEM32)
            } {
                return Ok(Self {
                    library: library.into(),
                });
            }
        }
        Err(CUresult::CUDA_ERROR_NOT_INITIALIZED)
    }

    unsafe fn resolve<T: Copy>(&self, name: &[u8]) -> Result<T, CUresult> {
        // SAFETY: callers supply the exact function pointer type for the named
        // symbol. Production symbols borrow a process-lifetime Driver below.
        unsafe { self.library.get::<T>(name) }
            .map(|symbol| *symbol)
            .map_err(|_| CUresult::CUDA_ERROR_NOT_SUPPORTED)
    }
}

pub(super) fn load_or_cached<T>(
    cache: &OnceLock<T>,
    load: impl FnOnce() -> Result<T, CUresult>,
) -> Result<&T, CUresult> {
    if let Some(value) = cache.get() {
        return Ok(value);
    }
    let loaded = load()?;
    // Concurrent first loads may race. The loser drops its extra library and
    // uses the process-lifetime winner before resolving any function pointer.
    let _ = cache.set(loaded);
    Ok(cache.get().expect("successful driver load is cached"))
}

pub(super) unsafe fn resolve<T: Copy>(name: &[u8]) -> Result<T, CUresult> {
    // Never unload the driver while a cached function, context, module or stream
    // may still refer to it. A failed load may retry after manager cooldown so
    // a driver installed after startup becomes usable without restarting.
    static DRIVER: OnceLock<Driver> = OnceLock::new();
    let driver = load_or_cached(&DRIVER, Driver::load)?;
    // SAFETY: binding wrappers supply their generated symbol's exact C signature.
    unsafe { driver.resolve(name) }
}

/// Keep `cust::CudaError` formatting usable when the loader itself fails. CUDA
/// supplies other diagnostics when available; these two statuses originate here.
pub(super) unsafe fn loader_error_text(
    error: CUresult,
    output: *mut *const std::os::raw::c_char,
    load_error: CUresult,
    name: bool,
) -> CUresult {
    if output.is_null() {
        return CUresult::CUDA_ERROR_INVALID_VALUE;
    }
    let text = match (error, name) {
        (CUresult::CUDA_ERROR_NOT_INITIALIZED, true) => c"CUDA_ERROR_NOT_INITIALIZED",
        (CUresult::CUDA_ERROR_NOT_INITIALIZED, false) => c"CUDA driver library is unavailable",
        (CUresult::CUDA_ERROR_NOT_SUPPORTED, true) => c"CUDA_ERROR_NOT_SUPPORTED",
        (CUresult::CUDA_ERROR_NOT_SUPPORTED, false) => c"CUDA driver entry point is unavailable",
        _ => return load_error,
    };
    // SAFETY: checked non-null; the API caller guarantees a writable output. The
    // returned C string is static and survives for the entire process lifetime.
    unsafe { output.write(text.as_ptr()) };
    CUresult::CUDA_SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_load_can_retry_but_success_is_process_lived() {
        let cache = OnceLock::<u32>::new();
        let mut attempts = 0;
        assert_eq!(
            load_or_cached(&cache, || {
                attempts += 1;
                Err(CUresult::CUDA_ERROR_NOT_INITIALIZED)
            }),
            Err(CUresult::CUDA_ERROR_NOT_INITIALIZED)
        );
        assert_eq!(
            load_or_cached(&cache, || {
                attempts += 1;
                Ok(7)
            }),
            Ok(&7)
        );
        assert_eq!(load_or_cached(&cache, || panic!("cached")), Ok(&7));
        assert_eq!(attempts, 2);
    }

    /// Exercise actual dynamic loading and typed dispatch without a CUDA toolkit
    /// or driver, using a temporary library with only two known driver symbols.
    #[test]
    #[cfg(unix)]
    fn typed_symbols_and_missing_symbols_preserve_driver_contract() {
        use std::{fs, process::Command};

        let directory = std::env::temp_dir().join(format!(
            "iroha-cuda-loader-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        fs::create_dir(&directory).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(directory.clone());
        let source = directory.join("driver.c");
        let library = directory.join("driver.so");
        fs::write(
            &source,
            "int cuInit(unsigned int flags) { return flags == 7 ? 0 : (flags == 9 ? 123456 : 1); }\n\
             int cuDriverGetVersion(int *out) { *out = 11040; return 0; }\n",
        )
        .unwrap();
        let output = Command::new("cc")
            .args(["-shared", "-fPIC"])
            .arg(&source)
            .arg("-o")
            .arg(&library)
            .output()
            .expect("host C compiler needed for loader qualification");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let driver = Driver {
            // SAFETY: the library was built above from the fixed test source.
            library: unsafe { Library::new(&library) }.unwrap(),
        };
        type Init = unsafe extern "C" fn(u32) -> i32;
        type Version = unsafe extern "C" fn(*mut i32) -> i32;
        // SAFETY: signatures match the test source, and driver stays alive.
        unsafe {
            let init = driver.resolve::<Init>(b"cuInit\0").unwrap();
            assert_eq!(CUresult::from_driver_code(init(7)), CUresult::CUDA_SUCCESS);
            assert_eq!(
                CUresult::from_driver_code(init(0)),
                CUresult::CUDA_ERROR_INVALID_VALUE
            );
            assert_eq!(
                CUresult::from_driver_code(init(9)),
                CUresult::CUDA_ERROR_UNKNOWN
            );
            let version_cache = OnceLock::<Version>::new();
            for failure in [
                CUresult::CUDA_ERROR_NOT_INITIALIZED,
                CUresult::CUDA_ERROR_NOT_SUPPORTED,
            ] {
                assert!(matches!(
                    load_or_cached(&version_cache, || Err(failure)),
                    Err(error) if error == failure
                ));
                assert!(
                    version_cache.get().is_none(),
                    "failed lookup stays retryable"
                );
            }
            let version = load_or_cached(&version_cache, || {
                driver.resolve::<Version>(b"cuDriverGetVersion\0")
            })
            .unwrap();
            let mut output = -1;
            assert_eq!(
                CUresult::from_driver_code(version(&mut output)),
                CUresult::CUDA_SUCCESS
            );
            assert_eq!(output, 11040);
            let cached =
                load_or_cached(&version_cache, || panic!("successful symbol stays cached"))
                    .unwrap();
            output = -1;
            assert_eq!(cached(&mut output), 0);
            assert_eq!(output, 11040);
            assert!(matches!(
                driver.resolve::<Version>(b"cuDeviceGetCount\0"),
                Err(CUresult::CUDA_ERROR_NOT_SUPPORTED)
            ));
        }
    }

    #[test]
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    fn unsupported_platform_reports_missing_driver() {
        assert!(matches!(
            Driver::load(),
            Err(CUresult::CUDA_ERROR_NOT_INITIALIZED)
        ));
    }

    #[test]
    fn loader_diagnostics_preserve_unknown_outputs_and_reject_null() {
        let mut output = c"unchanged".as_ptr();
        // SAFETY: output is a writable pointer; null is explicitly rejected.
        unsafe {
            assert_eq!(
                loader_error_text(
                    CUresult::CUDA_ERROR_INVALID_VALUE,
                    &mut output,
                    CUresult::CUDA_ERROR_NOT_INITIALIZED,
                    false
                ),
                CUresult::CUDA_ERROR_NOT_INITIALIZED,
            );
            assert_eq!(std::ffi::CStr::from_ptr(output), c"unchanged");
            assert_eq!(
                loader_error_text(
                    CUresult::CUDA_ERROR_NOT_INITIALIZED,
                    std::ptr::null_mut(),
                    CUresult::CUDA_ERROR_NOT_INITIALIZED,
                    false
                ),
                CUresult::CUDA_ERROR_INVALID_VALUE,
            );
            assert_eq!(
                loader_error_text(
                    CUresult::CUDA_ERROR_NOT_SUPPORTED,
                    &mut output,
                    CUresult::CUDA_ERROR_NOT_INITIALIZED,
                    true
                ),
                CUresult::CUDA_SUCCESS,
            );
            assert_eq!(
                std::ffi::CStr::from_ptr(output),
                c"CUDA_ERROR_NOT_SUPPORTED"
            );
        }
    }
}
