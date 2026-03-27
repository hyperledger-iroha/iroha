//! GPU-accelerated zstd compression utilities.
//!
//! The functions in this module detect the available GPU backend at runtime and
//! currently fall back to the CPU implementation if no supported accelerator is
//! present. They are structured so that true GPU offloading can be added later
//! without changing the public API.

#[cfg(unix)]
use std::ffi::{c_char, c_int};
#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
use std::process::{Command, Stdio};
#[cfg(windows)]
use std::{ffi::OsStr, os::windows::ffi::OsStrExt, ptr};
use std::{
    ffi::c_void,
    fmt,
    io::{self, Read},
    sync::OnceLock,
};
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[link(name = "objc")]
unsafe extern "C" {
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}

#[cfg(unix)]
unsafe extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
}
#[cfg(windows)]
extern "system" {
    fn SetDefaultDllDirectories(directory_flags: u32) -> i32;
    fn LoadLibraryExW(
        lp_lib_file_name: *const u16,
        h_file: *mut c_void,
        dw_flags: u32,
    ) -> *mut c_void;
    fn GetProcAddress(h_module: *mut c_void, lp_proc_name: *const u8) -> *mut c_void;
    fn FreeLibrary(h_lib_module: *mut c_void) -> i32;
}
#[cfg(windows)]
const LOAD_LIBRARY_SEARCH_DEFAULT_DIRS: u32 = 0x0000_1000;
#[cfg(windows)]
const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 0x0000_0800;

#[cfg(unix)]
const RTLD_LAZY: c_int = 1;
const RC_GPU_UNAVAILABLE: i32 = 3;

type CompressFn = unsafe extern "C" fn(
    src: *const u8,
    src_len: usize,
    level: i32,
    dst: *mut u8,
    dst_len: *mut usize,
) -> i32;
type DecompressFn =
    unsafe extern "C" fn(src: *const u8, src_len: usize, dst: *mut u8, dst_len: *mut usize) -> i32;

#[derive(Debug)]
enum SelfTestFailure {
    GpuCompress { rc: i32, len: usize, cap: usize },
    CpuDecodeGpu(io::Error),
    CpuDecodeMismatch,
    CpuEncode(io::Error),
    GpuDecodeCpu { rc: i32, len: usize, cap: usize },
    GpuDecodeMismatch,
    GpuRoundtripCompress { rc: i32, len: usize, cap: usize },
    GpuRoundtripDecompress { rc: i32, len: usize, cap: usize },
    GpuRoundtripMismatch,
}

impl SelfTestFailure {
    fn rc_label(rc: i32) -> &'static str {
        match rc {
            0 => "ok",
            1 => "invalid",
            2 => "no_space",
            3 => "gpu_unavailable",
            4 => "zstd_error",
            _ => "unknown",
        }
    }
}

impl fmt::Display for SelfTestFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GpuCompress { rc, len, cap } => write!(
                f,
                "gpu_compress rc={} ({}) output_len={} cap={}",
                rc,
                Self::rc_label(*rc),
                len,
                cap
            ),
            Self::CpuDecodeGpu(err) => {
                write!(f, "cpu_decode_gpu_output error={}", err)
            }
            Self::CpuDecodeMismatch => write!(f, "cpu_decode_gpu_output mismatch"),
            Self::CpuEncode(err) => write!(f, "cpu_encode_sample error={}", err),
            Self::GpuDecodeCpu { rc, len, cap } => write!(
                f,
                "gpu_decode_cpu_output rc={} ({}) output_len={} cap={}",
                rc,
                Self::rc_label(*rc),
                len,
                cap
            ),
            Self::GpuDecodeMismatch => write!(f, "gpu_decode_cpu_output mismatch"),
            Self::GpuRoundtripCompress { rc, len, cap } => write!(
                f,
                "gpu_roundtrip_compress rc={} ({}) output_len={} cap={}",
                rc,
                Self::rc_label(*rc),
                len,
                cap
            ),
            Self::GpuRoundtripDecompress { rc, len, cap } => write!(
                f,
                "gpu_roundtrip_decompress rc={} ({}) output_len={} cap={}",
                rc,
                Self::rc_label(*rc),
                len,
                cap
            ),
            Self::GpuRoundtripMismatch => write!(f, "gpu_roundtrip output mismatch"),
        }
    }
}

enum Backend {
    Cpu,
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    Metal {
        compress: CompressFn,
        decompress: DecompressFn,
    },
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    Cuda {
        compress: CompressFn,
        decompress: DecompressFn,
    },
}

static BACKEND: OnceLock<Backend> = OnceLock::new();
#[cfg(windows)]
static DLL_DIRECTORY_SETUP: OnceLock<Result<(), String>> = OnceLock::new();

#[cfg(windows)]
fn report_gpu_load_failure(message: impl AsRef<str>) {
    eprintln!(
        "[norito::gpu_zstd] {}. Falling back to the CPU backend.",
        message.as_ref()
    );
}

#[cfg(windows)]
fn ensure_secure_dll_search_path() -> Result<(), String> {
    DLL_DIRECTORY_SETUP
        .get_or_init(|| unsafe {
            let flags = LOAD_LIBRARY_SEARCH_DEFAULT_DIRS | LOAD_LIBRARY_SEARCH_SYSTEM32;
            if SetDefaultDllDirectories(flags) == 0 {
                Err(format!(
                    "SetDefaultDllDirectories failed: {}",
                    io::Error::last_os_error()
                ))
            } else {
                Ok(())
            }
        })
        .clone()
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn cuda_available() -> bool {
    Command::new("nvidia-smi")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(all(unix, not(all(target_os = "macos", target_arch = "aarch64"))))]
fn unix_gpu_helper_library_names() -> &'static [&'static str] {
    #[cfg(target_os = "macos")]
    {
        &[
            "libgpuzstd_cuda.dylib",
            "libgpuzstd_cuda.so",
            "libgpuzstd_metal.dylib",
            "libgpuzstd_metal.so",
        ]
    }
    #[cfg(not(target_os = "macos"))]
    {
        &["libgpuzstd_cuda.so", "libgpuzstd_metal.so"]
    }
}

#[cfg(windows)]
fn windows_gpu_helper_library_names() -> &'static [&'static str] {
    &["gpuzstd_cuda.dll", "gpuzstd_metal.dll"]
}

fn gpu_self_test(compress: CompressFn, decompress: DecompressFn) -> Result<(), SelfTestFailure> {
    const SAMPLE: &[u8] = b"norito gpu roundtrip parity check v1";
    // GPU encode the sample payload.
    let mut gpu_encoded = vec![0u8; SAMPLE.len().saturating_mul(4).saturating_add(512)];
    let mut gpu_len = gpu_encoded.len();
    let rc = unsafe {
        compress(
            SAMPLE.as_ptr(),
            SAMPLE.len(),
            1,
            gpu_encoded.as_mut_ptr(),
            &mut gpu_len,
        )
    };
    if rc == RC_GPU_UNAVAILABLE {
        // Helper is loaded but GPU kernels are currently unavailable.
        // Accept this mode if GPU-side decode still roundtrips CPU zstd frames.
        let cpu_encoded = zstd::encode_all(std::io::Cursor::new(SAMPLE), 1)
            .map_err(SelfTestFailure::CpuEncode)?;
        let mut gpu_decoded = vec![0u8; SAMPLE.len().saturating_mul(2).saturating_add(256)];
        let mut gpu_decoded_len = gpu_decoded.len();
        let rc = unsafe {
            decompress(
                cpu_encoded.as_ptr(),
                cpu_encoded.len(),
                gpu_decoded.as_mut_ptr(),
                &mut gpu_decoded_len,
            )
        };
        if rc != 0 || gpu_decoded_len == 0 || gpu_decoded_len > gpu_decoded.len() {
            return Err(SelfTestFailure::GpuDecodeCpu {
                rc,
                len: gpu_decoded_len,
                cap: gpu_decoded.len(),
            });
        }
        gpu_decoded.truncate(gpu_decoded_len);
        if gpu_decoded != SAMPLE {
            return Err(SelfTestFailure::GpuDecodeMismatch);
        }
        return Ok(());
    }
    if rc != 0 || gpu_len == 0 || gpu_len > gpu_encoded.len() {
        return Err(SelfTestFailure::GpuCompress {
            rc,
            len: gpu_len,
            cap: gpu_encoded.len(),
        });
    }
    gpu_encoded.truncate(gpu_len);
    // Ensure CPU zstd can decode the GPU output.
    let decoded_cpu = zstd::decode_all(std::io::Cursor::new(&gpu_encoded))
        .map_err(SelfTestFailure::CpuDecodeGpu)?;
    if decoded_cpu != SAMPLE {
        return Err(SelfTestFailure::CpuDecodeMismatch);
    }
    // Ensure the GPU decoder can roundtrip CPU-compressed bytes.
    let cpu_encoded =
        zstd::encode_all(std::io::Cursor::new(SAMPLE), 1).map_err(SelfTestFailure::CpuEncode)?;
    let mut gpu_decoded = vec![0u8; SAMPLE.len().saturating_mul(2).saturating_add(256)];
    let mut gpu_decoded_len = gpu_decoded.len();
    let rc = unsafe {
        decompress(
            cpu_encoded.as_ptr(),
            cpu_encoded.len(),
            gpu_decoded.as_mut_ptr(),
            &mut gpu_decoded_len,
        )
    };
    if rc != 0 || gpu_decoded_len == 0 || gpu_decoded_len > gpu_decoded.len() {
        return Err(SelfTestFailure::GpuDecodeCpu {
            rc,
            len: gpu_decoded_len,
            cap: gpu_decoded.len(),
        });
    }
    gpu_decoded.truncate(gpu_decoded_len);
    if gpu_decoded != SAMPLE {
        return Err(SelfTestFailure::GpuDecodeMismatch);
    }
    // Full GPU roundtrip for good measure.
    let mut gpu_roundtrip = vec![0u8; SAMPLE.len().saturating_mul(4).saturating_add(512)];
    let mut gpu_roundtrip_len = gpu_roundtrip.len();
    let rc = unsafe {
        compress(
            SAMPLE.as_ptr(),
            SAMPLE.len(),
            1,
            gpu_roundtrip.as_mut_ptr(),
            &mut gpu_roundtrip_len,
        )
    };
    if rc != 0 || gpu_roundtrip_len == 0 || gpu_roundtrip_len > gpu_roundtrip.len() {
        return Err(SelfTestFailure::GpuRoundtripCompress {
            rc,
            len: gpu_roundtrip_len,
            cap: gpu_roundtrip.len(),
        });
    }
    gpu_roundtrip.truncate(gpu_roundtrip_len);
    let mut gpu_roundtrip_out = vec![0u8; SAMPLE.len().saturating_mul(2).saturating_add(256)];
    let mut gpu_roundtrip_out_len = gpu_roundtrip_out.len();
    let rc = unsafe {
        decompress(
            gpu_roundtrip.as_ptr(),
            gpu_roundtrip.len(),
            gpu_roundtrip_out.as_mut_ptr(),
            &mut gpu_roundtrip_out_len,
        )
    };
    if rc != 0 || gpu_roundtrip_out_len == 0 || gpu_roundtrip_out_len > gpu_roundtrip_out.len() {
        return Err(SelfTestFailure::GpuRoundtripDecompress {
            rc,
            len: gpu_roundtrip_out_len,
            cap: gpu_roundtrip_out.len(),
        });
    }
    gpu_roundtrip_out.truncate(gpu_roundtrip_out_len);
    if gpu_roundtrip_out != SAMPLE {
        return Err(SelfTestFailure::GpuRoundtripMismatch);
    }
    Ok(())
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
unsafe fn init_backend() -> Option<Backend> {
    #[link(name = "Metal", kind = "framework")]
    unsafe extern "C" {
        fn MTLCreateSystemDefaultDevice() -> *mut c_void;
    }
    let pool = unsafe { objc_autoreleasePoolPush() };
    let device = unsafe { MTLCreateSystemDefaultDevice() };
    unsafe {
        objc_autoreleasePoolPop(pool);
    }
    let _has_device = !device.is_null();
    let (lib, compress_fn, decompress_fn) =
        match unsafe { load_gpu_symbols("libgpuzstd_metal.dylib") } {
            Some((lib, compress_fn, decompress_fn)) => (lib, compress_fn, decompress_fn),
            None => return None,
        };
    if let Err(err) = gpu_self_test(compress_fn, decompress_fn) {
        let _ = unsafe { dlclose(lib) };
        eprintln!(
            "[norito::gpu_zstd] Metal backend failed self-test ({}); falling back to CPU implementation",
            err
        );
        return None;
    }
    Some(Backend::Metal {
        compress: compress_fn,
        decompress: decompress_fn,
    })
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
unsafe fn init_backend() -> Option<Backend> {
    if !cuda_available() {
        return None;
    }
    #[cfg(unix)]
    {
        let (lib, compress_fn, decompress_fn) =
            match unsafe { load_gpu_symbols(unix_gpu_helper_library_names()) } {
                Some((lib, compress_fn, decompress_fn)) => (lib, compress_fn, decompress_fn),
                None => return None,
            };
        if let Err(err) = gpu_self_test(compress_fn, decompress_fn) {
            let _ = unsafe { dlclose(lib) };
            eprintln!(
                "[norito::gpu_zstd] CUDA backend failed self-test ({}); falling back to CPU implementation",
                err
            );
            return None;
        }
        return Some(Backend::Cuda {
            compress: compress_fn,
            decompress: decompress_fn,
        });
    }
    #[cfg(windows)]
    {
        if let Err(err) = ensure_secure_dll_search_path() {
            report_gpu_load_failure(err);
            return None;
        }
        let mut loaded = None;
        for helper_name in windows_gpu_helper_library_names() {
            let dll_name: Vec<u16> = OsStr::new(helper_name)
                .encode_wide()
                .chain(Some(0))
                .collect();
            let search_flags = LOAD_LIBRARY_SEARCH_DEFAULT_DIRS | LOAD_LIBRARY_SEARCH_SYSTEM32;
            let lib = LoadLibraryExW(dll_name.as_ptr(), ptr::null_mut(), search_flags);
            if lib.is_null() {
                continue;
            }
            let compress = GetProcAddress(lib, b"gpu_zstd_compress\0".as_ptr());
            let decompress = GetProcAddress(lib, b"gpu_zstd_decompress\0".as_ptr());
            if compress.is_null() || decompress.is_null() {
                let _ = FreeLibrary(lib);
                continue;
            }
            loaded = Some((
                lib,
                std::mem::transmute::<*mut c_void, CompressFn>(compress),
                std::mem::transmute::<*mut c_void, DecompressFn>(decompress),
            ));
            break;
        }
        let Some((lib, compress_fn, decompress_fn)) = loaded else {
            return None;
        };
        if let Err(err) = gpu_self_test(compress_fn, decompress_fn) {
            let _ = FreeLibrary(lib);
            report_gpu_load_failure(format!("CUDA backend failed self-test ({})", err));
            return None;
        }
        return Some(Backend::Cuda {
            compress: compress_fn,
            decompress: decompress_fn,
        });
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(unix)]
unsafe fn load_gpu_symbols(lib_names: &[&str]) -> Option<(*mut c_void, CompressFn, DecompressFn)> {
    use std::{env, ffi::CString, os::unix::ffi::OsStrExt, path::PathBuf};

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = env::current_exe()
        && let Some(dir) = exe.parent()
    {
        for lib_name in lib_names {
            candidates.extend(helper_candidates_from_exe_dir(dir, lib_name));
        }
    }

    for path in candidates {
        let bytes = path.as_os_str().as_bytes();
        if bytes.contains(&0) {
            continue;
        }
        if let Ok(path) = CString::new(bytes) {
            let handle = unsafe { dlopen(path.as_ptr(), RTLD_LAZY) };
            if !handle.is_null() {
                return unsafe { resolve_gpu_symbols(handle) };
            }
        }
    }

    for lib_name in lib_names {
        if let Ok(path) = CString::new(*lib_name) {
            let library = unsafe { dlopen(path.as_ptr(), RTLD_LAZY) };
            if !library.is_null()
                && let Some(loaded) = unsafe { resolve_gpu_symbols(library) }
            {
                return Some(loaded);
            }
        }
    }

    None
}

#[cfg(unix)]
unsafe fn resolve_gpu_symbols(
    library: *mut c_void,
) -> Option<(*mut c_void, CompressFn, DecompressFn)> {
    let compress = unsafe { dlsym(library, c"gpu_zstd_compress".as_ptr()) };
    let decompress = unsafe { dlsym(library, c"gpu_zstd_decompress".as_ptr()) };
    if compress.is_null() || decompress.is_null() {
        let _ = unsafe { dlclose(library) };
        return None;
    }

    let compress_fn: CompressFn = unsafe { std::mem::transmute(compress) };
    let decompress_fn: DecompressFn = unsafe { std::mem::transmute(decompress) };
    Some((library, compress_fn, decompress_fn))
}

#[cfg(unix)]
#[cfg_attr(all(target_os = "macos", target_arch = "aarch64"), allow(dead_code))]
fn helper_candidates_from_exe_dir(exe_dir: &Path, lib_name: &str) -> Vec<PathBuf> {
    vec![
        exe_dir.join(lib_name),
        exe_dir.join("../").join(lib_name),
        exe_dir.join("../lib").join(lib_name),
        exe_dir.join("../../lib").join(lib_name),
    ]
}

fn backend() -> &'static Backend {
    BACKEND.get_or_init(|| unsafe { init_backend().unwrap_or(Backend::Cpu) })
}

fn try_gpu_encode(compress: CompressFn, payload: &[u8], level: i32) -> Option<Vec<u8>> {
    let mut cap = payload.len().saturating_mul(2) + 128;
    for _ in 0..5 {
        let mut out = vec![0; cap];
        let mut out_len = out.len();
        let rc = unsafe {
            compress(
                payload.as_ptr(),
                payload.len(),
                level,
                out.as_mut_ptr(),
                &mut out_len,
            )
        };
        if rc == 0 {
            if out_len == 0 || out_len > out.len() {
                return None;
            }
            out.truncate(out_len);
            return Some(out);
        }
        cap = cap.saturating_mul(2);
    }
    None
}

fn try_gpu_decode(
    decompress: DecompressFn,
    compressed: &[u8],
    target_len: usize,
) -> Option<Vec<u8>> {
    let mut out = vec![0; target_len];
    let mut out_len = out.len();
    let rc = unsafe {
        decompress(
            compressed.as_ptr(),
            compressed.len(),
            out.as_mut_ptr(),
            &mut out_len,
        )
    };
    if rc != 0 || out_len > out.len() {
        return None;
    }
    out.truncate(out_len);
    Some(out)
}

/// Returns `true` if a supported GPU backend (CUDA or Metal) is available.
pub fn available() -> bool {
    if !super::hw::gpu_policy_allowed() {
        return false;
    }
    !matches!(backend(), Backend::Cpu)
}

pub fn encode_all(payload: Vec<u8>, level: i32) -> io::Result<Vec<u8>> {
    let min_gpu_bytes = super::heuristics::get().min_compress_bytes_gpu;
    if payload.len() < min_gpu_bytes {
        return zstd::encode_all(std::io::Cursor::new(payload), level);
    }
    match backend() {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        Backend::Metal { compress, .. } => {
            if let Some(out) = try_gpu_encode(*compress, &payload, level) {
                return Ok(out);
            }
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        Backend::Cuda { compress, .. } => {
            if let Some(out) = try_gpu_encode(*compress, &payload, level) {
                return Ok(out);
            }
        }
        Backend::Cpu => {}
    }

    // CPU fallback
    zstd::encode_all(std::io::Cursor::new(payload), level)
}

pub fn decode_all(compressed: &[u8], uncompressed_size: u64) -> Result<Vec<u8>, super::Error> {
    let target_len = super::payload_len_to_usize(uncompressed_size)?;
    match backend() {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        Backend::Metal { decompress, .. } => {
            if let Some(out) = try_gpu_decode(*decompress, compressed, target_len) {
                if out.len() != target_len {
                    return Err(super::Error::LengthMismatch);
                }
                return Ok(out);
            }
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        Backend::Cuda { decompress, .. } => {
            if let Some(out) = try_gpu_decode(*decompress, compressed, target_len) {
                if out.len() != target_len {
                    return Err(super::Error::LengthMismatch);
                }
                return Ok(out);
            }
        }
        Backend::Cpu => {}
    }

    // CPU fallback
    let decoder = zstd::Decoder::new(compressed)?;
    let mut out = Vec::with_capacity(target_len);
    let max_len = target_len.saturating_add(1);
    decoder.take(max_len as u64).read_to_end(&mut out)?;
    if out.len() != target_len {
        return Err(super::Error::LengthMismatch);
    }
    Ok(out)
}

#[cfg(all(test, feature = "gpu-compression"))]
mod tests {
    use rand::{Rng, SeedableRng, rngs::StdRng};
    use std::io::Cursor;

    use super::*;
    use crate::core::hw;

    #[test]
    fn raw_roundtrip() {
        let data = b"hello world".to_vec();
        let encoded = encode_all(data.clone(), 1).expect("encode");
        let decoded = decode_all(&encoded, data.len() as u64).expect("decode");
        assert_eq!(decoded, data);
    }

    #[test]
    fn decode_all_rejects_length_mismatch() {
        let data = b"length mismatch".to_vec();
        let encoded = encode_all(data.clone(), 1).expect("encode");
        let result = decode_all(&encoded, (data.len() as u64).saturating_sub(1));
        assert!(matches!(result, Err(crate::core::Error::LengthMismatch)));
    }

    #[test]
    fn availability_probe_runs() {
        // Should simply return a boolean without panicking
        let _ = available();
    }

    #[test]
    fn encode_all_small_payload_uses_cpu_path() {
        let min_gpu = crate::core::heuristics::get().min_compress_bytes_gpu;
        if min_gpu == 0 {
            return;
        }
        let size = min_gpu.saturating_sub(1).min(1024);
        let data = vec![0xA5u8; size];
        let cpu = zstd::encode_all(Cursor::new(&data), 1).expect("cpu encode");
        let gpu = encode_all(data, 1).expect("gpu encode");
        assert_eq!(gpu, cpu);
    }

    #[test]
    fn gpu_roundtrip_if_available() {
        if !available() {
            // Skip when no GPU backend is present
            return;
        }
        let data = b"gpu roundtrip".to_vec();
        let encoded = encode_all(data.clone(), 1).expect("encode");
        let decoded = decode_all(&encoded, data.len() as u64).expect("decode");
        assert_eq!(decoded, data);
    }

    fn sample_payload() -> Vec<u8> {
        let mut rng = StdRng::seed_from_u64(0xDEADBEEF);
        let mut payload = vec![0u8; 64 * 1024];
        rng.fill(payload.as_mut_slice());
        payload
    }

    #[test]
    fn gpu_encode_matches_cpu_when_available() {
        if !available() {
            eprintln!("GPU backend unavailable; skipping encode parity test");
            return;
        }
        let payload = sample_payload();
        let baseline_policy = hw::gpu_policy_allowed();
        hw::set_gpu_compression_allowed(false);
        let cpu_encoded =
            zstd::encode_all(std::io::Cursor::new(payload.clone()), 1).expect("cpu encode");
        hw::set_gpu_compression_allowed(true);
        let gpu_encoded = encode_all(payload.clone(), 1).expect("gpu encode");
        hw::set_gpu_compression_allowed(baseline_policy);
        let gpu_decoded = decode_all(&gpu_encoded, payload.len() as u64).expect("gpu decode");
        assert_eq!(
            gpu_decoded, payload,
            "GPU roundtrip must match original payload"
        );
        let cpu_decoded = decode_all(&cpu_encoded, payload.len() as u64).expect("cpu decode");
        assert_eq!(
            cpu_decoded, payload,
            "CPU roundtrip must match original payload"
        );
        assert_eq!(
            gpu_encoded.len(),
            cpu_encoded.len(),
            "GPU and CPU encoded outputs should match in length"
        );
    }

    #[test]
    fn gpu_decode_matches_cpu_when_available() {
        if !available() {
            eprintln!("GPU backend unavailable; skipping decode parity test");
            return;
        }
        let payload = sample_payload();
        let cpu_encoded =
            zstd::encode_all(std::io::Cursor::new(payload.clone()), 3).expect("cpu encode");
        let baseline_policy = hw::gpu_policy_allowed();
        hw::set_gpu_compression_allowed(true);
        let gpu_decoded = decode_all(&cpu_encoded, payload.len() as u64).expect("gpu decode");
        hw::set_gpu_compression_allowed(false);
        let cpu_decoded = decode_all(&cpu_encoded, payload.len() as u64).expect("cpu decode");
        hw::set_gpu_compression_allowed(baseline_policy);
        assert_eq!(gpu_decoded, payload, "GPU decode must match CPU reference");
        assert_eq!(
            cpu_decoded, payload,
            "CPU decode must match original payload"
        );
    }
}

#[cfg(test)]
mod self_test {
    use std::{io, path::Path, path::PathBuf, ptr, slice};

    use super::*;

    unsafe extern "C" fn compress_stub(
        src: *const u8,
        src_len: usize,
        level: i32,
        dst: *mut u8,
        dst_len: *mut usize,
    ) -> i32 {
        let input = unsafe { slice::from_raw_parts(src, src_len) };
        let encoded = zstd::encode_all(io::Cursor::new(input), level).expect("cpu encode");
        let capacity = unsafe { *dst_len };
        if encoded.len() > capacity {
            return 1;
        }
        unsafe {
            ptr::copy_nonoverlapping(encoded.as_ptr(), dst, encoded.len());
            *dst_len = encoded.len();
        }
        0
    }

    unsafe extern "C" fn decompress_stub(
        src: *const u8,
        src_len: usize,
        dst: *mut u8,
        dst_len: *mut usize,
    ) -> i32 {
        let input = unsafe { slice::from_raw_parts(src, src_len) };
        let decoded =
            zstd::decode_all(io::Cursor::new(input)).expect("cpu decode in stub should succeed");
        let capacity = unsafe { *dst_len };
        if decoded.len() > capacity {
            return 1;
        }
        unsafe {
            ptr::copy_nonoverlapping(decoded.as_ptr(), dst, decoded.len());
            *dst_len = decoded.len();
        }
        0
    }

    unsafe extern "C" fn compress_unavailable_stub(
        _src: *const u8,
        _src_len: usize,
        _level: i32,
        _dst: *mut u8,
        _dst_len: *mut usize,
    ) -> i32 {
        RC_GPU_UNAVAILABLE
    }

    unsafe extern "C" fn compress_invalid_len_success(
        _src: *const u8,
        _src_len: usize,
        _level: i32,
        _dst: *mut u8,
        dst_len: *mut usize,
    ) -> i32 {
        let capacity = unsafe { *dst_len };
        unsafe { *dst_len = capacity.saturating_add(1) };
        0
    }

    unsafe extern "C" fn decompress_invalid_len_success(
        _src: *const u8,
        _src_len: usize,
        _dst: *mut u8,
        dst_len: *mut usize,
    ) -> i32 {
        let capacity = unsafe { *dst_len };
        unsafe { *dst_len = capacity.saturating_add(1) };
        0
    }

    #[test]
    fn gpu_self_test_passes_for_cpu_stubs() {
        assert!(gpu_self_test(compress_stub, decompress_stub).is_ok());
    }

    #[test]
    fn gpu_self_test_accepts_unavailable_compress_when_decode_works() {
        assert!(gpu_self_test(compress_unavailable_stub, decompress_stub).is_ok());
    }

    #[test]
    fn try_gpu_encode_rejects_invalid_success_length() {
        let payload = b"encode helper length check";
        assert!(try_gpu_encode(compress_invalid_len_success, payload, 1).is_none());
    }

    #[test]
    fn try_gpu_decode_rejects_invalid_success_length() {
        let payload = b"decode helper length check";
        let encoded = zstd::encode_all(io::Cursor::new(payload), 1).expect("cpu encode");
        assert!(try_gpu_decode(decompress_invalid_len_success, &encoded, payload.len()).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn helper_candidates_include_parent_sibling_library() {
        let exe_dir = Path::new("/workspace/target/release/examples");
        let candidates = helper_candidates_from_exe_dir(exe_dir, "libgpuzstd_metal.dylib");
        assert_eq!(
            candidates[0],
            PathBuf::from("/workspace/target/release/examples/libgpuzstd_metal.dylib")
        );
        assert!(
            candidates.iter().any(|path| {
                path == &PathBuf::from(
                    "/workspace/target/release/examples/../libgpuzstd_metal.dylib",
                )
            }),
            "candidate list should include parent sibling dylib used by cargo-run examples"
        );
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn unix_gpu_helper_names_include_workspace_fallback() {
        let names = unix_gpu_helper_library_names();
        assert_eq!(
            names.first().copied(),
            Some("libgpuzstd_cuda.so"),
            "linux helper search should prefer the dedicated CUDA-named helper when both artifacts exist"
        );
        assert!(
            names.contains(&"libgpuzstd_cuda.so"),
            "linux helper search must keep the CUDA helper name first-class"
        );
        assert!(
            names.contains(&"libgpuzstd_metal.so"),
            "linux helper search should also accept the workspace-built gpuzstd_metal artifact"
        );
    }

    unsafe extern "C" fn compress_corrupt(
        _src: *const u8,
        _src_len: usize,
        _level: i32,
        dst: *mut u8,
        dst_len: *mut usize,
    ) -> i32 {
        let capacity = unsafe { *dst_len };
        if capacity == 0 {
            return 1;
        }
        let bytes = 8.min(capacity);
        unsafe {
            ptr::write_bytes(dst, 0xA5, bytes);
            *dst_len = bytes;
        }
        0
    }

    #[test]
    fn gpu_self_test_detects_corruption() {
        let err = gpu_self_test(compress_corrupt, decompress_stub)
            .expect_err("corrupted payload should fail self-test");
        assert!(matches!(err, SelfTestFailure::CpuDecodeGpu(_)));
    }

    #[test]
    fn self_test_error_reports_rc() {
        let err = SelfTestFailure::GpuCompress {
            rc: 3,
            len: 0,
            cap: 16,
        };
        let msg = err.to_string();
        assert!(msg.contains("rc=3"));
    }
}
