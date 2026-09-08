use crate::{
    Error, Result, TransitionBatch,
    fft::Planner,
    field::GoldilocksFp4V1,
    gadgets::transfer_integer_air,
    overrides,
    proof::{AirConstraintOpening, FriQueryOpening, FriRoundOpening, PublicIO},
    trace::{PoseidonPipelinePolicy, build_trace, derive_polynomial_data},
};
use core::convert::TryFrom;
use fastpq_isi::{
    FASTPQ_CATALOG_V1, FASTPQ_FINAL_V1_ID, GoldilocksDigest384DomainPrefixV1,
    GoldilocksDigest384V1, GoldilocksDigestDomainV1, StarkParameterSet, hash_bytes_384_v1,
};
use iroha_data_model::privacy::GoldilocksDigest384V1 as WireGoldilocksDigest384V1;
#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
use metal::{Device, MTLDeviceLocation};
use rayon::prelude::*;
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
#[path = "backend/air_quotient.rs"]
mod air_quotient;
pub(crate) use air_quotient::{AirQuotientDomain, AirQuotientWeights};
#[path = "backend/joint_fri.rs"]
mod joint_fri;
pub(crate) use joint_fri::JointFriBatch;
#[path = "backend/merkle_cache.rs"]
mod merkle_cache;
pub(crate) use merkle_cache::MerkleNodeCache;
#[cfg(test)]
#[path = "backend/compact_axt_air.rs"]
mod compact_axt_air;
#[path = "backend/compact_axt_batch.rs"]
mod compact_axt_batch;
#[cfg(test)]
#[path = "backend/compact_axt_bundle_diagnostic.rs"]
mod compact_axt_bundle_diagnostic;
#[path = "backend/compact_axt_context.rs"]
mod compact_axt_context;
#[path = "backend/compact_bundle.rs"]
mod compact_bundle;
#[cfg(test)]
#[path = "backend/compact_bundle_diagnostic.rs"]
mod compact_bundle_diagnostic;
#[path = "backend/compact_hash_quotient.rs"]
mod compact_hash_quotient;
#[path = "backend/compact_model_statement.rs"]
mod compact_model_statement;
#[path = "backend/compact_protocol.rs"]
mod compact_protocol;
#[path = "backend/compact_public_api.rs"]
mod compact_public_api;
#[path = "backend/compact_public_batch.rs"]
mod compact_public_batch;
#[path = "backend/compact_public_transfer.rs"]
mod compact_public_transfer;
#[cfg(test)]
#[path = "backend/compact_quantity_diagnostic.rs"]
mod compact_quantity_diagnostic;
#[cfg(test)]
#[path = "backend/compact_quantity_tests.rs"]
mod compact_quantity_tests;
#[path = "backend/compact_smt_quotient.rs"]
mod compact_smt_quotient;
#[path = "backend/compact_transfer_air.rs"]
mod compact_transfer_air;
#[path = "backend/compact_v1.rs"]
mod compact_v1;
#[path = "backend/compact_value_domain.rs"]
mod compact_value_domain;
#[cfg(test)]
#[path = "backend/extension_trace.rs"]
mod extension_trace;
#[path = "backend/fixed_domain.rs"]
mod fixed_domain;
#[path = "backend/fixed_schedule.rs"]
mod fixed_schedule;
#[path = "backend/fri_openings.rs"]
mod fri_openings;
#[path = "backend/merkle_multiproof.rs"]
mod merkle_multiproof;
#[path = "backend/offline_compact.rs"]
pub mod offline_compact;
#[cfg(test)]
#[path = "backend/phased_trace.rs"]
mod phased_trace;
#[path = "backend/public_table.rs"]
mod public_table;
const FIELD_ONE: u64 = 1;
const TRACE_COMMITMENT_ROLE_V1: &[u8] = b"trace-commitment";
const LDE_COMMITMENT_ROLE_V1: &[u8] = b"lde-commitment";
const AIR_TRACE_COMMITMENT_ROLE_V1: &[u8] = b"air-trace-commitment";
const AIR_COMPOSITION_COMMITMENT_ROLE_V1: &[u8] = b"air-composition-commitment";
const FRI_COMMITMENT_ROLE_V1: &[u8] = b"fri-commitment";
const TRANSCRIPT_ROLE_V1: &[u8] = b"fiat-shamir-transcript";
const MERKLE_LEAF_PHASE_V1: &[u8] = b"leaf";
const MERKLE_NODE_PHASE_V1: &[u8] = b"node";
const MERKLE_EMPTY_PHASE_V1: &[u8] = b"empty-root";
/// Transcript domain for the permission lookup grand-product accumulator.
pub const LOOKUP_PRODUCT_DOMAIN: &str = "fastpq:v1:lookup:product";

/// Typed native-STARK Merkle role; the role and FRI round are bound into every internal node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MerkleTreeRoleV1 {
    /// Base trace column-commitment tree.
    Trace,
    /// Random-column-combination LDE tree.
    Lde,
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
            Self::Lde => LDE_COMMITMENT_ROLE_V1,
            Self::AirTrace => AIR_TRACE_COMMITMENT_ROLE_V1,
            Self::AirComposition => AIR_COMPOSITION_COMMITMENT_ROLE_V1,
            Self::Fri(_) => FRI_COMMITMENT_ROLE_V1,
        }
    }

    fn counter(self) -> u64 {
        match self {
            Self::Fri(round) => u64::from(round),
            Self::Trace | Self::Lde | Self::AirTrace | Self::AirComposition => 0,
        }
    }
}
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
/// Fiat–Shamir domain for the permission lookup challenge.
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
pub const AIR_COMPOSITION_ALPHA_COUNT: usize = AIR_BOOLEAN_RESIDUE_COUNT
    + AIR_RELATION_RESIDUE_COUNT
    + AIR_STABLE_RESIDUE_COUNT
    + transfer_integer_air::CONSTRAINT_COUNT;
/// Conservative exclusive quotient degree bound as a multiple of the trace length.
pub const AIR_QUOTIENT_DEGREE_EXPANSION_V1: usize =
    fastpq_isi::FASTPQ_COMPOSITION_DEGREE_EXPANSION_V1 as usize;
/// Configuration for the FASTPQ backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Run the prover using the scalar CPU implementation.
    Cpu,
    /// Require GPU execution; final-V1 proof construction rejects this until implemented.
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

/// Check whether the complete final-V1 native proof pipeline can execute on GPU.
///
/// The current six-lane commitment and proof FFT/LDE paths execute on CPU.
/// Standalone scalar permutation or FFT kernel parity cannot qualify this
/// pipeline. Callers requiring GPU proofs must reject admission when false.
#[must_use]
pub const fn preflight_native_v1_gpu_backend() -> bool {
    // TODO: enable only after lane-aware digest dispatch, complete proof
    // integration, and fail-closed device parity checks are implemented.
    false
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
/// CPU resolutions report no GPU backend, even when an accelerator is available.
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
            usable_runtime_backend(*backend).is_some()
        }
        GpuOverride::Auto => {
            let backend = GPU_BACKEND.get_or_init(|| {
                let backend = detect_gpu_backend();
                log_detected_backend(backend);
                backend
            });
            usable_runtime_backend(*backend).is_some()
        }
    }
}
pub fn current_gpu_backend() -> Option<GpuBackend> {
    if GPU_BACKEND.get().is_none() {
        let _ = gpu_available();
    }
    usable_runtime_backend(GPU_BACKEND.get().and_then(|backend| *backend))
}
fn usable_runtime_backend(backend: Option<GpuBackend>) -> Option<GpuBackend> {
    #[cfg(any(test, feature = "fastpq-gpu"))]
    let cuda_quarantined =
        matches!(backend, Some(GpuBackend::Cuda)) && crate::fastpq_cuda::backend_quarantined();
    #[cfg(not(any(test, feature = "fastpq-gpu")))]
    let cuda_quarantined = false;
    runtime_backend(backend, cuda_quarantined)
}
fn runtime_backend(backend: Option<GpuBackend>, cuda_quarantined: bool) -> Option<GpuBackend> {
    match backend {
        Some(GpuBackend::Cuda) if cuda_quarantined => None,
        backend => backend,
    }
}
#[cfg(any(test, feature = "dev-tools"))]
fn default_batch_execution_mode() -> ExecutionMode {
    if current_gpu_backend().is_some() {
        ExecutionMode::Gpu
    } else {
        ExecutionMode::Cpu
    }
}
fn log_execution_resolution(requested: ExecutionMode, resolved: ExecutionMode) {
    // A discovered accelerator does not imply that this resolution uses it.
    // In particular, CPU-pinned runs must not be counted as GPU backend work.
    let backend = match resolved {
        ExecutionMode::Gpu => current_gpu_backend(),
        ExecutionMode::Cpu | ExecutionMode::Auto => None,
    };
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
    fn quarantined_cuda_is_removed_from_cached_availability() {
        assert_eq!(runtime_backend(Some(GpuBackend::Cuda), true), None);
        assert_eq!(
            runtime_backend(Some(GpuBackend::Cuda), false),
            Some(GpuBackend::Cuda)
        );
        assert_eq!(
            runtime_backend(Some(GpuBackend::Metal), true),
            Some(GpuBackend::Metal)
        );
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
    fn execution_mode_observer_reports_no_gpu_backend_for_cpu_resolution() {
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

        for requested in [ExecutionMode::Cpu, ExecutionMode::Auto, ExecutionMode::Gpu] {
            log_execution_resolution(requested, ExecutionMode::Cpu);
            assert_eq!(
                rx.recv_timeout(Duration::from_secs(2))
                    .expect("CPU resolution event"),
                (requested, ExecutionMode::Cpu, None),
                "CPU resolutions must report the route independently of available hardware"
            );
        }
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
    fn native_v1_proof_auto_reports_cpu_execution_and_poseidon() {
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
        let batch = TransitionBatch::new(params.name, crate::PublicInputs::default());
        for requested_poseidon in [PoseidonExecutionMode::Auto, PoseidonExecutionMode::Cpu] {
            let backend = StarkBackend::new(
                BackendConfig::new(params)
                    .with_execution_mode(ExecutionMode::Cpu)
                    .with_poseidon_mode(requested_poseidon),
            );
            backend
                .prove(&batch, &PublicIO::default(), 1)
                .expect("configured backend proof");

            let (requested, resolved, execution_backend) = execution_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("execution-mode event");
            assert_eq!(requested, ExecutionMode::Cpu);
            assert_eq!(resolved, ExecutionMode::Cpu);
            assert_eq!(execution_backend, None);
            let (policy, path, hashing_backend) = poseidon_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("native-STARK hashing event");
            assert_eq!(policy.requested(), requested_poseidon);
            assert_eq!(policy.resolved(), ExecutionMode::Cpu);
            assert_eq!(hashing_backend, None);
            assert_eq!(
                path,
                if requested_poseidon == PoseidonExecutionMode::Cpu {
                    "cpu_forced"
                } else {
                    "cpu_fallback"
                }
            );
            assert!(
                poseidon_rx.try_recv().is_err(),
                "one native hashing event per preparation"
            );
        }
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

        derive_batch_commitments(&params, &batch, &PublicIO::default(), 1)
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
/// Internal backend configuration used by the FASTPQ prover.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BackendConfig {
    /// Canonical parameter set driving this backend instance.
    params: StarkParameterSet,
    /// Execution mode used for FFT/LDE computations.
    execution_mode: ExecutionMode,
    /// Poseidon pipeline override (defaults to [`ExecutionMode::Auto`]).
    poseidon_mode: PoseidonExecutionMode,
}
impl BackendConfig {
    /// Construct a configuration from a canonical parameter set.
    pub(crate) fn new(params: StarkParameterSet) -> Self {
        Self {
            params,
            execution_mode: ExecutionMode::Cpu,
            poseidon_mode: PoseidonExecutionMode::Cpu,
        }
    }
    /// Override the execution mode used by the backend.
    #[must_use]
    pub(crate) fn with_execution_mode(mut self, mode: ExecutionMode) -> Self {
        self.execution_mode = mode;
        self
    }
    /// Override the Poseidon pipeline execution mode used by the backend.
    #[must_use]
    pub(crate) fn with_poseidon_mode(mut self, mode: PoseidonExecutionMode) -> Self {
        self.poseidon_mode = mode;
        self
    }
    /// Return the requested execution mode.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn execution_mode(&self) -> ExecutionMode {
        self.execution_mode
    }
    /// Return the configured Poseidon pipeline mode.
    #[must_use]
    pub(crate) fn poseidon_mode(&self) -> PoseidonExecutionMode {
        self.poseidon_mode
    }
    /// Reject requested proof execution paths that have no native-V1 implementation.
    pub(crate) fn validate_native_v1_modes(&self) -> Result<()> {
        if self.execution_mode == ExecutionMode::Gpu
            || self.poseidon_mode == PoseidonExecutionMode::Gpu
        {
            return Err(Error::NativeV1GpuUnavailable);
        }
        Ok(())
    }
    /// Resolve and report the execution path actually used by native-V1 proofs.
    fn resolve_native_v1_execution_mode(&self) -> Result<ExecutionMode> {
        self.validate_native_v1_modes()?;
        log_execution_resolution(self.execution_mode, ExecutionMode::Cpu);
        Ok(ExecutionMode::Cpu)
    }
}
/// Deterministic artifact emitted by the native STARK backend.
///
/// This mirrors the minimal data the verifier needs to
/// reconstruct the Fiat–Shamir transcript and query openings.
#[derive(Debug, Clone)]
pub(crate) struct BackendArtifact {
    /// Canonical parameter set name.
    pub(crate) parameter: String,
    /// Canonical commitment over the parameterised trace.
    pub(crate) trace_commitment: WireGoldilocksDigest384V1,
    /// Poseidon Merkle root over column commitments.
    pub(crate) trace_root: GoldilocksDigest384V1,
    /// Poseidon Merkle root over row-major AIR trace openings.
    pub(crate) air_trace_root: GoldilocksDigest384V1,
    /// Poseidon Merkle root over AIR composition evaluations.
    pub(crate) air_composition_root: GoldilocksDigest384V1,
    /// Poseidon Merkle root over the low-degree extension leaf hashes.
    pub(crate) lde_root: GoldilocksDigest384V1,
    /// Number of evaluation rows committed under `lde_root`.
    pub(crate) lde_domain_size: u32,
    /// Permission accumulator over the canonical witness LDE.
    pub(crate) lookup_grand_product: u64,
    /// Transcript-derived permission accumulator challenge.
    pub(crate) lookup_challenge: u64,
    /// Fp4 composition challenges sampled after the trace roots and permission challenge.
    pub(crate) alphas: Vec<GoldilocksFp4V1>,
    /// Commitment to each joint FRI layer plus the terminal root.
    pub(crate) fri_layers: Vec<GoldilocksDigest384V1>,
    /// Fiat–Shamir challenges used for each FRI folding round.
    pub(crate) fri_betas: Vec<GoldilocksFp4V1>,
    /// Sampled query openings into the evaluation domain.
    pub(crate) query_openings: Vec<(u32, GoldilocksFp4V1)>,
    /// Full LDE leaf chunks containing each queried evaluation.
    pub(crate) query_chunks: Vec<Vec<GoldilocksFp4V1>>,
    /// Merkle authentication paths for each queried evaluation chunk.
    pub(crate) query_paths: Vec<Vec<GoldilocksDigest384V1>>,
    /// Sampled AIR row/composition openings.
    pub(crate) air_openings: Vec<AirConstraintOpening>,
    /// Per-round FRI openings for sampled query indices.
    pub(crate) fri_query_openings: Vec<FriQueryOpening>,
}
/// Concrete backend implementing the deterministic FASTPQ STARK pipeline.
#[derive(Debug, Clone)]
pub(crate) struct StarkBackend {
    config: BackendConfig,
}
impl StarkBackend {
    /// Create a backend from a canonical configuration.
    pub(crate) fn new(config: BackendConfig) -> Self {
        Self { config }
    }
    /// Expose the configured execution mode, primarily for tests.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn execution_mode(&self) -> ExecutionMode {
        self.config.execution_mode()
    }

    pub(crate) fn parameter_name(&self) -> &'static str {
        self.config.params.name
    }
    /// Validate proof execution before statement or witness preprocessing.
    pub(crate) fn validate_native_v1_modes(&self) -> Result<()> {
        self.config.validate_native_v1_modes()
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

fn digest_domain_prefix_v1<'a>(
    role: &'a [u8],
    phase: &'a [u8],
    level: usize,
    counter: u64,
) -> Result<GoldilocksDigest384DomainPrefixV1<'a>> {
    GoldilocksDigest384DomainPrefixV1::new(digest_domain_v1(role, phase, level, 0, counter)?).ok_or(
        Error::PayloadLengthOverflow {
            length: role.len().saturating_add(phase.len()),
        },
    )
}

fn hash_at_prefix_v1(
    prefix: &GoldilocksDigest384DomainPrefixV1<'_>,
    index: usize,
    fields: &[&[u8]],
) -> Result<GoldilocksDigest384V1> {
    prefix
        .hash_at(
            u64::try_from(index).map_err(|_| Error::QueryIndexOverflow { index })?,
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
#[cfg(any(test, feature = "dev-tools"))]
pub fn hash_lde_leaves(evaluations: &[u64], arity: u32) -> Result<Vec<GoldilocksDigest384V1>> {
    hash_lde_leaves_with_mode(evaluations, arity, default_batch_execution_mode())
}
#[cfg(any(test, feature = "dev-tools"))]
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
                LDE_COMMITMENT_ROLE_V1,
                MERKLE_LEAF_PHASE_V1,
                0,
                index,
                0,
                values,
            )
        })
        .collect()
}
/// Hash one LDE leaf chunk using the same domain as the batched LDE leaf commitment.
///
/// # Errors
/// Returns an error if the leaf index cannot be represented as a field limb.
#[cfg(test)]
pub fn hash_lde_chunk(leaf_index: usize, values: &[u64]) -> Result<GoldilocksDigest384V1> {
    hash_u64_values_v1(
        LDE_COMMITMENT_ROLE_V1,
        MERKLE_LEAF_PHASE_V1,
        0,
        leaf_index,
        0,
        values,
    )
}
/// Hash a complete mixed-trace Fp4 leaf in coefficient order.
///
/// # Errors
/// Returns an error for a noncanonical coefficient or invalid digest framing.
pub fn hash_lde_chunk_fp4(
    leaf_index: usize,
    values: &[GoldilocksFp4V1],
) -> Result<GoldilocksDigest384V1> {
    hash_fp4_values_v1(LDE_COMMITMENT_ROLE_V1, 0, leaf_index, values)
}
fn hash_fp4_values_v1(
    role: &[u8],
    level: usize,
    index: usize,
    values: &[GoldilocksFp4V1],
) -> Result<GoldilocksDigest384V1> {
    let mut bytes = Vec::with_capacity(values.len().saturating_mul(32));
    for (value_index, value) in values.iter().enumerate() {
        for (coefficient_index, coefficient) in value.coefficients().into_iter().enumerate() {
            if coefficient >= GOLDILOCKS_MODULUS {
                return Err(Error::NonCanonicalGoldilocksElement {
                    context: "native_stark_fp4_digest_input",
                    indices: vec![index, value_index, coefficient_index],
                });
            }
        }
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    hash_bytes_v1(role, MERKLE_LEAF_PHASE_V1, level, index, 0, &[&bytes])
}
fn hash_lde_leaves_fp4(
    evaluations: &[GoldilocksFp4V1],
    arity: u32,
) -> Result<Vec<GoldilocksDigest384V1>> {
    evaluations
        .chunks(lde_chunk_size(arity)?)
        .enumerate()
        .map(|(index, values)| hash_lde_chunk_fp4(index, values))
        .collect()
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
pub fn hash_air_composition_leaf(
    index: usize,
    value: GoldilocksFp4V1,
) -> Result<GoldilocksDigest384V1> {
    hash_fp4_values_v1(AIR_COMPOSITION_COMMITMENT_ROLE_V1, 0, index, &[value])
}
/// Hash all row-major AIR trace leaves.
///
/// # Errors
/// Returns a shape error before hashing, or the first failing row in index order.
fn hash_air_trace_rows_with_mode(
    columns: &[Vec<u64>],
    mode: ExecutionMode,
) -> Result<Vec<GoldilocksDigest384V1>> {
    // Digest384 remains CPU-only for every requested mode. The caller reports
    // the requested policy and actual CPU route before building these leaves.
    let _ = mode;
    if columns.is_empty() {
        return Ok(Vec::new());
    }
    let row_count = columns[0].len();
    if !columns.iter().all(|column| column.len() == row_count) {
        return Err(Error::AirOpeningMismatch { index: 0 });
    }
    if row_count == 0 {
        return Ok(Vec::new());
    }
    // All rows share the exact typed domain through its tree-level field.
    // The immutable prefix still binds each full row index and payload; keep
    // hash_air_trace_row as the independent canonical one-shot oracle.
    let prefix = digest_domain_prefix_v1(AIR_TRACE_COMMITMENT_ROLE_V1, MERKLE_LEAF_PHASE_V1, 0, 0)?;
    let row_bytes = columns.len().saturating_mul(8);
    let hash_row = |bytes: &mut Vec<u8>, row_index: usize| {
        bytes.clear();
        for column in columns {
            let value = column[row_index];
            if value >= GOLDILOCKS_MODULUS {
                return Err(Error::NonCanonicalGoldilocksElement {
                    context: "native_stark_digest_input",
                    indices: vec![row_index],
                });
            }
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        hash_at_prefix_v1(&prefix, row_index, &[bytes.as_slice()])
    };
    // Keep fewer than two 16-row jobs sequential. This avoids scheduling tiny
    // batches and reuses each job's canonical byte buffer across several hashes.
    const ROWS_PER_JOB: usize = 16;
    if row_count < 2 * ROWS_PER_JOB {
        let mut bytes = Vec::with_capacity(row_bytes);
        return (0..row_count)
            .map(|row_index| hash_row(&mut bytes, row_index))
            .collect();
    }
    let results: Vec<Result<GoldilocksDigest384V1>> = (0..row_count)
        .into_par_iter()
        .with_min_len(ROWS_PER_JOB)
        .map_init(
            || Vec::with_capacity(row_bytes),
            |bytes, row_index| hash_row(bytes, row_index),
        )
        .collect();
    // Indexed collection preserves leaf order. Select errors serially as well:
    // a parallel Result reduction could report whichever malformed row finishes first.
    results.into_iter().collect()
}
/// Hash all AIR composition leaves.
///
/// # Errors
/// Returns an error if leaf hashing fails.
fn hash_air_composition_leaves_with_mode(
    values: &[GoldilocksFp4V1],
    mode: ExecutionMode,
) -> Result<Vec<GoldilocksDigest384V1>> {
    let _ = mode;
    hash_fp4_single_leaves_with_role(AIR_COMPOSITION_COMMITMENT_ROLE_V1, values)
}

// One immutable typed prefix serves an entire natural-order oracle. Each leaf
// still binds its full index and all four canonical coordinates. Stack payloads
// avoid a tiny allocation per leaf; indexed jobs preserve deterministic errors.
fn hash_fp4_single_leaves_with_role(
    role: &[u8],
    values: &[GoldilocksFp4V1],
) -> Result<Vec<GoldilocksDigest384V1>> {
    if values.is_empty() {
        return Ok(Vec::new());
    }
    let prefix = digest_domain_prefix_v1(role, MERKLE_LEAF_PHASE_V1, 0, 0)?;
    let hash_leaf = |(index, value): (usize, &GoldilocksFp4V1)| {
        for (coefficient_index, coefficient) in value.coefficients().into_iter().enumerate() {
            if coefficient >= GOLDILOCKS_MODULUS {
                return Err(Error::NonCanonicalGoldilocksElement {
                    context: "native_stark_fp4_digest_input",
                    indices: vec![index, 0, coefficient_index],
                });
            }
        }
        hash_at_prefix_v1(&prefix, index, &[&value.to_le_bytes()])
    };
    if values.len() < 32 {
        return values.iter().enumerate().map(hash_leaf).collect();
    }
    let results: Vec<Result<_>> = values.par_iter().enumerate().map(hash_leaf).collect();
    results.into_iter().collect()
}

#[derive(Debug)]
struct AirColumnLayout {
    boolean_selectors: [usize; AIR_BOOLEAN_RESIDUE_COUNT],
    operation_selectors: [usize; 6],
    numeric_selectors: [usize; 3],
    permission_selectors: [usize; 2],
    s_active: usize,
    s_transfer: usize,
    s_perm: usize,
    perm_hash: usize,
    delta: usize,
    value_old_limbs: Vec<usize>,
    value_new_limbs: Vec<usize>,
    value_old_len: usize,
    value_new_len: usize,
    integer_auxiliary: Option<[usize; transfer_integer_air::AUXILIARY_COLUMN_COUNT]>,
    stable_columns: [usize; AIR_STABLE_RESIDUE_COUNT],
}

impl AirColumnLayout {
    fn from_names<S: AsRef<str>>(column_names: &[S]) -> Result<Self> {
        let required = |name: &str| -> Result<usize> {
            column_names
                .iter()
                .position(|column| column.as_ref() == name)
                .ok_or_else(|| Error::MissingColumn(name.to_owned()))
        };
        let s_active = required("s_active")?;
        let s_transfer = required("s_transfer")?;
        let s_mint = required("s_mint")?;
        let s_burn = required("s_burn")?;
        let s_role_grant = required("s_role_grant")?;
        let s_role_revoke = required("s_role_revoke")?;
        let s_meta_set = required("s_meta_set")?;
        let s_perm = required("s_perm")?;
        let perm_hash = required("perm_hash")?;
        let delta = required("delta")?;
        let mut stable_columns = [0usize; AIR_STABLE_RESIDUE_COUNT];
        for (limb, column) in stable_columns[..crate::trace::METADATA_COMMITMENT_LIMBS]
            .iter_mut()
            .enumerate()
        {
            *column = required(&format!("metadata_hash_limb_{limb}"))?;
        }
        stable_columns[crate::trace::METADATA_COMMITMENT_LIMBS] = required("dsid")?;
        stable_columns[crate::trace::METADATA_COMMITMENT_LIMBS + 1] = required("slot")?;
        let integer_names = transfer_integer_air::auxiliary_column_names();
        let has_integer_columns = integer_names
            .iter()
            .any(|name| column_names.iter().any(|column| column.as_ref() == name));
        let integer_auxiliary = if has_integer_columns {
            let mut columns = [0; transfer_integer_air::AUXILIARY_COLUMN_COUNT];
            for (column, name) in columns.iter_mut().zip(&integer_names) {
                *column = required(name)?;
            }
            Some(columns)
        } else {
            None
        };
        Ok(Self {
            boolean_selectors: [
                s_active,
                s_transfer,
                s_mint,
                s_burn,
                s_role_grant,
                s_role_revoke,
                s_meta_set,
                s_perm,
            ],
            operation_selectors: [
                s_transfer,
                s_mint,
                s_burn,
                s_role_grant,
                s_role_revoke,
                s_meta_set,
            ],
            numeric_selectors: [s_transfer, s_mint, s_burn],
            permission_selectors: [s_role_grant, s_role_revoke],
            s_active,
            s_transfer,
            s_perm,
            perm_hash,
            delta,
            value_old_limbs: contiguous_limb_columns(column_names, "value_old_limb_"),
            value_new_limbs: contiguous_limb_columns(column_names, "value_new_limb_"),
            value_old_len: required("value_old_len")?,
            value_new_len: required("value_new_len")?,
            integer_auxiliary,
            stable_columns,
        })
    }

    fn residue_count(&self) -> usize {
        AIR_COMPOSITION_ALPHA_COUNT
            + self.value_old_limbs.len().saturating_sub(2)
            + self.value_new_limbs.len().saturating_sub(2)
    }
}

/// Number of independent coefficients required by the canonical column schema.
pub(crate) fn air_composition_alpha_count<S: AsRef<str>>(column_names: &[S]) -> usize {
    AIR_COMPOSITION_ALPHA_COUNT
        + contiguous_limb_columns(column_names, "value_old_limb_")
            .len()
            .saturating_sub(2)
        + contiguous_limb_columns(column_names, "value_new_limb_")
            .len()
            .saturating_sub(2)
}

fn contiguous_limb_columns<S: AsRef<str>>(column_names: &[S], prefix: &str) -> Vec<usize> {
    let mut columns = Vec::new();
    for limb in 0usize.. {
        let name = format!("{prefix}{limb}");
        let Some(index) = column_names
            .iter()
            .position(|column| column.as_ref() == name.as_str())
        else {
            break;
        };
        columns.push(index);
    }
    columns
}

fn packed_column_value_at<F>(columns: &[usize], value_at: &F) -> u64
where
    F: Fn(usize) -> u64,
{
    let mut value = 0u64;
    let mut radix_power = FIELD_ONE;
    let limb_radix = 1u64 << 56;
    for &column in columns {
        value = add_mod(value, mul_mod(value_at(column), radix_power));
        radix_power = mul_mod(radix_power, limb_radix);
    }
    value
}

fn air_constraint_residues_with_layout<C, N>(
    layout: &AirColumnLayout,
    current: C,
    next: N,
    residues: &mut [u64],
) where
    C: Fn(usize) -> u64,
    N: Fn(usize) -> u64,
{
    debug_assert_eq!(residues.len(), layout.residue_count());
    let mut residue_index = 0;
    for &selector in &layout.boolean_selectors {
        residues[residue_index] = mul_mod(current(selector), sub_mod(current(selector), FIELD_ONE));
        residue_index += 1;
    }
    let operation_sum = layout
        .operation_selectors
        .iter()
        .fold(0u64, |sum, &selector| add_mod(sum, current(selector)));
    residues[residue_index] = sub_mod(current(layout.s_active), operation_sum);
    residue_index += 1;
    let permission_sum = layout
        .permission_selectors
        .iter()
        .fold(0u64, |sum, &selector| add_mod(sum, current(selector)));
    residues[residue_index] = sub_mod(current(layout.s_perm), permission_sum);
    residue_index += 1;
    residues[residue_index] = mul_mod(
        next(layout.s_active),
        sub_mod(FIELD_ONE, current(layout.s_active)),
    );
    residue_index += 1;
    let value_old = packed_column_value_at(&layout.value_old_limbs, &current);
    let value_new = packed_column_value_at(&layout.value_new_limbs, &current);
    let expected_delta = sub_mod(value_new, value_old);
    let numeric_selector = layout
        .numeric_selectors
        .iter()
        .fold(0u64, |sum, &selector| add_mod(sum, current(selector)));
    residues[residue_index] = mul_mod(
        numeric_selector,
        sub_mod(expected_delta, current(layout.delta)),
    );
    residue_index += 1;
    for &stable in &layout.stable_columns {
        residues[residue_index] = sub_mod(current(stable), next(stable));
        residue_index += 1;
    }
    // Metadata-only schemas omit the transfer auxiliary columns. Their
    // canonical transfer witness is the identically-zero polynomial.
    let auxiliary = layout.integer_auxiliary.map_or(
        [0; transfer_integer_air::AUXILIARY_COLUMN_COUNT],
        |columns| columns.map(&current),
    );
    let before = core::array::from_fn(|limb| {
        layout
            .value_old_limbs
            .get(limb)
            .map_or(0, |&column| current(column))
    });
    let after = core::array::from_fn(|limb| {
        layout
            .value_new_limbs
            .get(limb)
            .map_or(0, |&column| current(column))
    });
    let witness =
        transfer_integer_air::TransferIntegerWitness::from_auxiliary(before, after, &auxiliary);
    let integer_residues = transfer_integer_air::constraint_residues(
        current(layout.s_transfer),
        current(layout.value_old_len),
        current(layout.value_new_len),
        &witness,
    );
    residues[residue_index..residue_index + integer_residues.len()]
        .copy_from_slice(&integer_residues);
    residue_index += integer_residues.len();
    for &column in layout
        .value_old_limbs
        .iter()
        .skip(2)
        .chain(layout.value_new_limbs.iter().skip(2))
    {
        residues[residue_index] = mul_mod(current(layout.s_transfer), current(column));
        residue_index += 1;
    }
    debug_assert_eq!(residue_index, residues.len());
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
    let layout = AirColumnLayout::from_names(column_names)?;
    let mut residues = vec![0; layout.residue_count()];
    air_constraint_residues_with_layout(
        &layout,
        |column| current[column],
        |column| next[column],
        &mut residues,
    );
    Ok(residues)
}

/// Field operations needed to combine base-field AIR residues.
pub(crate) trait AirCombinationField: Copy {
    /// Additive identity.
    const ZERO: Self;
    /// Add one combined residue.
    fn add(self, rhs: Self) -> Self;
    /// Scale by a base-field residue or zerofier weight.
    fn mul_base(self, rhs: u64) -> Self;
}
impl AirCombinationField for u64 {
    const ZERO: Self = 0;
    fn add(self, rhs: Self) -> Self {
        add_mod(self, rhs)
    }
    fn mul_base(self, rhs: u64) -> Self {
        mul_mod(self, rhs)
    }
}
impl AirCombinationField for GoldilocksFp4V1 {
    const ZERO: Self = Self::ZERO;
    fn add(self, rhs: Self) -> Self {
        self.add(rhs)
    }
    fn mul_base(self, rhs: u64) -> Self {
        self.mul_base(rhs)
    }
}
fn validate_air_composition_alphas<F>(alphas: &[F], expected: usize) -> Result<()> {
    if alphas.len() != expected {
        return Err(Error::AirChallengeCountMismatch {
            expected,
            actual: alphas.len(),
        });
    }
    Ok(())
}

#[cfg(test)]
fn combine_air_constraint_residues<F: AirCombinationField>(alphas: &[F], residues: &[u64]) -> F {
    alphas
        .iter()
        .zip(residues)
        .fold(F::ZERO, |acc, (&alpha, &residue)| {
            acc.add(alpha.mul_base(residue))
        })
}

fn combine_air_quotients<F: AirCombinationField>(
    alphas: &[F],
    residues: &[u64],
    weights: AirQuotientWeights,
) -> F {
    let mut rows = F::ZERO;
    let mut transitions = F::ZERO;
    for (index, (&alpha, &residue)) in alphas.iter().zip(residues).enumerate() {
        let weighted = alpha.mul_base(residue);
        // Prefix shape and stability relate adjacent rows. Their zerofier
        // excludes the last row; all other constraints hold at every row.
        if index == AIR_BOOLEAN_RESIDUE_COUNT + 2
            || (AIR_BOOLEAN_RESIDUE_COUNT + AIR_RELATION_RESIDUE_COUNT
                ..AIR_BOOLEAN_RESIDUE_COUNT + AIR_RELATION_RESIDUE_COUNT + AIR_STABLE_RESIDUE_COUNT)
                .contains(&index)
        {
            transitions = transitions.add(weighted);
        } else {
            rows = rows.add(weighted);
        }
    }
    rows.mul_base(weights.all_rows)
        .add(transitions.mul_base(weights.transitions))
}

/// Evaluate the quotient relation at a sampled authenticated coset point.
pub(crate) fn air_quotient_value_for_rows<F: AirCombinationField>(
    column_names: &[String],
    current: &[u64],
    next: &[u64],
    alphas: &[F],
    weights: AirQuotientWeights,
) -> Result<F> {
    validate_air_composition_alphas(alphas, air_composition_alpha_count(column_names))?;
    let residues = air_constraint_residues_for_rows(column_names, current, next)?;
    Ok(combine_air_quotients(alphas, &residues, weights))
}

/// Evaluate the sampled FASTPQ AIR composition value for two adjacent rows.
///
/// # Errors
/// Returns an error when the advertised column schema is missing mandatory columns, the
/// challenge count differs from the residue count, or row widths do not match the schema.
#[cfg(test)]
pub fn air_composition_value_for_rows(
    column_names: &[String],
    current: &[u64],
    next: &[u64],
    alphas: &[u64],
) -> Result<u64> {
    validate_air_composition_alphas(alphas, air_composition_alpha_count(column_names))?;
    let residues = air_constraint_residues_for_rows(column_names, current, next)?;
    Ok(combine_air_constraint_residues(alphas, &residues))
}
/// Evaluate FASTPQ AIR composition values over all row openings.
///
/// # Errors
/// Returns an error when columns have inconsistent lengths or the schema is malformed.
#[cfg(test)]
pub fn air_composition_values(
    column_names: &[String],
    columns: &[Vec<u64>],
    alphas: &[u64],
    next_step: usize,
) -> Result<Vec<u64>> {
    air_values(column_names, columns, alphas, next_step, None)
}

fn air_quotient_values<F: AirCombinationField>(
    params: &StarkParameterSet,
    column_names: &[String],
    columns: &[Vec<u64>],
    alphas: &[F],
) -> Result<Vec<F>> {
    let row_count = columns.first().map_or(0, Vec::len);
    let domain = AirQuotientDomain::new(params, row_count)?;
    let next_step =
        usize::try_from(params.fri.blowup_factor).expect("FRI blowup factor fits usize");
    air_values(column_names, columns, alphas, next_step, Some(&domain))
}

fn air_values<F: AirCombinationField>(
    column_names: &[String],
    columns: &[Vec<u64>],
    alphas: &[F],
    next_step: usize,
    quotient: Option<&AirQuotientDomain>,
) -> Result<Vec<F>> {
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
    if row_count == 0 {
        return Ok(Vec::new());
    }
    validate_air_composition_alphas(alphas, air_composition_alpha_count(column_names))?;
    if columns.len() != column_names.len() {
        return Err(Error::AirOpeningMismatch {
            index: columns.len(),
        });
    }
    let layout = AirColumnLayout::from_names(column_names)?;
    let next_step = next_step % row_count;
    let mut values = Vec::with_capacity(row_count);
    let mut quotient_weights = quotient.map(AirQuotientDomain::weights);
    let mut residues = vec![0; layout.residue_count()];
    for index in 0..row_count {
        let next_index = (index + next_step) % row_count;
        air_constraint_residues_with_layout(
            &layout,
            |column| columns[column][index],
            |column| columns[column][next_index],
            &mut residues,
        );
        if let Some(weights) = quotient_weights.as_mut().and_then(Iterator::next) {
            values.push(combine_air_quotients(alphas, &residues, weights));
        } else {
            #[cfg(test)]
            values.push(combine_air_constraint_residues(alphas, &residues));
            #[cfg(not(test))]
            return Err(Error::AirOpeningMismatch { index });
        }
    }
    Ok(values)
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
    if trace.padded_len == 0 {
        return Ok(());
    }
    let column_names = trace
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect::<Vec<_>>();
    let layout = AirColumnLayout::from_names(&column_names)?;
    let mut residues = vec![0; layout.residue_count()];
    for row_index in 0..trace.padded_len {
        let next_index = row_index
            .checked_add(1)
            .filter(|next| *next < trace.padded_len)
            .unwrap_or(row_index);
        air_constraint_residues_with_layout(
            &layout,
            |column| trace.columns[column].values[row_index],
            |column| trace.columns[column].values[next_index],
            &mut residues,
        );
        if residues.iter().any(|value| *value != 0) {
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
    composition_values: &[GoldilocksFp4V1],
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
                        .map(WireGoldilocksDigest384V1::from)
                        .collect(),
                    next_row_path: next_row_path
                        .into_iter()
                        .map(WireGoldilocksDigest384V1::from)
                        .collect(),
                    composition_value: *composition_values.get(index).ok_or(
                        Error::QueryIndexOutOfRange {
                            index,
                            len: composition_values.len(),
                        },
                    )?,
                    composition_path: composition_path
                        .into_iter()
                        .map(WireGoldilocksDigest384V1::from)
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
pub fn open_query_chunks<F: Copy>(
    evaluations: &[F],
    query_indices: &[usize],
    arity: u32,
) -> Result<Vec<Vec<F>>> {
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
#[cfg(any(test, feature = "dev-tools"))]
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
        MerkleTreeRoleV1::Lde,
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
#[cfg(test)]
#[allow(clippy::unnecessary_wraps)]
pub fn verify_merkle_path(
    root: GoldilocksDigest384V1,
    leaf: GoldilocksDigest384V1,
    leaf_index: usize,
    path: &[GoldilocksDigest384V1],
) -> Result<bool> {
    verify_merkle_path_for_role(MerkleTreeRoleV1::Lde, root, leaf, leaf_index, path)
}

/// Verify a Merkle authentication path under an explicit native-STARK tree role.
///
/// # Errors
/// Returns an error when a typed internal-node digest cannot be framed.
#[cfg(test)]
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
fn merkle_digest_execution_v1(
    mode: ExecutionMode,
) -> Result<crate::digest_executor::DigestExecutionV1> {
    use crate::digest_executor::DigestExecutionV1;
    match mode {
        ExecutionMode::Cpu | ExecutionMode::Auto => Ok(DigestExecutionV1::Cpu),
        ExecutionMode::Gpu => {
            #[cfg(feature = "fastpq-gpu")]
            {
                use crate::digest384_gpu::Digest384GpuBackendV1;
                let backend = match current_gpu_backend() {
                    Some(GpuBackend::Metal) => Digest384GpuBackendV1::Metal,
                    Some(GpuBackend::Cuda) => Digest384GpuBackendV1::Cuda,
                    _ => {
                        return Err(Error::NativeDigestExecution {
                            details: "no supported explicit six-lane device backend".into(),
                        });
                    }
                };
                Ok(DigestExecutionV1::Device(backend))
            }
            #[cfg(not(feature = "fastpq-gpu"))]
            {
                Err(Error::NativeDigestExecution {
                    details: "six-lane device support is not compiled".into(),
                })
            }
        }
    }
}

fn build_merkle_levels_with_mode(
    leaves: &[GoldilocksDigest384V1],
    role: MerkleTreeRoleV1,
    mode: ExecutionMode,
) -> Result<Vec<Vec<GoldilocksDigest384V1>>> {
    build_merkle_levels_with_execution_v1(leaves, role, merkle_digest_execution_v1(mode)?)
}

fn build_merkle_levels_with_execution_v1(
    leaves: &[GoldilocksDigest384V1],
    role: MerkleTreeRoleV1,
    execution: crate::digest_executor::DigestExecutionV1,
) -> Result<Vec<Vec<GoldilocksDigest384V1>>> {
    let levels = build_merkle_levels_with_executor_v1(leaves, role, &mut |frames| {
        crate::digest_executor::execute_digest384_frames_v1(frames, execution)
    })?;
    #[cfg(test)]
    if !leaves.is_empty() {
        use crate::digest_executor::DigestExecutionV1;
        crate::trace::notify_trace_merkle_mode_observer(match execution {
            DigestExecutionV1::Cpu => ExecutionMode::Cpu,
            #[cfg(feature = "fastpq-gpu")]
            DigestExecutionV1::Device(_) => ExecutionMode::Gpu,
        });
    }
    Ok(levels)
}

fn build_merkle_levels_with_executor_v1(
    leaves: &[GoldilocksDigest384V1],
    role: MerkleTreeRoleV1,
    execute: &mut impl FnMut(
        &[fastpq_isi::GoldilocksDigest384FrameV1<'_>],
    ) -> Result<Vec<GoldilocksDigest384V1>>,
) -> Result<Vec<Vec<GoldilocksDigest384V1>>> {
    if leaves.is_empty() {
        return Ok(Vec::new());
    }
    let mut levels = Vec::new();
    let mut current = leaves.to_vec();
    loop {
        if !current.len().is_multiple_of(2) {
            current.push(*current.last().expect("non-empty Merkle level"));
        }
        levels.push(current.clone());
        let level = levels.len();
        let next = crate::digest_executor::hash_digest384_pairs_v1(
            &current,
            |index| {
                digest_domain_v1(
                    role.role(),
                    MERKLE_NODE_PHASE_V1,
                    level,
                    index,
                    role.counter(),
                )
            },
            execute,
        )?;
        if next.len() == 1 {
            levels.push(next);
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
    merkle_root_with_execution_v1(leaves, role, merkle_digest_execution_v1(mode)?)
}

fn merkle_root_with_execution_v1(
    leaves: &[GoldilocksDigest384V1],
    role: MerkleTreeRoleV1,
    execution: crate::digest_executor::DigestExecutionV1,
) -> Result<GoldilocksDigest384V1> {
    let levels = build_merkle_levels_with_execution_v1(leaves, role, execution)?;
    match levels.last().and_then(|level| level.first()).copied() {
        Some(root) => Ok(root),
        None => {
            let frame = fastpq_isi::GoldilocksDigest384FrameV1::new(
                digest_domain_v1(role.role(), MERKLE_EMPTY_PHASE_V1, 0, 0, role.counter())?,
                &[],
            )
            .ok_or(Error::PayloadLengthOverflow { length: 0 })?;
            let result = crate::digest_executor::execute_digest384_frames_v1(&[frame], execution)?;
            Ok(result[0])
        }
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

/// Compute the Fiat–Shamir lookup grand-product accumulator over canonical
/// Goldilocks selector and witness evaluations.
///
/// Every non-zero `s_perm` evaluation selects the matching `perm_hash`
/// evaluation. The accumulator therefore multiplies `(perm_hash + γ)` for
/// exactly those selected positions, using the committed LDE columns rather
/// than the unextended trace.
///
/// TODO: Before permission operations can enter the production semantic
/// profile, extend this commitment with a table-side product plus a running
/// product trace constrained at both boundaries and bind that table to the
/// permission root. This deterministic accumulator alone is not a membership
/// or non-membership proof.
///
/// # Errors
///
/// Returns [`Error::LookupColumnLengthMismatch`] when the columns have
/// different lengths, or [`Error::NonCanonicalGoldilocksElement`] when the
/// challenge or an evaluation is outside the canonical field range.
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
            context: "lookup_challenge",
            indices: Vec::new(),
        });
    }

    let mut accumulator = FIELD_ONE;
    for (index, (&selector, &witness)) in selector_values
        .iter()
        .zip(witness_values.iter())
        .enumerate()
    {
        if selector >= GOLDILOCKS_MODULUS {
            return Err(Error::NonCanonicalGoldilocksElement {
                context: "lookup_selector",
                indices: vec![index],
            });
        }
        if witness >= GOLDILOCKS_MODULUS {
            return Err(Error::NonCanonicalGoldilocksElement {
                context: "lookup_witness",
                indices: vec![index],
            });
        }
        if selector != 0 {
            accumulator = mul_mod(accumulator, add_mod(witness, gamma));
        }
    }
    Ok(accumulator)
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
    while current.len() > fastpq_isi::FASTPQ_FRI_TERMINAL_DOMAIN_SIZE_V1 as usize
        && round < max_rounds
    {
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
    if current.len() > fastpq_isi::FASTPQ_FRI_TERMINAL_DOMAIN_SIZE_V1 as usize {
        return Err(Error::FriReductionLimit {
            max_reductions,
            remaining: current.len(),
            arity,
        });
    }
    let final_leaves = hash_fri_terminal_leaves(round, &current)?;
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
    opening_trees: Option<fri_openings::FriOpeningTrees>,
}

impl FriOpeningLayers {
    /// Open this exact committed owner without rebuilding any FRI leaves or trees.
    fn open_query_chains(
        &mut self,
        query_indices: &[usize],
        arity: u32,
    ) -> Result<Vec<FriQueryOpening>> {
        let arity_usize = fri_chunk_size(arity)?;
        if let Some(trees) = &mut self.opening_trees {
            open_fri_query_chains_with_trees(&self.layer_values, query_indices, arity_usize, trees)
        } else {
            // Preserve the legacy empty-input branch and its terminal-shape
            // error, including when there are no queries. No valid nonempty
            // commitment takes this fallback.
            open_fri_query_chains(&self.layer_values, query_indices, arity, ExecutionMode::Cpu)
        }
    }
}

fn fold_with_fri_opening_layers(
    evaluations: &[GoldilocksFp4V1],
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
            opening_trees: None,
        });
    }
    let arity_usize = usize::try_from(arity).expect("FRI arity fits usize");
    let max_rounds =
        usize::try_from(params.fri.max_reductions).expect("FRI reduction bound fits usize");
    let mut current = evaluations.to_vec();
    let mut domain = FriDomain::from_lde_parameters(
        params.lde_root,
        params.lde_log_size,
        current.len(),
        params.omega_coset,
    )?;
    let mut layer_values = Vec::new();
    let mut roots = Vec::new();
    let mut opening_levels = Vec::new();
    let mut betas = Vec::new();
    let mut round = 0usize;
    while current.len() > fastpq_isi::FASTPQ_FRI_TERMINAL_DOMAIN_SIZE_V1 as usize
        && round < max_rounds
    {
        let leaves = hash_fri_leaves_with_mode(round, &current, arity, mode)?;
        let levels = build_merkle_levels_with_mode(
            &leaves,
            MerkleTreeRoleV1::Fri(
                u32::try_from(round).map_err(|_| Error::QueryIndexOverflow { index: round })?,
            ),
            mode,
        )?;
        let root = levels
            .last()
            .and_then(|level| level.first())
            .copied()
            .expect("nonempty FRI leaves produce one root");
        opening_levels.push(levels);
        drop(leaves); // The first retained level already owns these digests.
        transcript.append_fri_layer(round, root);
        roots.push(root);
        let beta = transcript.challenge_beta(round);
        betas.push(beta);
        let round_arity = fri_round_arity(current.len(), arity_usize)?;
        let next = fold_round(&current, arity_usize, beta, domain)?;
        layer_values.push(current);
        current = next;
        domain = domain.folded(round_arity);
        round += 1;
    }
    if current.len() > fastpq_isi::FASTPQ_FRI_TERMINAL_DOMAIN_SIZE_V1 as usize {
        return Err(Error::FriReductionLimit {
            max_reductions: params.fri.max_reductions,
            remaining: current.len(),
            arity: arity_usize,
        });
    }
    let leaves = hash_fri_terminal_leaves(round, &current)?;
    let levels = build_merkle_levels_with_mode(
        &leaves,
        MerkleTreeRoleV1::Fri(
            u32::try_from(round).map_err(|_| Error::QueryIndexOverflow { index: round })?,
        ),
        mode,
    )?;
    let final_root = levels
        .last()
        .and_then(|level| level.first())
        .copied()
        .expect("the complete terminal leaf produces one duplicated-node root");
    opening_levels.push(levels);
    drop(leaves);
    transcript.append_fri_final(final_root);
    roots.push(final_root);
    layer_values.push(current);
    Ok(FriOpeningLayers {
        layer_values,
        roots,
        betas,
        opening_trees: Some(fri_openings::FriOpeningTrees::from_levels(
            opening_levels,
            mode,
        )?),
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
    let prefix = digest_domain_prefix_v1(FRI_COMMITMENT_ROLE_V1, MERKLE_LEAF_PHASE_V1, round, 0)?;
    let hash_leaf = |leaf_index: usize| {
        let mut bytes = [0; 64];
        for position in 0..round_arity {
            bytes[position * 32..(position + 1) * 32]
                .copy_from_slice(&values[leaf_index + position * output_len].to_le_bytes());
        }
        hash_at_prefix_v1(&prefix, leaf_index, &[&bytes[..round_arity * 32]])
    };
    if output_len < 32 {
        return (0..output_len).map(hash_leaf).collect();
    }
    let results: Vec<Result<_>> = (0..output_len).into_par_iter().map(hash_leaf).collect();
    results.into_iter().collect()
}
// The complete terminal domain is a single ordered leaf. Binary strided leaves
// are only appropriate while another fold follows: a terminal subset would not
// suffice to interpolate and check the final polynomial's degree.
fn hash_fri_terminal_leaves(
    round: usize,
    values: &[GoldilocksFp4V1],
) -> Result<Vec<GoldilocksDigest384V1>> {
    let terminal_size = fastpq_isi::FASTPQ_FRI_TERMINAL_DOMAIN_SIZE_V1 as usize;
    if !values.len().is_power_of_two() || values.len() > terminal_size {
        return Err(Error::FriDomainSize {
            length: values.len(),
            arity: terminal_size,
        });
    }
    Ok(vec![hash_fri_chunk(round, 0, values)?])
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
        round_leaves.push(if round + 1 == layer_values.len() {
            hash_fri_terminal_leaves(round, values)?
        } else {
            hash_fri_leaves_with_mode(round, values, arity, mode)?
        });
    }
    // Retain each layer's read-only levels across all queries. Building lazily
    // preserves the existing coordinate checks and avoids work for no queries.
    let mut trees = fri_openings::FriOpeningTrees::new(round_leaves, mode);
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
            let path = trees.path(round, leaf_index)?;
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
                merkle_path: path
                    .into_iter()
                    .map(WireGoldilocksDigest384V1::from)
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
        let final_leaf_index = 0;
        let final_group = final_values.to_vec();
        let final_path = trees.path(layer_values.len() - 1, final_leaf_index)?;
        openings.push(FriQueryOpening {
            initial_index: initial_index_u32,
            rounds,
            final_index: u32::try_from(index).map_err(|_| Error::QueryIndexOverflow { index })?,
            final_values: final_group,
            final_merkle_path: final_path
                .into_iter()
                .map(WireGoldilocksDigest384V1::from)
                .collect(),
        });
    }
    Ok(openings)
}

fn open_fri_query_chains_with_trees(
    layer_values: &[Vec<GoldilocksFp4V1>],
    query_indices: &[usize],
    arity_usize: usize,
    trees: &mut fri_openings::FriOpeningTrees,
) -> Result<Vec<FriQueryOpening>> {
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
            let path = trees.path(round, leaf_index)?;
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
                merkle_path: path
                    .into_iter()
                    .map(WireGoldilocksDigest384V1::from)
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
        let final_leaf_index = 0;
        let final_group = final_values.to_vec();
        let final_path = trees.path(layer_values.len() - 1, final_leaf_index)?;
        openings.push(FriQueryOpening {
            initial_index: initial_index_u32,
            rounds,
            final_index: u32::try_from(index).map_err(|_| Error::QueryIndexOverflow { index })?,
            final_values: final_group,
            final_merkle_path: final_path
                .into_iter()
                .map(WireGoldilocksDigest384V1::from)
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
// Deterministic engineering work limits, not a completion-probability or
// cryptographic-security claim. They must not vary with operator configuration.
const QUERY_MIN_DIGEST_DRAWS: u32 = 64;
const QUERY_DIGEST_DRAWS_PER_INDEX: u32 = 8;

/// Sample sorted unique indices with a deterministic per-attempt draw budget.
///
/// Successful transcripts within the budget retain the existing tags, digest
/// lane order, rejection rule and exact transcript state. Each draw consumes six
/// canonical field candidates. Exhaustion returns no partial indices; callers
/// must discard the failed attempt instead of resuming the advanced transcript.
/// The finite budget bounds work and does not guarantee sampling completion.
///
/// # Errors
/// Returns an error for unsupported nonempty geometry, an unsupported requested
/// cardinality, an exhausted transcript counter, or an exhausted draw budget.
pub fn sample_queries(
    domain_size: usize,
    target: usize,
    transcript: &mut Transcript,
) -> Result<Vec<usize>> {
    sample_queries_from(domain_size, target, |counter| {
        // Preserve a successful final draw that advances MAX-1 to MAX while
        // preventing the existing challenge implementation's checked-add panic.
        if transcript.counter == u64::MAX {
            return Err(Error::QuerySamplingTranscriptCounterExhausted);
        }
        let tag = format!("{TRANSCRIPT_TAG_QUERY_INDEX}:{counter}");
        Ok(transcript.challenge_digest(&tag))
    })
}

fn sample_queries_from(
    domain_size: usize,
    target: usize,
    mut draw: impl FnMut(u32) -> Result<GoldilocksDigest384V1>,
) -> Result<Vec<usize>> {
    if domain_size == 0 || target == 0 {
        return Ok(Vec::new());
    }
    let domain = u64::try_from(domain_size)
        .map_err(|_| Error::QuerySamplingDomainUnsupported { domain_size })?;
    if domain > GOLDILOCKS_MODULUS {
        return Err(Error::QuerySamplingDomainUnsupported { domain_size });
    }
    let desired = target.min(domain_size);
    let max_queries = usize::try_from(fastpq_isi::FASTPQ_MAX_QUERY_COUNT_V1)
        .map_err(|_| Error::QuerySamplingDomainUnsupported { domain_size })?;
    if desired > max_queries {
        return Err(Error::VerifierLimitExceeded {
            limit: "max_sampled_queries",
            actual: desired,
            max: max_queries,
        });
    }
    let desired_u32 = u32::try_from(desired)
        .map_err(|_| Error::QuerySamplingDomainUnsupported { domain_size })?;
    let draw_limit = desired_u32
        .checked_mul(QUERY_DIGEST_DRAWS_PER_INDEX)
        .ok_or(Error::QuerySamplingDomainUnsupported { domain_size })?
        .max(QUERY_MIN_DIGEST_DRAWS);
    let rejection_limit = GOLDILOCKS_MODULUS - GOLDILOCKS_MODULUS % domain;
    let mut indices = BTreeSet::new();
    for counter in 0..draw_limit {
        let digest = draw(counter)?;
        for candidate in digest.words() {
            if indices.len() == desired {
                break;
            }
            if candidate >= rejection_limit {
                continue;
            }
            let index = usize::try_from(candidate % domain)
                .map_err(|_| Error::QuerySamplingDomainUnsupported { domain_size })?;
            indices.insert(index);
        }
        if indices.len() == desired {
            return Ok(indices.into_iter().collect());
        }
    }
    Err(Error::QuerySamplingExhausted {
        domain_size,
        requested: desired,
        selected: indices.len(),
        draws: draw_limit,
    })
}

pub fn open_queries<F: Copy>(evaluations: &[F], indices: &[usize]) -> Result<Vec<(u32, F)>> {
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
    transcript_trace_root: Option<GoldilocksDigest384V1>,
    poseidon_policy: PoseidonPipelinePolicy,
}

#[derive(Debug)]
struct PreparedBatch {
    trace_commitment: WireGoldilocksDigest384V1,
    trace_root: GoldilocksDigest384V1,
    air_trace_root: GoldilocksDigest384V1,
    air_composition_root: GoldilocksDigest384V1,
    lde_root: GoldilocksDigest384V1,
    lde_domain_size: u32,
    lookup_grand_product: u64,
    lookup_challenge: u64,
    alphas: Vec<GoldilocksFp4V1>,
    lde_columns: Vec<Vec<u64>>,
    lde_values: Vec<GoldilocksFp4V1>,
    lde_hashes: Vec<GoldilocksDigest384V1>,
    air_trace_leaves: Vec<GoldilocksDigest384V1>,
    air_composition_values: Vec<GoldilocksFp4V1>,
    air_composition_leaves: Vec<GoldilocksDigest384V1>,
    transcript: Transcript,
}

/// Batch-derived commitments which a proof is required to advertise exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchDerivedCommitments {
    /// Canonical commitment over the parameterised trace.
    pub trace_commitment: WireGoldilocksDigest384V1,
    /// Canonical base-trace commitment.
    pub trace_root: GoldilocksDigest384V1,
    /// Canonical commitment to the LDE trace rows used by AIR openings.
    pub air_trace_root: GoldilocksDigest384V1,
    /// Canonical commitment to the AIR composition evaluations.
    pub air_composition_root: GoldilocksDigest384V1,
    /// Canonical LDE commitment.
    pub lde_root: GoldilocksDigest384V1,
    /// Canonical LDE evaluation-domain size.
    pub lde_domain_size: u32,
    /// Canonical permission lookup grand-product accumulator.
    pub lookup_grand_product: u64,
    /// Canonical transcript-derived permission lookup challenge.
    pub lookup_challenge: u64,
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

fn combine_lde_columns_v1(
    columns: &[Vec<u64>],
    coefficients: &[GoldilocksFp4V1],
) -> Result<Vec<GoldilocksFp4V1>> {
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
            columns.iter().zip(coefficients).fold(
                GoldilocksFp4V1::ZERO,
                |accumulator, (column, coefficient)| {
                    accumulator.add(coefficient.mul_base(column[row]))
                },
            )
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
        transcript_trace_root,
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
    let air_layout = AirColumnLayout::from_names(&column_names)?;
    let planner = Planner::new(params);
    let poseidon_mode = ExecutionMode::Cpu;
    let polynomial_data = derive_polynomial_data(&trace, &planner);
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
    crate::trace::notify_native_stark_cpu_hashing(poseidon_policy);
    let trace_column_leaves = hash_trace_columns_v1(&column_names, &polynomial_data.coefficients)?;
    let derived_trace_root =
        merkle_root_with_mode(&trace_column_leaves, MerkleTreeRoleV1::Trace, poseidon_mode)?;
    let trace_commitment = crate::digest::trace_commitment_from_trace(params, &trace)?;
    let trace_root = transcript_trace_root.unwrap_or(derived_trace_root);
    let lde_columns = polynomial_data.into_lde_columns();
    let mut transcript = Transcript::initialise(
        public_io,
        params.name,
        protocol_version,
        TRANSCRIPT_TAG_INIT,
    )?;
    let air_trace_leaves = hash_air_trace_rows_with_mode(&lde_columns, poseidon_mode)?;
    let air_trace_root =
        merkle_root_with_mode(&air_trace_leaves, MerkleTreeRoleV1::AirTrace, poseidon_mode)?;
    let lde_domain_size = u32::try_from(lde_columns.first().map_or(0, Vec::len))
        .map_err(|_| Error::TraceLengthOverflow { rows: usize::MAX })?;
    transcript.append_trace_oracles(
        trace_root,
        air_trace_root,
        lde_domain_size,
        column_names.len(),
    )?;
    let column_mix = (0..lde_columns.len())
        .map(|index| {
            transcript.challenge_extension(&format!("{TRANSCRIPT_TAG_COLUMN_MIX_PREFIX}:{index}"))
        })
        .collect::<Vec<_>>();
    let lde_values = combine_lde_columns_v1(&lde_columns, &column_mix)?;
    let lde_hashes = hash_lde_leaves_fp4(&lde_values, params.fri.arity)?;
    let lde_root = merkle_root_with_mode(&lde_hashes, MerkleTreeRoleV1::Lde, poseidon_mode)?;

    transcript.append_message(
        TRANSCRIPT_TAG_ROOTS,
        &[lde_root.to_le_bytes(), trace_root.to_le_bytes()].concat(),
    );
    let lookup_challenge = transcript.challenge_field(TRANSCRIPT_TAG_GAMMA);
    let lookup_grand_product = compute_lookup_grand_product(
        &lde_columns[air_layout.s_perm],
        &lde_columns[air_layout.perm_hash],
        lookup_challenge,
    )?;
    let alpha_count = air_composition_alpha_count(&column_names);
    let mut alphas = Vec::with_capacity(alpha_count);
    for idx in 0..alpha_count {
        let tag = format!("{TRANSCRIPT_TAG_ALPHA_PREFIX}:{idx}");
        alphas.push(transcript.challenge_extension(&tag));
    }
    let air_composition_values = air_quotient_values(params, &column_names, &lde_columns, &alphas)?;
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
    Ok(PreparedBatch {
        trace_commitment,
        trace_root,
        air_trace_root,
        air_composition_root,
        lde_root,
        lde_domain_size,
        lookup_grand_product,
        lookup_challenge,
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

/// Recompute every proof-carried root and lookup value that is deterministic
/// from the batch.
pub fn derive_batch_commitments(
    params: &StarkParameterSet,
    batch: &TransitionBatch,
    public_io: &PublicIO,
    protocol_version: u16,
) -> Result<BatchDerivedCommitments> {
    let prepared = prepare_batch(&BatchPreparationContext {
        params,
        batch,
        public_io,
        protocol_version,
        transcript_trace_root: None,
        poseidon_policy: PoseidonPipelinePolicy::for_mode(ExecutionMode::Cpu),
    })?;
    Ok(BatchDerivedCommitments {
        trace_commitment: prepared.trace_commitment,
        trace_root: prepared.trace_root,
        air_trace_root: prepared.air_trace_root,
        air_composition_root: prepared.air_composition_root,
        lde_root: prepared.lde_root,
        lde_domain_size: prepared.lde_domain_size,
        lookup_grand_product: prepared.lookup_grand_product,
        lookup_challenge: prepared.lookup_challenge,
    })
}

impl StarkBackend {
    pub(crate) fn prove(
        &self,
        batch: &TransitionBatch,
        public_io: &PublicIO,
        protocol_version: u16,
    ) -> Result<BackendArtifact> {
        self.prove_inner(batch, public_io, protocol_version, None)
    }

    #[allow(clippy::too_many_lines)]
    fn prove_inner(
        &self,
        batch: &TransitionBatch,
        public_io: &PublicIO,
        protocol_version: u16,
        transcript_trace_root: Option<GoldilocksDigest384V1>,
    ) -> Result<BackendArtifact> {
        let execution_mode = self.config.resolve_native_v1_execution_mode()?;
        let poseidon_policy =
            PoseidonPipelinePolicy::new(self.config.poseidon_mode(), execution_mode);
        // Native Digest384 commitments and opening paths currently run on CPU.
        // prepare_batch reports this actual route with the requested policy.
        let poseidon_mode = ExecutionMode::Cpu;
        let PreparedBatch {
            trace_commitment,
            trace_root,
            air_trace_root,
            air_composition_root,
            lde_root,
            lde_domain_size,
            lookup_grand_product,
            lookup_challenge,
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
            transcript_trace_root,
            poseidon_policy,
        })?;
        let next_step = usize::try_from(self.config.params.fri.blowup_factor)
            .expect("FRI blowup factor fits usize")
            .max(1);
        let joint_fri =
            JointFriBatch::from_transcript(&self.config.params, lde_values.len(), &mut transcript)?;
        let joint_values = joint_fri.values(&air_composition_values, &lde_values)?;
        let mut fri = fold_with_fri_opening_layers(
            &joint_values,
            &self.config.params,
            &mut transcript,
            poseidon_mode,
        )?;
        drop(joint_values); // The retained FRI owner has its own first layer.
        let query_indices = sample_queries(
            lde_values.len(),
            usize::try_from(self.config.params.fri.queries).expect("query count fits usize"),
            &mut transcript,
        )?;
        let query_openings = open_queries(&lde_values, &query_indices)?;
        let query_chunks =
            open_query_chunks(&lde_values, &query_indices, self.config.params.fri.arity)?;
        let query_paths = merkle_paths_for_queries_with_mode(
            &lde_hashes,
            &query_indices,
            self.config.params.fri.arity,
            lde_values.len(),
            MerkleTreeRoleV1::Lde,
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
        let fri_query_openings =
            fri.open_query_chains(&query_indices, self.config.params.fri.arity)?;
        let FriOpeningLayers {
            roots: fri_layers,
            betas: fri_betas,
            ..
        } = fri;
        Ok(BackendArtifact {
            parameter: self.config.params.name.to_string(),
            trace_commitment,
            trace_root,
            air_trace_root,
            air_composition_root,
            lde_root,
            lde_domain_size,
            lookup_grand_product,
            lookup_challenge,
            alphas,
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
    pub(crate) fn prove_with_transcript_trace_root(
        &self,
        batch: &TransitionBatch,
        public_io: &PublicIO,
        protocol_version: u16,
        trace_root: GoldilocksDigest384V1,
    ) -> Result<BackendArtifact> {
        self.prove_inner(batch, public_io, protocol_version, Some(trace_root))
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
        tag: &str,
    ) -> Result<Self> {
        // This first-release schema binds the quotient and exact-u64 AIR layout,
        // Fp4 aggregation, joint trace degree check and complete terminal opening. It is not a security-review
        // or production-qualification identifier.
        const SCHEMA: &str = "fastpq:v1:balance-key-v1:six-operation-air:air-quotient-exact-u64:fp4-oracles:joint-trace-degree:fri-terminal4";
        let params = fastpq_isi::FASTPQ_FINAL_V1;
        let field_and_hash = (
            params.field.name,
            params.field.modulus_decimal,
            params.field.extension_degree,
            params.field.extension_polynomial,
            params.hash.trace_commitment,
            params.hash.transcript,
            params.hash.digest_bytes,
        );
        let polynomial_profile = (
            params.trace_log_size,
            params.trace_root,
            params.lde_log_size,
            params.lde_root,
            params.omega_coset,
            params.fri.arity,
            params.fri.blowup_factor,
            params.fri.max_reductions,
            params.fri.queries,
            fastpq_isi::FASTPQ_COMPOSITION_DEGREE_EXPANSION_V1,
            fastpq_isi::FASTPQ_FRI_TERMINAL_DOMAIN_SIZE_V1,
        );
        // Transcript bytes cannot inherit an unrelated caller's Norito layout.
        let _canonical_flags =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let payload = norito::core::to_bytes(&(
            protocol_version,
            parameter,
            SCHEMA,
            params.name,
            params.grinding_bits,
            field_and_hash,
            polynomial_profile,
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
    /// Bind both trace commitments and their exact geometry before aggregation challenges.
    pub(crate) fn append_trace_oracles(
        &mut self,
        trace_root: GoldilocksDigest384V1,
        air_trace_root: GoldilocksDigest384V1,
        domain_size: u32,
        columns: usize,
    ) -> Result<()> {
        let columns =
            u32::try_from(columns).map_err(|_| Error::TraceLengthOverflow { rows: columns })?;
        // Transcript bytes cannot inherit an unrelated caller's Norito layout.
        let _canonical_flags =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let payload = norito::core::to_bytes(&(
            trace_root.to_le_bytes(),
            air_trace_root.to_le_bytes(),
            domain_size,
            columns,
        ))?;
        self.append_message(TRANSCRIPT_TAG_TRACE_ROOT, &payload);
        Ok(())
    }
    /// Sample one full extension-field aggregation coefficient.
    pub(crate) fn challenge_extension(&mut self, tag: &str) -> GoldilocksFp4V1 {
        GoldilocksFp4V1::from_digest(self.challenge_digest(tag))
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
#[inline]
fn add_mod(a: u64, b: u64) -> u64 {
    crate::field::add_base(a, b)
}
#[inline]
fn sub_mod(a: u64, b: u64) -> u64 {
    crate::field::sub_base(a, b)
}
#[inline]
fn mul_mod(a: u64, b: u64) -> u64 {
    crate::field::mul_base(a, b)
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

    fn lde_merkle_root(leaves: &[GoldilocksDigest384V1]) -> GoldilocksDigest384V1 {
        merkle_root_with_mode(leaves, MerkleTreeRoleV1::Lde, ExecutionMode::Cpu)
            .expect("typed LDE Merkle root")
    }
    fn sample_batch(rows: usize) -> TransitionBatch {
        let mut batch =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
        for idx in 0..rows {
            let key = format!("asset/xor/account-{idx:04}").into_bytes();
            let idx_u64 = u64::try_from(idx).expect("sample batch index fits u64");
            let pre = idx_u64.to_le_bytes().to_vec();
            let op = OperationKind::MetaSet;
            let post_value = idx_u64.wrapping_add(1);
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
    fn transcript_encoding_ignores_and_restores_ambient_norito_layout() {
        let params = fastpq_isi::FASTPQ_FINAL_V1;
        let public_io = PublicIO {
            slot: 0x1234_5678_90ab_cdef,
            ..PublicIO::default()
        };
        let baseline =
            Transcript::initialise(&public_io, params.name, 1, TRANSCRIPT_TAG_INIT).unwrap();
        let trace_root = GoldilocksDigest384V1::new([1, 2, 3, 4, 5, 6]).unwrap();
        let air_root = GoldilocksDigest384V1::new([7, 8, 9, 10, 11, 12]).unwrap();
        let mut expected = baseline.clone();
        expected
            .append_trace_oracles(trace_root, air_root, 64, 17)
            .unwrap();
        let expected_challenge = expected.challenge_extension(TRANSCRIPT_TAG_COLUMN_MIX_PREFIX);
        let probe = ("layout restoration", vec![1_u64, 2, 3]);
        let canonical_probe = norito::core::to_bytes(&probe).unwrap();
        for flags in [
            0,
            norito::core::header_flags::PACKED_SEQ,
            norito::core::header_flags::PACKED_STRUCT | norito::core::header_flags::COMPACT_LEN,
        ] {
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            let before = norito::core::to_bytes(&probe).unwrap();
            let actual =
                Transcript::initialise(&public_io, params.name, 1, TRANSCRIPT_TAG_INIT).unwrap();
            assert_eq!(actual.state, baseline.state);
            assert_eq!(actual.counter, baseline.counter);
            let mut appended = baseline.clone();
            appended
                .append_trace_oracles(trace_root, air_root, 64, 17)
                .unwrap();
            assert_eq!(
                appended.challenge_extension(TRANSCRIPT_TAG_COLUMN_MIX_PREFIX),
                expected_challenge
            );
            assert_eq!(
                norito::core::to_bytes(&probe).unwrap(),
                before,
                "transcript helpers must restore the caller's layout flags"
            );
            if flags == 0 {
                assert_ne!(
                    before, canonical_probe,
                    "the control must exercise an alternate layout"
                );
            }
        }
    }

    #[test]
    fn aggregation_challenges_bind_both_trace_roots_and_geometry() {
        let challenge = |trace, air_trace, domain_size, columns| {
            let mut transcript = Transcript::initialise(
                &PublicIO::default(),
                fastpq_isi::FASTPQ_FINAL_V1.name,
                1,
                TRANSCRIPT_TAG_INIT,
            )
            .unwrap();
            transcript
                .append_trace_oracles(trace, air_trace, domain_size, columns)
                .unwrap();
            transcript.challenge_extension(TRANSCRIPT_TAG_COLUMN_MIX_PREFIX)
        };
        let digest = |word| GoldilocksDigest384V1::new([word; 6]).unwrap();
        let baseline = challenge(digest(1), digest(2), 64, 10);
        assert_eq!(baseline, challenge(digest(1), digest(2), 64, 10));
        assert_ne!(baseline, challenge(digest(3), digest(2), 64, 10));
        assert_ne!(baseline, challenge(digest(1), digest(3), 64, 10));
        assert_ne!(baseline, challenge(digest(1), digest(2), 128, 10));
        assert_ne!(baseline, challenge(digest(1), digest(2), 64, 11));
        assert!(baseline.coefficients()[1..].iter().any(|value| *value != 0));
    }

    #[test]
    fn mixed_trace_uses_all_extension_coefficients_and_checks_column_shape() {
        let columns = vec![vec![2, 3], vec![5, 7]];
        let coefficients = [
            GoldilocksFp4V1::new([11, 13, 17, 19]).unwrap(),
            GoldilocksFp4V1::new([23, 29, 31, 37]).unwrap(),
        ];
        let actual = combine_lde_columns_v1(&columns, &coefficients).unwrap();
        for index in 0..2 {
            let expected = core::array::from_fn(|lane| {
                add_mod(
                    mul_mod(columns[0][index], coefficients[0].coefficients()[lane]),
                    mul_mod(columns[1][index], coefficients[1].coefficients()[lane]),
                )
            });
            assert_eq!(actual[index].coefficients(), expected);
        }
        assert!(combine_lde_columns_v1(&[], &[]).is_err());
        assert!(combine_lde_columns_v1(&columns, &coefficients[..1]).is_err());
        assert!(combine_lde_columns_v1(&[vec![1], vec![2, 3]], &coefficients).is_err());
    }

    #[test]
    fn extension_oracle_hashes_bind_every_coefficient_and_reject_noncanonical_values() {
        let value = GoldilocksFp4V1::new([1, 2, 3, 4]).unwrap();
        let leaf = hash_lde_chunk_fp4(0, &[value]).unwrap();
        assert_eq!(hash_lde_leaves_fp4(&[value], 2).unwrap(), vec![leaf]);
        assert_ne!(leaf, hash_lde_chunk_fp4(1, &[value]).unwrap());
        assert_ne!(leaf, hash_air_composition_leaf(0, value).unwrap());
        for lane in 0..4 {
            let mut coefficients = value.coefficients();
            coefficients[lane] += 1;
            assert_ne!(
                leaf,
                hash_lde_chunk_fp4(0, &[GoldilocksFp4V1::new(coefficients).unwrap()]).unwrap()
            );
            coefficients[lane] = GOLDILOCKS_MODULUS;
            let malformed = GoldilocksFp4V1::from_coefficients_unchecked_for_test(coefficients);
            assert!(matches!(
                hash_lde_chunk_fp4(0, &[malformed]),
                Err(Error::NonCanonicalGoldilocksElement { .. })
            ));
            assert!(hash_air_composition_leaf(0, malformed).is_err());
        }
        assert!(hash_lde_leaves_fp4(&[value], 8).is_err());
    }

    #[test]
    fn air_row_hash_batches_match_scalar_rows_indices_and_domains() {
        for row_count in [1, 31, 32, 33, 65] {
            let columns: Vec<Vec<u64>> = (0..5)
                .map(|column| {
                    (0..row_count)
                        .map(|row| {
                            if column == 0 && row % 7 == 0 {
                                GOLDILOCKS_MODULUS - 1
                            } else {
                                (column * 100 + row) as u64
                            }
                        })
                        .collect()
                })
                .collect();
            let expected: Vec<_> = (0..row_count)
                .map(|index| {
                    let row: Vec<_> = columns.iter().map(|column| column[index]).collect();
                    let digest = hash_air_trace_row(index, &row).unwrap();
                    assert_ne!(digest, hash_air_trace_row(index + 1, &row).unwrap());
                    assert_ne!(digest, hash_lde_chunk(index, &row).unwrap());
                    digest
                })
                .collect();
            for mode in [ExecutionMode::Cpu, ExecutionMode::Gpu] {
                assert_eq!(
                    hash_air_trace_rows_with_mode(&columns, mode).unwrap(),
                    expected,
                    "row count {row_count}, requested mode {mode:?}"
                );
            }
        }
    }

    #[test]
    fn air_row_hash_batches_check_shapes_before_hashing() {
        for mode in [ExecutionMode::Cpu, ExecutionMode::Gpu] {
            assert!(hash_air_trace_rows_with_mode(&[], mode).unwrap().is_empty());
            assert!(
                hash_air_trace_rows_with_mode(&[vec![], vec![]], mode)
                    .unwrap()
                    .is_empty()
            );
            for columns in [
                vec![vec![GOLDILOCKS_MODULUS], vec![]],
                vec![vec![], vec![GOLDILOCKS_MODULUS]],
                vec![vec![0; 64], vec![GOLDILOCKS_MODULUS; 63]],
            ] {
                assert!(matches!(
                    hash_air_trace_rows_with_mode(&columns, mode),
                    Err(Error::AirOpeningMismatch { index: 0 })
                ));
            }
        }
    }

    #[test]
    fn air_row_hash_batches_return_lowest_error_row_across_worker_schedules() {
        let mut columns = vec![vec![7; 65], vec![11; 65], vec![13; 65]];
        columns[2][17] = GOLDILOCKS_MODULUS;
        columns[0][31] = u64::MAX;
        columns[1][64] = GOLDILOCKS_MODULUS;
        let sequential: Vec<_> = columns.iter().map(|column| column[..31].to_vec()).collect();
        assert!(matches!(
            hash_air_trace_rows_with_mode(&sequential, ExecutionMode::Cpu),
            Err(Error::NonCanonicalGoldilocksElement {
                context: "native_stark_digest_input",
                indices,
            }) if indices == vec![17]
        ));
        for workers in [1, 2, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .expect("bounded hash regression worker pool");
            for iteration in 0..4 {
                let result =
                    pool.install(|| hash_air_trace_rows_with_mode(&columns, ExecutionMode::Cpu));
                assert!(
                    matches!(result, Err(Error::NonCanonicalGoldilocksElement {
                        context: "native_stark_digest_input",
                        indices,
                    }) if indices == vec![17]),
                    "lowest row must win with {workers} workers on iteration {iteration}"
                );
            }
        }
    }

    #[test]
    fn air_row_prefix_batches_preserve_wide_rows_across_worker_counts() {
        let pools = [1, 2, 4].map(|workers| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .expect("bounded AIR row prefix worker pool")
        });
        for width in [1, 7, 342] {
            for row_count in [1, 15, 16, 31, 32, 33, 65] {
                let columns: Vec<Vec<u64>> = (0..width)
                    .map(|column| {
                        (0..row_count)
                            .map(|row| match (column + row) % 7 {
                                0 => 0,
                                1 => GOLDILOCKS_MODULUS - 1,
                                2 => GOLDILOCKS_MODULUS - 2,
                                _ => (column * 71 + row * 37) as u64,
                            })
                            .collect()
                    })
                    .collect();
                let expected: Vec<_> = (0..row_count)
                    .map(|index| {
                        let row: Vec<_> = columns.iter().map(|column| column[index]).collect();
                        hash_air_trace_row(index, &row).unwrap()
                    })
                    .collect();
                for pool in &pools {
                    for mode in [ExecutionMode::Cpu, ExecutionMode::Gpu] {
                        assert_eq!(
                            pool.install(|| hash_air_trace_rows_with_mode(&columns, mode))
                                .unwrap(),
                            expected,
                            "width {width}, rows {row_count}, workers {}, requested mode {mode:?}",
                            pool.current_num_threads()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn air_row_prefix_batches_reject_every_noncanonical_wide_column_like_scalar() {
        let mut columns = vec![vec![7]; 342];
        let mut row = vec![7; 342];
        for column in 0..columns.len() {
            for invalid in [GOLDILOCKS_MODULUS, u64::MAX] {
                columns[column][0] = invalid;
                row[column] = invalid;
                for result in [
                    hash_air_trace_row(0, &row),
                    hash_air_trace_rows_with_mode(&columns, ExecutionMode::Cpu)
                        .map(|leaves| leaves[0]),
                ] {
                    assert!(matches!(
                        result,
                        Err(Error::NonCanonicalGoldilocksElement {
                            context: "native_stark_digest_input",
                            indices,
                        }) if indices == vec![0]
                    ));
                }
            }
            columns[column][0] = 7;
            row[column] = 7;
        }
        assert_eq!(
            hash_air_trace_rows_with_mode(&columns, ExecutionMode::Cpu).unwrap(),
            vec![hash_air_trace_row(0, &row).unwrap()]
        );
    }

    #[test]
    #[ignore = "bounded CPU timing diagnostic; no production speed assertion"]
    fn air_row_prefix_microdiagnostic() {
        const ROWS: usize = 512;
        const WIDTH: usize = 342;
        let columns: Vec<Vec<u64>> = (0..WIDTH)
            .map(|column| {
                (0..ROWS)
                    .map(|row| (column * 71 + row * 37) as u64)
                    .collect()
            })
            .collect();
        let mut row = vec![0; WIDTH];
        let started = std::time::Instant::now();
        let expected: Vec<_> = (0..ROWS)
            .map(|index| {
                for (value, column) in row.iter_mut().zip(&columns) {
                    *value = column[index];
                }
                hash_air_trace_row(index, &row).unwrap()
            })
            .collect();
        let canonical_elapsed = started.elapsed();
        for workers in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .expect("bounded AIR row prefix diagnostic pool");
            let started = std::time::Instant::now();
            let actual = pool
                .install(|| hash_air_trace_rows_with_mode(&columns, ExecutionMode::Cpu))
                .unwrap();
            let prefix_elapsed = started.elapsed();
            assert_eq!(actual, expected);
            eprintln!(
                "air_row_prefix_rows={ROWS}; width={WIDTH}; workers={workers}; canonical_scalar={canonical_elapsed:?}; prefix_batch={prefix_elapsed:?}; canonical_leaf_parity=true"
            );
        }
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
    fn merkle_paths_verify_against_lde_root_for_single_leaf() {
        let evaluations = vec![42u64];
        let leaves = hash_lde_leaves(&evaluations, 2).expect("hash leaves");
        let root = lde_merkle_root(&leaves);
        let paths =
            merkle_paths_for_queries(&leaves, &[0], 2, evaluations.len()).expect("query path");
        let chunks = open_query_chunks(&evaluations, &[0], 2).expect("query chunk");
        let leaf = hash_lde_chunk(0, &chunks[0]).expect("leaf hash");
        assert!(verify_merkle_path(root, leaf, 0, &paths[0]).expect("path verifies"));
        assert!(!verify_merkle_path(leaf, leaf, 0, &[]).expect("empty path is noncanonical"));
    }
    #[test]
    fn merkle_paths_verify_against_lde_root_for_odd_leaf_count() {
        let chunk_size = lde_chunk_size(2).expect("binary FRI chunk size");
        let evaluations = (0..(chunk_size * 3 - 1))
            .map(|idx| u64::try_from(idx).expect("index fits u64"))
            .collect::<Vec<_>>();
        let query_index = evaluations.len() - 1;
        let leaf_index = query_index / chunk_size;
        let leaves = hash_lde_leaves(&evaluations, 2).expect("hash leaves");
        let root = lde_merkle_root(&leaves);
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
        let root = lde_merkle_root(&leaves);
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
    fn integer_air_binding_rejects_lengths_auxiliary_and_trailing_limb_mutations() {
        let mut trace = build_trace(&sample_batch(2)).unwrap();
        let witness = transfer_integer_air::TransferIntegerWitness::from_balances(1, 2);
        for (name, value) in transfer_integer_air::auxiliary_column_names()
            .into_iter()
            .zip(witness.auxiliary_values())
        {
            trace.columns.push(crate::TraceColumn {
                name,
                values: vec![value, 0],
            });
        }
        trace.columns.push(crate::TraceColumn {
            name: "value_old_limb_2".into(),
            values: vec![0, 0],
        });
        for (name, value) in [
            ("s_transfer", 1),
            ("s_meta_set", 0),
            ("value_old_limb_0", 1),
            ("value_new_limb_0", 2),
            ("delta", 1),
        ] {
            trace
                .columns
                .iter_mut()
                .find(|column| column.name == name)
                .unwrap()
                .values[0] = value;
        }
        ensure_base_trace_constraints(&trace).expect("exact integer trace relation");
        let names: Vec<_> = trace
            .columns
            .iter()
            .map(|column| column.name.clone())
            .collect();
        assert_eq!(
            air_composition_alpha_count(&names),
            AIR_COMPOSITION_ALPHA_COUNT + 1
        );
        for (name, bad_value) in [
            ("value_old_len", 7),
            ("value_new_len", 9),
            ("value_old_limb_2", 1),
            ("transfer_old_bit_0", 2),
            ("transfer_carry_32", 1),
            ("transfer_is_debit", 2),
        ] {
            let mut invalid = trace.clone();
            invalid
                .columns
                .iter_mut()
                .find(|column| column.name == name)
                .unwrap()
                .values[0] = bad_value;
            assert!(
                matches!(
                    ensure_base_trace_constraints(&invalid),
                    Err(Error::AirConstraintMismatch { index: 0 })
                ),
                "{name}"
            );
        }
        trace
            .columns
            .retain(|column| column.name != "transfer_old_bit_0");
        assert!(
            matches!(ensure_base_trace_constraints(&trace), Err(Error::MissingColumn(name)) if name == "transfer_old_bit_0")
        );
    }

    #[test]
    fn extension_quotient_combination_matches_each_base_coefficient() {
        let params = fastpq_isi::FASTPQ_FINAL_V1;
        let trace = build_trace(&sample_batch(3)).unwrap();
        let names = trace
            .columns
            .iter()
            .map(|column| column.name.clone())
            .collect::<Vec<_>>();
        let mut columns = derive_polynomial_data(&trace, &Planner::new(&params)).into_lde_columns();
        // Activate multiple otherwise valid residues at one coset point, so this
        // comparison cannot pass merely because every tested quotient is zero.
        columns[0][3] = add_mod(columns[0][3], 7);
        let alphas = (0..air_composition_alpha_count(&names))
            .map(|index| {
                GoldilocksFp4V1::new(core::array::from_fn(|lane| 1 + (index * 4 + lane) as u64))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let combined = air_quotient_values(&params, &names, &columns, &alphas).unwrap();
        let domain = AirQuotientDomain::new(&params, columns[0].len()).unwrap();
        for lane in 0..4 {
            let base_alphas = alphas
                .iter()
                .map(|alpha| alpha.coefficients()[lane])
                .collect::<Vec<_>>();
            let base_values = air_quotient_values(&params, &names, &columns, &base_alphas).unwrap();
            assert!(base_values.iter().any(|value| *value != 0));
            for (index, &value) in combined.iter().enumerate() {
                assert_eq!(value.coefficients()[lane], base_values[index]);
                let next = (index + params.fri.blowup_factor as usize) % combined.len();
                assert_eq!(
                    value,
                    air_quotient_value_for_rows(
                        &names,
                        &air_row_at(&columns, index).unwrap(),
                        &air_row_at(&columns, next).unwrap(),
                        &alphas,
                        domain.weights_at(index).unwrap()
                    )
                    .unwrap()
                );
            }
        }
    }

    #[test]
    fn quotient_composition_matches_sampled_rows_and_excludes_only_the_final_transition() {
        let params = fastpq_isi::FASTPQ_FINAL_V1;
        let trace = build_trace(&sample_batch(3)).expect("padded trace");
        assert_eq!(trace.padded_len, 4);
        let names: Vec<_> = trace
            .columns
            .iter()
            .map(|column| column.name.clone())
            .collect();
        let columns = derive_polynomial_data(&trace, &Planner::new(&params)).into_lde_columns();
        let mut alphas = vec![0; AIR_COMPOSITION_ALPHA_COUNT];
        alphas[AIR_BOOLEAN_RESIDUE_COUNT + 2] = 1;
        let domain = AirQuotientDomain::new(&params, columns[0].len()).expect("disjoint coset");
        let values = air_quotient_values(&params, &names, &columns, &alphas).expect("quotients");
        let next_step = params.fri.blowup_factor as usize;
        for (index, value) in values.iter().enumerate() {
            let next = (index + next_step) % values.len();
            assert_eq!(
                *value,
                air_quotient_value_for_rows(
                    &names,
                    &air_row_at(&columns, index).unwrap(),
                    &air_row_at(&columns, next).unwrap(),
                    &alphas,
                    domain.weights_at(index).unwrap(),
                )
                .unwrap()
            );
        }
        let fri_domain = FriDomain::from_lde_parameters(
            params.lde_root,
            params.lde_log_size,
            values.len(),
            params.omega_coset,
        )
        .unwrap();
        let embedded: Vec<_> = values
            .into_iter()
            .map(|value| GoldilocksFp4V1::from_base(value).unwrap())
            .collect();
        assert!(
            fri_domain
                .evaluations_have_degree_below(&embedded, trace.padded_len)
                .unwrap()
        );

        // The padded final row is excluded, but an inactive interior row followed
        // by an active row must still produce a quotient above the allowed degree.
        let mut invalid = trace.clone();
        invalid
            .columns
            .iter_mut()
            .find(|column| column.name == "s_active")
            .unwrap()
            .values[1] = 0;
        let columns = derive_polynomial_data(&invalid, &Planner::new(&params)).into_lde_columns();
        let values = air_quotient_values(&params, &names, &columns, &alphas).unwrap();
        let embedded: Vec<_> = values
            .into_iter()
            .map(|value| GoldilocksFp4V1::from_base(value).unwrap())
            .collect();
        assert!(
            !fri_domain
                .evaluations_have_degree_below(&embedded, 2 * trace.padded_len)
                .unwrap()
        );
    }

    #[test]
    fn all_row_quotient_rejects_non_boolean_selector_degree() {
        let params = fastpq_isi::FASTPQ_FINAL_V1;
        let mut trace = build_trace(&sample_batch(3)).unwrap();
        let names: Vec<_> = trace
            .columns
            .iter()
            .map(|column| column.name.clone())
            .collect();
        let mut alphas = vec![0; AIR_COMPOSITION_ALPHA_COUNT];
        alphas[0] = 1;
        for valid in [true, false] {
            if !valid {
                trace
                    .columns
                    .iter_mut()
                    .find(|column| column.name == "s_active")
                    .unwrap()
                    .values[0] = 2;
            }
            let columns = derive_polynomial_data(&trace, &Planner::new(&params)).into_lde_columns();
            let values = air_quotient_values(&params, &names, &columns, &alphas).unwrap();
            let domain = FriDomain::from_lde_parameters(
                params.lde_root,
                params.lde_log_size,
                values.len(),
                params.omega_coset,
            )
            .unwrap();
            let values: Vec<_> = values
                .into_iter()
                .map(|value| GoldilocksFp4V1::from_base(value).unwrap())
                .collect();
            assert_eq!(
                domain
                    .evaluations_have_degree_below(&values, 2 * trace.padded_len)
                    .unwrap(),
                valid
            );
        }
    }

    #[test]
    fn air_composition_columnar_pass_matches_row_helper() {
        let trace = build_trace(&sample_batch(5)).expect("trace");
        let column_names = trace
            .columns
            .iter()
            .map(|column| column.name.clone())
            .collect::<Vec<_>>();
        let mut columns = trace
            .columns
            .iter()
            .map(|column| column.values.clone())
            .collect::<Vec<_>>();
        let active = column_names
            .iter()
            .position(|column| column == "s_active")
            .expect("active selector column");
        let metadata = column_names
            .iter()
            .position(|column| column == "metadata_hash_limb_3")
            .expect("metadata commitment column");
        columns[active][0] = 2;
        columns[metadata][1] = add_mod(columns[metadata][1], FIELD_ONE);
        let alphas = (1..=AIR_COMPOSITION_ALPHA_COUNT)
            .map(|alpha| u64::try_from(alpha).expect("AIR challenge index fits u64"))
            .collect::<Vec<_>>();
        let next_step = 1;
        let expected = (0..trace.padded_len)
            .map(|row_index| {
                let current = air_row_at(&columns, row_index)?;
                let next = air_row_at(&columns, (row_index + next_step) % trace.padded_len)?;
                air_composition_value_for_rows(&column_names, &current, &next, &alphas)
            })
            .collect::<Result<Vec<_>>>()
            .expect("row-wise AIR composition");

        assert_eq!(
            air_composition_values(&column_names, &columns, &alphas, next_step)
                .expect("columnar AIR composition"),
            expected
        );
        assert!(expected.iter().any(|value| *value != 0));

        let large_step = usize::MAX;
        assert_eq!(
            air_composition_values(&column_names, &columns, &alphas, large_step)
                .expect("large next-step offset"),
            air_composition_values(
                &column_names,
                &columns,
                &alphas,
                large_step % trace.padded_len,
            )
            .expect("reduced next-step offset")
        );
    }
    #[test]
    fn air_column_layout_preserves_schema_error_order() {
        let trace = build_trace(&sample_batch(2)).expect("trace");
        let column_names = trace
            .columns
            .iter()
            .map(|column| column.name.clone())
            .collect::<Vec<_>>();
        let columns = trace
            .columns
            .iter()
            .map(|column| column.values.clone())
            .collect::<Vec<_>>();
        let alphas = vec![FIELD_ONE; AIR_COMPOSITION_ALPHA_COUNT];
        let missing_index = column_names
            .iter()
            .position(|column| column == "s_transfer")
            .expect("transfer selector column");
        let mut missing_names = column_names.clone();
        missing_names.remove(missing_index);
        let mut missing_columns = columns.clone();
        missing_columns.remove(missing_index);

        let err = air_composition_values(&missing_names, &missing_columns, &alphas, 1)
            .expect_err("missing selector must reject the schema");
        assert!(matches!(err, Error::MissingColumn(name) if name == "s_transfer"));

        let err = air_composition_values(&missing_names, &missing_columns, &[], 1)
            .expect_err("challenge validation precedes schema validation");
        assert!(matches!(
            err,
            Error::AirChallengeCountMismatch {
                expected: AIR_COMPOSITION_ALPHA_COUNT,
                actual: 0
            }
        ));

        let mut short_names = column_names.clone();
        short_names.pop();
        let err = air_composition_values(&short_names, &columns, &alphas, 1)
            .expect_err("row width validation precedes schema validation");
        assert!(matches!(
            err,
            Error::AirOpeningMismatch { index } if index == columns.len()
        ));

        let mut missing_trace = trace;
        missing_trace.columns.remove(missing_index);
        let err = ensure_base_trace_constraints(&missing_trace)
            .expect_err("base trace pass must validate its layout");
        assert!(matches!(err, Error::MissingColumn(name) if name == "s_transfer"));

        assert!(
            air_composition_values(&[], &[Vec::new()], &[], 0)
                .expect("empty domains retain their validation behavior")
                .is_empty()
        );
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
            OperationKind::MetaSet,
        ));
        let mut trace = build_trace(&batch).expect("trace");
        trace
            .columns
            .iter_mut()
            .find(|column| column.name == "s_meta_set")
            .expect("metadata selector")
            .values[0] = 0;
        trace
            .columns
            .iter_mut()
            .find(|column| column.name == "s_transfer")
            .expect("transfer selector")
            .values[0] = 1;
        trace
            .columns
            .iter_mut()
            .find(|column| column.name == "delta")
            .expect("delta column")
            .values[0] = sub_mod(after, before);
        let witness = transfer_integer_air::TransferIntegerWitness::from_balances(before, after);
        for (name, value) in transfer_integer_air::auxiliary_column_names()
            .into_iter()
            .zip(witness.auxiliary_values())
        {
            trace.columns.push(crate::TraceColumn {
                name,
                values: vec![value],
            });
        }
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
    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    #[test]
    #[ignore = "requires actual Metal execution; no device skip is accepted"]
    fn native_merkle_metal_levels_roots_and_chunk_boundaries_match_cpu() {
        use crate::digest_executor::{
            DigestExecutionV1, execute_bounded_digest384_frames_v1, execute_digest384_frames_v1,
        };
        let device = DigestExecutionV1::Device(crate::digest384_gpu::Digest384GpuBackendV1::Metal);
        for role in [
            MerkleTreeRoleV1::Trace,
            MerkleTreeRoleV1::Lde,
            MerkleTreeRoleV1::AirTrace,
            MerkleTreeRoleV1::AirComposition,
            MerkleTreeRoleV1::Fri(0),
            MerkleTreeRoleV1::Fri(7),
        ] {
            for len in [0, 1, 3, 5, 17] {
                let leaves: Vec<_> = (0..len)
                    .map(|i| GoldilocksDigest384V1::new([i; 6]).unwrap())
                    .collect();
                let cpu =
                    build_merkle_levels_with_execution_v1(&leaves, role, DigestExecutionV1::Cpu)
                        .unwrap();
                let mut dispatch_sizes = Vec::new();
                let metal = build_merkle_levels_with_executor_v1(&leaves, role, &mut |frames| {
                    execute_bounded_digest384_frames_v1(
                        frames,
                        2,
                        frames[0].word_count() * 2,
                        &mut |chunk| {
                            dispatch_sizes.push(chunk.len());
                            execute_digest384_frames_v1(chunk, device)
                        },
                    )
                })
                .expect("actual bounded Metal node dispatch");
                assert_eq!(metal, cpu, "role {role:?}, leaves {len}");
                if len == 17 {
                    assert!(dispatch_sizes.len() > cpu.len());
                    assert!(dispatch_sizes.contains(&1));
                }
                assert_eq!(
                    merkle_root_with_execution_v1(&leaves, role, device).unwrap(),
                    merkle_root_with_execution_v1(&leaves, role, DigestExecutionV1::Cpu).unwrap()
                );
            }
        }
        let leaves =
            hash_lde_leaves_with_mode(&(0_u64..513).collect::<Vec<_>>(), 2, ExecutionMode::Cpu)
                .unwrap();
        assert_eq!(
            merkle_root_with_execution_v1(&leaves, MerkleTreeRoleV1::Lde, device).unwrap(),
            merkle_root_with_mode(&leaves, MerkleTreeRoleV1::Lde, ExecutionMode::Cpu).unwrap()
        );
        assert_ne!(
            merkle_root_with_execution_v1(&leaves, MerkleTreeRoleV1::Fri(0), device).unwrap(),
            merkle_root_with_execution_v1(&leaves, MerkleTreeRoleV1::Fri(7), device).unwrap()
        );
        assert!(!preflight_native_v1_gpu_backend());
    }

    #[test]
    fn native_merkle_device_failure_aborts_tree_without_cpu_substitution() {
        use crate::digest_executor::{
            DigestExecutionV1, execute_bounded_digest384_frames_v1, execute_digest384_frames_v1,
        };
        let leaves: Vec<_> = (0..17)
            .map(|i| GoldilocksDigest384V1::new([i; 6]).unwrap())
            .collect();
        let mut calls = 0;
        let result = build_merkle_levels_with_executor_v1(
            &leaves,
            MerkleTreeRoleV1::Fri(7),
            &mut |frames| {
                execute_bounded_digest384_frames_v1(frames, 2, 1000, &mut |chunk| {
                    calls += 1;
                    if calls == 2 {
                        Err(Error::NativeDigestExecution {
                            details: "injected Merkle device failure".into(),
                        })
                    } else {
                        execute_digest384_frames_v1(chunk, DigestExecutionV1::Cpu)
                    }
                })
            },
        );
        assert!(
            matches!(result, Err(Error::NativeDigestExecution { details }) if details == "injected Merkle device failure")
        );
        assert_eq!(calls, 2);
    }
    #[test]
    fn indexed_prefix_hashes_preserve_all_domain_coordinates_and_payload_framing() {
        for role in [
            MerkleTreeRoleV1::Trace,
            MerkleTreeRoleV1::Lde,
            MerkleTreeRoleV1::AirTrace,
            MerkleTreeRoleV1::AirComposition,
            MerkleTreeRoleV1::Fri(17),
        ] {
            for phase in [MERKLE_LEAF_PHASE_V1, MERKLE_NODE_PHASE_V1] {
                for level in [0, 1, 19] {
                    let prefix =
                        digest_domain_prefix_v1(role.role(), phase, level, role.counter()).unwrap();
                    for index in [0, 1, 7, 1 << 20, usize::MAX] {
                        for fields in [vec![], vec![&[][..]], vec![&[11; 48][..], &[29; 48][..]]] {
                            assert_eq!(
                                hash_at_prefix_v1(&prefix, index, &fields).unwrap(),
                                hash_bytes_v1(
                                    role.role(),
                                    phase,
                                    level,
                                    index,
                                    role.counter(),
                                    &fields
                                )
                                .unwrap(),
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn parallel_fri_prefix_leaves_match_scalar_strided_groups() {
        let pools = [1, 4].map(|workers| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
        });
        for round in [0, 1, 17] {
            for count in [0, 1, 2, 62, 64, 128, 256] {
                let values: Vec<_> = (0..count)
                    .map(|index| {
                        GoldilocksFp4V1::new(core::array::from_fn(|lane| {
                            (index * 43 + lane * 17) as u64
                        }))
                        .unwrap()
                    })
                    .collect();
                let expected = if count == 0 {
                    Vec::new()
                } else {
                    let arity = 2.min(count);
                    let output = count / arity;
                    (0..output)
                        .map(|index| {
                            let group: Vec<_> = (0..arity)
                                .map(|position| values[index + position * output])
                                .collect();
                            hash_fri_chunk(round, index, &group).unwrap()
                        })
                        .collect()
                };
                for pool in &pools {
                    for mode in [ExecutionMode::Cpu, ExecutionMode::Gpu] {
                        assert_eq!(
                            pool.install(|| hash_fri_leaves_with_mode(round, &values, 2, mode))
                                .unwrap(),
                            expected
                        );
                    }
                }
            }
        }
        assert!(matches!(
            hash_fri_leaves_with_mode(0, &[GoldilocksFp4V1::ZERO; 3], 2, ExecutionMode::Cpu),
            Err(Error::FriDomainSize {
                length: 3,
                arity: 2
            })
        ));
        assert!(matches!(
            hash_fri_leaves_with_mode(0, &[], 4, ExecutionMode::Cpu),
            Err(Error::FriArity(4))
        ));
    }

    #[test]
    fn parallel_single_fp4_leaves_match_canonical_hashes_and_first_error() {
        let pools = [1, 4].map(|workers| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
        });
        for role in [LDE_COMMITMENT_ROLE_V1, AIR_COMPOSITION_COMMITMENT_ROLE_V1] {
            for count in [0, 1, 31, 32, 65, 129] {
                let values: Vec<_> = (0..count)
                    .map(|index| {
                        GoldilocksFp4V1::new(core::array::from_fn(|lane| {
                            (index * 73 + lane * 11) as u64
                        }))
                        .unwrap()
                    })
                    .collect();
                let expected: Vec<_> = values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| hash_fp4_values_v1(role, 0, index, &[*value]).unwrap())
                    .collect();
                for pool in &pools {
                    assert_eq!(
                        pool.install(|| hash_fp4_single_leaves_with_role(role, &values))
                            .unwrap(),
                        expected
                    );
                }
            }
        }
        for lane in 0..4 {
            let mut coefficients = [0; 4];
            coefficients[lane] = GOLDILOCKS_MODULUS;
            let malformed = GoldilocksFp4V1::from_coefficients_unchecked_for_test(coefficients);
            let mut values = vec![GoldilocksFp4V1::ZERO; 65];
            values[3] = malformed;
            values[47] = malformed;
            for pool in &pools {
                assert!(matches!(
                    pool.install(|| hash_fp4_single_leaves_with_role(LDE_COMMITMENT_ROLE_V1, &values)),
                    Err(Error::NonCanonicalGoldilocksElement { context: "native_stark_fp4_digest_input", indices })
                        if indices == vec![3, 0, lane]
                ));
            }
        }
    }

    #[test]
    fn parallel_merkle_levels_match_scalar_trees_across_roles_padding_and_worker_counts() {
        fn scalar_levels(
            leaves: &[GoldilocksDigest384V1],
            role: MerkleTreeRoleV1,
        ) -> Vec<Vec<GoldilocksDigest384V1>> {
            if leaves.is_empty() {
                return Vec::new();
            }
            let mut current = leaves.to_vec();
            let mut levels = Vec::new();
            loop {
                if current.len() % 2 == 1 {
                    current.push(*current.last().unwrap());
                }
                levels.push(current.clone());
                let mut next = Vec::with_capacity(current.len() / 2);
                for parent in 0..current.len() / 2 {
                    next.push(
                        merkle_node_hash(
                            role,
                            levels.len(),
                            parent,
                            current[2 * parent],
                            current[2 * parent + 1],
                        )
                        .unwrap(),
                    );
                }
                if next.len() == 1 {
                    levels.push(next);
                    return levels;
                }
                current = next;
            }
        }
        let pools = [1, 4].map(|workers| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap()
        });
        for role in [
            MerkleTreeRoleV1::Trace,
            MerkleTreeRoleV1::Lde,
            MerkleTreeRoleV1::AirTrace,
            MerkleTreeRoleV1::AirComposition,
            MerkleTreeRoleV1::Fri(0),
            MerkleTreeRoleV1::Fri(17),
        ] {
            for count in [0, 1, 3, 31, 63, 64, 65, 129] {
                let leaves = (0..count)
                    .map(|index| {
                        GoldilocksDigest384V1::new(core::array::from_fn(|lane| {
                            1 + 7 * index as u64 + 13 * lane as u64
                        }))
                        .unwrap()
                    })
                    .collect::<Vec<_>>();
                let expected = scalar_levels(&leaves, role);
                for pool in &pools {
                    for mode in [ExecutionMode::Cpu, ExecutionMode::Auto] {
                        let actual = pool
                            .install(|| build_merkle_levels_with_mode(&leaves, role, mode))
                            .unwrap();
                        assert_eq!(
                            actual, expected,
                            "role={role:?}; leaves={count}; mode={mode:?}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn fold_with_fri_emits_layers_and_betas() {
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let mut transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            "fastpq-state-transition-stark-v1",
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
    fn sampler_test_transcript() -> Transcript {
        Transcript::initialise(
            &crate::proof::PublicIO::default(),
            fastpq_isi::FASTPQ_FINAL_V1_ID,
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .unwrap()
    }

    #[test]
    fn query_sampler_empty_and_unsupported_shapes_do_not_draw() {
        for (domain_size, target) in [(0, 0), (0, usize::MAX), (usize::MAX, 0)] {
            assert!(
                sample_queries_from(domain_size, target, |_| panic!("empty sampler drew"))
                    .unwrap()
                    .is_empty()
            );
        }
        let error =
            sample_queries_from(1024, 513, |_| panic!("oversized sampler drew")).unwrap_err();
        assert!(matches!(
            error,
            Error::VerifierLimitExceeded {
                limit: "max_sampled_queries",
                actual: 513,
                max: 512
            }
        ));
        if let Ok(domain) = usize::try_from(GOLDILOCKS_MODULUS) {
            if let Some(unsupported) = domain.checked_add(1) {
                assert!(matches!(
                    sample_queries_from(unsupported, 1, |_| panic!("invalid domain drew")),
                    Err(Error::QuerySamplingDomainUnsupported { domain_size }) if domain_size == unsupported
                ));
            }
            // Domain p accepts the entire canonical field, including p-1.
            let mut draws = 0;
            let selected = sample_queries_from(domain, 1, |counter| {
                assert_eq!(counter, 0);
                draws += 1;
                Ok(GoldilocksDigest384V1::new([GOLDILOCKS_MODULUS - 1; 6]).unwrap())
            })
            .unwrap();
            assert_eq!(selected, [domain - 1]);
            assert_eq!(draws, 1);
        }
    }

    #[test]
    fn query_sampler_rejected_and_duplicate_sources_exhaust_exactly() {
        for (desired, words, expected_selected) in
            [(1, [GOLDILOCKS_MODULUS - 1; 6], 0), (2, [0; 6], 1)]
        {
            let mut calls = 0_u32;
            let error = sample_queries_from(32, desired, |counter| {
                assert_eq!(counter, calls);
                calls += 1;
                Ok(GoldilocksDigest384V1::new(words).unwrap())
            })
            .unwrap_err();
            assert!(matches!(error, Error::QuerySamplingExhausted {
                domain_size: 32, requested, selected, draws: 64
            } if requested == desired && selected == expected_selected));
            assert_eq!(calls, 64);
        }
    }

    #[test]
    fn query_sampler_accepts_the_last_allowed_draw_and_never_draws_one_more() {
        for first_new_index_draw in [63, 64] {
            let mut calls = 0_u32;
            let result = sample_queries_from(32, 2, |counter| {
                assert_eq!(counter, calls);
                calls += 1;
                let word = u64::from(counter >= first_new_index_draw);
                Ok(GoldilocksDigest384V1::new([word; 6]).unwrap())
            });
            assert_eq!(calls, 64);
            if first_new_index_draw == 63 {
                assert_eq!(result.unwrap(), [0, 1]);
            } else {
                assert!(matches!(
                    result,
                    Err(Error::QuerySamplingExhausted {
                        domain_size: 32,
                        requested: 2,
                        selected: 1,
                        draws: 64
                    })
                ));
            }
        }
    }

    #[test]
    fn query_sampler_preserves_rejection_lane_order_clamping_and_source_errors() {
        let mut calls = 0;
        // p-1 is rejected for domain 32; 33 and 1 have the same accepted index.
        let selected = sample_queries_from(32, 2, |counter| {
            assert_eq!(counter, 0, "sampler made an unnecessary second draw");
            calls += 1;
            Ok(GoldilocksDigest384V1::new([GOLDILOCKS_MODULUS - 1, 33, 1, 2, 7, 9]).unwrap())
        })
        .unwrap();
        assert_eq!(selected, [1, 2]);
        assert_eq!(calls, 1);
        // Large raw targets retain the old min(target, domain) behavior before
        // applying the supported desired-cardinality bound.
        assert_eq!(
            sample_queries_from(5, usize::MAX, |_| {
                Ok(GoldilocksDigest384V1::new([4, 3, 2, 1, 0, 0]).unwrap())
            })
            .unwrap(),
            [0, 1, 2, 3, 4]
        );
        let mut calls = 0;
        let error = sample_queries_from(32, 2, |counter| {
            calls += 1;
            if counter == 2 {
                Err(Error::QuerySamplingTranscriptCounterExhausted)
            } else {
                Ok(GoldilocksDigest384V1::new([0; 6]).unwrap())
            }
        })
        .unwrap_err();
        assert!(matches!(
            error,
            Error::QuerySamplingTranscriptCounterExhausted
        ));
        assert_eq!(calls, 3);
    }

    #[test]
    fn query_sampler_engineering_caps_are_fixed_at_supported_cardinalities() {
        for (desired, expected_draws) in [(1, 64), (8, 64), (136, 1088), (200, 1600), (512, 4096)] {
            let mut calls = 0;
            let error = sample_queries_from(1024, desired, |counter| {
                assert_eq!(counter, calls);
                calls += 1;
                Ok(GoldilocksDigest384V1::new([GOLDILOCKS_MODULUS - 1; 6]).unwrap())
            })
            .unwrap_err();
            assert!(matches!(error, Error::QuerySamplingExhausted {
                domain_size: 1024, requested, selected: 0, draws
            } if requested == desired && draws == expected_draws));
            assert_eq!(calls, expected_draws);
        }
    }

    #[test]
    fn query_sampler_matches_legacy_success_and_exact_transcript_state() {
        // A bounded test-only copy of the former algorithm is an independent
        // compatibility oracle. It intentionally has no dependency on the new
        // sampler core/cap and is used only on fixed successful transcripts.
        fn legacy(domain_size: usize, target: usize, transcript: &mut Transcript) -> Vec<usize> {
            let desired = target.min(domain_size);
            let domain = u64::try_from(domain_size).unwrap();
            let mut indices = BTreeSet::new();
            for counter in 0_u32..4096 {
                let tag = format!("{TRANSCRIPT_TAG_QUERY_INDEX}:{counter}");
                let digest = transcript.challenge_digest(&tag);
                let rejection_limit = GOLDILOCKS_MODULUS - GOLDILOCKS_MODULUS % domain;
                for candidate in digest.words() {
                    if indices.len() == desired {
                        break;
                    }
                    if candidate >= rejection_limit {
                        continue;
                    }
                    indices.insert(usize::try_from(candidate % domain).unwrap());
                }
                if indices.len() == desired {
                    return indices.into_iter().collect();
                }
            }
            panic!("fixed legacy sampler fixture exceeded its test budget");
        }
        for (domain, desired) in [(5, 10), (128, 16), (4096, 136), (524_288, 136)] {
            let mut actual_transcript = sampler_test_transcript();
            let mut legacy_transcript = actual_transcript.clone();
            let expected = legacy(domain, desired, &mut legacy_transcript);
            assert_eq!(
                sample_queries(domain, desired, &mut actual_transcript).unwrap(),
                expected
            );
            assert_eq!(actual_transcript.state, legacy_transcript.state);
            assert_eq!(actual_transcript.counter, legacy_transcript.counter);
        }
    }

    #[test]
    fn query_sampler_transcript_counter_errors_without_panic_or_extra_draw() {
        let mut transcript = sampler_test_transcript();
        transcript.counter = u64::MAX;
        let state = transcript.state;
        for (domain, desired) in [(0, 1), (1, 0)] {
            assert!(
                sample_queries(domain, desired, &mut transcript)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(transcript.state, state);
            assert_eq!(transcript.counter, u64::MAX);
        }
        assert!(matches!(
            sample_queries(1, 1, &mut transcript),
            Err(Error::QuerySamplingTranscriptCounterExhausted)
        ));
        assert_eq!(transcript.state, state);
        assert_eq!(transcript.counter, u64::MAX);
        transcript.counter = u64::MAX - 1;
        // Domain one guarantees that every canonical digest lane selects zero.
        assert_eq!(sample_queries(1, 1, &mut transcript).unwrap(), [0]);
        assert_eq!(transcript.counter, u64::MAX);
    }

    #[test]
    fn sampled_queries_are_sorted_and_unique() {
        let mut transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            "fastpq-state-transition-stark-v1",
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .expect("transcript");
        let indices = super::sample_queries(128, 16, &mut transcript).unwrap();
        assert_eq!(indices.len(), 16);
        assert!(indices.windows(2).all(|window| window[0] < window[1]));
    }
    #[test]
    fn sampled_queries_cap_at_domain_size() {
        let mut transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            "fastpq-state-transition-stark-v1",
            1,
            TRANSCRIPT_TAG_INIT,
        )
        .expect("transcript");
        let indices = super::sample_queries(5, 10, &mut transcript).unwrap();
        assert_eq!(indices.len(), 5);
        let unique: BTreeSet<_> = indices.iter().copied().collect();
        assert_eq!(unique.len(), indices.len());
        assert!(indices.iter().all(|&idx| idx < 5));
        assert!(indices.windows(2).all(|window| window[0] < window[1]));
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
            .prove(&batch, &crate::proof::PublicIO::default(), 1)
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
        .expect_err("zero reductions cannot expose the complete terminal layer");
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
    fn fri_terminal_leaf_commits_every_value_in_domain_order() {
        let values = fp4_values(&[1, 2, 3, 4]);
        let leaves = hash_fri_terminal_leaves(7, &values).expect("complete terminal leaf");
        assert_eq!(leaves, [hash_fri_chunk(7, 0, &values).unwrap()]);
        for index in 0..values.len() {
            let mut mutated = values.clone();
            mutated[index] = mutated[index].add(fp4(1));
            assert_ne!(leaves, hash_fri_terminal_leaves(7, &mutated).unwrap());
        }
        let mut reordered = values.clone();
        reordered.swap(0, 1);
        assert_ne!(leaves, hash_fri_terminal_leaves(7, &reordered).unwrap());
        for length in [0, 3, 8] {
            assert!(matches!(
                hash_fri_terminal_leaves(7, &vec![fp4(0); length]),
                Err(Error::FriDomainSize { .. })
            ));
        }
    }
    #[test]
    fn retained_fri_layers_preserve_full_field_roots_transcript_and_opening_bytes() {
        for (length, offset, mode) in [
            (1, 7, ExecutionMode::Cpu),
            (2, 11, ExecutionMode::Auto),
            (4, 7, ExecutionMode::Cpu),
            (32, 11, ExecutionMode::Auto),
            (128, 7, ExecutionMode::Cpu),
        ] {
            let mut params = fastpq_isi::FASTPQ_FINAL_V1;
            params.omega_coset = offset;
            let mut transcript =
                Transcript::initialise(&PublicIO::default(), params.name, 1, TRANSCRIPT_TAG_INIT)
                    .unwrap();
            let mut reference = transcript.clone();
            let values = (0..length)
                .map(|index| {
                    let value = index as u64;
                    GoldilocksFp4V1::new([value + 1, value + 2, value + 3, value + 4]).unwrap()
                })
                .collect::<Vec<_>>();
            let mut retained =
                fold_with_fri_opening_layers(&values, &params, &mut transcript, mode).unwrap();
            assert_eq!(retained.layer_values[0], values);
            let mut domain = FriDomain::from_lde_parameters(
                params.lde_root,
                params.lde_log_size,
                length,
                offset,
            )
            .unwrap();
            // Rebuild every commitment independently and replay the original
            // root/beta schedule, including all four extension-field lanes.
            for (round, layer) in retained.layer_values.iter().enumerate() {
                let terminal = round + 1 == retained.layer_values.len();
                let leaves = if terminal {
                    hash_fri_terminal_leaves(round, layer).unwrap()
                } else {
                    hash_fri_leaves_with_mode(round, layer, 2, ExecutionMode::Cpu).unwrap()
                };
                let root = merkle_root_with_mode(
                    &leaves,
                    MerkleTreeRoleV1::Fri(round as u32),
                    ExecutionMode::Cpu,
                )
                .unwrap();
                assert_eq!(retained.roots[round], root);
                if terminal {
                    reference.append_fri_final(root);
                } else {
                    reference.append_fri_layer(round, root);
                    let beta = reference.challenge_beta(round);
                    assert_eq!(retained.betas[round], beta);
                    assert_eq!(
                        retained.layer_values[round + 1],
                        fold_round(layer, 2, beta, domain).unwrap(),
                    );
                    domain = domain.folded(2);
                }
            }
            assert_eq!(transcript.state, reference.state);
            let sampled = sample_queries(length, 136, &mut transcript).unwrap();
            assert_eq!(
                sampled,
                sample_queries(length, 136, &mut reference).unwrap()
            );
            assert_eq!(transcript.state, reference.state);
            assert_eq!(transcript.counter, reference.counter);
            assert_eq!(
                retained.opening_trees.as_ref().unwrap().tree_build_count(),
                0
            );
            for count in [0, 1, 7, 136] {
                let indices = sampled
                    .iter()
                    .copied()
                    .cycle()
                    .take(count)
                    .collect::<Vec<_>>();
                let expected =
                    open_fri_query_chains(&retained.layer_values, &indices, 2, ExecutionMode::Cpu)
                        .unwrap();
                let actual = retained.open_query_chains(&indices, 2).unwrap();
                assert_eq!(actual, expected);
                assert_eq!(
                    norito::core::to_bytes(&actual).unwrap(),
                    norito::core::to_bytes(&expected).unwrap(),
                );
                assert_eq!(
                    retained.opening_trees.as_ref().unwrap().tree_build_count(),
                    0
                );
                for opening in actual {
                    assert_eq!(opening.final_values, *retained.layer_values.last().unwrap());
                    assert_eq!(opening.final_merkle_path.len(), 1);
                    let round = retained.layer_values.len() - 1;
                    let terminal = hash_fri_chunk(round, 0, &opening.final_values).unwrap();
                    assert_eq!(opening.final_merkle_path[0].as_fastpq(), terminal);
                    assert_eq!(
                        retained.roots[round],
                        merkle_node_hash(
                            MerkleTreeRoleV1::Fri(round as u32),
                            1,
                            0,
                            terminal,
                            terminal
                        )
                        .unwrap(),
                    );
                }
            }
            // Explicit duplicate and upper-half indices exercise occurrence
            // order even when the transcript happens to sample another set.
            let indices = [length - 1, length / 2, 0, length - 1];
            assert_eq!(
                retained.open_query_chains(&indices, 2).unwrap(),
                open_fri_query_chains(&retained.layer_values, &indices, 2, mode).unwrap(),
            );
            assert_eq!(
                retained.opening_trees.as_ref().unwrap().tree_build_count(),
                0
            );
        }
    }

    #[test]
    fn retained_fri_opening_errors_match_legacy_empty_arity_and_index_priority() {
        let params = fastpq_isi::FASTPQ_FINAL_V1;
        for length in [0, 1, 4, 32] {
            let mut transcript =
                Transcript::initialise(&PublicIO::default(), params.name, 1, TRANSCRIPT_TAG_INIT)
                    .unwrap();
            let mut retained = fold_with_fri_opening_layers(
                &vec![fp4(42); length],
                &params,
                &mut transcript,
                ExecutionMode::Cpu,
            )
            .unwrap();
            for arity in [2, 4] {
                for indices in [
                    vec![],
                    vec![length, 0],
                    vec![usize::MAX, length],
                    vec![0, length],
                ] {
                    let expected = open_fri_query_chains(
                        &retained.layer_values,
                        &indices,
                        arity,
                        ExecutionMode::Cpu,
                    );
                    let actual = retained.open_query_chains(&indices, arity);
                    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
                }
            }
            if length == 0 {
                assert!(retained.opening_trees.is_none());
                assert!(matches!(
                    retained.open_query_chains(&[], 2),
                    Err(Error::FriDomainSize { length: 0, .. }),
                ));
            } else {
                assert_eq!(
                    retained.opening_trees.as_ref().unwrap().tree_build_count(),
                    0
                );
            }
        }
    }

    #[test]
    fn fri_terminal_query_opens_the_complete_four_point_domain() {
        let params = fastpq_isi::FASTPQ_FINAL_V1;
        let mut transcript =
            Transcript::initialise(&PublicIO::default(), params.name, 1, TRANSCRIPT_TAG_INIT)
                .unwrap();
        let result = fold_with_fri_opening_layers(
            &[fp4(42); 8],
            &params,
            &mut transcript,
            ExecutionMode::Cpu,
        )
        .expect("binary fold to four terminal points");
        assert_eq!(
            result.layer_values.iter().map(Vec::len).collect::<Vec<_>>(),
            [8, 4]
        );
        assert_eq!(result.betas.len(), 1);
        let queries =
            open_fri_query_chains(&result.layer_values, &[0, 3, 4, 7], 2, ExecutionMode::Cpu)
                .expect("complete terminal openings");
        let terminal_values = result.layer_values.last().unwrap();
        for query in queries {
            assert_eq!(query.final_values, *terminal_values);
            assert_eq!(query.final_merkle_path.len(), 1);
            assert_eq!(query.final_index, query.initial_index % 4);
            let leaf = hash_fri_chunk(1, 0, &query.final_values).unwrap();
            assert!(
                verify_merkle_path_for_role(
                    MerkleTreeRoleV1::Fri(1),
                    result.roots[1],
                    leaf,
                    0,
                    &query
                        .final_merkle_path
                        .iter()
                        .map(|digest| digest.as_fastpq())
                        .collect::<Vec<_>>(),
                )
                .unwrap()
            );
        }
    }
    #[test]
    fn transcript_initialisation_separates_the_quotient_integer_and_terminal_schema() {
        let params = fastpq_isi::FASTPQ_FINAL_V1;
        let public_io = PublicIO::default();
        let transcript =
            Transcript::initialise(&public_io, params.name, 1, TRANSCRIPT_TAG_INIT).unwrap();
        let old_payload = norito::core::to_bytes(&(1_u16, params.name, public_io.clone())).unwrap();
        let old_state = hash_bytes_v1(
            TRANSCRIPT_ROLE_V1,
            b"initialise",
            0,
            0,
            0,
            &[TRANSCRIPT_TAG_INIT.as_bytes(), &old_payload],
        )
        .unwrap();
        assert_ne!(transcript.state, old_state);
        let repeated =
            Transcript::initialise(&public_io, params.name, 1, TRANSCRIPT_TAG_INIT).unwrap();
        assert_eq!(transcript.state, repeated.state);
    }
    #[test]
    fn fri_folding_reduces_layer_length() {
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let mut transcript = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            params.name,
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
        assert_eq!(betas.len(), 2);
        let mut transcript_again = Transcript::initialise(
            &crate::proof::PublicIO::default(),
            params.name,
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
            TRANSCRIPT_TAG_INIT,
        )
        .expect("transcript");
        let (layers, betas) = super::fold_with_fri(
            &evaluations,
            params.fri.arity,
            4,
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
        while current.len() > 4 && round < 4 {
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
        let terminal_leaf = hash_fri_chunk(round, 0, &current).expect("complete terminal leaf");
        let final_root = merkle_root_with_mode(
            &[terminal_leaf],
            MerkleTreeRoleV1::Fri(round as u32),
            ExecutionMode::Cpu,
        )
        .expect("complete terminal root");
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
            TRANSCRIPT_TAG_INIT,
        )
        .expect("transcript");
        let (baseline_layers, _) = super::fold_with_fri(
            &evaluations,
            params.fri.arity,
            4,
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
        while current.len() > 4 && round < 4 {
            let root = reference_fri_layer_commitment(round, &current);
            reference_transcript.append_fri_layer(round, root);
            mutated_layers.push(root);
            let beta = reference_transcript.challenge_beta(round);
            let round_arity = arity.min(current.len());
            current = reference_fold_round(&current, arity, beta, domain);
            domain = domain.folded(round_arity);
            round += 1;
        }
        let terminal_leaf = hash_fri_chunk(round, 0, &current).expect("complete terminal leaf");
        let final_root = merkle_root_with_mode(
            &[terminal_leaf],
            MerkleTreeRoleV1::Fri(round as u32),
            ExecutionMode::Cpu,
        )
        .expect("complete terminal root");
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
                    TRANSCRIPT_TAG_INIT,
                )
                .expect("transcript");
                let mut transcript_b = Transcript::initialise(
                    &crate::proof::PublicIO::default(),
                    params.name,
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
