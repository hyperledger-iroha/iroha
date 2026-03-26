//! # Iroha VM (IVM)
#![allow(unexpected_cfgs)]
// Clippy hygiene: justified global allows
// - upper_case_acronyms: IVM is a proper name used pervasively in the API.
// - type_complexity: some internal tuples/locks intentionally carry rich types.
#![allow(clippy::upper_case_acronyms, clippy::type_complexity)]
//!
//! This library implements the production Iroha virtual machine.  The main
//! entry point is the [`IVM`] struct which can be instantiated and driven by a
//! host environment.  The code is split across a set of modules that roughly
//! follow the layout of the specification:
//!
//! * [`instruction`] – opcode constants and bit-field helpers
//! * [`decoder`] – 16/32‑bit instruction decoder
//! * [`memory`] – region based memory with permission checks
//! * [`gas`] – gas accounting utilities
//! * [`host`] – syscall trait and default implementation
//! * [`vector`] – SIMD helpers and crypto primitives
//! * [`zk`] – zero‑knowledge mode support
//!
//! Run `cargo doc --open` to build the API reference locally. Documentation for
//! the latest release is also hosted on <https://docs.rs/ivm>. You can use
//! `cargo doc --no-deps --features "parallel"` to see docs for optional
//! features or set `RUSTDOCFLAGS="--document-private-items"` for a
//! more exhaustive view. The generated `instruction` module provides detailed
//! commentary on every opcode constant and is summarised in
//! [`docs/opcodes.md`](../docs/opcodes.md).

mod aes;
pub mod analysis;
pub mod axt;
pub mod bn254_vec;
mod branch_predictor;
mod byte_merkle_tree;
pub mod contract_artifact;
mod core_host;
mod cuda;
mod decoder;
mod dev_env;
pub mod ec;
pub mod encoding;
mod error;
pub mod field;
pub mod field_dispatch;
pub mod gas;
mod gpu_manager;
pub mod halo2;
pub mod host;
pub mod instruction;
pub mod iso20022;
mod ivm;
pub mod ivm_cache;
pub mod kotodama;
pub mod kotodama_std;
pub mod limits;
mod memory;
pub mod merkle_utils;
mod metadata;
pub mod mock_wsv;
mod pedersen;
pub mod pointer_abi;
mod poseidon;
pub mod register_file;
mod registers;
pub mod runtime;
pub mod schema_registry;
pub mod segmented_memory;
mod sha3;
pub mod signature;
pub mod simple_instruction;
mod state_overlay;
pub mod syscalls;
mod vector;
pub mod zk;
mod zk_poseidon;
pub mod zk_verify;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};

use iroha_telemetry::metrics::{
    StackSettingsSnapshot, record_stack_gas_multiplier, record_stack_limits,
};
// Deterministic parallel execution utilities.
pub mod parallel;
pub mod tx_parallel;
// Test/fixture helpers (public for tests and dev tools)
pub mod predecoder_fixtures;
// Re-export AES helpers used by benches and downstream users.
// Re-export main types for users of the crate
// Expose the instruction decoder helper at the crate root for tests.
pub use crate::core_host::CoreHost;
#[cfg(test)]
pub use crate::field_dispatch::{clear_field_impl_for_tests, set_field_impl_for_tests};
// Publicly expose gas schedule helper for tests and tooling.
pub use crate::gas::{cost_of, cost_of_with_params};
#[cfg(feature = "cuda")]
pub use crate::gpu_manager::GpuManager;
#[cfg(not(feature = "cuda"))]
pub use crate::gpu_manager::GpuManager;
// Re-export stable mode bits so tests/users can import `ivm::ivm_mode::*`.
pub use crate::metadata::mode as ivm_mode;
// Re-export the canonical Merkle tree from iroha_crypto for general use.
pub use crate::contract_artifact::{
    ContractArtifactError, VerifiedContractArtifact, verify_contract_artifact,
};
pub use crate::metadata::{
    CONTRACT_FEATURE_BIT_VECTOR, CONTRACT_FEATURE_BIT_ZK, CONTRACT_FEATURE_KNOWN_BITS,
    EmbeddedContractInterfaceV1, EmbeddedEntrypointDescriptor, MAGIC as METADATA_MAGIC,
    ProgramMetadata, VECTOR_LENGTH_MAX,
};
pub use crate::{
    aes::{
        aes128_decrypt_many, aes128_encrypt_many, aes128_expand_key, aesdec, aesdec_impl,
        aesdec_n_rounds_many, aesenc, aesenc_impl, aesenc_n_rounds_many, sbox,
    },
    branch_predictor::BranchPredictor,
    byte_merkle_tree::ByteMerkleTree,
    cuda::{
        aesdec_cuda, aesenc_cuda, bn254_add_cuda, bn254_mul_cuda, bn254_sub_cuda, cuda_available,
        cuda_disabled, cuda_last_error_message, ed25519_verify_batch_cuda, ed25519_verify_cuda,
        keccak_f1600_cuda, poseidon2_cuda, poseidon2_cuda_many, poseidon6_cuda,
        poseidon6_cuda_many, reset_cuda_backend_for_tests, sha256_compress_cuda, vector_add_f32,
    },
    decoder::decode,
    ec::{
        ec_add, ec_add_truncated, ec_mul, ec_mul_truncated, pairing_check, pairing_check_truncated,
    },
    error::{Perm, VMError},
    field_dispatch::{Avx2Field, Avx512Field, FieldArithmetic, NeonField, ScalarField, Sse2Field},
    host::IVMHost,
    iso20022::*,
    ivm::{IVM, set_banner_enabled},
    ivm_cache::{
        CacheStats, DecodedOp, IvmCache, global_cache, global_counters, global_get_with_meta,
        global_stats,
    },
    kotodama::compiler::Compiler as KotodamaCompiler,
    memory::{AccessRange, Memory, WriteLogEntry},
    pedersen::{pedersen_commit, pedersen_commit_truncated},
    pointer_abi::{
        PointerType, Tlv, is_type_allowed_for_policy, render_pointer_types_markdown_table,
        validate_tlv_bytes,
    },
    poseidon::{
        poseidon2, poseidon2_many, poseidon2_simd, poseidon6, poseidon6_many, poseidon6_simd,
    },
    sha3::{keccak_f1600, sha3_absorb_block},
    zk_poseidon::{pair_hash_bytes, pair_hash_u64},
};

pub use iroha_crypto::{MerkleProof, MerkleTree};
/// Syscall policy determined by `ProgramMetadata.abi_version`.
pub use ivm_abi::SyscallPolicy;

pub use crate::signature::{Ed25519BatchItem, verify_ed25519_batch_items};
pub use crate::{
    mock_wsv::{
        AccountId, AssetDefinitionId, DomainId, MockWorldStateView, PermissionToken,
        ScopedAccountId, WsvHost,
    },
    registers::Registers,
    segmented_memory::{Memory as SegmentedMemory, Segment},
    signature::{SignatureScheme, verify_signature},
    simple_instruction::Instruction,
    state_overlay::{DurableStateOverlay, DurableStateSnapshot},
    tx_parallel::{PostRunPhase, Transaction, execute_transactions_parallel},
    vector::{
        SimdChoice, bit_pipe_compile_count, clear_forced_simd, clear_thread_forced_simd,
        forced_simd_test_lock, metal_available, metal_disabled, release_metal_state,
        reset_metal_backend_for_tests, set_forced_simd, set_thread_forced_simd, sha256_compress,
        simd_backend, simd_bits, simd_choice, simd_lanes, vadd32, vadd32_auto, vadd64, vadd64_auto,
        vand, vand_auto, vector_supported, vor, vor_auto, vrot32, vrot32_auto, vxor, vxor_auto,
        zero_vector,
    },
    zk::{MemEvent, RegEvent, RegisterState},
};

/// Public Norito-typed request envelopes for VRF syscalls.
pub mod vrf;

#[cfg(test)]
mod ptx_tests;

/// Optional acceleration policy applied at runtime by hosts.
///
/// By default, the VM will use all available hardware backends (SIMD, Metal, CUDA)
/// subject to golden-vector self-tests. Hosts may call
/// `set_acceleration_config` to override the availability of certain backends or
/// to cap the number of GPUs.
#[derive(Clone, Copy, Debug)]
pub struct AccelerationConfig {
    /// Enable SIMD acceleration (NEON/AVX/SSE) when available. When false, force scalar execution.
    pub enable_simd: bool,
    /// Enable Metal backend when available (macOS only). Default: true.
    pub enable_metal: bool,
    /// Enable CUDA backend when available (feature `cuda`). Default: true.
    pub enable_cuda: bool,
    /// Maximum number of GPUs to initialize (None = auto/no cap).
    pub max_gpus: Option<usize>,
    /// Minimum number of leaves to use GPU for Merkle leaf hashing (None = use default).
    pub merkle_min_leaves_gpu: Option<usize>,
    /// Backend-specific thresholds (None = inherit generic GPU threshold).
    pub merkle_min_leaves_metal: Option<usize>,
    pub merkle_min_leaves_cuda: Option<usize>,
    /// Prefer CPU SHA2 for trees up to this many leaves (per-arch). If None, use defaults.
    pub prefer_cpu_sha2_max_leaves_aarch64: Option<usize>,
    pub prefer_cpu_sha2_max_leaves_x86: Option<usize>,
}

impl Default for AccelerationConfig {
    fn default() -> Self {
        Self {
            enable_simd: true,
            enable_metal: true,
            enable_cuda: true,
            max_gpus: None,
            merkle_min_leaves_gpu: None,
            merkle_min_leaves_metal: None,
            merkle_min_leaves_cuda: None,
            prefer_cpu_sha2_max_leaves_aarch64: None,
            prefer_cpu_sha2_max_leaves_x86: None,
        }
    }
}

fn acceleration_config_store() -> &'static Mutex<AccelerationConfig> {
    static STORE: OnceLock<Mutex<AccelerationConfig>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(AccelerationConfig::default()))
}

fn read_acceleration_config() -> AccelerationConfig {
    let guard = acceleration_config_store()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    *guard
}

fn write_acceleration_config(cfg: AccelerationConfig) {
    let mut guard = acceleration_config_store()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    *guard = cfg;
}

/// Apply acceleration configuration. Optional; when not called the VM
/// automatically uses all available hardware, subject to golden self-tests.
pub fn set_acceleration_config(cfg: AccelerationConfig) {
    write_acceleration_config(cfg);
    // SIMD policy: force scalar when disabled, otherwise let runtime detection decide.
    crate::vector::set_simd_policy_enabled(cfg.enable_simd);
    // Metal policy
    #[cfg(all(target_os = "macos", feature = "metal"))]
    {
        crate::vector::set_metal_enabled(cfg.enable_metal);
        if cfg.enable_metal {
            crate::vector::warm_up_metal();
        }
    }
    // CUDA policy
    #[cfg(feature = "cuda")]
    {
        crate::cuda::set_cuda_enabled(cfg.enable_cuda);
        crate::gpu_manager::GpuManager::set_max_gpus(cfg.max_gpus);
    }
    if let Some(min) = cfg.merkle_min_leaves_gpu {
        crate::byte_merkle_tree::set_merkle_gpu_min_leaves(min);
    }
    if let Some(min) = cfg.merkle_min_leaves_metal {
        crate::byte_merkle_tree::set_merkle_metal_min_leaves(min);
    }
    if let Some(min) = cfg.merkle_min_leaves_cuda {
        crate::byte_merkle_tree::set_merkle_cuda_min_leaves(min);
    }
    #[cfg(target_arch = "aarch64")]
    if let Some(v) = cfg.prefer_cpu_sha2_max_leaves_aarch64 {
        crate::byte_merkle_tree::set_prefer_cpu_sha2_max_leaves_aarch64(v);
    }
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if let Some(v) = cfg.prefer_cpu_sha2_max_leaves_x86 {
        crate::byte_merkle_tree::set_prefer_cpu_sha2_max_leaves_x86(v);
    }
}

/// Return the most recently applied [`AccelerationConfig`]. When
/// `set_acceleration_config` has not been called, this returns the default.
#[must_use]
pub fn acceleration_config() -> AccelerationConfig {
    read_acceleration_config()
}

/// Runtime status for a single acceleration backend.
#[derive(Clone, Copy, Debug, Default)]
pub struct BackendRuntimeStatus {
    /// Whether the backend is supported on this build/architecture.
    pub supported: bool,
    /// Whether the active configuration enables the backend.
    pub configured: bool,
    /// Whether the backend is currently available after applying policy and hardware checks.
    pub available: bool,
    /// Whether parity/golden-vector self-tests passed for the backend.
    pub parity_ok: bool,
}

/// Runtime status for acceleration backends.
#[derive(Clone, Copy, Debug, Default)]
pub struct AccelerationRuntimeStatus {
    /// SIMD backend status (scalar vs. hardware vector).
    pub simd: BackendRuntimeStatus,
    /// Metal (macOS/iOS) backend status.
    pub metal: BackendRuntimeStatus,
    /// CUDA backend status.
    pub cuda: BackendRuntimeStatus,
}

/// Retrieve the latest acceleration runtime status including parity checks.
#[must_use]
pub fn acceleration_runtime_status() -> AccelerationRuntimeStatus {
    let cfg = acceleration_config();
    let mut status = AccelerationRuntimeStatus::default();

    // SIMD status (always compiled; may be forced to scalar).
    {
        let detected = crate::vector::detected_simd_choice();
        let choice = crate::vector::simd_choice();
        let simd_available = choice != crate::vector::SimdChoice::Scalar;
        status.simd = BackendRuntimeStatus {
            supported: detected != crate::vector::SimdChoice::Scalar,
            configured: cfg.enable_simd,
            available: cfg.enable_simd && simd_available,
            parity_ok: true,
        };
    }

    // Metal status (macOS + `metal` feature)
    #[cfg(all(target_os = "macos", feature = "metal"))]
    {
        let policy_enabled = crate::vector::metal_policy_enabled();
        let runtime_allowed = crate::vector::metal_runtime_allowed();
        let parity_ok = crate::vector::metal_parity_ok();
        status.metal = BackendRuntimeStatus {
            supported: true,
            configured: cfg.enable_metal,
            available: runtime_allowed,
            parity_ok: parity_ok && runtime_allowed,
        };
        // If policy disabled, runtime_allowed will be false. Preserve parity flag reflecting last self-test.
        if !policy_enabled {
            status.metal.available = false;
            status.metal.parity_ok = parity_ok;
        }
    }
    #[cfg(not(all(target_os = "macos", feature = "metal")))]
    {
        status.metal = BackendRuntimeStatus {
            supported: false,
            configured: cfg.enable_metal,
            available: false,
            parity_ok: false,
        };
    }

    // CUDA status (`cuda` feature)
    #[cfg(feature = "cuda")]
    {
        let available = crate::cuda::cuda_available();
        let parity_ok = !crate::cuda::cuda_disabled();
        status.cuda = BackendRuntimeStatus {
            supported: true,
            configured: cfg.enable_cuda,
            available,
            parity_ok,
        };
    }
    #[cfg(not(feature = "cuda"))]
    {
        status.cuda = BackendRuntimeStatus {
            supported: false,
            configured: cfg.enable_cuda,
            available: false,
            parity_ok: false,
        };
    }

    status
}

/// Last reported error or disable reason for each acceleration backend.
#[derive(Clone, Debug, Default)]
pub struct AccelerationErrorStatus {
    /// SIMD disable reason, if forced to scalar.
    pub simd: Option<String>,
    /// Most recent Metal error/disable message, if any.
    pub metal: Option<String>,
    /// Most recent CUDA error/disable message, if any.
    pub cuda: Option<String>,
}

/// Retrieve sticky error messages associated with acceleration backends.
///
/// These messages are cleared when the backend is re-enabled or reset via
/// [`set_acceleration_config`] / test reset helpers. They provide structured
/// diagnostics beyond the stderr banners.
#[must_use]
pub fn acceleration_runtime_errors() -> AccelerationErrorStatus {
    use crate::vector::SimdChoice;

    let simd_error = if !crate::vector::simd_policy_enabled() {
        Some("disabled by config".to_string())
    } else if matches!(
        crate::vector::forced_simd_choice(),
        Some(SimdChoice::Scalar)
    ) {
        Some("forced scalar override".to_string())
    } else {
        let detected = crate::vector::detected_simd_choice();
        let choice = crate::vector::simd_choice();
        if detected == SimdChoice::Scalar {
            Some("simd unsupported on hardware".to_string())
        } else if choice == SimdChoice::Scalar {
            Some("simd unavailable at runtime".to_string())
        } else {
            None
        }
    };

    AccelerationErrorStatus {
        simd: simd_error,
        metal: crate::vector::metal_last_error_message(),
        cuda: crate::cuda::cuda_last_error_message(),
    }
}

/// Default gas→stack multiplier (bytes of stack available per unit of gas).
const DEFAULT_GAS_TO_STACK_MULTIPLIER: u64 = 4;

/// Global gas→stack multiplier applied when deriving guest stack limits from gas.
static GAS_TO_STACK_MULTIPLIER: AtomicU64 = AtomicU64::new(DEFAULT_GAS_TO_STACK_MULTIPLIER);

/// Minimum stack size applied to scheduler/prover workers and guest stacks.
pub const MIN_STACK_BYTES: usize = 64 * 1024;
/// Maximum stack size applied to scheduler/prover workers and guest stacks.
pub const MAX_STACK_BYTES: usize = 1024 * 1024 * 1024;

/// Outcome of applying stack size configuration, including clamping flags.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StackSizeOutcome {
    /// Requested scheduler stack size (bytes).
    pub requested_scheduler_bytes: usize,
    /// Requested prover stack size (bytes).
    pub requested_prover_bytes: usize,
    /// Requested guest stack size (bytes).
    pub requested_guest_bytes: u64,
    /// Requested guest stack budget cap (bytes).
    pub requested_budget_bytes: u64,
    /// Applied scheduler stack size (bytes) after clamping.
    pub scheduler_bytes: usize,
    /// Applied prover stack size (bytes) after clamping.
    pub prover_bytes: usize,
    /// Applied guest stack size (bytes) after clamping.
    pub guest_bytes: u64,
    /// Applied guest stack budget cap (bytes) after clamping.
    pub budget_bytes: u64,
    /// Whether the scheduler stack request was clamped.
    pub scheduler_clamped: bool,
    /// Whether the prover stack request was clamped.
    pub prover_clamped: bool,
    /// Whether the guest stack request was clamped.
    pub guest_clamped: bool,
    /// Whether the guest stack budget cap was clamped.
    pub budget_clamped: bool,
}

/// Current gas→stack multiplier (bytes of stack permitted per unit of gas).
#[must_use]
pub fn gas_to_stack_multiplier() -> u64 {
    GAS_TO_STACK_MULTIPLIER.load(Ordering::Relaxed).max(1)
}

/// Override the gas→stack multiplier (bytes of stack permitted per unit of gas).
pub fn set_gas_to_stack_multiplier(multiplier: u64) {
    GAS_TO_STACK_MULTIPLIER.store(multiplier.max(1), Ordering::Relaxed);
    record_stack_gas_multiplier(gas_to_stack_multiplier());
}

/// Configure the default scheduler thread limits used by `IVM::new`.
///
/// Hosts should call this early in process startup (before creating VMs) to
/// avoid oversubscription when multiple thread pools are present. Passing
/// `None` for a bound keeps the automatic value (number of physical cores).
/// After resolving "auto" bounds, `min <= max` is enforced by clamping `min` down.
pub fn set_scheduler_thread_limits(min_threads: Option<usize>, max_threads: Option<usize>) {
    crate::parallel::set_default_scheduler_limits(min_threads, max_threads);
}

/// Override the default guest stack limit applied to new VMs.
pub fn set_guest_stack_limit(bytes: u64) {
    crate::memory::Memory::set_default_stack_limit(bytes);
}

/// Current default guest stack limit in bytes.
pub fn guest_stack_limit() -> u64 {
    crate::memory::Memory::default_stack_limit()
}

/// Validate and apply stack size knobs for scheduler/prover pools and guest stack budgets.
pub fn apply_stack_sizes(
    scheduler_bytes: usize,
    prover_bytes: usize,
    guest_bytes: u64,
    budget_bytes: u64,
) -> StackSizeOutcome {
    let sched = scheduler_bytes.clamp(MIN_STACK_BYTES, MAX_STACK_BYTES);
    let prover = prover_bytes.clamp(MIN_STACK_BYTES, MAX_STACK_BYTES);
    let guest = guest_bytes.clamp(MIN_STACK_BYTES as u64, MAX_STACK_BYTES as u64);
    let budget = budget_bytes.clamp(MIN_STACK_BYTES as u64, MAX_STACK_BYTES as u64);
    let outcome = StackSizeOutcome {
        requested_scheduler_bytes: scheduler_bytes,
        requested_prover_bytes: prover_bytes,
        requested_guest_bytes: guest_bytes,
        requested_budget_bytes: budget_bytes,
        scheduler_bytes: sched,
        prover_bytes: prover,
        guest_bytes: guest,
        budget_bytes: budget,
        scheduler_clamped: sched != scheduler_bytes,
        prover_clamped: prover != prover_bytes,
        guest_clamped: guest != guest_bytes,
        budget_clamped: budget != budget_bytes,
    };
    set_scheduler_stack_size(sched);
    set_prover_stack_size(prover);
    set_guest_stack_limit(guest);
    crate::memory::Memory::set_stack_budget_limit(budget);
    record_stack_limits(StackSettingsSnapshot {
        requested_scheduler_bytes: outcome.requested_scheduler_bytes as u64,
        requested_prover_bytes: outcome.requested_prover_bytes as u64,
        requested_guest_bytes: outcome.requested_guest_bytes,
        scheduler_bytes: outcome.scheduler_bytes as u64,
        prover_bytes: outcome.prover_bytes as u64,
        guest_bytes: outcome.guest_bytes,
        scheduler_clamped: outcome.scheduler_clamped,
        prover_clamped: outcome.prover_clamped,
        guest_clamped: outcome.guest_clamped,
        pool_fallback_total: 0,
        budget_hit_total: 0,
        gas_to_stack_multiplier: gas_to_stack_multiplier(),
    });
    outcome
}

/// Initialize the global Rayon thread pool with `num_threads` workers.
///
/// Safe to call once; if a global pool already exists, the request returns
/// `GlobalPoolAlreadyInitialized`.
pub fn init_global_rayon(num_threads: usize) -> Result<(), rayon::ThreadPoolBuildError> {
    let n = num_threads.max(1);
    rayon::ThreadPoolBuilder::new()
        .num_threads(n)
        .stack_size(crate::parallel::thread_stack_size())
        .build_global()
}

/// Override the stack size used by scheduler Rayon pools.
pub fn set_scheduler_stack_size(bytes: usize) {
    crate::parallel::set_thread_stack_size(bytes);
}

/// Override the stack size used by prover Rayon pools.
pub fn set_prover_stack_size(bytes: usize) {
    crate::zk::set_prover_stack_size(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acceleration_runtime_errors_default_none_or_hw_reason() {
        let status = acceleration_runtime_status();
        let errors = acceleration_runtime_errors();
        if status.simd.supported {
            assert!(errors.simd.is_none());
        } else {
            assert_eq!(errors.simd.as_deref(), Some("simd unsupported on hardware"));
        }
        assert!(errors.metal.is_none());
        #[cfg(feature = "cuda")]
        {
            if status.cuda.available {
                assert!(errors.cuda.is_none());
            } else {
                assert!(
                    errors.cuda.is_some(),
                    "expected a CUDA disable/unavailable reason when the CUDA feature is enabled"
                );
            }
        }
        #[cfg(not(feature = "cuda"))]
        {
            assert!(errors.cuda.is_none());
        }
    }

    #[test]
    fn apply_stack_sizes_clamps_and_records_snapshot() {
        let prev_multiplier = gas_to_stack_multiplier();
        record_stack_limits(StackSettingsSnapshot::default());
        record_stack_gas_multiplier(prev_multiplier);

        let outcome = apply_stack_sizes(1, usize::MAX, u64::MAX, u64::MAX);
        assert!(
            outcome.scheduler_clamped,
            "scheduler stack should clamp low values"
        );
        assert!(
            outcome.prover_clamped,
            "prover stack should clamp high values"
        );
        assert!(
            outcome.guest_clamped,
            "guest stack should clamp high values"
        );
        assert!(
            outcome.budget_clamped,
            "guest stack budget should clamp high values"
        );

        let snapshot = iroha_telemetry::metrics::stack_settings_snapshot();
        assert_eq!(
            snapshot.scheduler_bytes, MIN_STACK_BYTES as u64,
            "scheduler stack should clamp to minimum"
        );
        assert_eq!(
            snapshot.prover_bytes, MAX_STACK_BYTES as u64,
            "prover stack should clamp to maximum"
        );
        assert_eq!(
            snapshot.guest_bytes, MAX_STACK_BYTES as u64,
            "guest stack should clamp to maximum"
        );
        assert_eq!(
            crate::memory::Memory::stack_budget_limit(),
            MAX_STACK_BYTES as u64,
            "stack budget limit should clamp to maximum"
        );
        assert_eq!(
            snapshot.gas_to_stack_multiplier,
            prev_multiplier.max(1),
            "stack snapshot should carry gas multiplier"
        );

        let _ = apply_stack_sizes(
            32 * 1024 * 1024,
            32 * 1024 * 1024,
            crate::memory::Memory::STACK_SIZE,
            crate::memory::Memory::STACK_SIZE,
        );
        set_gas_to_stack_multiplier(prev_multiplier);
    }

    #[test]
    fn init_global_rayon_reports_existing_pool() {
        let threads = num_cpus::get_physical().max(1);
        let first = init_global_rayon(threads);
        assert!(
            first.is_ok() || first.is_err(),
            "global pool init should either succeed or report an existing pool"
        );
        let second = init_global_rayon(threads);
        let err = second.expect_err("second init must fail because the global pool is singleton");
        assert!(
            err.to_string().contains("initialized"),
            "expected existing-pool error, got {err}"
        );
    }

    #[test]
    fn guest_stack_limit_propagates_to_vm() {
        let prev = guest_stack_limit();
        let prev_budget = crate::memory::Memory::stack_budget_limit();
        let target = 128 * 1024_u64;
        set_guest_stack_limit(target);
        crate::memory::Memory::set_stack_budget_limit(target);
        // Use a gas limit large enough that the gas-derived ceiling does not clamp the stack.
        let vm = IVM::new(100_000);
        assert_eq!(vm.memory.stack_limit(), target);
        set_guest_stack_limit(prev);
        crate::memory::Memory::set_stack_budget_limit(prev_budget);
    }

    #[test]
    fn stack_limits_are_clamped() {
        apply_stack_sizes(1, usize::MAX, u64::MAX, u64::MAX);
        assert!(guest_stack_limit() <= 1024 * 1024 * 1024);
        assert!(guest_stack_limit() >= 64 * 1024);
        assert!(crate::memory::Memory::stack_budget_limit() <= 1024 * 1024 * 1024);
        assert!(crate::memory::Memory::stack_budget_limit() >= 64 * 1024);
    }

    #[test]
    fn metal_api_exports_are_available_on_all_targets() {
        release_metal_state();
        reset_metal_backend_for_tests();
        assert_eq!(metal_available(), crate::vector::metal_available());
        assert_eq!(metal_disabled(), crate::vector::metal_disabled());
        assert_eq!(
            bit_pipe_compile_count(),
            crate::vector::bit_pipe_compile_count()
        );
    }
}
