use crate::{
    Error, Result, TransitionBatch,
    fft::Planner,
    field::GoldilocksFp4V1,
    overrides,
    proof::{AirConstraintOpening, FriQueryOpening, FriRoundOpening, PublicIO},
    trace::{PoseidonPipelinePolicy, build_trace, column_index, derive_polynomial_data},
};
use core::convert::TryFrom;
use fastpq_isi::{
    FASTPQ_CATALOG_V1, FASTPQ_FINAL_V1_ID, GoldilocksDigest384V1, GoldilocksDigestDomainV1,
    StarkParameterSet, hash_bytes_384_v1,
};
#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
use metal::{Device, MTLDeviceLocation};
#[cfg(windows)]
use std::env;
#[cfg(unix)]
use std::fs;
#[cfg(windows)]
use std::path::PathBuf;
use std::{
    collections::BTreeSet,
    path::Path,
    process::{Command, Stdio},
    sync::{Arc, Mutex, MutexGuard, OnceLock, RwLock, TryLockError},
};
const GOLDILOCKS_MODULUS: u64 = 0xffff_ffff_0000_0001;
const FIELD_ONE: u64 = 1;
const TRACE_COMMITMENT_ROLE_V1: &[u8] = b"trace-commitment";
const LOOKUP_COMMITMENT_ROLE_V1: &[u8] = b"lookup-commitment";
const AIR_TRACE_COMMITMENT_ROLE_V1: &[u8] = b"air-trace-commitment";
const AIR_COMPOSITION_COMMITMENT_ROLE_V1: &[u8] = b"air-composition-commitment";
const FRI_COMMITMENT_ROLE_V1: &[u8] = b"fri-commitment";
const TRANSCRIPT_ROLE_V1: &[u8] = b"fiat-shamir-transcript";
const MERKLE_LEAF_PHASE_V1: &[u8] = b"leaf";
const MERKLE_NODE_PHASE_V1: &[u8] = b"node";
const MERKLE_EMPTY_PHASE_V1: &[u8] = b"empty-root";

/// Typed native-STARK Merkle role; the role and FRI round are bound into every internal node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MerkleTreeRoleV1 {
    /// Base trace column-commitment tree.
    Trace,
    /// Random-column-combination LDE/lookup tree.
    Lookup,
    /// Row-major AIR trace tree.
    AirTrace,
    /// AIR composition tree.
    AirComposition,
    /// One binary FRI layer and its zero-based round.
    Fri(u32),
}

impl MerkleTreeRoleV1 {
    fn role(self) -> &'static [u8] {
        match self {
            Self::Trace => TRACE_COMMITMENT_ROLE_V1,
            Self::Lookup => LOOKUP_COMMITMENT_ROLE_V1,
            Self::AirTrace => AIR_TRACE_COMMITMENT_ROLE_V1,
            Self::AirComposition => AIR_COMPOSITION_COMMITMENT_ROLE_V1,
            Self::Fri(_) => FRI_COMMITMENT_ROLE_V1,
        }
    }

    fn counter(self) -> u64 {
        match self {
            Self::Fri(round) => u64::from(round),
            Self::Trace | Self::Lookup | Self::AirTrace | Self::AirComposition => 0,
        }
    }
}
pub const LOOKUP_PRODUCT_DOMAIN: &str = "fastpq:v1:lookup:product";
const FRI_FINAL_DOMAIN: &str = "fastpq:v1:fri:final";
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
pub const TRANSCRIPT_TAG_TRACE_ROOT: &str = "fastpq:v1:trace_root";
pub const TRANSCRIPT_TAG_COLUMN_MIX_PREFIX: &str = "fastpq:v1:column_mix";
pub const TRANSCRIPT_TAG_GAMMA: &str = "fastpq:v1:gamma";
pub const TRANSCRIPT_TAG_ALPHA_PREFIX: &str = "fastpq:v1:alpha";
pub const TRANSCRIPT_TAG_AIR_ROOTS: &str = "fastpq:v1:air_roots";
pub const TRANSCRIPT_TAG_QUERY_INDEX: &str = "fastpq:v1:query_index";
pub const TRANSCRIPT_TAG_BETA_PREFIX: &str = "fastpq:v1:beta";
pub const TRANSCRIPT_TAG_FRI_LAYER_PREFIX: &str = "fastpq:v1:fri_layer";
const AIR_BOOLEAN_RESIDUE_COUNT: usize = 8;
const AIR_RELATION_RESIDUE_COUNT: usize = 4;
const AIR_STABLE_RESIDUE_COUNT: usize = crate::trace::METADATA_COMMITMENT_LIMBS + 2;
/// Number of V1 AIR composition challenges derived from the transcript.
///
/// Every constraint residue receives an independently derived coefficient. Reusing coefficients
/// lets equal-and-opposite residues at the same reuse offset cancel for every transcript.
pub const AIR_COMPOSITION_ALPHA_COUNT: usize =
    AIR_BOOLEAN_RESIDUE_COUNT + AIR_RELATION_RESIDUE_COUNT + AIR_STABLE_RESIDUE_COUNT;
/// Maximum algebraic degree of every implemented V1 AIR residue in trace columns.
pub const AIR_MAX_CONSTRAINT_DEGREE_V1: usize = 2;
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
#[cfg(any(test, feature = "fastpq-gpu"))]
pub fn acquire_gpu_lane() -> MutexGuard<'static, ()> {
    match gpu_workload_mutex().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
fn execution_mode_observer_slot() -> &'static RwLock<Option<Arc<ExecutionModeObserver>>> {
    EXECUTION_MODE_OBSERVER.get_or_init(|| RwLock::new(None))
}
fn replace_execution_mode_observer(replacement: Option<Arc<ExecutionModeObserver>>) {
    let previous = {
        let mut guard = match execution_mode_observer_slot().write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!(
                    target: "fastpq::planner",
                    "recovering poisoned execution mode observer registration lock"
                );
                let guard = poisoned.into_inner();
                execution_mode_observer_slot().clear_poison();
                guard
            }
        };
        core::mem::replace(&mut *guard, replacement)
    };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(previous))).is_err() {
        tracing::warn!(
            target: "fastpq::planner",
            "execution mode observer destructor panicked"
        );
    }
}
fn notify_execution_mode_observer(
    requested: ExecutionMode,
    resolved: ExecutionMode,
    backend: Option<GpuBackend>,
) {
    let observer = match execution_mode_observer_slot().read() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => {
            tracing::warn!(
                target: "fastpq::planner",
                "recovering poisoned execution mode observer notification lock"
            );
            let guard = poisoned.into_inner();
            execution_mode_observer_slot().clear_poison();
            guard.clone()
        }
    };
    if let Some(callback) = observer {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            callback(requested, resolved, backend)
        }));
        let drop_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(callback)));
        if result.is_err() {
            tracing::warn!(
                target: "fastpq::planner",
                requested = requested.as_str(),
                resolved = resolved.as_str(),
                "execution mode observer callback panicked"
            );
        }
        if drop_result.is_err() {
            tracing::warn!(
                target: "fastpq::planner",
                "execution mode observer destructor panicked"
            );
        }
    }
}
/// Install a hook invoked whenever an execution mode resolves to the concrete mode used.
///
/// The hook runs without holding the registration lock, may replace or clear itself, and its
/// panics are caught and logged.
pub fn set_execution_mode_observer<F>(observer: F)
where
    F: Fn(ExecutionMode, ExecutionMode, Option<GpuBackend>) + Send + Sync + 'static,
{
    let observer: Arc<ExecutionModeObserver> = Arc::new(observer);
    replace_execution_mode_observer(Some(observer));
}
/// Remove the previously installed execution mode observer, if any.
pub fn clear_execution_mode_observer() {
    replace_execution_mode_observer(None);
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
                        "FASTPQ_GPU=gpu requested but no accelerator was detected"
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
fn default_batch_execution_mode() -> ExecutionMode {
    if current_gpu_backend().is_some() {
        ExecutionMode::Gpu
    } else {
        ExecutionMode::Cpu
    }
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
            "No supported GPU backend detected"
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
        .with(
            GpuBackend::Cuda,
            cuda && gpu_backend_supported_in_build(GpuBackend::Cuda),
        )
        .with(
            GpuBackend::OpenCl,
            opencl && gpu_backend_supported_in_build(GpuBackend::OpenCl),
        )
        .with(
            GpuBackend::Metal,
            metal && gpu_backend_supported_in_build(GpuBackend::Metal),
        )
        .with(
            GpuBackend::Norito,
            norito && gpu_backend_supported_in_build(GpuBackend::Norito),
        );
    resolve_backend(availability)
}
fn gpu_backend_supported_in_build(backend: GpuBackend) -> bool {
    match backend {
        GpuBackend::Cuda => cfg!(all(feature = "fastpq-gpu", not(fastpq_cuda_unavailable))),
        GpuBackend::Metal => cfg!(all(feature = "fastpq-gpu", target_os = "macos")),
        // FASTPQ currently has no OpenCL or Norito FFT/LDE execution backend, so probing host
        // support must not opt the prover into those modes.
        GpuBackend::OpenCl | GpuBackend::Norito => false,
    }
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
    fn backend_support_map_excludes_unimplemented_backends() {
        assert!(!gpu_backend_supported_in_build(GpuBackend::OpenCl));
        assert!(!gpu_backend_supported_in_build(GpuBackend::Norito));
    }
    #[cfg(any(not(feature = "fastpq-gpu"), fastpq_cuda_unavailable))]
    #[test]
    fn cuda_backend_support_is_disabled_without_compiled_runtime() {
        assert!(!gpu_backend_supported_in_build(GpuBackend::Cuda));
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
    fn backend_config_defaults_to_cpu_execution_mode() {
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let config = BackendConfig::new(params);
        assert_eq!(config.execution_mode(), ExecutionMode::Cpu);
        assert_eq!(config.poseidon_mode(), PoseidonExecutionMode::Cpu);
    }
}
#[cfg(test)]
mod observer_tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;
    static OBSERVER_TEST_LOCK: Mutex<()> = Mutex::new(());
    struct ExecutionModeObserverGuard;
    impl Drop for ExecutionModeObserverGuard {
        fn drop(&mut self) {
            clear_execution_mode_observer();
        }
    }
    struct PoseidonPipelineObserverGuard;
    impl Drop for PoseidonPipelineObserverGuard {
        fn drop(&mut self) {
            crate::trace::clear_poseidon_pipeline_observer();
        }
    }
    struct TraceMerkleModeObserverGuard;
    impl Drop for TraceMerkleModeObserverGuard {
        fn drop(&mut self) {
            crate::trace::clear_trace_merkle_mode_observer();
        }
    }
    #[test]
    fn execution_mode_observer_receives_resolution() {
        let _lock = OBSERVER_TEST_LOCK.lock().expect("observer test lock");
        clear_execution_mode_observer();
        let _observer_guard = ExecutionModeObserverGuard;
        let (tx, rx) = mpsc::channel();
        let test_thread = std::thread::current().id();
        set_execution_mode_observer(move |requested, resolved, backend| {
            if std::thread::current().id() == test_thread {
                let _ = tx.send((requested, resolved, backend));
            }
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
    #[test]
    fn execution_mode_observer_panic_does_not_escape_or_block_reregistration() {
        let _lock = OBSERVER_TEST_LOCK.lock().expect("observer test lock");
        clear_execution_mode_observer();
        let _observer_guard = ExecutionModeObserverGuard;
        let test_thread = std::thread::current().id();
        set_execution_mode_observer(move |_, _, _| {
            assert!(
                std::thread::current().id() != test_thread,
                "observer failure"
            );
        });

        let result = std::panic::catch_unwind(|| {
            notify_execution_mode_observer(ExecutionMode::Auto, ExecutionMode::Cpu, None);
        });
        assert!(result.is_ok(), "observer panic must not escape resolution");

        let (tx, rx) = mpsc::channel();
        set_execution_mode_observer(move |requested, resolved, backend| {
            let _ = tx.send((requested, resolved, backend));
        });
        notify_execution_mode_observer(ExecutionMode::Auto, ExecutionMode::Cpu, None);
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(2))
                .expect("replacement observer payload"),
            (ExecutionMode::Auto, ExecutionMode::Cpu, None)
        );
    }
    #[test]
    fn execution_mode_observer_can_clear_itself_reentrantly() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let _lock = OBSERVER_TEST_LOCK.lock().expect("observer test lock");
        clear_execution_mode_observer();
        let _observer_guard = ExecutionModeObserverGuard;
        let observed = Arc::new(AtomicUsize::new(0));
        let observed_for_callback = Arc::clone(&observed);
        let test_thread = std::thread::current().id();
        set_execution_mode_observer(move |_, _, _| {
            if std::thread::current().id() == test_thread {
                observed_for_callback.fetch_add(1, Ordering::SeqCst);
                clear_execution_mode_observer();
            }
        });

        notify_execution_mode_observer(ExecutionMode::Auto, ExecutionMode::Cpu, None);
        notify_execution_mode_observer(ExecutionMode::Auto, ExecutionMode::Cpu, None);
        assert_eq!(observed.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn backend_prove_uses_configured_execution_and_poseidon_modes() {
        let _lock = OBSERVER_TEST_LOCK.lock().expect("observer test lock");
        let _poseidon_lock = crate::trace::POSEIDON_PIPELINE_OBSERVER_TEST_LOCK
            .lock()
            .expect("Poseidon observer test lock");
        clear_execution_mode_observer();
        crate::trace::clear_poseidon_pipeline_observer();
        let _observer_guard = ExecutionModeObserverGuard;
        let _poseidon_observer_guard = PoseidonPipelineObserverGuard;

        let (execution_tx, execution_rx) = mpsc::channel();
        let test_thread = std::thread::current().id();
        set_execution_mode_observer(move |requested, resolved, backend| {
            if std::thread::current().id() == test_thread {
                let _ = execution_tx.send((requested, resolved, backend));
            }
        });
        let (poseidon_tx, poseidon_rx) = mpsc::channel();
        crate::trace::set_poseidon_pipeline_observer(move |policy, path, backend| {
            if std::thread::current().id() == test_thread {
                let _ = poseidon_tx.send((policy, path, backend));
            }
        });

        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let backend = StarkBackend::new(
            BackendConfig::new(params)
                .with_execution_mode(ExecutionMode::Cpu)
                .with_poseidon_mode(PoseidonExecutionMode::Auto),
        );
        let batch = TransitionBatch::new(params.name, crate::PublicInputs::default());
        backend
            .prove(&batch, &PublicIO::default(), 1, 1)
            .expect("configured backend proof");

        let (requested, resolved, _) = execution_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("execution-mode event");
        assert_eq!(requested, ExecutionMode::Cpu);
        assert_eq!(resolved, ExecutionMode::Cpu);
        let (policy, path, _) = poseidon_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("Poseidon-policy event");
        assert_eq!(policy.requested(), PoseidonExecutionMode::Auto);
        assert_eq!(policy.resolved(), ExecutionMode::Cpu);
        assert_eq!(path, "cpu_fallback");
    }
    #[test]
    fn canonical_commitment_derivation_forces_cpu_trace_merkle_levels() {
        let _lock = OBSERVER_TEST_LOCK.lock().expect("observer test lock");
        crate::trace::clear_trace_merkle_mode_observer();
        let _observer_guard = TraceMerkleModeObserverGuard;
        let (tx, rx) = mpsc::channel();
        let test_thread = std::thread::current().id();
        crate::trace::set_trace_merkle_mode_observer(move |mode| {
            if std::thread::current().id() == test_thread {
                let _ = tx.send(mode);
            }
        });
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let batch = TransitionBatch::new(params.name, crate::PublicInputs::default());

        derive_batch_commitments(&params, &batch, &PublicIO::default(), 1, 1)
            .expect("canonical commitments");

        let modes = rx.try_iter().collect::<Vec<_>>();
        assert!(!modes.is_empty(), "trace Merkle mode must be observed");
        assert!(
            modes.iter().all(|mode| *mode == ExecutionMode::Cpu),
            "canonical commitment derivation must keep trace Merkle levels on CPU: {modes:?}"
        );
    }
}
#[cfg(target_os = "macos")]
fn metal_available() -> bool {
    // The offline Metal compiler is an optional Xcode component on recent macOS
    // releases. Runtime source compilation remains available through MTLDevice,
    // so accelerator discovery must reflect usable hardware rather than the
    // presence of a build-time `fastpq.metallib` artifact.
    metal_device_visible_via_api()
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
#[cfg(all(target_os = "macos", not(feature = "fastpq-gpu")))]
fn metal_device_visible_via_api() -> bool {
    false
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
#[cfg(not(target_os = "macos"))]
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
    .or_else(|| {
        option_env!("FASTPQ_METAL_LIB")
            .filter(|path| !path.is_empty() && Path::new(path).exists())
            .map(str::to_owned)
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
            execution_mode: ExecutionMode::Cpu,
            poseidon_mode: PoseidonExecutionMode::Cpu,
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
    /// Poseidon Merkle root over column commitments.
    pub trace_root: GoldilocksDigest384V1,
    /// Poseidon Merkle root over row-major AIR trace openings.
    pub air_trace_root: GoldilocksDigest384V1,
    /// Poseidon Merkle root over AIR composition evaluations.
    pub air_composition_root: GoldilocksDigest384V1,
    /// Poseidon Merkle root over the low-degree extension leaf hashes.
    pub lookup_root: GoldilocksDigest384V1,
    /// Number of evaluation rows committed under `lookup_root`.
    pub lde_domain_size: u32,
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
    /// Poseidon hash of each FRI layer plus the terminal root.
    pub fri_layers: Vec<GoldilocksDigest384V1>,
    /// Fiat–Shamir challenges used for each FRI folding round.
    pub fri_betas: Vec<GoldilocksFp4V1>,
    /// Sampled query openings into the evaluation domain.
    pub query_openings: Vec<(u32, u64)>,
    /// Full LDE leaf chunks containing each queried evaluation.
    pub query_chunks: Vec<Vec<u64>>,
    /// Merkle authentication paths for each queried evaluation chunk.
    pub query_paths: Vec<Vec<GoldilocksDigest384V1>>,
    /// Sampled AIR row/composition openings.
    pub air_openings: Vec<AirConstraintOpening>,
    /// Per-round FRI openings for sampled query indices.
    pub fri_query_openings: Vec<FriQueryOpening>,
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

fn digest_domain_v1<'a>(
    role: &'a [u8],
    phase: &'a [u8],
    level: usize,
    index: usize,
    counter: u64,
) -> Result<GoldilocksDigestDomainV1<'a>> {
    Ok(GoldilocksDigestDomainV1 {
        catalog: FASTPQ_CATALOG_V1.as_bytes(),
        protocol: FASTPQ_FINAL_V1_ID.as_bytes(),
        profile: FASTPQ_FINAL_V1_ID.as_bytes(),
        role,
        phase,
        level: u64::try_from(level).map_err(|_| Error::QueryIndexOverflow { index: level })?,
        index: u64::try_from(index).map_err(|_| Error::QueryIndexOverflow { index })?,
        counter,
    })
}

fn hash_bytes_v1(
    role: &[u8],
    phase: &[u8],
    level: usize,
    index: usize,
    counter: u64,
    fields: &[&[u8]],
) -> Result<GoldilocksDigest384V1> {
    hash_bytes_384_v1(
        digest_domain_v1(role, phase, level, index, counter)?,
        fields,
    )
    .ok_or_else(|| Error::PayloadLengthOverflow {
        length: fields
            .iter()
            .fold(0_usize, |total, field| total.saturating_add(field.len())),
    })
}

fn hash_u64_values_v1(
    role: &[u8],
    phase: &[u8],
    level: usize,
    index: usize,
    counter: u64,
    values: &[u64],
) -> Result<GoldilocksDigest384V1> {
    let mut bytes = Vec::with_capacity(values.len().saturating_mul(8));
    for value in values {
        if *value >= GOLDILOCKS_MODULUS {
            return Err(Error::NonCanonicalGoldilocksElement {
                context: "native_stark_digest_input",
                indices: vec![index],
            });
        }
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    hash_bytes_v1(role, phase, level, index, counter, &[&bytes])
}
/// Hash the low-degree extension evaluations into Merkle leaves grouped by the
/// canonical chunk size derived from the FRI arity.
///
/// # Errors
/// Returns an error if hashing a chunk into the Merkle leaf domain fails.
pub fn hash_lde_leaves(evaluations: &[u64], arity: u32) -> Result<Vec<GoldilocksDigest384V1>> {
    hash_lde_leaves_with_mode(evaluations, arity, default_batch_execution_mode())
}
fn hash_lde_leaves_with_mode(
    evaluations: &[u64],
    arity: u32,
    mode: ExecutionMode,
) -> Result<Vec<GoldilocksDigest384V1>> {
    let chunk = lde_chunk_size(arity)?;
    let _ = mode;
    if evaluations.is_empty() {
        return Ok(Vec::new());
    }
    evaluations
        .chunks(chunk)
        .enumerate()
        .map(|(index, values)| {
            hash_u64_values_v1(
                LOOKUP_COMMITMENT_ROLE_V1,
                MERKLE_LEAF_PHASE_V1,
                0,
                index,
                0,
                values,
            )
        })
        .collect()
}
/// Hash one LDE leaf chunk using the same domain as [`hash_lde_leaves`].
///
/// # Errors
/// Returns an error if the leaf index cannot be represented as a field limb.
pub fn hash_lde_chunk(leaf_index: usize, values: &[u64]) -> Result<GoldilocksDigest384V1> {
    hash_u64_values_v1(
        LOOKUP_COMMITMENT_ROLE_V1,
        MERKLE_LEAF_PHASE_V1,
        0,
        leaf_index,
        0,
        values,
    )
}
/// Hash one row-major AIR trace opening.
///
/// # Errors
/// Returns an error if the row index cannot be represented as a field limb.
pub fn hash_air_trace_row(row_index: usize, values: &[u64]) -> Result<GoldilocksDigest384V1> {
    hash_u64_values_v1(
        AIR_TRACE_COMMITMENT_ROLE_V1,
        MERKLE_LEAF_PHASE_V1,
        0,
        row_index,
        0,
        values,
    )
}
/// Hash one AIR composition leaf.
///
/// # Errors
/// Returns an error if the leaf index cannot be represented as a field limb.
pub fn hash_air_composition_leaf(index: usize, value: u64) -> Result<GoldilocksDigest384V1> {
    hash_u64_values_v1(
        AIR_COMPOSITION_COMMITMENT_ROLE_V1,
        MERKLE_LEAF_PHASE_V1,
        0,
        index,
        0,
        &[value],
    )
}
/// Hash all row-major AIR trace leaves.
///
/// # Errors
/// Returns an error if row hashing fails.
fn hash_air_trace_rows_with_mode(
    columns: &[Vec<u64>],
    mode: ExecutionMode,
) -> Result<Vec<GoldilocksDigest384V1>> {
    let _ = mode;
    if columns.is_empty() {
        return Ok(Vec::new());
    }
    let row_count = columns[0].len();
    if !columns.iter().all(|column| column.len() == row_count) {
        return Err(Error::AirOpeningMismatch { index: 0 });
    }
    (0..row_count)
        .map(|row_index| {
            let row = air_row_at(columns, row_index)?;
            hash_air_trace_row(row_index, &row)
        })
        .collect()
}
/// Hash all AIR composition leaves.
///
/// # Errors
/// Returns an error if leaf hashing fails.
fn hash_air_composition_leaves_with_mode(
    values: &[u64],
    mode: ExecutionMode,
) -> Result<Vec<GoldilocksDigest384V1>> {
    let _ = mode;
    values
        .iter()
        .copied()
        .enumerate()
        .map(|(index, value)| hash_air_composition_leaf(index, value))
        .collect()
}

fn packed_column_value(column_names: &[String], row: &[u64], prefix: &str) -> u64 {
    let mut value = 0u64;
    let mut radix_power = FIELD_ONE;
    let limb_radix = 1u64 << 56;
    for limb in 0usize.. {
        let name = format!("{prefix}{limb}");
        let Some(index) = column_names.iter().position(|column| column == &name) else {
            break;
        };
        value = add_mod(value, mul_mod(row[index], radix_power));
        radix_power = mul_mod(radix_power, limb_radix);
    }
    value
}

fn air_constraint_residues_for_rows(
    column_names: &[String],
    current: &[u64],
    next: &[u64],
) -> Result<Vec<u64>> {
    if current.len() != column_names.len() || next.len() != column_names.len() {
        return Err(Error::AirOpeningMismatch {
            index: current.len(),
        });
    }
    let get = |name: &str| -> Result<usize> {
        column_names
            .iter()
            .position(|column| column == name)
            .ok_or_else(|| Error::MissingColumn(name.to_owned()))
    };
    let s_active = get("s_active")?;
    let s_transfer = get("s_transfer")?;
    let s_mint = get("s_mint")?;
    let s_burn = get("s_burn")?;
    let s_role_grant = get("s_role_grant")?;
    let s_role_revoke = get("s_role_revoke")?;
    let s_meta_set = get("s_meta_set")?;
    let s_perm = get("s_perm")?;
    let delta = get("delta")?;
    let metadata_hash_limbs = (0..crate::trace::METADATA_COMMITMENT_LIMBS)
        .map(|limb| get(&format!("metadata_hash_limb_{limb}")))
        .collect::<Result<Vec<_>>>()?;
    let dsid = get("dsid")?;
    let slot = get("slot")?;
    let mut residues = Vec::with_capacity(AIR_COMPOSITION_ALPHA_COUNT);
    for selector in [
        s_active,
        s_transfer,
        s_mint,
        s_burn,
        s_role_grant,
        s_role_revoke,
        s_meta_set,
        s_perm,
    ] {
        residues.push(mul_mod(
            current[selector],
            sub_mod(current[selector], FIELD_ONE),
        ));
    }
    let operation_sum = [
        s_transfer,
        s_mint,
        s_burn,
        s_role_grant,
        s_role_revoke,
        s_meta_set,
    ]
    .into_iter()
    .fold(0u64, |sum, selector| add_mod(sum, current[selector]));
    residues.push(sub_mod(current[s_active], operation_sum));
    let permission_sum = add_mod(current[s_role_grant], current[s_role_revoke]);
    residues.push(sub_mod(current[s_perm], permission_sum));
    residues.push(mul_mod(
        next[s_active],
        sub_mod(FIELD_ONE, current[s_active]),
    ));
    let numeric_selector = [s_transfer, s_mint, s_burn]
        .into_iter()
        .fold(0u64, |sum, selector| add_mod(sum, current[selector]));
    let value_old = packed_column_value(column_names, current, "value_old_limb_");
    let value_new = packed_column_value(column_names, current, "value_new_limb_");
    let expected_delta = sub_mod(value_new, value_old);
    residues.push(mul_mod(
        numeric_selector,
        sub_mod(expected_delta, current[delta]),
    ));
    for stable in metadata_hash_limbs.into_iter().chain([dsid, slot]) {
        residues.push(sub_mod(current[stable], next[stable]));
    }
    Ok(residues)
}

/// Evaluate the sampled FASTPQ AIR composition value for two adjacent rows.
///
/// # Errors
/// Returns an error when the advertised column schema is missing mandatory columns, the
/// challenge count differs from the residue count, or row widths do not match the schema.
pub fn air_composition_value_for_rows(
    column_names: &[String],
    current: &[u64],
    next: &[u64],
    alphas: &[u64],
) -> Result<u64> {
    if alphas.len() != AIR_COMPOSITION_ALPHA_COUNT {
        return Err(Error::AirChallengeCountMismatch {
            expected: AIR_COMPOSITION_ALPHA_COUNT,
            actual: alphas.len(),
        });
    }
    let residues = air_constraint_residues_for_rows(column_names, current, next)?;
    if residues.len() != AIR_COMPOSITION_ALPHA_COUNT {
        return Err(Error::AirChallengeCountMismatch {
            expected: AIR_COMPOSITION_ALPHA_COUNT,
            actual: residues.len(),
        });
    }
    Ok(alphas
        .iter()
        .zip(residues)
        .fold(0u64, |acc, (&alpha, residue)| {
            add_mod(acc, mul_mod(alpha, residue))
        }))
}
/// Evaluate FASTPQ AIR composition values over all row openings.
///
/// # Errors
/// Returns an error when columns have inconsistent lengths or the schema is malformed.
pub fn air_composition_values(
    column_names: &[String],
    columns: &[Vec<u64>],
    alphas: &[u64],
    next_step: usize,
) -> Result<Vec<u64>> {
    if columns.is_empty() {
        return Ok(Vec::new());
    }
    let row_count = columns[0].len();
    if !columns.iter().all(|column| column.len() == row_count) {
        return Err(Error::AirOpeningMismatch { index: 0 });
    }
    if row_count != 0 && next_step == 0 {
        return Err(Error::QueryIndexOutOfRange {
            index: 0,
            len: row_count,
        });
    }
    (0..row_count)
        .map(|index| {
            let current = air_row_at(columns, index)?;
            let next = air_row_at(columns, (index + next_step) % row_count)?;
            air_composition_value_for_rows(column_names, &current, &next, alphas)
        })
        .collect()
}
fn air_row_at(columns: &[Vec<u64>], row_index: usize) -> Result<Vec<u64>> {
    columns
        .iter()
        .map(|column| {
            column
                .get(row_index)
                .copied()
                .ok_or(Error::QueryIndexOutOfRange {
                    index: row_index,
                    len: column.len(),
                })
        })
        .collect()
}

fn ensure_base_trace_constraints(trace: &crate::trace::Trace) -> Result<()> {
    let column_names = trace
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    for row_index in 0..trace.padded_len {
        let next_index = row_index
            .checked_add(1)
            .filter(|next| *next < trace.padded_len)
            .unwrap_or(row_index);
        let current = trace
            .columns
            .iter()
            .map(|column| column.values[row_index])
            .collect::<Vec<_>>();
        let next = trace
            .columns
            .iter()
            .map(|column| column.values[next_index])
            .collect::<Vec<_>>();
        let residues = air_constraint_residues_for_rows(&column_names, &current, &next)?;
        if residues.len() != AIR_COMPOSITION_ALPHA_COUNT || residues.iter().any(|value| *value != 0)
        {
            return Err(Error::AirConstraintMismatch { index: row_index });
        }
    }
    Ok(())
}

/// Open sampled AIR rows and composition values.
///
/// # Errors
/// Returns an error when any sampled index is outside the AIR domain.
fn open_air_constraint_openings_with_mode(
    columns: &[Vec<u64>],
    air_trace_leaves: &[GoldilocksDigest384V1],
    composition_values: &[u64],
    composition_leaves: &[GoldilocksDigest384V1],
    query_indices: &[usize],
    next_step: usize,
    mode: ExecutionMode,
) -> Result<Vec<AirConstraintOpening>> {
    if columns.is_empty() {
        return Ok(Vec::new());
    }
    let row_count = columns[0].len();
    if next_step == 0 {
        return Err(Error::QueryIndexOutOfRange {
            index: 0,
            len: row_count,
        });
    }
    let row_paths = merkle_paths_for_leaf_indices(
        air_trace_leaves,
        query_indices,
        MerkleTreeRoleV1::AirTrace,
        mode,
    )?;
    let next_indices: Vec<usize> = query_indices
        .iter()
        .map(|index| (index + next_step) % row_count)
        .collect();
    let next_paths = merkle_paths_for_leaf_indices(
        air_trace_leaves,
        &next_indices,
        MerkleTreeRoleV1::AirTrace,
        mode,
    )?;
    let composition_paths = merkle_paths_for_leaf_indices(
        composition_leaves,
        query_indices,
        MerkleTreeRoleV1::AirComposition,
        mode,
    )?;
    query_indices
        .iter()
        .copied()
        .zip(row_paths)
        .zip(next_indices.into_iter().zip(next_paths))
        .zip(composition_paths)
        .map(
            |(((index, current_row_path), (next_index, next_row_path)), composition_path)| {
                let compact =
                    u32::try_from(index).map_err(|_| Error::QueryIndexOverflow { index })?;
                Ok(AirConstraintOpening {
                    index: compact,
                    current_row: air_row_at(columns, index)?,
                    next_row: air_row_at(columns, next_index)?,
                    current_row_path: current_row_path
                        .into_iter()
                        .map(GoldilocksDigest384V1::to_le_bytes)
                        .collect(),
                    next_row_path: next_row_path
                        .into_iter()
                        .map(GoldilocksDigest384V1::to_le_bytes)
                        .collect(),
                    composition_value: *composition_values.get(index).ok_or(
                        Error::QueryIndexOutOfRange {
                            index,
                            len: composition_values.len(),
                        },
                    )?,
                    composition_path: composition_path
                        .into_iter()
                        .map(GoldilocksDigest384V1::to_le_bytes)
                        .collect(),
                })
            },
        )
        .collect()
}
/// Hash one FRI round leaf with domain separation from LDE openings.
///
/// # Errors
/// Returns an error if the leaf coordinates cannot be represented.
pub fn hash_fri_chunk(
    round: usize,
    leaf_index: usize,
    values: &[GoldilocksFp4V1],
) -> Result<GoldilocksDigest384V1> {
    let mut bytes = Vec::with_capacity(values.len().saturating_mul(32));
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    hash_bytes_v1(
        FRI_COMMITMENT_ROLE_V1,
        MERKLE_LEAF_PHASE_V1,
        round,
        leaf_index,
        0,
        &[&bytes],
    )
}
/// Return the V1 chunk size (number of evaluations per LDE leaf hash).
///
/// # Errors
/// Returns [`Error::FriArity`] unless `arity` is the sole V1 binary-FRI arity.
pub fn lde_chunk_size(arity: u32) -> Result<usize> {
    ensure_binary_fri_arity(arity)?;
    Ok(64)
}

fn ensure_binary_fri_arity(arity: u32) -> Result<()> {
    if arity == 2 {
        Ok(())
    } else {
        Err(Error::FriArity(arity))
    }
}

fn fri_chunk_size(arity: u32) -> Result<usize> {
    ensure_binary_fri_arity(arity)?;
    Ok(2)
}
/// Open the full LDE leaf chunks that contain the supplied query indices.
///
/// # Errors
/// Returns an error when any query index is outside the evaluation domain.
pub fn open_query_chunks(
    evaluations: &[u64],
    query_indices: &[usize],
    arity: u32,
) -> Result<Vec<Vec<u64>>> {
    let chunk_size = lde_chunk_size(arity)?;
    let mut chunks = Vec::with_capacity(query_indices.len());
    for &query_index in query_indices {
        if query_index >= evaluations.len() {
            return Err(Error::QueryIndexOutOfRange {
                index: query_index,
                len: evaluations.len(),
            });
        }
        let start = (query_index / chunk_size) * chunk_size;
        let end = start.saturating_add(chunk_size).min(evaluations.len());
        chunks.push(evaluations[start..end].to_vec());
    }
    Ok(chunks)
}
/// Compute Merkle authentication paths for the supplied query indices over the provided leaf set.
///
/// The query indices are expressed in terms of evaluation positions; internally they are grouped
/// into leaf chunks using the canonical chunk size derived from the FRI arity.
///
/// # Errors
/// Returns an error if hashing an internal Merkle level fails.
pub fn merkle_paths_for_queries(
    leaves: &[GoldilocksDigest384V1],
    query_indices: &[usize],
    arity: u32,
    evaluation_len: usize,
) -> Result<Vec<Vec<GoldilocksDigest384V1>>> {
    merkle_paths_for_queries_with_mode(
        leaves,
        query_indices,
        arity,
        evaluation_len,
        MerkleTreeRoleV1::Lookup,
        default_batch_execution_mode(),
    )
}
fn merkle_paths_for_queries_with_mode(
    leaves: &[GoldilocksDigest384V1],
    query_indices: &[usize],
    arity: u32,
    evaluation_len: usize,
    role: MerkleTreeRoleV1,
    mode: ExecutionMode,
) -> Result<Vec<Vec<GoldilocksDigest384V1>>> {
    let chunk_size = lde_chunk_size(arity)?;
    if query_indices.is_empty() {
        return Ok(Vec::new());
    }
    if evaluation_len == 0 || leaves.is_empty() {
        return Err(Error::QueryIndexOutOfRange {
            index: query_indices[0],
            len: evaluation_len,
        });
    }
    let levels = build_merkle_levels_with_mode(leaves, role, mode)?;
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
fn merkle_paths_for_leaf_indices(
    leaves: &[GoldilocksDigest384V1],
    leaf_indices: &[usize],
    role: MerkleTreeRoleV1,
    mode: ExecutionMode,
) -> Result<Vec<Vec<GoldilocksDigest384V1>>> {
    if leaf_indices.is_empty() {
        return Ok(Vec::new());
    }
    if leaves.is_empty() {
        return Err(Error::QueryIndexOutOfRange {
            index: leaf_indices[0],
            len: 0,
        });
    }
    let levels = build_merkle_levels_with_mode(leaves, role, mode)?;
    let leaf_count = levels
        .first()
        .expect("non-empty levels for non-empty leaves")
        .len();
    let mut paths = Vec::with_capacity(leaf_indices.len());
    for &leaf_index in leaf_indices {
        if leaf_index >= leaf_count {
            return Err(Error::QueryIndexOutOfRange {
                index: leaf_index,
                len: leaf_count,
            });
        }
        let mut index = leaf_index;
        let mut path = Vec::with_capacity(levels.len().saturating_sub(1));
        for level in levels.iter().take(levels.len().saturating_sub(1)) {
            let sibling_idx = if index.is_multiple_of(2) {
                index + 1
            } else {
                index.saturating_sub(1)
            };
            let sibling = level
                .get(sibling_idx)
                .copied()
                .unwrap_or_else(|| level[index]);
            path.push(sibling);
            index /= 2;
        }
        paths.push(path);
    }
    Ok(paths)
}
/// Verify a Merkle authentication path over FASTPQ Poseidon field leaves.
///
/// # Errors
/// Returns an error if an internal node hash cannot be computed.
#[allow(clippy::unnecessary_wraps)]
pub fn verify_merkle_path(
    root: GoldilocksDigest384V1,
    leaf: GoldilocksDigest384V1,
    leaf_index: usize,
    path: &[GoldilocksDigest384V1],
) -> Result<bool> {
    verify_merkle_path_for_role(MerkleTreeRoleV1::Lookup, root, leaf, leaf_index, path)
}

/// Verify a Merkle authentication path under an explicit native-STARK tree role.
///
/// # Errors
/// Returns an error when a typed internal-node digest cannot be framed.
pub fn verify_merkle_path_for_role(
    role: MerkleTreeRoleV1,
    root: GoldilocksDigest384V1,
    leaf: GoldilocksDigest384V1,
    leaf_index: usize,
    path: &[GoldilocksDigest384V1],
) -> Result<bool> {
    // FASTPQ's canonical tree duplicates a sole leaf and therefore always has
    // at least one authentication level. Accepting an empty path would treat a
    // raw leaf as a root even though the tree builder never emits that shape.
    if path.is_empty() {
        return Ok(false);
    }
    let mut current = leaf;
    let mut index = leaf_index;
    for (level, &sibling) in path.iter().enumerate() {
        let parent_index = index / 2;
        current = if index.is_multiple_of(2) {
            merkle_node_hash(role, level + 1, parent_index, current, sibling)?
        } else {
            merkle_node_hash(role, level + 1, parent_index, sibling, current)?
        };
        index /= 2;
    }
    // Reject indices whose high bits lie above the authenticated tree depth.
    // Without this check, `i` and `i + k * 2^path.len()` select identical
    // left/right branches and therefore accept the same path.
    Ok(index == 0 && current == root)
}
#[allow(clippy::unnecessary_wraps)]
fn build_merkle_levels_with_mode(
    leaves: &[GoldilocksDigest384V1],
    role: MerkleTreeRoleV1,
    mode: ExecutionMode,
) -> Result<Vec<Vec<GoldilocksDigest384V1>>> {
    let _ = mode;
    if leaves.is_empty() {
        return Ok(Vec::new());
    }
    let mut levels = Vec::new();
    let mut current = leaves.to_vec();
    loop {
        if current.len() % 2 == 1 {
            let last = *current.last().expect("non-empty Merkle level");
            current.push(last);
        }
        levels.push(current.clone());
        let level = levels.len();
        let next = current
            .chunks_exact(2)
            .enumerate()
            .map(|(index, pair)| merkle_node_hash(role, level, index, pair[0], pair[1]))
            .collect::<Result<Vec<_>>>()?;
        if next.len() == 1 {
            levels.push(next.clone());
            break;
        }
        current = next;
    }
    Ok(levels)
}
fn merkle_root_with_mode(
    leaves: &[GoldilocksDigest384V1],
    role: MerkleTreeRoleV1,
    mode: ExecutionMode,
) -> Result<GoldilocksDigest384V1> {
    let levels = build_merkle_levels_with_mode(leaves, role, mode)?;
    match levels.last().and_then(|level| level.first()).copied() {
        Some(root) => Ok(root),
        None => hash_bytes_v1(
            role.role(),
            MERKLE_EMPTY_PHASE_V1,
            0,
            0,
            role.counter(),
            &[],
        ),
    }
}

#[cfg(test)]
pub(crate) fn merkle_root_for_role(
    leaves: &[GoldilocksDigest384V1],
    role: MerkleTreeRoleV1,
) -> Result<GoldilocksDigest384V1> {
    merkle_root_with_mode(leaves, role, ExecutionMode::Cpu)
}
fn merkle_node_hash(
    role: MerkleTreeRoleV1,
    level: usize,
    index: usize,
    left: GoldilocksDigest384V1,
    right: GoldilocksDigest384V1,
) -> Result<GoldilocksDigest384V1> {
    hash_bytes_v1(
        role.role(),
        MERKLE_NODE_PHASE_V1,
        level,
        index,
        role.counter(),
        &[&left.to_le_bytes(), &right.to_le_bytes()],
    )
}
/// Compute the Fiat–Shamir lookup grand product accumulator over the supplied
/// selector and witness evaluations using the canonical Goldilocks field
/// arithmetic.
///
/// # Errors
///
/// Returns [`Error::LookupColumnLengthMismatch`] when the selector and witness
/// columns do not contain the same number of evaluations.
pub fn compute_lookup_grand_product(
    selector_values: &[u64],
    witness_values: &[u64],
    gamma: u64,
) -> Result<u64> {
    if selector_values.len() != witness_values.len() {
        return Err(Error::LookupColumnLengthMismatch {
            selector_len: selector_values.len(),
            witness_len: witness_values.len(),
        });
    }
    if gamma >= GOLDILOCKS_MODULUS {
        return Err(Error::NonCanonicalGoldilocksElement {
            context: "lookup_gamma",
            indices: Vec::new(),
        });
    }
    let mut acc = FIELD_ONE;
    for (index, (&selector, &witness)) in selector_values
        .iter()
        .zip(witness_values.iter())
        .enumerate()
    {
        if selector >= GOLDILOCKS_MODULUS || witness >= GOLDILOCKS_MODULUS {
            return Err(Error::NonCanonicalGoldilocksElement {
                context: "lookup_grand_product_input",
                indices: vec![index],
            });
        }
        if selector != 0 {
            acc = mul_mod(acc, add_mod(witness, gamma));
        }
    }
    Ok(acc)
}
#[cfg(test)]
pub fn fold_with_fri(
    evaluations: &[u64],
    arity: u32,
    max_reductions: u32,
    lde_root: u64,
    lde_log_size: u32,
    domain_offset: u64,
    transcript: &mut Transcript,
) -> Result<(Vec<GoldilocksDigest384V1>, Vec<GoldilocksFp4V1>)> {
    if arity != 2 {
        return Err(Error::FriArity(arity));
    }
    if evaluations.is_empty() {
        let root = merkle_root_with_mode(&[], MerkleTreeRoleV1::Fri(0), ExecutionMode::Cpu)?;
        transcript.append_fri_final(root);
        return Ok((vec![root], Vec::new()));
    }
    let arity = usize::try_from(arity).expect("FRI arity fits usize");
    let max_rounds = usize::try_from(max_reductions).expect("FRI reduction bound fits usize");
    let mut current = evaluations
        .iter()
        .copied()
        .map(|value| GoldilocksFp4V1::from_base(value).expect("evaluations are canonical"))
        .collect::<Vec<_>>();
    let mut domain =
        FriDomain::from_lde_parameters(lde_root, lde_log_size, current.len(), domain_offset)?;
    let mut layers = Vec::new();
    let mut betas = Vec::new();
    let mut round = 0usize;
    while current.len() > arity && round < max_rounds {
        let span = tracing::info_span!("fastpq_fri_round", round, layer_len = current.len(), arity);
        let _enter = span.enter();
        let leaves = hash_fri_leaves_with_mode(round, &current, arity as u32, ExecutionMode::Cpu)?;
        let root = merkle_root_with_mode(
            &leaves,
            MerkleTreeRoleV1::Fri(
                u32::try_from(round).map_err(|_| Error::QueryIndexOverflow { index: round })?,
            ),
            ExecutionMode::Cpu,
        )?;
        transcript.append_fri_layer(round, root);
        layers.push(root);
        let beta = transcript.challenge_beta(round);
        betas.push(beta);
        let round_arity = fri_round_arity(current.len(), arity)?;
        tracing::debug!(
            round,
            layer_len = current.len(),
            round_arity,
            "folding FRI layer with beta"
        );
        let next = fold_round(&current, arity, beta, domain)?;
        let next_len = next.len();
        tracing::info!(
            round,
            ?root,
            ?beta,
            layer_len = current.len(),
            round_arity,
            next_len,
            "fri round committed"
        );
        current = next;
        domain = domain.folded(round_arity);
        round += 1;
    }
    if current.len() > arity {
        return Err(Error::FriReductionLimit {
            max_reductions,
            remaining: current.len(),
            arity,
        });
    }
    let final_leaves =
        hash_fri_leaves_with_mode(round, &current, arity as u32, ExecutionMode::Cpu)?;
    let final_root = merkle_root_with_mode(
        &final_leaves,
        MerkleTreeRoleV1::Fri(
            u32::try_from(round).map_err(|_| Error::QueryIndexOverflow { index: round })?,
        ),
        ExecutionMode::Cpu,
    )?;
    transcript.append_fri_final(final_root);
    tracing::info!(round, ?final_root, "final FRI layer commitment");
    layers.push(final_root);
    Ok((layers, betas))
}
struct FriOpeningLayers {
    layer_values: Vec<Vec<GoldilocksFp4V1>>,
    roots: Vec<GoldilocksDigest384V1>,
    betas: Vec<GoldilocksFp4V1>,
}

fn fold_with_fri_opening_layers(
    evaluations: &[u64],
    params: &StarkParameterSet,
    transcript: &mut Transcript,
    mode: ExecutionMode,
) -> Result<FriOpeningLayers> {
    let arity = params.fri.arity;
    if arity != 2 {
        return Err(Error::FriArity(arity));
    }
    if evaluations.is_empty() {
        let root = merkle_root_with_mode(&[], MerkleTreeRoleV1::Fri(0), mode)?;
        transcript.append_fri_final(root);
        return Ok(FriOpeningLayers {
            layer_values: vec![Vec::new()],
            roots: vec![root],
            betas: Vec::new(),
        });
    }
    let arity_usize = usize::try_from(arity).expect("FRI arity fits usize");
    let max_rounds =
        usize::try_from(params.fri.max_reductions).expect("FRI reduction bound fits usize");
    let mut current = evaluations
        .iter()
        .copied()
        .map(|value| GoldilocksFp4V1::from_base(value).expect("evaluations are canonical"))
        .collect::<Vec<_>>();
    let mut domain = FriDomain::from_lde_parameters(
        params.lde_root,
        params.lde_log_size,
        current.len(),
        params.omega_coset,
    )?;
    let mut layer_values = Vec::new();
    let mut roots = Vec::new();
    let mut betas = Vec::new();
    let mut round = 0usize;
    while current.len() > arity_usize && round < max_rounds {
        let leaves = hash_fri_leaves_with_mode(round, &current, arity, mode)?;
        let root = merkle_root_with_mode(
            &leaves,
            MerkleTreeRoleV1::Fri(
                u32::try_from(round).map_err(|_| Error::QueryIndexOverflow { index: round })?,
            ),
            mode,
        )?;
        transcript.append_fri_layer(round, root);
        roots.push(root);
        layer_values.push(current.clone());
        let beta = transcript.challenge_beta(round);
        betas.push(beta);
        let round_arity = fri_round_arity(current.len(), arity_usize)?;
        current = fold_round(&current, arity_usize, beta, domain)?;
        domain = domain.folded(round_arity);
        round += 1;
    }
    if current.len() > arity_usize {
        return Err(Error::FriReductionLimit {
            max_reductions: params.fri.max_reductions,
            remaining: current.len(),
            arity: arity_usize,
        });
    }
    let leaves = hash_fri_leaves_with_mode(round, &current, arity, mode)?;
    let final_root = merkle_root_with_mode(
        &leaves,
        MerkleTreeRoleV1::Fri(
            u32::try_from(round).map_err(|_| Error::QueryIndexOverflow { index: round })?,
        ),
        mode,
    )?;
    transcript.append_fri_final(final_root);
    roots.push(final_root);
    layer_values.push(current);
    Ok(FriOpeningLayers {
        layer_values,
        roots,
        betas,
    })
}
fn hash_fri_leaves_with_mode(
    round: usize,
    values: &[GoldilocksFp4V1],
    arity: u32,
    mode: ExecutionMode,
) -> Result<Vec<GoldilocksDigest384V1>> {
    let configured_arity = fri_chunk_size(arity)?;
    let _ = mode;
    if values.is_empty() {
        return Ok(Vec::new());
    }
    let round_arity = fri_round_arity(values.len(), configured_arity)?;
    let output_len = values.len() / round_arity;
    (0..output_len)
        .map(|leaf_index| {
            let group = (0..round_arity)
                .map(|position| values[leaf_index + position * output_len])
                .collect::<Vec<_>>();
            hash_fri_chunk(round, leaf_index, &group)
        })
        .collect()
}
fn open_fri_query_chains(
    layer_values: &[Vec<GoldilocksFp4V1>],
    query_indices: &[usize],
    arity: u32,
    mode: ExecutionMode,
) -> Result<Vec<FriQueryOpening>> {
    let arity_usize = fri_chunk_size(arity)?;
    if layer_values.is_empty() {
        return Ok(Vec::new());
    }
    let mut round_leaves = Vec::with_capacity(layer_values.len());
    for (round, values) in layer_values.iter().enumerate() {
        round_leaves.push(hash_fri_leaves_with_mode(round, values, arity, mode)?);
    }
    let mut openings = Vec::with_capacity(query_indices.len());
    for &initial_index in query_indices {
        let initial_index_u32 =
            u32::try_from(initial_index).map_err(|_| Error::QueryIndexOverflow {
                index: initial_index,
            })?;
        let mut index = initial_index;
        let mut rounds = Vec::with_capacity(layer_values.len().saturating_sub(1));
        for round in 0..layer_values.len().saturating_sub(1) {
            let values = &layer_values[round];
            if index >= values.len() {
                return Err(Error::QueryIndexOutOfRange {
                    index,
                    len: values.len(),
                });
            }
            let round_arity = fri_round_arity(values.len(), arity_usize)?;
            let output_len = values.len() / round_arity;
            let leaf_index = index % output_len;
            let group = (0..round_arity)
                .map(|position| values[leaf_index + position * output_len])
                .collect::<Vec<_>>();
            let paths = merkle_paths_for_leaf_indices(
                &round_leaves[round],
                &[leaf_index],
                MerkleTreeRoleV1::Fri(
                    u32::try_from(round).map_err(|_| Error::QueryIndexOverflow { index: round })?,
                ),
                mode,
            )?;
            let folded_index = leaf_index;
            let folded_value = layer_values
                .get(round + 1)
                .and_then(|next| next.get(folded_index))
                .copied()
                .ok_or_else(|| Error::QueryIndexOutOfRange {
                    index: folded_index,
                    len: layer_values.get(round + 1).map_or(0, Vec::len),
                })?;
            rounds.push(FriRoundOpening {
                round: u32::try_from(round)
                    .map_err(|_| Error::QueryIndexOverflow { index: round })?,
                index: u32::try_from(index).map_err(|_| Error::QueryIndexOverflow { index })?,
                values: group,
                folded_value,
                merkle_path: paths
                    .into_iter()
                    .next()
                    .unwrap_or_default()
                    .into_iter()
                    .map(GoldilocksDigest384V1::to_le_bytes)
                    .collect(),
            });
            index = folded_index;
        }
        let final_values = layer_values
            .last()
            .expect("non-empty layer values")
            .as_slice();
        if index >= final_values.len() {
            return Err(Error::QueryIndexOutOfRange {
                index,
                len: final_values.len(),
            });
        }
        let final_arity = fri_round_arity(final_values.len(), arity_usize)?;
        let final_leaf_count = final_values.len() / final_arity;
        let final_leaf_index = index % final_leaf_count;
        let final_group = (0..final_arity)
            .map(|position| final_values[final_leaf_index + position * final_leaf_count])
            .collect::<Vec<_>>();
        let final_paths = merkle_paths_for_leaf_indices(
            round_leaves.last().expect("final leaves"),
            &[final_leaf_index],
            MerkleTreeRoleV1::Fri(u32::try_from(layer_values.len() - 1).map_err(|_| {
                Error::QueryIndexOverflow {
                    index: layer_values.len() - 1,
                }
            })?),
            mode,
        )?;
        openings.push(FriQueryOpening {
            initial_index: initial_index_u32,
            rounds,
            final_index: u32::try_from(index).map_err(|_| Error::QueryIndexOverflow { index })?,
            final_values: final_group,
            final_merkle_path: final_paths
                .into_iter()
                .next()
                .unwrap_or_default()
                .into_iter()
                .map(GoldilocksDigest384V1::to_le_bytes)
                .collect(),
        });
    }
    Ok(openings)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FriDomain {
    generator: u64,
    offset: u64,
}

impl FriDomain {
    pub fn from_lde_parameters(
        lde_root: u64,
        lde_log_size: u32,
        domain_size: usize,
        offset: u64,
    ) -> Result<Self> {
        if domain_size == 0 || !domain_size.is_power_of_two() {
            return Err(Error::FriDomainSize {
                length: domain_size,
                arity: 1,
            });
        }
        let domain_log = domain_size.ilog2();
        if domain_log > lde_log_size {
            return Err(Error::FriDomainSize {
                length: domain_size,
                arity: 1usize << lde_log_size,
            });
        }
        let root_stride = 1u64 << (lde_log_size - domain_log);
        Ok(Self {
            generator: field_pow(lde_root, root_stride),
            offset,
        })
    }

    pub fn point(self, index: usize) -> u64 {
        mul_mod(
            self.offset,
            field_pow(
                self.generator,
                u64::try_from(index).expect("FRI domain index fits u64"),
            ),
        )
    }

    pub fn coset_generator(self, output_len: usize) -> u64 {
        field_pow(
            self.generator,
            u64::try_from(output_len).expect("FRI output length fits u64"),
        )
    }

    pub fn folded(self, arity: usize) -> Self {
        let exponent = u64::try_from(arity).expect("FRI arity fits u64");
        Self {
            generator: field_pow(self.generator, exponent),
            offset: field_pow(self.offset, exponent),
        }
    }

    /// Return whether evaluations on this domain encode a polynomial below
    /// the exclusive degree bound.
    pub(crate) fn evaluations_have_degree_below(
        self,
        values: &[GoldilocksFp4V1],
        degree_bound: usize,
    ) -> Result<bool> {
        if values.is_empty() || !values.len().is_power_of_two() {
            return Err(Error::FriDomainSize {
                length: values.len(),
                arity: 1,
            });
        }
        if degree_bound == 0 || degree_bound > values.len() {
            return Err(Error::FriTerminalDegreeBound {
                degree_bound,
                domain_len: values.len(),
            });
        }
        let inverse_len =
            field_inverse(
                u64::try_from(values.len()).map_err(|_| Error::FriDomainSize {
                    length: values.len(),
                    arity: values.len(),
                })?,
            );
        let inverse_generator = field_inverse(self.generator);
        let mut inverse_frequency = field_pow(
            inverse_generator,
            u64::try_from(degree_bound).expect("terminal FRI degree bound fits u64"),
        );
        for _degree in degree_bound..values.len() {
            let mut coefficient = GoldilocksFp4V1::ZERO;
            let mut twiddle = FIELD_ONE;
            for &value in values {
                coefficient = coefficient.add(value.mul_base(twiddle));
                twiddle = mul_mod(twiddle, inverse_frequency);
            }
            if !coefficient.mul_base(inverse_len).is_zero() {
                return Ok(false);
            }
            inverse_frequency = mul_mod(inverse_frequency, inverse_generator);
        }
        Ok(true)
    }
}

pub fn fri_round_arity(length: usize, configured_arity: usize) -> Result<usize> {
    if configured_arity != 2 {
        return Err(Error::FriArity(
            u32::try_from(configured_arity).unwrap_or(u32::MAX),
        ));
    }
    if length == 0 {
        return Err(Error::FriDomainSize {
            length,
            arity: configured_arity,
        });
    }
    let round_arity = configured_arity.min(length);
    if !length.is_multiple_of(round_arity) {
        return Err(Error::FriDomainSize {
            length,
            arity: round_arity,
        });
    }
    Ok(round_arity)
}

pub fn fold_fri_coset(
    values: &[GoldilocksFp4V1],
    challenge: GoldilocksFp4V1,
    x: u64,
    coset_generator: u64,
) -> Result<GoldilocksFp4V1> {
    if values.len() != 2 || x == 0 || coset_generator != GOLDILOCKS_MODULUS - 1 {
        return Err(Error::FriDomainSize {
            length: values.len(),
            arity: 2,
        });
    }
    let inverse_two = field_inverse(2);
    let even = values[0].add(values[1]).mul_base(inverse_two);
    let odd = values[0]
        .sub(values[1])
        .mul_base(mul_mod(inverse_two, field_inverse(x)));
    Ok(even.add(challenge.mul(odd)))
}

fn fold_round(
    values: &[GoldilocksFp4V1],
    configured_arity: usize,
    challenge: GoldilocksFp4V1,
    domain: FriDomain,
) -> Result<Vec<GoldilocksFp4V1>> {
    let round_arity = fri_round_arity(values.len(), configured_arity)?;
    let output_len = values.len() / round_arity;
    let coset_generator = domain.coset_generator(output_len);
    let mut group = Vec::with_capacity(round_arity);
    let mut next = Vec::with_capacity(output_len);
    for leaf_index in 0..output_len {
        group.clear();
        for position in 0..round_arity {
            group.push(values[leaf_index + position * output_len]);
        }
        next.push(fold_fri_coset(
            &group,
            challenge,
            domain.point(leaf_index),
            coset_generator,
        )?);
    }
    Ok(next)
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
        let digest = transcript.challenge_digest(&tag);
        let rejection_limit = GOLDILOCKS_MODULUS - (GOLDILOCKS_MODULUS % domain);
        for candidate in digest.words() {
            if indices.len() == desired {
                break;
            }
            if candidate >= rejection_limit {
                continue;
            }
            let remainder = candidate % domain;
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

struct BatchPreparationContext<'a> {
    params: &'a StarkParameterSet,
    batch: &'a TransitionBatch,
    public_io: &'a PublicIO,
    protocol_version: u16,
    params_version: u16,
    transcript_trace_root: Option<GoldilocksDigest384V1>,
    execution_mode: ExecutionMode,
    poseidon_policy: PoseidonPipelinePolicy,
}

#[derive(Debug)]
struct PreparedBatch {
    trace_rows: u32,
    trace_root: GoldilocksDigest384V1,
    air_trace_root: GoldilocksDigest384V1,
    air_composition_root: GoldilocksDigest384V1,
    lde_root: GoldilocksDigest384V1,
    lde_domain_size: u32,
    lookup_grand_product: u64,
    gamma: u64,
    alphas: Vec<u64>,
    lde_columns: Vec<Vec<u64>>,
    lde_values: Vec<u64>,
    lde_hashes: Vec<GoldilocksDigest384V1>,
    air_trace_leaves: Vec<GoldilocksDigest384V1>,
    air_composition_values: Vec<u64>,
    air_composition_leaves: Vec<GoldilocksDigest384V1>,
    transcript: Transcript,
}

/// Batch-derived commitments which a proof is required to advertise exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchDerivedCommitments {
    /// Canonical base-trace commitment.
    pub trace_root: GoldilocksDigest384V1,
    /// Canonical commitment to the LDE trace rows used by AIR openings.
    pub air_trace_root: GoldilocksDigest384V1,
    /// Canonical commitment to the AIR composition evaluations.
    pub air_composition_root: GoldilocksDigest384V1,
    /// Canonical LDE/lookup commitment.
    pub lookup_root: GoldilocksDigest384V1,
    /// Canonical LDE evaluation-domain size.
    pub lde_domain_size: u32,
    /// Canonical lookup grand-product accumulator.
    pub lookup_grand_product: u64,
}

fn hash_trace_columns_v1(
    column_names: &[String],
    coefficients: &[Vec<u64>],
) -> Result<Vec<GoldilocksDigest384V1>> {
    if column_names.len() != coefficients.len() {
        return Err(Error::InvalidTraceShape {
            details: "column-name and coefficient counts differ".to_owned(),
        });
    }
    column_names
        .iter()
        .zip(coefficients)
        .enumerate()
        .map(|(index, (name, values))| {
            let mut bytes = Vec::with_capacity(values.len().saturating_mul(8));
            for value in values {
                if *value >= GOLDILOCKS_MODULUS {
                    return Err(Error::NonCanonicalGoldilocksElement {
                        context: "trace_column_coefficients",
                        indices: vec![index],
                    });
                }
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            hash_bytes_v1(
                TRACE_COMMITMENT_ROLE_V1,
                MERKLE_LEAF_PHASE_V1,
                0,
                index,
                0,
                &[name.as_bytes(), &bytes],
            )
        })
        .collect()
}

fn combine_lde_columns_v1(columns: &[Vec<u64>], coefficients: &[u64]) -> Result<Vec<u64>> {
    if columns.len() != coefficients.len() || columns.is_empty() {
        return Err(Error::InvalidTraceShape {
            details: "LDE columns and post-commitment coefficients differ".to_owned(),
        });
    }
    let row_count = columns[0].len();
    if !columns.iter().all(|column| column.len() == row_count) {
        return Err(Error::InvalidTraceShape {
            details: "LDE columns have inconsistent row counts".to_owned(),
        });
    }
    Ok((0..row_count)
        .map(|row| {
            columns
                .iter()
                .zip(coefficients)
                .fold(0_u64, |accumulator, (column, coefficient)| {
                    add_mod(accumulator, mul_mod(column[row], *coefficient))
                })
        })
        .collect())
}

#[allow(clippy::too_many_lines)]
fn prepare_batch(context: &BatchPreparationContext<'_>) -> Result<PreparedBatch> {
    let &BatchPreparationContext {
        params,
        batch,
        public_io,
        protocol_version,
        params_version,
        transcript_trace_root,
        execution_mode,
        poseidon_policy,
    } = context;
    if params.name != batch.parameter {
        return Err(Error::ParameterMismatch {
            expected: params.name.to_string(),
            actual: batch.parameter.clone(),
        });
    }
    crate::digest::ensure_trace_capacity(params, batch.transitions.len())?;
    crate::trace::ensure_trace_schema_limit(batch, crate::trace::DEFAULT_MAX_TRACE_COLUMNS)?;
    let trace = build_trace(batch)?;
    ensure_base_trace_constraints(&trace)?;
    let column_names = trace
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let planner = Planner::new(params);
    let poseidon_mode = poseidon_policy.resolved();
    let polynomial_data = derive_polynomial_data(&trace, &planner, execution_mode);
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
    let trace_column_leaves = hash_trace_columns_v1(&column_names, &polynomial_data.coefficients)?;
    let derived_trace_root =
        merkle_root_with_mode(&trace_column_leaves, MerkleTreeRoleV1::Trace, poseidon_mode)?;
    let trace_root = transcript_trace_root.unwrap_or(derived_trace_root);
    let lde_columns = polynomial_data.into_lde_columns();
    let mut transcript = Transcript::initialise(
        public_io,
        params.name,
        protocol_version,
        params_version,
        TRANSCRIPT_TAG_INIT,
    )?;
    transcript.append_message(TRANSCRIPT_TAG_TRACE_ROOT, &trace_root.to_le_bytes());
    let column_mix = (0..lde_columns.len())
        .map(|index| {
            transcript.challenge_field(&format!("{TRANSCRIPT_TAG_COLUMN_MIX_PREFIX}:{index}"))
        })
        .collect::<Vec<_>>();
    let lde_values = combine_lde_columns_v1(&lde_columns, &column_mix)?;
    let lde_domain_size =
        u32::try_from(lde_values.len()).map_err(|_| Error::TraceLengthOverflow {
            rows: lde_values.len(),
        })?;
    let lde_hashes = hash_lde_leaves_with_mode(&lde_values, params.fri.arity, poseidon_mode)?;
    let lde_root = merkle_root_with_mode(&lde_hashes, MerkleTreeRoleV1::Lookup, poseidon_mode)?;
    let air_trace_leaves = hash_air_trace_rows_with_mode(&lde_columns, poseidon_mode)?;
    let air_trace_root =
        merkle_root_with_mode(&air_trace_leaves, MerkleTreeRoleV1::AirTrace, poseidon_mode)?;
    transcript.append_message(
        TRANSCRIPT_TAG_ROOTS,
        &[lde_root.to_le_bytes(), trace_root.to_le_bytes()].concat(),
    );
    let gamma = transcript.challenge_field(TRANSCRIPT_TAG_GAMMA);
    let selector_index =
        column_index(&trace, "s_perm").ok_or_else(|| Error::MissingColumn("s_perm".to_string()))?;
    let witness_index = column_index(&trace, "perm_hash")
        .ok_or_else(|| Error::MissingColumn("perm_hash".to_string()))?;
    let lookup_grand_product = compute_lookup_grand_product(
        &lde_columns[selector_index],
        &lde_columns[witness_index],
        gamma,
    )?;
    let mut alphas = Vec::with_capacity(AIR_COMPOSITION_ALPHA_COUNT);
    for idx in 0..AIR_COMPOSITION_ALPHA_COUNT {
        let tag = format!("{TRANSCRIPT_TAG_ALPHA_PREFIX}:{idx}");
        alphas.push(transcript.challenge_field(&tag));
    }
    let next_step = usize::try_from(params.fri.blowup_factor)
        .expect("FRI blowup factor fits usize")
        .max(1);
    let air_composition_values =
        air_composition_values(&column_names, &lde_columns, &alphas, next_step)?;
    let air_composition_leaves =
        hash_air_composition_leaves_with_mode(&air_composition_values, poseidon_mode)?;
    let air_composition_root = merkle_root_with_mode(
        &air_composition_leaves,
        MerkleTreeRoleV1::AirComposition,
        poseidon_mode,
    )?;
    transcript.append_message(
        TRANSCRIPT_TAG_AIR_ROOTS,
        &[
            air_trace_root.to_le_bytes(),
            air_composition_root.to_le_bytes(),
        ]
        .concat(),
    );
    transcript.append_message(LOOKUP_PRODUCT_DOMAIN, &lookup_grand_product.to_le_bytes());
    let trace_rows =
        u32::try_from(trace.rows).map_err(|_| Error::TraceLengthOverflow { rows: trace.rows })?;
    Ok(PreparedBatch {
        trace_rows,
        trace_root,
        air_trace_root,
        air_composition_root,
        lde_root,
        lde_domain_size,
        lookup_grand_product,
        gamma,
        alphas,
        lde_columns,
        lde_values,
        lde_hashes,
        air_trace_leaves,
        air_composition_values,
        air_composition_leaves,
        transcript,
    })
}

/// Recompute every proof-carried root and accumulator that is deterministic from the batch.
pub fn derive_batch_commitments(
    params: &StarkParameterSet,
    batch: &TransitionBatch,
    public_io: &PublicIO,
    protocol_version: u16,
    params_version: u16,
) -> Result<BatchDerivedCommitments> {
    let prepared = prepare_batch(&BatchPreparationContext {
        params,
        batch,
        public_io,
        protocol_version,
        params_version,
        transcript_trace_root: None,
        execution_mode: ExecutionMode::Cpu,
        poseidon_policy: PoseidonPipelinePolicy::for_mode(ExecutionMode::Cpu),
    })?;
    Ok(BatchDerivedCommitments {
        trace_root: prepared.trace_root,
        air_trace_root: prepared.air_trace_root,
        air_composition_root: prepared.air_composition_root,
        lookup_root: prepared.lde_root,
        lde_domain_size: prepared.lde_domain_size,
        lookup_grand_product: prepared.lookup_grand_product,
    })
}

impl StarkBackend {
    #[allow(clippy::too_many_lines)]
    fn prove_inner(
        &self,
        batch: &TransitionBatch,
        public_io: &PublicIO,
        protocol_version: u16,
        params_version: u16,
        transcript_trace_root: Option<GoldilocksDigest384V1>,
    ) -> Result<BackendArtifact> {
        let execution_mode = self.config.execution_mode().resolve();
        let poseidon_policy =
            PoseidonPipelinePolicy::new(self.config.poseidon_mode(), execution_mode);
        let poseidon_mode = poseidon_policy.resolved();
        let PreparedBatch {
            trace_rows,
            trace_root,
            air_trace_root,
            air_composition_root,
            lde_root,
            lde_domain_size,
            lookup_grand_product,
            gamma,
            alphas,
            lde_columns,
            lde_values,
            lde_hashes,
            air_trace_leaves,
            air_composition_values,
            air_composition_leaves,
            mut transcript,
        } = prepare_batch(&BatchPreparationContext {
            params: &self.config.params,
            batch,
            public_io,
            protocol_version,
            params_version,
            transcript_trace_root,
            execution_mode,
            poseidon_policy,
        })?;
        let next_step = usize::try_from(self.config.params.fri.blowup_factor)
            .expect("FRI blowup factor fits usize")
            .max(1);
        let FriOpeningLayers {
            layer_values: fri_layer_values,
            roots: fri_layers,
            betas: fri_betas,
        } = fold_with_fri_opening_layers(
            &air_composition_values,
            &self.config.params,
            &mut transcript,
            poseidon_mode,
        )?;
        let query_indices = sample_queries(
            lde_values.len(),
            usize::try_from(self.config.params.fri.queries).expect("query count fits usize"),
            &mut transcript,
        );
        let query_openings = open_queries(&lde_values, &query_indices)?;
        let query_chunks =
            open_query_chunks(&lde_values, &query_indices, self.config.params.fri.arity)?;
        let query_paths = merkle_paths_for_queries_with_mode(
            &lde_hashes,
            &query_indices,
            self.config.params.fri.arity,
            lde_values.len(),
            MerkleTreeRoleV1::Lookup,
            poseidon_mode,
        )?;
        let air_openings = open_air_constraint_openings_with_mode(
            &lde_columns,
            &air_trace_leaves,
            &air_composition_values,
            &air_composition_leaves,
            &query_indices,
            next_step,
            poseidon_mode,
        )?;
        let fri_query_openings = open_fri_query_chains(
            &fri_layer_values,
            &query_indices,
            self.config.params.fri.arity,
            poseidon_mode,
        )?;
        Ok(BackendArtifact {
            parameter: self.config.params.name.to_string(),
            trace_rows,
            trace_root,
            air_trace_root,
            air_composition_root,
            lookup_root: lde_root,
            lde_domain_size,
            lookup_grand_product,
            lookup_challenge: gamma,
            alphas,
            fri_arity: self.config.params.fri.arity,
            fri_blowup: self.config.params.fri.blowup_factor,
            fri_layers,
            fri_betas,
            query_openings,
            query_chunks,
            query_paths,
            air_openings,
            fri_query_openings,
        })
    }

    #[cfg(test)]
    pub fn prove_with_transcript_trace_root(
        &self,
        batch: &TransitionBatch,
        public_io: &PublicIO,
        protocol_version: u16,
        params_version: u16,
        trace_root: GoldilocksDigest384V1,
    ) -> Result<BackendArtifact> {
        self.prove_inner(
            batch,
            public_io,
            protocol_version,
            params_version,
            Some(trace_root),
        )
    }
}

impl Backend for StarkBackend {
    fn prove(
        &self,
        batch: &TransitionBatch,
        public_io: &PublicIO,
        protocol_version: u16,
        params_version: u16,
    ) -> Result<BackendArtifact> {
        self.prove_inner(batch, public_io, protocol_version, params_version, None)
    }
}
#[derive(Debug, Clone)]
pub struct Transcript {
    state: GoldilocksDigest384V1,
    counter: u64,
}
impl Transcript {
    pub fn initialise(
        public_io: &PublicIO,
        parameter: &str,
        protocol_version: u16,
        params_version: u16,
        tag: &str,
    ) -> Result<Self> {
        let payload = norito::core::to_bytes(&(
            protocol_version,
            params_version,
            parameter,
            public_io.clone(),
        ))?;
        let state = hash_bytes_v1(
            TRANSCRIPT_ROLE_V1,
            b"initialise",
            0,
            0,
            0,
            &[tag.as_bytes(), &payload],
        )?;
        Ok(Self { state, counter: 1 })
    }
    pub fn append_message(&mut self, tag: &str, message: &[u8]) {
        self.state = hash_bytes_v1(
            TRANSCRIPT_ROLE_V1,
            b"append-message",
            0,
            0,
            self.counter,
            &[&self.state.to_le_bytes(), tag.as_bytes(), message],
        )
        .expect("bounded FASTPQ transcript message framing");
        self.counter = self
            .counter
            .checked_add(1)
            .expect("FASTPQ transcript counter overflow");
    }
    pub fn append_fri_layer(&mut self, round: usize, root: GoldilocksDigest384V1) {
        let tag = format!("{TRANSCRIPT_TAG_FRI_LAYER_PREFIX}:{round}");
        self.append_message(&tag, &root.to_le_bytes());
    }
    pub fn append_fri_final(&mut self, root: GoldilocksDigest384V1) {
        self.append_message(FRI_FINAL_DOMAIN, &root.to_le_bytes());
    }
    pub fn challenge_beta(&mut self, round: usize) -> GoldilocksFp4V1 {
        let tag = format!("{TRANSCRIPT_TAG_BETA_PREFIX}:{round}");
        GoldilocksFp4V1::from_digest(self.challenge_digest(&tag))
    }
    pub fn challenge_bytes(&mut self, tag: &str) -> [u8; 48] {
        self.challenge_digest(tag).to_le_bytes()
    }
    pub fn challenge_field(&mut self, tag: &str) -> u64 {
        self.challenge_digest(tag).words()[0]
    }

    fn challenge_digest(&mut self, tag: &str) -> GoldilocksDigest384V1 {
        let digest = hash_bytes_v1(
            TRANSCRIPT_ROLE_V1,
            b"challenge",
            0,
            0,
            self.counter,
            &[&self.state.to_le_bytes(), tag.as_bytes()],
        )
        .expect("bounded FASTPQ transcript challenge framing");
        self.state = digest;
        self.counter = self
            .counter
            .checked_add(1)
            .expect("FASTPQ transcript counter overflow");
        digest
    }
}
fn add_mod(a: u64, b: u64) -> u64 {
    let sum = u128::from(a) + u128::from(b);
    u64::try_from(sum % u128::from(GOLDILOCKS_MODULUS)).expect("modulus reduction fits in u64")
}
fn sub_mod(a: u64, b: u64) -> u64 {
    let reduced = (u128::from(a) + u128::from(GOLDILOCKS_MODULUS) - u128::from(b))
        % u128::from(GOLDILOCKS_MODULUS);
    u64::try_from(reduced).expect("modulus reduction fits in u64")
}
fn mul_mod(a: u64, b: u64) -> u64 {
    let product = u128::from(a) * u128::from(b);
    let reduced = product % u128::from(GOLDILOCKS_MODULUS);
    u64::try_from(reduced).expect("modulus reduction fits in u64")
}
fn field_pow(mut base: u64, mut exponent: u64) -> u64 {
    let mut result = FIELD_ONE;
    while exponent > 0 {
        if exponent & 1 == 1 {
            result = mul_mod(result, base);
        }
        base = mul_mod(base, base);
        exponent >>= 1;
    }
    result
}
fn field_inverse(value: u64) -> u64 {
    debug_assert_ne!(value, 0, "zero has no Goldilocks multiplicative inverse");
    field_pow(value, GOLDILOCKS_MODULUS - 2)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OperationKind, PublicInputs, StateTransition};
    use std::collections::BTreeSet;

    fn fp4(value: u64) -> GoldilocksFp4V1 {
        GoldilocksFp4V1::from_base(value).expect("canonical Goldilocks test value")
    }

    fn fp4_values(values: &[u64]) -> Vec<GoldilocksFp4V1> {
        values.iter().copied().map(fp4).collect()
    }

    fn lookup_merkle_root(leaves: &[GoldilocksDigest384V1]) -> GoldilocksDigest384V1 {
        merkle_root_with_mode(leaves, MerkleTreeRoleV1::Lookup, ExecutionMode::Cpu)
            .expect("typed lookup Merkle root")
    }
    fn sample_batch(rows: usize) -> TransitionBatch {
        let mut batch =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
        for idx in 0..rows {
            let key = format!("asset/xor/account-{idx:04}").into_bytes();
            let idx_u64 = u64::try_from(idx).expect("sample batch index fits u64");
            let pre = idx_u64.to_le_bytes().to_vec();
            let op = if idx % 2 == 0 {
                OperationKind::Mint
            } else {
                OperationKind::Burn
            };
            let post_value = if matches!(&op, OperationKind::Burn) {
                idx_u64 - 1
            } else {
                idx_u64.wrapping_add(1)
            };
            let post = post_value.to_le_bytes().to_vec();
            batch.push(StateTransition::new(key, pre, post, op));
        }
        batch.sort();
        batch
    }
    #[test]
    fn transcript_challenges_are_deterministic() {
        let mut transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            "fastpq-state-transition-stark-v1",
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
        let leaves = hash_lde_leaves(&evaluations, 2).expect("hash leaves");
        let err = merkle_paths_for_queries(&leaves, &[4], 2, evaluations.len())
            .expect_err("out of range");
        assert!(matches!(
            err,
            Error::QueryIndexOutOfRange { index: 4, len: 4 }
        ));
    }
    #[test]
    fn merkle_paths_verify_against_lookup_root_for_single_leaf() {
        let evaluations = vec![42u64];
        let leaves = hash_lde_leaves(&evaluations, 2).expect("hash leaves");
        let root = lookup_merkle_root(&leaves);
        let paths =
            merkle_paths_for_queries(&leaves, &[0], 2, evaluations.len()).expect("query path");
        let chunks = open_query_chunks(&evaluations, &[0], 2).expect("query chunk");
        let leaf = hash_lde_chunk(0, &chunks[0]).expect("leaf hash");
        assert!(verify_merkle_path(root, leaf, 0, &paths[0]).expect("path verifies"));
        assert!(!verify_merkle_path(leaf, leaf, 0, &[]).expect("empty path is noncanonical"));
    }
    #[test]
    fn merkle_paths_verify_against_lookup_root_for_odd_leaf_count() {
        let chunk_size = lde_chunk_size(2).expect("binary FRI chunk size");
        let evaluations = (0..(chunk_size * 3 - 1))
            .map(|idx| u64::try_from(idx).expect("index fits u64"))
            .collect::<Vec<_>>();
        let query_index = evaluations.len() - 1;
        let leaf_index = query_index / chunk_size;
        let leaves = hash_lde_leaves(&evaluations, 2).expect("hash leaves");
        let root = lookup_merkle_root(&leaves);
        let paths =
            merkle_paths_for_queries(&leaves, &[query_index], 2, evaluations.len()).expect("path");
        let chunks = open_query_chunks(&evaluations, &[query_index], 2).expect("chunk");
        let leaf = hash_lde_chunk(leaf_index, &chunks[0]).expect("leaf hash");
        assert!(verify_merkle_path(root, leaf, leaf_index, &paths[0]).expect("path verifies"));
    }
    #[test]
    fn merkle_path_rejects_indices_with_bits_above_the_tree_depth() {
        let evaluations = (0u64..256).collect::<Vec<_>>();
        let leaves = hash_lde_leaves(&evaluations, 2).expect("hash leaves");
        let root = lookup_merkle_root(&leaves);
        let paths =
            merkle_paths_for_queries(&leaves, &[0], 2, evaluations.len()).expect("query path");
        let chunks = open_query_chunks(&evaluations, &[0], 2).expect("query chunk");
        let leaf = hash_lde_chunk(0, &chunks[0]).expect("leaf hash");
        let aliased_index = 1usize
            .checked_shl(u32::try_from(paths[0].len()).expect("path depth fits u32"))
            .expect("test path depth fits usize");

        assert!(verify_merkle_path(root, leaf, 0, &paths[0]).expect("path verifies"));
        assert!(
            !verify_merkle_path(root, leaf, aliased_index, &paths[0])
                .expect("high-bit alias is rejected")
        );
    }

    #[test]
    fn lde_helpers_reject_non_binary_fri_arity() {
        let evaluations = [1u64, 2, 3, 4];
        let err = hash_lde_leaves(&evaluations, 8).expect_err("legacy arity must fail");
        assert!(matches!(err, Error::FriArity(8)));
        let err = open_query_chunks(&evaluations, &[0], 8).expect_err("legacy arity must fail");
        assert!(matches!(err, Error::FriArity(8)));
        let err = merkle_paths_for_queries(&[], &[], 8, evaluations.len())
            .expect_err("legacy arity must fail before empty-query handling");
        assert!(matches!(err, Error::FriArity(8)));
    }
    #[test]
    fn trace_row_hashes_match_cpu_reference_for_edge_shapes() {
        let cases = [
            vec![Vec::<u64>::new(), Vec::<u64>::new()],
            vec![vec![1u64], vec![2]],
            vec![vec![1u64, u64::MAX], vec![3, 4], vec![5, 6]],
            vec![(0u64..17).collect(), (20u64..37).collect()],
            (0u64..19).map(|offset| vec![offset, offset + 1]).collect(),
        ];
        for columns in cases {
            assert_eq!(
                hash_trace_rows_with_mode(&columns, ExecutionMode::Cpu),
                hash_trace_rows_cpu(&columns)
            );
            assert_eq!(
                hash_trace_rows_with_mode(&columns, ExecutionMode::Gpu),
                hash_trace_rows_cpu(&columns)
            );
        }
    }
    #[test]
    fn air_constrains_every_metadata_commitment_limb_to_be_stable() {
        let mut batch = sample_batch(2);
        batch
            .metadata
            .insert("axt_metadata_binding".to_owned(), vec![0x5a; 32]);
        let trace = build_trace(&batch).expect("trace");
        let column_names = trace
            .columns
            .iter()
            .map(|column| column.name.clone())
            .collect::<Vec<_>>();
        let current = trace
            .columns
            .iter()
            .map(|column| column.values[0])
            .collect::<Vec<_>>();
        let alphas = (1..=AIR_COMPOSITION_ALPHA_COUNT)
            .map(|alpha| u64::try_from(alpha).expect("AIR challenge index fits u64"))
            .collect::<Vec<_>>();
        assert_eq!(
            air_composition_value_for_rows(&column_names, &current, &current, &alphas)
                .expect("valid row composition"),
            0
        );
        for limb in 0..crate::trace::METADATA_COMMITMENT_LIMBS {
            let name = format!("metadata_hash_limb_{limb}");
            let index = column_names
                .iter()
                .position(|column| column == &name)
                .expect("metadata commitment limb column");
            let mut next = current.clone();
            next[index] = next[index].wrapping_add(1);
            assert_ne!(
                air_composition_value_for_rows(&column_names, &current, &next, &alphas)
                    .expect("mutated row composition"),
                0,
                "AIR must reject instability in {name}"
            );
        }
    }
    #[test]
    fn base_trace_constraint_check_rejects_non_boolean_selector() {
        let mut trace = build_trace(&sample_batch(2)).expect("trace");
        let selector = trace
            .columns
            .iter_mut()
            .find(|column| column.name == "s_active")
            .expect("active selector column");
        selector.values[0] = 2;
        let err = ensure_base_trace_constraints(&trace).unwrap_err();
        assert!(matches!(err, Error::AirConstraintMismatch { index: 0 }));
    }
    #[test]
    fn base_trace_delta_constraint_reconstructs_every_packed_value_limb() {
        let mut batch =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
        let before = (1u64 << 56) - 2;
        let after = (1u64 << 56) + 3;
        batch.push(StateTransition::new(
            b"asset/xor/account-multi-limb".to_vec(),
            before.to_le_bytes().to_vec(),
            after.to_le_bytes().to_vec(),
            OperationKind::Mint,
        ));
        let trace = build_trace(&batch).expect("trace");
        ensure_base_trace_constraints(&trace).expect("multi-limb delta must satisfy the AIR");
    }
    #[test]
    fn air_independent_challenges_prevent_same_parity_residue_cancellation() {
        let trace = build_trace(&sample_batch(2)).expect("trace");
        let column_names = trace
            .columns
            .iter()
            .map(|column| column.name.clone())
            .collect::<Vec<_>>();
        let current = trace
            .columns
            .iter()
            .map(|column| column.values[0])
            .collect::<Vec<_>>();
        let first = column_names
            .iter()
            .position(|column| column == "metadata_hash_limb_0")
            .expect("first metadata commitment limb");
        let third = column_names
            .iter()
            .position(|column| column == "metadata_hash_limb_2")
            .expect("third metadata commitment limb");
        let mut next = current.clone();
        next[first] = sub_mod(current[first], FIELD_ONE);
        next[third] = add_mod(current[third], FIELD_ONE);

        let legacy_same_parity_sum = add_mod(
            mul_mod(3, FIELD_ONE),
            mul_mod(3, GOLDILOCKS_MODULUS - FIELD_ONE),
        );
        assert_eq!(legacy_same_parity_sum, 0);
        let alphas = (1..=AIR_COMPOSITION_ALPHA_COUNT)
            .map(|alpha| u64::try_from(alpha).expect("AIR challenge index fits u64"))
            .collect::<Vec<_>>();
        assert_ne!(
            air_composition_value_for_rows(&column_names, &current, &next, &alphas)
                .expect("AIR composition"),
            0,
            "distinct coefficients must expose equal-and-opposite residues at old reuse offsets"
        );
    }
    #[test]
    fn lde_row_hashes_match_cpu_reference_for_trace_shape() {
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let planner = crate::Planner::new(&params);
        let trace = build_trace(&sample_batch(8)).expect("trace");
        let mut data = crate::trace::derive_polynomial_data(&trace, &planner, ExecutionMode::Cpu);
        let lde_columns = data.lde_columns();
        assert_eq!(
            hash_trace_rows_with_mode(lde_columns, ExecutionMode::Gpu),
            hash_trace_rows_cpu(lde_columns)
        );
    }
    #[test]
    fn native_stark_lde_digests_are_byte_identical_across_execution_modes() {
        let evaluations = (0_u64..257).collect::<Vec<_>>();
        let scalar = hash_lde_leaves_with_mode(&evaluations, 2, ExecutionMode::Cpu)
            .expect("scalar native-STARK LDE digests");
        let accelerated = hash_lde_leaves_with_mode(&evaluations, 2, ExecutionMode::Gpu)
            .expect("accelerated native-STARK LDE digests");
        assert_eq!(accelerated, scalar);
    }
    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn native_stark_fri_digests_are_byte_identical_for_mixed_layer_shapes() {
        for length in [2_usize, 4, 16, 256] {
            let values = (0..length)
                .map(|value| {
                    GoldilocksFp4V1::new([
                        u64::try_from(value).expect("test value fits u64"),
                        1,
                        2,
                        3,
                    ])
                    .expect("canonical Fp4 test value")
                })
                .collect::<Vec<_>>();
            let scalar = hash_fri_leaves_with_mode(7, &values, 2, ExecutionMode::Cpu)
                .expect("scalar native-STARK FRI digests");
            let accelerated = hash_fri_leaves_with_mode(7, &values, 2, ExecutionMode::Gpu)
                .expect("accelerated native-STARK FRI digests");
            assert_eq!(accelerated, scalar, "FRI layer length {length}");
        }
    }
    #[cfg(feature = "fastpq-gpu")]
    #[test]
    fn native_stark_merkle_roots_are_byte_identical_across_execution_modes() {
        let leaves =
            hash_lde_leaves_with_mode(&(0_u64..513).collect::<Vec<_>>(), 2, ExecutionMode::Cpu)
                .expect("native-STARK LDE leaves");
        let scalar = merkle_root_with_mode(&leaves, MerkleTreeRoleV1::Lookup, ExecutionMode::Cpu)
            .expect("scalar native-STARK Merkle root");
        let accelerated =
            merkle_root_with_mode(&leaves, MerkleTreeRoleV1::Lookup, ExecutionMode::Gpu)
                .expect("accelerated native-STARK Merkle root");
        assert_eq!(accelerated, scalar);
    }
    #[test]
    fn fold_with_fri_emits_layers_and_betas() {
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let mut transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            "fastpq-state-transition-stark-v1",
            1,
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .expect("transcript");
        let evaluations = (1u64..=16).collect::<Vec<_>>();
        let (layers, betas) = fold_with_fri(
            &evaluations,
            params.fri.arity,
            params.fri.max_reductions,
            params.lde_root,
            params.lde_log_size,
            params.omega_coset,
            &mut transcript,
        )
        .expect("fri folding");
        assert!(!betas.is_empty());
        assert_eq!(layers.len(), betas.len() + 1);
        assert!(
            betas.len()
                <= usize::try_from(params.fri.max_reductions).expect("max reductions fits usize")
        );
        assert!(
            layers
                .iter()
                .all(|layer| *layer != GoldilocksDigest384V1::default())
        );
    }
    #[test]
    fn fold_with_fri_rejects_invalid_arity() {
        let mut transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            "fastpq-state-transition-stark-v1",
            1,
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .expect("transcript");
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let err = super::fold_with_fri(
            &[1, 2, 3],
            4,
            1,
            params.lde_root,
            params.lde_log_size,
            params.omega_coset,
            &mut transcript,
        )
        .expect_err("invalid arity");
        assert!(matches!(err, super::Error::FriArity(4)));
    }
    #[test]
    fn sampled_queries_are_sorted_and_unique() {
        let mut transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            "fastpq-state-transition-stark-v1",
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
            "fastpq-state-transition-stark-v1",
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
        )
        .expect("matching lookup column lengths");
        assert_ne!(product, 0);
    }
    #[test]
    fn lookup_grand_product_rejects_mismatched_column_lengths() {
        let err = compute_lookup_grand_product(&[1, 0], &[7], 3).unwrap_err();
        assert!(matches!(
            err,
            Error::LookupColumnLengthMismatch {
                selector_len: 2,
                witness_len: 1
            }
        ));
    }
    #[test]
    fn low_level_backend_rejects_wide_trace_schema_before_allocation() {
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let mut batch = TransitionBatch::new(params.name, PublicInputs::default());
        batch.push(StateTransition::new(
            b"wide-value".to_vec(),
            vec![0xA5; (crate::trace::DEFAULT_MAX_TRACE_COLUMNS + 1) * crate::LIMB_BYTES],
            Vec::new(),
            OperationKind::MetaSet,
        ));
        let actual = crate::trace::column_count_for_batch(&batch).expect("schema count");
        assert!(actual > crate::trace::DEFAULT_MAX_TRACE_COLUMNS);

        let backend = StarkBackend::new(BackendConfig::new(params));
        let err = backend
            .prove(&batch, &crate::proof::PublicIO::default(), 1, 1)
            .expect_err("wide trace schema must fail before materialisation");
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_air_row_values",
                actual: observed,
                max: crate::trace::DEFAULT_MAX_TRACE_COLUMNS,
            } if observed == actual
        ));
    }
    #[test]
    fn fri_round_arity_uses_a_real_final_subgroup_and_rejects_padding() {
        assert_eq!(super::fri_round_arity(16, 2).unwrap(), 2);
        assert_eq!(super::fri_round_arity(1, 2).unwrap(), 1);
        let err = super::fri_round_arity(3, 2).unwrap_err();
        assert!(matches!(
            err,
            super::Error::FriDomainSize {
                length: 3,
                arity: 2
            }
        ));
    }
    #[test]
    fn fold_round_matches_direct_constant_and_linear_evaluation() {
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let domain = super::FriDomain::from_lde_parameters(
            params.lde_root,
            params.lde_log_size,
            16,
            params.omega_coset,
        )
        .expect("FRI domain");
        let challenge = fp4(0x1234_5678_9abc_def0 % super::GOLDILOCKS_MODULUS);
        let constant = 37;
        let constant_values = vec![fp4(constant); 16];
        assert_eq!(
            super::fold_round(&constant_values, 2, challenge, domain).unwrap(),
            vec![fp4(constant); 8]
        );

        let intercept = 11;
        let slope = 29;
        let linear_values = (0..16)
            .map(|index| {
                reference_add(
                    intercept,
                    reference_mul(slope, reference_domain_point(params, 16, index)),
                )
            })
            .map(fp4)
            .collect::<Vec<_>>();
        let expected = fp4(intercept).add(challenge.mul(fp4(slope)));
        assert_eq!(
            super::fold_round(&linear_values, 2, challenge, domain).unwrap(),
            vec![expected; 8]
        );
    }
    #[test]
    fn terminal_fri_degree_check_interpolates_the_folded_coset() {
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let domain = super::FriDomain::from_lde_parameters(
            params.lde_root,
            params.lde_log_size,
            8,
            params.omega_coset,
        )
        .expect("terminal FRI domain");
        let linear = (0..8)
            .map(|index| {
                let x = domain.point(index);
                reference_add(3, reference_mul(5, x))
            })
            .map(fp4)
            .collect::<Vec<_>>();
        assert!(domain.evaluations_have_degree_below(&linear, 2).unwrap());
        assert!(!domain.evaluations_have_degree_below(&linear, 1).unwrap());

        let quadratic = (0..8)
            .map(|index| {
                let x = domain.point(index);
                reference_add(
                    reference_add(3, reference_mul(5, x)),
                    reference_mul(7, reference_mul(x, x)),
                )
            })
            .map(fp4)
            .collect::<Vec<_>>();
        assert!(domain.evaluations_have_degree_below(&quadratic, 3).unwrap());
        assert!(!domain.evaluations_have_degree_below(&quadratic, 2).unwrap());
    }
    #[test]
    fn fri_rejects_a_reduction_limit_that_cannot_expose_the_terminal_layer() {
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let mut transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            params.name,
            1,
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .expect("transcript");
        let error = super::fold_with_fri(
            &(0u64..64).collect::<Vec<_>>(),
            params.fri.arity,
            0,
            params.lde_root,
            params.lde_log_size,
            params.omega_coset,
            &mut transcript,
        )
        .expect_err("zero reductions cannot expose an arity-sized terminal layer");
        assert!(matches!(error, super::Error::FriReductionLimit { .. }));
    }
    #[test]
    fn fri_merkle_leaves_commit_strided_cosets() {
        let values = fp4_values(&(0u64..16).collect::<Vec<_>>());
        let leaves = super::hash_fri_leaves_with_mode(0, &values, 2, ExecutionMode::Cpu)
            .expect("FRI leaves");
        let first_coset = (0..2)
            .map(|position| values[position * 8])
            .collect::<Vec<_>>();
        let second_coset = (0..2)
            .map(|position| values[1 + position * 8])
            .collect::<Vec<_>>();
        assert_eq!(
            leaves[0],
            super::hash_fri_chunk(0, 0, &first_coset).unwrap()
        );
        assert_eq!(
            leaves[1],
            super::hash_fri_chunk(0, 1, &second_coset).unwrap()
        );
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
        let (layers, betas) = super::fold_with_fri(
            &evaluations,
            2,
            params.fri.max_reductions,
            params.lde_root,
            params.lde_log_size,
            params.omega_coset,
            &mut transcript,
        )
        .expect("fri folding");
        assert_eq!(layers.len(), betas.len() + 1);
        assert_eq!(betas.len(), 3);
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
            2,
            params.fri.max_reductions,
            params.lde_root,
            params.lde_log_size,
            params.omega_coset,
            &mut transcript_again,
        )
        .expect("fri folding repeat");
        assert_eq!(layers, repeat_layers);
        assert_eq!(betas, repeat_betas);
    }
    fn reference_add(a: u64, b: u64) -> u64 {
        u64::try_from((u128::from(a) + u128::from(b)) % u128::from(super::GOLDILOCKS_MODULUS))
            .expect("reduced sum fits u64")
    }
    fn reference_mul(a: u64, b: u64) -> u64 {
        u64::try_from((u128::from(a) * u128::from(b)) % u128::from(super::GOLDILOCKS_MODULUS))
            .expect("reduced product fits u64")
    }
    fn reference_pow(mut base: u64, mut exponent: u64) -> u64 {
        let mut result = 1;
        while exponent > 0 {
            if exponent & 1 == 1 {
                result = reference_mul(result, base);
            }
            base = reference_mul(base, base);
            exponent >>= 1;
        }
        result
    }
    fn reference_inverse(value: u64) -> u64 {
        reference_pow(value, super::GOLDILOCKS_MODULUS - 2)
    }
    fn reference_domain_point(
        params: fastpq_isi::StarkParameterSet,
        domain_size: usize,
        index: usize,
    ) -> u64 {
        let domain_log = domain_size.ilog2();
        let stride = 1u64 << (params.lde_log_size - domain_log);
        let generator = reference_pow(params.lde_root, stride);
        reference_mul(
            params.omega_coset,
            reference_pow(generator, u64::try_from(index).expect("index fits u64")),
        )
    }
    fn reference_fold_round(
        values: &[GoldilocksFp4V1],
        configured_arity: usize,
        beta: GoldilocksFp4V1,
        domain: super::FriDomain,
    ) -> Vec<GoldilocksFp4V1> {
        let arity = configured_arity.min(values.len());
        assert_eq!(arity, 2, "V1 reference supports only binary FRI");
        assert_eq!(values.len() % arity, 0);
        let output_len = values.len() / arity;
        let inverse_two = reference_inverse(2);
        (0..output_len)
            .map(|leaf_index| {
                let positive = values[leaf_index];
                let negative = values[leaf_index + output_len];
                let even = positive.add(negative).mul_base(inverse_two);
                let odd = positive.sub(negative).mul_base(reference_mul(
                    inverse_two,
                    reference_inverse(domain.point(leaf_index)),
                ));
                even.add(beta.mul(odd))
            })
            .collect()
    }
    fn reference_fri_layer_commitment(
        round: usize,
        values: &[GoldilocksFp4V1],
    ) -> GoldilocksDigest384V1 {
        let leaves = hash_fri_leaves_with_mode(round, values, 2, ExecutionMode::Cpu)
            .expect("typed FRI leaves");
        merkle_root_with_mode(
            &leaves,
            MerkleTreeRoleV1::Fri(u32::try_from(round).expect("round fits u32")),
            ExecutionMode::Cpu,
        )
        .expect("typed FRI root")
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
        let (layers, betas) = super::fold_with_fri(
            &evaluations,
            params.fri.arity,
            3,
            params.lde_root,
            params.lde_log_size,
            params.omega_coset,
            &mut transcript,
        )
        .expect("fold with fri");
        let mut reference_transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            params.name,
            1,
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .expect("reference transcript");
        let mut current = evaluations.iter().copied().map(fp4).collect::<Vec<_>>();
        let mut reference_layers = Vec::new();
        let mut reference_betas = Vec::new();
        let mut domain = super::FriDomain::from_lde_parameters(
            params.lde_root,
            params.lde_log_size,
            current.len(),
            params.omega_coset,
        )
        .expect("reference FRI domain");
        let mut round = 0usize;
        while current.len() > arity && round < 3 {
            let root = reference_fri_layer_commitment(round, &current);
            reference_transcript.append_fri_layer(round, root);
            reference_layers.push(root);
            let beta = reference_transcript.challenge_beta(round);
            reference_betas.push(beta);
            let round_arity = arity.min(current.len());
            current = reference_fold_round(&current, arity, beta, domain);
            domain = domain.folded(round_arity);
            round += 1;
        }
        let final_root = reference_fri_layer_commitment(round, &current);
        reference_transcript.append_fri_final(final_root);
        reference_layers.push(final_root);
        assert_eq!(layers, reference_layers);
        assert_eq!(betas, reference_betas);
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
        let (baseline_layers, _) = super::fold_with_fri(
            &evaluations,
            params.fri.arity,
            3,
            params.lde_root,
            params.lde_log_size,
            params.omega_coset,
            &mut transcript,
        )
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
        let mut current = mutated.iter().copied().map(fp4).collect::<Vec<_>>();
        let mut mutated_layers = Vec::new();
        let mut domain = super::FriDomain::from_lde_parameters(
            params.lde_root,
            params.lde_log_size,
            current.len(),
            params.omega_coset,
        )
        .expect("reference FRI domain");
        let mut round = 0usize;
        while current.len() > arity && round < 3 {
            let root = reference_fri_layer_commitment(round, &current);
            reference_transcript.append_fri_layer(round, root);
            mutated_layers.push(root);
            let beta = reference_transcript.challenge_beta(round);
            let round_arity = arity.min(current.len());
            current = reference_fold_round(&current, arity, beta, domain);
            domain = domain.folded(round_arity);
            round += 1;
        }
        let final_root = reference_fri_layer_commitment(round, &current);
        reference_transcript.append_fri_final(final_root);
        mutated_layers.push(final_root);
        assert_ne!(baseline_layers, mutated_layers);
    }
    mod fri_properties {
        use super::*;
        use crate::Planner;
        use fastpq_isi::CANONICAL_PARAMETER_SETS;
        const MAX_TRACE_LOG: u32 = 4;
        fn fri_input_cases() -> Vec<(u32, Vec<u64>)> {
            let mut cases = Vec::new();
            for trace_log in 0..=MAX_TRACE_LOG {
                let len = 1usize << trace_log;
                for seed in [0_u64, 1, 0x1234, 0xFEED] {
                    let coeffs = (0..len)
                        .map(|idx| {
                            seed.wrapping_add(idx as u64)
                                .wrapping_mul(0xD6E8_FEB8_6659_FD93)
                                % crate::poseidon::FIELD_MODULUS
                        })
                        .collect();
                    cases.push((trace_log, coeffs));
                }
            }
            cases
        }
        #[test]
        fn fri_layers_and_betas_are_deterministic() {
            for (trace_log, coeffs) in fri_input_cases() {
                let params = CANONICAL_PARAMETER_SETS[0];
                let planner = Planner::new(&params);
                let trace_len = 1usize << trace_log;
                assert_eq!(coeffs.len(), trace_len);
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
                    params.lde_root,
                    params.lde_log_size,
                    params.omega_coset,
                    &mut transcript_a,
                )
                .expect("fri folding");
                let (layers_b, betas_b) = fold_with_fri(
                    &evaluation,
                    params.fri.arity,
                    params.fri.max_reductions,
                    params.lde_root,
                    params.lde_log_size,
                    params.omega_coset,
                    &mut transcript_b,
                )
                .expect("fri folding");
                assert_eq!(layers_a, layers_b);
                assert_eq!(betas_a, betas_b);
                let expected_len = trace_len << planner.blowup_log();
                assert_eq!(evaluation.len(), expected_len);
            }
        }
    }
}
