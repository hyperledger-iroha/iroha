#![allow(unsafe_op_in_unsafe_fn)]
//! CRC-64/XZ checksums for Norito payloads.
//!
//! CRC-64/XZ uses the ECMA-182 polynomial in reflected (LSB-first) form with the initial
//! register and the final XOR set to all ones. [`crc64_fallback`] is the portable
//! slicing-by-16 reference. [`hardware_crc64`], [`crc64_update`] and [`Crc64Digest`] use the
//! carry-less multiply kernel (PCLMULQDQ on x86_64, PMULL on aarch64) when the `simd-accel`
//! feature is enabled, the CPU supports it and the kernel passes a startup parity check
//! against the reference. Every path returns identical results.
#[allow(unused_imports)]
use core::ffi::{c_char, c_int, c_void};
#[allow(unused_imports)]
use std::ffi::{CStr, CString};
use std::sync::OnceLock;
#[allow(unused_imports)]
use std::sync::atomic::{AtomicUsize, Ordering};
/// CRC-64/XZ polynomial (ECMA-182) in normal (MSB-first) form.
const POLY: u64 = 0x42F0_E1EB_A9EA_3693;
/// [`POLY`] in reflected (LSB-first) form, as processed by the table and folding kernels.
const POLY_REFLECTED: u64 = POLY.reverse_bits();
/// Register value before the first byte; the finished checksum is also XORed with it.
const INIT: u64 = !0;
/// Raw register update: consumes `bytes` starting from an unfinished register `state`.
type UpdateFn = fn(u64, &[u8]) -> u64;
/// Portable CRC-64/XZ over the slicing-by-16 tables.
///
/// This is the canonical reference: accelerated kernels are selected only when they match it.
pub fn crc64_fallback(data: &[u8]) -> u64 {
    !update_table(INIT, data)
}
/// Compute CRC-64/XZ with the fastest available implementation.
///
/// - With `metal-crc64`/`cuda-crc64`, payloads above the GPU cutoff try the GPU helper first,
///   and sampled results are re-checked against [`crc64_fallback`].
/// - With `simd-accel` on a CPU that supports it, uses CLMUL (x86_64) or PMULL (aarch64).
/// - Otherwise uses the portable slicing-by-16 tables.
///
/// The result always equals [`crc64_fallback`].
pub fn hardware_crc64(data: &[u8]) -> u64 {
    if let Some(gpu) = try_gpu_crc64(data) {
        return gpu;
    }
    !selected_update()(INIT, data)
}
/// Update a running CRC-64/XZ value with `data`.
///
/// `crc` is a finished checksum (`0` for empty input), so `crc64_update(crc64_update(0, a), b)`
/// equals the checksum of `a` followed by `b`.
pub fn crc64_update(crc: u64, data: &[u8]) -> u64 {
    !selected_update()(!crc, data)
}
/// Streaming CRC-64/XZ.
///
/// Writing the input in any split yields the checksum of the concatenated bytes.
#[derive(Clone, Copy, Debug)]
pub struct Crc64Digest {
    state: u64,
}
impl Crc64Digest {
    /// Start a digest over no bytes.
    pub const fn new() -> Self {
        Self { state: INIT }
    }
    /// Append `bytes` to the digested input.
    pub fn write(&mut self, bytes: &[u8]) {
        self.state = selected_update()(self.state, bytes);
    }
    /// Return the checksum of every byte written so far.
    pub const fn sum64(&self) -> u64 {
        !self.state
    }
}
impl Default for Crc64Digest {
    fn default() -> Self {
        Self::new()
    }
}
/// The CPU update kernel chosen once per process by [`detect_best_impl`].
fn selected_update() -> UpdateFn {
    static SELECTED: OnceLock<UpdateFn> = OnceLock::new();
    *SELECTED.get_or_init(detect_best_impl)
}
#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
pub(super) const GPU_MIN_DEFAULT: usize = 192 * 1024;
#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
pub(super) static GPU_MIN_LEN: AtomicUsize = AtomicUsize::new(0);
#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
static GPU_VALIDATE_CALLS: AtomicUsize = AtomicUsize::new(0);
#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
const GPU_VALIDATE_INITIAL_CALLS: usize = 4;
#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
const GPU_VALIDATE_INTERVAL: usize = 257;
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
        #[cfg(all(feature = "metal-crc64", target_os = "macos"))]
        GpuBackend::Metal(lib) => unsafe { lib.compute(data) },
        #[cfg(feature = "cuda-crc64")]
        GpuBackend::Cuda(lib) => unsafe { lib.compute(data) },
        GpuBackend::Custom(lib) => unsafe { lib.compute(data) },
        GpuBackend::Unavailable => None,
    }
    .and_then(|gpu| {
        if should_validate_gpu_crc() {
            let expected = crc64_fallback(data);
            if gpu != expected {
                *guard = GpuBackend::Unavailable;
                return None;
            }
        }
        Some(gpu)
    })
}
#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
fn should_validate_gpu_crc() -> bool {
    let call = GPU_VALIDATE_CALLS
        .fetch_add(1, Ordering::Relaxed)
        .saturating_add(1);
    call <= GPU_VALIDATE_INITIAL_CALLS || call.is_multiple_of(GPU_VALIDATE_INTERVAL)
}
#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
#[cfg(test)]
fn reset_gpu_crc_validation_counter() {
    GPU_VALIDATE_CALLS.store(0, Ordering::Relaxed);
}
#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
fn check_gpu_crc_sample(func: GpuFn, sample: &[u8]) -> Result<(), GpuSelfTestFailure> {
    let expected = crc64_fallback(sample);
    let mut actual = 0u64;
    let rc = unsafe { func(sample.as_ptr(), sample.len(), &mut actual) };
    if rc != 0 {
        return Err(GpuSelfTestFailure::HelperError(rc));
    }
    if actual != expected {
        return Err(GpuSelfTestFailure::Mismatch { expected, actual });
    }
    Ok(())
}
#[cfg(not(any(feature = "metal-crc64", feature = "cuda-crc64")))]
fn try_gpu_crc64(_data: &[u8]) -> Option<u64> {
    None
}
#[cfg(all(feature = "simd-accel", target_arch = "x86_64"))]
fn update_pclmul_runtime(state: u64, data: &[u8]) -> u64 {
    // SAFETY: selected only after `has_x86_accel` proves the required target features.
    unsafe { simd::update(state, data) }
}
#[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
fn update_pmull_runtime(state: u64, data: &[u8]) -> u64 {
    // SAFETY: selected only after `has_aarch64_accel` proves the required target features.
    unsafe { simd::update(state, data) }
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
        check_gpu_crc_sample(func, sample)?;
    }
    for &len in &[
        16 * 1024 - 1,
        16 * 1024,
        16 * 1024 + 1,
        GPU_MIN_DEFAULT,
        GPU_MIN_DEFAULT + 17,
        512 * 1024 + 31,
    ] {
        let sample = gpu_crc_self_test_payload(len, len as u64 ^ 0x4e4f_5249_544f_4750);
        check_gpu_crc_sample(func, &sample)?;
    }
    Ok(())
}
#[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
fn gpu_crc_self_test_payload(len: usize, mut seed: u64) -> Vec<u8> {
    let mut out = vec![0u8; len];
    for byte in &mut out {
        seed ^= seed << 7;
        seed ^= seed >> 9;
        seed ^= seed << 8;
        *byte = seed as u8;
    }
    out
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
    unsafe extern "system" {
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
    #[cfg(all(feature = "metal-crc64", target_os = "macos"))]
    Metal(GpuLib),
    #[cfg(feature = "cuda-crc64")]
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
            unsafe { load_library_unix(path.as_c_str()) }.and_then(validate_gpu_lib)
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
    unsafe extern "system" {
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
        let handle = unsafe { LoadLibraryExW(wide.as_ptr(), ptr::null_mut(), search_flags) };
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
    let handle = unsafe { dlopen(path.as_ptr(), RTLD_LAZY) };
    if handle.is_null() {
        return None;
    }
    let func = resolve_symbol_unix(handle, GPU_SYMBOLS)?;
    validate_gpu_lib(GpuLib { handle, func })
}
#[cfg(all(any(feature = "metal-crc64", feature = "cuda-crc64"), windows))]
unsafe fn load_library_windows(path: &CStr) -> Option<GpuLib> {
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt, ptr};
    unsafe extern "system" {
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
    let handle = unsafe { LoadLibraryExW(wide.as_ptr(), ptr::null_mut(), search_flags) };
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
        let ptr = unsafe { dlsym(handle, sym.as_ptr() as *const c_char) };
        if !ptr.is_null() {
            return Some(unsafe { std::mem::transmute::<*mut c_void, GpuFn>(ptr) });
        }
    }
    let _ = unsafe { dlclose(handle) };
    None
}
#[cfg(all(any(feature = "metal-crc64", feature = "cuda-crc64"), windows))]
unsafe fn resolve_symbol_windows(handle: *mut c_void, symbols: &[&[u8]]) -> Option<GpuFn> {
    unsafe extern "system" {
        fn GetProcAddress(hModule: *mut c_void, lpProcName: *const u8) -> *mut c_void;
        fn FreeLibrary(hLibModule: *mut c_void) -> i32;
    }
    for sym in symbols {
        let ptr = unsafe { GetProcAddress(handle, sym.as_ptr()) };
        if !ptr.is_null() {
            return Some(unsafe { std::mem::transmute(ptr) });
        }
    }
    let _ = unsafe { FreeLibrary(handle) };
    None
}
#[cfg(all(test, any(feature = "metal-crc64", feature = "cuda-crc64")))]
pub(super) fn reset_gpu_backend_for_tests() {
    if let Some(cache) = GPU_BACKEND.get() {
        *cache.lock().expect("crc64 gpu backend cache poisoned") = GpuBackend::Unavailable;
    }
    GPU_MIN_LEN.store(0, Ordering::Relaxed);
    reset_gpu_crc_validation_counter();
}
#[cfg(feature = "simd-accel")]
fn detect_best_impl() -> UpdateFn {
    #[cfg(target_arch = "x86_64")]
    {
        if has_x86_accel() && crc64_candidate_matches_reference(update_pclmul_runtime) {
            return update_pclmul_runtime;
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if has_aarch64_accel() && crc64_candidate_matches_reference(update_pmull_runtime) {
            return update_pmull_runtime;
        }
    }
    update_table
}
#[cfg(not(feature = "simd-accel"))]
fn detect_best_impl() -> UpdateFn {
    update_table
}
#[cfg(all(
    feature = "simd-accel",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn crc64_candidate_matches_reference(candidate: UpdateFn) -> bool {
    /// Fixed alignment, so the samples deterministically cover kernel heads and tails.
    #[repr(align(16))]
    struct Aligned([u8; 8192]);
    // Stack storage: the one-time selection runs inside the first checksum
    // call, which allocation-free frame writers may be the first to make.
    let mut large = Aligned([0u8; 8192]);
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    for byte in &mut large.0 {
        seed ^= seed << 7;
        seed ^= seed >> 9;
        seed ^= seed << 8;
        *byte = seed as u8;
    }
    let samples = [
        &b""[..],
        &b"123456789"[..],
        &b"norito-crc64-simd-startup-check"[..],
        &large.0[..],
        &large.0[3..],
        &large.0[..large.0.len() - 5],
    ];
    // Streaming digests resume from arbitrary registers, so check a non-initial one too.
    [INIT, 0x0123_4567_89ab_cdef].into_iter().all(|state| {
        samples
            .iter()
            .all(|sample| candidate(state, sample) == update_table(state, sample))
    })
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
fn detect_best_impl_with(features: SimdFeatureToggle) -> UpdateFn {
    if features.pclmul && features.sse2 && features.sse41 {
        update_pclmul_runtime
    } else {
        update_table
    }
}
#[cfg(all(feature = "simd-accel", test, target_arch = "aarch64"))]
fn detect_best_impl_with(features: SimdFeatureToggle) -> UpdateFn {
    if features.pmull && features.neon {
        update_pmull_runtime
    } else {
        update_table
    }
}
#[cfg(all(
    feature = "simd-accel",
    test,
    not(any(target_arch = "x86_64", target_arch = "aarch64"))
))]
fn detect_best_impl_with(_features: SimdFeatureToggle) -> UpdateFn {
    update_table
}
// Carry-less folding constants for the reflected polynomial: `K_n` is `x^n mod P` (checked by
// `folding_constants_are_powers_of_x_mod_p`) and `MU` is the Barrett quotient `x^128 / P`.
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
/// [`POLY_REFLECTED`] as `(POLY_REFLECTED << 1) | 1`, the operand layout of the kernel's
/// Barrett reduction.
#[cfg(feature = "simd-accel")]
const BARRETT_POLY: u64 = (POLY_REFLECTED << 1) | 1;
#[cfg(all(
    feature = "simd-accel",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod simd {
    use super::{
        BARRETT_POLY, K_127, K_191, K_255, K_319, K_383, K_447, K_511, K_575, K_639, K_703, K_767,
        K_831, K_895, K_959, K_1023, K_1087, MU, update_table,
    };
    use core::{
        fmt::Debug,
        mem,
        ops::{BitXor, BitXorAssign},
    };
    pub(super) trait SimdOps: Copy + Debug + BitXor<Output = Self> + BitXorAssign {
        unsafe fn new(high: u64, low: u64) -> Self;
        unsafe fn fold_16(self, coeff: Self) -> Self;
        unsafe fn fold_8(self, coeff: u64) -> Self;
        unsafe fn barrett(self, poly: u64, mu: u64) -> u64;
    }
    #[cfg(target_arch = "x86_64")]
    mod arch {
        use super::*;
        use core::arch::x86_64::*;
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
                let h = Self(_mm_clmulepi64_si128(self.0, coeff.0, 0x11));
                let l = Self(_mm_clmulepi64_si128(self.0, coeff.0, 0x00));
                h ^ l
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
        pub use self::Simd as PlatformSimd;
    }
    #[cfg(target_arch = "aarch64")]
    mod arch {
        use super::*;
        use core::arch::aarch64::*;
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
                let lanes = vreinterpretq_p64_u8(self.0);
                [vgetq_lane_p64(lanes, 0), vgetq_lane_p64(lanes, 1)]
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
                let lanes = vreinterpretq_p64_u8(self.0);
                vgetq_lane_p64(lanes, 1)
            }
            #[inline]
            #[target_feature(enable = "neon")]
            unsafe fn low_64(self) -> u64 {
                let lanes = vreinterpretq_p64_u8(self.0);
                vgetq_lane_p64(lanes, 0)
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
        pub use self::Simd as PlatformSimd;
    }
    #[cfg(target_arch = "x86_64")]
    use arch::PlatformSimd as Simd;
    #[cfg(target_arch = "aarch64")]
    use arch::PlatformSimd as Simd;
    #[inline(always)]
    unsafe fn make_simd(high: u64, low: u64) -> Simd {
        unsafe { <Simd as SimdOps>::new(high, low) }
    }
    /// Raw register update: table head and tail around 128-byte aligned folding blocks.
    ///
    /// # Safety
    /// The CPU must support the features proven by `has_x86_accel` / `has_aarch64_accel`.
    pub(super) unsafe fn update(state: u64, bytes: &[u8]) -> u64 {
        // SAFETY: every bit pattern is a valid `Simd` lane.
        let (head, blocks, tail) = unsafe { bytes.align_to::<[Simd; 8]>() };
        let Some((first, rest)) = blocks.split_first() else {
            return update_table(state, bytes);
        };
        let state = update_table(state, head);
        let state = unsafe { update_simd(state, first, rest) };
        update_table(state, tail)
    }
    #[cfg_attr(
        target_arch = "x86_64",
        target_feature(enable = "pclmulqdq", enable = "sse2", enable = "sse4.1")
    )]
    #[cfg_attr(
        target_arch = "aarch64",
        target_feature(enable = "aes", enable = "neon")
    )]
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
            .barrett(BARRETT_POLY, MU)
    }
}
/// Slicing-by-16 tables for [`POLY_REFLECTED`], built at compile time.
///
/// `TABLES[0][b]` is byte `b` shifted through the register; `TABLES[k][b]` is
/// `TABLES[k - 1][b]` followed by one more zero byte.
static TABLES: [[u64; 256]; 16] = build_tables();
const fn build_tables() -> [[u64; 256]; 16] {
    let mut tables = [[0u64; 256]; 16];
    let mut byte = 0;
    while byte < 256 {
        let mut crc = byte as u64;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ POLY_REFLECTED
            };
            bit += 1;
        }
        tables[0][byte] = crc;
        byte += 1;
    }
    let mut k = 1;
    while k < 16 {
        let mut byte = 0;
        while byte < 256 {
            let prev = tables[k - 1][byte];
            tables[k][byte] = (prev >> 8) ^ tables[0][(prev & 0xff) as usize];
            byte += 1;
        }
        k += 1;
    }
    tables
}
/// Raw register update over the portable slicing-by-16 tables.
fn update_table(mut state: u64, bytes: &[u8]) -> u64 {
    let (blocks, rest) = bytes.as_chunks::<16>();
    for block in blocks {
        // State-independent lookups first, so they overlap the previous block's latency.
        let s = state.to_le_bytes();
        state = TABLES[0][usize::from(block[15])]
            ^ TABLES[1][usize::from(block[14])]
            ^ TABLES[2][usize::from(block[13])]
            ^ TABLES[3][usize::from(block[12])]
            ^ TABLES[4][usize::from(block[11])]
            ^ TABLES[5][usize::from(block[10])]
            ^ TABLES[6][usize::from(block[9])]
            ^ TABLES[7][usize::from(block[8])]
            ^ TABLES[8][usize::from(block[7] ^ s[7])]
            ^ TABLES[9][usize::from(block[6] ^ s[6])]
            ^ TABLES[10][usize::from(block[5] ^ s[5])]
            ^ TABLES[11][usize::from(block[4] ^ s[4])]
            ^ TABLES[12][usize::from(block[3] ^ s[3])]
            ^ TABLES[13][usize::from(block[2] ^ s[2])]
            ^ TABLES[14][usize::from(block[1] ^ s[1])]
            ^ TABLES[15][usize::from(block[0] ^ s[0])];
    }
    for &byte in rest {
        state = (state >> 8) ^ TABLES[0][usize::from(byte ^ state as u8)];
    }
    state
}
/// CRC-64/XZ with x86_64 CLMUL when `simd-accel` is enabled and the CPU supports it.
///
/// Uses the portable tables otherwise; the result always equals [`crc64_fallback`].
pub fn crc64_sse42(data: &[u8]) -> u64 {
    #[cfg(all(feature = "simd-accel", target_arch = "x86_64"))]
    {
        if has_x86_accel() {
            // SAFETY: `has_x86_accel` proved the required target features.
            return unsafe { crc64_pclmul(data) };
        }
    }
    crc64_fallback(data)
}
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
/// CRC-64/XZ with aarch64 PMULL when `simd-accel` is enabled and the CPU supports it.
///
/// Uses the portable tables otherwise; the result always equals [`crc64_fallback`].
pub fn crc64_neon(data: &[u8]) -> u64 {
    #[cfg(feature = "simd-accel")]
    {
        if has_aarch64_accel() {
            // SAFETY: `has_aarch64_accel` proved the required target features.
            return unsafe { crc64_pmull(data) };
        }
    }
    crc64_fallback(data)
}
#[cfg(all(feature = "simd-accel", target_arch = "x86_64"))]
#[target_feature(enable = "pclmulqdq,sse2,sse4.1")]
/// CRC-64/XZ using x86_64 CLMUL intrinsics.
///
/// # Safety
/// The CPU must support `pclmulqdq`, `sse2` and `sse4.1`.
pub unsafe fn crc64_pclmul(data: &[u8]) -> u64 {
    !simd::update(INIT, data)
}
#[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
#[target_feature(enable = "neon,aes")]
/// CRC-64/XZ using aarch64 PMULL intrinsics.
///
/// # Safety
/// The CPU must support `neon` and `aes` (PMULL).
pub unsafe fn crc64_pmull(data: &[u8]) -> u64 {
    !simd::update(INIT, data)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
    use std::sync::atomic::Ordering;
    #[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
    use std::{fs, path::PathBuf, process::Command, sync::Mutex};
    #[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
    static GPU_TEST_LOCK: Mutex<()> = Mutex::new(());
    #[cfg(feature = "simd-accel")]
    fn fallback_ptr() -> usize {
        update_table as *const () as usize
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
        assert_eq!(ptr, update_pclmul_runtime as *const () as usize);
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
        assert_eq!(ptr, update_pmull_runtime as *const () as usize);
    }
    #[cfg(feature = "simd-accel")]
    #[test]
    fn select_impl_falls_back_without_features() {
        let ptr = detect_best_impl_with(SimdFeatureToggle::none()) as *const () as usize;
        assert_eq!(ptr, fallback_ptr());
    }
    #[test]
    fn selected_impl_matches_fallback() {
        let sample = b"norito-crc64-selfcheck";
        assert_eq!(
            !selected_update()(INIT, sample),
            crc64_fallback(sample),
            "selected CRC64 kernel must match the fallback implementation"
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
    /// CRC-64/XZ reference values; `b"123456789"` is the catalogue check value.
    const KNOWN_VECTORS: &[(&[u8], u64)] = &[
        (b"", 0),
        (b"123456789", 0x995d_c9bb_df19_39fa),
        (b"@", 0x7b1b_8ab9_8fa4_b8f8),
        (b"1\x97", 0xfeb8_f7a1_ae3b_9bd4),
        (b"M\"\xdf", 0xc016_0ce8_dd46_74d3),
        (b"l\xcd\x13\xd7", 0x5c60_a6af_8299_6ea8),
        (&[0; 32], 0xc95a_f861_7cd5_330c),
        (&[255; 32], 0xe95d_ce9e_faa0_9acf),
        (
            b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0A\x0B\x0C\x0D\x0E\x0F\x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1A\x1B\x1C\x1D\x1E\x1F",
            0x7fe5_71a5_8708_4d10,
        ),
        (&[0; 1024], 0xc378_6397_2069_270c),
    ];
    #[test]
    fn every_entry_point_matches_known_vectors() {
        for &(input, expected) in KNOWN_VECTORS {
            assert_eq!(crc64_fallback(input), expected, "fallback {input:x?}");
            assert_eq!(hardware_crc64(input), expected, "hardware {input:x?}");
            assert_eq!(crc64_update(0, input), expected, "update {input:x?}");
            assert_eq!(crc64_sse42(input), expected, "sse42 {input:x?}");
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            assert_eq!(crc64_neon(input), expected, "neon {input:x?}");
            let mut digest = Crc64Digest::new();
            digest.write(input);
            assert_eq!(digest.sum64(), expected, "digest {input:x?}");
        }
    }
    fn pseudo_random(len: usize, mut seed: u64) -> Vec<u8> {
        (0..len)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                (seed >> 24) as u8
            })
            .collect()
    }
    #[test]
    fn streaming_and_chained_updates_match_one_shot() {
        let data = pseudo_random(4099, 0x6e6f_7269_746f_2d63);
        let expected = crc64_fallback(&data);
        for split in [0, 1, 15, 16, 17, 127, 128, 129, 1000, 4098, 4099] {
            let (left, right) = data.split_at(split);
            let mut digest = Crc64Digest::new();
            digest.write(left);
            digest.write(right);
            assert_eq!(digest.sum64(), expected, "digest split {split}");
            let chained = crc64_update(crc64_update(0, left), right);
            assert_eq!(chained, expected, "update split {split}");
        }
        let mut digest = Crc64Digest::default();
        for chunk in data.chunks(7) {
            digest.write(chunk);
        }
        assert_eq!(digest.sum64(), expected);
    }
    #[cfg(all(
        feature = "simd-accel",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[test]
    fn simd_kernel_matches_table_at_every_alignment() {
        #[cfg(target_arch = "x86_64")]
        let supported = has_x86_accel();
        #[cfg(target_arch = "aarch64")]
        let supported = has_aarch64_accel();
        if !supported {
            return;
        }
        let data = pseudo_random(1024 + 16, 0x5eed_c0de_1234_5678);
        for offset in 0..16 {
            for len in 0..=700 {
                let input = &data[offset..offset + len];
                for state in [INIT, 0, 0x0123_4567_89ab_cdef] {
                    // SAFETY: guarded by the runtime feature check above.
                    let simd = unsafe { simd::update(state, input) };
                    assert_eq!(
                        simd,
                        update_table(state, input),
                        "offset {offset} len {len}"
                    );
                }
            }
        }
    }
    #[test]
    fn table_matches_bitwise_definition() {
        assert_eq!(POLY_REFLECTED, 0xC96C_5795_D787_0F42);
        for byte in 0..=255u8 {
            let mut crc = u64::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 == 0 {
                    crc >> 1
                } else {
                    (crc >> 1) ^ POLY_REFLECTED
                };
            }
            assert_eq!(TABLES[0][usize::from(byte)], crc, "byte {byte}");
        }
    }
    #[cfg(feature = "simd-accel")]
    #[test]
    fn folding_constants_are_powers_of_x_mod_p() {
        // Reflected representation: bit 63 holds the coefficient of x^0.
        let x_pow_mod_p = |n: u32| {
            (0..n).fold(1u64 << 63, |r, _| {
                if r & 1 == 0 {
                    r >> 1
                } else {
                    (r >> 1) ^ POLY_REFLECTED
                }
            })
        };
        for (n, k) in [
            (127, K_127),
            (191, K_191),
            (255, K_255),
            (319, K_319),
            (383, K_383),
            (447, K_447),
            (511, K_511),
            (575, K_575),
            (639, K_639),
            (703, K_703),
            (767, K_767),
            (831, K_831),
            (895, K_895),
            (959, K_959),
            (1023, K_1023),
            (1087, K_1087),
        ] {
            assert_eq!(x_pow_mod_p(n), k, "K_{n}");
        }
        assert_eq!(BARRETT_POLY, 0x92d8_af2b_af0e_1e85);
    }
    #[test]
    fn gpu_crc_matches_when_available() {
        let payload = vec![0u8; 192 * 1024];
        let expected = crc64_fallback(&payload);
        if let Some(gpu) = try_gpu_crc64(&payload) {
            assert_eq!(gpu, expected);
        }
    }
    #[cfg(feature = "cuda-crc64")]
    #[test]
    fn cuda_crc64_loads_required_helper_when_requested() {
        if std::env::var_os("NORITO_CRC64_CUDA_REQUIRE").is_none() {
            return;
        }
        let _guard = GPU_TEST_LOCK.lock().expect("crc64 gpu test lock poisoned");
        reset_gpu_backend_for_tests();
        let previous_min_len = GPU_MIN_LEN.swap(1, Ordering::Relaxed);
        let payload = b"required norito cuda crc64 helper";
        let gpu = try_gpu_crc64(payload).expect(
            "NORITO_CRC64_CUDA_REQUIRE requires the CUDA CRC64 helper to load and pass self-test",
        );
        assert_eq!(gpu, crc64_fallback(payload));
        GPU_MIN_LEN.store(previous_min_len, Ordering::Relaxed);
        reset_gpu_backend_for_tests();
    }
    #[cfg(any(feature = "metal-crc64", feature = "cuda-crc64"))]
    #[test]
    fn gpu_min_len_defaults_and_env_override() {
        let _guard = GPU_TEST_LOCK.lock().expect("crc64 gpu test lock poisoned");
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
    #[test]
    fn gpu_crc_validation_schedule_checks_initial_and_interval_calls() {
        let _guard = GPU_TEST_LOCK.lock().expect("crc64 gpu test lock poisoned");
        reset_gpu_crc_validation_counter();
        for _ in 0..GPU_VALIDATE_INITIAL_CALLS {
            assert!(should_validate_gpu_crc());
        }
        for _ in (GPU_VALIDATE_INITIAL_CALLS + 1)..GPU_VALIDATE_INTERVAL {
            assert!(!should_validate_gpu_crc());
        }
        assert!(should_validate_gpu_crc());
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
    unsafe extern "C" fn crc64_large_bad_stub(data: *const u8, len: usize, out: *mut u64) -> i32 {
        if data.is_null() || out.is_null() {
            return -1;
        }
        let bytes = unsafe { std::slice::from_raw_parts(data, len) };
        let mut crc = crc64_fallback(bytes);
        if len >= GPU_MIN_DEFAULT {
            crc ^= 1;
        }
        unsafe { *out = crc };
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
    fn gpu_crc64_self_test_rejects_large_only_mismatch() {
        let err = gpu_crc64_self_test(crc64_large_bad_stub)
            .expect_err("crc64 helper with large-payload mismatch must fail self-test");
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
    #[test]
    fn gpu_crc_sample_mismatch_falls_back_and_disables_backend() {
        let _guard = GPU_TEST_LOCK.lock().expect("crc64 gpu test lock poisoned");
        unsafe {
            std::env::remove_var("NORITO_CRC64_GPU_LIB");
            std::env::remove_var("NORITO_GPU_CRC64_MIN_BYTES");
        }
        reset_gpu_backend_for_tests();
        GPU_MIN_LEN.store(1, Ordering::Relaxed);
        {
            let mut backend = gpu_backend()
                .lock()
                .expect("crc64 gpu backend cache poisoned");
            *backend = GpuBackend::Custom(GpuLib {
                handle: std::ptr::null_mut(),
                func: crc64_bad_stub,
            });
        }
        let payload = b"payload that cannot have a zero crc64";
        reset_gpu_crc_validation_counter();
        assert_eq!(try_gpu_crc64(payload), None);
        assert!(matches!(
            *gpu_backend()
                .lock()
                .expect("crc64 gpu backend cache poisoned"),
            GpuBackend::Unavailable
        ));
        reset_gpu_backend_for_tests();
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
        let _guard = GPU_TEST_LOCK.lock().expect("crc64 gpu test lock poisoned");
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
