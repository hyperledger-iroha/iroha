#![allow(unsafe_op_in_unsafe_fn)]

#[cfg(all(feature = "simd-accel", target_arch = "x86_64"))]
use core::arch::x86_64::{_mm_clmulepi64_si128, _mm_cvtsi128_si64, _mm_set_epi64x, _mm_srli_si128};
#[allow(unused_imports)]
use core::ffi::{c_char, c_int, c_void};
#[allow(unused_imports)]
use std::ffi::{CStr, CString};
use std::sync::OnceLock;
#[allow(unused_imports)]
use std::sync::atomic::{AtomicUsize, Ordering};

use crc64fast::Digest;

/// CRC64-XZ polynomial (ECMA-182, normal form).
const POLY: u64 = 0x42F0_E1EB_A9EA_3693;

/// Portable CRC64-XZ implementation using the `crc64fast` crate.
/// Serves as the canonical reference and fallback path.
pub fn crc64_fallback(data: &[u8]) -> u64 {
    let mut digest = Digest::new_table();
    digest.write(data);
    digest.sum64()
}

#[inline]
#[allow(dead_code)]
fn crc64_runtime_detect(data: &[u8]) -> u64 {
    // Use crc64fast's own runtime-selected implementation (including PMULL/CLMUL
    // when available) to ensure the checksum matches the reference path.
    let mut digest = Digest::new();
    digest.write(data);
    digest.sum64()
}

/// Select the fastest available CRC64 implementation for the current target
/// while preserving identical results to the fallback.
///
/// - When compiled with the `simd-accel` feature and supported by the CPU,
///   uses CLMUL (x86_64) or PMULL (aarch64) accelerated paths.
/// - Otherwise falls back to the portable slicing-by-8 implementation from
///   `crc64fast`.
pub fn hardware_crc64(data: &[u8]) -> u64 {
    if let Some(gpu) = try_gpu_crc64(data) {
        return gpu;
    }
    static CRC_IMPL: OnceLock<fn(&[u8]) -> u64> = OnceLock::new();
    let f = CRC_IMPL.get_or_init(detect_best_impl);
    f(data)
}

#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
pub(super) const GPU_MIN_DEFAULT: usize = 192 * 1024;
#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
pub(super) static GPU_MIN_LEN: AtomicUsize = AtomicUsize::new(0);

#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
pub(super) fn gpu_min_len() -> usize {
    let cached = GPU_MIN_LEN.load(Ordering::Relaxed);
    if cached != 0 {
        return cached;
    }
    #[cfg(any(test, debug_assertions))]
    let configured = std::env::var("NORITO_GPU_CRC64_MIN_BYTES")
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(GPU_MIN_DEFAULT);
    #[cfg(not(any(test, debug_assertions)))]
    let configured = GPU_MIN_DEFAULT;
    GPU_MIN_LEN.store(configured, Ordering::Relaxed);
    configured
}

#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
const GPU_SYMBOLS: &[&[u8]] = &[
    b"norito_crc64_gpu\0",
    b"norito_crc64_metal\0",
    b"norito_crc64_cuda\0",
];

#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
fn try_gpu_crc64(data: &[u8]) -> Option<u64> {
    if data.len() < gpu_min_len() {
        return None;
    }
    let cache = gpu_backend();
    let mut guard = cache.lock().expect("crc64 gpu backend cache poisoned");
    if matches!(*guard, GpuBackend::Unavailable) {
        *guard = load_gpu_backend();
    }
    match &*guard {
        GpuBackend::Metal(lib) | GpuBackend::Cuda(lib) | GpuBackend::Custom(lib) => unsafe {
            lib.compute(data)
        },
        GpuBackend::Unavailable => None,
    }
}

#[cfg(not(any(feature = "metal-crc64", feature = "cuda-crc64")))]
fn try_gpu_crc64(_data: &[u8]) -> Option<u64> {
    None
}

#[cfg(all(feature = "simd-accel", target_arch = "x86_64"))]
fn crc64_pclmul_runtime(data: &[u8]) -> u64 {
    crc64_runtime_detect(data)
}

#[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
fn crc64_pmull_runtime(data: &[u8]) -> u64 {
    crc64_runtime_detect(data)
}

#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
type GpuFn = unsafe extern "C" fn(*const u8, usize, *mut u64) -> i32;

#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GpuSelfTestFailure {
    HelperError(i32),
    Mismatch { expected: u64, actual: u64 },
}

#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
#[derive(Clone, Copy)]
struct GpuLib {
    handle: *mut c_void,
    func: GpuFn,
}

#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
unsafe impl Send for GpuLib {}
#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
unsafe impl Sync for GpuLib {}

#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
impl GpuLib {
    unsafe fn compute(&self, data: &[u8]) -> Option<u64> {
        let mut out = 0u64;
        let rc = (self.func)(data.as_ptr(), data.len(), &mut out);
        if rc == 0 { Some(out) } else { None }
    }
}

#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
fn gpu_crc64_self_test(func: GpuFn) -> Result<(), GpuSelfTestFailure> {
    const SAMPLE_A: &[u8] = b"norito-crc64-gpu-self-test";
    const SAMPLE_B: &[u8] = &[
        0x00, 0xFF, 0x10, 0x20, 0x7E, 0x33, 0x44, 0x99, 0xAB, 0xCD, 0xEF, 0x01, 0x12, 0x23, 0x34,
        0x45, 0x56,
    ];
    const SAMPLE_C: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const SAMPLES: &[&[u8]] = &[SAMPLE_A, SAMPLE_B, SAMPLE_C];

    for sample in SAMPLES {
        let expected = crc64_fallback(sample);
        let mut actual = 0u64;
        let rc = unsafe { func(sample.as_ptr(), sample.len(), &mut actual) };
        if rc != 0 {
            return Err(GpuSelfTestFailure::HelperError(rc));
        }
        if actual != expected {
            return Err(GpuSelfTestFailure::Mismatch { expected, actual });
        }
    }

    Ok(())
}

#[cfg(all(any(feature = "metal-crc64", feature = "cuda-crc64"), unix))]
unsafe fn close_gpu_library_handle(handle: *mut c_void) {
    unsafe extern "C" {
        fn dlclose(handle: *mut c_void) -> c_int;
    }

    if !handle.is_null() {
        let _ = unsafe { dlclose(handle) };
    }
}

#[cfg(all(any(feature = "metal-crc64", feature = "cuda-crc64"), windows))]
unsafe fn close_gpu_library_handle(handle: *mut c_void) {
    extern "system" {
        fn FreeLibrary(hLibModule: *mut c_void) -> i32;
    }

    if !handle.is_null() {
        let _ = unsafe { FreeLibrary(handle) };
    }
}

#[cfg(all(
    any(feature = "metal-crc64", feature = "cuda-crc64"),
    not(any(unix, windows))
))]
unsafe fn close_gpu_library_handle(_handle: *mut c_void) {}

#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
fn validate_gpu_lib(lib: GpuLib) -> Option<GpuLib> {
    match gpu_crc64_self_test(lib.func) {
        Ok(()) => Some(lib),
        Err(_) => {
            unsafe { close_gpu_library_handle(lib.handle) };
            None
        }
    }
}

#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
#[derive(Clone, Copy)]
enum GpuBackend {
    Unavailable,
    Metal(GpuLib),
    Cuda(GpuLib),
    Custom(GpuLib),
}

#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
static GPU_BACKEND: OnceLock<std::sync::Mutex<GpuBackend>> = OnceLock::new();

#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
fn gpu_backend() -> &'static std::sync::Mutex<GpuBackend> {
    GPU_BACKEND.get_or_init(|| std::sync::Mutex::new(GpuBackend::Unavailable))
}

#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
fn load_gpu_backend() -> GpuBackend {
    if let Some(lib) = load_override_from_env() {
        return GpuBackend::Custom(lib);
    }

    #[cfg(all(feature = "metal-crc64", target_os = "macos"))]
    if let Some(lib) = unsafe { load_metal_crc64() } {
        return GpuBackend::Metal(lib);
    }

    #[cfg(feature = "cuda-crc64")]
    if let Some(lib) = unsafe { load_cuda_crc64() } {
        return GpuBackend::Cuda(lib);
    }

    GpuBackend::Unavailable
}

#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
fn load_override_from_env() -> Option<GpuLib> {
    #[cfg(any(test, debug_assertions))]
    {
        let raw = std::env::var_os("NORITO_CRC64_GPU_LIB")?;

        #[cfg(unix)]
        {
            use std::{ffi::CString, os::unix::ffi::OsStrExt};

            let bytes = raw.as_os_str().as_bytes();
            if bytes.is_empty() || bytes.contains(&0) {
                return None;
            }
            let path = CString::new(bytes).ok()?;
            return unsafe { load_library_unix(path.as_c_str()) }.and_then(validate_gpu_lib);
        }

        #[cfg(windows)]
        {
            let path = std::ffi::CString::new(raw.to_str()?).ok()?;
            return unsafe { load_library_windows(path.as_c_str()) }.and_then(validate_gpu_lib);
        }

        #[cfg(not(any(unix, windows)))]
        {
            let _ = raw;
            None
        }
    }
    #[cfg(not(any(test, debug_assertions)))]
    {
        None
    }
}

#[cfg(all(
    any(feature = "metal-crc64", feature = "cuda-crc64"),
    target_os = "macos"
))]
unsafe fn load_metal_crc64() -> Option<GpuLib> {
    const RTLD_LAZY: c_int = 1;
    unsafe extern "C" {
        fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
        fn dlclose(handle: *mut c_void) -> c_int;
        fn objc_autoreleasePoolPush() -> *mut c_void;
        fn objc_autoreleasePoolPop(pool: *mut c_void);
    }
    #[link(name = "Metal", kind = "framework")]
    unsafe extern "C" {
        fn MTLCreateSystemDefaultDevice() -> *mut c_void;
    }

    let pool = objc_autoreleasePoolPush();
    let avail = !MTLCreateSystemDefaultDevice().is_null();
    objc_autoreleasePoolPop(pool);
    if !avail {
        return None;
    }

    use std::{env, ffi::CString, os::unix::ffi::OsStrExt, path::PathBuf};

    const NAMES: &[&str] = &["libnorito_crc64_metal.dylib", "libjsonstage1_metal.dylib"];
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = env::current_exe()
        && let Some(dir) = exe.parent()
    {
        for name in NAMES {
            candidates.push(dir.join(name));
            candidates.push(dir.join("../lib").join(name));
        }
    }

    for path in candidates {
        let bytes = path.as_os_str().as_bytes();
        if bytes.contains(&0) {
            continue;
        }
        let Ok(cpath) = CString::new(bytes) else {
            continue;
        };
        let handle = dlopen(cpath.as_ptr(), RTLD_LAZY);
        if handle.is_null() {
            continue;
        }
        if let Some(func) = resolve_symbol_unix(handle, GPU_SYMBOLS) {
            return validate_gpu_lib(GpuLib { handle, func });
        }
    }
    None
}

#[cfg(all(any(feature = "metal-crc64", feature = "cuda-crc64"), unix))]
unsafe fn load_cuda_crc64() -> Option<GpuLib> {
    const RTLD_LAZY: c_int = 1;
    unsafe extern "C" {
        fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    }

    use std::{env, ffi::CString, os::unix::ffi::OsStrExt, path::PathBuf};

    #[cfg(target_os = "macos")]
    const NAMES: &[&str] = &[
        "libnorito_crc64_cuda.dylib",
        "libnorito_crc64_cuda.so",
        "libjsonstage1_cuda.dylib",
        "libjsonstage1_cuda.so",
    ];
    #[cfg(not(target_os = "macos"))]
    const NAMES: &[&str] = &["libnorito_crc64_cuda.so", "libjsonstage1_cuda.so"];

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = env::current_exe()
        && let Some(dir) = exe.parent()
    {
        for name in NAMES {
            candidates.push(dir.join(name));
            candidates.push(dir.join("../lib").join(name));
        }
    }

    for path in candidates {
        let bytes = path.as_os_str().as_bytes();
        if bytes.contains(&0) {
            continue;
        }
        let Ok(cpath) = CString::new(bytes) else {
            continue;
        };
        let handle = dlopen(cpath.as_ptr(), RTLD_LAZY);
        if handle.is_null() {
            continue;
        }
        if let Some(func) = resolve_symbol_unix(handle, GPU_SYMBOLS) {
            return validate_gpu_lib(GpuLib { handle, func });
        }
    }
    None
}

#[cfg(all(any(feature = "metal-crc64", feature = "cuda-crc64"), windows))]
unsafe fn load_cuda_crc64() -> Option<GpuLib> {
    use std::{env, os::windows::ffi::OsStrExt, path::PathBuf, ptr};

    extern "system" {
        fn SetDefaultDllDirectories(directory_flags: u32) -> i32;
        fn LoadLibraryExW(
            lp_lib_file_name: *const u16,
            h_file: *mut c_void,
            dw_flags: u32,
        ) -> *mut c_void;
    }

    const LOAD_LIBRARY_SEARCH_DEFAULT_DIRS: u32 = 0x0000_1000;
    const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 0x0000_0800;
    const LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR: u32 = 0x0000_0100;

    static DLL_DIRECTORY_SETUP: OnceLock<bool> = OnceLock::new();
    if !*DLL_DIRECTORY_SETUP.get_or_init(|| unsafe {
        let flags = LOAD_LIBRARY_SEARCH_DEFAULT_DIRS | LOAD_LIBRARY_SEARCH_SYSTEM32;
        SetDefaultDllDirectories(flags) != 0
    }) {
        return None;
    }

    const NAMES: &[&str] = &["norito_crc64_cuda.dll", "jsonstage1_cuda.dll"];
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = env::current_exe()
        && let Some(dir) = exe.parent()
    {
        for name in NAMES {
            candidates.push(dir.join(name));
            candidates.push(dir.join("../lib").join(name));
        }
    }

    for path in candidates {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let search_flags = LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32;
        let handle = LoadLibraryExW(wide.as_ptr(), ptr::null_mut(), search_flags);
        if handle.is_null() {
            continue;
        }
        if let Some(func) = resolve_symbol_windows(handle, GPU_SYMBOLS) {
            return validate_gpu_lib(GpuLib { handle, func });
        }
    }
    None
}

#[cfg(all(any(feature = "metal-crc64", feature = "cuda-crc64"), unix))]
unsafe fn load_library_unix(path: &CStr) -> Option<GpuLib> {
    const RTLD_LAZY: c_int = 1;
    unsafe extern "C" {
        fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    }
    let handle = dlopen(path.as_ptr(), RTLD_LAZY);
    if handle.is_null() {
        return None;
    }
    let func = resolve_symbol_unix(handle, GPU_SYMBOLS)?;
    validate_gpu_lib(GpuLib { handle, func })
}

#[cfg(all(any(feature = "metal-crc64", feature = "cuda-crc64"), windows))]
unsafe fn load_library_windows(path: &CStr) -> Option<GpuLib> {
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt, ptr};

    extern "system" {
        fn SetDefaultDllDirectories(directory_flags: u32) -> i32;
        fn LoadLibraryExW(
            lp_lib_file_name: *const u16,
            h_file: *mut c_void,
            dw_flags: u32,
        ) -> *mut c_void;
    }

    const LOAD_LIBRARY_SEARCH_DEFAULT_DIRS: u32 = 0x0000_1000;
    const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 0x0000_0800;
    const LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR: u32 = 0x0000_0100;

    static DLL_DIRECTORY_SETUP: OnceLock<bool> = OnceLock::new();
    if !*DLL_DIRECTORY_SETUP.get_or_init(|| unsafe {
        let flags = LOAD_LIBRARY_SEARCH_DEFAULT_DIRS | LOAD_LIBRARY_SEARCH_SYSTEM32;
        SetDefaultDllDirectories(flags) != 0
    }) {
        return None;
    }

    let path_str = path.to_str().ok()?;
    let wide: Vec<u16> = OsStr::new(path_str).encode_wide().chain(Some(0)).collect();
    let search_flags = LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32;
    let handle = LoadLibraryExW(wide.as_ptr(), ptr::null_mut(), search_flags);
    if handle.is_null() {
        return None;
    }
    let func = resolve_symbol_windows(handle, GPU_SYMBOLS)?;
    validate_gpu_lib(GpuLib { handle, func })
}

#[cfg(all(any(feature = "metal-crc64", feature = "cuda-crc64"), unix))]
unsafe fn resolve_symbol_unix(handle: *mut c_void, symbols: &[&[u8]]) -> Option<GpuFn> {
    unsafe extern "C" {
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlclose(handle: *mut c_void) -> c_int;
    }
    for sym in symbols {
        let ptr = dlsym(handle, sym.as_ptr() as *const c_char);
        if !ptr.is_null() {
            return Some(std::mem::transmute(ptr));
        }
    }
    let _ = dlclose(handle);
    None
}

#[cfg(all(any(feature = "metal-crc64", feature = "cuda-crc64"), windows))]
unsafe fn resolve_symbol_windows(handle: *mut c_void, symbols: &[&[u8]]) -> Option<GpuFn> {
    extern "system" {
        fn GetProcAddress(hModule: *mut c_void, lpProcName: *const u8) -> *mut c_void;
        fn FreeLibrary(hLibModule: *mut c_void) -> i32;
    }
    for sym in symbols {
        let ptr = GetProcAddress(handle, sym.as_ptr());
        if !ptr.is_null() {
            return Some(std::mem::transmute(ptr));
        }
    }
    let _ = FreeLibrary(handle);
    None
}

#[cfg(all(test, any(feature = "metal-crc64", feature = "cuda-crc64")))]
pub(super) fn reset_gpu_backend_for_tests() {
    if let Some(cache) = GPU_BACKEND.get() {
        *cache.lock().expect("crc64 gpu backend cache poisoned") = GpuBackend::Unavailable;
    }
    GPU_MIN_LEN.store(0, Ordering::Relaxed);
}

#[cfg(feature = "simd-accel")]
fn detect_best_impl() -> fn(&[u8]) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        if has_x86_accel() {
            return crc64_pclmul_runtime;
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if has_aarch64_accel() {
            return crc64_pmull_runtime;
        }
    }

    crc64_fallback
}

#[cfg(not(feature = "simd-accel"))]
fn detect_best_impl() -> fn(&[u8]) -> u64 {
    crc64_fallback
}

// Metal/CUDA accelerators are loaded dynamically from optional helper
// libraries (explicit `NORITO_CRC64_GPU_LIB` override or the default
// `libnorito_crc64_{metal,cuda}`/`jsonstage1_cuda` shims). When unavailable we
// fall back to the SIMD or portable CPU implementations above.

#[cfg(all(feature = "simd-accel", target_arch = "x86_64"))]
fn has_x86_accel() -> bool {
    std::is_x86_feature_detected!("pclmulqdq")
        && std::is_x86_feature_detected!("sse2")
        && std::is_x86_feature_detected!("sse4.1")
}

#[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
fn has_aarch64_accel() -> bool {
    std::arch::is_aarch64_feature_detected!("pmull")
        && std::arch::is_aarch64_feature_detected!("neon")
}

#[cfg(all(feature = "simd-accel", test))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SimdFeatureToggle {
    pub pclmul: bool,
    pub sse2: bool,
    pub sse41: bool,
    pub pmull: bool,
    pub neon: bool,
}

#[cfg(all(feature = "simd-accel", test))]
impl SimdFeatureToggle {
    pub const fn none() -> Self {
        Self {
            pclmul: false,
            sse2: false,
            sse41: false,
            pmull: false,
            neon: false,
        }
    }
}

#[cfg(all(feature = "simd-accel", test, target_arch = "x86_64"))]
fn detect_best_impl_with(features: SimdFeatureToggle) -> fn(&[u8]) -> u64 {
    if features.pclmul && features.sse2 && features.sse41 {
        crc64_pclmul_runtime
    } else {
        crc64_fallback
    }
}

#[cfg(all(feature = "simd-accel", test, target_arch = "aarch64"))]
fn detect_best_impl_with(features: SimdFeatureToggle) -> fn(&[u8]) -> u64 {
    if features.pmull && features.neon {
        crc64_pmull_runtime
    } else {
        crc64_fallback
    }
}

#[cfg(all(
    feature = "simd-accel",
    test,
    not(any(target_arch = "x86_64", target_arch = "aarch64"))
))]
fn detect_best_impl_with(_features: SimdFeatureToggle) -> fn(&[u8]) -> u64 {
    crc64_fallback
}

#[cfg(feature = "simd-accel")]
const K_127: u64 = 0xdabe_95af_c787_5f40;
#[cfg(feature = "simd-accel")]
const K_191: u64 = 0xe05d_d497_ca39_3ae4;
#[cfg(feature = "simd-accel")]
const K_255: u64 = 0x3be6_53a3_0fe1_af51;
#[cfg(feature = "simd-accel")]
const K_319: u64 = 0x6009_5b00_8a9e_fa44;
#[cfg(feature = "simd-accel")]
const K_383: u64 = 0x69a3_5d91_c373_0254;
#[cfg(feature = "simd-accel")]
const K_447: u64 = 0xb5ea_1af9_c013_aca4;
#[cfg(feature = "simd-accel")]
const K_511: u64 = 0x081f_6054_a784_2df4;
#[cfg(feature = "simd-accel")]
const K_575: u64 = 0x6ae3_efbb_9dd4_41f3;
#[cfg(feature = "simd-accel")]
const K_639: u64 = 0x0e31_d519_421a_63a5;
#[cfg(feature = "simd-accel")]
const K_703: u64 = 0x2e30_2032_12ca_c325;
#[cfg(feature = "simd-accel")]
const K_767: u64 = 0xe4ce_2cd5_5fea_0037;
#[cfg(feature = "simd-accel")]
const K_831: u64 = 0x2fe3_fd29_20ce_82ec;
#[cfg(feature = "simd-accel")]
const K_895: u64 = 0x9478_74de_5950_52cb;
#[cfg(feature = "simd-accel")]
const K_959: u64 = 0x9e73_5cb5_9b47_24da;
#[cfg(feature = "simd-accel")]
const K_1023: u64 = 0xd7d8_6b2a_f73d_e740;
#[cfg(feature = "simd-accel")]
const K_1087: u64 = 0x8757_d71d_4fcc_1000;
#[cfg(feature = "simd-accel")]
const MU: u64 = 0x9c3e_466c_1729_63d5;

#[cfg(all(
    feature = "simd-accel",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod simd {
    use core::{
        fmt::Debug,
        mem,
        ops::{BitXor, BitXorAssign},
    };

    use super::{
        K_127, K_191, K_255, K_319, K_383, K_447, K_511, K_575, K_639, K_703, K_767, K_831, K_895,
        K_959, K_1023, K_1087, MU, POLY, crc64_slicing_by_8,
    };

    pub(super) trait SimdOps: Copy + Debug + BitXor<Output = Self> + BitXorAssign {
        unsafe fn new(high: u64, low: u64) -> Self;
        unsafe fn fold_16(self, coeff: Self) -> Self;
        unsafe fn fold_8(self, coeff: u64) -> Self;
        unsafe fn barrett(self, poly: u64, mu: u64) -> u64;
    }

    #[cfg(target_arch = "x86_64")]
    mod arch {
        use core::arch::x86_64::*;

        use super::*;

        #[repr(transparent)]
        #[derive(Copy, Clone, Debug)]
        pub struct Simd(__m128i);

        impl super::SimdOps for Simd {
            #[inline]
            #[target_feature(enable = "sse2")]
            unsafe fn new(high: u64, low: u64) -> Self {
                Self(_mm_set_epi64x(high as i64, low as i64))
            }

            #[inline]
            #[target_feature(enable = "sse2", enable = "pclmulqdq")]
            unsafe fn fold_16(self, coeff: Self) -> Self {
                unsafe {
                    let h = Self(_mm_clmulepi64_si128(self.0, coeff.0, 0x11));
                    let l = Self(_mm_clmulepi64_si128(self.0, coeff.0, 0x00));
                    h ^ l
                }
            }

            #[inline]
            #[target_feature(enable = "sse2", enable = "pclmulqdq")]
            unsafe fn fold_8(self, coeff: u64) -> Self {
                unsafe {
                    let coeff = Self::new(0, coeff);
                    let h = Self(_mm_clmulepi64_si128(self.0, coeff.0, 0x00));
                    let l = Self(_mm_srli_si128(self.0, 8));
                    h ^ l
                }
            }

            #[inline]
            #[target_feature(enable = "sse2", enable = "sse4.1", enable = "pclmulqdq")]
            unsafe fn barrett(self, poly: u64, mu: u64) -> u64 {
                unsafe {
                    let polymu = Self::new(poly, mu);
                    let t1 = _mm_clmulepi64_si128(self.0, polymu.0, 0x00);
                    let h = Self(_mm_slli_si128(t1, 8));
                    let l = Self(_mm_clmulepi64_si128(t1, polymu.0, 0x10));
                    let reduced = h ^ l ^ self;
                    _mm_extract_epi64(reduced.0, 1) as u64
                }
            }
        }

        impl BitXor for Simd {
            type Output = Self;

            fn bitxor(self, other: Self) -> Self {
                Self(unsafe { _mm_xor_si128(self.0, other.0) })
            }
        }

        impl BitXorAssign for Simd {
            fn bitxor_assign(&mut self, other: Self) {
                *self = *self ^ other;
            }
        }

        impl PartialEq for Simd {
            fn eq(&self, other: &Self) -> bool {
                let lhs: u128 = unsafe { mem::transmute::<Self, u128>(*self) };
                let rhs: u128 = unsafe { mem::transmute::<Self, u128>(*other) };
                lhs == rhs
            }
        }

        impl Eq for Simd {}

        pub use Simd as PlatformSimd;
    }

    #[cfg(target_arch = "aarch64")]
    mod arch {
        use core::arch::aarch64::*;

        use super::*;

        #[repr(transparent)]
        #[derive(Copy, Clone, Debug)]
        pub struct Simd(uint8x16_t);

        impl Simd {
            #[inline]
            #[target_feature(enable = "neon")]
            unsafe fn from_mul(a: u64, b: u64) -> Self {
                unsafe {
                    let mul = vmull_p64(a, b);
                    Self(vreinterpretq_u8_p128(mul))
                }
            }

            #[inline]
            #[target_feature(enable = "neon")]
            unsafe fn into_poly64s(self) -> [u64; 2] {
                unsafe {
                    let lanes = vreinterpretq_p64_u8(self.0);
                    [vgetq_lane_p64(lanes, 0), vgetq_lane_p64(lanes, 1)]
                }
            }
        }

        impl super::SimdOps for Simd {
            #[inline]
            #[target_feature(enable = "neon")]
            unsafe fn new(high: u64, low: u64) -> Self {
                Self(vcombine_u8(vcreate_u8(low), vcreate_u8(high)))
            }

            #[inline]
            #[target_feature(enable = "aes", enable = "neon")]
            unsafe fn fold_16(self, coeff: Self) -> Self {
                unsafe {
                    let [x0, x1] = self.into_poly64s();
                    let [c0, c1] = coeff.into_poly64s();
                    let h = Self::from_mul(c0, x0);
                    let l = Self::from_mul(c1, x1);
                    h ^ l
                }
            }

            #[inline]
            #[target_feature(enable = "neon")]
            unsafe fn fold_8(self, coeff: u64) -> Self {
                unsafe {
                    let [x0, x1] = self.into_poly64s();
                    let h = Self::from_mul(coeff, x0);
                    let l = Self::new(0, x1);
                    h ^ l
                }
            }

            #[inline]
            #[target_feature(enable = "neon")]
            unsafe fn barrett(self, poly: u64, mu: u64) -> u64 {
                unsafe {
                    let t1 = Self::from_mul(self.low_64(), mu).low_64();
                    let l = Self::from_mul(t1, poly);
                    let reduced = (self ^ l).high_64();
                    reduced ^ t1
                }
            }
        }

        impl Simd {
            #[inline]
            #[target_feature(enable = "neon")]
            unsafe fn high_64(self) -> u64 {
                unsafe {
                    let lanes = vreinterpretq_p64_u8(self.0);
                    vgetq_lane_p64(lanes, 1)
                }
            }

            #[inline]
            #[target_feature(enable = "neon")]
            unsafe fn low_64(self) -> u64 {
                unsafe {
                    let lanes = vreinterpretq_p64_u8(self.0);
                    vgetq_lane_p64(lanes, 0)
                }
            }
        }

        impl BitXor for Simd {
            type Output = Self;

            fn bitxor(self, other: Self) -> Self {
                unsafe { Self(veorq_u8(self.0, other.0)) }
            }
        }

        impl BitXorAssign for Simd {
            fn bitxor_assign(&mut self, other: Self) {
                *self = *self ^ other;
            }
        }

        impl PartialEq for Simd {
            fn eq(&self, other: &Self) -> bool {
                let lhs: u128 = unsafe { mem::transmute::<Self, u128>(*self) };
                let rhs: u128 = unsafe { mem::transmute::<Self, u128>(*other) };
                lhs == rhs
            }
        }

        impl Eq for Simd {}

        pub use Simd as PlatformSimd;
    }

    #[cfg(target_arch = "x86_64")]
    use arch::PlatformSimd as Simd;
    #[cfg(target_arch = "aarch64")]
    use arch::PlatformSimd as Simd;

    #[inline(always)]
    unsafe fn make_simd(high: u64, low: u64) -> Simd {
        unsafe { <Simd as SimdOps>::new(high, low) }
    }

    #[inline]
    pub(super) unsafe fn crc64_simd(bytes: &[u8]) -> u64 {
        let mut state = !0u64;
        state = update(state, bytes);
        !state
    }

    fn update(mut state: u64, bytes: &[u8]) -> u64 {
        let (head, simd_blocks, tail) = unsafe { bytes.align_to::<[Simd; 8]>() };
        state = update_scalar(state, head);
        if let Some((first, rest)) = simd_blocks.split_first() {
            state = unsafe { update_simd(state, first, rest) };
            update_scalar(state, tail)
        } else {
            update_scalar(state, tail)
        }
    }

    fn update_scalar(state: u64, bytes: &[u8]) -> u64 {
        let crc = !state;
        let updated_crc = crc64_slicing_by_8(crc, bytes);
        !updated_crc
    }

    unsafe fn update_simd(state: u64, first: &[Simd; 8], rest: &[[Simd; 8]]) -> u64 {
        let mut lanes = *first;
        lanes[0] ^= make_simd(0, state);

        let coeff = make_simd(K_1023, K_1087);
        for chunk in rest {
            for (dst, src) in lanes.iter_mut().zip(chunk.iter()) {
                *dst = *src ^ dst.fold_16(coeff);
            }
        }

        let coeffs = [
            make_simd(K_895, K_959),
            make_simd(K_767, K_831),
            make_simd(K_639, K_703),
            make_simd(K_511, K_575),
            make_simd(K_383, K_447),
            make_simd(K_255, K_319),
            make_simd(K_127, K_191),
        ];

        lanes
            .iter()
            .zip(&coeffs)
            .fold(lanes[7], |acc, (lane, coeff)| acc ^ lane.fold_16(*coeff))
            .fold_8(K_127)
            .barrett(POLY, MU)
    }
}

// Note: helper constants and Barrett reduction helpers removed as they were unused.

/// Build slicing-by-8 tables for MSB-first CRC64 (ECMA-182) with polynomial POLY.
fn build_tables() -> [[u64; 256]; 8] {
    let mut t = [[0u64; 256]; 8];
    // T0
    for (b, entry) in t[0].iter_mut().enumerate() {
        let mut crc = (b as u64) << 56;
        for _ in 0..8 {
            if (crc & 0x8000_0000_0000_0000) != 0 {
                crc = (crc << 1) ^ POLY;
            } else {
                crc <<= 1;
            }
        }
        *entry = crc;
    }
    // T1..T7
    let t0 = t[0];
    for i in 1..8 {
        let prev_table = t[i - 1];
        for (b, slot) in t[i].iter_mut().enumerate() {
            let prev = prev_table[b];
            let idx = (prev >> 56) as u8 as usize;
            *slot = (prev << 8) ^ t0[idx];
        }
    }
    t
}

fn tables() -> &'static [[u64; 256]; 8] {
    static T: OnceLock<[[u64; 256]; 8]> = OnceLock::new();
    T.get_or_init(build_tables)
}

#[inline(always)]
fn crc64_slicing_by_8(mut crc: u64, mut data: &[u8]) -> u64 {
    let t = tables();
    while data.len() >= 8 {
        let b0 = data[0] as usize;
        let b1 = data[1] as usize;
        let b2 = data[2] as usize;
        let b3 = data[3] as usize;
        let b4 = data[4] as usize;
        let b5 = data[5] as usize;
        let b6 = data[6] as usize;
        let b7 = data[7] as usize;
        let idx = ((crc >> 56) as u8 as usize) ^ b0;
        crc =
            t[0][idx] ^ t[1][b1] ^ t[2][b2] ^ t[3][b3] ^ t[4][b4] ^ t[5][b5] ^ t[6][b6] ^ t[7][b7];
        data = &data[8..];
    }
    for &b in data {
        let idx = ((crc >> 56) as u8) ^ b;
        crc = t[0][idx as usize] ^ (crc << 8);
    }
    crc
}

/// Update a running CRC64-XZ value with `data`.
#[inline(always)]
pub fn crc64_update(crc: u64, data: &[u8]) -> u64 {
    crc64_slicing_by_8(crc, data)
}

pub fn crc64_sse42(data: &[u8]) -> u64 {
    // Safe default: use portable slicing-by-8.
    // Optional SIMD acceleration is gated and only compiled when enabled.
    #[cfg(all(feature = "simd-accel", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("pclmulqdq")
            && std::is_x86_feature_detected!("sse2")
            && std::is_x86_feature_detected!("sse4.1")
        {
            // SAFETY: compiled only when SIMD accel feature is enabled and feature check passes.
            return unsafe { crc64_pclmul(data) };
        }
    }
    crc64_slicing_by_8(0, data)
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
/// CRC64 implementation using NEON intrinsics with vectorized polynomial reduction.
pub fn crc64_neon(data: &[u8]) -> u64 {
    // Safe default: use portable slicing-by-8.
    // Optional SIMD acceleration is gated and only compiled when enabled.
    #[cfg(feature = "simd-accel")]
    {
        if std::arch::is_aarch64_feature_detected!("pmull")
            && std::arch::is_aarch64_feature_detected!("neon")
        {
            // SAFETY: compiled only when SIMD accel feature is enabled and feature check passes.
            unsafe { return crc64_pmull(data) };
        }
    }
    crc64_slicing_by_8(0, data)
}

#[cfg(all(feature = "simd-accel", target_arch = "x86_64"))]
#[target_feature(enable = "pclmulqdq")]
#[inline]
unsafe fn clmul_u64_x86(a: u64, b: u64) -> (u64, u64) {
    unsafe {
        let va = _mm_set_epi64x(0, a as i64);
        let vb = _mm_set_epi64x(0, b as i64);
        let p = _mm_clmulepi64_si128(va, vb, 0x00);
        let lo = _mm_cvtsi128_si64(p) as u64;
        let hi = _mm_cvtsi128_si64(_mm_srli_si128(p, 8)) as u64;
        (hi, lo)
    }
}

#[cfg(all(feature = "simd-accel", target_arch = "x86_64"))]
#[target_feature(enable = "pclmulqdq")]
#[inline]
unsafe fn reduce128_barrett_x86(_t_hi: u64, _t_lo: u64) -> u64 {
    unsafe {
        // Deprecated in v1; not used by the current path.
        0
    }
}

#[cfg(all(feature = "simd-accel", target_arch = "x86_64"))]
#[target_feature(enable = "pclmulqdq")]
/// CRC64 implementation using x86 CLMUL intrinsics.
///
/// # Safety
/// The caller must ensure the `pclmulqdq` instruction is supported by the CPU.
pub unsafe fn crc64_pclmul(data: &[u8]) -> u64 {
    unsafe { simd::crc64_simd(data) }
}

#[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
#[target_feature(enable = "neon,aes")]
/// CRC64 implementation using aarch64 PMULL intrinsics.
///
/// # Safety
/// The caller must ensure the `pmull` feature is supported by the CPU.
pub unsafe fn crc64_pmull(data: &[u8]) -> u64 {
    unsafe { simd::crc64_simd(data) }
}

#[cfg(test)]
mod tests {
    #[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
    use std::sync::atomic::Ordering;
    #[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
    use std::{fs, path::PathBuf, process::Command};

    use super::*;

    #[cfg(feature = "simd-accel")]
    fn fallback_ptr() -> usize {
        crc64_fallback as *const () as usize
    }

    #[cfg(all(feature = "simd-accel", target_arch = "x86_64"))]
    #[test]
    fn select_impl_prefers_pclmul() {
        let features = SimdFeatureToggle {
            pclmul: true,
            sse2: true,
            sse41: true,
            ..SimdFeatureToggle::none()
        };
        let ptr = detect_best_impl_with(features) as *const () as usize;
        assert_eq!(ptr, crc64_pclmul_runtime as *const () as usize);
    }

    #[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
    #[test]
    fn select_impl_prefers_pmull() {
        let features = SimdFeatureToggle {
            pmull: true,
            neon: true,
            ..SimdFeatureToggle::none()
        };
        let ptr = detect_best_impl_with(features) as *const () as usize;
        assert_eq!(ptr, crc64_pmull_runtime as *const () as usize);
    }

    #[cfg(feature = "simd-accel")]
    #[test]
    fn select_impl_falls_back_without_features() {
        let ptr = detect_best_impl_with(SimdFeatureToggle::none()) as *const () as usize;
        assert_eq!(ptr, fallback_ptr());
    }

    #[test]
    fn runtime_detect_matches_fallback() {
        let sample = b"norito-crc64-selfcheck";
        assert_eq!(
            crc64_runtime_detect(sample),
            crc64_fallback(sample),
            "runtime-selected CRC64 must match the fallback implementation"
        );
    }

    #[cfg(all(feature = "simd-accel", target_arch = "x86_64"))]
    #[test]
    fn crc64_fallback_sanity() {
        let payloads: &[&[u8]] = &[
            &b""[..],
            &b"a"[..],
            &b"abc"[..],
            &b"The quick brown fox jumps over the lazy dog"[..],
            &[0u8; 64][..],
        ];
        for p in payloads.iter().copied() {
            // Fallback must be stable and deterministic
            let a = crc64_fallback(p);
            let b = crc64_fallback(p);
            assert_eq!(a, b, "crc mismatch for {p:?}");
        }
    }

    #[test]
    fn hardware_crc_matches_fallback() {
        let inputs: &[&[u8]] = &[
            &b""[..],
            &b"123456789"[..],
            &[0xFFu8; 128][..],
            &b"deterministic hardware crc"[..],
        ];

        for data in inputs.iter().copied() {
            let fallback = crc64_fallback(data);
            let hw = hardware_crc64(data);
            assert_eq!(fallback, hw, "hardware crc mismatch for {data:?}");
        }
    }

    #[test]
    fn gpu_crc_matches_when_available() {
        let payload = vec![0u8; 192 * 1024];
        let expected = crc64_fallback(&payload);
        if let Some(gpu) = try_gpu_crc64(&payload) {
            assert_eq!(gpu, expected);
        }
    }

    #[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
    #[test]
    fn gpu_min_len_defaults_and_env_override() {
        unsafe { std::env::remove_var("NORITO_GPU_CRC64_MIN_BYTES") };
        GPU_MIN_LEN.store(0, Ordering::Relaxed);
        assert_eq!(gpu_min_len(), GPU_MIN_DEFAULT);

        unsafe {
            std::env::set_var("NORITO_GPU_CRC64_MIN_BYTES", "1024");
        }
        GPU_MIN_LEN.store(0, Ordering::Relaxed);
        assert_eq!(gpu_min_len(), 1024);
        unsafe {
            std::env::remove_var("NORITO_GPU_CRC64_MIN_BYTES");
        }
    }

    #[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
    unsafe extern "C" fn crc64_good_stub(data: *const u8, len: usize, out: *mut u64) -> i32 {
        if data.is_null() || out.is_null() {
            return -1;
        }
        let bytes = unsafe { std::slice::from_raw_parts(data, len) };
        let crc = crc64_fallback(bytes);
        unsafe { *out = crc };
        0
    }

    #[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
    unsafe extern "C" fn crc64_bad_stub(_data: *const u8, _len: usize, out: *mut u64) -> i32 {
        if out.is_null() {
            return -1;
        }
        unsafe { *out = 0 };
        0
    }

    #[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
    unsafe extern "C" fn crc64_error_stub(_data: *const u8, _len: usize, _out: *mut u64) -> i32 {
        7
    }

    #[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
    #[test]
    fn gpu_crc64_self_test_accepts_matching_helper() {
        assert_eq!(gpu_crc64_self_test(crc64_good_stub), Ok(()));
    }

    #[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
    #[test]
    fn gpu_crc64_self_test_rejects_mismatched_helper_output() {
        let err = gpu_crc64_self_test(crc64_bad_stub)
            .expect_err("crc64 helper with mismatched output must fail self-test");
        assert!(matches!(err, GpuSelfTestFailure::Mismatch { .. }));
    }

    #[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
    #[test]
    fn gpu_crc64_self_test_rejects_helper_errors() {
        let err = gpu_crc64_self_test(crc64_error_stub)
            .expect_err("crc64 helper with non-zero return code must fail self-test");
        assert_eq!(err, GpuSelfTestFailure::HelperError(7));
    }

    #[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
    fn build_crc64_stub(dir: &PathBuf) -> PathBuf {
        const SRC: &str = r#"
        const POLY_REV: u64 = 0xC96C_5795_D787_0F42;

        #[no_mangle]
        pub unsafe extern "C" fn norito_crc64_gpu(
            data: *const u8,
            len: usize,
            out: *mut u64,
        ) -> i32 {
            if data.is_null() || out.is_null() {
                return -1;
            }
            let bytes = std::slice::from_raw_parts(data, len);
            let mut crc = !0u64;
            for &b in bytes {
                crc ^= b as u64;
                for _ in 0..8 {
                    if (crc & 1) != 0 {
                        crc = (crc >> 1) ^ POLY_REV;
                    } else {
                        crc >>= 1;
                    }
                }
            }
            unsafe { *out = !crc };
            0
        }
        "#;

        fs::create_dir_all(dir).expect("failed to create stub dir");
        let src_path = dir.join("crc64_stub.rs");
        fs::write(&src_path, SRC).expect("failed to write stub source");

        let lib_name = format!(
            "{}crc64_stub{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        );
        let lib_path = dir.join(lib_name);
        let status = Command::new("rustc")
            .args([
                "--crate-type",
                "cdylib",
                src_path.to_str().expect("stub path utf-8"),
                "-o",
                lib_path.to_str().expect("lib path utf-8"),
            ])
            .status()
            .expect("failed to invoke rustc for crc64 stub");
        assert!(status.success(), "stub rustc failed: {status}");
        lib_path
    }

    #[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
    #[test]
    fn gpu_crc64_can_load_env_stub() {
        let tmp_dir =
            std::env::temp_dir().join(format!("norito_crc64_stub_{}", std::process::id()));
        let lib_path = build_crc64_stub(&tmp_dir);
        unsafe {
            std::env::set_var("NORITO_CRC64_GPU_LIB", lib_path);
            std::env::set_var("NORITO_GPU_CRC64_MIN_BYTES", "1024");
        }
        GPU_MIN_LEN.store(0, Ordering::Relaxed);
        reset_gpu_backend_for_tests();

        let payload = vec![1u8; 12 * 1024];
        let expected = crc64_fallback(&payload);
        let gpu = try_gpu_crc64(&payload).expect("stub gpu path should load");
        assert_eq!(gpu, expected);

        unsafe {
            std::env::remove_var("NORITO_CRC64_GPU_LIB");
            std::env::remove_var("NORITO_GPU_CRC64_MIN_BYTES");
        }
        reset_gpu_backend_for_tests();
        let _ = fs::remove_dir_all(tmp_dir);
    }
}
