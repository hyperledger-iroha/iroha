use core::convert::TryFrom;
#[cfg(windows)]
use std::env;
#[cfg(windows)]
use std::path::PathBuf;
use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    process::{Command, Stdio},
    sync::{Arc, Mutex, MutexGuard, OnceLock, RwLock, TryLockError},
};

use fastpq_isi::StarkParameterSet;
use iroha_crypto::Hash;
#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
use metal::{Device, MTLDeviceLocation};
use rayon::prelude::*;

use crate::{
    Error, Result, TransitionBatch,
    fft::Planner,
    overrides, pack_bytes,
    poseidon::{self, PoseidonSponge},
    proof::PublicIO,
    trace::{
        PoseidonPipelinePolicy, build_trace, column_index, derive_polynomial_data,
        hash_columns_from_coefficients, merkle_root, merkle_root_with_first_level,
    },
};

const GOLDILOCKS_MODULUS: u64 = 0xffff_ffff_0000_0001;
const FIELD_ONE: u64 = 1;
const LDE_LEAF_DOMAIN: &[u8] = b"fastpq:v1:lde:leaf";
const TRACE_NODE_DOMAIN: &[u8] = b"fastpq:v1:trace:node";
pub const LOOKUP_PRODUCT_DOMAIN: &str = "fastpq:v1:lookup:product";
const FRI_FINAL_DOMAIN: &str = "fastpq:v1:fri:final";
#[cfg(target_os = "macos")]
const METAL_FRAMEWORK: &str = "/System/Library/Frameworks/Metal.framework/Metal";
#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
static DEBUG_METAL_ENUM_ENV: OnceLock<bool> = OnceLock::new();

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
fn warm_up_core_graphics_display() {}

static GPU_BACKEND: OnceLock<Option<GpuBackend>> = OnceLock::new();
static GPU_OVERRIDE: OnceLock<GpuOverride> = OnceLock::new();
static AUTO_RESOLVED_MODE: OnceLock<ExecutionMode> = OnceLock::new();
static GPU_WORKLOAD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

type ExecutionModeObserver =
    dyn Fn(ExecutionMode, ExecutionMode, Option<GpuBackend>) + Send + Sync + 'static;

static EXECUTION_MODE_OBSERVER: OnceLock<RwLock<Option<Arc<ExecutionModeObserver>>>> =
    OnceLock::new();

pub const TRANSCRIPT_TAG_INIT: &str = "fastpq:v1:init";
pub const TRANSCRIPT_TAG_ROOTS: &str = "fastpq:v1:roots";
pub const TRANSCRIPT_TAG_GAMMA: &str = "fastpq:v1:gamma";
pub const TRANSCRIPT_TAG_ALPHA_PREFIX: &str = "fastpq:v1:alpha";
pub const TRANSCRIPT_TAG_QUERY_INDEX: &str = "fastpq:v1:query_index";
pub const TRANSCRIPT_TAG_BETA_PREFIX: &str = "fastpq:v1:beta";
pub const TRANSCRIPT_TAG_FRI_LAYER_PREFIX: &str = "fastpq:v1:fri_layer";

/// Configuration for the FASTPQ backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Run the prover using the scalar CPU implementation.
    Cpu,
    /// Prefer GPU acceleration when available.
    Gpu,
    /// Detect hardware support at runtime and pick the best available mode.
    Auto,
}

/// Poseidon pipeline execution override.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoseidonExecutionMode {
    /// Follow the primary execution mode (default).
    Auto,
    /// Force CPU hashing even if FFT/LDE use the GPU.
    Cpu,
    /// Attempt GPU hashing regardless of the primary execution mode.
    Gpu,
}

impl PoseidonExecutionMode {
    /// Return the poseidon override label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
        }
    }
}

impl ExecutionMode {
    /// Return the execution mode as a lowercase label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
            Self::Auto => "auto",
        }
    }

    /// Resolve the execution mode, performing runtime detection when required.
    #[must_use]
    pub fn resolve(self) -> Self {
        let resolved = match self {
            Self::Auto => {
                if gpu_available() {
                    Self::Gpu
                } else {
                    Self::Cpu
                }
            }
            mode => mode,
        };
        log_execution_resolution(self, resolved);
        resolved
    }
}

fn gpu_workload_mutex() -> &'static Mutex<()> {
    GPU_WORKLOAD_LOCK.get_or_init(|| Mutex::new(()))
}

pub fn try_acquire_gpu_lane() -> Option<MutexGuard<'static, ()>> {
    match gpu_workload_mutex().try_lock() {
        Ok(guard) => Some(guard),
        Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
        Err(TryLockError::WouldBlock) => None,
    }
}

pub fn acquire_gpu_lane() -> MutexGuard<'static, ()> {
    match gpu_workload_mutex().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn execution_mode_observer_slot() -> &'static RwLock<Option<Arc<ExecutionModeObserver>>> {
    EXECUTION_MODE_OBSERVER.get_or_init(|| RwLock::new(None))
}

fn notify_execution_mode_observer(
    requested: ExecutionMode,
    resolved: ExecutionMode,
    backend: Option<GpuBackend>,
) {
    let observer = execution_mode_observer_slot()
        .read()
        .ok()
        .and_then(|guard| guard.clone());
    if let Some(callback) = observer {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            callback(requested, resolved, backend)
        }));
        if result.is_err() {
            tracing::warn!(
                target: "fastpq::planner",
                requested = requested.as_str(),
                resolved = resolved.as_str(),
                "execution mode observer callback panicked"
            );
        }
    }
}

/// Install a hook invoked whenever [`ExecutionMode::Auto`] resolves to a concrete mode.
pub fn set_execution_mode_observer<F>(observer: F)
where
    F: Fn(ExecutionMode, ExecutionMode, Option<GpuBackend>) + Send + Sync + 'static,
{
    let mut guard = execution_mode_observer_slot()
        .write()
        .expect("execution mode observer lock poisoned");
    *guard = Some(Arc::new(observer));
}

/// Remove the previously installed execution mode observer, if any.
pub fn clear_execution_mode_observer() {
    if let Ok(mut guard) = execution_mode_observer_slot().write() {
        guard.take();
    }
}

fn gpu_available() -> bool {
    match gpu_override() {
        GpuOverride::ForceCpu => false,
        GpuOverride::ForceGpu => {
            let backend = GPU_BACKEND.get_or_init(|| {
                let detected = detect_gpu_backend();
                if detected.is_none() {
                    tracing::warn!(
                        target: "fastpq::planner",
                        "FASTPQ_GPU=gpu requested but no accelerator was detected; falling back to CPU"
                    );
                } else {
                    log_detected_backend(detected);
                }
                detected
            });
            backend.is_some()
        }
        GpuOverride::Auto => GPU_BACKEND
            .get_or_init(|| {
                let backend = detect_gpu_backend();
                log_detected_backend(backend);
                backend
            })
            .is_some(),
    }
}

pub fn current_gpu_backend() -> Option<GpuBackend> {
    if GPU_BACKEND.get().is_none() {
        let _ = gpu_available();
    }
    GPU_BACKEND.get().and_then(|backend| *backend)
}

fn log_execution_resolution(requested: ExecutionMode, resolved: ExecutionMode) {
    let backend = current_gpu_backend();
    let backend_label = backend.map_or("none", GpuBackend::as_str);
    let planner_backend_label = backend.map_or("unknown", GpuBackend::as_str);
    tracing::info!(
        target: "telemetry::fastpq.execution_mode",
        requested = requested.as_str(),
        resolved = resolved.as_str(),
        backend = backend_label,
        "FASTPQ execution mode resolved"
    );
    notify_execution_mode_observer(requested, resolved, backend);
    if requested != ExecutionMode::Auto {
        return;
    }
    let _ = AUTO_RESOLVED_MODE.get_or_init(|| {
        match resolved {
            ExecutionMode::Gpu => {
                tracing::info!(
                    target: "fastpq::planner",
                    resolved_mode = "gpu",
                    backend = planner_backend_label,
                    "FASTPQ planner resolved to GPU execution mode"
                );
            }
            ExecutionMode::Cpu => {
                tracing::info!(
                    target: "fastpq::planner",
                    resolved_mode = "cpu",
                    "FASTPQ planner resolved to CPU execution mode; GPU acceleration unavailable"
                );
            }
            ExecutionMode::Auto => {
                tracing::debug!(
                    target: "fastpq::planner",
                    "ExecutionMode::Auto resolved without runtime detection"
                );
            }
        }
        resolved
    });
}

fn gpu_override() -> GpuOverride {
    *GPU_OVERRIDE.get_or_init(|| {
        let value = overrides::guard_env_override(|| overrides::debug_env_string("FASTPQ_GPU"));
        let parsed = value
            .as_deref()
            .and_then(parse_gpu_override)
            .unwrap_or(GpuOverride::Auto);
        if let Some(raw) = value {
            log_gpu_override(&raw, parsed);
        }
        parsed
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GpuOverride {
    Auto,
    ForceCpu,
    ForceGpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackend {
    /// CUDA runtime is available.
    Cuda,
    /// Metal device discovered (macOS).
    Metal,
    /// `OpenCL` platforms discovered.
    OpenCl,
    /// Reused GPU detection from Norito compression backend.
    Norito,
}

impl GpuBackend {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cuda => "cuda",
            Self::Metal => "metal",
            Self::OpenCl => "opencl",
            Self::Norito => "norito",
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct BackendAvailability {
    bits: u8,
}

impl BackendAvailability {
    const CUDA_BIT: u8 = 1 << 0;
    const OPENCL_BIT: u8 = 1 << 1;
    const METAL_BIT: u8 = 1 << 2;
    const NORITO_BIT: u8 = 1 << 3;

    const fn empty() -> Self {
        Self { bits: 0 }
    }

    fn with(mut self, backend: GpuBackend, available: bool) -> Self {
        if available {
            self.bits |= Self::mask(backend);
        }
        self
    }

    fn has(self, backend: GpuBackend) -> bool {
        self.bits & Self::mask(backend) != 0
    }

    const fn mask(backend: GpuBackend) -> u8 {
        match backend {
            GpuBackend::Cuda => Self::CUDA_BIT,
            GpuBackend::Metal => Self::METAL_BIT,
            GpuBackend::OpenCl => Self::OPENCL_BIT,
            GpuBackend::Norito => Self::NORITO_BIT,
        }
    }
}

fn log_detected_backend(backend: Option<GpuBackend>) {
    if let Some(kind) = backend {
        tracing::info!(
            target: "fastpq::planner",
            backend = kind.as_str(),
            "GPU backend detected"
        );
    } else {
        tracing::info!(
            target: "fastpq::planner",
            "No supported GPU backend detected; using CPU fallback"
        );
    }
}

fn log_gpu_override(raw: &str, parsed: GpuOverride) {
    match parsed {
        GpuOverride::ForceCpu => tracing::info!(
            target: "fastpq::planner",
            override_value = %raw,
            "GPU override forcing CPU execution"
        ),
        GpuOverride::ForceGpu => tracing::info!(
            target: "fastpq::planner",
            override_value = %raw,
            "GPU override forcing GPU execution"
        ),
        GpuOverride::Auto => tracing::info!(
            target: "fastpq::planner",
            override_value = %raw,
            "GPU override set to auto detection"
        ),
    }
}

fn warn_unknown_override(raw: &str) {
    tracing::warn!(
        target: "fastpq::planner",
        override_value = %raw,
        "Unknown FASTPQ_GPU override; falling back to auto detection"
    );
}

fn detect_gpu_backend() -> Option<GpuBackend> {
    let cuda = cuda_available();
    let metal = metal_available();
    let opencl = opencl_available();
    let norito = norito_gpu_available();
    let availability = BackendAvailability::empty()
        .with(GpuBackend::Cuda, cuda)
        .with(GpuBackend::OpenCl, opencl)
        .with(GpuBackend::Metal, metal)
        .with(GpuBackend::Norito, norito);
    resolve_backend(availability)
}

fn parse_gpu_override(raw: &str) -> Option<GpuOverride> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Some(GpuOverride::Auto);
    }
    match trimmed.to_ascii_lowercase().as_str() {
        "cpu" | "off" | "disable" | "disabled" | "0" => Some(GpuOverride::ForceCpu),
        "gpu" | "on" | "enable" | "enabled" | "1" => Some(GpuOverride::ForceGpu),
        "auto" | "detect" | "auto-detect" => Some(GpuOverride::Auto),
        _ => {
            warn_unknown_override(trimmed);
            None
        }
    }
}

fn resolve_backend(availability: BackendAvailability) -> Option<GpuBackend> {
    [
        GpuBackend::Cuda,
        GpuBackend::Metal,
        GpuBackend::OpenCl,
        GpuBackend::Norito,
    ]
    .into_iter()
    .find(|&backend| availability.has(backend))
}

fn norito_gpu_available() -> bool {
    norito::core::hw::has_gpu_compression()
}

fn cuda_available() -> bool {
    #[cfg(unix)]
    {
        if Path::new("/dev/nvidia0").exists() || Path::new("/dev/nvidiactl").exists() {
            return true;
        }
    }
    #[cfg(windows)]
    {
        if let Some(root) = system_root() {
            let system32 = root.join("System32").join("nvcuda.dll");
            let syswow64 = root.join("SysWOW64").join("nvcuda.dll");
            if system32.exists() || syswow64.exists() {
                return true;
            }
        }
    }
    if command_success(
        "nvidia-smi",
        &["--query-gpu=name", "--format=csv,noheader", "--id=0"],
    ) {
        return true;
    }
    if command_success("nvcc", &["--version"]) {
        return true;
    }
    false
}

fn opencl_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        if Path::new("/System/Library/Frameworks/OpenCL.framework/OpenCL").exists()
            && macos_opencl_devices_present()
        {
            return true;
        }
    }
    #[cfg(unix)]
    {
        if has_icd_entries("/etc/OpenCL/vendors") {
            return true;
        }
    }
    #[cfg(windows)]
    {
        if let Some(root) = system_root() {
            let system32 = root.join("System32").join("OpenCL.dll");
            let syswow64 = root.join("SysWOW64").join("OpenCL.dll");
            if system32.exists() || syswow64.exists() {
                return true;
            }
        }
    }
    command_success("clinfo", &["--list"])
}

#[cfg(unix)]
fn has_icd_entries(dir: &str) -> bool {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .any(|path| {
                    path.extension()
                        .and_then(|ext| ext.to_str())
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("icd"))
                })
        })
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn has_icd_entries(_dir: &str) -> bool {
    false
}

fn command_success(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn run_command_capture(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

#[cfg(windows)]
fn system_root() -> Option<PathBuf> {
    env::var_os("SystemRoot").map(PathBuf::from)
}

#[cfg(test)]
mod detection_tests {
    use fastpq_isi::CANONICAL_PARAMETER_SETS;

    use super::*;

    fn availability(enabled: &[GpuBackend]) -> BackendAvailability {
        enabled
            .iter()
            .fold(BackendAvailability::empty(), |state, backend| {
                state.with(*backend, true)
            })
    }

    #[test]
    fn resolve_backend_prefers_cuda() {
        assert_eq!(
            resolve_backend(availability(&[
                GpuBackend::Cuda,
                GpuBackend::OpenCl,
                GpuBackend::Metal,
                GpuBackend::Norito,
            ])),
            Some(GpuBackend::Cuda)
        );
    }

    #[test]
    fn resolve_backend_prefers_metal_before_opencl() {
        assert_eq!(
            resolve_backend(availability(&[
                GpuBackend::OpenCl,
                GpuBackend::Metal,
                GpuBackend::Norito,
            ])),
            Some(GpuBackend::Metal)
        );
    }

    #[test]
    fn resolve_backend_prefers_opencl_when_metal_missing() {
        assert_eq!(
            resolve_backend(availability(&[GpuBackend::OpenCl, GpuBackend::Norito])),
            Some(GpuBackend::OpenCl)
        );
    }

    #[test]
    fn resolve_backend_falls_back_to_metal() {
        assert_eq!(
            resolve_backend(availability(&[GpuBackend::Metal, GpuBackend::Norito])),
            Some(GpuBackend::Metal)
        );
    }

    #[test]
    fn resolve_backend_uses_norito_last() {
        assert_eq!(
            resolve_backend(availability(&[GpuBackend::Norito])),
            Some(GpuBackend::Norito)
        );
    }

    #[test]
    fn resolve_backend_returns_none_when_unavailable() {
        assert_eq!(resolve_backend(availability(&[])), None);
    }

    #[test]
    fn parse_override_accepts_cpu_gpu_auto() {
        assert_eq!(parse_gpu_override("cpu"), Some(GpuOverride::ForceCpu));
        assert_eq!(parse_gpu_override("GPU"), Some(GpuOverride::ForceGpu));
        assert_eq!(parse_gpu_override("auto"), Some(GpuOverride::Auto));
        assert_eq!(parse_gpu_override("  detect  "), Some(GpuOverride::Auto));
    }

    #[test]
    fn parse_override_rejects_unknown_values() {
        assert_eq!(parse_gpu_override("unknown-mode"), None);
        assert_eq!(parse_gpu_override(""), Some(GpuOverride::Auto));
    }

    #[test]
    fn backend_config_defaults_to_auto_execution_mode() {
        let params = CANONICAL_PARAMETER_SETS[0];
        let config = BackendConfig::new(params);
        assert_eq!(config.execution_mode(), ExecutionMode::Auto);
    }
}

#[cfg(test)]
mod observer_tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    struct ExecutionModeObserverGuard;

    impl Drop for ExecutionModeObserverGuard {
        fn drop(&mut self) {
            clear_execution_mode_observer();
        }
    }

    #[test]
    fn execution_mode_observer_receives_resolution() {
        clear_execution_mode_observer();
        let _observer_guard = ExecutionModeObserverGuard;
        let (tx, rx) = mpsc::channel();
        set_execution_mode_observer(move |requested, resolved, backend| {
            let _ = tx.send((requested, resolved, backend));
        });

        let resolved = ExecutionMode::Auto.resolve();

        let (requested, resolved_event, backend) = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("observer payload");
        assert_eq!(requested, ExecutionMode::Auto);
        assert_eq!(resolved_event, resolved);
        match resolved {
            ExecutionMode::Cpu => assert!(
                backend.is_none(),
                "CPU resolution should not report a GPU backend"
            ),
            ExecutionMode::Gpu => assert!(
                backend.is_some(),
                "GPU resolution should report the detected backend"
            ),
            ExecutionMode::Auto => unreachable!("resolution never returns Auto"),
        }

        clear_execution_mode_observer();
    }
}

#[cfg(target_os = "macos")]
fn metal_available() -> bool {
    if metal_library_path().is_some() {
        return true;
    }
    if metal_device_visible_via_api() {
        return true;
    }
    if !Path::new(METAL_FRAMEWORK).exists() {
        return false;
    }
    let output = match Command::new("system_profiler")
        .args(["SPDisplaysDataType", "-detailLevel", "basic"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    {
        Ok(output) => output,
        Err(_) => return false,
    };
    if !output.status.success() {
        return false;
    }
    let text = match String::from_utf8(output.stdout) {
        Ok(text) => text,
        Err(_) => return false,
    };
    let lowered = text.to_lowercase();
    lowered.contains("metal: supported")
        || lowered.contains("metal gpu family")
        || lowered.contains("metal: up to date")
}

#[cfg(target_os = "macos")]
fn macos_opencl_devices_present() -> bool {
    if let Some(result) = macos_system_profiler_opencl_devices() {
        return result;
    }
    if command_success("clinfo", &["-l"]) {
        return true;
    }
    macos_ioreg_reports_accelerator()
}

#[cfg(target_os = "macos")]
fn macos_system_profiler_opencl_devices() -> Option<bool> {
    let output = run_command_capture(
        "system_profiler",
        &["SPOpenCLDataType", "-detailLevel", "mini"],
    )?;
    let lowered = output.to_lowercase();
    if lowered.contains("no opencl devices") || lowered.contains("opencl software") {
        return Some(false);
    }
    if lowered.contains("devices:") || lowered.contains("device type:") {
        return Some(true);
    }
    None
}

#[cfg(target_os = "macos")]
fn macos_ioreg_reports_accelerator() -> bool {
    if let Some(output) = run_command_capture("ioreg", &["-l", "-w0", "-c", "IOAccelerator"]) {
        let lowered = output.to_lowercase();
        if lowered.contains("ioaccelerator") || lowered.contains("metalpluginclass") {
            return true;
        }
    }
    false
}

#[cfg(not(target_os = "macos"))]
fn metal_available() -> bool {
    metal_library_path().is_some()
}

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
fn metal_device_visible_via_api() -> bool {
    warm_up_core_graphics_display();
    let debug = fastpq_debug_metal_enum();
    if let Some(device) = Device::system_default() {
        if debug {
            eprintln!(
                "fastpq::planner: MTLCreateSystemDefaultDevice succeeded ({} / headless={} / low_power={})",
                device.name(),
                bool_flag(device.is_headless()),
                bool_flag(device.is_low_power())
            );
        }
        return true;
    }
    let devices = Device::all();
    if debug {
        eprintln!(
            "fastpq::planner: MTLCopyAllDevices returned {} device(s)",
            devices.len()
        );
        for (index, device) in devices.iter().enumerate() {
            eprintln!(
                "fastpq::planner:   device #{index}: name=\"{}\", location={}, headless={}, low_power={}",
                device.name(),
                device_location_label(device.location()),
                bool_flag(device.is_headless()),
                bool_flag(device.is_low_power())
            );
        }
    }
    !devices.is_empty()
}

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
fn fastpq_debug_metal_enum() -> bool {
    if let Some(enabled) = overrides::metal_debug_enum_override() {
        return enabled;
    }
    *DEBUG_METAL_ENUM_ENV.get_or_init(|| {
        overrides::guard_env_override(|| overrides::debug_env_bool("FASTPQ_DEBUG_METAL_ENUM"))
            .unwrap_or(false)
    })
}

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
fn bool_flag(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
fn device_location_label(location: MTLDeviceLocation) -> &'static str {
    match location {
        MTLDeviceLocation::BuiltIn => "built_in",
        MTLDeviceLocation::Slot => "slot",
        MTLDeviceLocation::External => "external",
        _ => "unknown",
    }
}

fn metal_library_path() -> Option<String> {
    overrides::guard_env_override(|| {
        overrides::debug_env_string("FASTPQ_METAL_LIB").and_then(|path| {
            if !path.is_empty() && Path::new(&path).exists() {
                Some(path)
            } else {
                None
            }
        })
    })
}

/// Preview backend configuration used by the FASTPQ prover.
#[derive(Debug, Clone, Copy)]
pub struct BackendConfig {
    /// Canonical parameter set driving this backend instance.
    pub params: StarkParameterSet,
    /// Execution mode used for FFT/LDE computations.
    pub execution_mode: ExecutionMode,
    /// Poseidon pipeline override (defaults to [`ExecutionMode::Auto`]).
    pub poseidon_mode: PoseidonExecutionMode,
}

impl BackendConfig {
    /// Construct a configuration from a canonical parameter set.
    pub fn new(params: StarkParameterSet) -> Self {
        Self {
            params,
            execution_mode: ExecutionMode::Auto,
            poseidon_mode: PoseidonExecutionMode::Auto,
        }
    }

    /// Override the execution mode used by the backend.
    #[must_use]
    pub fn with_execution_mode(mut self, mode: ExecutionMode) -> Self {
        self.execution_mode = mode;
        self
    }

    /// Override the Poseidon pipeline execution mode used by the backend.
    #[must_use]
    pub fn with_poseidon_mode(mut self, mode: PoseidonExecutionMode) -> Self {
        self.poseidon_mode = mode;
        self
    }

    /// Return the requested execution mode.
    #[must_use]
    pub fn execution_mode(&self) -> ExecutionMode {
        self.execution_mode
    }

    /// Return the configured Poseidon pipeline mode.
    #[must_use]
    pub fn poseidon_mode(&self) -> PoseidonExecutionMode {
        self.poseidon_mode
    }
}

/// Trait describing the behaviour of a FASTPQ prover backend.
pub trait Backend {
    /// Attempt to generate a proof artifact for the supplied batch.
    ///
    /// # Errors
    fn prove(
        &self,
        batch: &TransitionBatch,
        public_io: &PublicIO,
        protocol_version: u16,
        params_version: u16,
    ) -> Result<BackendArtifact>;
}

/// Deterministic artifact emitted by the backend after running the placeholder
/// FASTPQ pipeline.  This mirrors the minimal data the verifier needs to
/// reconstruct the Fiat–Shamir transcript and query openings.
#[derive(Debug, Clone)]
pub struct BackendArtifact {
    /// Canonical parameter set name.
    pub parameter: String,
    /// Number of real trace rows prior to padding.
    pub trace_rows: u32,
    /// Poseidon2 Merkle root over column commitments.
    pub trace_root: u64,
    /// Poseidon2 Merkle root over the low-degree extension leaf hashes.
    pub lookup_root: u64,
    /// Lookup grand-product accumulator over the permission witness column.
    pub lookup_grand_product: u64,
    /// Lookup Fiat–Shamir challenge (`γ`).
    pub lookup_challenge: u64,
    /// Composition challenges sampled after `γ`.
    pub alphas: Vec<u64>,
    /// FRI folding arity advertised by the parameter set.
    pub fri_arity: u32,
    /// Low-degree extension blowup factor.
    pub fri_blowup: u32,
    /// Poseidon2 hash of each FRI layer plus the terminal root.
    pub fri_layers: Vec<u64>,
    /// Fiat–Shamir challenges used for each FRI folding round.
    pub fri_betas: Vec<u64>,
    /// Sampled query openings into the evaluation domain.
    pub query_openings: Vec<(u32, u64)>,
    /// Merkle authentication paths for each queried evaluation chunk.
    pub query_paths: Vec<Vec<u64>>,
}

/// Concrete backend implementing the deterministic FASTPQ STARK pipeline.
#[derive(Debug, Clone)]
pub struct StarkBackend {
    config: BackendConfig,
}

impl StarkBackend {
    /// Create a backend from a canonical configuration.
    pub fn new(config: BackendConfig) -> Self {
        Self { config }
    }

    /// Expose the configured execution mode, primarily for tests.
    #[cfg_attr(not(test), allow(dead_code))]
    #[must_use]
    pub fn execution_mode(&self) -> ExecutionMode {
        self.config.execution_mode()
    }
}

/// Hash the low-degree extension evaluations into Merkle leaves grouped by the
/// canonical chunk size derived from the FRI arity.
///
/// # Errors
/// Returns an error if hashing a chunk into the Merkle leaf domain fails.
pub fn hash_lde_leaves(evaluations: &[u64], arity: u32) -> Result<Vec<u64>> {
    if evaluations.is_empty() {
        return Ok(Vec::new());
    }
    let chunk = lde_chunk_size(arity);
    let mut leaves = Vec::with_capacity(evaluations.len().div_ceil(chunk));
    for (idx, group) in evaluations.chunks(chunk).enumerate() {
        let mut limbs = Vec::with_capacity(group.len() + 1);
        limbs.push(u64::try_from(idx).expect("chunk index fits field limb"));
        limbs.extend(group.iter().copied());
        leaves.push(hash_with_domain(LDE_LEAF_DOMAIN, &limbs)?);
    }
    Ok(leaves)
}

/// Return the chunk size (number of evaluations per leaf hash) for the given FRI arity.
pub fn lde_chunk_size(arity: u32) -> usize {
    usize::try_from(arity.saturating_mul(8).max(1)).expect("FRI chunk size fits usize")
}

/// Compute Merkle authentication paths for the supplied query indices over the provided leaf set.
///
/// The query indices are expressed in terms of evaluation positions; internally they are grouped
/// into leaf chunks using the canonical chunk size derived from the FRI arity.
///
/// # Errors
/// Returns an error if hashing an internal Merkle level fails.
pub fn merkle_paths_for_queries(
    leaves: &[u64],
    query_indices: &[usize],
    arity: u32,
    evaluation_len: usize,
) -> Result<Vec<Vec<u64>>> {
    if query_indices.is_empty() {
        return Ok(Vec::new());
    }
    if evaluation_len == 0 || leaves.is_empty() {
        return Err(Error::QueryIndexOutOfRange {
            index: query_indices[0],
            len: evaluation_len,
        });
    }
    let chunk_size = lde_chunk_size(arity).max(1);
    let levels = build_merkle_levels(leaves)?;
    let leaf_level = levels
        .first()
        .expect("non-empty levels for non-empty leaves");
    let leaf_count = leaf_level.len();
    let mut paths = Vec::with_capacity(query_indices.len());
    for &query_index in query_indices {
        if query_index >= evaluation_len {
            return Err(Error::QueryIndexOutOfRange {
                index: query_index,
                len: evaluation_len,
            });
        }
        let mut leaf_index = query_index / chunk_size;
        if leaf_index >= leaf_count {
            return Err(Error::QueryIndexOutOfRange {
                index: query_index,
                len: evaluation_len,
            });
        }
        let mut path = Vec::with_capacity(levels.len().saturating_sub(1));
        for level in levels.iter().take(levels.len().saturating_sub(1)) {
            let sibling_idx = if leaf_index.is_multiple_of(2) {
                leaf_index + 1
            } else {
                leaf_index.saturating_sub(1)
            };
            let sibling = level
                .get(sibling_idx)
                .copied()
                .unwrap_or_else(|| level[leaf_index]);
            path.push(sibling);
            leaf_index /= 2;
        }
        paths.push(path);
    }
    Ok(paths)
}

fn build_merkle_levels(leaves: &[u64]) -> Result<Vec<Vec<u64>>> {
    if leaves.is_empty() {
        return Ok(Vec::new());
    }
    let mut levels = Vec::new();
    let mut current = leaves.to_vec();
    if current.len() == 1 {
        levels.push(current);
        return Ok(levels);
    }
    loop {
        if current.len() % 2 == 1 {
            let last = *current.last().expect("non-empty Merkle level");
            current.push(last);
        }
        levels.push(current.clone());
        let mut next = Vec::with_capacity(current.len() / 2);
        for pair in current.chunks(2) {
            next.push(hash_with_domain(TRACE_NODE_DOMAIN, &[pair[0], pair[1]])?);
        }
        if next.len() == 1 {
            levels.push(next.clone());
            break;
        }
        current = next;
    }
    Ok(levels)
}

/// Compute the Fiat–Shamir lookup grand product accumulator over the supplied
/// selector and witness evaluations using the canonical Goldilocks field
/// arithmetic.
pub fn compute_lookup_grand_product(
    selector_values: &[u64],
    witness_values: &[u64],
    gamma: u64,
) -> u64 {
    assert_eq!(
        selector_values.len(),
        witness_values.len(),
        "lookup witness LDE columns must share a length"
    );
    let mut acc = FIELD_ONE;
    let mut running = Vec::with_capacity(witness_values.len());
    for (&selector, &witness) in selector_values.iter().zip(witness_values.iter()) {
        if selector != 0 {
            acc = mul_mod(acc, add_mod(witness, gamma));
        }
        running.push(acc);
    }
    poseidon::hash_field_elements(&running)
}

pub fn hash_trace_rows(columns: &[Vec<u64>]) -> Vec<u64> {
    if columns.is_empty() {
        return Vec::new();
    }

    let row_count = columns[0].len();
    let column_count = columns.len();
    assert!(
        columns.iter().all(|column| column.len() == row_count),
        "LDE columns must have a consistent length"
    );

    let column_count_field = (column_count as u64) % GOLDILOCKS_MODULUS;
    let hashes: Vec<u64> = (0..row_count)
        .into_par_iter()
        .map(|row_index| {
            let mut sponge = PoseidonSponge::new();
            sponge.absorb((row_index as u64) % GOLDILOCKS_MODULUS);
            sponge.absorb(column_count_field);
            for column in columns {
                sponge.absorb(column[row_index]);
            }
            sponge.squeeze()
        })
        .collect();
    hashes
}

pub fn extend_row_hashes(
    planner: &Planner,
    mode: ExecutionMode,
    rows: Vec<u64>,
    trace_len: usize,
) -> Vec<u64> {
    if rows.is_empty() || trace_len == 0 {
        return rows;
    }

    let blowup = 1usize << planner.blowup_log();
    let expected_eval_len = trace_len
        .checked_mul(blowup)
        .expect("trace length times blowup fits usize");
    if rows.len() == expected_eval_len {
        return rows;
    }
    assert_eq!(
        rows.len(),
        trace_len,
        "row hash length ({}) must match trace domain ({trace_len}) or the evaluation domain ({expected_eval_len})",
        rows.len()
    );

    let mut coeffs = vec![rows];
    match mode {
        ExecutionMode::Gpu => {
            #[cfg(test)]
            let mut cpu_coeffs = coeffs.clone();
            #[cfg(test)]
            planner.ifft_columns(&mut cpu_coeffs);

            planner.ifft_gpu(&mut coeffs);

            #[cfg(test)]
            assert_eq!(
                cpu_coeffs, coeffs,
                "ifft gpu output diverged from cpu reference"
            );

            let lde = planner.lde_gpu(&coeffs);

            #[cfg(test)]
            {
                let cpu_lde = planner.lde_columns(&coeffs);
                assert_eq!(cpu_lde, lde, "lde gpu output diverged from cpu reference");
            }

            lde.into_iter().next().unwrap_or_default()
        }
        ExecutionMode::Cpu | ExecutionMode::Auto => {
            planner.ifft_columns(&mut coeffs);
            planner
                .lde_columns(&coeffs)
                .into_iter()
                .next()
                .unwrap_or_default()
        }
    }
}

pub fn fold_with_fri(
    evaluations: &[u64],
    arity: u32,
    max_reductions: u32,
    transcript: &mut Transcript,
) -> Result<(Vec<u64>, Vec<u64>)> {
    if arity != 8 && arity != 16 {
        return Err(Error::FriArity(arity));
    }
    if evaluations.is_empty() {
        transcript.append_fri_final(0);
        return Ok((vec![0], Vec::new()));
    }

    let arity = usize::try_from(arity).expect("FRI arity fits usize");
    let max_rounds = usize::try_from(max_reductions).expect("FRI reduction bound fits usize");

    let mut current = evaluations.to_vec();
    let mut layers = Vec::new();
    let mut betas = Vec::new();
    let mut round = 0usize;

    while current.len() > 1 && round < max_rounds {
        let span = tracing::info_span!("fastpq_fri_round", round, layer_len = current.len(), arity);
        let _enter = span.enter();

        let root = fri_layer_commitment(round, &current);
        transcript.append_fri_layer(round, root);
        layers.push(root);

        let beta = transcript.challenge_beta(round);
        betas.push(beta);

        let mut padded = current;
        pad_to_arity(&mut padded, arity);
        tracing::debug!(
            round,
            padded_len = padded.len(),
            "folding FRI layer with beta"
        );
        let next = fold_round(&padded, arity, beta);
        let next_len = next.len();
        tracing::info!(
            round,
            root,
            beta,
            padded_len = padded.len(),
            next_len,
            "fri round committed"
        );
        current = next;
        round += 1;
    }

    let final_root = fri_layer_commitment(round, &current);
    transcript.append_fri_final(final_root);
    tracing::info!(round, final_root, "final FRI layer commitment");
    layers.push(final_root);

    Ok((layers, betas))
}

fn pad_to_arity(values: &mut Vec<u64>, arity: usize) {
    if values.is_empty() || arity == 0 {
        return;
    }
    let remainder = values.len() % arity;
    if remainder == 0 {
        return;
    }
    let pad_value = *values.last().expect("non-empty values");
    let padding = arity - remainder;
    values.extend(std::iter::repeat_n(pad_value, padding));
}

fn fold_round(values: &[u64], arity: usize, challenge: u64) -> Vec<u64> {
    if values.is_empty() || arity == 0 {
        return Vec::new();
    }
    let chunk_count = values.len().div_ceil(arity);
    let mut next = Vec::with_capacity(chunk_count);
    for chunk in values.chunks(arity) {
        let mut acc = 0u64;
        let mut power = FIELD_ONE;
        for &value in chunk {
            acc = add_mod(acc, mul_mod(value, power));
            power = mul_mod(power, challenge);
        }
        next.push(acc);
    }
    next
}

fn fri_layer_commitment(round: usize, values: &[u64]) -> u64 {
    let mut sponge = PoseidonSponge::new();
    let modulus = u128::from(GOLDILOCKS_MODULUS);
    let round_field =
        u64::try_from((round as u128) % modulus).expect("round reduced modulo field fits u64");
    let len_field = u64::try_from((values.len() as u128) % modulus)
        .expect("evaluation count reduced modulo field fits u64");
    sponge.absorb(round_field);
    sponge.absorb(len_field);
    for (idx, &value) in values.iter().enumerate() {
        let index_field =
            u64::try_from((idx as u128) % modulus).expect("index reduced modulo field fits u64");
        sponge.absorb(index_field);
        sponge.absorb(value);
    }
    sponge.squeeze()
}

pub fn sample_queries(
    domain_size: usize,
    target: usize,
    transcript: &mut Transcript,
) -> Vec<usize> {
    if domain_size == 0 || target == 0 {
        return Vec::new();
    }
    let domain = u64::try_from(domain_size).expect("domain size fits u64");
    let desired = target.min(domain_size);
    let mut indices = BTreeSet::new();
    let mut counter: u32 = 0;

    while indices.len() < desired {
        let tag = format!("{TRANSCRIPT_TAG_QUERY_INDEX}:{counter}");
        counter = counter
            .checked_add(1)
            .expect("query sampler counter overflow");
        let bytes = transcript.challenge_bytes(&tag);
        for chunk in bytes.chunks_exact(8) {
            if indices.len() == desired {
                break;
            }
            let mut buf = [0u8; 8];
            buf.copy_from_slice(chunk);
            let remainder = u64::from_le_bytes(buf) % domain;
            let index =
                usize::try_from(remainder).expect("query index derived from transcript fits usize");
            indices.insert(index);
        }
    }

    indices.into_iter().collect()
}

pub fn open_queries(evaluations: &[u64], indices: &[usize]) -> Result<Vec<(u32, u64)>> {
    let mut openings = Vec::with_capacity(indices.len());
    for &index in indices {
        let value = evaluations
            .get(index)
            .copied()
            .ok_or(Error::QueryIndexOutOfRange {
                index,
                len: evaluations.len(),
            })?;
        let compact_index =
            u32::try_from(index).map_err(|_| Error::QueryIndexOverflow { index })?;
        openings.push((compact_index, value));
    }
    Ok(openings)
}

impl Backend for StarkBackend {
    fn prove(
        &self,
        batch: &TransitionBatch,
        public_io: &PublicIO,
        protocol_version: u16,
        params_version: u16,
    ) -> Result<BackendArtifact> {
        if self.config.params.name != batch.parameter {
            return Err(Error::ParameterMismatch {
                expected: self.config.params.name.to_string(),
                actual: batch.parameter.clone(),
            });
        }

        let trace = build_trace(batch)?;
        let planner = Planner::new(&self.config.params);
        let requested_mode = self.config.execution_mode();
        let resolved_mode = requested_mode.resolve();
        let poseidon_policy =
            PoseidonPipelinePolicy::new(self.config.poseidon_mode(), resolved_mode);
        let mut polynomial_data = derive_polynomial_data(&trace, &planner, resolved_mode);
        let transfer_plan = polynomial_data.transfer_plan().clone();
        if transfer_plan.total_deltas() > 0 {
            tracing::debug!(
                target: "fastpq::transfer",
                batches = transfer_plan.batch_count(),
                deltas = transfer_plan.total_deltas(),
                estimated_rows = transfer_plan.estimated_row_budget(),
                "transfer gadget witnesses planned"
            );
        }

        let column_digests = hash_columns_from_coefficients(
            &trace,
            &polynomial_data.coefficients,
            &planner,
            resolved_mode,
            poseidon_policy,
        );
        let trace_root =
            merkle_root_with_first_level(column_digests.leaves(), column_digests.fused_parents());

        let lde_columns = polynomial_data.lde_columns();
        let lde_rows = hash_trace_rows(lde_columns);
        let lde_values = extend_row_hashes(&planner, resolved_mode, lde_rows, trace.padded_len);
        let lde_hashes = hash_lde_leaves(&lde_values, self.config.params.fri.arity)?;
        let lde_root = merkle_root(&lde_hashes);

        let mut transcript = Transcript::initialise(
            public_io,
            self.config.params.name,
            protocol_version,
            params_version,
            TRANSCRIPT_TAG_INIT,
        )?;
        transcript.append_message(
            TRANSCRIPT_TAG_ROOTS,
            &[lde_root.to_le_bytes(), trace_root.to_le_bytes()].concat(),
        );
        let gamma = transcript.challenge_field(TRANSCRIPT_TAG_GAMMA);
        let selector_index = column_index(&trace, "s_perm")
            .ok_or_else(|| Error::MissingColumn("s_perm".to_string()))?;
        let witness_index = column_index(&trace, "perm_hash")
            .ok_or_else(|| Error::MissingColumn("perm_hash".to_string()))?;
        let selector_values = &lde_columns[selector_index];
        let witness_values = &lde_columns[witness_index];
        let lookup_grand_product =
            compute_lookup_grand_product(selector_values, witness_values, gamma);
        let mut alphas = Vec::with_capacity(2);
        for idx in 0..2 {
            let tag = format!("{TRANSCRIPT_TAG_ALPHA_PREFIX}:{idx}");
            alphas.push(transcript.challenge_field(&tag));
        }
        transcript.append_message(LOOKUP_PRODUCT_DOMAIN, &lookup_grand_product.to_le_bytes());

        let (fri_layers, fri_betas) = fold_with_fri(
            &lde_values,
            self.config.params.fri.arity,
            self.config.params.fri.max_reductions,
            &mut transcript,
        )?;
        let query_indices = sample_queries(
            lde_values.len(),
            usize::try_from(self.config.params.fri.queries).expect("query count fits usize"),
            &mut transcript,
        );
        let query_openings = open_queries(&lde_values, &query_indices)?;
        let query_paths = merkle_paths_for_queries(
            &lde_hashes,
            &query_indices,
            self.config.params.fri.arity,
            lde_values.len(),
        )?;

        let trace_rows = u32::try_from(trace.rows)
            .map_err(|_| Error::TraceLengthOverflow { rows: trace.rows })?;

        Ok(BackendArtifact {
            parameter: self.config.params.name.to_string(),
            trace_rows,
            trace_root,
            lookup_root: lde_root,
            lookup_grand_product,
            lookup_challenge: gamma,
            alphas,
            fri_arity: self.config.params.fri.arity,
            fri_blowup: self.config.params.fri.blowup_factor,
            fri_layers,
            fri_betas,
            query_openings,
            query_paths,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Transcript {
    state: Vec<u8>,
}

impl Transcript {
    pub fn initialise(
        public_io: &PublicIO,
        parameter: &str,
        protocol_version: u16,
        params_version: u16,
        tag: &str,
    ) -> Result<Self> {
        let mut transcript = Self { state: Vec::new() };
        let payload = norito::core::to_bytes(&(
            protocol_version,
            params_version,
            parameter,
            public_io.clone(),
        ))?;
        transcript.append_message(tag, &payload);
        Ok(transcript)
    }

    pub fn append_message(&mut self, tag: &str, message: &[u8]) {
        let tag_len = u32::try_from(tag.len()).expect("transcript tag length fits u32");
        self.state.extend_from_slice(&tag_len.to_le_bytes());
        self.state.extend_from_slice(tag.as_bytes());
        let message_len = u32::try_from(message.len()).expect("transcript message length fits u32");
        self.state.extend_from_slice(&message_len.to_le_bytes());
        self.state.extend_from_slice(message);
    }

    pub fn append_fri_layer(&mut self, round: usize, root: u64) {
        let tag = format!("{TRANSCRIPT_TAG_FRI_LAYER_PREFIX}:{round}");
        self.append_message(&tag, &root.to_le_bytes());
    }

    pub fn append_fri_final(&mut self, root: u64) {
        self.append_message(FRI_FINAL_DOMAIN, &root.to_le_bytes());
    }

    pub fn challenge_beta(&mut self, round: usize) -> u64 {
        let tag = format!("{TRANSCRIPT_TAG_BETA_PREFIX}:{round}");
        self.challenge_field(&tag)
    }

    pub fn challenge_bytes(&mut self, tag: &str) -> [u8; Hash::LENGTH] {
        let mut payload = self.state.clone();
        let tag_len = u32::try_from(tag.len()).expect("transcript challenge tag fits u32");
        payload.extend_from_slice(&tag_len.to_le_bytes());
        payload.extend_from_slice(tag.as_bytes());
        let digest = Hash::new(&payload);
        self.state.extend_from_slice(digest.as_ref());
        digest.into()
    }

    pub fn challenge_field(&mut self, tag: &str) -> u64 {
        let bytes = self.challenge_bytes(tag);
        let mut chunk = [0u8; 8];
        chunk.copy_from_slice(&bytes[..8]);
        let value = u128::from(u64::from_le_bytes(chunk)) % u128::from(GOLDILOCKS_MODULUS);
        u64::try_from(value).expect("modulus reduction stays within u64")
    }
}

fn hash_with_domain(domain: &[u8], values: &[u64]) -> Result<u64> {
    let mut payload = Vec::with_capacity(values.len() * 8);
    for value in values {
        payload.extend_from_slice(&value.to_le_bytes());
    }
    let domain_packed = pack_bytes(domain);
    let payload_packed = pack_bytes(&payload);
    let mut limbs = Vec::with_capacity(domain_packed.limbs.len() + payload_packed.limbs.len() + 2);
    let domain_len = u64::try_from(domain_packed.length).map_err(|_| Error::ValueWidth {
        length: domain_packed.length,
    })?;
    limbs.push(domain_len);
    limbs.extend(domain_packed.limbs);
    let payload_len = u64::try_from(payload_packed.length).map_err(|_| Error::ValueWidth {
        length: payload_packed.length,
    })?;
    limbs.push(payload_len);
    limbs.extend(payload_packed.limbs);
    Ok(poseidon::hash_field_elements(&limbs))
}

fn add_mod(a: u64, b: u64) -> u64 {
    let sum = a.wrapping_add(b);
    if sum >= GOLDILOCKS_MODULUS {
        sum - GOLDILOCKS_MODULUS
    } else {
        sum
    }
}

fn mul_mod(a: u64, b: u64) -> u64 {
    let product = u128::from(a) * u128::from(b);
    let reduced = product % u128::from(GOLDILOCKS_MODULUS);
    u64::try_from(reduced).expect("modulus reduction fits in u64")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::{OperationKind, PublicInputs, StateTransition};

    fn sample_batch(rows: usize) -> TransitionBatch {
        let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
        for idx in 0..rows {
            let key = format!("asset/xor/account/{idx:04}").into_bytes();
            let idx_u64 = u64::try_from(idx).expect("sample batch index fits u64");
            let pre = idx_u64.to_le_bytes().to_vec();
            let post = idx_u64.wrapping_add(1).to_le_bytes().to_vec();
            let op = if idx % 2 == 0 {
                OperationKind::Mint
            } else {
                OperationKind::Burn
            };
            batch.push(StateTransition::new(key, pre, post, op));
        }
        batch.sort();
        batch
    }

    #[test]
    fn transcript_challenges_are_deterministic() {
        let mut transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            "fastpq-lane-balanced",
            1,
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .expect("transcript");
        transcript.append_message("tag", b"payload");
        let a = transcript.challenge_field("gamma");
        let b = transcript.challenge_field("gamma");
        assert_ne!(a, 0);
        assert_ne!(b, 0);
        assert_ne!(a, b);
    }

    #[test]
    fn open_queries_rejects_out_of_range() {
        let err = open_queries(&[10u64, 11u64], &[2]).expect_err("out-of-range query");
        assert!(matches!(
            err,
            Error::QueryIndexOutOfRange { index: 2, len: 2 }
        ));
    }

    #[test]
    fn merkle_paths_rejects_out_of_range_indices() {
        let evaluations = vec![1u64, 2, 3, 4];
        let leaves = hash_lde_leaves(&evaluations, 8).expect("hash leaves");
        let err = merkle_paths_for_queries(&leaves, &[4], 8, evaluations.len())
            .expect_err("out of range");
        assert!(matches!(
            err,
            Error::QueryIndexOutOfRange { index: 4, len: 4 }
        ));
    }

    #[test]
    fn fold_with_fri_emits_layers_and_betas() {
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let mut transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            "fastpq-lane-balanced",
            1,
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .expect("transcript");
        let evaluations = [1u64, 2, 3, 4];
        let (layers, betas) = fold_with_fri(
            &evaluations,
            params.fri.arity,
            params.fri.max_reductions,
            &mut transcript,
        )
        .expect("fri folding");
        assert!(!betas.is_empty());
        assert_eq!(layers.len(), betas.len() + 1);
        assert!(
            betas.len()
                <= usize::try_from(params.fri.max_reductions).expect("max reductions fits usize")
        );
        assert!(layers.iter().all(|&layer| layer != 0));
    }

    #[test]
    fn fold_with_fri_rejects_invalid_arity() {
        let mut transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            "fastpq-lane-balanced",
            1,
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .expect("transcript");
        let err =
            super::fold_with_fri(&[1, 2, 3], 4, 1, &mut transcript).expect_err("invalid arity");
        assert!(matches!(err, super::Error::FriArity(4)));
    }

    #[test]
    fn sampled_queries_are_sorted_and_unique() {
        let mut transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            "fastpq-lane-balanced",
            1,
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .expect("transcript");
        let indices = super::sample_queries(128, 16, &mut transcript);
        assert_eq!(indices.len(), 16);
        assert!(indices.windows(2).all(|window| window[0] < window[1]));
    }

    #[test]
    fn sampled_queries_cap_at_domain_size() {
        let mut transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            "fastpq-lane-balanced",
            1,
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .expect("transcript");
        let indices = super::sample_queries(5, 10, &mut transcript);
        assert_eq!(indices.len(), 5);
        let unique: BTreeSet<_> = indices.iter().copied().collect();
        assert_eq!(unique.len(), indices.len());
        assert!(indices.iter().all(|&idx| idx < 5));
        assert!(indices.windows(2).all(|window| window[0] < window[1]));
    }

    #[test]
    fn lde_row_hashes_match_expected_length() {
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let planner = crate::Planner::new(&params);
        let trace = build_trace(&sample_batch(8)).expect("trace");
        let mut data = crate::trace::derive_polynomial_data(&trace, &planner, ExecutionMode::Cpu);
        let hashes = hash_trace_rows(data.lde_columns());
        let expected = trace.padded_len
            * usize::try_from(params.fri.blowup_factor).expect("blowup fits usize");
        assert_eq!(hashes.len(), expected);
    }

    #[test]
    fn lookup_grand_product_consumes_lde_witness() {
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let planner = crate::Planner::new(&params);
        let trace = build_trace(&sample_batch(4)).expect("trace");
        let mut data = crate::trace::derive_polynomial_data(&trace, &planner, ExecutionMode::Cpu);
        let perm_index =
            crate::trace::column_index(&trace, "perm_hash").expect("perm column present");
        let selector_index =
            crate::trace::column_index(&trace, "s_perm").expect("selector column present");
        let gamma = 7;
        let lde_columns = data.lde_columns();
        let product = compute_lookup_grand_product(
            lde_columns[selector_index].as_slice(),
            lde_columns[perm_index].as_slice(),
            gamma,
        );
        assert_ne!(product, 0);
    }

    #[test]
    fn pad_to_arity_extends_with_last_value() {
        let mut values = vec![1u64, 2, 3];
        super::pad_to_arity(&mut values, 4);
        assert_eq!(values.len(), 4);
        assert_eq!(values[3], 3);
        super::pad_to_arity(&mut values, 4);
        assert_eq!(values.len(), 4);
    }

    #[test]
    fn fold_round_weights_by_challenge_powers() {
        let values = vec![1u64, 2, 3, 4];
        let challenge = 5;
        let folded = super::fold_round(&values, values.len(), challenge);
        assert_eq!(folded.len(), 1);
        let mut expected = 0u64;
        let mut power = super::FIELD_ONE;
        for &value in &values {
            expected = super::add_mod(expected, super::mul_mod(value, power));
            power = super::mul_mod(power, challenge);
        }
        assert_eq!(folded[0], expected);
    }

    #[test]
    fn fri_layer_commitment_changes_when_values_change() {
        let original = super::fri_layer_commitment(0, &[1, 2, 3, 4]);
        let mutated = super::fri_layer_commitment(0, &[1, 2, 3, 5]);
        assert_ne!(original, mutated);
        let repeated = super::fri_layer_commitment(0, &[1, 2, 3, 4]);
        assert_eq!(original, repeated);
    }

    #[test]
    fn fri_folding_reduces_layer_length() {
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let mut transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            params.name,
            1,
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .expect("transcript");
        let evaluations: Vec<u64> = (0u64..16).map(|idx| idx + 1).collect();
        let (layers, betas) =
            super::fold_with_fri(&evaluations, 8, params.fri.max_reductions, &mut transcript)
                .expect("fri folding");
        assert_eq!(layers.len(), betas.len() + 1);
        assert_eq!(betas.len(), 2);
        let mut transcript_again = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            params.name,
            1,
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .expect("transcript");
        let (repeat_layers, repeat_betas) = super::fold_with_fri(
            &evaluations,
            8,
            params.fri.max_reductions,
            &mut transcript_again,
        )
        .expect("fri folding repeat");
        assert_eq!(layers, repeat_layers);
        assert_eq!(betas, repeat_betas);
    }

    fn reference_pad_to_arity(values: &mut Vec<u64>, arity: usize) {
        if values.is_empty() || arity == 0 {
            return;
        }
        let remainder = values.len() % arity;
        if remainder == 0 {
            return;
        }
        let pad_value = *values.last().expect("non-empty values");
        let padding = arity - remainder;
        values.extend(std::iter::repeat_n(pad_value, padding));
    }

    fn reference_fold_round(values: &[u64], arity: usize, beta: u64) -> Vec<u64> {
        if values.is_empty() || arity == 0 {
            return Vec::new();
        }
        let mut next = Vec::with_capacity(values.len().div_ceil(arity));
        for chunk in values.chunks(arity) {
            let mut acc = 0u64;
            let mut power = super::FIELD_ONE;
            for &value in chunk {
                acc = super::add_mod(acc, super::mul_mod(value, power));
                power = super::mul_mod(power, beta);
            }
            next.push(acc);
        }
        next
    }

    fn reference_fri_layer_commitment(round: usize, values: &[u64]) -> u64 {
        let modulus = u128::from(super::GOLDILOCKS_MODULUS);
        let round_field = u64::try_from(u128::try_from(round).unwrap_or(0) % modulus)
            .expect("round reduced modulo field fits u64");
        let len_field = u64::try_from(u128::try_from(values.len()).unwrap_or(0) % modulus)
            .expect("evaluation count reduced modulo field fits u64");
        let mut sponge = PoseidonSponge::new();
        sponge.absorb(round_field);
        sponge.absorb(len_field);
        for (idx, &value) in values.iter().enumerate() {
            let index_field = u64::try_from(u128::try_from(idx).unwrap_or(0) % modulus)
                .expect("index reduced modulo field fits u64");
            sponge.absorb(index_field);
            sponge.absorb(value);
        }
        sponge.squeeze()
    }

    #[test]
    fn fri_layers_match_reference_harness() {
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let arity = params.fri.arity as usize;
        let evaluations: Vec<u64> = (0u64..64)
            .map(|idx| idx.wrapping_mul(37).wrapping_add(5) % super::GOLDILOCKS_MODULUS)
            .collect();
        let mut transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            params.name,
            1,
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .expect("transcript");
        let (layers, betas) =
            super::fold_with_fri(&evaluations, params.fri.arity, 3, &mut transcript)
                .expect("fold with fri");

        let mut reference_transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            params.name,
            1,
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .expect("reference transcript");
        let mut current = evaluations.clone();
        let mut reference_layers = Vec::new();
        let mut reference_betas = Vec::new();
        let mut round = 0usize;
        while current.len() > 1 && round < 3 {
            let root = reference_fri_layer_commitment(round, &current);
            reference_transcript.append_fri_layer(round, root);
            reference_layers.push(root);
            let beta = reference_transcript.challenge_beta(round);
            reference_betas.push(beta);
            let mut padded = current.clone();
            reference_pad_to_arity(&mut padded, arity);
            current = reference_fold_round(&padded, arity, beta);
            round += 1;
        }
        let final_root = reference_fri_layer_commitment(round, &current);
        reference_transcript.append_fri_final(final_root);
        reference_layers.push(final_root);

        assert_eq!(layers, reference_layers);
        assert_eq!(betas[..reference_betas.len()], reference_betas[..]);
    }

    #[test]
    fn fri_reference_detects_mutation() {
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let arity = params.fri.arity as usize;
        let evaluations: Vec<u64> = (0u64..64)
            .map(|idx| idx.wrapping_mul(19).wrapping_add(11) % super::GOLDILOCKS_MODULUS)
            .collect();
        let mut transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            params.name,
            1,
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .expect("transcript");
        let (baseline_layers, _) =
            super::fold_with_fri(&evaluations, params.fri.arity, 3, &mut transcript)
                .expect("baseline fold");

        let mut mutated = evaluations.clone();
        mutated[0] = mutated[0].wrapping_add(1);
        let mut reference_transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            params.name,
            1,
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .expect("reference transcript");
        let mut current = mutated.clone();
        let mut mutated_layers = Vec::new();
        let mut round = 0usize;
        while current.len() > 1 && round < 3 {
            let root = reference_fri_layer_commitment(round, &current);
            reference_transcript.append_fri_layer(round, root);
            mutated_layers.push(root);
            let beta = reference_transcript.challenge_beta(round);
            let mut padded = current.clone();
            reference_pad_to_arity(&mut padded, arity);
            current = reference_fold_round(&padded, arity, beta);
            round += 1;
        }
        let final_root = reference_fri_layer_commitment(round, &current);
        reference_transcript.append_fri_final(final_root);
        mutated_layers.push(final_root);

        assert_ne!(baseline_layers, mutated_layers);
    }

    mod fri_properties {
        use fastpq_isi::CANONICAL_PARAMETER_SETS;
        use proptest::{collection::vec as pvec, prelude::*};

        use super::*;
        use crate::Planner;

        const MAX_TRACE_LOG: u32 = 4;

        fn fri_input_strategy() -> impl Strategy<Value = (u32, Vec<u64>)> {
            (0u32..=MAX_TRACE_LOG).prop_flat_map(|trace_log| {
                let len = 1usize << trace_log;
                pvec(0u64..crate::poseidon::FIELD_MODULUS, len)
                    .prop_map(move |coeffs| (trace_log, coeffs))
            })
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(32))]
            #[test]
            fn fri_layers_and_betas_are_deterministic((trace_log, coeffs) in fri_input_strategy()) {
                let params = CANONICAL_PARAMETER_SETS[0];
                let planner = Planner::new(&params);
                let trace_len = 1usize << trace_log;
                prop_assert_eq!(coeffs.len(), trace_len);

                let evaluations = planner.lde_columns(std::slice::from_ref(&coeffs));
                let evaluation = evaluations.into_iter().next().expect("evaluation column");

                let mut transcript_a = Transcript::initialise(
                    &crate::proof::PublicIO::default(),
                    params.name,
                    1,
                    1,
                    TRANSCRIPT_TAG_INIT,
                )
                .expect("transcript");
                let mut transcript_b = Transcript::initialise(
                    &crate::proof::PublicIO::default(),
                    params.name,
                    1,
                    1,
                    TRANSCRIPT_TAG_INIT,
                )
                .expect("transcript");

                let (layers_a, betas_a) = fold_with_fri(
                    &evaluation,
                    params.fri.arity,
                    params.fri.max_reductions,
                    &mut transcript_a,
                )
                .expect("fri folding");
                let (layers_b, betas_b) = fold_with_fri(
                    &evaluation,
                    params.fri.arity,
                    params.fri.max_reductions,
                    &mut transcript_b,
                )
                .expect("fri folding");

                prop_assert_eq!(layers_a, layers_b);
                prop_assert_eq!(betas_a, betas_b);

                let expected_len = trace_len << planner.blowup_log();
                prop_assert_eq!(evaluation.len(), expected_len);
            }
        }
    }
}
