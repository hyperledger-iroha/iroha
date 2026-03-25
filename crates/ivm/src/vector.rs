//! Vector and cryptographic helper functions used by the VM.
//!
//! This module implements the primitives required by the specification,
//! including lane-wise arithmetic, bitwise operations and the SHA‑256
//! compression round used by the `SHA256BLOCK` instruction.
//!
//! The updated spec defines a **scalable vector extension** which adapts to the
//! host CPU's SIMD width.  These helpers now detect the available SIMD features
//! (SSE, AVX2, AVX‑512 or NEON) at runtime and operate on a lane count derived
//! from that width.  When no SIMD support is available, a 32-bit scalar fallback
//! is used.  Additional cryptographic primitives such as AESENC or BLAKE2s
//! should also be implemented here with optional hardware acceleration.

#[cfg(all(target_os = "macos", feature = "metal"))]
use objc2::Message;
#[cfg(all(target_os = "macos", feature = "metal"))]
use objc2_foundation::NSUInteger;
#[cfg(all(target_os = "macos", feature = "metal"))]
use objc2_metal::{MTLCommandBuffer, MTLDevice};
#[cfg(all(target_os = "macos", feature = "metal"))]
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGMainDisplayID() -> u32;
}

#[cfg(all(target_os = "macos", feature = "metal"))]
fn warm_up_core_graphics_display() {
    unsafe {
        let _ = CGMainDisplayID();
    }
}
#[cfg(all(target_os = "macos", feature = "metal"))]
const METAL_ED25519_SHADER: &str = include_str!("metal_ed25519.metal");

use std::sync::{Mutex, MutexGuard};
/// Return true if the host CPU supports SIMD operations that the vector
/// extension can leverage. This is a lightweight stand‑in for more elaborate
/// feature detection used by a production implementation.
/// Determine if SIMD vector operations are available on the current
/// architecture.  This helper is intentionally small and only checks for a
/// minimal baseline feature on supported platforms.  Additional runtime feature
/// detection can be added as the implementation grows.
use std::{
    cell::Cell,
    sync::{
        OnceLock,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
};
#[cfg(all(target_os = "macos", feature = "metal"))]
use std::{mem::transmute, sync::atomic::AtomicUsize};

/// The SIMD backend selected at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdChoice {
    /// AVX-512 back-end (requires AVX2 for the current 256-bit implementations).
    Avx512,
    Avx2,
    Sse2,
    Neon,
    Scalar,
}

static SIMD_CHOICE: OnceLock<SimdChoice> = OnceLock::new();
static SIMD_POLICY_ENABLED: AtomicBool = AtomicBool::new(true);
static SIMD_OVERRIDE: AtomicU8 = AtomicU8::new(0);
thread_local! {
    static TLS_SIMD_OVERRIDE: Cell<u8> = const { Cell::new(0) };
}

fn forced_simd_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub fn forced_simd_test_lock() -> MutexGuard<'static, ()> {
    forced_simd_lock()
        .lock()
        .expect("forced_simd lock poisoned")
}

fn encode_override(choice: Option<SimdChoice>) -> u8 {
    match choice {
        None => 0,
        Some(SimdChoice::Scalar) => 1,
        Some(SimdChoice::Sse2) => 2,
        Some(SimdChoice::Avx2) => 3,
        Some(SimdChoice::Avx512) => 4,
        Some(SimdChoice::Neon) => 5,
    }
}

fn decode_override(val: u8) -> Option<SimdChoice> {
    match val {
        1 => Some(SimdChoice::Scalar),
        2 => Some(SimdChoice::Sse2),
        3 => Some(SimdChoice::Avx2),
        4 => Some(SimdChoice::Avx512),
        5 => Some(SimdChoice::Neon),
        _ => None,
    }
}

fn clamp_to_supported(choice: SimdChoice) -> SimdChoice {
    match choice {
        SimdChoice::Scalar => SimdChoice::Scalar,
        #[cfg(target_arch = "x86_64")]
        SimdChoice::Avx512 => {
            if std::is_x86_feature_detected!("avx512f") && std::is_x86_feature_detected!("avx2") {
                SimdChoice::Avx512
            } else {
                SimdChoice::Scalar
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        SimdChoice::Avx512 => SimdChoice::Scalar,
        #[cfg(target_arch = "x86_64")]
        SimdChoice::Avx2 => {
            if std::is_x86_feature_detected!("avx2") {
                SimdChoice::Avx2
            } else {
                SimdChoice::Scalar
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        SimdChoice::Avx2 => SimdChoice::Scalar,
        #[cfg(target_arch = "x86_64")]
        SimdChoice::Sse2 => {
            if std::is_x86_feature_detected!("sse2") {
                SimdChoice::Sse2
            } else {
                SimdChoice::Scalar
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        SimdChoice::Sse2 => SimdChoice::Scalar,
        #[cfg(target_arch = "aarch64")]
        SimdChoice::Neon => {
            if std::arch::is_aarch64_feature_detected!("neon") {
                SimdChoice::Neon
            } else {
                SimdChoice::Scalar
            }
        }
        #[cfg(not(target_arch = "aarch64"))]
        SimdChoice::Neon => SimdChoice::Scalar,
    }
}

/// Return any configured process-wide SIMD override, clamped to supported backends.
pub(crate) fn forced_simd_choice() -> Option<SimdChoice> {
    if let Some(choice) = TLS_SIMD_OVERRIDE.with(|c| decode_override(c.get())) {
        return Some(clamp_to_supported(choice));
    }
    decode_override(SIMD_OVERRIDE.load(Ordering::Relaxed)).map(clamp_to_supported)
}

/// Override runtime SIMD detection for this process.
///
/// This is intended for configuration- or test-driven overrides rather than
/// production feature gating; unsupported overrides fall back to `Scalar`.
/// Returns the previously configured override, if any.
pub fn set_forced_simd(choice: Option<SimdChoice>) -> Option<SimdChoice> {
    decode_override(SIMD_OVERRIDE.swap(encode_override(choice), Ordering::Relaxed))
}

/// Clear any configured SIMD override and resume hardware detection.
pub fn clear_forced_simd() {
    set_forced_simd(None);
}

fn detect_simd_choice() -> SimdChoice {
    *SIMD_CHOICE.get_or_init(|| {
        #[cfg(target_arch = "x86_64")]
        {
            if std::is_x86_feature_detected!("avx512f") && std::is_x86_feature_detected!("avx2") {
                return SimdChoice::Avx512;
            }
            if std::is_x86_feature_detected!("avx2") {
                return SimdChoice::Avx2;
            }
            if std::is_x86_feature_detected!("sse2") {
                return SimdChoice::Sse2;
            }
            SimdChoice::Scalar
        }
        #[cfg(target_arch = "aarch64")]
        {
            if std::arch::is_aarch64_feature_detected!("neon") {
                return SimdChoice::Neon;
            }
            SimdChoice::Scalar
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            SimdChoice::Scalar
        }
    })
}

/// Return the hardware-detected SIMD choice without policy/override influence.
pub(crate) fn detected_simd_choice() -> SimdChoice {
    detect_simd_choice()
}

/// Whether SIMD is allowed by the current acceleration policy.
pub(crate) fn simd_policy_enabled() -> bool {
    SIMD_POLICY_ENABLED.load(Ordering::Relaxed)
}

/// Apply a process-wide SIMD policy. When disabled, SIMD detection/overrides are
/// ignored and scalar execution is forced.
pub(crate) fn set_simd_policy_enabled(enabled: bool) {
    SIMD_POLICY_ENABLED.store(enabled, Ordering::SeqCst);
    if enabled {
        clear_forced_simd();
    } else {
        set_forced_simd(Some(SimdChoice::Scalar));
    }
}

/// Override runtime SIMD detection for the current thread only.
/// Returns the previous thread-local override (if any).
pub fn set_thread_forced_simd(choice: Option<SimdChoice>) -> Option<SimdChoice> {
    TLS_SIMD_OVERRIDE.with(|cell| {
        let prev = cell.replace(encode_override(choice));
        decode_override(prev)
    })
}

/// Clear the thread-local override.
#[allow(dead_code)]
pub fn clear_thread_forced_simd() {
    set_thread_forced_simd(None);
}

/// Return the globally configured SIMD override, if any.
///
/// When `None`, the runtime auto-detects the best SIMD backend. A concrete
/// `SimdChoice` here means the override is currently forcing a specific backend
/// (typically scalar) regardless of hardware support.
#[allow(dead_code)]
pub fn simd_override() -> Option<SimdChoice> {
    decode_override(SIMD_OVERRIDE.load(Ordering::Relaxed))
}

#[cfg(all(target_os = "macos", feature = "metal"))]
static METAL_DISABLED: AtomicBool = AtomicBool::new(false);
#[cfg(all(target_os = "macos", feature = "metal"))]
static METAL_FORCED_DISABLED: AtomicBool = AtomicBool::new(false);
#[cfg(all(target_os = "macos", feature = "metal"))]
static METAL_CONFIG_ENABLED: AtomicBool = AtomicBool::new(true);
#[cfg(all(target_os = "macos", feature = "metal"))]
static METAL_LAST_ERROR: OnceLock<Mutex<Option<String>>> = OnceLock::new();

#[cfg(all(target_os = "macos", feature = "metal"))]
fn metal_error_slot() -> &'static Mutex<Option<String>> {
    METAL_LAST_ERROR.get_or_init(|| Mutex::new(None))
}

#[cfg(all(target_os = "macos", feature = "metal"))]
fn set_metal_status_message(message: Option<String>) {
    if let Ok(mut guard) = metal_error_slot().lock() {
        *guard = message;
    }
}

#[cfg(all(target_os = "macos", feature = "metal"))]
fn record_metal_disable(reason: impl Into<String>) {
    let message = reason.into();
    METAL_DISABLED.store(true, Ordering::SeqCst);
    if let Ok(mut guard) = metal_error_slot().lock() {
        *guard = Some(message.clone());
    }
    eprintln!("ivm: metal backend disabled: {message}");
}

#[cfg(all(target_os = "macos", feature = "metal"))]
pub(crate) fn metal_last_error_message() -> Option<String> {
    metal_error_slot()
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
pub(crate) fn metal_last_error_message() -> Option<String> {
    None
}

#[cfg(all(target_os = "macos", feature = "metal"))]
fn metal_env_disabled() -> bool {
    if !crate::dev_env::dev_env_flag("IVM_DISABLE_METAL") {
        return false;
    }
    let disabled = std::env::var("IVM_DISABLE_METAL")
        .map(|v| v.trim() == "1")
        .unwrap_or(false);
    if disabled {
        set_metal_status_message(Some(
            "disabled by IVM_DISABLE_METAL environment override".to_owned(),
        ));
    }
    disabled
}

#[cfg(all(target_os = "macos", feature = "metal"))]
pub(crate) fn metal_policy_enabled() -> bool {
    if metal_env_disabled() {
        return false;
    }
    if !METAL_CONFIG_ENABLED.load(Ordering::SeqCst) {
        return false;
    }
    if METAL_FORCED_DISABLED.load(Ordering::SeqCst) {
        return false;
    }
    true
}

#[cfg(all(target_os = "macos", feature = "metal"))]
pub(crate) fn metal_runtime_allowed() -> bool {
    metal_policy_enabled() && !METAL_DISABLED.load(Ordering::SeqCst)
}

#[cfg(all(target_os = "macos", feature = "metal"))]
pub fn metal_disabled() -> bool {
    !metal_policy_enabled() || METAL_DISABLED.load(Ordering::SeqCst)
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
pub fn metal_disabled() -> bool {
    false
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
pub(crate) fn metal_policy_enabled() -> bool {
    false
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
pub(crate) fn metal_runtime_allowed() -> bool {
    false
}

#[cfg(all(target_os = "macos", feature = "metal"))]
pub(crate) fn metal_parity_ok() -> bool {
    !METAL_DISABLED.load(Ordering::SeqCst)
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
pub(crate) fn metal_parity_ok() -> bool {
    false
}

/// Ensure Metal pipelines are compiled ahead of time to avoid first-use latency.
#[cfg(all(target_os = "macos", feature = "metal"))]
pub fn warm_up_metal() {
    let _ = with_metal_state(|_| ());
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
pub fn warm_up_metal() {}

/// Detect the best available SIMD option for the current platform.
pub fn simd_choice() -> SimdChoice {
    if !simd_policy_enabled() {
        return SimdChoice::Scalar;
    }
    if let Some(choice) = forced_simd_choice() {
        return choice;
    }
    detect_simd_choice()
}

/// Return true if the host CPU supports SIMD operations that the vector
/// extension can leverage.
pub fn vector_supported() -> bool {
    simd_choice() != SimdChoice::Scalar
}

/// Return the detected SIMD vector width in bits. Platforms without SIMD
/// support fall back to 32-bit scalar operations.
pub fn simd_bits() -> usize {
    match simd_choice() {
        SimdChoice::Avx512 => 512,
        SimdChoice::Avx2 => 256,
        SimdChoice::Sse2 | SimdChoice::Neon => 128,
        SimdChoice::Scalar => 32,
    }
}

/// Number of 32-bit lanes available for vector operations.
pub fn simd_lanes() -> usize {
    simd_bits() / 32
}

/// Return a short string describing the SIMD backend selected at runtime.
pub fn simd_backend() -> &'static str {
    match simd_choice() {
        SimdChoice::Avx512 => "avx512",
        SimdChoice::Avx2 => "avx2",
        SimdChoice::Sse2 => "sse2",
        SimdChoice::Neon => "neon",
        SimdChoice::Scalar => "scalar",
    }
}

/// Return true if Apple Metal GPU acceleration can be used.
#[allow(unused_variables)]
pub fn metal_available() -> bool {
    #[cfg(all(target_os = "macos", feature = "metal"))]
    {
        if !metal_runtime_allowed() {
            return false;
        }
        discover_metal_device().is_some()
    }
    #[cfg(not(all(target_os = "macos", feature = "metal")))]
    {
        false
    }
}

#[cfg(all(target_os = "macos", feature = "metal"))]
use std::cell::OnceCell;

#[cfg(all(target_os = "macos", feature = "metal"))]
use objc2::rc::Retained;
#[cfg(all(target_os = "macos", feature = "metal"))]
use objc2::runtime::ProtocolObject;

#[cfg(all(target_os = "macos", feature = "metal"))]
fn discover_metal_device() -> Option<Retained<ProtocolObject<dyn objc2_metal::MTLDevice>>> {
    // Debug knob: force enumeration even if system default device is available.
    let force_enumeration = if crate::dev_env::dev_env_flag("IVM_FORCE_METAL_ENUM") {
        std::env::var("IVM_FORCE_METAL_ENUM")
            .map(|v| matches!(v.trim(), "1" | "true" | "TRUE"))
            .unwrap_or(false)
    } else {
        false
    };

    // Touch the CoreGraphics display connection on headless hosts so
    // MTLCopyAllDevices can enumerate GPU drivers.
    warm_up_core_graphics_display();

    // Prefer the system default device once the CoreGraphics session is ready.
    if !force_enumeration {
        if let Some(device) = objc2_metal::MTLCreateSystemDefaultDevice() {
            return Some(device);
        }
    }

    let devices = objc2_metal::MTLCopyAllDevices();
    let mut retained_devices = Vec::new();
    let mut traits = Vec::new();
    for device in devices.iter() {
        retained_devices.push(device.retain());
        traits.push(DeviceTraits {
            headless: device.isHeadless(),
            low_power: device.isLowPower(),
        });
    }

    let debug_enum = if crate::dev_env::dev_env_flag("IVM_DEBUG_METAL_ENUM") {
        std::env::var("IVM_DEBUG_METAL_ENUM")
            .map(|v| matches!(v.trim(), "1" | "true" | "TRUE"))
            .unwrap_or(false)
    } else {
        false
    };

    if debug_enum {
        eprintln!(
            "ivm: MTLCopyAllDevices returned {} device(s)",
            retained_devices.len()
        );
        for (idx, info) in traits.iter().enumerate() {
            eprintln!(
                "ivm:   device #{idx}: headless={}, low_power={}",
                info.headless, info.low_power
            );
        }
    }

    let Some(index) = select_device_index(&traits) else {
        set_metal_status_message(Some(
            "no Metal devices returned by MTLCopyAllDevices".to_owned(),
        ));
        if debug_enum {
            eprintln!("ivm: unable to select Metal device (empty enumeration)");
        }

        // As a last resort (and only when enumeration is not forced) try the
        // default device again now that CoreGraphics has been warmed up. Some
        // headless shells load the Metal drivers lazily and only expose the
        // default device once CG establishes a session, so retrying here lets
        // CLI captures use GPU acceleration even when MTLCopyAllDevices fails.
        if !force_enumeration {
            if let Some(device) = objc2_metal::MTLCreateSystemDefaultDevice() {
                if debug_enum {
                    eprintln!(
                        "ivm: MTLCopyAllDevices returned zero devices; using MTLCreateSystemDefaultDevice fallback"
                    );
                }
                set_metal_status_message(None);
                return Some(device);
            }
        }
        return None;
    };

    if debug_enum {
        eprintln!("ivm: selecting device #{}", index);
    }

    retained_devices.into_iter().nth(index)
}

#[cfg(all(target_os = "macos", feature = "metal"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DeviceTraits {
    headless: bool,
    low_power: bool,
}

#[cfg(all(target_os = "macos", feature = "metal"))]
fn select_device_index(traits: &[DeviceTraits]) -> Option<usize> {
    let mut first_non_headless = None;
    let mut first_any = None;

    for (idx, info) in traits.iter().enumerate() {
        if !info.headless {
            if !info.low_power {
                return Some(idx);
            }
            if first_non_headless.is_none() {
                first_non_headless = Some(idx);
            }
        }
        if first_any.is_none() {
            first_any = Some(idx);
        }
    }

    first_non_headless.or(first_any)
}

#[cfg(all(target_os = "macos", feature = "metal"))]
static BIT_PIPE_COMPILES: AtomicUsize = AtomicUsize::new(0);

#[cfg(all(target_os = "macos", feature = "metal"))]
pub fn bit_pipe_compile_count() -> usize {
    BIT_PIPE_COMPILES.load(Ordering::SeqCst)
}

// Provide no-op fallbacks for Metal counters when Metal is not enabled.
#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
pub fn bit_pipe_compile_count() -> usize {
    0
}

#[cfg(all(target_os = "macos", feature = "metal"))]
struct MetalState {
    device: Retained<ProtocolObject<dyn objc2_metal::MTLDevice>>,
    queue: Retained<ProtocolObject<dyn objc2_metal::MTLCommandQueue>>,
    vadd64: Retained<ProtocolObject<dyn objc2_metal::MTLComputePipelineState>>,
    vadd32: Retained<ProtocolObject<dyn objc2_metal::MTLComputePipelineState>>,
    vand: Retained<ProtocolObject<dyn objc2_metal::MTLComputePipelineState>>,
    vxor: Retained<ProtocolObject<dyn objc2_metal::MTLComputePipelineState>>,
    vor: Retained<ProtocolObject<dyn objc2_metal::MTLComputePipelineState>>,
    sha256: Retained<ProtocolObject<dyn objc2_metal::MTLComputePipelineState>>,
    sha256_leaves: Retained<ProtocolObject<dyn objc2_metal::MTLComputePipelineState>>,
    sha256_pairs: Retained<ProtocolObject<dyn objc2_metal::MTLComputePipelineState>>,
    keccak: Retained<ProtocolObject<dyn objc2_metal::MTLComputePipelineState>>,
    aesenc: Retained<ProtocolObject<dyn objc2_metal::MTLComputePipelineState>>,
    aesdec: Retained<ProtocolObject<dyn objc2_metal::MTLComputePipelineState>>,
    aesenc_batch: Retained<ProtocolObject<dyn objc2_metal::MTLComputePipelineState>>,
    aesdec_batch: Retained<ProtocolObject<dyn objc2_metal::MTLComputePipelineState>>,
    aesenc_rounds: Retained<ProtocolObject<dyn objc2_metal::MTLComputePipelineState>>,
    aesdec_rounds: Retained<ProtocolObject<dyn objc2_metal::MTLComputePipelineState>>,
    ed25519_signature: Option<Retained<ProtocolObject<dyn objc2_metal::MTLComputePipelineState>>>,
}

#[cfg(all(target_os = "macos", feature = "metal"))]
impl MetalState {
    fn new() -> Option<Self> {
        use objc2_foundation::{NSString, ns_string};
        use objc2_metal::*;

        let device = discover_metal_device()?;
        let queue = device.newCommandQueue()?;

        let src = r#"
            #include <metal_stdlib>
            using namespace metal;
            kernel void vadd64(device const ulong2* a [[buffer(0)]],
                               device const ulong2* b [[buffer(1)]],
                               device ulong2* out [[buffer(2)]],
                               uint id [[thread_position_in_grid]]) {
                out[id] = a[id] + b[id];
            }
        "#;
        let lib = device
            .newLibraryWithSource_options_error(&NSString::from_str(src), None)
            .ok()?;
        let func = lib.newFunctionWithName(ns_string!("vadd64"))?;
        let vadd64 = device
            .newComputePipelineStateWithFunction_error(&func)
            .ok()?;

        let src = r#"
            #include <metal_stdlib>
            using namespace metal;
            kernel void vadd32(device const uint4* a [[buffer(0)]],
                               device const uint4* b [[buffer(1)]],
                               device uint4* out [[buffer(2)]],
                               uint id [[thread_position_in_grid]]) {
                out[id] = a[id] + b[id];
            }
        "#;
        let lib = device
            .newLibraryWithSource_options_error(&NSString::from_str(src), None)
            .ok()?;
        let func = lib.newFunctionWithName(ns_string!("vadd32"))?;
        let vadd32 = device
            .newComputePipelineStateWithFunction_error(&func)
            .ok()?;

        fn compile_bit(
            device: &ProtocolObject<dyn objc2_metal::MTLDevice>,
            op: &str,
            name: &str,
        ) -> Option<Retained<ProtocolObject<dyn objc2_metal::MTLComputePipelineState>>> {
            use objc2_foundation::NSString;
            use objc2_metal::*;
            #[cfg(target_os = "macos")]
            BIT_PIPE_COMPILES.fetch_add(1, Ordering::SeqCst);
            let source = format!(
                "#include <metal_stdlib>\nusing namespace metal;\nkernel void {name}(device const uint4* a [[buffer(0)]], device const uint4* b [[buffer(1)]], device uint4* out [[buffer(2)]], uint id [[thread_position_in_grid]]) {{ out[id] = a[id] {op} b[id]; }}",
                name = name,
                op = op,
            );
            let lib = device
                .newLibraryWithSource_options_error(&NSString::from_str(&source), None)
                .ok()?;
            let func = lib.newFunctionWithName(&NSString::from_str(name))?;
            let pipe = device
                .newComputePipelineStateWithFunction_error(&func)
                .ok()?;
            Some(pipe)
        }

        let vand = compile_bit(&device, "&", "vand")?;
        let vxor = compile_bit(&device, "^", "vxor")?;
        let vor = compile_bit(&device, "|", "vor")?;

        let src = r#"
            #include <metal_stdlib>
            using namespace metal;
            inline uint rotr(uint x, uint n) {
                return (x >> n) | (x << (32 - n));
            }
            constant uint K[64] = {
                0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
                0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
                0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
                0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
                0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
                0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
                0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
                0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
                0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
                0xc67178f2,
            };
            kernel void sha256_compress(device uint* state [[buffer(0)]],
                                         device const uchar* block [[buffer(1)]],
                                         uint id [[thread_position_in_grid]]) {
                uint w[64];
                for (uint t = 0; t < 16; ++t) {
                    uint i = t * 4;
                    w[t] = (uint(block[i]) << 24) | (uint(block[i+1]) << 16) |
                           (uint(block[i+2]) << 8) | uint(block[i+3]);
                }
                for (uint t = 16; t < 64; ++t) {
                    uint s0 = rotr(w[t-15],7) ^ rotr(w[t-15],18) ^ (w[t-15] >> 3);
                    uint s1 = rotr(w[t-2],17) ^ rotr(w[t-2],19) ^ (w[t-2] >> 10);
                    w[t] = w[t-16] + s0 + w[t-7] + s1;
                }
                uint a = state[0];
                uint b = state[1];
                uint c = state[2];
                uint d = state[3];
                uint e = state[4];
                uint f = state[5];
                uint g = state[6];
                uint h = state[7];
                for (uint t = 0; t < 64; ++t) {
                    uint s1 = rotr(e,6) ^ rotr(e,11) ^ rotr(e,25);
                    uint ch = (e & f) ^ ((~e) & g);
                    uint temp1 = h + s1 + ch + K[t] + w[t];
                    uint s0 = rotr(a,2) ^ rotr(a,13) ^ rotr(a,22);
                    uint maj = (a & b) ^ (a & c) ^ (b & c);
                    uint temp2 = s0 + maj;
                    h = g;
                    g = f;
                    f = e;
                    e = d + temp1;
                    d = c;
                    c = b;
                    b = a;
                    a = temp1 + temp2;
                }
                state[0] += a;
                state[1] += b;
                state[2] += c;
                state[3] += d;
                state[4] += e;
                state[5] += f;
                state[6] += g;
                state[7] += h;
            }
        "#;
        let lib = device
            .newLibraryWithSource_options_error(&NSString::from_str(src), None)
            .ok()?;
        let func = lib.newFunctionWithName(ns_string!("sha256_compress"))?;
        let sha256 = device
            .newComputePipelineStateWithFunction_error(&func)
            .ok()?;

        // Batch SHA-256 leaves kernel: one 64-byte block per thread
        let src = r#"
            #include <metal_stdlib>
            using namespace metal;
            inline uint rotr(uint x, uint n) { return (x >> n) | (x << (32 - n)); }
            constant uint K[64] = {
                0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
                0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
                0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
                0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
                0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
                0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
                0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
                0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
            };
            constant uint H0[8] = {
                0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,
                0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19
            };
            kernel void sha256_leaves(device const uchar* blocks [[buffer(0)]],
                                      device uint* out_states [[buffer(1)]],
                                      uint id [[thread_position_in_grid]]) {
                const device uchar* block = blocks + (id * 64);
                uint w[64];
                for (uint t = 0; t < 16; ++t) {
                    uint i = t * 4;
                    w[t] = (uint(block[i]) << 24) | (uint(block[i+1]) << 16) |
                           (uint(block[i+2]) << 8) | uint(block[i+3]);
                }
                for (uint t = 16; t < 64; ++t) {
                    uint s0 = rotr(w[t-15],7) ^ rotr(w[t-15],18) ^ (w[t-15] >> 3);
                    uint s1 = rotr(w[t-2],17) ^ rotr(w[t-2],19) ^ (w[t-2] >> 10);
                    w[t] = w[t-16] + s0 + w[t-7] + s1;
                }
                uint a=H0[0], b=H0[1], c=H0[2], d=H0[3], e=H0[4], f=H0[5], g=H0[6], h=H0[7];
                for (uint t = 0; t < 64; ++t) {
                    uint s1 = rotr(e,6) ^ rotr(e,11) ^ rotr(e,25);
                    uint ch = (e & f) ^ ((~e) & g);
                    uint temp1 = h + s1 + ch + K[t] + w[t];
                    uint s0 = rotr(a,2) ^ rotr(a,13) ^ rotr(a,22);
                    uint maj = (a & b) ^ (a & c) ^ (b & c);
                    uint temp2 = s0 + maj;
                    h = g; g = f; f = e; e = d + temp1; d = c; c = b; b = a; a = temp1 + temp2;
                }
                uint out0 = H0[0] + a;
                uint out1 = H0[1] + b;
                uint out2 = H0[2] + c;
                uint out3 = H0[3] + d;
                uint out4 = H0[4] + e;
                uint out5 = H0[5] + f;
                uint out6 = H0[6] + g;
                uint out7 = H0[7] + h;
                device uint* dst = out_states + (id * 8);
                dst[0]=out0; dst[1]=out1; dst[2]=out2; dst[3]=out3;
                dst[4]=out4; dst[5]=out5; dst[6]=out6; dst[7]=out7;
            }
        "#;
        let lib = device
            .newLibraryWithSource_options_error(&NSString::from_str(src), None)
            .ok()?;
        let func = lib.newFunctionWithName(ns_string!("sha256_leaves"))?;
        let sha256_leaves = device
            .newComputePipelineStateWithFunction_error(&func)
            .ok()?;

        let src = r#"
            #include <metal_stdlib>
            using namespace metal;
            inline uint rotr(uint x, uint n) { return (x >> n) | (x << (32 - n)); }
            constant uint K[64] = {
                0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
                0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
                0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
                0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
                0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
                0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
                0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
                0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
            };
            constant uint H0[8] = {
                0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,
                0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19
            };
            kernel void sha256_pairs_reduce(device const uchar* digests [[buffer(0)]],
                                            device uint* out_states [[buffer(1)]],
                                            uint id [[thread_position_in_grid]]) {
                const device uchar* left = digests + (id * 64);
                const device uchar* right = left + 32;
                uint w[64];
                for (uint t = 0; t < 16; ++t) {
                    const device uchar* p = (t < 8) ? (left + t * 4) : (right + (t - 8) * 4);
                    w[t] = (uint(p[0]) << 24) | (uint(p[1]) << 16) | (uint(p[2]) << 8) | uint(p[3]);
                }
                for (uint t = 16; t < 64; ++t) {
                    uint s0 = rotr(w[t-15],7) ^ rotr(w[t-15],18) ^ (w[t-15] >> 3);
                    uint s1 = rotr(w[t-2],17) ^ rotr(w[t-2],19) ^ (w[t-2] >> 10);
                    w[t] = w[t-16] + s0 + w[t-7] + s1;
                }
                uint a=H0[0], b=H0[1], c=H0[2], d=H0[3], e=H0[4], f=H0[5], g=H0[6], h=H0[7];
                for (uint t = 0; t < 64; ++t) {
                    uint s1 = rotr(e,6) ^ rotr(e,11) ^ rotr(e,25);
                    uint ch = (e & f) ^ ((~e) & g);
                    uint temp1 = h + s1 + ch + K[t] + w[t];
                    uint s0 = rotr(a,2) ^ rotr(a,13) ^ rotr(a,22);
                    uint maj = (a & b) ^ (a & c) ^ (b & c);
                    uint temp2 = s0 + maj;
                    h = g; g = f; f = e; e = d + temp1; d = c; c = b; b = a; a = temp1 + temp2;
                }
                uint st0 = H0[0] + a;
                uint st1 = H0[1] + b;
                uint st2 = H0[2] + c;
                uint st3 = H0[3] + d;
                uint st4 = H0[4] + e;
                uint st5 = H0[5] + f;
                uint st6 = H0[6] + g;
                uint st7 = H0[7] + h;

                uint wp[64];
                wp[0] = 0x80000000u;
                for (uint i = 1; i < 15; ++i) {
                    wp[i] = 0;
                }
                wp[15] = 512u;
                for (uint t = 16; t < 64; ++t) {
                    uint s0 = rotr(wp[t-15],7) ^ rotr(wp[t-15],18) ^ (wp[t-15] >> 3);
                    uint s1 = rotr(wp[t-2],17) ^ rotr(wp[t-2],19) ^ (wp[t-2] >> 10);
                    wp[t] = wp[t-16] + s0 + wp[t-7] + s1;
                }
                a = st0; b = st1; c = st2; d = st3; e = st4; f = st5; g = st6; h = st7;
                for (uint t = 0; t < 64; ++t) {
                    uint s1 = rotr(e,6) ^ rotr(e,11) ^ rotr(e,25);
                    uint ch = (e & f) ^ ((~e) & g);
                    uint temp1 = h + s1 + ch + K[t] + wp[t];
                    uint s0 = rotr(a,2) ^ rotr(a,13) ^ rotr(a,22);
                    uint maj = (a & b) ^ (a & c) ^ (b & c);
                    uint temp2 = s0 + maj;
                    h = g; g = f; f = e; e = d + temp1; d = c; c = b; b = a; a = temp1 + temp2;
                }
                uint out0 = st0 + a;
                uint out1 = st1 + b;
                uint out2 = st2 + c;
                uint out3 = st3 + d;
                uint out4 = st4 + e;
                uint out5 = st5 + f;
                uint out6 = st6 + g;
                uint out7 = st7 + h;
                device uint* dst = out_states + (id * 8);
                dst[0] = out0; dst[1] = out1; dst[2] = out2; dst[3] = out3;
                dst[4] = out4; dst[5] = out5; dst[6] = out6; dst[7] = out7;
            }
        "#;
        let lib = device
            .newLibraryWithSource_options_error(&NSString::from_str(src), None)
            .ok()?;
        let func = lib.newFunctionWithName(ns_string!("sha256_pairs_reduce"))?;
        let sha256_pairs = device
            .newComputePipelineStateWithFunction_error(&func)
            .ok()?;

        let src = r#"
            #include <metal_stdlib>
            using namespace metal;
            inline ulong rotl64(ulong x, uint n) { return (x << n) | (x >> (64 - n)); }
            constant ulong RC[24] = {
                0x0000000000000001ull, 0x0000000000008082ull, 0x800000000000808aull, 0x8000000080008000ull,
                0x000000000000808bull, 0x0000000080000001ull, 0x8000000080008081ull, 0x8000000000008009ull,
                0x000000000000008aull, 0x0000000000000088ull, 0x0000000080008009ull, 0x000000008000000aull,
                0x000000008000808bull, 0x800000000000008bull, 0x8000000000008089ull, 0x8000000000008003ull,
                0x8000000000008002ull, 0x8000000000000080ull, 0x000000000000800aull, 0x800000008000000aull,
                0x8000000080008081ull, 0x8000000000008080ull, 0x0000000080000001ull, 0x8000000080008008ull
            };
            constant ushort ROT[5][5] = {
                {0, 36, 3, 41, 18},
                {1, 44, 10, 45, 2},
                {62, 6, 43, 15, 61},
                {28, 55, 25, 21, 56},
                {27, 20, 39, 8, 14}
            };
            kernel void keccak_f1600(device ulong* state [[buffer(0)]], uint tid [[thread_position_in_grid]]) {
                if (tid != 0) { return; }
                ulong a[25];
                for (uint i = 0; i < 25; ++i) { a[i] = state[i]; }
                for (uint round = 0; round < 24; ++round) {
                    ulong c[5];
                    for (uint x = 0; x < 5; ++x) {
                        c[x] = a[x] ^ a[x + 5] ^ a[x + 10] ^ a[x + 15] ^ a[x + 20];
                    }
                    ulong d[5];
                    for (uint x = 0; x < 5; ++x) {
                        d[x] = c[(x + 4) % 5] ^ rotl64(c[(x + 1) % 5], 1);
                    }
                    for (uint x = 0; x < 5; ++x) {
                        for (uint y = 0; y < 5; ++y) {
                            a[x + 5 * y] ^= d[x];
                        }
                    }
                    ulong b[25];
                    for (uint x = 0; x < 5; ++x) {
                        for (uint y = 0; y < 5; ++y) {
                            uint rot = ROT[x][y];
                            ulong val = a[x + 5 * y];
                            ulong rotated = rot ? rotl64(val, rot) : val;
                            uint new_x = y;
                            uint new_y = (2 * x + 3 * y) % 5;
                            b[new_x + 5 * new_y] = rotated;
                        }
                    }
                    for (uint y = 0; y < 5; ++y) {
                        for (uint x = 0; x < 5; ++x) {
                            ulong current = b[x + 5 * y];
                            ulong next1 = b[((x + 1) % 5) + 5 * y];
                            ulong next2 = b[((x + 2) % 5) + 5 * y];
                            a[x + 5 * y] = current ^ ((~next1) & next2);
                        }
                    }
                    a[0] ^= RC[round];
                }
                for (uint i = 0; i < 25; ++i) {
                    state[i] = a[i];
                }
            }
        "#;
        let lib = device
            .newLibraryWithSource_options_error(&NSString::from_str(src), None)
            .ok()?;
        let func = lib.newFunctionWithName(ns_string!("keccak_f1600"))?;
        let keccak = device
            .newComputePipelineStateWithFunction_error(&func)
            .ok()?;

        // AES round kernels (enc/dec)
        let src = r#"
            #include <metal_stdlib>
            using namespace metal;
            constant uchar SBOX[256] = {
                0x63,0x7c,0x77,0x7b,0xf2,0x6b,0x6f,0xc5,0x30,0x01,0x67,0x2b,0xfe,0xd7,0xab,0x76,
                0xca,0x82,0xc9,0x7d,0xfa,0x59,0x47,0xf0,0xad,0xd4,0xa2,0xaf,0x9c,0xa4,0x72,0xc0,
                0xb7,0xfd,0x93,0x26,0x36,0x3f,0xf7,0xcc,0x34,0xa5,0xe5,0xf1,0x71,0xd8,0x31,0x15,
                0x04,0xc7,0x23,0xc3,0x18,0x96,0x05,0x9a,0x07,0x12,0x80,0xe2,0xeb,0x27,0xb2,0x75,
                0x09,0x83,0x2c,0x1a,0x1b,0x6e,0x5a,0xa0,0x52,0x3b,0xd6,0xb3,0x29,0xe3,0x2f,0x84,
                0x53,0xd1,0x00,0xed,0x20,0xfc,0xb1,0x5b,0x6a,0xcb,0xbe,0x39,0x4a,0x4c,0x58,0xcf,
                0xd0,0xef,0xaa,0xfb,0x43,0x4d,0x33,0x85,0x45,0xf9,0x02,0x7f,0x50,0x3c,0x9f,0xa8,
                0x51,0xa3,0x40,0x8f,0x92,0x9d,0x38,0xf5,0xbc,0xb6,0xda,0x21,0x10,0xff,0xf3,0xd2,
                0xcd,0x0c,0x13,0xec,0x5f,0x97,0x44,0x17,0xc4,0xa7,0x7e,0x3d,0x64,0x5d,0x19,0x73,
                0x60,0x81,0x4f,0xdc,0x22,0x2a,0x90,0x88,0x46,0xee,0xb8,0x14,0xde,0x5e,0x0b,0xdb,
                0xe0,0x32,0x3a,0x0a,0x49,0x06,0x24,0x5c,0xc2,0xd3,0xac,0x62,0x91,0x95,0xe4,0x79,
                0xe7,0xc8,0x37,0x6d,0x8d,0xd5,0x4e,0xa9,0x6c,0x56,0xf4,0xea,0x65,0x7a,0xae,0x08,
                0xba,0x78,0x25,0x2e,0x1c,0xa6,0xb4,0xc6,0xe8,0xdd,0x74,0x1f,0x4b,0xbd,0x8b,0x8a,
                0x70,0x3e,0xb5,0x66,0x48,0x03,0xf6,0x0e,0x61,0x35,0x57,0xb9,0x86,0xc1,0x1d,0x9e,
                0xe1,0xf8,0x98,0x11,0x69,0xd9,0x8e,0x94,0x9b,0x1e,0x87,0xe9,0xce,0x55,0x28,0xdf,
                0x8c,0xa1,0x89,0x0d,0xbf,0xe6,0x42,0x68,0x41,0x99,0x2d,0x0f,0xb0,0x54,0xbb,0x16
            };
            inline uchar gmul(uchar a, uchar b) {
                uchar res = 0;
                for (int i = 0; i < 8; ++i) {
                    if (b & 1) res ^= a;
                    uchar hi = a & 0x80; a <<= 1; if (hi) a ^= 0x1B; b >>= 1;
                }
                return res;
            }
            kernel void aesenc_round(device const uchar* state [[buffer(0)]],
                                    device const uchar* rk [[buffer(1)]],
                                    device uchar* out [[buffer(2)]],
                                    uint id [[thread_position_in_grid]]) {
                uchar s[16];
                for (int i=0;i<16;++i) s[i]=state[i];
                // SubBytes
                for (int i=0;i<16;++i) s[i] = SBOX[s[i]];
                // ShiftRows
                uchar t[16]; for (int i=0;i<16;++i) t[i]=s[i];
                s[1]=t[5]; s[5]=t[9]; s[9]=t[13]; s[13]=t[1];
                s[2]=t[10]; s[6]=t[14]; s[10]=t[2]; s[14]=t[6];
                s[3]=t[15]; s[7]=t[3]; s[11]=t[7]; s[15]=t[11];
                // MixColumns
                for (int c=0;c<4;++c){ int i=c*4; uchar a0=s[i],a1=s[i+1],a2=s[i+2],a3=s[i+3];
                    s[i]=gmul(a0,2)^gmul(a1,3)^a2^a3;
                    s[i+1]=a0^gmul(a1,2)^gmul(a2,3)^a3;
                    s[i+2]=a0^a1^gmul(a2,2)^gmul(a3,3);
                    s[i+3]=gmul(a0,3)^a1^a2^gmul(a3,2);
                }
                // AddRoundKey
                for (int i=0;i<16;++i) out[i] = s[i] ^ rk[i];
            }

            kernel void aesdec_round(device const uchar* state [[buffer(0)]],
                                    device const uchar* rk [[buffer(1)]],
                                    device uchar* out [[buffer(2)]],
                                    uint id [[thread_position_in_grid]]) {
                uchar s[16]; for (int i=0;i<16;++i) s[i]=state[i];
                // AddRoundKey
                for (int i=0;i<16;++i) s[i] ^= rk[i];
                // InvMixColumns
                for (int c=0;c<4;++c){ int i=c*4; uchar a0=s[i],a1=s[i+1],a2=s[i+2],a3=s[i+3];
                    s[i]=gmul(a0,14)^gmul(a1,11)^gmul(a2,13)^gmul(a3,9);
                    s[i+1]=gmul(a0,9)^gmul(a1,14)^gmul(a2,11)^gmul(a3,13);
                    s[i+2]=gmul(a0,13)^gmul(a1,9)^gmul(a2,14)^gmul(a3,11);
                    s[i+3]=gmul(a0,11)^gmul(a1,13)^gmul(a2,9)^gmul(a3,14);
                }
                // InvShiftRows
                uchar t[16]; for (int i=0;i<16;++i) t[i]=s[i];
                s[5]=t[1]; s[9]=t[5]; s[13]=t[9]; s[1]=t[13];
                s[10]=t[2]; s[14]=t[6]; s[2]=t[10]; s[6]=t[14];
                s[15]=t[3]; s[3]=t[7]; s[7]=t[11]; s[11]=t[15];
                // InvSubBytes
                // For brevity, reuse SBOX inverse would be needed; here we reconstruct via search.
                for (int i=0;i<16;++i){ uchar v=s[i];
                    for (int j=0;j<256;++j){ if (SBOX[j]==v){ s[i]=(uchar)j; break; } }
                }
                for (int i=0;i<16;++i) out[i]=s[i];
            }
            kernel void aesenc_round_batch(device const uchar* states [[buffer(0)]],
                                           device const uchar* rk [[buffer(1)]],
                                           device uchar* out [[buffer(2)]],
                                           uint id [[thread_position_in_grid]]) {
                const device uchar* state = states + id * 16;
                device uchar* dst = out + id * 16;
                uchar s[16]; for (int i=0;i<16;++i) s[i]=state[i];
                for (int i=0;i<16;++i) s[i] = SBOX[s[i]];
                uchar t[16]; for (int i=0;i<16;++i) t[i]=s[i];
                s[1]=t[5]; s[5]=t[9]; s[9]=t[13]; s[13]=t[1];
                s[2]=t[10]; s[6]=t[14]; s[10]=t[2]; s[14]=t[6];
                s[3]=t[15]; s[7]=t[3]; s[11]=t[7]; s[15]=t[11];
                for (int c=0;c<4;++c){ int i=c*4; uchar a0=s[i],a1=s[i+1],a2=s[i+2],a3=s[i+3];
                    s[i]=gmul(a0,2)^gmul(a1,3)^a2^a3;
                    s[i+1]=a0^gmul(a1,2)^gmul(a2,3)^a3;
                    s[i+2]=a0^a1^gmul(a2,2)^gmul(a3,3);
                    s[i+3]=gmul(a0,3)^a1^a2^gmul(a3,2);
                }
                for (int i=0;i<16;++i) dst[i] = s[i] ^ rk[i];
            }
            kernel void aesdec_round_batch(device const uchar* states [[buffer(0)]],
                                           device const uchar* rk [[buffer(1)]],
                                           device uchar* out [[buffer(2)]],
                                           uint id [[thread_position_in_grid]]) {
                const device uchar* state = states + id * 16;
                device uchar* dst = out + id * 16;
                uchar s[16]; for (int i=0;i<16;++i) s[i]=state[i];
                for (int i=0;i<16;++i) s[i] ^= rk[i];
                for (int c=0;c<4;++c){ int i=c*4; uchar a0=s[i],a1=s[i+1],a2=s[i+2],a3=s[i+3];
                    s[i]=gmul(a0,14)^gmul(a1,11)^gmul(a2,13)^gmul(a3,9);
                    s[i+1]=gmul(a0,9)^gmul(a1,14)^gmul(a2,11)^gmul(a3,13);
                    s[i+2]=gmul(a0,13)^gmul(a1,9)^gmul(a2,14)^gmul(a3,11);
                    s[i+3]=gmul(a0,11)^gmul(a1,13)^gmul(a2,9)^gmul(a3,14);
                }
                uchar t[16]; for (int i=0;i<16;++i) t[i]=s[i];
                s[5]=t[1]; s[9]=t[5]; s[13]=t[9]; s[1]=t[13];
                s[10]=t[2]; s[14]=t[6]; s[2]=t[10]; s[6]=t[14];
                s[15]=t[3]; s[3]=t[7]; s[7]=t[11]; s[11]=t[15];
                for (int i=0;i<16;++i){ uchar v=s[i];
                    for (int j=0;j<256;++j){ if (SBOX[j]==v){ s[i]=(uchar)j; break; } }
                }
                for (int i=0;i<16;++i) dst[i]=s[i];
            }
            // Fused N-round batch: round_keys is a contiguous array of nrounds*16 bytes
            kernel void aesenc_rounds_batch(device const uchar* states [[buffer(0)]],
                                           device const uchar* round_keys [[buffer(1)]],
                                           constant uint& nrounds [[buffer(3)]],
                                           device uchar* out [[buffer(2)]],
                                           uint id [[thread_position_in_grid]]) {
                const device uchar* state = states + id * 16;
                device uchar* dst = out + id * 16;
                uchar s[16]; for (int i=0;i<16;++i) s[i]=state[i];
                for (uint r=0;r<nrounds;++r){
                    const device uchar* rk = round_keys + r * 16;
                    for (int i=0;i<16;++i) s[i] = SBOX[s[i]];
                    uchar t[16]; for (int i=0;i<16;++i) t[i]=s[i];
                    s[1]=t[5]; s[5]=t[9]; s[9]=t[13]; s[13]=t[1];
                    s[2]=t[10]; s[6]=t[14]; s[10]=t[2]; s[14]=t[6];
                    s[3]=t[15]; s[7]=t[3]; s[11]=t[7]; s[15]=t[11];
                    for (int c=0;c<4;++c){ int i=c*4; uchar a0=s[i],a1=s[i+1],a2=s[i+2],a3=s[i+3];
                        s[i]=gmul(a0,2)^gmul(a1,3)^a2^a3;
                        s[i+1]=a0^gmul(a1,2)^gmul(a2,3)^a3;
                        s[i+2]=a0^a1^gmul(a2,2)^gmul(a3,3);
                        s[i+3]=gmul(a0,3)^a1^a2^gmul(a3,2);
                    }
                    for (int i=0;i<16;++i) s[i] ^= rk[i];
                }
                for (int i=0;i<16;++i) dst[i]=s[i];
            }
            kernel void aesdec_rounds_batch(device const uchar* states [[buffer(0)]],
                                           device const uchar* round_keys [[buffer(1)]],
                                           constant uint& nrounds [[buffer(3)]],
                                           device uchar* out [[buffer(2)]],
                                           uint id [[thread_position_in_grid]]) {
                const device uchar* state = states + id * 16;
                device uchar* dst = out + id * 16;
                uchar s[16]; for (int i=0;i<16;++i) s[i]=state[i];
                for (uint r=0;r<nrounds;++r){
                    const device uchar* rk = round_keys + r * 16;
                    for (int i=0;i<16;++i) s[i] ^= rk[i];
                    for (int c=0;c<4;++c){ int i=c*4; uchar a0=s[i],a1=s[i+1],a2=s[i+2],a3=s[i+3];
                        s[i]=gmul(a0,14)^gmul(a1,11)^gmul(a2,13)^gmul(a3,9);
                        s[i+1]=gmul(a0,9)^gmul(a1,14)^gmul(a2,11)^gmul(a3,13);
                        s[i+2]=gmul(a0,13)^gmul(a1,9)^gmul(a2,14)^gmul(a3,11);
                        s[i+3]=gmul(a0,11)^gmul(a1,13)^gmul(a2,9)^gmul(a3,14);
                    }
                    uchar t[16]; for (int i=0;i<16;++i) t[i]=s[i];
                    s[5]=t[1]; s[9]=t[5]; s[13]=t[9]; s[1]=t[13];
                    s[10]=t[2]; s[14]=t[6]; s[2]=t[10]; s[6]=t[14];
                    s[15]=t[3]; s[3]=t[7]; s[7]=t[11]; s[11]=t[15];
                    for (int i=0;i<16;++i){ uchar v=s[i];
                        for (int j=0;j<256;++j){ if (SBOX[j]==v){ s[i]=(uchar)j; break; } }
                    }
                }
                for (int i=0;i<16;++i) dst[i]=s[i];
            }
        "#;
        let lib = device
            .newLibraryWithSource_options_error(&NSString::from_str(src), None)
            .ok()?;
        let func_e = lib.newFunctionWithName(ns_string!("aesenc_round"))?;
        let aesenc = device
            .newComputePipelineStateWithFunction_error(&func_e)
            .ok()?;
        let func_d = lib.newFunctionWithName(ns_string!("aesdec_round"))?;
        let aesdec = device
            .newComputePipelineStateWithFunction_error(&func_d)
            .ok()?;
        let func_eb = lib.newFunctionWithName(ns_string!("aesenc_round_batch"))?;
        let aesenc_batch = device
            .newComputePipelineStateWithFunction_error(&func_eb)
            .ok()?;
        let func_db = lib.newFunctionWithName(ns_string!("aesdec_round_batch"))?;
        let aesdec_batch = device
            .newComputePipelineStateWithFunction_error(&func_db)
            .ok()?;
        let func_ebr = lib.newFunctionWithName(ns_string!("aesenc_rounds_batch"))?;
        let aesenc_rounds = device
            .newComputePipelineStateWithFunction_error(&func_ebr)
            .ok()?;
        let func_dbr = lib.newFunctionWithName(ns_string!("aesdec_rounds_batch"))?;
        let aesdec_rounds = device
            .newComputePipelineStateWithFunction_error(&func_dbr)
            .ok()?;

        let mut ed25519_signature = {
            match device
                .newLibraryWithSource_options_error(&NSString::from_str(METAL_ED25519_SHADER), None)
            {
                Ok(lib) => match lib.newFunctionWithName(ns_string!("signature_kernel")) {
                    Some(func) => device.newComputePipelineStateWithFunction_error(&func).ok(),
                    None => None,
                },
                Err(_) => None,
            }
        };

        // Startup self-tests prove parity for required Metal pipelines. Optional
        // pipelines, such as Ed25519 batch verification, can fail closed
        // individually while the rest of the backend remains available.
        let force_fail = if crate::dev_env::dev_env_flag("IVM_FORCE_METAL_SELFTEST_FAIL") {
            std::env::var("IVM_FORCE_METAL_SELFTEST_FAIL")
                .map(|v| v == "1")
                .unwrap_or(false)
        } else {
            false
        };

        if force_fail {
            record_metal_disable("self-test failure forced via IVM_FORCE_METAL_SELFTEST_FAIL");
            return None;
        }

        let debug_selftest = if crate::dev_env::dev_env_flag("IVM_DEBUG_METAL_SELFTEST") {
            std::env::var("IVM_DEBUG_METAL_SELFTEST")
                .map(|v| matches!(v.trim(), "1" | "true" | "TRUE"))
                .unwrap_or(false)
        } else {
            false
        };

        let self_test_ok = {
            // vadd32 quick check
            let a = [1u32, 2, 3, 4];
            let b = [4u32, 3, 2, 1];
            let expect = [5u32, 5, 5, 5];
            let add_ok = {
                use core::ptr::NonNull;

                use objc2_metal::*;

                let buf_a = unsafe {
                    device.newBufferWithBytes_length_options(
                        NonNull::new_unchecked(a.as_ptr() as *mut core::ffi::c_void),
                        a.len() * core::mem::size_of::<u32>(),
                        MTLResourceOptions::CPUCacheModeDefaultCache,
                    )?
                };
                let buf_b = unsafe {
                    device.newBufferWithBytes_length_options(
                        NonNull::new_unchecked(b.as_ptr() as *mut core::ffi::c_void),
                        b.len() * core::mem::size_of::<u32>(),
                        MTLResourceOptions::CPUCacheModeDefaultCache,
                    )?
                };
                let buf_out = device.newBufferWithLength_options(
                    a.len() * core::mem::size_of::<u32>(),
                    MTLResourceOptions::StorageModeShared,
                )?;
                let cmd_buf = queue.commandBuffer()?;
                let encoder = cmd_buf.computeCommandEncoder()?;
                encoder.setComputePipelineState(&vadd32);
                unsafe {
                    encoder.setBuffer_offset_atIndex(Some(&buf_a), 0, 0);
                    encoder.setBuffer_offset_atIndex(Some(&buf_b), 0, 1);
                    encoder.setBuffer_offset_atIndex(Some(&buf_out), 0, 2);
                }
                let grid = MTLSize {
                    width: 1,
                    height: 1,
                    depth: 1,
                };
                let threads = MTLSize {
                    width: 1,
                    height: 1,
                    depth: 1,
                };
                encoder.dispatchThreads_threadsPerThreadgroup(grid, threads);
                encoder.endEncoding();
                cmd_buf.commit();
                if !finalize_command_buffer(&cmd_buf, "metal self-test vadd32") {
                    return None;
                }
                let ptr = buf_out.contents().as_ptr() as *const u32;
                let out_slice = unsafe { std::slice::from_raw_parts(ptr, 4) };
                out_slice == expect
            };
            if debug_selftest {
                eprintln!(
                    "ivm: metal self-test vadd32 {}",
                    if add_ok { "ok" } else { "FAIL" }
                );
            }

            let add64_ok = {
                use core::ptr::NonNull;

                use objc2_metal::*;

                let a = [
                    (0x0000_0000u64 << 32) | 0xffff_ffff,
                    (0x8000_0000u64 << 32) | 0x0000_0001,
                ];
                let b = [
                    (0x0000_0001u64 << 32) | 0x0000_0001,
                    (0x7fff_ffffu64 << 32) | 0xffff_ffff,
                ];
                let expect = [a[0].wrapping_add(b[0]), a[1].wrapping_add(b[1])];
                let buf_a = unsafe {
                    device.newBufferWithBytes_length_options(
                        NonNull::new_unchecked(a.as_ptr() as *mut core::ffi::c_void),
                        a.len() * core::mem::size_of::<u64>(),
                        MTLResourceOptions::CPUCacheModeDefaultCache,
                    )?
                };
                let buf_b = unsafe {
                    device.newBufferWithBytes_length_options(
                        NonNull::new_unchecked(b.as_ptr() as *mut core::ffi::c_void),
                        b.len() * core::mem::size_of::<u64>(),
                        MTLResourceOptions::CPUCacheModeDefaultCache,
                    )?
                };
                let buf_out = device.newBufferWithLength_options(
                    a.len() * core::mem::size_of::<u64>(),
                    MTLResourceOptions::StorageModeShared,
                )?;
                let cmd_buf = queue.commandBuffer()?;
                let encoder = cmd_buf.computeCommandEncoder()?;
                encoder.setComputePipelineState(&vadd64);
                unsafe {
                    encoder.setBuffer_offset_atIndex(Some(&buf_a), 0, 0);
                    encoder.setBuffer_offset_atIndex(Some(&buf_b), 0, 1);
                    encoder.setBuffer_offset_atIndex(Some(&buf_out), 0, 2);
                }
                let grid = MTLSize {
                    width: 1,
                    height: 1,
                    depth: 1,
                };
                let threads = MTLSize {
                    width: 1,
                    height: 1,
                    depth: 1,
                };
                encoder.dispatchThreads_threadsPerThreadgroup(grid, threads);
                encoder.endEncoding();
                cmd_buf.commit();
                if !finalize_command_buffer(&cmd_buf, "metal self-test vadd64") {
                    return None;
                }
                let ptr = buf_out.contents().as_ptr() as *const u64;
                let out_slice = unsafe { std::slice::from_raw_parts(ptr, 2) };
                out_slice == expect
            };
            if debug_selftest {
                eprintln!(
                    "ivm: metal self-test vadd64 {}",
                    if add64_ok { "ok" } else { "FAIL" }
                );
            }

            let bit_ops_ok = {
                use core::ptr::NonNull;

                use objc2_metal::*;

                let lhs = [0xffff_0000u32, 0x1234_5678, 0x0f0f_0f0f, 0xaaaa_5555];
                let rhs = [0x00ff_ff00u32, 0xf0f0_f0f0, 0x3333_cccc, 0x5555_aaaa];
                let run_bit = |pipeline, expect: [u32; 4], label: &str| -> Option<bool> {
                    let buf_lhs = unsafe {
                        device.newBufferWithBytes_length_options(
                            NonNull::new_unchecked(lhs.as_ptr() as *mut core::ffi::c_void),
                            lhs.len() * core::mem::size_of::<u32>(),
                            MTLResourceOptions::CPUCacheModeDefaultCache,
                        )?
                    };
                    let buf_rhs = unsafe {
                        device.newBufferWithBytes_length_options(
                            NonNull::new_unchecked(rhs.as_ptr() as *mut core::ffi::c_void),
                            rhs.len() * core::mem::size_of::<u32>(),
                            MTLResourceOptions::CPUCacheModeDefaultCache,
                        )?
                    };
                    let buf_out = device.newBufferWithLength_options(
                        lhs.len() * core::mem::size_of::<u32>(),
                        MTLResourceOptions::StorageModeShared,
                    )?;
                    let cmd_buf = queue.commandBuffer()?;
                    let encoder = cmd_buf.computeCommandEncoder()?;
                    encoder.setComputePipelineState(pipeline);
                    unsafe {
                        encoder.setBuffer_offset_atIndex(Some(&buf_lhs), 0, 0);
                        encoder.setBuffer_offset_atIndex(Some(&buf_rhs), 0, 1);
                        encoder.setBuffer_offset_atIndex(Some(&buf_out), 0, 2);
                    }
                    let grid = MTLSize {
                        width: 1,
                        height: 1,
                        depth: 1,
                    };
                    let threads = MTLSize {
                        width: 1,
                        height: 1,
                        depth: 1,
                    };
                    encoder.dispatchThreads_threadsPerThreadgroup(grid, threads);
                    encoder.endEncoding();
                    cmd_buf.commit();
                    if !finalize_command_buffer(&cmd_buf, label) {
                        return None;
                    }
                    let ptr = buf_out.contents().as_ptr() as *const u32;
                    let out_slice = unsafe { std::slice::from_raw_parts(ptr, 4) };
                    Some(out_slice == expect)
                };

                let and_ok = run_bit(
                    &vand,
                    [
                        lhs[0] & rhs[0],
                        lhs[1] & rhs[1],
                        lhs[2] & rhs[2],
                        lhs[3] & rhs[3],
                    ],
                    "metal self-test vand",
                )
                .unwrap_or(false);
                let xor_ok = run_bit(
                    &vxor,
                    [
                        lhs[0] ^ rhs[0],
                        lhs[1] ^ rhs[1],
                        lhs[2] ^ rhs[2],
                        lhs[3] ^ rhs[3],
                    ],
                    "metal self-test vxor",
                )
                .unwrap_or(false);
                let or_ok = run_bit(
                    &vor,
                    [
                        lhs[0] | rhs[0],
                        lhs[1] | rhs[1],
                        lhs[2] | rhs[2],
                        lhs[3] | rhs[3],
                    ],
                    "metal self-test vor",
                )
                .unwrap_or(false);
                and_ok && xor_ok && or_ok
            };
            if debug_selftest {
                eprintln!(
                    "ivm: metal self-test vector-bit {}",
                    if bit_ops_ok { "ok" } else { "FAIL" }
                );
            }

            // sha256 compress quick check against local scalar
            let sha_ok = {
                let mut st_scalar = [
                    0x6a09e667u32,
                    0xbb67ae85,
                    0x3c6ef372,
                    0xa54ff53a,
                    0x510e527f,
                    0x9b05688c,
                    0x1f83d9ab,
                    0x5be0cd19,
                ];
                let mut st_metal = st_scalar;
                let mut block = [0u8; 64];
                // "abc" padded
                block[0] = b'a';
                block[1] = b'b';
                block[2] = b'c';
                block[3] = 0x80;
                block[63] = 24;
                sha256_compress_scalar_ref(&mut st_scalar, &block);
                // Run through Metal pipeline
                use core::ptr::NonNull;

                use objc2_metal::*;
                let buf_state = unsafe {
                    device.newBufferWithBytes_length_options(
                        NonNull::new_unchecked(st_metal.as_ptr() as *mut core::ffi::c_void),
                        st_metal.len() * core::mem::size_of::<u32>(),
                        MTLResourceOptions::CPUCacheModeDefaultCache,
                    )?
                };
                let buf_block = unsafe {
                    device.newBufferWithBytes_length_options(
                        NonNull::new_unchecked(block.as_ptr() as *mut core::ffi::c_void),
                        block.len(),
                        MTLResourceOptions::CPUCacheModeDefaultCache,
                    )?
                };
                let cmd_buf = queue.commandBuffer()?;
                let encoder = cmd_buf.computeCommandEncoder()?;
                encoder.setComputePipelineState(&sha256);
                unsafe {
                    encoder.setBuffer_offset_atIndex(Some(&buf_state), 0, 0);
                    encoder.setBuffer_offset_atIndex(Some(&buf_block), 0, 1);
                }
                let grid = MTLSize {
                    width: 1,
                    height: 1,
                    depth: 1,
                };
                let threads = MTLSize {
                    width: 1,
                    height: 1,
                    depth: 1,
                };
                encoder.dispatchThreads_threadsPerThreadgroup(grid, threads);
                encoder.endEncoding();
                cmd_buf.commit();
                if !finalize_command_buffer(&cmd_buf, "metal self-test sha256") {
                    return None;
                }
                let ptr = buf_state.contents().as_ptr() as *const u32;
                let out_slice = unsafe { std::slice::from_raw_parts(ptr, 8) };
                st_metal.copy_from_slice(out_slice);
                st_metal == st_scalar
            };
            if debug_selftest {
                eprintln!(
                    "ivm: metal self-test sha256 {}",
                    if sha_ok { "ok" } else { "FAIL" }
                );
            }

            let sha_leaves_ok = {
                use core::ptr::NonNull;

                use objc2_metal::*;

                let mut block_a = [0u8; 64];
                block_a[0] = b'a';
                block_a[1] = b'b';
                block_a[2] = b'c';
                block_a[3] = 0x80;
                block_a[63] = 24;

                let mut block_b = [0u8; 64];
                block_b[0] = b'n';
                block_b[1] = b'o';
                block_b[2] = b'r';
                block_b[3] = b'i';
                block_b[4] = b't';
                block_b[5] = b'o';
                block_b[6] = 0x80;
                block_b[63] = 48;

                let blocks = [block_a, block_b];
                let expected: Vec<[u8; 32]> = blocks
                    .iter()
                    .map(|block| {
                        let mut state = [
                            0x6a09e667u32,
                            0xbb67ae85,
                            0x3c6ef372,
                            0xa54ff53a,
                            0x510e527f,
                            0x9b05688c,
                            0x1f83d9ab,
                            0x5be0cd19,
                        ];
                        sha256_compress_scalar_ref(&mut state, block);
                        let mut digest = [0u8; 32];
                        for (index, word) in state.iter().enumerate() {
                            digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
                        }
                        digest
                    })
                    .collect();

                let flat: Vec<u8> = blocks
                    .iter()
                    .flat_map(|block| block.iter())
                    .copied()
                    .collect();
                let buf_blocks = unsafe {
                    device.newBufferWithBytes_length_options(
                        NonNull::new_unchecked(flat.as_ptr() as *mut core::ffi::c_void),
                        flat.len(),
                        MTLResourceOptions::CPUCacheModeDefaultCache,
                    )?
                };
                let buf_out = device.newBufferWithLength_options(
                    blocks.len() * 8 * core::mem::size_of::<u32>(),
                    MTLResourceOptions::StorageModeShared,
                )?;
                let cmd_buf = queue.commandBuffer()?;
                let encoder = cmd_buf.computeCommandEncoder()?;
                encoder.setComputePipelineState(&sha256_leaves);
                unsafe {
                    encoder.setBuffer_offset_atIndex(Some(&buf_blocks), 0, 0);
                    encoder.setBuffer_offset_atIndex(Some(&buf_out), 0, 1);
                }
                let grid = MTLSize {
                    width: blocks.len() as NSUInteger,
                    height: 1,
                    depth: 1,
                };
                let threads = MTLSize {
                    width: 1,
                    height: 1,
                    depth: 1,
                };
                encoder.dispatchThreads_threadsPerThreadgroup(grid, threads);
                encoder.endEncoding();
                cmd_buf.commit();
                if !finalize_command_buffer(&cmd_buf, "metal self-test sha256 leaves") {
                    return None;
                }
                let words = unsafe {
                    std::slice::from_raw_parts(
                        buf_out.contents().as_ptr() as *const u32,
                        blocks.len() * 8,
                    )
                };
                let actual: Vec<[u8; 32]> = words
                    .chunks_exact(8)
                    .map(|chunk| {
                        let mut digest = [0u8; 32];
                        for (index, word) in chunk.iter().enumerate() {
                            digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
                        }
                        digest
                    })
                    .collect();
                actual == expected
            };
            if debug_selftest {
                eprintln!(
                    "ivm: metal self-test sha256-leaves {}",
                    if sha_leaves_ok { "ok" } else { "FAIL" }
                );
            }

            // AES round quick check against CPU reference
            let aes_ok = {
                use core::ptr::NonNull;

                use objc2_metal::*;
                let state = [
                    0x00u8, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                    0xdd, 0xee, 0xff,
                ];
                let rk = [
                    0x0f, 0x15, 0x71, 0xc9, 0x47, 0xd9, 0xe8, 0x59, 0x0c, 0xb7, 0xad, 0xd6, 0xaf,
                    0x7f, 0x67, 0x98,
                ];
                let cpu_enc = crate::aes::aesenc_impl(state, rk);
                let cpu_dec = crate::aes::aesdec_impl(cpu_enc, rk);
                // Run AESENC
                let buf_s = unsafe {
                    device.newBufferWithBytes_length_options(
                        NonNull::new_unchecked(state.as_ptr() as *mut core::ffi::c_void),
                        16,
                        MTLResourceOptions::CPUCacheModeDefaultCache,
                    )?
                };
                let buf_k = unsafe {
                    device.newBufferWithBytes_length_options(
                        NonNull::new_unchecked(rk.as_ptr() as *mut core::ffi::c_void),
                        16,
                        MTLResourceOptions::CPUCacheModeDefaultCache,
                    )?
                };
                let buf_out = device
                    .newBufferWithLength_options(16, MTLResourceOptions::StorageModeShared)?;
                let cmd_buf = queue.commandBuffer()?;
                let enc = cmd_buf.computeCommandEncoder()?;
                enc.setComputePipelineState(&aesenc);
                unsafe {
                    enc.setBuffer_offset_atIndex(Some(&buf_s), 0, 0);
                    enc.setBuffer_offset_atIndex(Some(&buf_k), 0, 1);
                    enc.setBuffer_offset_atIndex(Some(&buf_out), 0, 2);
                }
                let grid = MTLSize {
                    width: 1,
                    height: 1,
                    depth: 1,
                };
                let threads = MTLSize {
                    width: 1,
                    height: 1,
                    depth: 1,
                };
                enc.dispatchThreads_threadsPerThreadgroup(grid, threads);
                enc.endEncoding();
                cmd_buf.commit();
                if !finalize_command_buffer(&cmd_buf, "metal self-test aesenc") {
                    return None;
                }
                let mut enc_out = [0u8; 16];
                unsafe {
                    let ptr = buf_out.contents().as_ptr() as *const u8;
                    core::ptr::copy_nonoverlapping(ptr, enc_out.as_mut_ptr(), 16);
                }
                if enc_out != cpu_enc {
                    false
                } else {
                    // Run AESDEC on enc_out
                    let buf_s2 = unsafe {
                        device.newBufferWithBytes_length_options(
                            NonNull::new_unchecked(enc_out.as_ptr() as *mut core::ffi::c_void),
                            16,
                            MTLResourceOptions::CPUCacheModeDefaultCache,
                        )?
                    };
                    let buf_out2 = device
                        .newBufferWithLength_options(16, MTLResourceOptions::StorageModeShared)?;
                    let cmd_buf2 = queue.commandBuffer()?;
                    let dec = cmd_buf2.computeCommandEncoder()?;
                    dec.setComputePipelineState(&aesdec);
                    unsafe {
                        dec.setBuffer_offset_atIndex(Some(&buf_s2), 0, 0);
                        dec.setBuffer_offset_atIndex(Some(&buf_k), 0, 1);
                        dec.setBuffer_offset_atIndex(Some(&buf_out2), 0, 2);
                    }
                    dec.dispatchThreads_threadsPerThreadgroup(grid, threads);
                    dec.endEncoding();
                    cmd_buf2.commit();
                    if !finalize_command_buffer(&cmd_buf2, "metal self-test aesdec") {
                        return None;
                    }
                    let mut dec_out = [0u8; 16];
                    unsafe {
                        let ptr2 = buf_out2.contents().as_ptr() as *const u8;
                        core::ptr::copy_nonoverlapping(ptr2, dec_out.as_mut_ptr(), 16);
                    }
                    dec_out == cpu_dec
                }
            };
            if debug_selftest {
                eprintln!(
                    "ivm: metal self-test aes-round {}",
                    if aes_ok { "ok" } else { "FAIL" }
                );
            }

            let aes_batch_ok = {
                use core::ptr::NonNull;

                use objc2_metal::*;

                let states = [
                    [
                        0x00u8, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb,
                        0xcc, 0xdd, 0xee, 0xff,
                    ],
                    [
                        0xffu8, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x99, 0x88, 0x77, 0x66, 0x55, 0x44,
                        0x33, 0x22, 0x11, 0x00,
                    ],
                ];
                let rk = [
                    0x0f, 0x15, 0x71, 0xc9, 0x47, 0xd9, 0xe8, 0x59, 0x0c, 0xb7, 0xad, 0xd6, 0xaf,
                    0x7f, 0x67, 0x98,
                ];
                let flat_states: Vec<u8> = states
                    .iter()
                    .flat_map(|state| state.iter())
                    .copied()
                    .collect();
                let run_batch = |pipeline, expected: [[u8; 16]; 2], label: &str| -> Option<bool> {
                    let buf_states = unsafe {
                        device.newBufferWithBytes_length_options(
                            NonNull::new_unchecked(flat_states.as_ptr() as *mut core::ffi::c_void),
                            flat_states.len(),
                            MTLResourceOptions::CPUCacheModeDefaultCache,
                        )?
                    };
                    let buf_rk = unsafe {
                        device.newBufferWithBytes_length_options(
                            NonNull::new_unchecked(rk.as_ptr() as *mut core::ffi::c_void),
                            16,
                            MTLResourceOptions::CPUCacheModeDefaultCache,
                        )?
                    };
                    let buf_out = device.newBufferWithLength_options(
                        flat_states.len(),
                        MTLResourceOptions::StorageModeShared,
                    )?;
                    let cmd_buf = queue.commandBuffer()?;
                    let encoder = cmd_buf.computeCommandEncoder()?;
                    encoder.setComputePipelineState(pipeline);
                    unsafe {
                        encoder.setBuffer_offset_atIndex(Some(&buf_states), 0, 0);
                        encoder.setBuffer_offset_atIndex(Some(&buf_rk), 0, 1);
                        encoder.setBuffer_offset_atIndex(Some(&buf_out), 0, 2);
                    }
                    let grid = MTLSize {
                        width: states.len() as NSUInteger,
                        height: 1,
                        depth: 1,
                    };
                    let threads = MTLSize {
                        width: 1,
                        height: 1,
                        depth: 1,
                    };
                    encoder.dispatchThreads_threadsPerThreadgroup(grid, threads);
                    encoder.endEncoding();
                    cmd_buf.commit();
                    if !finalize_command_buffer(&cmd_buf, label) {
                        return None;
                    }
                    let ptr = buf_out.contents().as_ptr() as *const u8;
                    let out_slice = unsafe { std::slice::from_raw_parts(ptr, flat_states.len()) };
                    let actual: Vec<[u8; 16]> = out_slice
                        .chunks_exact(16)
                        .map(|chunk| {
                            let mut block = [0u8; 16];
                            block.copy_from_slice(chunk);
                            block
                        })
                        .collect();
                    Some(actual == expected)
                };

                let enc_expected = [
                    crate::aes::aesenc_impl(states[0], rk),
                    crate::aes::aesenc_impl(states[1], rk),
                ];
                let dec_expected = [
                    crate::aes::aesdec_impl(states[0], rk),
                    crate::aes::aesdec_impl(states[1], rk),
                ];
                run_batch(&aesenc_batch, enc_expected, "metal self-test aesenc batch")
                    .unwrap_or(false)
                    && run_batch(&aesdec_batch, dec_expected, "metal self-test aesdec batch")
                        .unwrap_or(false)
            };
            if debug_selftest {
                eprintln!(
                    "ivm: metal self-test aes-batch {}",
                    if aes_batch_ok { "ok" } else { "FAIL" }
                );
            }

            // AES fused two-round quick check against CPU streaming
            let aes_fused_ok = {
                use objc2_metal::*;
                let state = [
                    0x00u8, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                    0xdd, 0xee, 0xff,
                ];
                let rk1 = [
                    0x0f, 0x15, 0x71, 0xc9, 0x47, 0xd9, 0xe8, 0x59, 0x0c, 0xb7, 0xad, 0xd6, 0xaf,
                    0x7f, 0x67, 0x98,
                ];
                let mut rk2 = rk1;
                rk2[0] ^= 0xAA;
                rk2[1] ^= 0x55;
                let cpu_enc2 = {
                    let r1 = crate::aes::aesenc_impl(state, rk1);
                    crate::aes::aesenc_impl(r1, rk2)
                };
                let cpu_dec2 = {
                    let r1 = crate::aes::aesdec_impl(cpu_enc2, rk1);
                    crate::aes::aesdec_impl(r1, rk2)
                };
                use core::ptr::NonNull;
                let flat_states: [u8; 16] = state;
                let rks: [u8; 32] = {
                    let mut buf = [0u8; 32];
                    buf[..16].copy_from_slice(&rk1);
                    buf[16..].copy_from_slice(&rk2);
                    buf
                };
                let buf_states = unsafe {
                    device.newBufferWithBytes_length_options(
                        NonNull::new_unchecked(flat_states.as_ptr() as *mut core::ffi::c_void),
                        16,
                        MTLResourceOptions::CPUCacheModeDefaultCache,
                    )?
                };
                let buf_rks = unsafe {
                    device.newBufferWithBytes_length_options(
                        NonNull::new_unchecked(rks.as_ptr() as *mut core::ffi::c_void),
                        32,
                        MTLResourceOptions::CPUCacheModeDefaultCache,
                    )?
                };
                let n_buf = [2u32; 1];
                let buf_n = unsafe {
                    device.newBufferWithBytes_length_options(
                        NonNull::new_unchecked(n_buf.as_ptr() as *mut core::ffi::c_void),
                        core::mem::size_of::<u32>(),
                        MTLResourceOptions::CPUCacheModeDefaultCache,
                    )?
                };
                let buf_out = device
                    .newBufferWithLength_options(16, MTLResourceOptions::StorageModeShared)?;
                // ENC 2 rounds
                let cmd = queue.commandBuffer()?;
                let enc = cmd.computeCommandEncoder()?;
                enc.setComputePipelineState(&aesenc_rounds);
                unsafe {
                    enc.setBuffer_offset_atIndex(Some(&buf_states), 0, 0);
                    enc.setBuffer_offset_atIndex(Some(&buf_rks), 0, 1);
                    enc.setBuffer_offset_atIndex(Some(&buf_out), 0, 2);
                    enc.setBuffer_offset_atIndex(Some(&buf_n), 0, 3);
                }
                let grid = MTLSize {
                    width: 1,
                    height: 1,
                    depth: 1,
                };
                let threads = MTLSize {
                    width: 1,
                    height: 1,
                    depth: 1,
                };
                enc.dispatchThreads_threadsPerThreadgroup(grid, threads);
                enc.endEncoding();
                cmd.commit();
                if !finalize_command_buffer(&cmd, "metal self-test aesenc rounds") {
                    return None;
                }
                let mut enc2 = [0u8; 16];
                unsafe {
                    let ptr = buf_out.contents().as_ptr() as *const u8;
                    core::ptr::copy_nonoverlapping(ptr, enc2.as_mut_ptr(), 16);
                }
                if enc2 != cpu_enc2 {
                    false
                } else {
                    // DEC 2 rounds
                    let cmd2 = queue.commandBuffer()?;
                    let dec = cmd2.computeCommandEncoder()?;
                    dec.setComputePipelineState(&aesdec_rounds);
                    let buf_states2 = unsafe {
                        device.newBufferWithBytes_length_options(
                            NonNull::new_unchecked(enc2.as_ptr() as *mut core::ffi::c_void),
                            16,
                            MTLResourceOptions::CPUCacheModeDefaultCache,
                        )?
                    };
                    let buf_out2 = device
                        .newBufferWithLength_options(16, MTLResourceOptions::StorageModeShared)?;
                    unsafe {
                        dec.setBuffer_offset_atIndex(Some(&buf_states2), 0, 0);
                        dec.setBuffer_offset_atIndex(Some(&buf_rks), 0, 1);
                        dec.setBuffer_offset_atIndex(Some(&buf_out2), 0, 2);
                        dec.setBuffer_offset_atIndex(Some(&buf_n), 0, 3);
                    }
                    dec.dispatchThreads_threadsPerThreadgroup(grid, threads);
                    dec.endEncoding();
                    cmd2.commit();
                    if !finalize_command_buffer(&cmd2, "metal self-test aesdec rounds") {
                        return None;
                    }
                    let mut dec2 = [0u8; 16];
                    unsafe {
                        let ptr2 = buf_out2.contents().as_ptr() as *const u8;
                        core::ptr::copy_nonoverlapping(ptr2, dec2.as_mut_ptr(), 16);
                    }
                    dec2 == cpu_dec2
                }
            };
            if debug_selftest {
                eprintln!(
                    "ivm: metal self-test aes-fused {}",
                    if aes_fused_ok { "ok" } else { "FAIL" }
                );
            }

            let keccak_ok = {
                use core::ptr::NonNull;

                use objc2_metal::*;
                let mut init = [0u64; 25];
                for i in 0..25 {
                    init[i] = (i as u64) * 0x0101_0101_0101_0101u64;
                }
                let mut cpu_state = init;
                crate::sha3::keccak_f1600_impl(&mut cpu_state);
                let mut gpu_state = init;
                let buf_state = unsafe {
                    device.newBufferWithBytes_length_options(
                        NonNull::new_unchecked(gpu_state.as_mut_ptr() as *mut core::ffi::c_void),
                        gpu_state.len() * core::mem::size_of::<u64>(),
                        MTLResourceOptions::CPUCacheModeDefaultCache,
                    )?
                };
                let cmd = queue.commandBuffer()?;
                let enc = cmd.computeCommandEncoder()?;
                enc.setComputePipelineState(&keccak);
                unsafe {
                    enc.setBuffer_offset_atIndex(Some(&buf_state), 0, 0);
                }
                let grid = MTLSize {
                    width: 1,
                    height: 1,
                    depth: 1,
                };
                let threads = MTLSize {
                    width: 1,
                    height: 1,
                    depth: 1,
                };
                enc.dispatchThreads_threadsPerThreadgroup(grid, threads);
                enc.endEncoding();
                cmd.commit();
                if !finalize_command_buffer(&cmd, "metal self-test keccak") {
                    return None;
                }
                let ptr = buf_state.contents().as_ptr() as *const u64;
                let out = unsafe { std::slice::from_raw_parts(ptr, 25) };
                gpu_state.copy_from_slice(out);
                let ok = gpu_state == cpu_state;
                if debug_selftest && !ok {
                    eprintln!("ivm: metal self-test keccak mismatch:");
                    for i in 0..25 {
                        eprintln!(
                            "  lane {i:02}: cpu=0x{cpu:016x} metal=0x{gpu:016x}",
                            cpu = cpu_state[i],
                            gpu = gpu_state[i]
                        );
                    }
                }
                ok
            };
            if debug_selftest {
                eprintln!(
                    "ivm: metal self-test keccak {}",
                    if keccak_ok { "ok" } else { "FAIL" }
                );
            }

            let sha_pairs_ok = {
                use core::ptr::NonNull;

                use objc2_metal::*;

                fn cpu_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
                    let mut state = [
                        0x6a09e667u32,
                        0xbb67ae85,
                        0x3c6ef372,
                        0xa54ff53a,
                        0x510e527f,
                        0x9b05688c,
                        0x1f83d9ab,
                        0x5be0cd19,
                    ];
                    let mut block = [0u8; 64];
                    block[..32].copy_from_slice(left);
                    block[32..].copy_from_slice(right);
                    sha256_compress_scalar_ref(&mut state, &block);
                    let mut pad = [0u8; 64];
                    pad[0] = 0x80;
                    pad[62] = 0x02;
                    pad[63] = 0x00;
                    sha256_compress_scalar_ref(&mut state, &pad);
                    let mut out = [0u8; 32];
                    for (i, word) in state.iter().enumerate() {
                        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
                    }
                    out
                }

                let mut d0 = [0u8; 32];
                let mut d1 = [0u8; 32];
                let mut d2 = [0u8; 32];
                for i in 0..32 {
                    d0[i] = i as u8;
                    d1[i] = 0x40 + i as u8;
                    d2[i] = 0x80 + i as u8;
                }
                let digests = [d0, d1, d2];
                let first = cpu_pair(&digests[0], &digests[1]);
                let expected = cpu_pair(&first, &digests[2]);

                let mut cur = digests
                    .iter()
                    .flat_map(|d| d.iter())
                    .copied()
                    .collect::<Vec<u8>>();
                let mut count = digests.len();
                while count > 1 {
                    let pairs = count / 2;
                    let has_leftover = (count & 1) != 0;
                    let mut next = vec![0u8; (pairs + if has_leftover { 1 } else { 0 }) * 32];
                    if pairs > 0 {
                        let in_buf = unsafe {
                            device.newBufferWithBytes_length_options(
                                NonNull::new_unchecked(cur.as_ptr() as *mut core::ffi::c_void),
                                cur.len(),
                                MTLResourceOptions::CPUCacheModeDefaultCache,
                            )?
                        };
                        let out_buf = device.newBufferWithLength_options(
                            pairs * 8 * core::mem::size_of::<u32>(),
                            MTLResourceOptions::StorageModeShared,
                        )?;
                        let cmd = queue.commandBuffer()?;
                        let enc = cmd.computeCommandEncoder()?;
                        enc.setComputePipelineState(&sha256_pairs);
                        unsafe {
                            enc.setBuffer_offset_atIndex(Some(&in_buf), 0, 0);
                            enc.setBuffer_offset_atIndex(Some(&out_buf), 0, 1);
                        }
                        let grid = MTLSize {
                            width: pairs as NSUInteger,
                            height: 1,
                            depth: 1,
                        };
                        let threads = MTLSize {
                            width: 1,
                            height: 1,
                            depth: 1,
                        };
                        enc.dispatchThreads_threadsPerThreadgroup(grid, threads);
                        enc.endEncoding();
                        cmd.commit();
                        if !finalize_command_buffer(&cmd, "metal self-test sha256 pairs") {
                            return None;
                        }
                        let words = unsafe {
                            std::slice::from_raw_parts(
                                out_buf.contents().as_ptr() as *const u32,
                                pairs * 8,
                            )
                        };
                        for pair in 0..pairs {
                            for j in 0..8 {
                                let word = words[pair * 8 + j];
                                next[pair * 32 + j * 4..pair * 32 + j * 4 + 4]
                                    .copy_from_slice(&word.to_be_bytes());
                            }
                        }
                    }
                    if has_leftover {
                        let src_idx = (count - 1) * 32;
                        let dst_idx = pairs * 32;
                        next[dst_idx..dst_idx + 32].copy_from_slice(&cur[src_idx..src_idx + 32]);
                    }
                    cur = next;
                    count = cur.len() / 32;
                }
                cur[..32] == expected
            };
            if debug_selftest {
                eprintln!(
                    "ivm: metal self-test sha256-pairs {}",
                    if sha_pairs_ok { "ok" } else { "FAIL" }
                );
            }

            let ed25519_ok = {
                use core::ptr::NonNull;

                use ed25519_dalek::{Signer, SigningKey};

                let key = SigningKey::from_bytes(&[7u8; 32]);
                let pk = key.verifying_key();
                let msg = b"ivm-metal-ed25519";
                let sig = key.sign(msg).to_bytes();
                let hram =
                    crate::signature::ed25519_challenge_scalar_bytes(&sig, pk.as_bytes(), msg);

                let mut sig_bad = sig;
                sig_bad[0] ^= 0x80;
                let sigs = [sig, sig_bad];
                let pks = [pk.to_bytes(), pk.to_bytes()];
                let hrams = [hram, hram];

                let flat_sigs: Vec<u8> = sigs.iter().flat_map(|s| s.iter()).copied().collect();
                let flat_pks: Vec<u8> = pks.iter().flat_map(|p| p.iter()).copied().collect();
                let flat_hrams: Vec<u8> = hrams.iter().flat_map(|h| h.iter()).copied().collect();
                let count_buf = [sigs.len() as u32];

                let buf_sigs = unsafe {
                    device.newBufferWithBytes_length_options(
                        NonNull::new_unchecked(flat_sigs.as_ptr() as *mut core::ffi::c_void),
                        flat_sigs.len(),
                        MTLResourceOptions::CPUCacheModeDefaultCache,
                    )?
                };
                let buf_pks = unsafe {
                    device.newBufferWithBytes_length_options(
                        NonNull::new_unchecked(flat_pks.as_ptr() as *mut core::ffi::c_void),
                        flat_pks.len(),
                        MTLResourceOptions::CPUCacheModeDefaultCache,
                    )?
                };
                let buf_hrams = unsafe {
                    device.newBufferWithBytes_length_options(
                        NonNull::new_unchecked(flat_hrams.as_ptr() as *mut core::ffi::c_void),
                        flat_hrams.len(),
                        MTLResourceOptions::CPUCacheModeDefaultCache,
                    )?
                };
                let buf_count = unsafe {
                    device.newBufferWithBytes_length_options(
                        NonNull::new_unchecked(count_buf.as_ptr() as *mut core::ffi::c_void),
                        core::mem::size_of::<u32>(),
                        MTLResourceOptions::CPUCacheModeDefaultCache,
                    )?
                };
                let buf_out = device.newBufferWithLength_options(
                    sigs.len(),
                    MTLResourceOptions::StorageModeShared,
                )?;

                if let Some(ref pipeline) = ed25519_signature {
                    let cmd = queue.commandBuffer()?;
                    let enc = cmd.computeCommandEncoder()?;
                    enc.setComputePipelineState(pipeline);
                    unsafe {
                        enc.setBuffer_offset_atIndex(Some(&buf_sigs), 0, 0);
                        enc.setBuffer_offset_atIndex(Some(&buf_pks), 0, 1);
                        enc.setBuffer_offset_atIndex(Some(&buf_hrams), 0, 2);
                        enc.setBuffer_offset_atIndex(Some(&buf_count), 0, 3);
                        enc.setBuffer_offset_atIndex(Some(&buf_out), 0, 4);
                    }
                    let grid = MTLSize {
                        width: sigs.len() as NSUInteger,
                        height: 1,
                        depth: 1,
                    };
                    let threads = MTLSize {
                        width: pipeline.threadExecutionWidth(),
                        height: 1,
                        depth: 1,
                    };
                    enc.dispatchThreads_threadsPerThreadgroup(grid, threads);
                    enc.endEncoding();
                    cmd.commit();
                    if !finalize_command_buffer(&cmd, "metal self-test ed25519 batch") {
                        return None;
                    }
                    let out = unsafe {
                        std::slice::from_raw_parts(
                            buf_out.contents().as_ptr() as *const u8,
                            sigs.len(),
                        )
                    };
                    out == [1u8, 0u8]
                } else {
                    true
                }
            };
            if debug_selftest {
                eprintln!(
                    "ivm: metal self-test ed25519 {}",
                    if ed25519_ok { "ok" } else { "FAIL" }
                );
            }
            if !ed25519_ok {
                eprintln!(
                    "ivm: metal signature kernel self-test failed; disabling only the ed25519 Metal pipeline"
                );
                ed25519_signature = None;
                set_metal_status_message(Some(
                    "metal ed25519 signature kernel self-test mismatch; CPU fallback remains active"
                        .to_owned(),
                ));
            }

            add_ok
                && add64_ok
                && bit_ops_ok
                && sha_ok
                && sha_leaves_ok
                && aes_ok
                && aes_batch_ok
                && aes_fused_ok
                && keccak_ok
                && sha_pairs_ok
        };

        if !self_test_ok {
            if debug_selftest {
                eprintln!("ivm: metal self-test overall result = FAIL");
            }
            #[cfg(target_os = "macos")]
            {
                record_metal_disable("golden-vector self-test mismatch");
            }
            return None;
        } else if debug_selftest {
            eprintln!("ivm: metal self-test overall result = ok");
        }

        Some(Self {
            device,
            queue,
            vadd64,
            vadd32,
            vand,
            vxor,
            vor,
            sha256,
            sha256_leaves,
            sha256_pairs,
            aesenc,
            aesdec,
            aesenc_batch,
            aesdec_batch,
            aesenc_rounds,
            aesdec_rounds,
            keccak,
            ed25519_signature,
            // Store fused pipelines in place of single-round ones for future extension if needed
            // (keeping single-round as well)
        })
    }
}

#[cfg(all(target_os = "macos", feature = "metal"))]
thread_local! {
    static METAL_STATE: std::cell::RefCell<OnceCell<MetalState>> = std::cell::RefCell::new(OnceCell::new());
}

#[cfg(all(target_os = "macos", feature = "metal"))]
fn with_metal_state<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&MetalState) -> R,
{
    if !metal_runtime_allowed() {
        return None;
    }
    METAL_STATE.with(|cell| {
        // Fast path: already initialized
        if let Some(state) = cell.borrow().get() {
            return Some(f(state));
        }

        // Slow path: try to initialize
        let state_new = MetalState::new()?;
        {
            let cell_mut = cell.borrow_mut();
            // It's okay if another thread initialized while we computed
            let _ = cell_mut.set(state_new);
            if let Some(state) = cell_mut.get() {
                return Some(f(state));
            }
        }
        None
    })
}

#[cfg(all(target_os = "macos", feature = "metal"))]
fn with_metal_state_try<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&MetalState) -> Option<R>,
{
    with_metal_state(|state| f(state)).and_then(|result| result)
}

#[cfg(all(target_os = "macos", feature = "metal"))]
fn finalize_command_buffer(
    cmd_buf: &ProtocolObject<dyn objc2_metal::MTLCommandBuffer>,
    context: &str,
) -> bool {
    #[cfg(all(target_os = "macos", feature = "metal"))]
    const MTL_COMMAND_BUFFER_STATUS_COMPLETED: NSUInteger = 4;

    let status_raw: NSUInteger = unsafe {
        cmd_buf.waitUntilCompleted();
        transmute(cmd_buf.status())
    };
    if status_raw == MTL_COMMAND_BUFFER_STATUS_COMPLETED {
        true
    } else {
        record_metal_disable(format!(
            "{context} command buffer exited with status {status_raw:?}"
        ));
        false
    }
}

#[cfg(all(target_os = "macos", feature = "metal"))]
#[allow(dead_code)]
pub fn release_metal_state() {
    METAL_STATE.with(|cell| {
        let _ = cell.borrow_mut().take();
    });
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
pub fn release_metal_state() {}

#[cfg(all(target_os = "macos", feature = "metal"))]
#[allow(dead_code)]
fn metal_vadd64(a: [u32; 4], b: [u32; 4]) -> Option<[u32; 4]> {
    if !metal_runtime_allowed() {
        return None;
    }
    use core::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_metal::*;

    autoreleasepool(|_| {
        with_metal_state(|ctx| {
            let mut in_a = [0u64; 2];
            let mut in_b = [0u64; 2];
            in_a[0] = (a[0] as u64) | ((a[1] as u64) << 32);
            in_a[1] = (a[2] as u64) | ((a[3] as u64) << 32);
            in_b[0] = (b[0] as u64) | ((b[1] as u64) << 32);
            in_b[1] = (b[2] as u64) | ((b[3] as u64) << 32);

            let buf_a = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(in_a.as_ptr() as *mut core::ffi::c_void),
                    in_a.len() * core::mem::size_of::<u64>(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )?
            };
            let buf_b = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(in_b.as_ptr() as *mut core::ffi::c_void),
                    in_b.len() * core::mem::size_of::<u64>(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )?
            };
            let buf_out = ctx.device.newBufferWithLength_options(
                in_a.len() * core::mem::size_of::<u64>(),
                MTLResourceOptions::StorageModeShared,
            )?;
            let cmd_buf = ctx.queue.commandBuffer()?;
            let encoder = cmd_buf.computeCommandEncoder()?;
            encoder.setComputePipelineState(&ctx.vadd64);
            unsafe {
                encoder.setBuffer_offset_atIndex(Some(&buf_a), 0, 0);
                encoder.setBuffer_offset_atIndex(Some(&buf_b), 0, 1);
                encoder.setBuffer_offset_atIndex(Some(&buf_out), 0, 2);
            }
            let grid = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            let threads = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            encoder.dispatchThreads_threadsPerThreadgroup(grid, threads);
            encoder.endEncoding();
            cmd_buf.commit();
            if !finalize_command_buffer(&cmd_buf, "metal vadd64") {
                return None;
            }
            let ptr = buf_out.contents().as_ptr() as *const u64;
            let out_slice = unsafe { std::slice::from_raw_parts(ptr, 2) };
            let r0 = out_slice[0];
            let r1 = out_slice[1];
            Some([
                (r0 & 0xffff_ffff) as u32,
                (r0 >> 32) as u32,
                (r1 & 0xffff_ffff) as u32,
                (r1 >> 32) as u32,
            ])
        })?
    })
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
fn metal_vadd64(_a: [u32; 4], _b: [u32; 4]) -> Option<[u32; 4]> {
    None
}

#[cfg(all(target_os = "macos", feature = "metal"))]
#[allow(dead_code)]
fn metal_vadd32(a: [u32; 4], b: [u32; 4]) -> Option<[u32; 4]> {
    if !metal_runtime_allowed() {
        return None;
    }
    use core::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_metal::*;

    autoreleasepool(|_| {
        with_metal_state(|ctx| {
            let buf_a = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(a.as_ptr() as *mut core::ffi::c_void),
                    a.len() * core::mem::size_of::<u32>(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )?
            };
            let buf_b = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(b.as_ptr() as *mut core::ffi::c_void),
                    b.len() * core::mem::size_of::<u32>(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )?
            };
            let buf_out = ctx.device.newBufferWithLength_options(
                a.len() * core::mem::size_of::<u32>(),
                MTLResourceOptions::StorageModeShared,
            )?;
            let cmd_buf = ctx.queue.commandBuffer()?;
            let encoder = cmd_buf.computeCommandEncoder()?;
            encoder.setComputePipelineState(&ctx.vadd32);
            unsafe {
                encoder.setBuffer_offset_atIndex(Some(&buf_a), 0, 0);
                encoder.setBuffer_offset_atIndex(Some(&buf_b), 0, 1);
                encoder.setBuffer_offset_atIndex(Some(&buf_out), 0, 2);
            }
            let grid = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            let threads = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            encoder.dispatchThreads_threadsPerThreadgroup(grid, threads);
            encoder.endEncoding();
            cmd_buf.commit();
            if !finalize_command_buffer(&cmd_buf, "metal vadd32") {
                return None;
            }
            let ptr = buf_out.contents().as_ptr() as *const u32;
            let out_slice = unsafe { std::slice::from_raw_parts(ptr, 4) };
            Some([out_slice[0], out_slice[1], out_slice[2], out_slice[3]])
        })?
    })
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
fn metal_vadd32(_a: [u32; 4], _b: [u32; 4]) -> Option<[u32; 4]> {
    None
}

#[cfg(target_os = "macos")]
#[cfg(all(target_os = "macos", feature = "metal"))]
fn metal_vbit_cached(
    a: [u32; 4],
    b: [u32; 4],
    select: fn(&MetalState) -> &ProtocolObject<dyn objc2_metal::MTLComputePipelineState>,
) -> Option<[u32; 4]> {
    if !metal_runtime_allowed() {
        return None;
    }
    use core::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_metal::*;

    autoreleasepool(|_| {
        with_metal_state(|ctx| {
            let pipeline = select(ctx);
            let buf_a = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(a.as_ptr() as *mut core::ffi::c_void),
                    a.len() * core::mem::size_of::<u32>(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )?
            };
            let buf_b = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(b.as_ptr() as *mut core::ffi::c_void),
                    b.len() * core::mem::size_of::<u32>(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )?
            };
            let buf_out = ctx.device.newBufferWithLength_options(
                a.len() * core::mem::size_of::<u32>(),
                MTLResourceOptions::StorageModeShared,
            )?;
            let cmd_buf = ctx.queue.commandBuffer()?;
            let encoder = cmd_buf.computeCommandEncoder()?;
            encoder.setComputePipelineState(pipeline);
            unsafe {
                encoder.setBuffer_offset_atIndex(Some(&buf_a), 0, 0);
                encoder.setBuffer_offset_atIndex(Some(&buf_b), 0, 1);
                encoder.setBuffer_offset_atIndex(Some(&buf_out), 0, 2);
            }
            let grid = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            let threads = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            encoder.dispatchThreads_threadsPerThreadgroup(grid, threads);
            encoder.endEncoding();
            cmd_buf.commit();
            if !finalize_command_buffer(&cmd_buf, "metal vector bit pipeline") {
                return None;
            }
            let ptr = buf_out.contents().as_ptr() as *const u32;
            let out_slice = unsafe { std::slice::from_raw_parts(ptr, 4) };
            Some([out_slice[0], out_slice[1], out_slice[2], out_slice[3]])
        })?
    })
}

#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[cfg(all(target_os = "macos", feature = "metal"))]
fn metal_vand(a: [u32; 4], b: [u32; 4]) -> Option<[u32; 4]> {
    metal_vbit_cached(a, b, |ctx| &ctx.vand)
}

#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[cfg(all(target_os = "macos", feature = "metal"))]
fn metal_vxor(a: [u32; 4], b: [u32; 4]) -> Option<[u32; 4]> {
    metal_vbit_cached(a, b, |ctx| &ctx.vxor)
}

#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[cfg(all(target_os = "macos", feature = "metal"))]
fn metal_vor(a: [u32; 4], b: [u32; 4]) -> Option<[u32; 4]> {
    metal_vbit_cached(a, b, |ctx| &ctx.vor)
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
fn metal_vand(_a: [u32; 4], _b: [u32; 4]) -> Option<[u32; 4]> {
    None
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
fn metal_vxor(_a: [u32; 4], _b: [u32; 4]) -> Option<[u32; 4]> {
    None
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
fn metal_vor(_a: [u32; 4], _b: [u32; 4]) -> Option<[u32; 4]> {
    None
}

#[cfg(target_os = "macos")]
#[cfg(all(target_os = "macos", feature = "metal"))]
fn metal_sha256_compress(state: &mut [u32; 8], block: &[u8; 64]) -> bool {
    if !metal_runtime_allowed() {
        return false;
    }
    use core::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_metal::*;

    autoreleasepool(|_| {
        with_metal_state(|ctx| {
            let buf_state = unsafe {
                match ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(state.as_ptr() as *mut core::ffi::c_void),
                    state.len() * core::mem::size_of::<u32>(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                ) {
                    Some(buf) => buf,
                    None => return false,
                }
            };
            let buf_block = unsafe {
                match ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(block.as_ptr() as *mut core::ffi::c_void),
                    block.len(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                ) {
                    Some(buf) => buf,
                    None => return false,
                }
            };
            let cmd_buf = match ctx.queue.commandBuffer() {
                Some(buf) => buf,
                None => return false,
            };
            let encoder = match cmd_buf.computeCommandEncoder() {
                Some(enc) => enc,
                None => return false,
            };
            encoder.setComputePipelineState(&ctx.sha256);
            unsafe {
                encoder.setBuffer_offset_atIndex(Some(&buf_state), 0, 0);
                encoder.setBuffer_offset_atIndex(Some(&buf_block), 0, 1);
            }
            let grid = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            let threads = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            encoder.dispatchThreads_threadsPerThreadgroup(grid, threads);
            encoder.endEncoding();
            cmd_buf.commit();
            if !finalize_command_buffer(&cmd_buf, "metal sha256 compress") {
                return false;
            }
            let ptr = buf_state.contents().as_ptr() as *const u32;
            let out_slice = unsafe { std::slice::from_raw_parts(ptr, 8) };
            state.copy_from_slice(out_slice);
            true
        })
        .unwrap_or(false)
    })
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
fn metal_sha256_compress(_state: &mut [u32; 8], _block: &[u8; 64]) -> bool {
    false
}

#[cfg(all(target_os = "macos", feature = "metal"))]
pub fn metal_sha256_leaves(blocks: &[[u8; 64]]) -> Option<Vec<[u8; 32]>> {
    if !metal_runtime_allowed() {
        return None;
    }
    use core::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_metal::*;

    autoreleasepool(|_| {
        with_metal_state_try(|ctx| {
            let n = blocks.len();
            if n == 0 {
                return Some(Vec::new());
            }
            let flat: Vec<u8> = blocks.iter().flat_map(|b| b.iter()).copied().collect();
            let buf_blocks = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(flat.as_ptr() as *mut core::ffi::c_void),
                    flat.len(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )?
            };
            let buf_out = ctx.device.newBufferWithLength_options(
                n * 8 * core::mem::size_of::<u32>(),
                MTLResourceOptions::StorageModeShared,
            )?;
            let cmd_buf = ctx.queue.commandBuffer()?;
            let encoder = cmd_buf.computeCommandEncoder()?;
            encoder.setComputePipelineState(&ctx.sha256_leaves);
            unsafe {
                encoder.setBuffer_offset_atIndex(Some(&buf_blocks), 0, 0);
                encoder.setBuffer_offset_atIndex(Some(&buf_out), 0, 1);
            }
            let grid = MTLSize {
                width: n as NSUInteger,
                height: 1,
                depth: 1,
            };
            let threads = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            encoder.dispatchThreads_threadsPerThreadgroup(grid, threads);
            encoder.endEncoding();
            cmd_buf.commit();
            if !finalize_command_buffer(&cmd_buf, "metal sha256 leaves") {
                return None;
            }
            let ptr = buf_out.contents().as_ptr() as *const u32;
            let words = unsafe { std::slice::from_raw_parts(ptr, n * 8) };
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let w = &words[i * 8..i * 8 + 8];
                let mut d = [0u8; 32];
                for (j, &word) in w.iter().enumerate() {
                    d[j * 4..j * 4 + 4].copy_from_slice(&word.to_be_bytes());
                }
                out.push(d);
            }
            Some(out)
        })
    })
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
pub fn metal_sha256_leaves(_blocks: &[[u8; 64]]) -> Option<Vec<[u8; 32]>> {
    None
}

#[cfg(all(target_os = "macos", feature = "metal"))]
pub fn metal_sha256_pairs_reduce(digests: &[[u8; 32]]) -> Option<[u8; 32]> {
    if !metal_runtime_allowed() {
        return None;
    }
    use core::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_metal::*;
    if digests.is_empty() {
        return None;
    }
    if digests.len() == 1 {
        return Some(digests[0]);
    }
    let mut cur: Vec<u8> = digests.iter().flat_map(|d| d.iter()).copied().collect();
    autoreleasepool(|_| {
        with_metal_state_try(|ctx| {
            let mut count = cur.len() / 32;
            while count > 1 {
                let pairs = count / 2;
                let has_leftover = (count & 1) != 0;
                let mut next = vec![0u8; (pairs + if has_leftover { 1 } else { 0 }) * 32];
                if pairs > 0 {
                    let pair_len = pairs * 64;
                    let in_buf = unsafe {
                        ctx.device.newBufferWithBytes_length_options(
                            NonNull::new_unchecked(cur.as_ptr() as *mut core::ffi::c_void),
                            pair_len,
                            MTLResourceOptions::CPUCacheModeDefaultCache,
                        )?
                    };
                    let out_buf = ctx.device.newBufferWithLength_options(
                        pairs * 8 * core::mem::size_of::<u32>(),
                        MTLResourceOptions::StorageModeShared,
                    )?;
                    let cmd_buf = ctx.queue.commandBuffer()?;
                    let enc = cmd_buf.computeCommandEncoder()?;
                    enc.setComputePipelineState(&ctx.sha256_pairs);
                    unsafe {
                        enc.setBuffer_offset_atIndex(Some(&in_buf), 0, 0);
                        enc.setBuffer_offset_atIndex(Some(&out_buf), 0, 1);
                    }
                    let grid = MTLSize {
                        width: pairs as NSUInteger,
                        height: 1,
                        depth: 1,
                    };
                    let threads = MTLSize {
                        width: 1,
                        height: 1,
                        depth: 1,
                    };
                    enc.dispatchThreads_threadsPerThreadgroup(grid, threads);
                    enc.endEncoding();
                    cmd_buf.commit();
                    if !finalize_command_buffer(&cmd_buf, "metal sha256 pairs reduce") {
                        return None;
                    }
                    let words = unsafe {
                        std::slice::from_raw_parts(
                            out_buf.contents().as_ptr() as *const u32,
                            pairs * 8,
                        )
                    };
                    for pair in 0..pairs {
                        for j in 0..8 {
                            let word = words[pair * 8 + j];
                            next[pair * 32 + j * 4..pair * 32 + j * 4 + 4]
                                .copy_from_slice(&word.to_be_bytes());
                        }
                    }
                }
                if has_leftover {
                    let src_idx = (count - 1) * 32;
                    let dst_idx = pairs * 32;
                    next[dst_idx..dst_idx + 32].copy_from_slice(&cur[src_idx..src_idx + 32]);
                }
                cur = next;
                count = cur.len() / 32;
            }
            let mut root = [0u8; 32];
            root.copy_from_slice(&cur[..32]);
            Some(root)
        })
    })
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
pub fn metal_sha256_pairs_reduce(_digests: &[[u8; 32]]) -> Option<[u8; 32]> {
    None
}

#[cfg(all(target_os = "macos", feature = "metal"))]
pub(crate) fn metal_ed25519_verify_batch(
    signatures: &[[u8; 64]],
    public_keys: &[[u8; 32]],
    hrams: &[[u8; 32]],
) -> Option<Vec<bool>> {
    if signatures.len() != public_keys.len() || signatures.len() != hrams.len() {
        return None;
    }
    if signatures.is_empty() {
        return Some(Vec::new());
    }
    if !metal_runtime_allowed() {
        return None;
    }
    use core::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_metal::*;

    autoreleasepool(|_| {
        with_metal_state_try(|ctx| {
            let n = signatures.len();
            let flat_sigs: Vec<u8> = signatures.iter().flat_map(|s| s.iter()).copied().collect();
            let flat_pks: Vec<u8> = public_keys.iter().flat_map(|p| p.iter()).copied().collect();
            let flat_hrams: Vec<u8> = hrams.iter().flat_map(|h| h.iter()).copied().collect();
            let count_buf = [n as u32];

            let Some(pipeline) = ctx.ed25519_signature.as_ref() else {
                return None;
            };

            let buf_sigs = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(flat_sigs.as_ptr() as *mut core::ffi::c_void),
                    flat_sigs.len(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )?
            };
            let buf_pks = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(flat_pks.as_ptr() as *mut core::ffi::c_void),
                    flat_pks.len(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )?
            };
            let buf_hrams = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(flat_hrams.as_ptr() as *mut core::ffi::c_void),
                    flat_hrams.len(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )?
            };
            let buf_count = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(count_buf.as_ptr() as *mut core::ffi::c_void),
                    core::mem::size_of::<u32>(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )?
            };
            let buf_out = ctx
                .device
                .newBufferWithLength_options(n, MTLResourceOptions::StorageModeShared)?;

            let cmd = ctx.queue.commandBuffer()?;
            let enc = cmd.computeCommandEncoder()?;
            enc.setComputePipelineState(pipeline);
            unsafe {
                enc.setBuffer_offset_atIndex(Some(&buf_sigs), 0, 0);
                enc.setBuffer_offset_atIndex(Some(&buf_pks), 0, 1);
                enc.setBuffer_offset_atIndex(Some(&buf_hrams), 0, 2);
                enc.setBuffer_offset_atIndex(Some(&buf_count), 0, 3);
                enc.setBuffer_offset_atIndex(Some(&buf_out), 0, 4);
            }
            let grid = MTLSize {
                width: n as NSUInteger,
                height: 1,
                depth: 1,
            };
            let threads = MTLSize {
                width: pipeline.threadExecutionWidth().max(1),
                height: 1,
                depth: 1,
            };
            enc.dispatchThreads_threadsPerThreadgroup(grid, threads);
            enc.endEncoding();
            cmd.commit();
            if !finalize_command_buffer(&cmd, "metal ed25519 batch verify") {
                return None;
            }
            let out =
                unsafe { std::slice::from_raw_parts(buf_out.contents().as_ptr() as *const u8, n) };
            Some(out.iter().map(|b| *b != 0).collect())
        })
    })
}

#[cfg(all(target_os = "macos", feature = "metal", test))]
fn metal_ed25519_run_kernel_for_tests(
    function_name: &str,
    signatures: &[[u8; 64]],
    public_keys: &[[u8; 32]],
    hrams: &[[u8; 32]],
) -> Option<Vec<u8>> {
    if signatures.len() != public_keys.len() || signatures.len() != hrams.len() {
        return None;
    }
    if signatures.is_empty() {
        return Some(Vec::new());
    }

    use core::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_foundation::NSString;
    use objc2_metal::*;

    autoreleasepool(|_| {
        let device = discover_metal_device()?;
        let queue = device.newCommandQueue()?;
        let lib = device
            .newLibraryWithSource_options_error(&NSString::from_str(METAL_ED25519_SHADER), None)
            .ok()?;
        let function_name = NSString::from_str(function_name);
        let func = lib.newFunctionWithName(&function_name)?;
        let pipeline = device
            .newComputePipelineStateWithFunction_error(&func)
            .ok()?;

        let n = signatures.len();
        let flat_sigs: Vec<u8> = signatures.iter().flat_map(|s| s.iter()).copied().collect();
        let flat_pks: Vec<u8> = public_keys.iter().flat_map(|p| p.iter()).copied().collect();
        let flat_hrams: Vec<u8> = hrams.iter().flat_map(|h| h.iter()).copied().collect();
        let count_buf = [n as u32];

        let buf_sigs = unsafe {
            device.newBufferWithBytes_length_options(
                NonNull::new_unchecked(flat_sigs.as_ptr() as *mut core::ffi::c_void),
                flat_sigs.len(),
                MTLResourceOptions::CPUCacheModeDefaultCache,
            )?
        };
        let buf_pks = unsafe {
            device.newBufferWithBytes_length_options(
                NonNull::new_unchecked(flat_pks.as_ptr() as *mut core::ffi::c_void),
                flat_pks.len(),
                MTLResourceOptions::CPUCacheModeDefaultCache,
            )?
        };
        let buf_hrams = unsafe {
            device.newBufferWithBytes_length_options(
                NonNull::new_unchecked(flat_hrams.as_ptr() as *mut core::ffi::c_void),
                flat_hrams.len(),
                MTLResourceOptions::CPUCacheModeDefaultCache,
            )?
        };
        let buf_count = unsafe {
            device.newBufferWithBytes_length_options(
                NonNull::new_unchecked(count_buf.as_ptr() as *mut core::ffi::c_void),
                core::mem::size_of::<u32>(),
                MTLResourceOptions::CPUCacheModeDefaultCache,
            )?
        };
        let buf_out =
            device.newBufferWithLength_options(n, MTLResourceOptions::StorageModeShared)?;

        let cmd = queue.commandBuffer()?;
        let enc = cmd.computeCommandEncoder()?;
        enc.setComputePipelineState(&pipeline);
        unsafe {
            enc.setBuffer_offset_atIndex(Some(&buf_sigs), 0, 0);
            enc.setBuffer_offset_atIndex(Some(&buf_pks), 0, 1);
            enc.setBuffer_offset_atIndex(Some(&buf_hrams), 0, 2);
            enc.setBuffer_offset_atIndex(Some(&buf_count), 0, 3);
            enc.setBuffer_offset_atIndex(Some(&buf_out), 0, 4);
        }
        let grid = MTLSize {
            width: n as NSUInteger,
            height: 1,
            depth: 1,
        };
        let threads = MTLSize {
            width: pipeline.threadExecutionWidth().max(1),
            height: 1,
            depth: 1,
        };
        enc.dispatchThreads_threadsPerThreadgroup(grid, threads);
        enc.endEncoding();
        cmd.commit();
        if !finalize_command_buffer(&cmd, "metal ed25519 batch verify direct") {
            return None;
        }
        let out =
            unsafe { std::slice::from_raw_parts(buf_out.contents().as_ptr() as *const u8, n) };
        Some(out.to_vec())
    })
}

#[cfg(all(target_os = "macos", feature = "metal", test))]
fn metal_ed25519_verify_batch_direct_for_tests(
    signatures: &[[u8; 64]],
    public_keys: &[[u8; 32]],
    hrams: &[[u8; 32]],
) -> Option<Vec<bool>> {
    let out =
        metal_ed25519_run_kernel_for_tests("signature_kernel", signatures, public_keys, hrams)?;
    Some(out.into_iter().map(|byte| byte != 0).collect())
}

#[cfg(all(target_os = "macos", feature = "metal", test))]
fn metal_ed25519_status_batch_direct_for_tests(
    signatures: &[[u8; 64]],
    public_keys: &[[u8; 32]],
    hrams: &[[u8; 32]],
) -> Option<Vec<u8>> {
    metal_ed25519_run_kernel_for_tests("signature_status_kernel", signatures, public_keys, hrams)
}

#[cfg(all(target_os = "macos", feature = "metal", test))]
fn metal_ed25519_check_bytes_for_tests(
    signatures: &[[u8; 64]],
    public_keys: &[[u8; 32]],
    hrams: &[[u8; 32]],
) -> Option<Vec<[u8; 32]>> {
    if signatures.is_empty() {
        return Some(Vec::new());
    }

    use core::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_foundation::NSString;
    use objc2_metal::*;

    if signatures.len() != public_keys.len() || signatures.len() != hrams.len() {
        return None;
    }

    autoreleasepool(|_| {
        let device = discover_metal_device()?;
        let queue = device.newCommandQueue()?;
        let lib = device
            .newLibraryWithSource_options_error(&NSString::from_str(METAL_ED25519_SHADER), None)
            .ok()?;
        let function_name = NSString::from_str("signature_check_bytes_kernel");
        let func = lib.newFunctionWithName(&function_name)?;
        let pipeline = device
            .newComputePipelineStateWithFunction_error(&func)
            .ok()?;

        let n = signatures.len();
        let flat_sigs: Vec<u8> = signatures
            .iter()
            .flat_map(|sig| sig.iter())
            .copied()
            .collect();
        let flat_pks: Vec<u8> = public_keys
            .iter()
            .flat_map(|pk| pk.iter())
            .copied()
            .collect();
        let flat_hrams: Vec<u8> = hrams
            .iter()
            .flat_map(|scalar| scalar.iter())
            .copied()
            .collect();
        let count_buf = [n as u32];

        let buf_sigs = unsafe {
            device.newBufferWithBytes_length_options(
                NonNull::new_unchecked(flat_sigs.as_ptr() as *mut core::ffi::c_void),
                flat_sigs.len(),
                MTLResourceOptions::CPUCacheModeDefaultCache,
            )?
        };
        let buf_pks = unsafe {
            device.newBufferWithBytes_length_options(
                NonNull::new_unchecked(flat_pks.as_ptr() as *mut core::ffi::c_void),
                flat_pks.len(),
                MTLResourceOptions::CPUCacheModeDefaultCache,
            )?
        };
        let buf_hrams = unsafe {
            device.newBufferWithBytes_length_options(
                NonNull::new_unchecked(flat_hrams.as_ptr() as *mut core::ffi::c_void),
                flat_hrams.len(),
                MTLResourceOptions::CPUCacheModeDefaultCache,
            )?
        };
        let buf_count = unsafe {
            device.newBufferWithBytes_length_options(
                NonNull::new_unchecked(count_buf.as_ptr() as *mut core::ffi::c_void),
                core::mem::size_of::<u32>(),
                MTLResourceOptions::CPUCacheModeDefaultCache,
            )?
        };
        let buf_out =
            device.newBufferWithLength_options(n * 32, MTLResourceOptions::StorageModeShared)?;

        let cmd = queue.commandBuffer()?;
        let enc = cmd.computeCommandEncoder()?;
        enc.setComputePipelineState(&pipeline);
        unsafe {
            enc.setBuffer_offset_atIndex(Some(&buf_sigs), 0, 0);
            enc.setBuffer_offset_atIndex(Some(&buf_pks), 0, 1);
            enc.setBuffer_offset_atIndex(Some(&buf_hrams), 0, 2);
            enc.setBuffer_offset_atIndex(Some(&buf_count), 0, 3);
            enc.setBuffer_offset_atIndex(Some(&buf_out), 0, 4);
        }
        let grid = MTLSize {
            width: n as NSUInteger,
            height: 1,
            depth: 1,
        };
        let threads = MTLSize {
            width: pipeline.threadExecutionWidth().max(1),
            height: 1,
            depth: 1,
        };
        enc.dispatchThreads_threadsPerThreadgroup(grid, threads);
        enc.endEncoding();
        cmd.commit();
        if !finalize_command_buffer(&cmd, "metal ed25519 check bytes direct") {
            return None;
        }
        let out =
            unsafe { std::slice::from_raw_parts(buf_out.contents().as_ptr() as *const u8, n * 32) };
        Some(
            out.chunks_exact(32)
                .map(|chunk| {
                    let mut bytes = [0u8; 32];
                    bytes.copy_from_slice(chunk);
                    bytes
                })
                .collect(),
        )
    })
}

#[cfg(all(target_os = "macos", feature = "metal", test))]
fn metal_ed25519_field_roundtrip_for_tests(inputs: &[[u8; 32]]) -> Option<Vec<[u8; 32]>> {
    if inputs.is_empty() {
        return Some(Vec::new());
    }

    use core::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_foundation::NSString;
    use objc2_metal::*;

    autoreleasepool(|_| {
        let device = discover_metal_device()?;
        let queue = device.newCommandQueue()?;
        let lib = device
            .newLibraryWithSource_options_error(&NSString::from_str(METAL_ED25519_SHADER), None)
            .ok()?;
        let function_name = NSString::from_str("field_roundtrip_kernel");
        let func = lib.newFunctionWithName(&function_name)?;
        let pipeline = device
            .newComputePipelineStateWithFunction_error(&func)
            .ok()?;

        let n = inputs.len();
        let flat_inputs: Vec<u8> = inputs
            .iter()
            .flat_map(|point| point.iter())
            .copied()
            .collect();
        let count_buf = [n as u32];

        let buf_inputs = unsafe {
            device.newBufferWithBytes_length_options(
                NonNull::new_unchecked(flat_inputs.as_ptr() as *mut core::ffi::c_void),
                flat_inputs.len(),
                MTLResourceOptions::CPUCacheModeDefaultCache,
            )?
        };
        let buf_count = unsafe {
            device.newBufferWithBytes_length_options(
                NonNull::new_unchecked(count_buf.as_ptr() as *mut core::ffi::c_void),
                core::mem::size_of::<u32>(),
                MTLResourceOptions::CPUCacheModeDefaultCache,
            )?
        };
        let buf_out =
            device.newBufferWithLength_options(n * 32, MTLResourceOptions::StorageModeShared)?;

        let cmd = queue.commandBuffer()?;
        let enc = cmd.computeCommandEncoder()?;
        enc.setComputePipelineState(&pipeline);
        unsafe {
            enc.setBuffer_offset_atIndex(Some(&buf_inputs), 0, 0);
            enc.setBuffer_offset_atIndex(Some(&buf_count), 0, 1);
            enc.setBuffer_offset_atIndex(Some(&buf_out), 0, 2);
        }
        let grid = MTLSize {
            width: n as NSUInteger,
            height: 1,
            depth: 1,
        };
        let threads = MTLSize {
            width: pipeline.threadExecutionWidth().max(1),
            height: 1,
            depth: 1,
        };
        enc.dispatchThreads_threadsPerThreadgroup(grid, threads);
        enc.endEncoding();
        cmd.commit();
        if !finalize_command_buffer(&cmd, "metal ed25519 field roundtrip direct") {
            return None;
        }
        let out =
            unsafe { std::slice::from_raw_parts(buf_out.contents().as_ptr() as *const u8, n * 32) };
        Some(
            out.chunks_exact(32)
                .map(|chunk| {
                    let mut value = [0u8; 32];
                    value.copy_from_slice(chunk);
                    value
                })
                .collect(),
        )
    })
}

#[cfg(all(target_os = "macos", feature = "metal", test))]
fn metal_ed25519_point_decompress_for_tests(
    inputs: &[[u8; 32]],
) -> Option<(Vec<u8>, Vec<[u8; 32]>)> {
    if inputs.is_empty() {
        return Some((Vec::new(), Vec::new()));
    }

    use core::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_foundation::NSString;
    use objc2_metal::*;

    autoreleasepool(|_| {
        let device = discover_metal_device()?;
        let queue = device.newCommandQueue()?;
        let lib = device
            .newLibraryWithSource_options_error(&NSString::from_str(METAL_ED25519_SHADER), None)
            .ok()?;
        let function_name = NSString::from_str("point_decompress_status_kernel");
        let func = lib.newFunctionWithName(&function_name)?;
        let pipeline = device
            .newComputePipelineStateWithFunction_error(&func)
            .ok()?;

        let n = inputs.len();
        let flat_inputs: Vec<u8> = inputs
            .iter()
            .flat_map(|point| point.iter())
            .copied()
            .collect();
        let count_buf = [n as u32];

        let buf_inputs = unsafe {
            device.newBufferWithBytes_length_options(
                NonNull::new_unchecked(flat_inputs.as_ptr() as *mut core::ffi::c_void),
                flat_inputs.len(),
                MTLResourceOptions::CPUCacheModeDefaultCache,
            )?
        };
        let buf_count = unsafe {
            device.newBufferWithBytes_length_options(
                NonNull::new_unchecked(count_buf.as_ptr() as *mut core::ffi::c_void),
                core::mem::size_of::<u32>(),
                MTLResourceOptions::CPUCacheModeDefaultCache,
            )?
        };
        let buf_status =
            device.newBufferWithLength_options(n, MTLResourceOptions::StorageModeShared)?;
        let buf_out =
            device.newBufferWithLength_options(n * 32, MTLResourceOptions::StorageModeShared)?;

        let cmd = queue.commandBuffer()?;
        let enc = cmd.computeCommandEncoder()?;
        enc.setComputePipelineState(&pipeline);
        unsafe {
            enc.setBuffer_offset_atIndex(Some(&buf_inputs), 0, 0);
            enc.setBuffer_offset_atIndex(Some(&buf_count), 0, 1);
            enc.setBuffer_offset_atIndex(Some(&buf_status), 0, 2);
            enc.setBuffer_offset_atIndex(Some(&buf_out), 0, 3);
        }
        let grid = MTLSize {
            width: n as NSUInteger,
            height: 1,
            depth: 1,
        };
        let threads = MTLSize {
            width: pipeline.threadExecutionWidth().max(1),
            height: 1,
            depth: 1,
        };
        enc.dispatchThreads_threadsPerThreadgroup(grid, threads);
        enc.endEncoding();
        cmd.commit();
        if !finalize_command_buffer(&cmd, "metal ed25519 point decompress direct") {
            return None;
        }
        let statuses =
            unsafe { std::slice::from_raw_parts(buf_status.contents().as_ptr() as *const u8, n) };
        let out =
            unsafe { std::slice::from_raw_parts(buf_out.contents().as_ptr() as *const u8, n * 32) };
        Some((
            statuses.to_vec(),
            out.chunks_exact(32)
                .map(|chunk| {
                    let mut value = [0u8; 32];
                    value.copy_from_slice(chunk);
                    value
                })
                .collect(),
        ))
    })
}

#[cfg(all(target_os = "macos", feature = "metal"))]
pub fn metal_aesenc_round(state: [u8; 16], rk: [u8; 16]) -> Option<[u8; 16]> {
    if !metal_runtime_allowed() {
        return None;
    }
    use core::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_metal::*;
    autoreleasepool(|_| {
        with_metal_state_try(|ctx| {
            let buf_s = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(state.as_ptr() as *mut core::ffi::c_void),
                    16,
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )
            }?;
            let buf_k = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(rk.as_ptr() as *mut core::ffi::c_void),
                    16,
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )
            }?;
            let buf_out = ctx
                .device
                .newBufferWithLength_options(16, MTLResourceOptions::StorageModeShared)?;
            let cmd_buf = ctx.queue.commandBuffer()?;
            let enc = cmd_buf.computeCommandEncoder()?;
            enc.setComputePipelineState(&ctx.aesenc);
            unsafe {
                enc.setBuffer_offset_atIndex(Some(&buf_s), 0, 0);
                enc.setBuffer_offset_atIndex(Some(&buf_k), 0, 1);
                enc.setBuffer_offset_atIndex(Some(&buf_out), 0, 2);
            }
            let grid = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            let threads = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            enc.dispatchThreads_threadsPerThreadgroup(grid, threads);
            enc.endEncoding();
            cmd_buf.commit();
            if !finalize_command_buffer(&cmd_buf, "metal aesenc round") {
                return None;
            }
            let mut out = [0u8; 16];
            unsafe {
                let ptr = buf_out.contents().as_ptr() as *const u8;
                core::ptr::copy_nonoverlapping(ptr, out.as_mut_ptr(), 16);
            }
            Some(out)
        })
    })
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
pub fn metal_aesenc_round(_state: [u8; 16], _rk: [u8; 16]) -> Option<[u8; 16]> {
    None
}

#[cfg(all(target_os = "macos", feature = "metal"))]
#[allow(dead_code)]
pub fn metal_aesdec_round(state: [u8; 16], rk: [u8; 16]) -> Option<[u8; 16]> {
    if !metal_runtime_allowed() {
        return None;
    }
    use core::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_metal::*;
    autoreleasepool(|_| {
        with_metal_state_try(|ctx| {
            let buf_s = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(state.as_ptr() as *mut core::ffi::c_void),
                    16,
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )
            }?;
            let buf_k = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(rk.as_ptr() as *mut core::ffi::c_void),
                    16,
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )
            }?;
            let buf_out = ctx
                .device
                .newBufferWithLength_options(16, MTLResourceOptions::StorageModeShared)?;
            let cmd_buf = ctx.queue.commandBuffer()?;
            let enc = cmd_buf.computeCommandEncoder()?;
            enc.setComputePipelineState(&ctx.aesdec);
            unsafe {
                enc.setBuffer_offset_atIndex(Some(&buf_s), 0, 0);
                enc.setBuffer_offset_atIndex(Some(&buf_k), 0, 1);
                enc.setBuffer_offset_atIndex(Some(&buf_out), 0, 2);
            }
            let grid = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            let threads = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            enc.dispatchThreads_threadsPerThreadgroup(grid, threads);
            enc.endEncoding();
            cmd_buf.commit();
            if !finalize_command_buffer(&cmd_buf, "metal aesdec round") {
                return None;
            }
            let mut out = [0u8; 16];
            unsafe {
                let ptr = buf_out.contents().as_ptr() as *const u8;
                core::ptr::copy_nonoverlapping(ptr, out.as_mut_ptr(), 16);
            }
            Some(out)
        })
    })
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
pub fn metal_aesdec_round(_state: [u8; 16], _rk: [u8; 16]) -> Option<[u8; 16]> {
    None
}

#[cfg(all(target_os = "macos", feature = "metal"))]
#[allow(dead_code)]
pub fn metal_keccak_f1600(state: &mut [u64; 25]) -> bool {
    if !metal_runtime_allowed() {
        return false;
    }
    use core::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_metal::*;
    autoreleasepool(|_| {
        with_metal_state_try(|ctx| {
            let buf = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(state.as_ptr() as *mut core::ffi::c_void),
                    25 * core::mem::size_of::<u64>(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )
            }?;
            let cmd = ctx.queue.commandBuffer()?;
            let enc = cmd.computeCommandEncoder()?;
            enc.setComputePipelineState(&ctx.keccak);
            unsafe {
                enc.setBuffer_offset_atIndex(Some(&buf), 0, 0);
            }
            let grid = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            let threads = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            enc.dispatchThreads_threadsPerThreadgroup(grid, threads);
            enc.endEncoding();
            cmd.commit();
            if !finalize_command_buffer(&cmd, "metal keccak_f1600") {
                return None;
            }
            let ptr = buf.contents().as_ptr() as *const u64;
            let out = unsafe { std::slice::from_raw_parts(ptr, 25) };
            state.copy_from_slice(out);
            Some(())
        })
    })
    .is_some()
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
pub fn metal_keccak_f1600(_state: &mut [u64; 25]) -> bool {
    false
}

#[cfg(all(target_os = "macos", feature = "metal"))]
#[allow(dead_code)]
pub fn metal_aesenc_batch(states: &[[u8; 16]], rk: [u8; 16]) -> Option<Vec<[u8; 16]>> {
    if !metal_runtime_allowed() {
        return None;
    }
    use core::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_metal::*;
    if states.is_empty() {
        return Some(Vec::new());
    }
    autoreleasepool(|_| {
        with_metal_state_try(|ctx| {
            let flat: Vec<u8> = states.iter().flat_map(|b| b.iter()).copied().collect();
            let buf_states = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(flat.as_ptr() as *mut core::ffi::c_void),
                    flat.len(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )
            }?;
            let buf_rk = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(rk.as_ptr() as *mut core::ffi::c_void),
                    16,
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )
            }?;
            let mut out = vec![0u8; states.len() * 16];
            let buf_out = ctx
                .device
                .newBufferWithLength_options(out.len(), MTLResourceOptions::StorageModeShared)?;
            let cmd = ctx.queue.commandBuffer()?;
            let enc = cmd.computeCommandEncoder()?;
            enc.setComputePipelineState(&ctx.aesenc_batch);
            unsafe {
                enc.setBuffer_offset_atIndex(Some(&buf_states), 0, 0);
                enc.setBuffer_offset_atIndex(Some(&buf_rk), 0, 1);
                enc.setBuffer_offset_atIndex(Some(&buf_out), 0, 2);
            }
            let grid = MTLSize {
                width: states.len() as NSUInteger,
                height: 1,
                depth: 1,
            };
            let threads = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            enc.dispatchThreads_threadsPerThreadgroup(grid, threads);
            enc.endEncoding();
            cmd.commit();
            if !finalize_command_buffer(&cmd, "metal aesenc batch") {
                return None;
            }
            unsafe {
                let ptr = buf_out.contents().as_ptr() as *const u8;
                core::ptr::copy_nonoverlapping(ptr, out.as_mut_ptr(), out.len());
            }
            let mut vec_out = Vec::with_capacity(states.len());
            for i in 0..states.len() {
                let mut block = [0u8; 16];
                block.copy_from_slice(&out[i * 16..i * 16 + 16]);
                vec_out.push(block);
            }
            Some(vec_out)
        })
    })
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
pub fn metal_aesenc_batch(_states: &[[u8; 16]], _rk: [u8; 16]) -> Option<Vec<[u8; 16]>> {
    None
}

#[cfg(all(target_os = "macos", feature = "metal"))]
#[allow(dead_code)]
pub fn metal_aesdec_batch(states: &[[u8; 16]], rk: [u8; 16]) -> Option<Vec<[u8; 16]>> {
    if !metal_runtime_allowed() {
        return None;
    }
    use core::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_metal::*;
    if states.is_empty() {
        return Some(Vec::new());
    }
    autoreleasepool(|_| {
        with_metal_state_try(|ctx| {
            let flat: Vec<u8> = states.iter().flat_map(|b| b.iter()).copied().collect();
            let buf_states = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(flat.as_ptr() as *mut core::ffi::c_void),
                    flat.len(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )
            }?;
            let buf_rk = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(rk.as_ptr() as *mut core::ffi::c_void),
                    16,
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )
            }?;
            let mut out = vec![0u8; states.len() * 16];
            let buf_out = ctx
                .device
                .newBufferWithLength_options(out.len(), MTLResourceOptions::StorageModeShared)?;
            let cmd = ctx.queue.commandBuffer()?;
            let enc = cmd.computeCommandEncoder()?;
            enc.setComputePipelineState(&ctx.aesdec_batch);
            unsafe {
                enc.setBuffer_offset_atIndex(Some(&buf_states), 0, 0);
                enc.setBuffer_offset_atIndex(Some(&buf_rk), 0, 1);
                enc.setBuffer_offset_atIndex(Some(&buf_out), 0, 2);
            }
            let grid = MTLSize {
                width: states.len() as NSUInteger,
                height: 1,
                depth: 1,
            };
            let threads = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            enc.dispatchThreads_threadsPerThreadgroup(grid, threads);
            enc.endEncoding();
            cmd.commit();
            if !finalize_command_buffer(&cmd, "metal aesdec batch") {
                return None;
            }
            unsafe {
                let ptr = buf_out.contents().as_ptr() as *const u8;
                core::ptr::copy_nonoverlapping(ptr, out.as_mut_ptr(), out.len());
            }
            let mut vec_out = Vec::with_capacity(states.len());
            for i in 0..states.len() {
                let mut block = [0u8; 16];
                block.copy_from_slice(&out[i * 16..i * 16 + 16]);
                vec_out.push(block);
            }
            Some(vec_out)
        })
    })
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
pub fn metal_aesdec_batch(_states: &[[u8; 16]], _rk: [u8; 16]) -> Option<Vec<[u8; 16]>> {
    None
}

#[cfg(all(target_os = "macos", feature = "metal"))]
pub fn metal_aesenc_rounds_batch(
    states: &[[u8; 16]],
    round_keys: &[[u8; 16]],
) -> Option<Vec<[u8; 16]>> {
    if !metal_runtime_allowed() {
        return None;
    }
    use core::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_metal::*;
    if states.is_empty() {
        return Some(Vec::new());
    }
    let nrounds = round_keys.len() as u32;
    autoreleasepool(|_| {
        with_metal_state_try(|ctx| {
            let flat_states: Vec<u8> = states.iter().flat_map(|b| b.iter()).copied().collect();
            let flat_rks: Vec<u8> = round_keys.iter().flat_map(|b| b.iter()).copied().collect();
            let buf_states = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(flat_states.as_ptr() as *mut core::ffi::c_void),
                    flat_states.len(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )
            }?;
            let buf_rks = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(flat_rks.as_ptr() as *mut core::ffi::c_void),
                    flat_rks.len(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )
            }?;
            let mut n_buf = [0u32; 1];
            n_buf[0] = nrounds;
            let buf_n = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(n_buf.as_ptr() as *mut core::ffi::c_void),
                    core::mem::size_of::<u32>(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )
            }?;
            let mut out = vec![0u8; states.len() * 16];
            let buf_out = ctx
                .device
                .newBufferWithLength_options(out.len(), MTLResourceOptions::StorageModeShared)?;
            let cmd = ctx.queue.commandBuffer()?;
            let enc = cmd.computeCommandEncoder()?;
            enc.setComputePipelineState(&ctx.aesenc_rounds);
            unsafe {
                enc.setBuffer_offset_atIndex(Some(&buf_states), 0, 0);
                enc.setBuffer_offset_atIndex(Some(&buf_rks), 0, 1);
                enc.setBuffer_offset_atIndex(Some(&buf_out), 0, 2);
                enc.setBuffer_offset_atIndex(Some(&buf_n), 0, 3);
            }
            let grid = MTLSize {
                width: states.len() as NSUInteger,
                height: 1,
                depth: 1,
            };
            let threads = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            enc.dispatchThreads_threadsPerThreadgroup(grid, threads);
            enc.endEncoding();
            cmd.commit();
            if !finalize_command_buffer(&cmd, "metal aesenc rounds batch") {
                return None;
            }
            unsafe {
                let ptr = buf_out.contents().as_ptr() as *const u8;
                core::ptr::copy_nonoverlapping(ptr, out.as_mut_ptr(), out.len());
            }
            let mut vec_out = Vec::with_capacity(states.len());
            for i in 0..states.len() {
                let mut block = [0u8; 16];
                block.copy_from_slice(&out[i * 16..i * 16 + 16]);
                vec_out.push(block);
            }
            Some(vec_out)
        })
    })
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
pub fn metal_aesenc_rounds_batch(
    _states: &[[u8; 16]],
    _round_keys: &[[u8; 16]],
) -> Option<Vec<[u8; 16]>> {
    None
}

#[cfg(all(target_os = "macos", feature = "metal"))]
#[allow(dead_code)]
pub fn metal_aesdec_rounds_batch(
    states: &[[u8; 16]],
    round_keys: &[[u8; 16]],
) -> Option<Vec<[u8; 16]>> {
    if !metal_runtime_allowed() {
        return None;
    }
    use core::ptr::NonNull;

    use objc2::rc::autoreleasepool;
    use objc2_metal::*;
    if states.is_empty() {
        return Some(Vec::new());
    }
    let nrounds = round_keys.len() as u32;
    autoreleasepool(|_| {
        with_metal_state_try(|ctx| {
            let flat_states: Vec<u8> = states.iter().flat_map(|b| b.iter()).copied().collect();
            let flat_rks: Vec<u8> = round_keys.iter().flat_map(|b| b.iter()).copied().collect();
            let buf_states = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(flat_states.as_ptr() as *mut core::ffi::c_void),
                    flat_states.len(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )
            }?;
            let buf_rks = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(flat_rks.as_ptr() as *mut core::ffi::c_void),
                    flat_rks.len(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )
            }?;
            let mut n_buf = [0u32; 1];
            n_buf[0] = nrounds;
            let buf_n = unsafe {
                ctx.device.newBufferWithBytes_length_options(
                    NonNull::new_unchecked(n_buf.as_ptr() as *mut core::ffi::c_void),
                    core::mem::size_of::<u32>(),
                    MTLResourceOptions::CPUCacheModeDefaultCache,
                )
            }?;
            let mut out = vec![0u8; states.len() * 16];
            let buf_out = ctx
                .device
                .newBufferWithLength_options(out.len(), MTLResourceOptions::StorageModeShared)?;
            let cmd = ctx.queue.commandBuffer()?;
            let enc = cmd.computeCommandEncoder()?;
            enc.setComputePipelineState(&ctx.aesdec_rounds);
            unsafe {
                enc.setBuffer_offset_atIndex(Some(&buf_states), 0, 0);
                enc.setBuffer_offset_atIndex(Some(&buf_rks), 0, 1);
                enc.setBuffer_offset_atIndex(Some(&buf_out), 0, 2);
                enc.setBuffer_offset_atIndex(Some(&buf_n), 0, 3);
            }
            let grid = MTLSize {
                width: states.len() as NSUInteger,
                height: 1,
                depth: 1,
            };
            let threads = MTLSize {
                width: 1,
                height: 1,
                depth: 1,
            };
            enc.dispatchThreads_threadsPerThreadgroup(grid, threads);
            enc.endEncoding();
            cmd.commit();
            if !finalize_command_buffer(&cmd, "metal aesdec rounds batch") {
                return None;
            }
            unsafe {
                let ptr = buf_out.contents().as_ptr() as *const u8;
                core::ptr::copy_nonoverlapping(ptr, out.as_mut_ptr(), out.len());
            }
            let mut vec_out = Vec::with_capacity(states.len());
            for i in 0..states.len() {
                let mut block = [0u8; 16];
                block.copy_from_slice(&out[i * 16..i * 16 + 16]);
                vec_out.push(block);
            }
            Some(vec_out)
        })
    })
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[allow(dead_code)]
pub fn metal_aesdec_rounds_batch(
    _states: &[[u8; 16]],
    _round_keys: &[[u8; 16]],
) -> Option<Vec<[u8; 16]>> {
    None
}

/// Perform one SHA-256 compression round on a 64 byte block.
pub fn sha256_compress(state: &mut [u32; 8], block: &[u8; 64]) {
    if metal_sha256_compress(state, block) {
        return;
    }
    #[cfg(feature = "cuda")]
    if crate::cuda::sha256_compress_cuda(state, block) {
        return;
    }
    #[cfg(target_arch = "aarch64")]
    {
        if sha256_compress_armv8(state, block) {
            return;
        }
    }
    #[cfg(target_arch = "x86_64")]
    {
        if sha256_compress_x86_shani(state, block) {
            return;
        }
    }
    sha256_compress_scalar_ref(state, block)
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
fn sha256_compress_armv8(state: &mut [u32; 8], block: &[u8; 64]) -> bool {
    use std::sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    };
    static FORCED_DISABLED: AtomicBool = AtomicBool::new(false);
    static SELFTEST_OK: OnceLock<bool> = OnceLock::new();
    if FORCED_DISABLED.load(Ordering::SeqCst) {
        return false;
    }
    if !std::arch::is_aarch64_feature_detected!("sha2") {
        return false;
    }
    let ok = *SELFTEST_OK.get_or_init(|| {
        // Golden self-test: compress the single-block padded "abc" starting from IV.
        let mut st_scalar = [
            0x6a09e667u32,
            0xbb67ae85,
            0x3c6ef372,
            0xa54ff53a,
            0x510e527f,
            0x9b05688c,
            0x1f83d9ab,
            0x5be0cd19,
        ];
        let mut st_hw = st_scalar;
        let mut blk = [0u8; 64];
        blk[0] = b'a';
        blk[1] = b'b';
        blk[2] = b'c';
        blk[3] = 0x80;
        blk[63] = 24; // 24-bit length
        sha256_compress_scalar_ref(&mut st_scalar, &blk);
        unsafe { sha256_compress_armv8_impl(&mut st_hw, &blk) };
        if st_scalar != st_hw {
            FORCED_DISABLED.store(true, Ordering::SeqCst);
            return false;
        }
        true
    });
    if !ok {
        return false;
    }
    unsafe { sha256_compress_armv8_impl(state, block) };
    true
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "sha2")]
#[allow(unused_unsafe)]
unsafe fn sha256_compress_armv8_impl(state: &mut [u32; 8], block: &[u8; 64]) {
    use core::arch::aarch64::*;

    // Load state
    let st0 = unsafe { vld1q_u32(state.as_ptr()) };
    let st1 = unsafe { vld1q_u32(state.as_ptr().add(4)) };
    let mut abcd = st0;
    let mut efgh = st1;

    // Helper to load 16 bytes -> 4 big-endian u32 lanes
    #[inline(always)]
    unsafe fn load_be_u32x4(ptr: *const u8) -> uint32x4_t {
        let v = unsafe { vld1q_u8(ptr) };
        let v = unsafe { vrev32q_u8(v) };
        unsafe { vreinterpretq_u32_u8(v) }
    }

    // Load first 16 message words
    let mut w0 = unsafe { load_be_u32x4(block.as_ptr().add(0)) };
    let mut w1 = unsafe { load_be_u32x4(block.as_ptr().add(16)) };
    let mut w2 = unsafe { load_be_u32x4(block.as_ptr().add(32)) };
    let mut w3 = unsafe { load_be_u32x4(block.as_ptr().add(48)) };

    // K constants
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut kptr = K.as_ptr();

    // Process 64 rounds in 4 groups of 16 (vectors of 4)
    for _ in 0..4 {
        // Rounds: w0
        let k0 = unsafe { vld1q_u32(kptr) };
        let wk0 = unsafe { vaddq_u32(w0, k0) };
        efgh = unsafe { vsha256hq_u32(efgh, abcd, wk0) };
        abcd = unsafe { vsha256h2q_u32(abcd, efgh, wk0) };

        // Prepare w1
        w1 = unsafe { vsha256su0q_u32(w1, w0) };
        let k1 = unsafe { vld1q_u32(kptr.add(4)) };
        let wk1 = unsafe { vaddq_u32(w1, k1) };
        efgh = unsafe { vsha256hq_u32(efgh, abcd, wk1) };
        abcd = unsafe { vsha256h2q_u32(abcd, efgh, wk1) };

        // Prepare w2
        w2 = unsafe { vsha256su0q_u32(w2, w1) };
        let k2 = unsafe { vld1q_u32(kptr.add(8)) };
        let wk2 = unsafe { vaddq_u32(w2, k2) };
        efgh = unsafe { vsha256hq_u32(efgh, abcd, wk2) };
        abcd = unsafe { vsha256h2q_u32(abcd, efgh, wk2) };

        // Prepare w3
        w3 = unsafe { vsha256su0q_u32(w3, w2) };
        let k3 = unsafe { vld1q_u32(kptr.add(12)) };
        let wk3 = unsafe { vaddq_u32(w3, k3) };
        efgh = unsafe { vsha256hq_u32(efgh, abcd, wk3) };
        abcd = unsafe { vsha256h2q_u32(abcd, efgh, wk3) };

        // Next schedule words
        w0 = unsafe { vsha256su1q_u32(w0, w3, w2) };
        kptr = unsafe { kptr.add(16) };
    }

    // state += (a..h)
    let abcd_out = unsafe { vaddq_u32(abcd, st0) };
    let efgh_out = unsafe { vaddq_u32(efgh, st1) };
    unsafe { vst1q_u32(state.as_mut_ptr(), abcd_out) };
    unsafe { vst1q_u32(state.as_mut_ptr().add(4), efgh_out) };
}

// x86/x86_64 SHA-NI accelerated SHA-256 compression. Returns `true` when the
// SHA/SSSE3 intrinsics successfully ran and updated `state`, otherwise falls
// back to the scalar reference path.
#[cfg(target_arch = "x86_64")]
#[inline(always)]
fn sha256_compress_x86_shani(state: &mut [u32; 8], block: &[u8; 64]) -> bool {
    use std::sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    };
    static FORCED_DISABLED: AtomicBool = AtomicBool::new(false);
    static SELFTEST_OK: OnceLock<bool> = OnceLock::new();
    if FORCED_DISABLED.load(Ordering::SeqCst) {
        return false;
    }
    if !std::is_x86_feature_detected!("sha") || !std::is_x86_feature_detected!("ssse3") {
        return false;
    }
    let ok = *SELFTEST_OK.get_or_init(|| {
        let mut st_scalar = [
            0x6a09e667u32,
            0xbb67ae85,
            0x3c6ef372,
            0xa54ff53a,
            0x510e527f,
            0x9b05688c,
            0x1f83d9ab,
            0x5be0cd19,
        ];
        let mut st_hw = st_scalar;
        let mut blk = [0u8; 64];
        blk[0] = b'a';
        blk[1] = b'b';
        blk[2] = b'c';
        blk[3] = 0x80;
        blk[63] = 24;
        sha256_compress_scalar_ref(&mut st_scalar, &blk);
        unsafe { sha256_compress_x86_shani_impl(&mut st_hw, &blk) };
        if st_scalar != st_hw {
            FORCED_DISABLED.store(true, Ordering::SeqCst);
            return false;
        }
        // Second vector: arbitrary 64-byte pattern to exercise schedule ops.
        let mut blk2 = [0u8; 64];
        for i in 0..64 {
            blk2[i] = (i as u8).wrapping_mul(37).wrapping_add(13);
        }
        sha256_compress_scalar_ref(&mut st_scalar, &blk2);
        unsafe { sha256_compress_x86_shani_impl(&mut st_hw, &blk2) };
        if st_scalar != st_hw {
            FORCED_DISABLED.store(true, Ordering::SeqCst);
            return false;
        }
        true
    });
    if !ok {
        return false;
    }
    unsafe { sha256_compress_x86_shani_impl(state, block) };
    true
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sha")]
#[target_feature(enable = "ssse3")]
unsafe fn sha256_compress_x86_shani_impl(state: &mut [u32; 8], block: &[u8; 64]) {
    use core::arch::x86_64::*;

    unsafe {
        // Byte-swap mask: reverse each 32-bit lane
        let be_mask = _mm_set_epi8(3, 2, 1, 0, 7, 6, 5, 4, 11, 10, 9, 8, 15, 14, 13, 12);

        // Load state (a..h)
        let st0 = _mm_loadu_si128(state.as_ptr() as *const __m128i);
        let st1 = _mm_loadu_si128(state.as_ptr().add(4) as *const __m128i);
        let mut abcd = st0;
        let mut efgh = st1;

        // Load message as big-endian words
        let mut w0 = _mm_shuffle_epi8(_mm_loadu_si128(block.as_ptr() as *const __m128i), be_mask);
        let mut w1 = _mm_shuffle_epi8(
            _mm_loadu_si128(block.as_ptr().add(16) as *const __m128i),
            be_mask,
        );
        let mut w2 = _mm_shuffle_epi8(
            _mm_loadu_si128(block.as_ptr().add(32) as *const __m128i),
            be_mask,
        );
        let mut w3 = _mm_shuffle_epi8(
            _mm_loadu_si128(block.as_ptr().add(48) as *const __m128i),
            be_mask,
        );

        // K constants in u32
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut kptr = K.as_ptr();

        // Process 64 rounds in 4 groups of 16 rounds.
        for _ in 0..4 {
            // Rounds for w0
            let mut wk = _mm_add_epi32(w0, _mm_loadu_si128(kptr as *const __m128i));
            efgh = _mm_sha256rnds2_epu32(efgh, abcd, wk);
            wk = _mm_shuffle_epi32(wk, 0x0E);
            abcd = _mm_sha256rnds2_epu32(abcd, efgh, wk);

            // Prepare w1
            w1 = _mm_sha256msg1_epu32(w1, w0);
            wk = _mm_add_epi32(w1, _mm_loadu_si128(kptr.add(4) as *const __m128i));
            efgh = _mm_sha256rnds2_epu32(efgh, abcd, wk);
            wk = _mm_shuffle_epi32(wk, 0x0E);
            abcd = _mm_sha256rnds2_epu32(abcd, efgh, wk);

            // Prepare w2
            w2 = _mm_sha256msg1_epu32(w2, w1);
            wk = _mm_add_epi32(w2, _mm_loadu_si128(kptr.add(8) as *const __m128i));
            efgh = _mm_sha256rnds2_epu32(efgh, abcd, wk);
            wk = _mm_shuffle_epi32(wk, 0x0E);
            abcd = _mm_sha256rnds2_epu32(abcd, efgh, wk);

            // Prepare w3
            w3 = _mm_sha256msg1_epu32(w3, w2);
            wk = _mm_add_epi32(w3, _mm_loadu_si128(kptr.add(12) as *const __m128i));
            efgh = _mm_sha256rnds2_epu32(efgh, abcd, wk);
            wk = _mm_shuffle_epi32(wk, 0x0E);
            abcd = _mm_sha256rnds2_epu32(abcd, efgh, wk);

            // Extend schedule for next group
            w0 = _mm_sha256msg2_epu32(w0, w3);
            kptr = kptr.add(16);
        }

        // state += working vars
        let out0 = _mm_add_epi32(abcd, st0);
        let out1 = _mm_add_epi32(efgh, st1);
        _mm_storeu_si128(state.as_mut_ptr() as *mut __m128i, out0);
        _mm_storeu_si128(state.as_mut_ptr().add(4) as *mut __m128i, out1);
    }
}

fn sha256_compress_scalar_ref(state: &mut [u32; 8], block: &[u8; 64]) {
    // Constants as defined in FIPS 180-4 section 4.2.2.
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    // Message schedule
    let mut w = [0u32; 64];
    for (t, chunk) in block.chunks(4).enumerate().take(16) {
        w[t] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    for t in 16..64 {
        let s0 = w[t - 15].rotate_right(7) ^ w[t - 15].rotate_right(18) ^ (w[t - 15] >> 3);
        let s1 = w[t - 2].rotate_right(17) ^ w[t - 2].rotate_right(19) ^ (w[t - 2] >> 10);
        w[t] = w[t - 16]
            .wrapping_add(s0)
            .wrapping_add(w[t - 7])
            .wrapping_add(s1);
    }

    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];
    let mut e = state[4];
    let mut f = state[5];
    let mut g = state[6];
    let mut h = state[7];

    for t in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let temp1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[t])
            .wrapping_add(w[t]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(maj);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

/// Lane-wise addition of two 32-bit vectors. The length of `a` and `b` must
/// match and determines the number of lanes processed.
pub fn vadd32_slice(a: &[u32], b: &[u32], out: &mut [u32]) {
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len(), out.len());
    #[cfg(target_arch = "x86_64")]
    unsafe {
        match simd_choice() {
            SimdChoice::Avx512 | SimdChoice::Avx2 => {
                use core::arch::x86_64::*;
                let mut i = 0usize;
                let len = a.len();
                while i + 8 <= len {
                    let va = _mm256_loadu_si256(a.as_ptr().add(i) as *const __m256i);
                    let vb = _mm256_loadu_si256(b.as_ptr().add(i) as *const __m256i);
                    let vr = _mm256_add_epi32(va, vb);
                    _mm256_storeu_si256(out.as_mut_ptr().add(i) as *mut __m256i, vr);
                    i += 8;
                }
                while i + 4 <= len {
                    let va = _mm_loadu_si128(a.as_ptr().add(i) as *const __m128i);
                    let vb = _mm_loadu_si128(b.as_ptr().add(i) as *const __m128i);
                    let vr = _mm_add_epi32(va, vb);
                    _mm_storeu_si128(out.as_mut_ptr().add(i) as *mut __m128i, vr);
                    i += 4;
                }
                while i < len {
                    out[i] = a[i].wrapping_add(b[i]);
                    i += 1;
                }
                return;
            }
            SimdChoice::Sse2 => {
                use core::arch::x86_64::*;
                let mut i = 0usize;
                let len = a.len();
                while i + 4 <= len {
                    let va = _mm_loadu_si128(a.as_ptr().add(i) as *const __m128i);
                    let vb = _mm_loadu_si128(b.as_ptr().add(i) as *const __m128i);
                    let vr = _mm_add_epi32(va, vb);
                    _mm_storeu_si128(out.as_mut_ptr().add(i) as *mut __m128i, vr);
                    i += 4;
                }
                while i < len {
                    out[i] = a[i].wrapping_add(b[i]);
                    i += 1;
                }
                return;
            }
            _ => {}
        }
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        if matches!(simd_choice(), SimdChoice::Neon) {
            use core::arch::aarch64::*;
            let mut i = 0usize;
            let len = a.len();
            while i + 4 <= len {
                let va = vld1q_u32(a.as_ptr().add(i));
                let vb = vld1q_u32(b.as_ptr().add(i));
                let vr = vaddq_u32(va, vb);
                vst1q_u32(out.as_mut_ptr().add(i), vr);
                i += 4;
            }
            while i < len {
                out[i] = a[i].wrapping_add(b[i]);
                i += 1;
            }
            return;
        }
    }
    for ((o, &x), &y) in out.iter_mut().zip(a).zip(b) {
        *o = x.wrapping_add(y);
    }
}

/// Lane-wise bitwise AND of two vectors.
pub fn vand_slice(a: &[u32], b: &[u32], out: &mut [u32]) {
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len(), out.len());
    #[cfg(target_arch = "x86_64")]
    unsafe {
        match simd_choice() {
            SimdChoice::Avx512 | SimdChoice::Avx2 => {
                use core::arch::x86_64::*;
                let mut i = 0usize;
                let len = a.len();
                while i + 8 <= len {
                    let va = _mm256_loadu_si256(a.as_ptr().add(i) as *const __m256i);
                    let vb = _mm256_loadu_si256(b.as_ptr().add(i) as *const __m256i);
                    let vr = _mm256_and_si256(va, vb);
                    _mm256_storeu_si256(out.as_mut_ptr().add(i) as *mut __m256i, vr);
                    i += 8;
                }
                while i + 4 <= len {
                    let va = _mm_loadu_si128(a.as_ptr().add(i) as *const __m128i);
                    let vb = _mm_loadu_si128(b.as_ptr().add(i) as *const __m128i);
                    let vr = _mm_and_si128(va, vb);
                    _mm_storeu_si128(out.as_mut_ptr().add(i) as *mut __m128i, vr);
                    i += 4;
                }
                while i < len {
                    out[i] = a[i] & b[i];
                    i += 1;
                }
                return;
            }
            SimdChoice::Sse2 => {
                use core::arch::x86_64::*;
                let mut i = 0usize;
                let len = a.len();
                while i + 4 <= len {
                    let va = _mm_loadu_si128(a.as_ptr().add(i) as *const __m128i);
                    let vb = _mm_loadu_si128(b.as_ptr().add(i) as *const __m128i);
                    let vr = _mm_and_si128(va, vb);
                    _mm_storeu_si128(out.as_mut_ptr().add(i) as *mut __m128i, vr);
                    i += 4;
                }
                while i < len {
                    out[i] = a[i] & b[i];
                    i += 1;
                }
                return;
            }
            _ => {}
        }
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        if matches!(simd_choice(), SimdChoice::Neon) {
            use core::arch::aarch64::*;
            let mut i = 0usize;
            let len = a.len();
            while i + 4 <= len {
                let va = vld1q_u32(a.as_ptr().add(i));
                let vb = vld1q_u32(b.as_ptr().add(i));
                let vr = vandq_u32(va, vb);
                vst1q_u32(out.as_mut_ptr().add(i), vr);
                i += 4;
            }
            while i < len {
                out[i] = a[i] & b[i];
                i += 1;
            }
            return;
        }
    }
    for ((o, &x), &y) in out.iter_mut().zip(a).zip(b) {
        *o = x & y;
    }
}

/// Lane-wise bitwise XOR of two vectors.
pub fn vxor_slice(a: &[u32], b: &[u32], out: &mut [u32]) {
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len(), out.len());
    #[cfg(target_arch = "x86_64")]
    unsafe {
        match simd_choice() {
            SimdChoice::Avx512 | SimdChoice::Avx2 => {
                use core::arch::x86_64::*;
                let mut i = 0usize;
                let len = a.len();
                while i + 8 <= len {
                    let va = _mm256_loadu_si256(a.as_ptr().add(i) as *const __m256i);
                    let vb = _mm256_loadu_si256(b.as_ptr().add(i) as *const __m256i);
                    let vr = _mm256_xor_si256(va, vb);
                    _mm256_storeu_si256(out.as_mut_ptr().add(i) as *mut __m256i, vr);
                    i += 8;
                }
                while i + 4 <= len {
                    let va = _mm_loadu_si128(a.as_ptr().add(i) as *const __m128i);
                    let vb = _mm_loadu_si128(b.as_ptr().add(i) as *const __m128i);
                    let vr = _mm_xor_si128(va, vb);
                    _mm_storeu_si128(out.as_mut_ptr().add(i) as *mut __m128i, vr);
                    i += 4;
                }
                while i < len {
                    out[i] = a[i] ^ b[i];
                    i += 1;
                }
                return;
            }
            SimdChoice::Sse2 => {
                use core::arch::x86_64::*;
                let mut i = 0usize;
                let len = a.len();
                while i + 4 <= len {
                    let va = _mm_loadu_si128(a.as_ptr().add(i) as *const __m128i);
                    let vb = _mm_loadu_si128(b.as_ptr().add(i) as *const __m128i);
                    let vr = _mm_xor_si128(va, vb);
                    _mm_storeu_si128(out.as_mut_ptr().add(i) as *mut __m128i, vr);
                    i += 4;
                }
                while i < len {
                    out[i] = a[i] ^ b[i];
                    i += 1;
                }
                return;
            }
            _ => {}
        }
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        if matches!(simd_choice(), SimdChoice::Neon) {
            use core::arch::aarch64::*;
            let mut i = 0usize;
            let len = a.len();
            while i + 4 <= len {
                let va = vld1q_u32(a.as_ptr().add(i));
                let vb = vld1q_u32(b.as_ptr().add(i));
                let vr = veorq_u32(va, vb);
                vst1q_u32(out.as_mut_ptr().add(i), vr);
                i += 4;
            }
            while i < len {
                out[i] = a[i] ^ b[i];
                i += 1;
            }
            return;
        }
    }
    for ((o, &x), &y) in out.iter_mut().zip(a).zip(b) {
        *o = x ^ y;
    }
}

/// Lane-wise bitwise OR of two vectors.
pub fn vor_slice(a: &[u32], b: &[u32], out: &mut [u32]) {
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len(), out.len());
    #[cfg(target_arch = "x86_64")]
    unsafe {
        match simd_choice() {
            SimdChoice::Avx512 | SimdChoice::Avx2 => {
                use core::arch::x86_64::*;
                let mut i = 0usize;
                let len = a.len();
                while i + 8 <= len {
                    let va = _mm256_loadu_si256(a.as_ptr().add(i) as *const __m256i);
                    let vb = _mm256_loadu_si256(b.as_ptr().add(i) as *const __m256i);
                    let vr = _mm256_or_si256(va, vb);
                    _mm256_storeu_si256(out.as_mut_ptr().add(i) as *mut __m256i, vr);
                    i += 8;
                }
                while i + 4 <= len {
                    let va = _mm_loadu_si128(a.as_ptr().add(i) as *const __m128i);
                    let vb = _mm_loadu_si128(b.as_ptr().add(i) as *const __m128i);
                    let vr = _mm_or_si128(va, vb);
                    _mm_storeu_si128(out.as_mut_ptr().add(i) as *mut __m128i, vr);
                    i += 4;
                }
                while i < len {
                    out[i] = a[i] | b[i];
                    i += 1;
                }
                return;
            }
            SimdChoice::Sse2 => {
                use core::arch::x86_64::*;
                let mut i = 0usize;
                let len = a.len();
                while i + 4 <= len {
                    let va = _mm_loadu_si128(a.as_ptr().add(i) as *const __m128i);
                    let vb = _mm_loadu_si128(b.as_ptr().add(i) as *const __m128i);
                    let vr = _mm_or_si128(va, vb);
                    _mm_storeu_si128(out.as_mut_ptr().add(i) as *mut __m128i, vr);
                    i += 4;
                }
                while i < len {
                    out[i] = a[i] | b[i];
                    i += 1;
                }
                return;
            }
            _ => {}
        }
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        if matches!(simd_choice(), SimdChoice::Neon) {
            use core::arch::aarch64::*;
            let mut i = 0usize;
            let len = a.len();
            while i + 4 <= len {
                let va = vld1q_u32(a.as_ptr().add(i));
                let vb = vld1q_u32(b.as_ptr().add(i));
                let vr = vorrq_u32(va, vb);
                vst1q_u32(out.as_mut_ptr().add(i), vr);
                i += 4;
            }
            while i < len {
                out[i] = a[i] | b[i];
                i += 1;
            }
            return;
        }
    }
    for ((o, &x), &y) in out.iter_mut().zip(a).zip(b) {
        *o = x | y;
    }
}

/// Rotate each 32-bit lane left by `k` bits.
pub fn vrot32_slice(a: &[u32], k: u32, out: &mut [u32]) {
    debug_assert_eq!(a.len(), out.len());
    #[cfg(target_arch = "x86_64")]
    unsafe {
        match simd_choice() {
            SimdChoice::Avx512 | SimdChoice::Avx2 => {
                use core::arch::x86_64::*;
                let mut i = 0usize;
                let len = a.len();
                let k = (k & 31) as i32;
                let shl = _mm_cvtsi32_si128(k);
                let shr = _mm_cvtsi32_si128(32 - k);
                while i + 8 <= len {
                    let va = _mm256_loadu_si256(a.as_ptr().add(i) as *const __m256i);
                    let left = _mm256_sll_epi32(va, shl);
                    let right = _mm256_srl_epi32(va, shr);
                    let vr = _mm256_or_si256(left, right);
                    _mm256_storeu_si256(out.as_mut_ptr().add(i) as *mut __m256i, vr);
                    i += 8;
                }
                while i + 4 <= len {
                    let va = _mm_loadu_si128(a.as_ptr().add(i) as *const __m128i);
                    let left = _mm_sll_epi32(va, shl);
                    let right = _mm_srl_epi32(va, shr);
                    let vr = _mm_or_si128(left, right);
                    _mm_storeu_si128(out.as_mut_ptr().add(i) as *mut __m128i, vr);
                    i += 4;
                }
                while i < len {
                    out[i] = a[i].rotate_left(k as u32);
                    i += 1;
                }
                return;
            }
            SimdChoice::Sse2 => {
                use core::arch::x86_64::*;
                let mut i = 0usize;
                let len = a.len();
                let k = (k & 31) as i32;
                let shl = _mm_cvtsi32_si128(k);
                let shr = _mm_cvtsi32_si128(32 - k);
                while i + 4 <= len {
                    let va = _mm_loadu_si128(a.as_ptr().add(i) as *const __m128i);
                    let left = _mm_sll_epi32(va, shl);
                    let right = _mm_srl_epi32(va, shr);
                    let vr = _mm_or_si128(left, right);
                    _mm_storeu_si128(out.as_mut_ptr().add(i) as *mut __m128i, vr);
                    i += 4;
                }
                while i < len {
                    out[i] = a[i].rotate_left(k as u32);
                    i += 1;
                }
                return;
            }
            _ => {}
        }
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        if matches!(simd_choice(), SimdChoice::Neon) {
            use core::arch::aarch64::*;
            let mut i = 0usize;
            let len = a.len();
            let k = (k & 31) as i32;
            let sh_left = vdupq_n_s32(k);
            let sh_right = vdupq_n_s32(-(32 - k));
            while i + 4 <= len {
                let va = vld1q_u32(a.as_ptr().add(i));
                let left = vshlq_u32(va, sh_left);
                let right = vshlq_u32(va, sh_right);
                let vr = vorrq_u32(left, right);
                vst1q_u32(out.as_mut_ptr().add(i), vr);
                i += 4;
            }
            while i < len {
                out[i] = a[i].rotate_left(k as u32);
                i += 1;
            }
            return;
        }
    }
    for (o, &x) in out.iter_mut().zip(a) {
        *o = x.rotate_left(k);
    }
}

/// Lane-wise 64-bit addition. The vector must have an even number of lanes.
pub fn vadd64_slice(a: &[u32], b: &[u32], out: &mut [u32]) {
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len(), out.len());
    debug_assert!(a.len().is_multiple_of(2));
    #[cfg(target_arch = "x86_64")]
    unsafe {
        match simd_choice() {
            SimdChoice::Avx512 | SimdChoice::Avx2 => {
                use core::arch::x86_64::*;
                let mut i = 0usize;
                let len = a.len();
                while i + 8 <= len {
                    let va = _mm256_loadu_si256(a.as_ptr().add(i) as *const __m256i);
                    let vb = _mm256_loadu_si256(b.as_ptr().add(i) as *const __m256i);
                    let vr = _mm256_add_epi64(va, vb);
                    _mm256_storeu_si256(out.as_mut_ptr().add(i) as *mut __m256i, vr);
                    i += 8;
                }
                while i + 4 <= len {
                    let va = _mm_loadu_si128(a.as_ptr().add(i) as *const __m128i);
                    let vb = _mm_loadu_si128(b.as_ptr().add(i) as *const __m128i);
                    let vr = _mm_add_epi64(va, vb);
                    _mm_storeu_si128(out.as_mut_ptr().add(i) as *mut __m128i, vr);
                    i += 4;
                }
                while i < len {
                    let a0 = (a[i] as u64) | ((a[i + 1] as u64) << 32);
                    let b0 = (b[i] as u64) | ((b[i + 1] as u64) << 32);
                    let r = a0.wrapping_add(b0);
                    out[i] = (r & 0xffff_ffff) as u32;
                    out[i + 1] = (r >> 32) as u32;
                    i += 2;
                }
                return;
            }
            SimdChoice::Sse2 => {
                use core::arch::x86_64::*;
                let mut i = 0usize;
                let len = a.len();
                while i + 4 <= len {
                    let va = _mm_loadu_si128(a.as_ptr().add(i) as *const __m128i);
                    let vb = _mm_loadu_si128(b.as_ptr().add(i) as *const __m128i);
                    let vr = _mm_add_epi64(va, vb);
                    _mm_storeu_si128(out.as_mut_ptr().add(i) as *mut __m128i, vr);
                    i += 4;
                }
                while i < len {
                    let a0 = (a[i] as u64) | ((a[i + 1] as u64) << 32);
                    let b0 = (b[i] as u64) | ((b[i + 1] as u64) << 32);
                    let r = a0.wrapping_add(b0);
                    out[i] = (r & 0xffff_ffff) as u32;
                    out[i + 1] = (r >> 32) as u32;
                    i += 2;
                }
                return;
            }
            _ => {}
        }
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        if matches!(simd_choice(), SimdChoice::Neon) {
            use core::arch::aarch64::*;
            let mut i = 0usize;
            let len = a.len();
            while i + 4 <= len {
                // Load as u32x4, reinterpret to u64x2, add, reinterpret back to u32x4.
                let va32 = vld1q_u32(a.as_ptr().add(i));
                let vb32 = vld1q_u32(b.as_ptr().add(i));
                let va64 = vreinterpretq_u64_u32(va32);
                let vb64 = vreinterpretq_u64_u32(vb32);
                let vr64 = vaddq_u64(va64, vb64);
                let vr32 = vreinterpretq_u32_u64(vr64);
                vst1q_u32(out.as_mut_ptr().add(i), vr32);
                i += 4;
            }
            while i < len {
                let a0 = (a[i] as u64) | ((a[i + 1] as u64) << 32);
                let b0 = (b[i] as u64) | ((b[i + 1] as u64) << 32);
                let r = a0.wrapping_add(b0);
                out[i] = (r & 0xffff_ffff) as u32;
                out[i + 1] = (r >> 32) as u32;
                i += 2;
            }
            return;
        }
    }
    for i in (0..a.len()).step_by(2) {
        let a0 = (a[i] as u64) | ((a[i + 1] as u64) << 32);
        let b0 = (b[i] as u64) | ((b[i + 1] as u64) << 32);
        let r = a0.wrapping_add(b0);
        out[i] = (r & 0xffff_ffff) as u32;
        out[i + 1] = (r >> 32) as u32;
    }
}

/// Dynamic lane version of [`vadd32`]. Returns a new vector of equal length.
#[allow(dead_code)]
pub fn vadd32_dyn(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; a.len()];
    vadd32_slice(a, b, &mut out);
    out
}

/// Dynamic lane version of [`vadd64`].
#[allow(dead_code)]
pub fn vadd64_dyn(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; a.len()];
    vadd64_slice(a, b, &mut out);
    out
}

/// Dynamic lane version of [`vand`].
#[allow(dead_code)]
pub fn vand_dyn(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; a.len()];
    vand_slice(a, b, &mut out);
    out
}

/// Dynamic lane version of [`vxor`].
#[allow(dead_code)]
pub fn vxor_dyn(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; a.len()];
    vxor_slice(a, b, &mut out);
    out
}

/// Dynamic lane version of [`vor`].
#[allow(dead_code)]
pub fn vor_dyn(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; a.len()];
    vor_slice(a, b, &mut out);
    out
}

/// Dynamic lane version of [`vrot32`].
#[allow(dead_code)]
pub fn vrot32_dyn(a: &[u32], k: u32) -> Vec<u32> {
    let mut out = vec![0u32; a.len()];
    vrot32_slice(a, k, &mut out);
    out
}

/// Allocate a vector sized to the host's SIMD width and fill with zeros.
pub fn zero_vector() -> Vec<u32> {
    vec![0u32; simd_lanes()]
}

/// Lane-wise 32-bit addition using the host SIMD width.
pub fn vadd32_auto(a: &[u32], b: &[u32]) -> Vec<u32> {
    let lanes = simd_lanes();
    assert_eq!(a.len(), lanes);
    assert_eq!(b.len(), lanes);
    let mut out = vec![0u32; lanes];
    vadd32_slice(a, b, &mut out);
    out
}

/// Lane-wise 64-bit addition using the host SIMD width.
pub fn vadd64_auto(a: &[u32], b: &[u32]) -> Vec<u32> {
    let lanes = simd_lanes();
    assert_eq!(a.len(), lanes);
    assert_eq!(b.len(), lanes);
    let mut out = vec![0u32; lanes];
    vadd64_slice(a, b, &mut out);
    out
}

/// Bitwise AND using the host SIMD width.
pub fn vand_auto(a: &[u32], b: &[u32]) -> Vec<u32> {
    let lanes = simd_lanes();
    assert_eq!(a.len(), lanes);
    assert_eq!(b.len(), lanes);
    let mut out = vec![0u32; lanes];
    vand_slice(a, b, &mut out);
    out
}

/// Bitwise XOR using the host SIMD width.
pub fn vxor_auto(a: &[u32], b: &[u32]) -> Vec<u32> {
    let lanes = simd_lanes();
    assert_eq!(a.len(), lanes);
    assert_eq!(b.len(), lanes);
    let mut out = vec![0u32; lanes];
    vxor_slice(a, b, &mut out);
    out
}

/// Bitwise OR using the host SIMD width.
pub fn vor_auto(a: &[u32], b: &[u32]) -> Vec<u32> {
    let lanes = simd_lanes();
    assert_eq!(a.len(), lanes);
    assert_eq!(b.len(), lanes);
    let mut out = vec![0u32; lanes];
    vor_slice(a, b, &mut out);
    out
}

/// Rotate each 32-bit lane left using the host SIMD width.
pub fn vrot32_auto(a: &[u32], k: u32) -> Vec<u32> {
    let lanes = simd_lanes();
    assert_eq!(a.len(), lanes);
    let mut out = vec![0u32; lanes];
    vrot32_slice(a, k, &mut out);
    out
}

/// Lane-wise 32-bit addition of two vectors.
pub fn vadd32(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    #[cfg(target_os = "macos")]
    if let Some(res) = metal_vadd32(a, b) {
        return res;
    }
    #[cfg(feature = "cuda")]
    if let Some(res) = crate::cuda::vadd32_cuda(&a, &b) {
        if res.len() == 4 {
            return [res[0], res[1], res[2], res[3]];
        }
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        match simd_choice() {
            SimdChoice::Avx512 | SimdChoice::Avx2 => {
                use std::arch::x86_64::*;
                let va = _mm256_castsi128_si256(_mm_loadu_si128(a.as_ptr() as *const __m128i));
                let vb = _mm256_castsi128_si256(_mm_loadu_si128(b.as_ptr() as *const __m128i));
                let vr = _mm256_add_epi32(va, vb);
                let out128 = _mm256_castsi256_si128(vr);
                return std::mem::transmute(out128);
            }
            SimdChoice::Sse2 => {
                use std::arch::x86_64::*;
                let va = _mm_loadu_si128(a.as_ptr() as *const __m128i);
                let vb = _mm_loadu_si128(b.as_ptr() as *const __m128i);
                let vr = _mm_add_epi32(va, vb);
                return std::mem::transmute(vr);
            }
            _ => {}
        }
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        if matches!(simd_choice(), SimdChoice::Neon) {
            use std::arch::aarch64::*;
            let va = vld1q_u32(a.as_ptr());
            let vb = vld1q_u32(b.as_ptr());
            let vr = vaddq_u32(va, vb);
            let mut out = [0u32; 4];
            vst1q_u32(out.as_mut_ptr(), vr);
            return out;
        }
    }
    [
        a[0].wrapping_add(b[0]),
        a[1].wrapping_add(b[1]),
        a[2].wrapping_add(b[2]),
        a[3].wrapping_add(b[3]),
    ]
}

/// Lane-wise 64-bit addition (two lanes).
pub fn vadd64(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    #[cfg(target_os = "macos")]
    if let Some(res) = metal_vadd64(a, b) {
        return res;
    }

    #[cfg(feature = "cuda")]
    {
        let a64 = [
            (a[0] as u64) | ((a[1] as u64) << 32),
            (a[2] as u64) | ((a[3] as u64) << 32),
        ];
        let b64 = [
            (b[0] as u64) | ((b[1] as u64) << 32),
            (b[2] as u64) | ((b[3] as u64) << 32),
        ];
        if let Some(res) = crate::cuda::vadd64_cuda(&a64, &b64) {
            if res.len() == 2 {
                let r0 = res[0];
                let r1 = res[1];
                return [
                    (r0 & 0xffff_ffff) as u32,
                    (r0 >> 32) as u32,
                    (r1 & 0xffff_ffff) as u32,
                    (r1 >> 32) as u32,
                ];
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    unsafe {
        match simd_choice() {
            SimdChoice::Avx512 | SimdChoice::Avx2 => {
                use std::arch::x86_64::*;
                let va = _mm256_castsi128_si256(_mm_loadu_si128(a.as_ptr() as *const __m128i));
                let vb = _mm256_castsi128_si256(_mm_loadu_si128(b.as_ptr() as *const __m128i));
                let vr = _mm256_add_epi64(va, vb);
                let out128 = _mm256_castsi256_si128(vr);
                return std::mem::transmute(out128);
            }
            SimdChoice::Sse2 => {
                use std::arch::x86_64::*;
                let va = _mm_loadu_si128(a.as_ptr() as *const __m128i);
                let vb = _mm_loadu_si128(b.as_ptr() as *const __m128i);
                let vr = _mm_add_epi64(va, vb);
                return std::mem::transmute(vr);
            }
            _ => {}
        }
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        if matches!(simd_choice(), SimdChoice::Neon) {
            use std::arch::aarch64::*;
            // Treat pairs of u32 as u64 lanes via reinterpret
            let va32 = vld1q_u32(a.as_ptr());
            let vb32 = vld1q_u32(b.as_ptr());
            let va64 = vreinterpretq_u64_u32(va32);
            let vb64 = vreinterpretq_u64_u32(vb32);
            let sum64 = vaddq_u64(va64, vb64);
            let sum32 = vreinterpretq_u32_u64(sum64);
            let mut out = [0u32; 4];
            vst1q_u32(out.as_mut_ptr(), sum32);
            return out;
        }
    }

    let a0 = (a[0] as u64) | ((a[1] as u64) << 32);
    let a1 = (a[2] as u64) | ((a[3] as u64) << 32);
    let b0 = (b[0] as u64) | ((b[1] as u64) << 32);
    let b1 = (b[2] as u64) | ((b[3] as u64) << 32);
    let r0 = a0.wrapping_add(b0);
    let r1 = a1.wrapping_add(b1);
    [
        (r0 & 0xffff_ffff) as u32,
        (r0 >> 32) as u32,
        (r1 & 0xffff_ffff) as u32,
        (r1 >> 32) as u32,
    ]
}

/// Bitwise AND of two vectors.
pub fn vand(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    #[cfg(all(target_os = "macos", feature = "metal"))]
    if let Some(res) = metal_vand(a, b) {
        return res;
    }
    #[cfg(feature = "cuda")]
    if let Some(res) = crate::cuda::vand_cuda(&a, &b) {
        if res.len() == 4 {
            return [res[0], res[1], res[2], res[3]];
        }
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        match simd_choice() {
            SimdChoice::Avx512 | SimdChoice::Avx2 => {
                use std::arch::x86_64::*;
                let va = _mm256_castsi128_si256(_mm_loadu_si128(a.as_ptr() as *const __m128i));
                let vb = _mm256_castsi128_si256(_mm_loadu_si128(b.as_ptr() as *const __m128i));
                let vr = _mm256_and_si256(va, vb);
                let out128 = _mm256_castsi256_si128(vr);
                return std::mem::transmute(out128);
            }
            SimdChoice::Sse2 => {
                use std::arch::x86_64::*;
                let va = _mm_loadu_si128(a.as_ptr() as *const __m128i);
                let vb = _mm_loadu_si128(b.as_ptr() as *const __m128i);
                let vr = _mm_and_si128(va, vb);
                return std::mem::transmute(vr);
            }
            _ => {}
        }
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        if matches!(simd_choice(), SimdChoice::Neon) {
            use std::arch::aarch64::*;
            let va = vld1q_u32(a.as_ptr());
            let vb = vld1q_u32(b.as_ptr());
            let vr = vandq_u32(va, vb);
            let mut out = [0u32; 4];
            vst1q_u32(out.as_mut_ptr(), vr);
            return out;
        }
    }
    [a[0] & b[0], a[1] & b[1], a[2] & b[2], a[3] & b[3]]
}

/// Bitwise XOR of two vectors.
pub fn vxor(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    #[cfg(all(target_os = "macos", feature = "metal"))]
    if let Some(res) = metal_vxor(a, b) {
        return res;
    }
    #[cfg(feature = "cuda")]
    if let Some(res) = crate::cuda::vxor_cuda(&a, &b) {
        if res.len() == 4 {
            return [res[0], res[1], res[2], res[3]];
        }
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        match simd_choice() {
            SimdChoice::Avx512 | SimdChoice::Avx2 => {
                use std::arch::x86_64::*;
                let va = _mm256_castsi128_si256(_mm_loadu_si128(a.as_ptr() as *const __m128i));
                let vb = _mm256_castsi128_si256(_mm_loadu_si128(b.as_ptr() as *const __m128i));
                let vr = _mm256_xor_si256(va, vb);
                let out128 = _mm256_castsi256_si128(vr);
                return std::mem::transmute(out128);
            }
            SimdChoice::Sse2 => {
                use std::arch::x86_64::*;
                let va = _mm_loadu_si128(a.as_ptr() as *const __m128i);
                let vb = _mm_loadu_si128(b.as_ptr() as *const __m128i);
                let vr = _mm_xor_si128(va, vb);
                return std::mem::transmute(vr);
            }
            _ => {}
        }
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        if matches!(simd_choice(), SimdChoice::Neon) {
            use std::arch::aarch64::*;
            let va = vld1q_u32(a.as_ptr());
            let vb = vld1q_u32(b.as_ptr());
            let vr = veorq_u32(va, vb);
            let mut out = [0u32; 4];
            vst1q_u32(out.as_mut_ptr(), vr);
            return out;
        }
    }
    [a[0] ^ b[0], a[1] ^ b[1], a[2] ^ b[2], a[3] ^ b[3]]
}

/// Bitwise OR of two vectors.
pub fn vor(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    #[cfg(all(target_os = "macos", feature = "metal"))]
    if let Some(res) = metal_vor(a, b) {
        return res;
    }
    #[cfg(feature = "cuda")]
    if let Some(res) = crate::cuda::vor_cuda(&a, &b) {
        if res.len() == 4 {
            return [res[0], res[1], res[2], res[3]];
        }
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        match simd_choice() {
            SimdChoice::Avx512 | SimdChoice::Avx2 => {
                use std::arch::x86_64::*;
                let va = _mm256_castsi128_si256(_mm_loadu_si128(a.as_ptr() as *const __m128i));
                let vb = _mm256_castsi128_si256(_mm_loadu_si128(b.as_ptr() as *const __m128i));
                let vr = _mm256_or_si256(va, vb);
                let out128 = _mm256_castsi256_si128(vr);
                return std::mem::transmute(out128);
            }
            SimdChoice::Sse2 => {
                use std::arch::x86_64::*;
                let va = _mm_loadu_si128(a.as_ptr() as *const __m128i);
                let vb = _mm_loadu_si128(b.as_ptr() as *const __m128i);
                let vr = _mm_or_si128(va, vb);
                return std::mem::transmute(vr);
            }
            _ => {}
        }
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        if matches!(simd_choice(), SimdChoice::Neon) {
            use std::arch::aarch64::*;
            let va = vld1q_u32(a.as_ptr());
            let vb = vld1q_u32(b.as_ptr());
            let vr = vorrq_u32(va, vb);
            let mut out = [0u32; 4];
            vst1q_u32(out.as_mut_ptr(), vr);
            return out;
        }
    }
    [a[0] | b[0], a[1] | b[1], a[2] | b[2], a[3] | b[3]]
}

/// Rotate each 32-bit lane left by `k` bits.
pub fn vrot32(a: [u32; 4], k: u32) -> [u32; 4] {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        let k = k & 31;
        match simd_choice() {
            SimdChoice::Avx512 | SimdChoice::Avx2 => {
                use std::arch::x86_64::*;
                let va = _mm256_castsi128_si256(_mm_loadu_si128(a.as_ptr() as *const __m128i));
                let sh = _mm_cvtsi32_si128(k as i32);
                let left = _mm256_sll_epi32(va, sh);
                let right = _mm256_srl_epi32(va, _mm_cvtsi32_si128((32 - k) as i32));
                let vr = _mm256_or_si256(left, right);
                let out128 = _mm256_castsi256_si128(vr);
                return std::mem::transmute(out128);
            }
            SimdChoice::Sse2 => {
                use std::arch::x86_64::*;
                let va = _mm_loadu_si128(a.as_ptr() as *const __m128i);
                let sh = _mm_cvtsi32_si128(k as i32);
                let left = _mm_sll_epi32(va, sh);
                let right = _mm_srl_epi32(va, _mm_cvtsi32_si128((32 - k) as i32));
                let vr = _mm_or_si128(left, right);
                return std::mem::transmute(vr);
            }
            _ => {}
        }
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        let k = k & 31;
        if matches!(simd_choice(), SimdChoice::Neon) {
            use std::arch::aarch64::*;
            let va = vld1q_u32(a.as_ptr());
            let sh_left = vdupq_n_s32(k as i32);
            // Negative shift count performs logical right-shift for unsigned lanes
            let sh_right = vdupq_n_s32(-((32 - k) as i32));
            let left = vshlq_u32(va, sh_left);
            let right = vshlq_u32(va, sh_right);
            let vr = vorrq_u32(left, right);
            let mut out = [0u32; 4];
            vst1q_u32(out.as_mut_ptr(), vr);
            return out;
        }
    }
    [
        a[0].rotate_left(k),
        a[1].rotate_left(k),
        a[2].rotate_left(k),
        a[3].rotate_left(k),
    ]
}

// Expose a small toggle to force-enable/disable Metal in tests and tooling.
#[cfg(all(target_os = "macos", feature = "metal"))]
pub fn set_metal_enabled(enabled: bool) {
    METAL_CONFIG_ENABLED.store(enabled, Ordering::SeqCst);
    METAL_FORCED_DISABLED.store(!enabled, Ordering::SeqCst);
    if !enabled {
        set_metal_status_message(Some("disabled by configuration".to_owned()));
        release_metal_state();
    } else {
        set_metal_status_message(None);
    }
}

#[cfg(all(target_os = "macos", feature = "metal"))]
#[doc(hidden)]
pub fn reset_metal_backend_for_tests() {
    METAL_DISABLED.store(false, Ordering::SeqCst);
    METAL_FORCED_DISABLED.store(false, Ordering::SeqCst);
    METAL_CONFIG_ENABLED.store(true, Ordering::SeqCst);
    release_metal_state();
    set_metal_status_message(None);
}

#[cfg(not(all(target_os = "macos", feature = "metal")))]
#[doc(hidden)]
pub fn reset_metal_backend_for_tests() {}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn metal_acceleration_speed() {
        if !metal_available() {
            return;
        }
        let a = [1u32, 2, 3, 4];
        let b = [4u32, 3, 2, 1];
        const ITER: usize = 100_000;

        let mut out = [0u32; 4];
        let start = Instant::now();
        for _ in 0..ITER {
            out[0] = std::hint::black_box(a[0].wrapping_add(b[0]));
            out[1] = std::hint::black_box(a[1].wrapping_add(b[1]));
            out[2] = std::hint::black_box(a[2].wrapping_add(b[2]));
            out[3] = std::hint::black_box(a[3].wrapping_add(b[3]));
            std::hint::black_box(out);
        }
        let cpu_time = start.elapsed();

        let start = Instant::now();
        for _ in 0..ITER {
            std::hint::black_box(vadd32(a, b));
        }
        let metal_time = start.elapsed();

        if metal_time >= cpu_time {
            eprintln!(
                "Metal path slower than CPU: cpu {cpu_time:?}, metal {metal_time:?}; skipping speed assertion",
            );
            return;
        }

        assert!(
            metal_time.as_secs_f64() * 1.10 < cpu_time.as_secs_f64(),
            "Metal path must be >10% faster: cpu {cpu_time:?}, metal {metal_time:?}"
        );
    }

    #[cfg(all(target_os = "macos", feature = "metal"))]
    #[test]
    fn metal_sha256_leaves_matches_cpu() {
        if !metal_available() {
            eprintln!("Metal unavailable; skipping sha256 leaves parity test");
            return;
        }

        reset_metal_backend_for_tests();
        warm_up_metal();

        let mut block_a = [0u8; 64];
        block_a[0] = b'a';
        block_a[1] = b'b';
        block_a[2] = b'c';
        block_a[3] = 0x80;
        block_a[63] = 24;

        let mut block_b = [0u8; 64];
        block_b[0] = b'n';
        block_b[1] = b'o';
        block_b[2] = b'r';
        block_b[3] = b'i';
        block_b[4] = b't';
        block_b[5] = b'o';
        block_b[6] = 0x80;
        block_b[63] = 48;

        let blocks = [block_a, block_b];
        let expected: Vec<[u8; 32]> = blocks
            .iter()
            .map(|block| {
                let mut state = [
                    0x6a09e667u32,
                    0xbb67ae85,
                    0x3c6ef372,
                    0xa54ff53a,
                    0x510e527f,
                    0x9b05688c,
                    0x1f83d9ab,
                    0x5be0cd19,
                ];
                sha256_compress_scalar_ref(&mut state, block);
                let mut digest = [0u8; 32];
                for (index, word) in state.iter().enumerate() {
                    digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
                }
                digest
            })
            .collect();

        let Some(actual) = metal_sha256_leaves(&blocks) else {
            eprintln!(
                "Metal backend unavailable after startup self-tests; skipping sha256 leaves parity test",
            );
            return;
        };
        assert_eq!(actual, expected);
    }

    #[cfg(all(target_os = "macos", feature = "metal"))]
    #[test]
    fn metal_sha256_pairs_reduce_matches_cpu() {
        fn cpu_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
            let mut state = [
                0x6a09e667u32,
                0xbb67ae85,
                0x3c6ef372,
                0xa54ff53a,
                0x510e527f,
                0x9b05688c,
                0x1f83d9ab,
                0x5be0cd19,
            ];
            let mut block = [0u8; 64];
            block[..32].copy_from_slice(left);
            block[32..].copy_from_slice(right);
            sha256_compress_scalar_ref(&mut state, &block);
            let mut pad = [0u8; 64];
            pad[0] = 0x80;
            pad[62] = 0x02;
            pad[63] = 0x00;
            sha256_compress_scalar_ref(&mut state, &pad);
            let mut out = [0u8; 32];
            for (index, word) in state.iter().enumerate() {
                out[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
            }
            out
        }

        if !metal_available() {
            eprintln!("Metal unavailable; skipping sha256 pair-reduce parity test");
            return;
        }

        reset_metal_backend_for_tests();

        let mut d0 = [0u8; 32];
        let mut d1 = [0u8; 32];
        let mut d2 = [0u8; 32];
        for (index, byte) in d0.iter_mut().enumerate() {
            *byte = index as u8;
        }
        for (index, byte) in d1.iter_mut().enumerate() {
            *byte = 0x40 + index as u8;
        }
        for (index, byte) in d2.iter_mut().enumerate() {
            *byte = 0x80 + index as u8;
        }
        let digests = [d0, d1, d2];
        let first = cpu_pair(&digests[0], &digests[1]);
        let expected = cpu_pair(&first, &digests[2]);

        assert_eq!(metal_sha256_pairs_reduce(&digests), Some(expected));
    }

    #[cfg(all(target_os = "macos", feature = "metal"))]
    #[test]
    fn metal_ed25519_batch_matches_cpu() {
        use curve25519_dalek::{
            constants::{ED25519_BASEPOINT_COMPRESSED, ED25519_BASEPOINT_POINT},
            edwards::{CompressedEdwardsY, EdwardsPoint},
            scalar::Scalar,
        };
        use ed25519_dalek::{Signer, SigningKey};

        if !metal_available() {
            eprintln!("Metal unavailable; skipping ed25519 batch parity test");
            return;
        }

        reset_metal_backend_for_tests();
        let key = SigningKey::from_bytes(&[5u8; 32]);
        let pk = key.verifying_key();
        let msg = b"metal batch test";
        let sig = key.sign(msg).to_bytes();
        let hram = crate::signature::ed25519_challenge_scalar_bytes(&sig, pk.as_bytes(), msg);

        let mut bad_sig = sig;
        bad_sig[0] ^= 0x40;

        let sigs = [sig, bad_sig];
        let pks = [pk.to_bytes(), pk.to_bytes()];
        let hrams = [hram, hram];
        let points = [pk.to_bytes(), *ED25519_BASEPOINT_COMPRESSED.as_bytes()];
        let good_r: [u8; 32] = sig[..32]
            .try_into()
            .expect("signature R slice has 32 bytes");
        let good_s: [u8; 32] = sig[32..]
            .try_into()
            .expect("signature S slice has 32 bytes");
        let a_point = CompressedEdwardsY(pk.to_bytes())
            .decompress()
            .expect("public key should decompress on CPU");
        let s_scalar =
            Scalar::from_canonical_bytes(good_s).expect("signature scalar should be canonical");
        let h_scalar =
            Scalar::from_canonical_bytes(hram).expect("challenge scalar should be canonical");
        let cpu_s_only = EdwardsPoint::vartime_double_scalar_mul_basepoint(
            &Scalar::ZERO,
            &(-a_point),
            &s_scalar,
        )
        .compress()
        .to_bytes();
        let cpu_h_only = EdwardsPoint::vartime_double_scalar_mul_basepoint(
            &h_scalar,
            &(-a_point),
            &Scalar::ZERO,
        )
        .compress()
        .to_bytes();
        let mut s_only_sig = [0u8; 64];
        s_only_sig[32..].copy_from_slice(&good_s);
        let h_only_sig = [0u8; 64];
        let mut unit_sig = [0u8; 64];
        unit_sig[32] = 1;
        let mut double_sig = [0u8; 64];
        double_sig[32] = 2;
        let mut identity = [0u8; 32];
        identity[0] = 1;
        let metal_s_only =
            metal_ed25519_check_bytes_for_tests(&[s_only_sig], &[identity], &[[0u8; 32]]);
        let metal_h_only =
            metal_ed25519_check_bytes_for_tests(&[h_only_sig], &[pk.to_bytes()], &[hram]);
        let metal_b1 = metal_ed25519_check_bytes_for_tests(&[unit_sig], &[identity], &[[0u8; 32]]);
        let metal_b2 =
            metal_ed25519_check_bytes_for_tests(&[double_sig], &[identity], &[[0u8; 32]]);
        let cpu_b1 = ED25519_BASEPOINT_COMPRESSED.to_bytes();
        let cpu_b2 = (Scalar::from(2u64) * ED25519_BASEPOINT_POINT)
            .compress()
            .to_bytes();
        let pow2_sigs: Vec<[u8; 64]> = (0..64)
            .map(|bit| {
                let mut sig_bytes = [0u8; 64];
                sig_bytes[32 + bit / 8] = 1u8 << (bit % 8);
                sig_bytes
            })
            .collect();
        let pow2_pks = vec![identity; 64];
        let pow2_hrams = vec![[0u8; 32]; 64];
        let metal_pow2 = metal_ed25519_check_bytes_for_tests(&pow2_sigs, &pow2_pks, &pow2_hrams);
        let pow2_first_mismatch = metal_pow2.as_ref().and_then(|metal_points| {
            metal_points.iter().enumerate().find_map(|(bit, actual)| {
                let mut scalar_bytes = [0u8; 32];
                scalar_bytes[bit / 8] = 1u8 << (bit % 8);
                let scalar = Scalar::from_canonical_bytes(scalar_bytes)
                    .expect("power-of-two scalar should be canonical");
                let expected = (scalar * ED25519_BASEPOINT_POINT).compress().to_bytes();
                (expected != *actual).then_some((bit, expected, *actual))
            })
        });

        let direct = metal_ed25519_verify_batch_direct_for_tests(&sigs, &pks, &hrams);
        let status = metal_ed25519_status_batch_direct_for_tests(&sigs, &pks, &hrams);
        let check_bytes = metal_ed25519_check_bytes_for_tests(&sigs, &pks, &hrams);
        let field_roundtrip = metal_ed25519_field_roundtrip_for_tests(&points);
        let point_decompress = metal_ed25519_point_decompress_for_tests(&points);

        let Some(field_roundtrip) = field_roundtrip else {
            panic!("Metal field roundtrip diagnostic kernel should return results");
        };
        for (expected, actual) in points.iter().zip(field_roundtrip.iter()) {
            let mut expected_y = *expected;
            expected_y[31] &= 0x7f;
            assert_eq!(
                *actual, expected_y,
                "Metal fe_frombytes/fe_tobytes should preserve the encoded y-coordinate",
            );
        }
        let Some((point_statuses, point_outputs)) = point_decompress else {
            panic!("Metal point decompress diagnostic kernel should return results");
        };
        let expected_neg_points: Vec<[u8; 32]> = points
            .iter()
            .map(|point| {
                let mut negated = *point;
                negated[31] ^= 0x80;
                negated
            })
            .collect();
        assert_eq!(
            point_statuses,
            vec![5, 5],
            "Metal point decompress should accept the public key and basepoint",
        );
        assert_eq!(
            point_outputs, expected_neg_points,
            "Metal ge_frombytes_negate_vartime should negate the encoded point sign",
        );
        assert_eq!(
            direct,
            Some(vec![true, false]),
            "Metal signature kernel should match the CPU on good and bad signatures",
        );
        assert_eq!(
            status,
            Some(vec![5, 4]),
            "Metal signature status kernel should accept the good signature and reject the tampered one",
        );
        assert_eq!(
            check_bytes,
            Some(vec![good_r, good_r]),
            "Metal signature check-bytes kernel should reconstruct the canonical R value",
        );
        assert_eq!(
            metal_s_only,
            Some(vec![cpu_s_only]),
            "Metal [s]B multiplication should match the CPU",
        );
        assert_eq!(
            metal_h_only,
            Some(vec![cpu_h_only]),
            "Metal [h](-A) multiplication should match the CPU",
        );
        assert_eq!(
            metal_b1,
            Some(vec![cpu_b1]),
            "Metal scalar-1 basepoint multiplication should produce the canonical basepoint",
        );
        assert_eq!(
            metal_b2,
            Some(vec![cpu_b2]),
            "Metal scalar-2 basepoint multiplication should match the CPU",
        );
        assert_eq!(
            pow2_first_mismatch, None,
            "Metal power-of-two basepoint multiplications should match the CPU across the scalar bit range",
        );

        reset_metal_backend_for_tests();
        assert!(
            metal_available(),
            "Metal backend should initialize on Metal-capable hosts",
        );
        assert!(
            with_metal_state_try(|ctx| Some(ctx.ed25519_signature.is_some())).unwrap_or(false),
            "Metal startup self-test should keep the ed25519 signature pipeline enabled",
        );
        assert_eq!(
            metal_ed25519_verify_batch(&sigs, &pks, &hrams),
            Some(vec![true, false]),
            "Metal ed25519 batch verification should stay on the accelerator after the self-test passes",
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_metal_sha256_compress_returns_true() {
        if !metal_available() {
            eprintln!("No Metal GPU available; skipping test");
            return;
        }
        let mut state = [
            0x6a09e667u32,
            0xbb67ae85,
            0x3c6ef372,
            0xa54ff53a,
            0x510e527f,
            0x9b05688c,
            0x1f83d9ab,
            0x5be0cd19,
        ];
        let block = [0u8; 64];
        assert!(metal_sha256_compress(&mut state, &block));
    }

    #[cfg(all(target_os = "macos", feature = "metal"))]
    #[test]
    fn metal_vadd32_single_vector_matches_scalar() {
        if !metal_available() {
            eprintln!("No Metal GPU available; skipping test");
            return;
        }
        reset_metal_backend_for_tests();
        let a = [0xFFFF_FFFFu32, 0x0000_0001, 0x1234_5678, 0x9ABC_DEF0];
        let b = [0x0000_0001u32, 0x0000_0001, 0x8765_4321, 0x1111_1111];
        let expected = [
            a[0].wrapping_add(b[0]),
            a[1].wrapping_add(b[1]),
            a[2].wrapping_add(b[2]),
            a[3].wrapping_add(b[3]),
        ];
        assert_eq!(metal_vadd32(a, b), Some(expected));
    }

    #[cfg(all(target_os = "macos", feature = "metal"))]
    #[test]
    fn metal_vadd64_single_vector_matches_scalar() {
        if !metal_available() {
            eprintln!("No Metal GPU available; skipping test");
            return;
        }
        reset_metal_backend_for_tests();
        let a = [0xFFFF_FFFFu32, 0x0000_0000, 0x0000_0001, 0x8000_0000];
        let b = [0x0000_0001u32, 0x0000_0001, 0xFFFF_FFFF, 0x7FFF_FFFF];
        let lane0 =
            ((a[1] as u64) << 32 | a[0] as u64).wrapping_add(((b[1] as u64) << 32) | b[0] as u64);
        let lane1 =
            ((a[3] as u64) << 32 | a[2] as u64).wrapping_add(((b[3] as u64) << 32) | b[2] as u64);
        let expected = [
            (lane0 & 0xFFFF_FFFF) as u32,
            (lane0 >> 32) as u32,
            (lane1 & 0xFFFF_FFFF) as u32,
            (lane1 >> 32) as u32,
        ];
        assert_eq!(metal_vadd64(a, b), Some(expected));
    }

    #[cfg(all(target_os = "macos", feature = "metal"))]
    #[test]
    fn metal_bitwise_single_vector_matches_scalar() {
        if !metal_available() {
            eprintln!("No Metal GPU available; skipping test");
            return;
        }
        reset_metal_backend_for_tests();
        let lhs = [0xffff_0000u32, 0x1234_5678, 0x0f0f_0f0f, 0xaaaa_5555];
        let rhs = [0x00ff_ff00u32, 0xf0f0_f0f0, 0x3333_cccc, 0x5555_aaaa];
        assert_eq!(
            metal_vand(lhs, rhs),
            Some([
                lhs[0] & rhs[0],
                lhs[1] & rhs[1],
                lhs[2] & rhs[2],
                lhs[3] & rhs[3],
            ]),
        );
        assert_eq!(
            metal_vxor(lhs, rhs),
            Some([
                lhs[0] ^ rhs[0],
                lhs[1] ^ rhs[1],
                lhs[2] ^ rhs[2],
                lhs[3] ^ rhs[3],
            ]),
        );
        assert_eq!(
            metal_vor(lhs, rhs),
            Some([
                lhs[0] | rhs[0],
                lhs[1] | rhs[1],
                lhs[2] | rhs[2],
                lhs[3] | rhs[3],
            ]),
        );
    }

    #[cfg(all(target_os = "macos", feature = "metal"))]
    #[test]
    fn metal_aes_batch_matches_scalar() {
        if !metal_available() {
            eprintln!("No Metal GPU available; skipping test");
            return;
        }
        reset_metal_backend_for_tests();
        let states = [
            [
                0x00u8, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                0xdd, 0xee, 0xff,
            ],
            [
                0xffu8, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x99, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33,
                0x22, 0x11, 0x00,
            ],
        ];
        let rk = [
            0x0f, 0x15, 0x71, 0xc9, 0x47, 0xd9, 0xe8, 0x59, 0x0c, 0xb7, 0xad, 0xd6, 0xaf, 0x7f,
            0x67, 0x98,
        ];
        let expected_enc: Vec<[u8; 16]> = states
            .iter()
            .map(|&state| crate::aes::aesenc_impl(state, rk))
            .collect();
        let expected_dec: Vec<[u8; 16]> = states
            .iter()
            .map(|&state| crate::aes::aesdec_impl(state, rk))
            .collect();
        assert_eq!(metal_aesenc_batch(&states, rk), Some(expected_enc));
        assert_eq!(metal_aesdec_batch(&states, rk), Some(expected_dec));
    }

    #[cfg(all(target_os = "macos", feature = "metal"))]
    #[test]
    fn metal_aes_rounds_batch_matches_scalar() {
        if !metal_available() {
            eprintln!("No Metal GPU available; skipping test");
            return;
        }

        reset_metal_backend_for_tests();

        let states = [
            [
                0x00u8, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                0xdd, 0xee, 0xff,
            ],
            [
                0xffu8, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x99, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33,
                0x22, 0x11, 0x00,
            ],
        ];
        let round_keys = [
            [
                0x0f, 0x15, 0x71, 0xc9, 0x47, 0xd9, 0xe8, 0x59, 0x0c, 0xb7, 0xad, 0xd6, 0xaf, 0x7f,
                0x67, 0x98,
            ],
            [
                0xa5, 0x40, 0x76, 0x28, 0x10, 0x4f, 0xdc, 0xe6, 0x43, 0xdd, 0x27, 0x0f, 0x6c, 0xa7,
                0x63, 0x6f,
            ],
            [
                0x2c, 0x5e, 0x2f, 0x88, 0x6a, 0x84, 0xd2, 0x57, 0x8b, 0x3f, 0x8c, 0x9c, 0x4f, 0x11,
                0x64, 0x15,
            ],
        ];
        let expected_enc: Vec<[u8; 16]> = states
            .iter()
            .copied()
            .map(|state| {
                round_keys
                    .iter()
                    .copied()
                    .fold(state, |block, rk| crate::aes::aesenc_impl(block, rk))
            })
            .collect();
        let expected_dec: Vec<[u8; 16]> = states
            .iter()
            .copied()
            .map(|state| {
                round_keys
                    .iter()
                    .copied()
                    .fold(state, |block, rk| crate::aes::aesdec_impl(block, rk))
            })
            .collect();

        assert_eq!(
            metal_aesenc_rounds_batch(&states, &round_keys),
            Some(expected_enc)
        );
        assert_eq!(
            metal_aesdec_rounds_batch(&states, &round_keys),
            Some(expected_dec)
        );
    }

    #[cfg(all(target_os = "macos", feature = "metal"))]
    #[test]
    fn device_selector_prefers_non_headless_perf_device() {
        use super::{DeviceTraits, select_device_index};
        let traits = [
            DeviceTraits {
                headless: false,
                low_power: true,
            },
            DeviceTraits {
                headless: false,
                low_power: false,
            },
        ];
        assert_eq!(select_device_index(&traits), Some(1));
    }

    #[cfg(all(target_os = "macos", feature = "metal"))]
    #[test]
    fn device_selector_falls_back_to_first_non_headless() {
        use super::{DeviceTraits, select_device_index};
        let traits = [
            DeviceTraits {
                headless: false,
                low_power: true,
            },
            DeviceTraits {
                headless: true,
                low_power: false,
            },
        ];
        assert_eq!(select_device_index(&traits), Some(0));
    }

    #[cfg(all(target_os = "macos", feature = "metal"))]
    #[test]
    fn device_selector_handles_all_headless_devices() {
        use super::{DeviceTraits, select_device_index};
        let traits = [
            DeviceTraits {
                headless: true,
                low_power: false,
            },
            DeviceTraits {
                headless: true,
                low_power: true,
            },
        ];
        assert_eq!(select_device_index(&traits), Some(0));
        let empty: [DeviceTraits; 0] = [];
        assert_eq!(select_device_index(&empty), None);
    }

    #[cfg(not(all(target_os = "macos", feature = "metal")))]
    #[test]
    fn warm_up_metal_is_noop_on_non_metal_targets() {
        warm_up_metal();
        warm_up_metal();
    }

    #[cfg(all(target_os = "macos", feature = "metal"))]
    #[test]
    fn warm_up_metal_reuses_cached_state() {
        if !metal_runtime_allowed() {
            eprintln!("Metal runtime disabled; skipping reuse assertion");
            return;
        }
        reset_metal_backend_for_tests();
        let first = METAL_STATE.with(|cell| {
            cell.borrow_mut().take();
            with_metal_state(|state| objc2::rc::Retained::as_ptr(&state.queue) as usize)
        });
        let second = with_metal_state(|state| objc2::rc::Retained::as_ptr(&state.queue) as usize);

        match (first, second) {
            (Some(a), Some(b)) => assert_eq!(a, b, "warm_up_metal should reuse cached Metal state"),
            _ => eprintln!("Metal state unavailable; skipping reuse assertion"),
        }
    }
}
