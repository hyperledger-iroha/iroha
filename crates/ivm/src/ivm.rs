//! Core IVM struct implementing the complete Iroha VM v1.1 specification.
//!
//! The VM handles instruction decoding, gas accounting, memory safety and
//! optional zero-knowledge padding exactly as defined by the spec. All opcodes,
//! including field arithmetic, vector operations and advanced control flow are
//! supported and `MAXCYCLES` is enforced when a cycle limit is set.
//!
//! This implementation incorporates the updated architecture with 256 tagged
//! registers, optional hardware transactional memory and basic hardware feature
//! detection. Vector operations currently operate on a fixed 128‑bit width and
//! a fully scalable vector extension remains future work.
#[cfg(feature = "beep")]
use std::time::Duration;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    panic::AssertUnwindSafe,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use likely_stable::{likely, unlikely};
#[cfg(feature = "beep")]
use rodio::{OutputStream, OutputStreamHandle, Sink, Source, source::SineWave};
use sha2::{Digest, Sha256};

use crate::{
    SyscallPolicy, decoder,
    error::VMError,
    gas,
    host::{AccessLog, DefaultHost, IVMHost},
    instruction,
    memory::Memory,
    metadata::ProgramMetadata,
    parallel::{
        self, Block, BlockResult, ExecutionContext, Scheduler, State, Transaction, TxResult,
    },
    pointer_abi::PointerPolicyGuard,
    registers::Registers,
    simple_instruction::Instruction as SimpleInstruction,
    vector,
    vector::SimdChoice,
    zk::{self, Constraint, DeltaTraceLog, MemEvent, MemLog, RegisterState},
};

static SUPPRESS_BANNER: AtomicBool = AtomicBool::new(false);
static WORKER_CACHE_ID: AtomicU64 = AtomicU64::new(1);
static HARDWARE_CAPABILITIES: OnceLock<HardwareCapabilities> = OnceLock::new();

/// Upper bound on logical vector length supported by the VM.
const LOGICAL_VECTOR_MAX: usize = crate::metadata::VECTOR_LENGTH_MAX as usize;
/// Default logical vector length when not specified by metadata.
const DEFAULT_VECTOR_LENGTH: usize = 4;
/// Baseline lane count used for gas accounting of vector ops.
const GAS_VECTOR_BASE_LANES: usize = crate::gas::VECTOR_BASE_LANES;

fn default_vector_length() -> usize {
    DEFAULT_VECTOR_LENGTH.clamp(1, LOGICAL_VECTOR_MAX)
}

fn isqrt_u64(mut n: u64) -> u64 {
    // Shift to the highest power-of-four <= n.
    let mut bit = 1u64 << 62;
    while bit > n {
        bit >>= 2;
    }
    let mut res = 0u64;
    while bit != 0 {
        if n >= res + bit {
            n -= res + bit;
            res = (res >> 1) + bit;
        } else {
            res >>= 1;
        }
        bit >>= 2;
    }
    res
}

fn div_ceil_i64(num: i64, denom: i64) -> Result<i64, VMError> {
    if denom == 0 {
        return Err(VMError::AssertionFailed);
    }
    // Avoid the i64::MIN / -1 overflow case to keep behavior deterministic.
    if num == i64::MIN && denom == -1 {
        return Ok(i64::MIN);
    }
    let q = num / denom;
    let r = num % denom;
    if r == 0 {
        Ok(q)
    } else if (r > 0 && denom > 0) || (r < 0 && denom < 0) {
        Ok(q + 1)
    } else {
        Ok(q)
    }
}

fn abs_i64_to_u64(value: i64) -> u64 {
    value.unsigned_abs()
}

fn gcd_i64(a: i64, b: i64) -> u64 {
    let mut a = abs_i64_to_u64(a);
    let mut b = abs_i64_to_u64(b);
    if a == 0 {
        return b;
    }
    if b == 0 {
        return a;
    }
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

/// Snapshot of hardware accelerators detected on the current host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HardwareCapabilities {
    cuda_available: bool,
    metal_available: bool,
}

impl HardwareCapabilities {
    /// Construct a capability snapshot with explicit CUDA/Metal availability.
    pub const fn new(cuda_available: bool, metal_available: bool) -> Self {
        Self {
            cuda_available,
            metal_available,
        }
    }

    /// Capability snapshot representing no optional accelerators.
    pub const fn none() -> Self {
        Self::new(false, false)
    }

    /// Indicates whether CUDA devices are available.
    pub const fn cuda_available(self) -> bool {
        self.cuda_available
    }

    /// Indicates whether Metal is available (macOS/iOS GPU backend).
    pub const fn metal_available(self) -> bool {
        self.metal_available
    }
}

thread_local! {
    static WORKER_CACHE: RefCell<Option<WorkerCache>> = const { RefCell::new(None) };
}

struct WorkerCache {
    id: u64,
    resources: WorkerResources,
}

struct WorkerResources {
    vm: IVM,
    ctx: ExecutionContext,
    template_memory: Memory,
    template_input_bump: u64,
}

impl WorkerResources {
    fn new(
        template: &Arc<Mutex<IVM>>,
        host: Option<&Arc<Mutex<Option<Box<dyn IVMHost + Send>>>>>,
    ) -> Self {
        let (mut vm, template_memory, template_input_bump) = {
            let template = template.lock().unwrap_or_else(|err| err.into_inner());
            (
                template.clone(),
                template.memory.clone(),
                template.input_bump_next,
            )
        };
        if let Some(host_arc) = host {
            vm.host = Some(Box::new(crate::runtime::SyscallDispatcher::shared(
                Arc::clone(host_arc),
            )));
        }
        Self {
            vm,
            ctx: ExecutionContext::new(),
            template_memory,
            template_input_bump,
        }
    }

    fn execute(&mut self, tx: Transaction) -> TxResult {
        self.vm.memory.reset_from_template(&self.template_memory);
        self.vm.input_bump_next = self.template_input_bump;
        self.ctx.init_for_transaction(&tx, &self.vm.state);
        let result = self.vm.execute_transaction(&tx, &mut self.ctx);
        if result.success {
            self.vm.commit_transaction(&self.ctx, &tx.access.reg_tags);
        }
        result
    }
}

/// Control whether the VM prints the ASCII banner and hardware feature summary
/// at construction time. Benches can call this to disable noisy output.
#[allow(dead_code)]
pub fn set_banner_enabled(enabled: bool) {
    SUPPRESS_BANNER.store(!enabled, Ordering::Relaxed);
}

pub(crate) fn rtm_available() -> bool {
    #[cfg(all(feature = "htm", target_arch = "x86_64"))]
    {
        use std::arch::x86_64::{__cpuid_count, __get_cpuid_max};

        const RTM_FLAG: u32 = 1 << 11;

        // RTM detection is not available on stable via `is_x86_feature_detected!`, so use CPUID
        // leaf 7 (EBX bit 11) which is supported on all processors implementing RTM.
        unsafe {
            let (max_leaf, _) = __get_cpuid_max(0);
            if max_leaf < 7 {
                return false;
            }
            (__cpuid_count(0x7, 0).ebx & RTM_FLAG) != 0
        }
    }
    #[cfg(all(feature = "htm", target_arch = "x86"))]
    {
        use std::arch::x86::{__cpuid_count, __get_cpuid_max};

        const RTM_FLAG: u32 = 1 << 11;

        unsafe {
            let (max_leaf, _) = __get_cpuid_max(0);
            if max_leaf < 7 {
                return false;
            }
            (__cpuid_count(0x7, 0).ebx & RTM_FLAG) != 0
        }
    }
    #[cfg(not(all(feature = "htm", any(target_arch = "x86", target_arch = "x86_64"))))]
    {
        false
    }
}

fn hardware_capabilities_snapshot() -> &'static HardwareCapabilities {
    HARDWARE_CAPABILITIES.get_or_init(|| {
        HardwareCapabilities::new(crate::cuda::cuda_available(), vector::metal_available())
    })
}

// Integration with the parallel execution module providing a persistent
// scheduler and deterministic commit of transaction results.

/// Hardware-acceleration policy applied when constructing an [`IVM`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccelerationPolicy {
    allow_cuda: bool,
    allow_metal: bool,
    forced_simd: Option<SimdChoice>,
}

impl AccelerationPolicy {
    /// Construct a policy with explicit toggles.
    pub const fn new(allow_cuda: bool, allow_metal: bool) -> Self {
        Self {
            allow_cuda,
            allow_metal,
            forced_simd: None,
        }
    }

    /// Deterministic policy that disables all optional accelerators.
    pub const fn deterministic() -> Self {
        Self::new(false, false)
    }

    fn env_disables(name: &str) -> bool {
        crate::dev_env::dev_env_flag(name)
    }

    /// Default policy honouring the environment toggles (`IVM_DISABLE_*`).
    pub fn adaptive() -> Self {
        let allow_cuda = !Self::env_disables("IVM_DISABLE_CUDA");
        let allow_metal = !Self::env_disables("IVM_DISABLE_METAL");
        Self::new(allow_cuda, allow_metal)
    }

    /// Enable or disable CUDA.
    pub const fn with_cuda(mut self, allow: bool) -> Self {
        self.allow_cuda = allow;
        self
    }

    /// Enable or disable Metal.
    pub const fn with_metal(mut self, allow: bool) -> Self {
        self.allow_metal = allow;
        self
    }

    /// Override SIMD backend selection. Unsupported choices fall back to scalar.
    pub const fn with_forced_simd(mut self, choice: Option<SimdChoice>) -> Self {
        self.forced_simd = choice;
        self
    }

    pub(crate) const fn allow_cuda(&self) -> bool {
        self.allow_cuda
    }

    pub(crate) const fn allow_metal(&self) -> bool {
        self.allow_metal
    }

    pub(crate) const fn forced_simd(&self) -> Option<SimdChoice> {
        self.forced_simd
    }
}

impl Default for AccelerationPolicy {
    fn default() -> Self {
        Self::adaptive()
    }
}

/// Configuration describing how a new [`IVM`] should be initialised.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IvmConfig {
    gas_limit: u64,
    acceleration: AccelerationPolicy,
    capabilities: HardwareCapabilities,
    stack_limit_bytes: u64,
    /// Per-route stack budget (bytes) used as an additional cap when deriving the effective stack.
    ///
    /// Hosts can set this to a compute route/profile budget (e.g.,
    /// `ComputeResourceBudget.max_stack_bytes`) to ensure guest stacks never exceed the
    /// advertised per-call limit.
    stack_budget_bytes: u64,
}

impl IvmConfig {
    /// Create a configuration with the provided gas limit and adaptive acceleration policy.
    #[must_use]
    pub fn new(gas_limit: u64) -> Self {
        Self {
            gas_limit,
            acceleration: AccelerationPolicy::default(),
            capabilities: *hardware_capabilities_snapshot(),
            stack_limit_bytes: crate::memory::Memory::default_stack_limit(),
            stack_budget_bytes: crate::memory::Memory::stack_budget_limit(),
        }
    }

    /// Deterministic configuration disabling accelerator usage.
    #[must_use]
    pub fn deterministic(gas_limit: u64) -> Self {
        Self::new(gas_limit).with_acceleration(AccelerationPolicy::deterministic())
    }

    /// Adaptive configuration honouring environment toggles for accelerators.
    #[must_use]
    pub fn adaptive(gas_limit: u64) -> Self {
        Self::new(gas_limit)
    }

    /// Override the acceleration policy.
    #[must_use]
    pub fn with_acceleration(mut self, policy: AccelerationPolicy) -> Self {
        self.acceleration = policy;
        self
    }

    /// Override the SIMD backend used by this configuration.
    #[must_use]
    pub fn with_forced_simd(self, choice: Option<SimdChoice>) -> Self {
        self.with_acceleration(self.acceleration().with_forced_simd(choice))
    }

    /// Override the detected hardware capabilities.
    #[must_use]
    pub fn with_capabilities(mut self, capabilities: HardwareCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Override the guest stack limit applied when constructing the VM.
    #[must_use]
    pub fn with_stack_limit_bytes(mut self, stack_limit_bytes: u64) -> Self {
        self.stack_limit_bytes = stack_limit_bytes;
        self
    }

    /// Update the guest stack limit without consuming the builder.
    pub fn set_stack_limit_bytes(&mut self, stack_limit_bytes: u64) -> &mut Self {
        self.stack_limit_bytes = stack_limit_bytes;
        self
    }

    /// Gas limit to enforce for the VM.
    #[must_use]
    pub const fn gas_limit(&self) -> u64 {
        self.gas_limit
    }

    /// Acceleration policy to apply after construction.
    #[must_use]
    pub const fn acceleration(&self) -> AccelerationPolicy {
        self.acceleration
    }

    /// Hardware capabilities the VM should expose.
    #[must_use]
    pub const fn capabilities(&self) -> HardwareCapabilities {
        self.capabilities
    }

    /// Stack limit (bytes) enforced for the VM's guest stack.
    #[must_use]
    pub const fn stack_limit_bytes(&self) -> u64 {
        self.stack_limit_bytes
    }

    /// Per-route stack budget (bytes) applied when deriving the guest stack limit.
    #[must_use]
    pub const fn stack_budget_bytes(&self) -> u64 {
        self.stack_budget_bytes
    }

    /// Return a copy with the guest stack limit overridden.
    #[must_use]
    pub fn map_stack_limit(self, bytes: u64) -> Self {
        self.with_stack_limit_bytes(bytes)
    }

    /// Override the per-route stack budget applied when constructing the VM.
    #[must_use]
    pub fn with_stack_budget_bytes(mut self, stack_budget_bytes: u64) -> Self {
        self.stack_budget_bytes = stack_budget_bytes;
        self
    }

    /// Update the per-route stack budget without consuming the builder.
    pub fn set_stack_budget_bytes(&mut self, stack_budget_bytes: u64) -> &mut Self {
        self.stack_budget_bytes = stack_budget_bytes;
        self
    }

    /// Derive a stack limit bounded by both the configured stack cap and a gas-aware ceiling.
    #[must_use]
    pub fn stack_limit_for_gas(&self) -> u64 {
        // Policy: clamp to min(configured cap, gas_limit*multiplier bytes, global budget), with a 64 KiB floor.
        let gas_cap_bytes = self
            .gas_limit
            .saturating_mul(crate::gas_to_stack_multiplier());
        let configured = self.stack_limit_bytes.max(64 * 1024);
        let budget = self.stack_budget_bytes.max(1);
        configured.min(gas_cap_bytes).min(budget).max(64 * 1024)
    }

    /// Create a builder seeded with this configuration's values.
    #[must_use]
    pub fn builder(self) -> IvmConfigBuilder {
        IvmConfigBuilder::from_config(self)
    }

    /// Produce a modified copy of this configuration.
    #[must_use]
    pub fn map<F>(self, mut f: F) -> Self
    where
        F: FnMut(IvmConfigBuilder) -> IvmConfigBuilder,
    {
        f(self.builder()).build()
    }

    /// Convert this configuration into an [`IvmConfigBuilder`] for further tweaks.
    #[must_use]
    pub fn to_builder(self) -> IvmConfigBuilder {
        self.builder()
    }

    /// Return a copy with capabilities transformed by `f`.
    #[must_use]
    pub fn map_capabilities<F>(self, f: F) -> Self
    where
        F: FnOnce(HardwareCapabilities) -> HardwareCapabilities,
    {
        Self {
            capabilities: f(self.capabilities),
            ..self
        }
    }
}

/// Builder for `IvmConfig` allowing ergonomic incremental construction.
#[derive(Debug)]
pub struct IvmConfigBuilder {
    gas_limit: u64,
    acceleration: AccelerationPolicy,
    capabilities: HardwareCapabilities,
    stack_limit_bytes: u64,
    stack_budget_bytes: u64,
}

impl IvmConfigBuilder {
    /// Begin building a configuration with the provided gas limit.
    #[must_use]
    pub fn new(gas_limit: u64) -> Self {
        Self {
            gas_limit,
            acceleration: AccelerationPolicy::default(),
            capabilities: *hardware_capabilities_snapshot(),
            stack_limit_bytes: crate::memory::Memory::default_stack_limit(),
            stack_budget_bytes: crate::memory::Memory::stack_budget_limit(),
        }
    }

    /// Create a builder initialised from an existing configuration.
    #[must_use]
    pub fn from_config(config: IvmConfig) -> Self {
        Self {
            gas_limit: config.gas_limit(),
            acceleration: config.acceleration(),
            capabilities: config.capabilities(),
            stack_limit_bytes: config.stack_limit_bytes(),
            stack_budget_bytes: config.stack_budget_bytes(),
        }
    }

    /// Configuration enabling deterministic execution (no accelerators).
    #[must_use]
    pub fn deterministic(gas_limit: u64) -> Self {
        Self::new(gas_limit).with_acceleration(AccelerationPolicy::deterministic())
    }

    /// Configuration using the adaptive acceleration policy.
    #[must_use]
    pub fn adaptive(gas_limit: u64) -> Self {
        Self::new(gas_limit)
    }

    /// Override the gas limit.
    #[must_use]
    pub fn with_gas_limit(mut self, gas: u64) -> Self {
        self.gas_limit = gas;
        self
    }

    /// Override the acceleration policy.
    #[must_use]
    pub fn with_acceleration(mut self, policy: AccelerationPolicy) -> Self {
        self.acceleration = policy;
        self
    }

    /// Override SIMD backend selection for this configuration.
    #[must_use]
    pub fn with_forced_simd(mut self, choice: Option<SimdChoice>) -> Self {
        self.acceleration = self.acceleration.with_forced_simd(choice);
        self
    }

    /// Override hardware capabilities.
    #[must_use]
    pub fn with_capabilities(mut self, capabilities: HardwareCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Override the guest stack limit (bytes).
    #[must_use]
    pub fn with_stack_limit_bytes(mut self, stack_limit_bytes: u64) -> Self {
        self.stack_limit_bytes = stack_limit_bytes;
        self
    }

    /// Override the per-route stack budget (bytes).
    #[must_use]
    pub fn with_stack_budget_bytes(mut self, stack_budget_bytes: u64) -> Self {
        self.stack_budget_bytes = stack_budget_bytes;
        self
    }

    /// Finalise the builder and produce a configuration.
    #[must_use]
    pub fn build(self) -> IvmConfig {
        IvmConfig {
            gas_limit: self.gas_limit,
            acceleration: self.acceleration,
            capabilities: self.capabilities,
            stack_limit_bytes: self.stack_limit_bytes,
            stack_budget_bytes: self.stack_budget_bytes,
        }
    }

    /// Convert the builder into a configuration without consuming further state.
    #[must_use]
    pub fn into_config(self) -> IvmConfig {
        self.build()
    }
}

/// Builder for configuring [`IVM`] construction.
pub struct IvmBuilder {
    config: IvmConfig,
    suppress_banner: bool,
    host_config: Option<Box<dyn FnOnce(&mut IVM)>>,
}

impl IvmBuilder {
    /// Start building an [`IVM`] with the provided gas limit.
    pub fn new(gas_limit: u64) -> Self {
        Self::from_config(IvmConfig::adaptive(gas_limit))
    }

    /// Builder preset disabling accelerators.
    pub fn deterministic(gas_limit: u64) -> Self {
        Self::from_config(IvmConfig::deterministic(gas_limit))
    }

    /// Builder preset using the adaptive accelerator detection.
    pub fn adaptive(gas_limit: u64) -> Self {
        Self::from_config(IvmConfig::adaptive(gas_limit))
    }

    /// Builder preset returning both configuration and builder for deterministic runs.
    /// The returned builder does not suppress the startup banner; callers who
    /// want a quiet VM should call [`suppress_startup_banner`](IvmBuilder::suppress_startup_banner).
    pub fn deterministic_config(gas_limit: u64) -> (IvmConfig, IvmBuilder) {
        let config = IvmConfig::deterministic(gas_limit);
        let builder = IvmBuilder::with_config(config);
        (builder.config(), builder)
    }

    /// Builder preset returning both configuration and builder for deterministic runs with banner suppressed.
    #[must_use]
    pub fn deterministic_config_suppressed(gas_limit: u64) -> (IvmConfig, IvmBuilder) {
        let (cfg, builder) = Self::deterministic_config(gas_limit);
        (cfg, builder.suppress_startup_banner())
    }

    /// Builder preset returning both configuration and builder for adaptive runs.
    /// The builder leaves the startup banner enabled by default.
    pub fn adaptive_config(gas_limit: u64) -> (IvmConfig, IvmBuilder) {
        let config = IvmConfig::adaptive(gas_limit);
        let builder = IvmBuilder::with_config(config);
        (builder.config(), builder)
    }

    /// Builder preset returning both configuration and banner-suppressed builder for adaptive runs.
    #[must_use]
    pub fn adaptive_config_suppressed(gas_limit: u64) -> (IvmConfig, IvmBuilder) {
        let (cfg, builder) = Self::adaptive_config(gas_limit);
        (cfg, builder.suppress_startup_banner())
    }

    /// Construct a builder from an existing configuration.
    pub fn from_config(config: IvmConfig) -> Self {
        Self {
            config,
            suppress_banner: false,
            host_config: None,
        }
    }

    /// Construct a builder from an `IvmConfigBuilder`.
    pub fn from_config_builder(builder: IvmConfigBuilder) -> Self {
        Self::from_config(builder.build())
    }

    /// Create a new builder with the provided configuration, leaving the original untouched.
    #[must_use]
    pub fn with_config(config: IvmConfig) -> Self {
        Self::from_config(config)
    }

    /// Create a new builder using an `IvmConfigBuilder`.
    #[must_use]
    pub fn with_config_builder(builder: IvmConfigBuilder) -> Self {
        Self::from_config_builder(builder)
    }
    /// Override the acceleration policy applied after construction.
    pub fn with_acceleration(mut self, policy: AccelerationPolicy) -> Self {
        self.config.acceleration = policy;
        self
    }

    /// Override SIMD backend selection for subsequent builds.
    pub fn with_forced_simd(mut self, choice: Option<SimdChoice>) -> Self {
        self.config.acceleration = self.config.acceleration.with_forced_simd(choice);
        self
    }

    /// Return a new builder with an updated gas limit.
    #[must_use]
    pub fn with_gas_limit(mut self, gas_limit: u64) -> Self {
        self.config.gas_limit = gas_limit;
        self
    }

    /// Override the guest stack limit (bytes) for subsequent builds.
    #[must_use]
    pub fn with_stack_limit_bytes(mut self, stack_limit_bytes: u64) -> Self {
        self.config.stack_limit_bytes = stack_limit_bytes;
        self
    }

    /// Override the per-route stack budget (bytes) for subsequent builds.
    #[must_use]
    pub fn with_stack_budget_bytes(mut self, stack_budget_bytes: u64) -> Self {
        self.config.stack_budget_bytes = stack_budget_bytes;
        self
    }

    /// Update the acceleration policy without consuming the builder.
    pub fn set_acceleration(&mut self, policy: AccelerationPolicy) -> &mut Self {
        self.config.acceleration = policy;
        self
    }

    /// Update SIMD backend selection without consuming the builder.
    pub fn set_forced_simd(&mut self, choice: Option<SimdChoice>) -> &mut Self {
        let policy = self.config.acceleration.with_forced_simd(choice);
        self.config.acceleration = policy;
        self
    }

    /// Override the gas limit for subsequent builds.
    pub fn set_gas_limit(&mut self, gas_limit: u64) -> &mut Self {
        self.config.gas_limit = gas_limit;
        self
    }

    /// Update the guest stack limit (bytes) without consuming the builder.
    pub fn set_stack_limit_bytes(&mut self, stack_limit_bytes: u64) -> &mut Self {
        self.config.stack_limit_bytes = stack_limit_bytes;
        self
    }

    /// Update the per-route stack budget (bytes) without consuming the builder.
    pub fn set_stack_budget_bytes(&mut self, stack_budget_bytes: u64) -> &mut Self {
        self.config.stack_budget_bytes = stack_budget_bytes;
        self
    }

    /// Provide a custom host implementation.
    pub fn with_host<H: IVMHost + Send + 'static>(mut self, host: H) -> Self {
        self.host_config = Some(Box::new(move |vm| vm.set_host(host)));
        self
    }

    /// Override the hardware capabilities seen by the constructed VM.
    /// Useful for deterministic tests that need to force accelerator availability.
    pub fn with_capabilities(mut self, capabilities: HardwareCapabilities) -> Self {
        self.config.capabilities = capabilities;
        self
    }

    /// Return a builder with capabilities transformed by `f`.
    #[must_use]
    pub fn map_capabilities_builder<F>(mut self, f: F) -> Self
    where
        F: FnOnce(HardwareCapabilities) -> HardwareCapabilities,
    {
        self.config = self.config.map_capabilities(f);
        self
    }

    /// Update the capabilities override without consuming the builder.
    pub fn set_capabilities(&mut self, capabilities: HardwareCapabilities) -> &mut Self {
        self.config.capabilities = capabilities;
        self
    }

    /// Skip the startup banner regardless of global flags.
    pub fn suppress_startup_banner(mut self) -> Self {
        self.suppress_banner = true;
        self
    }

    /// Access the current configuration snapshot.
    #[must_use]
    pub fn config(&self) -> IvmConfig {
        self.config
    }

    /// Consume the builder and return its configuration.
    #[must_use]
    pub fn into_config(self) -> IvmConfig {
        self.config
    }

    /// Produce a configuration builder seeded from the current builder state.
    #[must_use]
    pub fn config_builder(&self) -> IvmConfigBuilder {
        self.config.builder()
    }

    /// Consume the builder and return a configuration builder for further edits.
    #[must_use]
    pub fn into_config_builder(self) -> IvmConfigBuilder {
        self.config.builder()
    }

    /// Retrieve the current configuration without consuming the builder.
    #[must_use]
    pub fn build_config(&self) -> IvmConfig {
        self.config
    }

    /// Apply a transformation to the underlying configuration.
    pub fn map_config<F>(&mut self, mut f: F) -> &mut Self
    where
        F: FnMut(IvmConfig) -> IvmConfig,
    {
        self.config = f(self.config);
        self
    }

    /// Try to update the configuration, propagating an error from the closure if any.
    pub fn try_map_config<F, E>(&mut self, mut f: F) -> Result<&mut Self, E>
    where
        F: FnMut(IvmConfig) -> Result<IvmConfig, E>,
    {
        self.config = f(self.config)?;
        Ok(self)
    }

    /// Current gas limit that will be applied when building.
    #[must_use]
    pub fn gas_limit(&self) -> u64 {
        self.config.gas_limit()
    }

    /// Consume the builder and create an [`IVM`].
    ///
    /// This respects the banner suppression flag and leaves the configuration
    /// unreturned; prefer [`build_with_config`](Self::build_with_config) when the
    /// caller needs to reuse the configuration later.
    pub fn build(self) -> IVM {
        struct BannerGuard {
            previous: bool,
            active: bool,
        }

        impl BannerGuard {
            fn new(suppress: bool) -> Self {
                if suppress {
                    let prev = SUPPRESS_BANNER.swap(true, Ordering::Relaxed);
                    Self {
                        previous: prev,
                        active: true,
                    }
                } else {
                    Self {
                        previous: SUPPRESS_BANNER.load(Ordering::Relaxed),
                        active: false,
                    }
                }
            }
        }

        impl Drop for BannerGuard {
            fn drop(&mut self) {
                if self.active {
                    SUPPRESS_BANNER.store(self.previous, Ordering::Relaxed);
                }
            }
        }

        let guard = BannerGuard::new(self.suppress_banner);
        let mut vm = IVM::new_from_config(self.config);
        if let Some(config) = self.host_config {
            config(&mut vm);
        }
        let scheduler_threads = vm.scheduler.thread_count();
        vm.core_count = scheduler_threads;
        IVM::startup_banner(
            scheduler_threads,
            vm.max_vector_lanes,
            vm.htm_supported,
            vm.hardware_capabilities(),
            vm.use_metal,
            vm.use_cuda,
        );
        drop(guard);
        vm
    }

    /// Consume the builder and return both the final configuration and VM.
    ///
    /// The returned configuration is the exact snapshot applied to the VM and
    /// can be fed back into [`IvmBuilder::with_config`] to construct additional
    /// VMs with identical settings.
    pub fn build_with_config(self) -> (IvmConfig, IVM) {
        let cfg = self.config;
        let vm = self.build();
        (cfg, vm)
    }
}

impl From<IvmConfig> for IvmBuilder {
    fn from(config: IvmConfig) -> Self {
        IvmBuilder::from_config(config)
    }
}

impl From<IvmConfigBuilder> for IvmBuilder {
    fn from(builder: IvmConfigBuilder) -> Self {
        IvmBuilder::from_config_builder(builder)
    }
}

pub struct IVM {
    pub registers: Registers,
    // The vector register file has been folded into `registers`. Vector
    // operations access groups of general registers rather than a separate
    // structure.
    pub memory: Memory,
    pub pc: u64,
    host: Option<Box<dyn IVMHost + Send>>,
    pub gas_remaining: u64,
    cycles: u64,
    halted: bool,
    constraint_failed: bool,
    constraints: zk::ConstraintLog,
    mem_log: MemLog,
    reg_log: zk::RegLog,
    trace_log: DeltaTraceLog,
    step_log: zk::StepLog,
    vector_enabled: bool,
    /// Maximum number of 64-bit lanes supported natively by the host CPU.
    max_vector_lanes: usize,
    /// Current logical vector length for vector operations.
    vector_length: usize,
    max_cycles: u64,
    metadata: ProgramMetadata,
    code_hash: [u8; 32],
    predecoded: Option<Arc<[crate::ivm_cache::DecodedOp]>>,
    predecoded_index: HashMap<u64, usize>,
    branch_predictor: crate::branch_predictor::BranchPredictor,
    branch_predictions: u64,
    branch_correct: u64,
    #[cfg(test)]
    #[allow(dead_code)]
    /// When true (tests only), prints PC and instruction words as they execute.
    pub(crate) decode_trace: bool,
    #[cfg(test)]
    predecoded_misses: u64,
    // Shared world state used for block execution.
    state: State,
    // Parallel scheduler reused across blocks.
    scheduler: std::sync::Arc<Scheduler>,
    // Execution contexts allocated per worker thread.
    _contexts: Vec<ExecutionContext>,
    // Number of threads used for scheduling.
    core_count: usize,
    /// Is Metal GPU acceleration available?
    use_metal: bool,
    /// Is CUDA GPU acceleration available?
    use_cuda: bool,
    /// Flag indicating if zero-knowledge mode is active.
    pub zk_mode: bool,
    /// Does the host CPU support hardware transactional memory?
    htm_supported: bool,
    /// Next free offset (relative to `Memory::INPUT_START`) used by the
    /// simple INPUT TLV bump allocator for host-returned pointers.
    input_bump_next: u64,
    acceleration_policy: AccelerationPolicy,
    hardware_capabilities: HardwareCapabilities,
}

// The VM is self-contained and can be safely transferred across threads when no
// host is shared between them.
unsafe impl Send for IVM {}
unsafe impl Sync for IVM {}
impl std::panic::RefUnwindSafe for IVM {}

impl Clone for IVM {
    fn clone(&self) -> Self {
        Self {
            registers: self.registers.clone(),
            memory: self.memory.clone(),
            pc: self.pc,
            host: None,
            gas_remaining: self.gas_remaining,
            cycles: self.cycles,
            halted: self.halted,
            constraint_failed: self.constraint_failed,
            constraints: self.constraints.clone(),
            mem_log: self.mem_log.clone(),
            reg_log: self.reg_log.clone(),
            trace_log: self.trace_log.clone(),
            step_log: self.step_log.clone(),
            vector_enabled: self.vector_enabled,
            max_vector_lanes: self.max_vector_lanes,
            vector_length: self.vector_length,
            max_cycles: self.max_cycles,
            metadata: self.metadata.clone(),
            code_hash: self.code_hash,
            predecoded: self.predecoded.clone(),
            predecoded_index: self.predecoded_index.clone(),
            branch_predictor: self.branch_predictor.clone(),
            branch_predictions: 0,
            branch_correct: 0,
            #[cfg(test)]
            decode_trace: false,
            #[cfg(test)]
            predecoded_misses: 0,
            state: self.state.clone(),
            scheduler: std::sync::Arc::clone(&self.scheduler),
            _contexts: Vec::new(),
            core_count: self.core_count,
            use_metal: self.use_metal,
            use_cuda: self.use_cuda,
            zk_mode: self.zk_mode,
            htm_supported: self.htm_supported,
            input_bump_next: self.input_bump_next,
            acceleration_policy: self.acceleration_policy,
            hardware_capabilities: self.hardware_capabilities,
        }
    }
}

impl IVM {
    /// Construct a builder for configuring VM creation.
    #[must_use]
    pub fn builder(gas_limit: u64) -> IvmBuilder {
        IvmBuilder::new(gas_limit)
    }

    /// Construct a builder preset that disables accelerator usage.
    #[must_use]
    pub fn deterministic_builder(gas_limit: u64) -> IvmBuilder {
        IvmBuilder::deterministic(gas_limit)
    }

    /// Construct a builder preset that applies the adaptive acceleration policy.
    #[must_use]
    pub fn adaptive_builder(gas_limit: u64) -> IvmBuilder {
        IvmBuilder::adaptive(gas_limit)
    }

    /// Construct a new VM directly from a configuration.
    pub fn with_config(config: IvmConfig) -> IvmBuilder {
        IvmBuilder::from_config(config)
    }

    /// Construct a builder from an `IvmConfigBuilder`.
    pub fn with_config_builder(builder: IvmConfigBuilder) -> IvmBuilder {
        IvmBuilder::from_config_builder(builder)
    }

    /// Create a new VM using the default adaptive acceleration policy.
    pub fn new(gas_limit: u64) -> Self {
        IvmBuilder::new(gas_limit).suppress_startup_banner().build()
    }

    /// Create a new VM using the provided configuration.
    pub fn new_with_config(config: IvmConfig) -> Self {
        IVM::new_from_config(config)
    }

    /// First general register index used to hold vector register data.
    const VECTOR_BASE: usize = 32;
    /// Gas costs for the simple interpreter.
    const GAS_ALU: u64 = 1;
    const GAS_MEM: u64 = 3;
    const GAS_JUMP: u64 = 1;
    const GAS_SHA256_BASE: u64 = 10;
    const GAS_SHA256_PER_BYTE: u64 = 1;
    const GAS_ED25519_VERIFY: u64 = 1000;
    const GAS_ED25519_BATCH_PER_ENTRY: u64 = 500;
    const MAX_ED25519_BATCH_ENTRIES: usize = 512;
    #[allow(dead_code)]
    const GAS_DILITHIUM_VERIFY: u64 = 5000;

    /// Play a short tune on the default audio device.
    ///
    /// This function is only available when built with the `beep` feature.
    #[cfg(feature = "beep")]
    pub fn beep_music() {
        const BPM: u64 = 120;
        const BEAT_MS: u64 = 60_000 / BPM;

        #[derive(Clone, Copy)]
        enum Note {
            A4,
            B4,
            G4,
            E4,
            Rest,
        }

        impl Note {
            fn freq(self) -> Option<u32> {
                match self {
                    Note::A4 => Some(440),
                    Note::B4 => Some(494),
                    Note::G4 => Some(392),
                    Note::E4 => Some(330),
                    Note::Rest => None,
                }
            }
        }

        fn play_element(handle: &OutputStreamHandle, note: Note, beats: f32) {
            let dur_ms = (BEAT_MS as f32 * beats) as u64;
            if let Some(f) = note.freq() {
                let sink = Sink::try_new(handle).unwrap();
                let src = SineWave::new(f as f32)
                    .take_duration(Duration::from_millis(dur_ms))
                    .amplify(0.20);
                sink.append(src);
                sink.sleep_until_end();
            } else {
                std::thread::sleep(Duration::from_millis(dur_ms));
            }
        }

        // Japanese song, Kagome-Kagome
        fn play_kagome(handle: &OutputStreamHandle) {
            let score = vec![
                (Note::A4, 2.0),
                (Note::A4, 1.0),
                (Note::B4, 1.0),
                (Note::A4, 1.0),
                (Note::A4, 1.0),
                (Note::A4, 1.0),
                (Note::Rest, 1.0),
                (Note::A4, 1.0),
                (Note::A4, 0.5),
                (Note::A4, 0.5),
                (Note::A4, 1.0),
                (Note::G4, 0.5),
                (Note::G4, 0.5),
                (Note::A4, 1.0),
                (Note::A4, 0.5),
                (Note::G4, 0.5),
                (Note::E4, 1.0),
                (Note::Rest, 1.0),
                (Note::A4, 1.0),
                (Note::G4, 1.0),
                (Note::A4, 1.0),
                (Note::G4, 1.0),
                (Note::A4, 1.0),
                (Note::A4, 0.5),
                (Note::G4, 0.5),
                (Note::E4, 1.0),
                (Note::Rest, 1.0),
                (Note::A4, 1.0),
                (Note::A4, 1.0),
                (Note::A4, 1.0),
                (Note::B4, 1.0),
                (Note::A4, 1.0),
                (Note::A4, 1.0),
                (Note::A4, 1.0),
                (Note::Rest, 1.0),
                (Note::A4, 1.0),
                (Note::G4, 0.5),
                (Note::G4, 0.5),
                (Note::A4, 1.0),
                (Note::G4, 0.5),
                (Note::G4, 0.5),
                (Note::A4, 1.0),
                (Note::A4, 1.0),
                (Note::E4, 1.0),
                (Note::Rest, 1.0),
                (Note::A4, 0.5),
                (Note::A4, 0.5),
                (Note::A4, 0.5),
                (Note::A4, 0.5),
                (Note::A4, 1.0),
                (Note::B4, 1.0),
                (Note::A4, 1.5),
                (Note::G4, 0.5),
                (Note::A4, 1.0),
                (Note::Rest, 1.0),
            ];

            for (n, b) in score {
                play_element(handle, n, b);
            }
        }

        if let Ok((_stream, handle)) = OutputStream::try_default() {
            play_kagome(&handle);
        }
    }

    fn startup_banner(
        core_count: usize,
        max_vector: usize,
        htm: bool,
        capabilities: HardwareCapabilities,
        metal: bool,
        cuda: bool,
    ) {
        // Suppress banner when disabled programmatically (e.g., in benches)
        if SUPPRESS_BANNER.load(Ordering::Relaxed) {
            return;
        }
        const ART: &str = r#" 
 ██╗██████╗  ██████╗ ██╗ ██╗  █████╗ 
 ██║██╔══██╗██╔═══██╗██║ ██║ ██╔══██╗
 ██║██████╔╝██║   ██║███████████████║
 ██║██╔═██║ ██║   ██║██╔═██╔═██╔══██║
 ██║██║ ║██╗╚██████╔╝██║ ██║ ██║  ██║
 ╚═╝╚═╝ ╚══╝ ╚═════╝ ╚═╝ ╚═╝ ╚═╝  ╚═╝

  ╔════════╗ ╔════════╗ ╔════════╗
  ║   イ   ║ ║   ロ   ║ ║   ハ   ║
  ╚════════╝ ╚════════╝ ╚════════╝
  
 ＜ バ　ー　チ　ャ　ル　・　マ　シ　ン ＞

"#;
        println!("{ART}");
        println!(
            "Platform: {} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        let accel = if max_vector > 1 || htm || metal || cuda {
            "yes"
        } else {
            "no"
        };
        println!("Hardware acceleration detected: {accel}");
        if capabilities.metal_available() {
            let status = if metal {
                "enabled"
            } else {
                "disabled by policy"
            };
            println!("Metal GPU available ({status})");
        }
        if capabilities.cuda_available() {
            let status = if cuda {
                "enabled"
            } else {
                "disabled by policy"
            };
            println!("CUDA GPU available ({status})");
        }
        let core_label = if core_count == 1 { "core" } else { "cores" };
        println!("Using {core_count} {core_label}");
    }
    /// Create a new IVM instance with default host and no program loaded.
    fn new_from_config(config: IvmConfig) -> Self {
        // Initially allocate memory for a reasonable code size (can be adjusted upon loading).
        let gas_limit = config.gas_limit();
        let mem = Memory::new_with_stack_limit(0, config.stack_limit_for_gas());
        vector::set_thread_forced_simd(config.acceleration().forced_simd());
        let max_vector_lanes = {
            #[cfg(target_arch = "x86_64")]
            {
                if std::is_x86_feature_detected!("avx512f") {
                    8
                } else if std::is_x86_feature_detected!("avx2") {
                    4
                } else if std::is_x86_feature_detected!("sse2") {
                    2
                } else {
                    1
                }
            }
            #[cfg(target_arch = "aarch64")]
            {
                2
            }
            #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
            {
                1
            }
        };

        // Global Rayon pool initialization is handled by the host (e.g., irohad)
        // based on configuration. Avoid initializing a large global pool here to
        // reduce oversubscription when multiple thread pools coexist.

        let htm_supported = cfg!(all(feature = "htm", target_arch = "x86_64")) && rtm_available();

        let mut vm = IVM {
            registers: Registers::new(),
            memory: mem,
            pc: 0,
            host: Some(Box::new(crate::runtime::SyscallDispatcher::new(
                DefaultHost::new(),
            ))),
            gas_remaining: gas_limit,
            cycles: 0,
            halted: false,
            constraint_failed: false,
            constraints: zk::ConstraintLog::default(),
            mem_log: MemLog::default(),
            reg_log: zk::RegLog::default(),
            trace_log: DeltaTraceLog::default(),
            step_log: zk::StepLog::default(),
            vector_enabled: false,
            max_vector_lanes,
            vector_length: default_vector_length(),
            max_cycles: 0,
            metadata: ProgramMetadata::default(),
            code_hash: [0u8; 32],
            predecoded: None,
            predecoded_index: HashMap::new(),
            branch_predictor: crate::branch_predictor::BranchPredictor::new(1024),
            branch_predictions: 0,
            branch_correct: 0,
            #[cfg(test)]
            decode_trace: false,
            #[cfg(test)]
            predecoded_misses: 0,
            state: State::new(),
            core_count: {
                let (min, _max) = crate::parallel::default_scheduler_limits();
                min
            },
            scheduler: {
                let (min, max) = crate::parallel::default_scheduler_limits();
                std::sync::Arc::new(Scheduler::new_dynamic(min, max))
            },
            _contexts: Vec::new(),
            use_metal: false,
            use_cuda: false,
            zk_mode: false,
            htm_supported,
            input_bump_next: 0,
            acceleration_policy: AccelerationPolicy::deterministic(),
            hardware_capabilities: config.capabilities(),
        };
        vm.scheduler
            .set_forced_simd(config.acceleration().forced_simd());
        vm.set_hardware_capabilities(config.capabilities());
        vm.set_acceleration_policy(config.acceleration());
        vm.core_count = vm.scheduler.thread_count();
        vm
    }

    fn apply_acceleration_policy(&mut self, policy: AccelerationPolicy) {
        let caps = self.hardware_capabilities;
        self.acceleration_policy = policy;
        vector::set_thread_forced_simd(policy.forced_simd());
        self.scheduler.set_forced_simd(policy.forced_simd());
        self.use_metal = policy.allow_metal() && caps.metal_available();
        self.use_cuda = policy.allow_cuda() && caps.cuda_available();
    }

    /// Decode the next instruction from code memory and advance the program counter.
    ///
    /// The simple decoder understands a compact 16-bit encoding for basic
    /// arithmetic, memory and branch operations as well as a 32-bit form for
    /// absolute jumps. Unknown opcodes or invalid fetches yield
    /// `VMError::DecodeError`.
    pub fn decode_next(&self) -> Result<(crate::simple_instruction::Instruction, u8), VMError> {
        use crate::simple_instruction::Instruction;
        let half = self
            .memory
            .fetch_u16(self.pc)
            .map_err(|_| VMError::DecodeError)?;

        // Check for 32-bit jump prefix 0b11111xxxx_xxxxxxxx
        if (half >> 11) == 0x1F {
            let next = self
                .memory
                .fetch_u16(self.pc + 2)
                .map_err(|_| VMError::DecodeError)?;
            let hi = (half & 0x07FF) as u32;
            let target = ((hi << 16) | next as u32) as u64;
            return Ok((Instruction::Jump { target }, 4));
        }

        let op = (half >> 12) & 0xF;
        let f1 = (half >> 8) & 0xF;
        let f2 = (half >> 4) & 0xF;
        let f3 = half & 0xF;
        let instr = match op {
            0x0 => Instruction::Halt,
            0x1 => Instruction::Add {
                rd: f1,
                rs: f2,
                rt: f3,
            },
            0x2 => Instruction::Sub {
                rd: f1,
                rs: f2,
                rt: f3,
            },
            0x3 => {
                let offset = ((f3 as i8) << 4) >> 4;
                Instruction::Load {
                    rd: f1,
                    addr_reg: f2,
                    offset,
                }
            }
            0x4 => {
                let offset = ((f3 as i8) << 4) >> 4;
                Instruction::Store {
                    rs: f1,
                    addr_reg: f2,
                    offset,
                }
            }
            0x6 => Instruction::Xor {
                rd: f1,
                rs: f2,
                rt: f3,
            },
            0x5 => {
                let offset = ((f3 as i8) << 4) >> 4;
                Instruction::Beq {
                    rs: f1,
                    rt: f2,
                    offset: offset as i16,
                }
            }
            _ => return Err(VMError::DecodeError),
        };
        Ok((instr, 2))
    }

    /// Execute a single simple instruction.
    pub fn execute_instruction(&mut self, instr: SimpleInstruction) -> Result<(), VMError> {
        // Determine gas cost for this instruction
        let cost = match instr {
            SimpleInstruction::Add { .. }
            | SimpleInstruction::Sub { .. }
            | SimpleInstruction::And { .. }
            | SimpleInstruction::Or { .. }
            | SimpleInstruction::AddImm { .. }
            | SimpleInstruction::SubImm { .. }
            | SimpleInstruction::Xor { .. }
            | SimpleInstruction::Sll { .. }
            | SimpleInstruction::Srl { .. }
            | SimpleInstruction::Sra { .. }
            | SimpleInstruction::SetVL { .. } => Self::GAS_ALU,
            SimpleInstruction::Vadd { .. } => {
                let lanes = self.vector_length.clamp(1, GAS_VECTOR_BASE_LANES);
                Self::GAS_ALU * lanes as u64
            }
            SimpleInstruction::Load { .. } | SimpleInstruction::Store { .. } => Self::GAS_MEM,
            SimpleInstruction::Jump { .. } | SimpleInstruction::Beq { .. } => Self::GAS_JUMP,
            SimpleInstruction::Sha256 { len, .. } => {
                Self::GAS_SHA256_BASE + Self::GAS_SHA256_PER_BYTE * len
            }
            SimpleInstruction::Ed25519Verify { .. } => Self::GAS_ED25519_VERIFY,
            SimpleInstruction::DilithiumVerify { .. } => Self::GAS_DILITHIUM_VERIFY,
            SimpleInstruction::Halt => 0,
        };
        if self.gas_remaining < cost {
            return Err(VMError::OutOfGas);
        }
        self.gas_remaining -= cost;

        match instr {
            SimpleInstruction::Add { rd, rs, rt } => {
                let a = self.registers.get(rs as usize);
                let b = self.registers.get(rt as usize);
                if self.zk_mode {
                    let tag_a = self.registers.tag(rs as usize);
                    let tag_b = self.registers.tag(rt as usize);
                    if tag_a != tag_b {
                        return Err(VMError::PrivacyViolation);
                    }
                    self.registers.set_tag(rd as usize, tag_a);
                }
                let sum = a.wrapping_add(b);
                self.registers.set(rd as usize, sum);
            }
            SimpleInstruction::Sub { rd, rs, rt } => {
                let a = self.registers.get(rs as usize);
                let b = self.registers.get(rt as usize);
                if self.zk_mode {
                    let tag_a = self.registers.tag(rs as usize);
                    let tag_b = self.registers.tag(rt as usize);
                    if tag_a != tag_b {
                        return Err(VMError::PrivacyViolation);
                    }
                    self.registers.set_tag(rd as usize, tag_a);
                }
                let diff = a.wrapping_sub(b);
                self.registers.set(rd as usize, diff);
            }
            SimpleInstruction::And { rd, rs, rt } => {
                let a = self.registers.get(rs as usize);
                let b = self.registers.get(rt as usize);
                if self.zk_mode {
                    let tag_a = self.registers.tag(rs as usize);
                    let tag_b = self.registers.tag(rt as usize);
                    if tag_a != tag_b {
                        return Err(VMError::PrivacyViolation);
                    }
                    self.registers.set_tag(rd as usize, tag_a);
                }
                self.registers.set(rd as usize, a & b);
            }
            SimpleInstruction::Or { rd, rs, rt } => {
                let a = self.registers.get(rs as usize);
                let b = self.registers.get(rt as usize);
                if self.zk_mode {
                    let tag_a = self.registers.tag(rs as usize);
                    let tag_b = self.registers.tag(rt as usize);
                    if tag_a != tag_b {
                        return Err(VMError::PrivacyViolation);
                    }
                    self.registers.set_tag(rd as usize, tag_a);
                }
                self.registers.set(rd as usize, a | b);
            }
            SimpleInstruction::AddImm { rd, rs, imm } => {
                let a = self.registers.get(rs as usize);
                let tag = self.zk_unary_tag(rs as usize);
                self.zk_apply_tag(rd as usize, tag);
                self.registers
                    .set(rd as usize, a.wrapping_add(imm as i64 as u64));
            }
            SimpleInstruction::SubImm { rd, rs, imm } => {
                let a = self.registers.get(rs as usize);
                let tag = self.zk_unary_tag(rs as usize);
                self.zk_apply_tag(rd as usize, tag);
                self.registers
                    .set(rd as usize, a.wrapping_sub(imm as i64 as u64));
            }
            SimpleInstruction::Xor { rd, rs, rt } => {
                let a = self.registers.get(rs as usize);
                let b = self.registers.get(rt as usize);
                if self.zk_mode {
                    let tag_a = self.registers.tag(rs as usize);
                    let tag_b = self.registers.tag(rt as usize);
                    if tag_a != tag_b {
                        return Err(VMError::PrivacyViolation);
                    }
                    self.registers.set_tag(rd as usize, tag_a);
                }
                self.registers.set(rd as usize, a ^ b);
            }
            SimpleInstruction::Sll { rd, rs, rt } => {
                let tag = self.zk_match_tags(rs as usize, rt as usize)?;
                self.zk_apply_tag(rd as usize, tag);
                let a = self.registers.get(rs as usize);
                let b = self.registers.get(rt as usize) & 0x3F;
                self.registers.set(rd as usize, a << b);
            }
            SimpleInstruction::Srl { rd, rs, rt } => {
                let tag = self.zk_match_tags(rs as usize, rt as usize)?;
                self.zk_apply_tag(rd as usize, tag);
                let a = self.registers.get(rs as usize);
                let b = self.registers.get(rt as usize) & 0x3F;
                self.registers.set(rd as usize, a >> b);
            }
            SimpleInstruction::Sra { rd, rs, rt } => {
                let tag = self.zk_match_tags(rs as usize, rt as usize)?;
                self.zk_apply_tag(rd as usize, tag);
                let a = self.registers.get(rs as usize) as i64;
                let b = (self.registers.get(rt as usize) & 0x3F) as u32;
                self.registers.set(rd as usize, (a >> b) as u64);
            }
            SimpleInstruction::SetVL { new_vl } => {
                if !self.vector_enabled {
                    return Err(VMError::VectorExtensionDisabled);
                }
                let mut vl = new_vl as usize;
                if vl == 0 {
                    vl = 1;
                }
                self.vector_length = vl.min(LOGICAL_VECTOR_MAX);
            }
            SimpleInstruction::Vadd { rd, rs, rt } => {
                if !self.vector_enabled {
                    return Err(VMError::VectorExtensionDisabled);
                }
                let n = self.vector_length;
                let stride = n;
                let rd = Self::VECTOR_BASE + rd as usize * stride;
                let rs = Self::VECTOR_BASE + rs as usize * stride;
                let rt = Self::VECTOR_BASE + rt as usize * stride;
                if rd + n > 256 || rs + n > 256 || rt + n > 256 {
                    return Err(VMError::RegisterOutOfBounds);
                }
                for i in 0..n {
                    let a = self.registers.get(rs + i);
                    let b = self.registers.get(rt + i);
                    if self.zk_mode {
                        let tag_a = self.registers.tag(rs + i);
                        let tag_b = self.registers.tag(rt + i);
                        if tag_a != tag_b {
                            return Err(VMError::PrivacyViolation);
                        }
                        self.registers.set_tag(rd + i, tag_a);
                    }
                    let sum = a.wrapping_add(b);
                    self.registers.set(rd + i, sum);
                }
            }
            SimpleInstruction::Load {
                rd,
                addr_reg,
                offset,
            } => {
                if self.zk_mode && self.registers.tag(addr_reg as usize) {
                    // Disallow secret-dependent memory access in ZK mode.
                    return Err(VMError::PrivacyViolation);
                }
                let base = self.registers.get(addr_reg as usize) as i64;
                let addr = base.wrapping_add(offset as i64) as u64;
                let value = self.memory.load_u64(addr)?;
                self.registers.set(rd as usize, value);
                if self.zk_mode {
                    self.registers.set_tag(rd as usize, false);
                }
            }
            SimpleInstruction::Store {
                rs,
                addr_reg,
                offset,
            } => {
                if self.zk_mode && self.registers.tag(addr_reg as usize) {
                    return Err(VMError::PrivacyViolation);
                }
                let base = self.registers.get(addr_reg as usize) as i64;
                let addr = base.wrapping_add(offset as i64) as u64;
                let value = self.registers.get(rs as usize);
                self.memory.store_u64(addr, value)?;
            }
            SimpleInstruction::Jump { target } => {
                self.pc = target;
            }
            SimpleInstruction::Beq { rs, rt, offset } => {
                if self.zk_mode {
                    let cond_left_private = self.registers.tag(rs as usize);
                    let cond_right_private = self.registers.tag(rt as usize);
                    if cond_left_private || cond_right_private {
                        // Branching on private data would leak information.
                        return Err(VMError::PrivacyViolation);
                    }
                }
                let a = self.registers.get(rs as usize);
                let b = self.registers.get(rt as usize);
                let predicted = self.branch_predictor.predict(self.pc);
                let taken = a == b;
                self.branch_predictions += 1;
                if likely(predicted == taken) {
                    self.branch_correct += 1;
                } else {
                    self.cycles += 1;
                }
                self.branch_predictor.update(self.pc, taken);
                if likely(taken) {
                    // Offset is in units of instructions (2 bytes each)
                    let offs = offset as i64 * 2;
                    self.pc = ((self.pc as i64) + offs) as u64;
                }
            }
            SimpleInstruction::Sha256 {
                dest,
                src_addr,
                len,
            } => {
                let data = self.memory.load_region(src_addr, len)?;
                use sha2::{Digest, Sha256};
                let digest = Sha256::digest(data);
                for i in 0..4 {
                    let mut chunk = [0u8; 8];
                    chunk.copy_from_slice(&digest[i * 8..(i + 1) * 8]);
                    let val = u64::from_le_bytes(chunk);
                    self.registers.set(dest as usize + i, val);
                    if self.zk_mode {
                        self.registers.set_tag(dest as usize + i, false);
                    }
                }
            }
            SimpleInstruction::Ed25519Verify {
                pubkey_addr,
                sig_addr,
                msg_addr,
                msg_len,
                result_reg,
            } => {
                use ed25519_dalek::{Signature, VerifyingKey};
                let pk_slice = self.memory.load_region(pubkey_addr, 32)?;
                let sig_slice = self.memory.load_region(sig_addr, 64)?;
                let msg = self.memory.load_region(msg_addr, msg_len)?;
                let pk_bytes: [u8; 32] = match pk_slice.try_into() {
                    Ok(b) => b,
                    Err(_) => {
                        self.registers.set(result_reg as usize, 0);
                        return Ok(());
                    }
                };
                let sig_bytes_arr: [u8; 64] = match sig_slice.try_into() {
                    Ok(b) => b,
                    Err(_) => {
                        self.registers.set(result_reg as usize, 0);
                        return Ok(());
                    }
                };
                #[cfg(feature = "cuda")]
                if self.use_cuda {
                    if let Some(res) =
                        crate::cuda::ed25519_verify_cuda(&msg, &sig_bytes_arr, &pk_bytes)
                    {
                        let value = if res { 1 } else { 0 };
                        if result_reg == 0 {
                            self.registers.force_set(0, value);
                            if self.zk_mode {
                                self.registers.force_set_tag(0, false);
                            }
                        } else {
                            self.registers.set(result_reg as usize, value);
                            if self.zk_mode {
                                self.registers.set_tag(result_reg as usize, false);
                            }
                        }
                        return Ok(());
                    }
                }
                let pk = match VerifyingKey::from_bytes(&pk_bytes) {
                    Ok(k) => k,
                    Err(_) => {
                        self.registers.set(result_reg as usize, 0);
                        return Ok(());
                    }
                };
                let sig = match Signature::from_slice(&sig_bytes_arr) {
                    Ok(s) => s,
                    Err(_) => {
                        self.registers.set(result_reg as usize, 0);
                        return Ok(());
                    }
                };
                let valid = pk.verify_strict(msg, &sig).is_ok();
                let value = if valid { 1 } else { 0 };
                if result_reg == 0 {
                    self.registers.force_set(0, value);
                    if self.zk_mode {
                        self.registers.force_set_tag(0, false);
                    }
                } else {
                    self.registers.set(result_reg as usize, value);
                    if self.zk_mode {
                        self.registers.set_tag(result_reg as usize, false);
                    }
                }
            }
            SimpleInstruction::DilithiumVerify {
                level,
                pubkey_addr,
                sig_addr,
                msg_addr,
                msg_len,
                result_reg,
            } => {
                use pqcrypto_dilithium::{dilithium2, dilithium3, dilithium5};
                use pqcrypto_traits::sign::{DetachedSignature as _, PublicKey as _};
                let msg = self.memory.load_region(msg_addr, msg_len)?;
                let valid = match level {
                    2 => {
                        let pk_slice = self
                            .memory
                            .load_region(pubkey_addr, dilithium2::public_key_bytes() as u64)?;
                        let sig_slice = self
                            .memory
                            .load_region(sig_addr, dilithium2::signature_bytes() as u64)?;
                        let pk = match dilithium2::PublicKey::from_bytes(pk_slice) {
                            Ok(p) => p,
                            Err(_) => {
                                self.registers.set(result_reg as usize, 0);
                                return Ok(());
                            }
                        };
                        let sig = match dilithium2::DetachedSignature::from_bytes(sig_slice) {
                            Ok(s) => s,
                            Err(_) => {
                                self.registers.set(result_reg as usize, 0);
                                return Ok(());
                            }
                        };
                        dilithium2::verify_detached_signature(&sig, msg, &pk).is_ok()
                    }
                    3 => {
                        let pk_slice = self
                            .memory
                            .load_region(pubkey_addr, dilithium3::public_key_bytes() as u64)?;
                        let sig_slice = self
                            .memory
                            .load_region(sig_addr, dilithium3::signature_bytes() as u64)?;
                        let pk = match dilithium3::PublicKey::from_bytes(pk_slice) {
                            Ok(p) => p,
                            Err(_) => {
                                self.registers.set(result_reg as usize, 0);
                                return Ok(());
                            }
                        };
                        let sig = match dilithium3::DetachedSignature::from_bytes(sig_slice) {
                            Ok(s) => s,
                            Err(_) => {
                                self.registers.set(result_reg as usize, 0);
                                return Ok(());
                            }
                        };
                        dilithium3::verify_detached_signature(&sig, msg, &pk).is_ok()
                    }
                    5 => {
                        let pk_slice = self
                            .memory
                            .load_region(pubkey_addr, dilithium5::public_key_bytes() as u64)?;
                        let sig_slice = self
                            .memory
                            .load_region(sig_addr, dilithium5::signature_bytes() as u64)?;
                        let pk = match dilithium5::PublicKey::from_bytes(pk_slice) {
                            Ok(p) => p,
                            Err(_) => {
                                self.registers.set(result_reg as usize, 0);
                                return Ok(());
                            }
                        };
                        let sig = match dilithium5::DetachedSignature::from_bytes(sig_slice) {
                            Ok(s) => s,
                            Err(_) => {
                                self.registers.set(result_reg as usize, 0);
                                return Ok(());
                            }
                        };
                        dilithium5::verify_detached_signature(&sig, msg, &pk).is_ok()
                    }
                    _ => false,
                };
                let value = if valid { 1 } else { 0 };
                if result_reg == 0 {
                    self.registers.force_set(0, value);
                    if self.zk_mode {
                        self.registers.force_set_tag(0, false);
                    }
                } else {
                    self.registers.set(result_reg as usize, value);
                    if self.zk_mode {
                        self.registers.set_tag(result_reg as usize, false);
                    }
                }
            }
            SimpleInstruction::Halt => {}
        }
        Ok(())
    }

    /// Execute a program made of `SimpleInstruction`s.
    pub fn run_simple(&mut self) -> Result<(), VMError> {
        const MAX_STEPS: u64 = 1_000_000;
        let mut steps = 0u64;
        let _pointer_policy_guard =
            PointerPolicyGuard::install(self.syscall_policy(), self.abi_version());
        loop {
            if steps >= MAX_STEPS {
                return Err(VMError::ExceededMaxCycles);
            }

            let mut block = Vec::new();
            let mut terminal = None;

            loop {
                let (instr, len) = self.decode_next()?;
                self.pc = self.pc.wrapping_add(len as u64);
                if matches!(
                    instr,
                    SimpleInstruction::Jump { .. }
                        | SimpleInstruction::Beq { .. }
                        | SimpleInstruction::Halt
                ) {
                    terminal = Some(instr);
                    break;
                } else {
                    block.push(instr);
                    steps += 1;
                    if steps >= MAX_STEPS {
                        break;
                    }
                }
            }

            if !block.is_empty() {
                self.execute_block_parallel(&block)?;
            }

            if let Some(term) = terminal {
                if matches!(term, SimpleInstruction::Halt) {
                    // In the simple pipeline, require at least one unit of gas to
                    // complete the final step so that programs with N instructions
                    // need >= N gas. This matches edge-case expectations in tests.
                    if self.gas_remaining == 0 {
                        return Err(VMError::OutOfGas);
                    }
                    // HALT itself has zero cost; the gas check above enforces step budget.
                    break;
                }
                self.execute_instruction(term)?;
                steps += 1;
            } else {
                break;
            }
        }
        self.memory.commit();
        Ok(())
    }

    /// Create a new IVM with custom state and optional core count.
    pub fn new_with_options(core_count: Option<usize>, state: State, gas_limit: u64) -> Self {
        let config = IvmConfig::new(gas_limit);
        IVM::new_with_options_and_config(core_count, state, config)
    }

    /// Create a new IVM with custom state using the provided configuration.
    pub fn new_with_options_and_config(
        core_count: Option<usize>,
        state: State,
        config: IvmConfig,
    ) -> Self {
        let mut vm = IVM::new_with_config(config);
        let (min, max) = match core_count {
            Some(n) => (n.max(1), n.max(1)),
            None => crate::parallel::default_scheduler_limits(),
        };
        vm.core_count = min;
        vm.scheduler = std::sync::Arc::new(Scheduler::new_dynamic(min, max));
        vm.state = state;
        vm.htm_supported = cfg!(all(feature = "htm", target_arch = "x86_64")) && rtm_available();
        let scheduler_threads = vm.scheduler.thread_count();
        vm.core_count = scheduler_threads;
        IVM::startup_banner(
            scheduler_threads,
            vm.max_vector_lanes,
            vm.htm_supported,
            vm.hardware_capabilities(),
            vm.use_metal,
            vm.use_cuda,
        );
        vm
    }

    /// Enable or disable zero-knowledge features.
    ///
    /// When enabled and no explicit cycle limit has been set, the default
    /// [`zk::MAX_CYCLES`] value is used. Disabling ZK clears the cycle limit.
    pub fn set_zk_mode(&mut self, enabled: bool) {
        self.zk_mode = enabled;
        if enabled {
            if self.max_cycles == 0 {
                self.max_cycles = zk::MAX_CYCLES;
            }
        } else {
            self.max_cycles = 0;
        }
    }

    /// Set the maximum cycle count used for zero-knowledge padding and enforcement.
    pub fn set_max_cycles(&mut self, max: u64) {
        self.max_cycles = max;
    }

    /// Load raw code bytes into memory without parsing metadata.
    pub fn load_code(&mut self, code: &[u8]) -> Result<(), VMError> {
        if code.len() > Memory::HEAP_START as usize {
            return Err(VMError::MemoryOutOfBounds);
        }
        // Preserve INPUT/STACK contents but reset OUTPUT for a clean run.
        self.memory.load_code(code);
        self.memory.clear_output();
        self.pc = 0;
        self.predecoded = None;
        self.predecoded_index.clear();
        Ok(())
    }

    /// Load a program (bytecode) into the VM's code memory.
    pub fn load_program(&mut self, program: &[u8]) -> Result<(), VMError> {
        let parsed = ProgramMetadata::parse(program)?;
        let header_len = parsed.header_len;
        let literal_prefix = parsed.literal_prefix_len();
        let code_region = &program[header_len..];
        if code_region.len() as u64 > Memory::HEAP_START {
            return Err(VMError::InvalidMetadata);
        }
        if literal_prefix > code_region.len() {
            return Err(VMError::InvalidMetadata);
        }
        let instruction_region = &code_region[literal_prefix..];
        let meta = parsed.metadata;
        self.metadata = meta.clone();
        self.vector_enabled = meta.mode & crate::metadata::mode::VECTOR != 0;
        self.max_cycles = meta.max_cycles;
        self.zk_mode = meta.mode & crate::metadata::mode::ZK != 0;
        if self.zk_mode && self.max_cycles == 0 {
            self.max_cycles = zk::MAX_CYCLES;
        }
        self.vector_length = if meta.vector_length == 0 {
            default_vector_length()
        } else {
            std::cmp::min(meta.vector_length as usize, LOGICAL_VECTOR_MAX)
        };
        // Overlay code region while preserving INPUT/STACK contents that may
        // have been preloaded by the host/tests. OUTPUT is cleared per load.
        self.predecoded = None;
        self.predecoded_index.clear();
        self.memory.load_code(code_region);
        self.memory.clear_output();
        self.registers.set(31, self.memory.stack_top());
        let mut hasher = Sha256::new();
        hasher.update(code_region);
        self.code_hash = hasher.finalize().into();
        self.pc = literal_prefix as u64;
        self.halted = false;
        self.constraint_failed = false;
        self.constraints = zk::ConstraintLog::default();
        self.mem_log = MemLog::default();
        self.reg_log = zk::RegLog::default();
        self.trace_log = DeltaTraceLog::default();
        self.step_log = zk::StepLog::default();
        self.cycles = 0;
        if !instruction_region.is_empty() {
            let decoded =
                crate::ivm_cache::global_get_with_meta(instruction_region, &self.metadata)?;
            for op in decoded.iter() {
                let instr = op.inst;
                let wide_op = instruction::wide::opcode(instr);
                if !instruction::wide::is_valid_opcode(wide_op) {
                    return Err(VMError::InvalidOpcode((instr & 0xFFFF) as u16));
                }
            }
            let mut index = HashMap::with_capacity(decoded.len());
            for (idx, op) in decoded.iter().enumerate() {
                index.insert(op.pc + literal_prefix as u64, idx);
            }
            self.predecoded = Some(decoded);
            self.predecoded_index = index;
        } else {
            self.predecoded = None;
            self.predecoded_index.clear();
        }
        // Recompute the INPUT bump allocator based on any preloaded TLVs so that
        // host allocations append instead of overwriting existing entries.
        self.recompute_input_bump_from_memory();
        if crate::dev_env::decode_trace_enabled() {
            eprintln!(
                "[IVM] input_bump_next set to 0x{off:x}",
                off = self.input_bump_next
            );
        }
        Ok(())
    }

    /// Set the gas limit for execution.
    pub fn set_gas_limit(&mut self, limit: u64) {
        self.gas_remaining = limit;
    }

    /// Access the parsed program metadata for the currently loaded program.
    pub fn metadata(&self) -> &ProgramMetadata {
        &self.metadata
    }

    /// Returns `true` when the VM is executing in zero-knowledge mode.
    #[inline]
    pub fn zk_mode_enabled(&self) -> bool {
        self.zk_mode
    }

    #[inline]
    fn zk_match_tags(&self, rs1: usize, rs2: usize) -> Result<Option<bool>, VMError> {
        if self.zk_mode {
            let tag_a = self.registers.tag(rs1);
            let tag_b = self.registers.tag(rs2);
            if tag_a != tag_b {
                return Err(VMError::PrivacyViolation);
            }
            Ok(Some(tag_a))
        } else {
            Ok(None)
        }
    }

    #[inline]
    fn zk_unary_tag(&self, rs: usize) -> Option<bool> {
        if self.zk_mode {
            Some(self.registers.tag(rs))
        } else {
            None
        }
    }

    #[inline]
    fn zk_apply_tag(&mut self, rd: usize, tag: Option<bool>) {
        if let Some(tag) = tag {
            self.registers.set_tag(rd, tag);
        }
    }

    #[cfg(test)]
    pub(crate) fn reset_predecode_misses(&mut self) {
        self.predecoded_misses = 0;
    }

    #[cfg(test)]
    pub(crate) fn predecode_misses(&self) -> u64 {
        self.predecoded_misses
    }

    /// Returns `true` when CUDA acceleration is enabled for this VM instance.
    pub fn uses_cuda(&self) -> bool {
        self.use_cuda
    }

    /// Returns the acceleration policy currently applied to this VM.
    pub fn acceleration_policy(&self) -> AccelerationPolicy {
        self.acceleration_policy
    }

    /// Returns the hardware accelerators detected on this host.
    pub fn hardware_capabilities(&self) -> HardwareCapabilities {
        self.hardware_capabilities
    }

    /// Override the hardware capabilities snapshot for this VM instance.
    pub fn set_hardware_capabilities(&mut self, capabilities: HardwareCapabilities) {
        self.hardware_capabilities = capabilities;
        self.apply_acceleration_policy(self.acceleration_policy);
    }

    /// Update the acceleration policy and recompute hardware usage flags.
    pub fn set_acceleration_policy(&mut self, policy: AccelerationPolicy) {
        self.apply_acceleration_policy(policy);
    }

    /// Returns `true` when Metal acceleration is enabled for this VM instance.
    pub fn uses_metal(&self) -> bool {
        self.use_metal
    }

    /// Current program counter (byte offset into code region).
    pub fn pc(&self) -> u64 {
        self.pc
    }

    /// Allocate space in the INPUT region and write the provided TLV bytes.
    /// Returns the absolute pointer to the start of the TLV.
    pub fn alloc_input_tlv(&mut self, tlv: &[u8]) -> Result<u64, VMError> {
        // 8-byte alignment between entries to preserve natural alignment of common payloads.
        const ALIGN: u64 = 8;
        let mut off = self.input_bump_next;
        let rem = off % ALIGN;
        if rem != 0 {
            off += ALIGN - rem;
        }
        let end = off
            .checked_add(tlv.len() as u64)
            .ok_or(VMError::MemoryOutOfBounds)?;
        if end > Memory::INPUT_SIZE {
            return Err(VMError::MemoryOutOfBounds);
        }
        self.memory.preload_input(off, tlv)?;
        self.input_bump_next = end;
        Ok(Memory::INPUT_START + off)
    }

    /// Recompute the simple INPUT bump pointer based on existing TLVs.
    ///
    /// Scans the INPUT region from the start and advances `input_bump_next`
    /// past any valid TLVs found. This allows hosts/tests that preloaded TLVs
    /// before `load_program()` to avoid new allocations overwriting them.
    fn recompute_input_bump_from_memory(&mut self) {
        let mut off: u64 = 0;
        loop {
            if off + 7 > Memory::INPUT_SIZE {
                break;
            }
            // Read tentative header; abort on failure
            let hdr = match self.memory.load_region(Memory::INPUT_START + off, 7) {
                Ok(h) => h,
                Err(_) => break,
            };
            // Decode header; stop if it doesn't look like a TLV (unknown type ids are fine)
            let len = u32::from_be_bytes([hdr[3], hdr[4], hdr[5], hdr[6]]) as u64;
            let total = 7u64.saturating_add(len).saturating_add(32);
            if Memory::INPUT_START + off + total > Memory::INPUT_START + Memory::INPUT_SIZE {
                break;
            }
            // Try to validate hash quickly; if it fails, stop scanning
            let ok = self
                .memory
                .load_region(Memory::INPUT_START + off + 7, len)
                .ok()
                .and_then(|payload| {
                    self.memory
                        .load_region(Memory::INPUT_START + off + 7 + len, 32)
                        .ok()
                        .map(|hash| (payload, hash))
                })
                .map(|(payload, hash)| {
                    let mut hb = [0u8; 32];
                    hb.copy_from_slice(hash);
                    let expected: [u8; 32] = iroha_crypto::Hash::new(payload).into();
                    hb == expected
                })
                .unwrap_or(false);
            if !ok {
                break;
            }
            // Advance to next aligned slot
            let mut next = off + total;
            let rem = next % 8;
            if rem != 0 {
                next += 8 - rem;
            }
            off = next;
        }
        self.input_bump_next = off;
    }

    /// Attempt to verify an Ed25519 batch with the CUDA backend.
    ///
    /// Returns:
    /// - `None` if CUDA is disabled or unavailable (caller should fall back to CPU).
    /// - `Some(Ok(()))` if every entry verified successfully on the GPU.
    /// - `Some(Err(index))` if a malformed entry or invalid signature was detected.
    fn verify_ed25519_batch_cuda(
        &self,
        request: &crate::signature::Ed25519BatchRequest,
    ) -> Option<Result<(), usize>> {
        if !self.use_cuda {
            return None;
        }
        for (index, entry) in request.entries.iter().enumerate() {
            let sig_bytes: [u8; 64] = match entry.signature.as_slice().try_into() {
                Ok(bytes) => bytes,
                Err(_) => return Some(Err(index)),
            };
            let pk_bytes: [u8; 32] = match entry.public_key.as_slice().try_into() {
                Ok(bytes) => bytes,
                Err(_) => return Some(Err(index)),
            };
            match crate::cuda::ed25519_verify_cuda(entry.message.as_slice(), &sig_bytes, &pk_bytes)
            {
                Some(true) => continue,
                Some(false) => return Some(Err(index)),
                None => return None,
            }
        }
        Some(Ok(()))
    }

    #[cfg(all(feature = "metal", target_os = "macos"))]
    fn verify_ed25519_batch_metal(
        &self,
        request: &crate::signature::Ed25519BatchRequest,
    ) -> Option<Result<(), usize>> {
        if !self.use_metal || !crate::vector::metal_available() {
            return None;
        }
        use curve25519_dalek::scalar::Scalar;
        use ed25519_dalek::{Signature, VerifyingKey};
        use sha2::{Digest, Sha512};

        let mut sigs = Vec::with_capacity(request.entries.len());
        let mut pks = Vec::with_capacity(request.entries.len());
        let mut hrams = Vec::with_capacity(request.entries.len());

        for (index, entry) in request.entries.iter().enumerate() {
            let sig_bytes: [u8; 64] = match entry.signature.as_slice().try_into() {
                Ok(bytes) => bytes,
                Err(_) => return Some(Err(index)),
            };
            let pk_bytes: [u8; 32] = match entry.public_key.as_slice().try_into() {
                Ok(bytes) => bytes,
                Err(_) => return Some(Err(index)),
            };
            let pk = match VerifyingKey::from_bytes(&pk_bytes) {
                Ok(pk) => pk,
                Err(_) => return Some(Err(index)),
            };
            let mut hasher = Sha512::new();
            hasher.update(&sig_bytes[..32]);
            hasher.update(pk.as_bytes());
            hasher.update(entry.message.as_slice());
            let hram = Scalar::from_hash(hasher).to_bytes();

            // Ensure signature parses to preserve canonical checks before GPU launch.
            if Signature::from_slice(&sig_bytes).is_err() {
                return Some(Err(index));
            }

            sigs.push(sig_bytes);
            pks.push(pk_bytes);
            hrams.push(hram);
        }

        match crate::vector::metal_ed25519_verify_batch(&sigs, &pks, &hrams) {
            Some(results) => {
                for (idx, ok) in results.into_iter().enumerate() {
                    if !ok {
                        return Some(Err(idx));
                    }
                }
                Some(Ok(()))
            }
            None => None,
        }
    }

    /// ABI version extracted from the program header.
    pub fn abi_version(&self) -> u8 {
        self.metadata.abi_version
    }

    /// Effective syscall policy inferred from `abi_version`.
    /// This can be used by hosts to gate syscall ranges or features.
    pub fn syscall_policy(&self) -> SyscallPolicy {
        match self.metadata.abi_version {
            1 => SyscallPolicy::AbiV1,
            v => SyscallPolicy::Experimental(v),
        }
    }

    /// Execute a full block of transactions using the parallel scheduler.
    ///
    /// Each transaction is run on a cloned instance of the VM so worker
    /// threads never share mutable state. When hardware transactional memory is
    /// available the scheduler wraps the entire execution in an RTM region which
    /// eliminates the global mutex normally used for committing updates. The
    /// shared [`State`] uses a `DashMap` internally for thread-safe access so
    /// applying the write sets is safe from multiple threads.
    pub fn execute_block(&mut self, block: Block) -> BlockResult {
        let allow_parallel = self
            .host
            .as_ref()
            .is_none_or(|h| h.supports_concurrent_blocks());
        if !allow_parallel {
            return self.execute_block_sequential(block);
        }

        // Capture the current host (if any) so worker clones can access it via
        // a thread-safe wrapper. The original host is restored before returning
        // to preserve downcast behaviour for the caller.
        let shared_host = self
            .host
            .take()
            .map(|host| Arc::new(Mutex::new(Some(host))));
        let host_for_workers = AssertUnwindSafe(shared_host.as_ref().map(Arc::clone));

        // Clone self once as a template for worker threads. The `State` inside
        // the clone is an `Arc` so all threads operate on the same underlying
        // data.
        let template = Arc::new(Mutex::new(self.clone()));
        let template_ref = &template;
        let host_for_workers_ref = &host_for_workers;

        let cache_id = WORKER_CACHE_ID.fetch_add(1, Ordering::Relaxed);
        let result = parallel::execute_block_predicted(&self.scheduler, block, |tx| {
            WORKER_CACHE.with(|slot| {
                let mut slot = slot.borrow_mut();
                if slot.as_ref().map(|cache| cache.id) != Some(cache_id) {
                    let resources =
                        WorkerResources::new(template_ref, host_for_workers_ref.0.as_ref());
                    *slot = Some(WorkerCache {
                        id: cache_id,
                        resources,
                    });
                }
                let cache = slot.as_mut().expect("worker cache must be initialized");
                cache.resources.execute(tx)
            })
        });

        if let Some(shared_host) = shared_host {
            let mut guard = shared_host.lock().unwrap_or_else(|err| err.into_inner());
            self.host = guard.take();
        }

        result
    }

    /// Sequential block execution path used when the host is not concurrency-safe.
    fn execute_block_sequential(&mut self, block: Block) -> BlockResult {
        let template_memory = self.memory.clone();
        let template_input_bump = self.input_bump_next;
        let mut ctx = ExecutionContext::new();
        let mut tx_results = Vec::with_capacity(block.transactions.len());

        for tx in block.transactions {
            self.memory.reset_from_template(&template_memory);
            self.input_bump_next = template_input_bump;
            ctx.init_for_transaction(&tx, &self.state);
            let snapshot = self.host.as_ref().and_then(|h| h.checkpoint());
            let result = self.execute_transaction(&tx, &mut ctx);
            if result.success {
                self.commit_transaction(&ctx, &tx.access.reg_tags);
            } else if let (Some(snap), Some(host)) = (snapshot, self.host.as_deref_mut()) {
                let _ = host.restore(snap.as_ref());
            }
            tx_results.push(result);
        }

        BlockResult { tx_results }
    }

    /// Execute a single transaction using this VM instance.
    fn execute_transaction(&mut self, tx: &Transaction, _ctx: &mut ExecutionContext) -> TxResult {
        let logging_supported = self
            .host
            .as_ref()
            .is_some_and(|h| h.access_logging_supported());
        if !logging_supported
            && (!tx.access.read_keys.is_empty()
                || !tx.access.write_keys.is_empty()
                || !tx.access.reg_tags.is_empty())
        {
            return TxResult {
                success: false,
                gas_used: 0,
            };
        }

        if let Some(host) = self.host.as_deref_mut()
            && host.begin_tx(&tx.access).is_err()
        {
            return TxResult {
                success: false,
                gas_used: 0,
            };
        }

        if self.load_program(&tx.code).is_err() {
            if let Some(host) = self.host.as_deref_mut() {
                let _ = host.finish_tx();
            }
            return TxResult {
                success: false,
                gas_used: 0,
            };
        }
        self.set_gas_limit(tx.gas_limit);
        self.reset();
        let run_result = self.run();
        let finish_result = if let Some(host) = self.host.as_deref_mut() {
            host.finish_tx()
        } else {
            Ok(AccessLog::default())
        };
        let (access_log, finish_ok) = match finish_result {
            Ok(log) => (log, true),
            Err(_) => (AccessLog::default(), false),
        };
        _ctx.write_set = if finish_ok {
            access_log.state_writes.clone()
        } else {
            Vec::new()
        };
        let usage = self.registers.usage_summary();
        let metrics = iroha_telemetry::metrics::global_or_default();
        metrics
            .ivm_register_max_index
            .observe(usage.max_index as f64);
        metrics
            .ivm_register_unique_count
            .observe(usage.unique_registers as f64);
        self.registers.clear_usage();
        let access_ok = access_log.read_keys.is_subset(&tx.access.read_keys)
            && access_log.write_keys.is_subset(&tx.access.write_keys)
            && access_log.reg_tags.is_subset(&tx.access.reg_tags);
        let success = run_result.is_ok() && access_ok && finish_ok;
        TxResult {
            success,
            gas_used: tx.gas_limit.saturating_sub(self.gas_remaining),
        }
    }

    /// Reset an execution context for reuse.
    #[allow(dead_code)]
    fn reset_context(&mut self, ctx: &mut ExecutionContext) {
        ctx.reset();
    }

    /// Commit a transaction's state updates to the shared state.
    fn commit_transaction(&mut self, ctx: &ExecutionContext, tags: &HashSet<usize>) {
        self.state
            .apply_atomic(&ctx.write_set, tags, self.htm_supported);
    }

    /// Reset the VM state (registers, PC, cycles) but preserve loaded program and host.
    pub fn reset(&mut self) {
        let resume_pc = self.predecoded_index.keys().copied().min().unwrap_or(0);
        self.registers = Registers::new();
        self.pc = resume_pc;
        self.cycles = 0;
        self.halted = false;
        self.constraint_failed = false;
        self.constraints = zk::ConstraintLog::default();
        self.mem_log = MemLog::default();
        self.reg_log = zk::RegLog::default();
        self.trace_log = DeltaTraceLog::default();
        self.step_log = zk::StepLog::default();
        // Gas remaining is not reset here; set_gas_limit should be called if needed.
        self.vector_length = if self.metadata.vector_length == 0 {
            default_vector_length()
        } else {
            usize::from(self.metadata.vector_length).min(LOGICAL_VECTOR_MAX)
        };
        self.branch_predictions = 0;
        self.branch_correct = 0;
        let sz = self.branch_predictor.size();
        self.branch_predictor = crate::branch_predictor::BranchPredictor::new(sz);
    }

    /// Get the value of a general-purpose register.
    pub fn register(&self, idx: usize) -> u64 {
        self.registers.get(idx)
    }

    /// Set the value of a general-purpose register.
    pub fn set_register(&mut self, idx: usize, value: u64) {
        self.registers.set(idx, value);
    }

    /// Request a graceful halt after the current instruction.
    pub fn request_exit(&mut self) {
        self.halted = true;
    }

    /// Request an abort and mark the run as failed.
    pub fn request_abort(&mut self) {
        self.halted = true;
        self.constraint_failed = true;
    }

    /// Get a copy of a vector register (128-bit value as four 32-bit lanes).
    pub fn vector_register(&self, idx: usize) -> [u32; 4] {
        let stride = self.vector_length.max(1);
        let start = Self::VECTOR_BASE + idx * stride;
        let mut out = [0u32; 4];
        if start >= 256 {
            return out;
        }
        let lanes = stride.min(4).min(256 - start);
        for (lane, slot) in out.iter_mut().enumerate().take(lanes) {
            *slot = self.registers.get(start + lane) as u32;
        }
        out
    }

    /// Set the contents of a vector register.
    pub fn set_vector_register(&mut self, idx: usize, values: [u32; 4]) {
        let stride = self.vector_length.max(1);
        let start = Self::VECTOR_BASE + idx * stride;
        if start >= 256 {
            return;
        }
        let lanes = stride.min(4).min(256 - start);
        for (lane, value) in values.iter().enumerate().take(lanes) {
            self.registers.set(start + lane, u64::from(*value));
        }
    }

    /// Maximum vector lanes supported by hardware.
    pub fn max_vector_lanes(&self) -> usize {
        self.max_vector_lanes
    }

    /// Current logical vector length.
    pub fn vector_length(&self) -> usize {
        self.vector_length
    }

    /// Replace the host environment used for syscalls.
    pub fn set_host<H: IVMHost + Send + 'static>(&mut self, host: H) {
        self.host = Some(Box::new(crate::runtime::SyscallDispatcher::new(host)));
    }

    /// Get the remaining gas after execution.
    pub fn remaining_gas(&self) -> u64 {
        self.gas_remaining
    }

    /// Access the underlying host as a type-erased value to support downcasting.
    pub fn host_mut_any(&mut self) -> Option<&mut dyn std::any::Any> {
        self.host.as_deref_mut().map(|h| h.as_any())
    }

    /// Get the SHA-256 hash of the loaded program code.
    pub fn code_hash(&self) -> [u8; 32] {
        self.code_hash
    }

    /// Access the log of memory events collected during the last run.
    pub fn memory_log(&self) -> &[MemEvent] {
        &self.mem_log.events
    }

    /// Access the log of register events collected during the last run.
    pub fn register_log(&self) -> &[zk::RegEvent] {
        &self.reg_log.events
    }

    /// Get the Merkle root of the register file.
    pub fn register_root(&self) -> [u8; 32] {
        *self.registers.merkle_root().as_ref()
    }

    /// Fraction of correctly predicted branches during last run.
    pub fn branch_prediction_accuracy(&self) -> f64 {
        if self.branch_predictions == 0 {
            1.0
        } else {
            self.branch_correct as f64 / self.branch_predictions as f64
        }
    }

    /// Access the register trace collected during the last run when zero-knowledge padding was enabled.
    pub fn register_trace(&self) -> Vec<RegisterState> {
        self.trace_log.expand()
    }

    /// Access per-cycle Merkle roots collected during the last run.
    pub fn step_log(&self) -> &[zk::StepEntry] {
        &self.step_log.steps
    }

    /// Access constraints logged during execution.
    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints.list
    }

    #[inline]
    fn flush_cycle_logs(&mut self, last_logged_cycle: &mut u64) {
        // Cycle-by-cycle trace logging is only required for ZK execution.
        // Leaving it enabled for non-ZK programs (when `max_cycles` is set)
        // makes validation orders of magnitude slower due to per-cycle
        // snapshotting and Merkle proof bookkeeping.
        if !self.zk_mode || self.max_cycles == 0 {
            return;
        }
        while *last_logged_cycle < self.cycles {
            self.trace_log.record(
                self.pc,
                self.registers.snapshot(),
                self.registers.snapshot_tags(),
            );
            self.step_log.record(
                self.pc,
                self.registers.merkle_root(),
                self.memory.current_root(),
            );
            *last_logged_cycle += 1;
        }
    }

    /// Execute the loaded program starting at the current `pc`.
    ///
    /// This is the heart of the VM: a classic fetch‑decode‑execute loop.  For
    /// each instruction we deduct gas, perform the operation and then advance
    /// the program counter by the actual length of the instruction (16 or
    /// 32 bits).  When zero‑knowledge mode is active the register state is
    /// logged on every cycle so that a prover can later reconstruct a trace.
    /// The loop terminates on `HALT` or when an error is encountered.
    pub fn run(&mut self) -> Result<(), VMError> {
        let Some(mut host) = self.host.take() else {
            return Err(VMError::HostUnavailable);
        };
        let result = self.run_with_host_ref(host.as_mut());
        self.host = Some(host);
        result
    }

    /// Execute the loaded program using a borrowed host without storing it in the VM.
    pub fn run_with_host(&mut self, host: &mut dyn IVMHost) -> Result<(), VMError> {
        self.run_with_host_ref(host)
    }

    fn run_with_host_ref(&mut self, host: &mut dyn IVMHost) -> Result<(), VMError> {
        self.halted = false;
        self.constraint_failed = false;
        self.cycles = 0;
        let _pointer_policy_guard =
            PointerPolicyGuard::install(self.syscall_policy(), self.abi_version());
        let _reg_logger_guard = if self.zk_mode {
            Some(zk::RegLoggerGuard::install(&mut self.reg_log))
        } else {
            None
        };
        // Basic instruction-level parallelism: build blocks of simple
        // arithmetic instructions that have no side effects and execute them
        // using the parallel scheduler. Complex instructions are still executed
        // sequentially.
        let mut ilp_block: Vec<SimpleInstruction> = Vec::new();
        if self.zk_mode {
            self.trace_log = DeltaTraceLog::default();
            self.step_log = zk::StepLog::default();
        }
        // Optional pre-decode: build a map from PC to (inst,len) using the canonical decoder once
        // Use global, thread-safe cache keyed by header version when available
        // Optional pre-decode: build a map from PC to (inst,len) using the canonical decoder once
        // Use global, thread-safe cache keyed by header version when available
        let _code_bytes = self.memory.read_code_bytes();
        let _predecoded: Option<(
            Arc<[crate::ivm_cache::DecodedOp]>,
            std::collections::HashMap<u64, usize>,
        )> = None;

        let mut last_logged_cycle = 0;
        // Fetch-Decode-Execute loop
        loop {
            self.flush_cycle_logs(&mut last_logged_cycle);
            // Stop the loop if HALT was executed. When a cycle limit is set we
            // continue executing even after a failed assertion so the trace
            // length is independent of witness values.
            if unlikely(self.halted || (!self.zk_mode && self.constraint_failed)) {
                break;
            }
            // Gracefully stop exactly at end of code: treat as end-of-stream.
            // Do not stop for pc > code_len() so that jumps into non-code
            // regions still fault with execute permission errors.
            if unlikely(self.pc == self.memory.code_len()) {
                break;
            }
            if unlikely(self.max_cycles != 0 && self.cycles >= self.max_cycles) {
                return Err(VMError::ExceededMaxCycles);
            }
            // Decode the next instruction. Prefer pre-decoded mapping when available.
            let (instr, length) = if let Some(decoded) = &self.predecoded {
                if let Some(&idx) = self.predecoded_index.get(&self.pc) {
                    let op = &decoded[idx];
                    (op.inst, op.len)
                } else {
                    #[cfg(test)]
                    {
                        self.predecoded_misses += 1;
                    }
                    #[cfg(any(test, debug_assertions))]
                    eprintln!(
                        "[ivm] predecoded miss: has_predecoded=true contains_pc={} pc=0x{pc:x}",
                        self.predecoded_index.contains_key(&self.pc),
                        pc = self.pc
                    );
                    decoder::decode(&self.memory, self.pc)?
                }
            } else {
                #[cfg(test)]
                {
                    self.predecoded_misses += 1;
                }
                #[cfg(any(test, debug_assertions))]
                eprintln!(
                    "[ivm] predecoded miss: has_predecoded=false contains_pc=false pc=0x{pc:x}",
                    pc = self.pc
                );
                decoder::decode(&self.memory, self.pc)?
            };
            if crate::dev_env::decode_trace_enabled() {
                eprintln!(
                    "pc=0x{pc:08x} instr=0x{w:08x} len={len}",
                    pc = self.pc,
                    w = instr,
                    len = length
                );
            }

            // Parallel execution blocks interfere with trace collection used by
            // zero-knowledge circuits. Disable the ILP scheduler whenever a
            // cycle limit is set so that every instruction executes
            // sequentially and the trace contains one entry per step.
            if self.max_cycles == 0 {
                if let Some(simple) = to_simple(instr) {
                    // Part of a parallelisable block – defer execution.
                    self.pc = self.pc.wrapping_add(length as u64);
                    ilp_block.push(simple);
                    continue;
                } else if !ilp_block.is_empty() {
                    self.execute_block_parallel(&ilp_block)?;
                    self.cycles += ilp_block.len() as u64;
                    ilp_block.clear();
                }
            }

            let opcode_hi = (instr >> 24) as u8;
            let wide_op = instruction::wide::opcode(instr);
            if !instruction::wide::is_valid_opcode(wide_op) {
                return Err(VMError::InvalidOpcode((instr & 0xFFFF) as u16));
            }

            // Determine the gas cost for this operation and deduct it.
            // Scale vector op costs by the current logical vector length.
            // Future: include HTM retry penalties if enabled.
            let cost = gas::cost_of_with_params(instr, self.vector_length, 0)
                .ok_or(VMError::InvalidOpcode((instr & 0xFFFF) as u16))?;
            if unlikely(self.gas_remaining < cost) {
                return Err(VMError::OutOfGas);
            }
            self.gas_remaining -= cost;
            // Execute the instruction
            let opcode = instr & 0x7F;
            if crate::dev_env::decode_trace_enabled() {
                eprintln!("[IVM] opcode_lo=0x{opcode:02x} opcode_hi=0x{opcode_hi:02x}");
            }

            // Dispatch via the canonical wide (8-bit opcode) instruction table. Classic
            // encodings are no longer executable; unknown opcodes trap with `InvalidOpcode`.
            match wide_op {
                instruction::wide::system::GETGAS => {
                    let rd = instruction::wide::rd(instr);
                    self.registers.set(rd, self.gas_remaining);
                    if self.zk_mode {
                        self.registers.set_tag(rd, false);
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::system::SCALL => {
                    let imm8 = instruction::wide::imm8(instr) as u8 as u32;
                    let extra_cost = host.syscall(imm8, self);
                    match extra_cost {
                        Ok(extra) => {
                            if self.gas_remaining < extra {
                                return Err(VMError::OutOfGas);
                            }
                            self.gas_remaining -= extra;
                        }
                        Err(e) => return Err(e),
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::ADD => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let val = self
                        .registers
                        .get(rs1)
                        .wrapping_add(self.registers.get(rs2));
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::SUB => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let val = self
                        .registers
                        .get(rs1)
                        .wrapping_sub(self.registers.get(rs2));
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::AND => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let val = self.registers.get(rs1) & self.registers.get(rs2);
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::OR => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let val = self.registers.get(rs1) | self.registers.get(rs2);
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::XOR => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let val = self.registers.get(rs1) ^ self.registers.get(rs2);
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::SLL => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let val = self.registers.get(rs1) << (self.registers.get(rs2) & 0x3f);
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::SRL => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let val = self.registers.get(rs1) >> (self.registers.get(rs2) & 0x3f);
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::SRA => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let val = ((self.registers.get(rs1) as i64) >> (self.registers.get(rs2) & 0x3f))
                        as u64;
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::MUL => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let val = self
                        .registers
                        .get(rs1)
                        .wrapping_mul(self.registers.get(rs2));
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::MULH => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let a = self.registers.get(rs1) as i64 as i128;
                    let b = self.registers.get(rs2) as i64 as i128;
                    let val = ((a * b) >> 64) as u64;
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::MULHU => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let prod = (self.registers.get(rs1) as u128)
                        .wrapping_mul(self.registers.get(rs2) as u128);
                    let val = (prod >> 64) as u64;
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::MULHSU => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let a = self.registers.get(rs1) as i64 as i128;
                    let b = self.registers.get(rs2) as u64 as i128;
                    let val = ((a * b) >> 64) as u64;
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::DIV => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let num = self.registers.get(rs1) as i64;
                    let denom = self.registers.get(rs2) as i64;
                    if denom == 0 {
                        return Err(VMError::AssertionFailed);
                    }
                    let val = if num == i64::MIN && denom == -1 {
                        num as u64
                    } else {
                        (num / denom) as u64
                    };
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::DIVU => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let denom = self.registers.get(rs2);
                    if denom == 0 {
                        return Err(VMError::AssertionFailed);
                    }
                    let val = self.registers.get(rs1) / denom;
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::REM => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let num = self.registers.get(rs1) as i64;
                    let denom = self.registers.get(rs2) as i64;
                    if denom == 0 {
                        return Err(VMError::AssertionFailed);
                    }
                    let val = if num == i64::MIN && denom == -1 {
                        0
                    } else {
                        (num % denom) as u64
                    };
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::REMU => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let denom = self.registers.get(rs2);
                    if denom == 0 {
                        return Err(VMError::AssertionFailed);
                    }
                    let val = self.registers.get(rs1) % denom;
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::SLT => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let lhs = self.registers.get(rs1) as i64;
                    let rhs = self.registers.get(rs2) as i64;
                    self.registers.set(rd, u64::from(lhs < rhs));
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::SLTU => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let lhs = self.registers.get(rs1);
                    let rhs = self.registers.get(rs2);
                    self.registers.set(rd, u64::from(lhs < rhs));
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::SEQ => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let lhs = self.registers.get(rs1);
                    let rhs = self.registers.get(rs2);
                    self.registers.set(rd, u64::from(lhs == rhs));
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::SNE => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let lhs = self.registers.get(rs1);
                    let rhs = self.registers.get(rs2);
                    self.registers.set(rd, u64::from(lhs != rhs));
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::CMOV => {
                    let rd = instruction::wide::rd(instr);
                    let src = instruction::wide::rs1(instr);
                    let cond = instruction::wide::rs2(instr);
                    if self.zk_mode && self.registers.tag(cond) {
                        return Err(VMError::PrivacyViolation);
                    }
                    if self.registers.get(cond) != 0 {
                        let val = self.registers.get(src);
                        self.registers.set(rd, val);
                        let tag = self.zk_unary_tag(src);
                        self.zk_apply_tag(rd, tag);
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::NEG => {
                    let rd = instruction::wide::rd(instr);
                    let src = instruction::wide::rs1(instr);
                    let tag = self.zk_unary_tag(src);
                    let val = self.registers.get(src).wrapping_neg();
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::NOT => {
                    let rd = instruction::wide::rd(instr);
                    let src = instruction::wide::rs1(instr);
                    let tag = self.zk_unary_tag(src);
                    let val = !self.registers.get(src);
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::ROTL => {
                    let rd = instruction::wide::rd(instr);
                    let src = instruction::wide::rs1(instr);
                    let amt = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(src, amt)?;
                    let sh = (self.registers.get(amt) & 0x3f) as u32;
                    let val = self.registers.get(src).rotate_left(sh);
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::ROTR => {
                    let rd = instruction::wide::rd(instr);
                    let src = instruction::wide::rs1(instr);
                    let amt = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(src, amt)?;
                    let sh = (self.registers.get(amt) & 0x3f) as u32;
                    let val = self.registers.get(src).rotate_right(sh);
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::ROTL_IMM => {
                    let rd = instruction::wide::rd(instr);
                    let src = instruction::wide::rs1(instr);
                    let tag = self.zk_unary_tag(src);
                    let sh = (instruction::wide::imm8(instr) as u8 as u32) & 0x3f;
                    let val = self.registers.get(src).rotate_left(sh);
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::ROTR_IMM => {
                    let rd = instruction::wide::rd(instr);
                    let src = instruction::wide::rs1(instr);
                    let tag = self.zk_unary_tag(src);
                    let sh = (instruction::wide::imm8(instr) as u8 as u32) & 0x3f;
                    let val = self.registers.get(src).rotate_right(sh);
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::POPCNT => {
                    let rd = instruction::wide::rd(instr);
                    let src = instruction::wide::rs1(instr);
                    let tag = self.zk_unary_tag(src);
                    let val = self.registers.get(src).count_ones() as u64;
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::CLZ => {
                    let rd = instruction::wide::rd(instr);
                    let src = instruction::wide::rs1(instr);
                    let tag = self.zk_unary_tag(src);
                    let val = self.registers.get(src).leading_zeros() as u64;
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::CTZ => {
                    let rd = instruction::wide::rd(instr);
                    let src = instruction::wide::rs1(instr);
                    let tag = self.zk_unary_tag(src);
                    let val = self.registers.get(src).trailing_zeros() as u64;
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::ISQRT => {
                    let rd = instruction::wide::rd(instr);
                    let src = instruction::wide::rs1(instr);
                    let tag = self.zk_unary_tag(src);
                    let val = self.registers.get(src);
                    let root = isqrt_u64(val);
                    self.registers.set(rd, root);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 6;
                    continue;
                }
                instruction::wide::arithmetic::MIN => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let a = self.registers.get(rs1) as i64;
                    let b = self.registers.get(rs2) as i64;
                    self.registers
                        .set(rd, if a < b { a as u64 } else { b as u64 });
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::MAX => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let a = self.registers.get(rs1) as i64;
                    let b = self.registers.get(rs2) as i64;
                    self.registers
                        .set(rd, if a > b { a as u64 } else { b as u64 });
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::ABS => {
                    let rd = instruction::wide::rd(instr);
                    let rs = instruction::wide::rs1(instr);
                    let tag = self.zk_unary_tag(rs);
                    let v = self.registers.get(rs) as i64;
                    let abs = v.wrapping_abs();
                    self.registers.set(rd, abs as u64);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::DIV_CEIL => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let num = self.registers.get(rs1) as i64;
                    let denom = self.registers.get(rs2) as i64;
                    let val = div_ceil_i64(num, denom)?;
                    self.registers.set(rd, val as u64);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 12;
                    continue;
                }
                instruction::wide::arithmetic::GCD => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let a = self.registers.get(rs1) as i64;
                    let b = self.registers.get(rs2) as i64;
                    let g = gcd_i64(a, b);
                    self.registers.set(rd, g);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 12;
                    continue;
                }
                instruction::wide::arithmetic::MEAN => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let tag = self.zk_match_tags(rs1, rs2)?;
                    let a = self.registers.get(rs1) as i64;
                    let b = self.registers.get(rs2) as i64;
                    let sum = (a as i128) + (b as i128);
                    let avg = (sum / 2) as i64;
                    self.registers.set(rd, avg as u64);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 3;
                    continue;
                }
                instruction::wide::arithmetic::ADDI => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let tag = self.zk_unary_tag(rs1);
                    let imm = i64::from(i16::from(instruction::wide::imm8(instr)));
                    let val = (self.registers.get(rs1) as i64).wrapping_add(imm) as u64;
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::ANDI => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let tag = self.zk_unary_tag(rs1);
                    let imm = instruction::wide::imm8(instr) as i64 as u64;
                    let val = self.registers.get(rs1) & imm;
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::ORI => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let tag = self.zk_unary_tag(rs1);
                    let imm = instruction::wide::imm8(instr) as i64 as u64;
                    let val = self.registers.get(rs1) | imm;
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::XORI => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let tag = self.zk_unary_tag(rs1);
                    let imm = instruction::wide::imm8(instr) as i64 as u64;
                    let val = self.registers.get(rs1) ^ imm;
                    self.registers.set(rd, val);
                    self.zk_apply_tag(rd, tag);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::arithmetic::CMOVI => {
                    let rd = instruction::wide::rd(instr);
                    let cond = instruction::wide::rs1(instr);
                    let imm = i64::from(instruction::wide::imm8(instr)) as u64;
                    if self.zk_mode && self.registers.tag(cond) {
                        return Err(VMError::PrivacyViolation);
                    }
                    if self.registers.get(cond) != 0 {
                        self.registers.set(rd, imm);
                        if self.zk_mode {
                            self.registers.set_tag(rd, false);
                        }
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::memory::LOAD64 => {
                    let rd = instruction::wide::rd(instr);
                    let base = instruction::wide::rs1(instr);
                    let imm = i64::from(i16::from(instruction::wide::imm8(instr)));
                    if self.zk_mode && self.registers.tag(base) {
                        return Err(VMError::PrivacyViolation);
                    }
                    let addr = (self.registers.get(base) as i64).wrapping_add(imm) as u64;
                    let value = self.memory.load_u64(addr)?;
                    self.registers.set(rd, value);
                    if self.zk_mode {
                        self.registers.set_tag(rd, false);
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::memory::LOAD128 => {
                    if unlikely(!self.vector_enabled) {
                        return Err(VMError::VectorExtensionDisabled);
                    }
                    let rd_lo = instruction::wide::rd(instr);
                    let base = instruction::wide::rs1(instr);
                    let rd_hi = instruction::wide::rs2(instr);
                    if self.zk_mode && self.registers.tag(base) {
                        return Err(VMError::PrivacyViolation);
                    }
                    if rd_lo >= 256 || rd_hi >= 256 {
                        return Err(VMError::RegisterOutOfBounds);
                    }
                    let addr = self.registers.get(base);
                    if !addr.is_multiple_of(16) {
                        return Err(VMError::MisalignedAccess { addr: addr as u32 });
                    }
                    let value = self.memory.load_u128(addr)?;
                    self.registers.set(rd_lo, value as u64);
                    self.registers.set(rd_hi, (value >> 64) as u64);
                    if self.zk_mode {
                        self.registers.set_tag(rd_lo, false);
                        self.registers.set_tag(rd_hi, false);
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::memory::STORE64 => {
                    let base = instruction::wide::rd(instr);
                    let rs = instruction::wide::rs1(instr);
                    let imm = i64::from(i16::from(instruction::wide::imm8(instr)));
                    if self.zk_mode && self.registers.tag(base) {
                        return Err(VMError::PrivacyViolation);
                    }
                    let addr = (self.registers.get(base) as i64).wrapping_add(imm) as u64;
                    let value = self.registers.get(rs);
                    self.memory.store_u64(addr, value)?;
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::memory::STORE128 => {
                    if unlikely(!self.vector_enabled) {
                        return Err(VMError::VectorExtensionDisabled);
                    }
                    let base = instruction::wide::rd(instr);
                    let rs_lo = instruction::wide::rs1(instr);
                    let rs_hi = instruction::wide::rs2(instr);
                    if self.zk_mode && self.registers.tag(base) {
                        return Err(VMError::PrivacyViolation);
                    }
                    if rs_lo >= 256 || rs_hi >= 256 {
                        return Err(VMError::RegisterOutOfBounds);
                    }
                    let addr = self.registers.get(base);
                    if !addr.is_multiple_of(16) {
                        return Err(VMError::MisalignedAccess { addr: addr as u32 });
                    }
                    let lo = self.registers.get(rs_lo);
                    let hi = self.registers.get(rs_hi);
                    let value = ((hi as u128) << 64) | lo as u128;
                    self.memory.store_u128(addr, value)?;
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::control::HALT => {
                    self.halted = true;
                    self.cycles += 1;
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.flush_cycle_logs(&mut last_logged_cycle);
                    break;
                }
                instruction::wide::crypto::SETVL => {
                    if unlikely(!self.vector_enabled) {
                        return Err(VMError::VectorExtensionDisabled);
                    }
                    let vl = {
                        let raw = instruction::wide::rs2(instr);
                        if raw == 0 { 1 } else { raw }
                    };
                    self.vector_length = vl.min(LOGICAL_VECTOR_MAX);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::VADD32 | instruction::wide::crypto::VADD64 => {
                    if unlikely(!self.vector_enabled) {
                        return Err(VMError::VectorExtensionDisabled);
                    }
                    let vl = self.vector_length.max(1);
                    let stride = vl;
                    let rd = Self::VECTOR_BASE + instruction::wide::rd(instr) * stride;
                    let rs = Self::VECTOR_BASE + instruction::wide::rs1(instr) * stride;
                    let rt = Self::VECTOR_BASE + instruction::wide::rs2(instr) * stride;
                    if rd + vl > 256 || rs + vl > 256 || rt + vl > 256 {
                        return Err(VMError::RegisterOutOfBounds);
                    }
                    if wide_op == instruction::wide::crypto::VADD64 {
                        // Process full 128-bit chunks through the vector helpers when possible.
                        let mut lane = 0usize;
                        while lane + 4 <= vl {
                            let mut a = [0u32; 4];
                            let mut b = [0u32; 4];
                            for offset in 0..4 {
                                let idx = lane + offset;
                                if self.zk_mode {
                                    let a_tag = self.registers.tag(rs + idx);
                                    let b_tag = self.registers.tag(rt + idx);
                                    if a_tag != b_tag {
                                        return Err(VMError::PrivacyViolation);
                                    }
                                    self.registers.set_tag(rd + idx, a_tag);
                                }
                                a[offset] = self.registers.get(rs + idx) as u32;
                                b[offset] = self.registers.get(rt + idx) as u32;
                            }

                            // Enforce pair tag alignment for zk mode (lo/hi halves must share a tag).
                            if self.zk_mode {
                                for pair in 0..2 {
                                    let lo = lane + pair * 2;
                                    let hi = lo + 1;
                                    let tag_lo = self.registers.tag(rd + lo);
                                    let tag_hi = self.registers.tag(rd + hi);
                                    if tag_lo != tag_hi {
                                        return Err(VMError::PrivacyViolation);
                                    }
                                }
                            }

                            let sum = vector::vadd64(a, b);
                            for (offset, value) in sum.iter().enumerate() {
                                let idx = lane + offset;
                                self.registers.set(rd + idx, u64::from(*value));
                            }
                            lane += 4;
                        }

                        // Handle any remaining pairs with the scalar fallback.
                        let pairs = (vl - lane) / 2;
                        for pair in 0..pairs {
                            let lo_idx = lane + pair * 2;
                            let hi_idx = lo_idx + 1;
                            let a_lo = self.registers.get(rs + lo_idx) & 0xffff_ffff;
                            let a_hi = self.registers.get(rs + hi_idx) & 0xffff_ffff;
                            let b_lo = self.registers.get(rt + lo_idx) & 0xffff_ffff;
                            let b_hi = self.registers.get(rt + hi_idx) & 0xffff_ffff;
                            if self.zk_mode {
                                let a_lo_tag = self.registers.tag(rs + lo_idx);
                                let a_hi_tag = self.registers.tag(rs + hi_idx);
                                let b_lo_tag = self.registers.tag(rt + lo_idx);
                                let b_hi_tag = self.registers.tag(rt + hi_idx);
                                if a_lo_tag != a_hi_tag
                                    || b_lo_tag != b_hi_tag
                                    || a_lo_tag != b_lo_tag
                                {
                                    return Err(VMError::PrivacyViolation);
                                }
                                self.registers.set_tag(rd + lo_idx, a_lo_tag);
                                self.registers.set_tag(rd + hi_idx, a_lo_tag);
                            }
                            let a = (a_hi << 32) | a_lo;
                            let b = (b_hi << 32) | b_lo;
                            let sum = a.wrapping_add(b);
                            self.registers.set(rd + lo_idx, sum & 0xffff_ffff);
                            self.registers.set(rd + hi_idx, sum >> 32);
                        }
                    } else {
                        let mut lane = 0usize;
                        while lane + 4 <= vl {
                            let mut a = [0u32; 4];
                            let mut b = [0u32; 4];
                            for offset in 0..4 {
                                let idx = lane + offset;
                                if self.zk_mode {
                                    let taga = self.registers.tag(rs + idx);
                                    let tagb = self.registers.tag(rt + idx);
                                    if taga != tagb {
                                        return Err(VMError::PrivacyViolation);
                                    }
                                    self.registers.set_tag(rd + idx, taga);
                                }
                                a[offset] = self.registers.get(rs + idx) as u32;
                                b[offset] = self.registers.get(rt + idx) as u32;
                            }
                            let sum = vector::vadd32(a, b);
                            for (offset, value) in sum.iter().enumerate() {
                                let idx = lane + offset;
                                self.registers.set(rd + idx, u64::from(*value));
                            }
                            lane += 4;
                        }
                        for idx in lane..vl {
                            let a = self.registers.get(rs + idx) as u32;
                            let b = self.registers.get(rt + idx) as u32;
                            if self.zk_mode {
                                let taga = self.registers.tag(rs + idx);
                                let tagb = self.registers.tag(rt + idx);
                                if taga != tagb {
                                    return Err(VMError::PrivacyViolation);
                                }
                                self.registers.set_tag(rd + idx, taga);
                            }
                            self.registers.set(rd + idx, u64::from(a.wrapping_add(b)));
                        }
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::control::BEQ => {
                    let rs = instruction::wide::rd(instr);
                    let rt = instruction::wide::rs1(instr);
                    let offset = i64::from(instruction::wide::imm8(instr));
                    if self.zk_mode && (self.registers.tag(rs) || self.registers.tag(rt)) {
                        return Err(VMError::PrivacyViolation);
                    }
                    let a = self.registers.get(rs);
                    let b = self.registers.get(rt);
                    let predicted = self.branch_predictor.predict(self.pc);
                    let taken = a == b;
                    self.branch_predictions += 1;
                    if likely(predicted == taken) {
                        self.branch_correct += 1;
                    } else {
                        self.cycles += 1;
                    }
                    self.branch_predictor.update(self.pc, taken);
                    if taken {
                        let byte_off = offset as i64 * 4;
                        self.pc = ((self.pc as i64) + byte_off) as u64;
                    } else {
                        self.pc = self.pc.wrapping_add(length as u64);
                    }
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::control::BNE => {
                    let rs = instruction::wide::rd(instr);
                    let rt = instruction::wide::rs1(instr);
                    let offset = i64::from(instruction::wide::imm8(instr));
                    if self.zk_mode && (self.registers.tag(rs) || self.registers.tag(rt)) {
                        return Err(VMError::PrivacyViolation);
                    }
                    let a = self.registers.get(rs);
                    let b = self.registers.get(rt);
                    let predicted = self.branch_predictor.predict(self.pc);
                    let taken = a != b;
                    self.branch_predictions += 1;
                    if likely(predicted == taken) {
                        self.branch_correct += 1;
                    } else {
                        self.cycles += 1;
                    }
                    self.branch_predictor.update(self.pc, taken);
                    if taken {
                        let byte_off = offset as i64 * 4;
                        self.pc = ((self.pc as i64) + byte_off) as u64;
                    } else {
                        self.pc = self.pc.wrapping_add(length as u64);
                    }
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::control::BLT => {
                    let rs = instruction::wide::rd(instr);
                    let rt = instruction::wide::rs1(instr);
                    let offset = i64::from(instruction::wide::imm8(instr));
                    if self.zk_mode && (self.registers.tag(rs) || self.registers.tag(rt)) {
                        return Err(VMError::PrivacyViolation);
                    }
                    let a = self.registers.get(rs) as i64;
                    let b = self.registers.get(rt) as i64;
                    let predicted = self.branch_predictor.predict(self.pc);
                    let taken = a < b;
                    self.branch_predictions += 1;
                    if likely(predicted == taken) {
                        self.branch_correct += 1;
                    } else {
                        self.cycles += 1;
                    }
                    self.branch_predictor.update(self.pc, taken);
                    if taken {
                        let byte_off = offset as i64 * 4;
                        self.pc = ((self.pc as i64) + byte_off) as u64;
                    } else {
                        self.pc = self.pc.wrapping_add(length as u64);
                    }
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::control::BGE => {
                    let rs = instruction::wide::rd(instr);
                    let rt = instruction::wide::rs1(instr);
                    let offset = i64::from(instruction::wide::imm8(instr));
                    if self.zk_mode && (self.registers.tag(rs) || self.registers.tag(rt)) {
                        return Err(VMError::PrivacyViolation);
                    }
                    let a = self.registers.get(rs) as i64;
                    let b = self.registers.get(rt) as i64;
                    let predicted = self.branch_predictor.predict(self.pc);
                    let taken = a >= b;
                    self.branch_predictions += 1;
                    if likely(predicted == taken) {
                        self.branch_correct += 1;
                    } else {
                        self.cycles += 1;
                    }
                    self.branch_predictor.update(self.pc, taken);
                    if taken {
                        let byte_off = offset as i64 * 4;
                        self.pc = ((self.pc as i64) + byte_off) as u64;
                    } else {
                        self.pc = self.pc.wrapping_add(length as u64);
                    }
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::control::BLTU => {
                    let rs = instruction::wide::rd(instr);
                    let rt = instruction::wide::rs1(instr);
                    let offset = i64::from(instruction::wide::imm8(instr));
                    if self.zk_mode && (self.registers.tag(rs) || self.registers.tag(rt)) {
                        return Err(VMError::PrivacyViolation);
                    }
                    let a = self.registers.get(rs);
                    let b = self.registers.get(rt);
                    let predicted = self.branch_predictor.predict(self.pc);
                    let taken = a < b;
                    self.branch_predictions += 1;
                    if likely(predicted == taken) {
                        self.branch_correct += 1;
                    } else {
                        self.cycles += 1;
                    }
                    self.branch_predictor.update(self.pc, taken);
                    if taken {
                        let byte_off = offset as i64 * 4;
                        self.pc = ((self.pc as i64) + byte_off) as u64;
                    } else {
                        self.pc = self.pc.wrapping_add(length as u64);
                    }
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::control::BGEU => {
                    let rs = instruction::wide::rd(instr);
                    let rt = instruction::wide::rs1(instr);
                    let offset = i64::from(instruction::wide::imm8(instr));
                    if self.zk_mode && (self.registers.tag(rs) || self.registers.tag(rt)) {
                        return Err(VMError::PrivacyViolation);
                    }
                    let a = self.registers.get(rs);
                    let b = self.registers.get(rt);
                    let predicted = self.branch_predictor.predict(self.pc);
                    let taken = a >= b;
                    self.branch_predictions += 1;
                    if likely(predicted == taken) {
                        self.branch_correct += 1;
                    } else {
                        self.cycles += 1;
                    }
                    self.branch_predictor.update(self.pc, taken);
                    if taken {
                        let byte_off = offset as i64 * 4;
                        self.pc = ((self.pc as i64) + byte_off) as u64;
                    } else {
                        self.pc = self.pc.wrapping_add(length as u64);
                    }
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::control::JR => {
                    let rs = instruction::wide::rd(instr);
                    if self.zk_mode && self.registers.tag(rs) {
                        return Err(VMError::PrivacyViolation);
                    }
                    self.pc = self.registers.get(rs);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::control::JALR => {
                    let rd = instruction::wide::rd(instr);
                    let rs = instruction::wide::rs1(instr);
                    if self.zk_mode && self.registers.tag(rs) {
                        return Err(VMError::PrivacyViolation);
                    }
                    let imm = i64::from(instruction::wide::imm8(instr));
                    let target = ((self.registers.get(rs) as i64) + imm) as u64 & !3;
                    let return_pc = self.pc.wrapping_add(length as u64);
                    self.registers.set(rd, return_pc);
                    if self.zk_mode {
                        self.registers.set_tag(rd, false);
                    }
                    self.pc = target;
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::control::JAL => {
                    let rd = instruction::wide::rd(instr);
                    let imm = instruction::wide::imm16(instr) as i64;
                    let return_pc = self.pc.wrapping_add(length as u64);
                    self.registers.set(rd, return_pc);
                    if self.zk_mode {
                        self.registers.set_tag(rd, false);
                    }
                    self.pc = ((self.pc as i64) + (imm * 4)) as u64;
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::control::JMP => {
                    let imm = instruction::wide::imm16(instr) as i64;
                    self.pc = ((self.pc as i64) + (imm * 4)) as u64;
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::control::JALS => {
                    let rd = instruction::wide::rd(instr);
                    let imm = instruction::wide::imm16(instr) as i64;
                    let return_pc = self.pc.wrapping_add(length as u64);
                    self.registers.set(rd, return_pc);
                    if self.zk_mode {
                        self.registers.set_tag(rd, false);
                    }
                    self.pc = ((self.pc as i64) + (imm * 4)) as u64;
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::VAND => {
                    if unlikely(!self.vector_enabled) {
                        return Err(VMError::VectorExtensionDisabled);
                    }
                    let vl = self.vector_length.max(1);
                    let stride = vl;
                    let rd = Self::VECTOR_BASE + instruction::wide::rd(instr) * stride;
                    let rs = Self::VECTOR_BASE + instruction::wide::rs1(instr) * stride;
                    let rt = Self::VECTOR_BASE + instruction::wide::rs2(instr) * stride;
                    if rd + vl > 256 || rs + vl > 256 || rt + vl > 256 {
                        return Err(VMError::RegisterOutOfBounds);
                    }
                    let mut lane = 0usize;
                    while lane + 4 <= vl {
                        let mut a = [0u32; 4];
                        let mut b = [0u32; 4];
                        for offset in 0..4 {
                            let idx = lane + offset;
                            if self.zk_mode {
                                let taga = self.registers.tag(rs + idx);
                                let tagb = self.registers.tag(rt + idx);
                                if taga != tagb {
                                    return Err(VMError::PrivacyViolation);
                                }
                                self.registers.set_tag(rd + idx, taga);
                            }
                            a[offset] = self.registers.get(rs + idx) as u32;
                            b[offset] = self.registers.get(rt + idx) as u32;
                        }
                        let out = vector::vand(a, b);
                        for (offset, value) in out.iter().enumerate() {
                            let idx = lane + offset;
                            self.registers.set(rd + idx, u64::from(*value));
                        }
                        lane += 4;
                    }
                    for idx in lane..vl {
                        if self.zk_mode {
                            let taga = self.registers.tag(rs + idx);
                            let tagb = self.registers.tag(rt + idx);
                            if taga != tagb {
                                return Err(VMError::PrivacyViolation);
                            }
                            self.registers.set_tag(rd + idx, taga);
                        }
                        let a = self.registers.get(rs + idx) as u32;
                        let b = self.registers.get(rt + idx) as u32;
                        self.registers.set(rd + idx, u64::from(a & b));
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::VXOR => {
                    if unlikely(!self.vector_enabled) {
                        return Err(VMError::VectorExtensionDisabled);
                    }
                    let vl = self.vector_length.max(1);
                    let stride = vl;
                    let rd = Self::VECTOR_BASE + instruction::wide::rd(instr) * stride;
                    let rs = Self::VECTOR_BASE + instruction::wide::rs1(instr) * stride;
                    let rt = Self::VECTOR_BASE + instruction::wide::rs2(instr) * stride;
                    if rd + vl > 256 || rs + vl > 256 || rt + vl > 256 {
                        return Err(VMError::RegisterOutOfBounds);
                    }
                    let mut lane = 0usize;
                    while lane + 4 <= vl {
                        let mut a = [0u32; 4];
                        let mut b = [0u32; 4];
                        for offset in 0..4 {
                            let idx = lane + offset;
                            if self.zk_mode {
                                let taga = self.registers.tag(rs + idx);
                                let tagb = self.registers.tag(rt + idx);
                                if taga != tagb {
                                    return Err(VMError::PrivacyViolation);
                                }
                                self.registers.set_tag(rd + idx, taga);
                            }
                            a[offset] = self.registers.get(rs + idx) as u32;
                            b[offset] = self.registers.get(rt + idx) as u32;
                        }
                        let out = vector::vxor(a, b);
                        for (offset, value) in out.iter().enumerate() {
                            let idx = lane + offset;
                            self.registers.set(rd + idx, u64::from(*value));
                        }
                        lane += 4;
                    }
                    for idx in lane..vl {
                        if self.zk_mode {
                            let taga = self.registers.tag(rs + idx);
                            let tagb = self.registers.tag(rt + idx);
                            if taga != tagb {
                                return Err(VMError::PrivacyViolation);
                            }
                            self.registers.set_tag(rd + idx, taga);
                        }
                        let a = self.registers.get(rs + idx) as u32;
                        let b = self.registers.get(rt + idx) as u32;
                        self.registers.set(rd + idx, u64::from(a ^ b));
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::VOR => {
                    if unlikely(!self.vector_enabled) {
                        return Err(VMError::VectorExtensionDisabled);
                    }
                    let vl = self.vector_length.max(1);
                    let stride = vl;
                    let rd = Self::VECTOR_BASE + instruction::wide::rd(instr) * stride;
                    let rs = Self::VECTOR_BASE + instruction::wide::rs1(instr) * stride;
                    let rt = Self::VECTOR_BASE + instruction::wide::rs2(instr) * stride;
                    if rd + vl > 256 || rs + vl > 256 || rt + vl > 256 {
                        return Err(VMError::RegisterOutOfBounds);
                    }
                    let mut lane = 0usize;
                    while lane + 4 <= vl {
                        let mut a = [0u32; 4];
                        let mut b = [0u32; 4];
                        for offset in 0..4 {
                            let idx = lane + offset;
                            if self.zk_mode {
                                let taga = self.registers.tag(rs + idx);
                                let tagb = self.registers.tag(rt + idx);
                                if taga != tagb {
                                    return Err(VMError::PrivacyViolation);
                                }
                                self.registers.set_tag(rd + idx, taga);
                            }
                            a[offset] = self.registers.get(rs + idx) as u32;
                            b[offset] = self.registers.get(rt + idx) as u32;
                        }
                        let out = vector::vor(a, b);
                        for (offset, value) in out.iter().enumerate() {
                            let idx = lane + offset;
                            self.registers.set(rd + idx, u64::from(*value));
                        }
                        lane += 4;
                    }
                    for idx in lane..vl {
                        if self.zk_mode {
                            let taga = self.registers.tag(rs + idx);
                            let tagb = self.registers.tag(rt + idx);
                            if taga != tagb {
                                return Err(VMError::PrivacyViolation);
                            }
                            self.registers.set_tag(rd + idx, taga);
                        }
                        let a = self.registers.get(rs + idx) as u32;
                        let b = self.registers.get(rt + idx) as u32;
                        self.registers.set(rd + idx, u64::from(a | b));
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::VROT32 => {
                    if unlikely(!self.vector_enabled) {
                        return Err(VMError::VectorExtensionDisabled);
                    }
                    let vl = self.vector_length.max(1);
                    let stride = vl;
                    let rd = Self::VECTOR_BASE + instruction::wide::rd(instr) * stride;
                    let rs = Self::VECTOR_BASE + instruction::wide::rs1(instr) * stride;
                    if rd + vl > 256 || rs + vl > 256 {
                        return Err(VMError::RegisterOutOfBounds);
                    }
                    let shift = (instruction::wide::imm8(instr) as u8 & 0x1F) as u32;
                    let mut lane = 0usize;
                    while lane + 4 <= vl {
                        let mut a = [0u32; 4];
                        for (offset, value) in a.iter_mut().enumerate() {
                            let idx = lane + offset;
                            if self.zk_mode {
                                let src_tag = self.registers.tag(rs + idx);
                                self.registers.set_tag(rd + idx, src_tag);
                            }
                            *value = self.registers.get(rs + idx) as u32;
                        }
                        let rotated = vector::vrot32(a, shift);
                        for (offset, value) in rotated.iter().enumerate() {
                            let idx = lane + offset;
                            self.registers.set(rd + idx, u64::from(*value));
                        }
                        lane += 4;
                    }
                    for idx in lane..vl {
                        if self.zk_mode {
                            let src_tag = self.registers.tag(rs + idx);
                            self.registers.set_tag(rd + idx, src_tag);
                        }
                        let value = self.registers.get(rs + idx) as u32;
                        let rotated = value.rotate_left(shift) as u64;
                        self.registers.set(rd + idx, rotated);
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::SHA256BLOCK => {
                    if unlikely(!self.vector_enabled) {
                        return Err(VMError::VectorExtensionDisabled);
                    }
                    let vl = self.vector_length.max(1);
                    let stride = vl;
                    let rd = instruction::wide::rd(instr);
                    let ptr_reg = instruction::wide::rs1(instr);
                    if self.zk_mode && self.registers.tag(ptr_reg) {
                        return Err(VMError::PrivacyViolation);
                    }
                    let base = Self::VECTOR_BASE + rd * stride;
                    let second = base + stride;
                    if second + stride > 256 || ptr_reg >= 256 {
                        return Err(VMError::RegisterOutOfBounds);
                    }
                    let addr = self.registers.get(ptr_reg);
                    let mut block = [0u8; 64];
                    self.memory.load_bytes(addr, &mut block)?;
                    if self.zk_mode {
                        for (i, b) in block.iter().enumerate() {
                            let (root, path) = self.memory.merkle_root_and_path(addr + i as u64);
                            self.mem_log.record(MemEvent::Load {
                                addr: addr + i as u64,
                                value: *b as u128,
                                size: 1,
                                path,
                                root,
                            });
                        }
                    }
                    let mut state = [0u32; 8];
                    let first_lanes = vl.min(4);
                    for (lane, slot) in state.iter_mut().take(first_lanes).enumerate() {
                        *slot = self.registers.get(base + lane) as u32;
                    }
                    let second_lanes = vl.min(4);
                    for (lane, slot) in state.iter_mut().skip(4).take(second_lanes).enumerate() {
                        *slot = self.registers.get(second + lane) as u32;
                    }
                    vector::sha256_compress(&mut state, &block);
                    for (lane, value) in state.iter().take(first_lanes).enumerate() {
                        self.registers.set(base + lane, u64::from(*value));
                    }
                    for (lane, value) in state.iter().skip(4).take(second_lanes).enumerate() {
                        self.registers.set(second + lane, u64::from(*value));
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::SHA3BLOCK => {
                    let rd = instruction::wide::rd(instr);
                    let rs_state = instruction::wide::rs1(instr);
                    let rs_block = instruction::wide::rs2(instr);
                    if self.zk_mode
                        && (self.registers.tag(rd)
                            || self.registers.tag(rs_state)
                            || self.registers.tag(rs_block))
                    {
                        return Err(VMError::PrivacyViolation);
                    }
                    if rd >= 256 || rs_state >= 256 || rs_block >= 256 {
                        return Err(VMError::RegisterOutOfBounds);
                    }
                    let state_ptr = self.registers.get(rs_state);
                    let block_ptr = self.registers.get(rs_block);
                    let out_ptr = self.registers.get(rd);
                    let mut state = [0u64; 25];
                    let mut state_bytes = [0u8; 25 * 8];
                    self.memory.load_bytes(state_ptr, &mut state_bytes)?;
                    for i in 0..25 {
                        let mut lane = [0u8; 8];
                        lane.copy_from_slice(&state_bytes[i * 8..(i + 1) * 8]);
                        state[i] = u64::from_le_bytes(lane);
                    }
                    let mut block = [0u8; 136];
                    self.memory.load_bytes(block_ptr, &mut block)?;
                    crate::sha3::sha3_absorb_block(&mut state, &block);
                    let mut out_bytes = [0u8; 25 * 8];
                    for i in 0..25 {
                        out_bytes[i * 8..(i + 1) * 8].copy_from_slice(&state[i].to_le_bytes());
                    }
                    self.memory.store_bytes(out_ptr, &out_bytes)?;
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::AESENC => {
                    if unlikely(!self.vector_enabled) {
                        return Err(VMError::VectorExtensionDisabled);
                    }
                    let rd = instruction::wide::rd(instr);
                    let rs_state = instruction::wide::rs1(instr);
                    let rs_key = instruction::wide::rs2(instr);
                    if rd + 1 >= 256 || rs_state + 1 >= 256 || rs_key + 1 >= 256 {
                        return Err(VMError::RegisterOutOfBounds);
                    }
                    if self.zk_mode {
                        let state_tag_lo = self.registers.tag(rs_state);
                        let state_tag_hi = self.registers.tag(rs_state + 1);
                        let key_tag_lo = self.registers.tag(rs_key);
                        let key_tag_hi = self.registers.tag(rs_key + 1);
                        if state_tag_lo != state_tag_hi
                            || key_tag_lo != key_tag_hi
                            || state_tag_lo != key_tag_lo
                        {
                            return Err(VMError::PrivacyViolation);
                        }
                        self.registers.set_tag(rd, state_tag_lo);
                        self.registers.set_tag(rd + 1, state_tag_lo);
                    }
                    let mut state = [0u8; 16];
                    let s_lo = self.registers.get(rs_state).to_le_bytes();
                    let s_hi = self.registers.get(rs_state + 1).to_le_bytes();
                    state[..8].copy_from_slice(&s_lo);
                    state[8..].copy_from_slice(&s_hi);
                    let mut rk = [0u8; 16];
                    let k_lo = self.registers.get(rs_key).to_le_bytes();
                    let k_hi = self.registers.get(rs_key + 1).to_le_bytes();
                    rk[..8].copy_from_slice(&k_lo);
                    rk[8..].copy_from_slice(&k_hi);
                    let out = crate::aes::aesenc(state, rk);
                    let lo = u64::from_le_bytes(out[..8].try_into().unwrap());
                    let hi = u64::from_le_bytes(out[8..].try_into().unwrap());
                    self.registers.set(rd, lo);
                    self.registers.set(rd + 1, hi);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::AESDEC => {
                    if unlikely(!self.vector_enabled) {
                        return Err(VMError::VectorExtensionDisabled);
                    }
                    let rd = instruction::wide::rd(instr);
                    let rs_state = instruction::wide::rs1(instr);
                    let rs_key = instruction::wide::rs2(instr);
                    if rd + 1 >= 256 || rs_state + 1 >= 256 || rs_key + 1 >= 256 {
                        return Err(VMError::RegisterOutOfBounds);
                    }
                    if self.zk_mode {
                        let state_tag_lo = self.registers.tag(rs_state);
                        let state_tag_hi = self.registers.tag(rs_state + 1);
                        let key_tag_lo = self.registers.tag(rs_key);
                        let key_tag_hi = self.registers.tag(rs_key + 1);
                        if state_tag_lo != state_tag_hi
                            || key_tag_lo != key_tag_hi
                            || state_tag_lo != key_tag_lo
                        {
                            return Err(VMError::PrivacyViolation);
                        }
                        self.registers.set_tag(rd, state_tag_lo);
                        self.registers.set_tag(rd + 1, state_tag_lo);
                    }
                    let mut state = [0u8; 16];
                    let s_lo = self.registers.get(rs_state).to_le_bytes();
                    let s_hi = self.registers.get(rs_state + 1).to_le_bytes();
                    state[..8].copy_from_slice(&s_lo);
                    state[8..].copy_from_slice(&s_hi);
                    let mut rk = [0u8; 16];
                    let k_lo = self.registers.get(rs_key).to_le_bytes();
                    let k_hi = self.registers.get(rs_key + 1).to_le_bytes();
                    rk[..8].copy_from_slice(&k_lo);
                    rk[8..].copy_from_slice(&k_hi);
                    let out = crate::aes::aesdec(state, rk);
                    let lo = u64::from_le_bytes(out[..8].try_into().unwrap());
                    let hi = u64::from_le_bytes(out[8..].try_into().unwrap());
                    self.registers.set(rd, lo);
                    self.registers.set(rd + 1, hi);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::BLAKE2S => {
                    use iroha_crypto::blake2::{Blake2s256, Digest as _};
                    let rd = instruction::wide::rd(instr);
                    let rs = instruction::wide::rs1(instr);
                    if self.zk_mode && self.registers.tag(rs) {
                        return Err(VMError::PrivacyViolation);
                    }
                    if rd + 1 >= 256 || rs >= 256 {
                        return Err(VMError::RegisterOutOfBounds);
                    }
                    let ptr = self.registers.get(rs);
                    let mut input = [0u8; 64];
                    self.memory.load_bytes(ptr, &mut input)?;
                    let mut hasher = Blake2s256::new();
                    hasher.update(input);
                    let digest = hasher.finalize();
                    let mut out = [0u8; 32];
                    out.copy_from_slice(&digest);
                    let lo = u64::from_le_bytes(out[..8].try_into().unwrap());
                    let hi = u64::from_le_bytes(out[8..16].try_into().unwrap());
                    self.registers.set(rd, lo);
                    self.registers.set(rd + 1, hi);
                    if self.zk_mode {
                        self.registers.set_tag(rd, false);
                        self.registers.set_tag(rd + 1, false);
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::POSEIDON2 => {
                    let rd = instruction::wide::rd(instr);
                    let base = instruction::wide::rs1(instr);
                    let imm = i64::from(i16::from(instruction::wide::imm8(instr)));
                    if self.zk_mode && self.registers.tag(base) {
                        return Err(VMError::PrivacyViolation);
                    }
                    if rd >= 256 || base >= 256 {
                        return Err(VMError::RegisterOutOfBounds);
                    }
                    let addr = (self.registers.get(base) as i64).wrapping_add(imm) as u64;
                    let a = self.memory.load_u64(addr)?;
                    let b = self.memory.load_u64(addr + 8)?;
                    if self.zk_mode {
                        let (root, path_a) = self.memory.merkle_root_and_path(addr);
                        let (_root2, path_b) = self.memory.merkle_root_and_path(addr + 8);
                        self.mem_log.record(MemEvent::Load {
                            addr,
                            value: a as u128,
                            size: 8,
                            path: path_a,
                            root,
                        });
                        self.mem_log.record(MemEvent::Load {
                            addr: addr + 8,
                            value: b as u128,
                            size: 8,
                            path: path_b,
                            root,
                        });
                    }
                    let res = crate::poseidon::poseidon2(a, b);
                    self.registers.set(rd, res);
                    if self.zk_mode {
                        self.registers.set_tag(rd, false);
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::POSEIDON6 => {
                    let rd = instruction::wide::rd(instr);
                    let base = instruction::wide::rs1(instr);
                    let imm = i64::from(i16::from(instruction::wide::imm8(instr)));
                    if self.zk_mode && self.registers.tag(base) {
                        return Err(VMError::PrivacyViolation);
                    }
                    if rd >= 256 || base >= 256 {
                        return Err(VMError::RegisterOutOfBounds);
                    }
                    let addr = (self.registers.get(base) as i64).wrapping_add(imm) as u64;
                    let mut bytes = [0u8; 48];
                    self.memory.load_bytes(addr, &mut bytes)?;
                    let mut vals = [0u64; 6];
                    for (i, slot) in vals.iter_mut().enumerate() {
                        let off = i * 8;
                        let word = u64::from_le_bytes(bytes[off..off + 8].try_into().unwrap());
                        if self.zk_mode {
                            let (root, path) = self.memory.merkle_root_and_path(addr + off as u64);
                            self.mem_log.record(MemEvent::Load {
                                addr: addr + off as u64,
                                value: word as u128,
                                size: 8,
                                path,
                                root,
                            });
                        }
                        *slot = word;
                    }
                    let res = crate::poseidon::poseidon6(vals);
                    self.registers.set(rd, res);
                    if self.zk_mode {
                        self.registers.set_tag(rd, false);
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::PUBKGEN => {
                    let rd = instruction::wide::rd(instr);
                    let rs = instruction::wide::rs1(instr);
                    let secret = self.registers.get(rs);
                    let res = crate::field::mul(secret, 2);
                    self.registers.set(rd, res);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::VALCOM => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let value = self.registers.get(rs1);
                    let randomness = self.registers.get(rs2);
                    let res = crate::pedersen::pedersen_commit_truncated(value, randomness);
                    self.registers.set(rd, res);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::ECADD => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let p = self.registers.get(rs1);
                    let q = self.registers.get(rs2);
                    let res = crate::ec::ec_add_truncated(p, q);
                    self.registers.set(rd, res);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::ECMUL_VAR => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let point = self.registers.get(rs1);
                    let scalar = self.registers.get(rs2);
                    let res = crate::ec::ec_mul_truncated(point, scalar);
                    self.registers.set(rd, res);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::PAIRING => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let a = self.registers.get(rs1);
                    let b = self.registers.get(rs2);
                    let res = crate::ec::pairing_check_truncated(a, b);
                    self.registers.set(rd, res);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::ED25519BATCHVERIFY => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    if self.zk_mode && self.registers.tag(rs1) {
                        return Err(VMError::PrivacyViolation);
                    }
                    let mut fail_index = 0u64;
                    let ok = if let Ok(tlv_req) = self.memory.validate_tlv(self.registers.get(rs1))
                    {
                        let is_norito = tlv_req.type_id_raw()
                            == crate::pointer_abi::PointerType::NoritoBytes as u16;
                        is_norito
                            && match norito::decode_from_bytes::<
                                crate::signature::Ed25519BatchRequest,
                            >(tlv_req.payload)
                            {
                                Ok(req) => {
                                    if req.entries.is_empty() {
                                        false
                                    } else if req.entries.len() > Self::MAX_ED25519_BATCH_ENTRIES {
                                        fail_index = req.entries.len() as u64;
                                        false
                                    } else {
                                        let extra_cost = Self::GAS_ED25519_BATCH_PER_ENTRY
                                            .saturating_mul(req.entries.len() as u64);
                                        if self.gas_remaining < extra_cost {
                                            return Err(VMError::OutOfGas);
                                        }
                                        self.gas_remaining -= extra_cost;

                                        #[cfg(all(feature = "metal", target_os = "macos"))]
                                        let batch_result = self
                                            .verify_ed25519_batch_cuda(&req)
                                            .or_else(|| self.verify_ed25519_batch_metal(&req));
                                        #[cfg(not(all(feature = "metal", target_os = "macos")))]
                                        let batch_result = self.verify_ed25519_batch_cuda(&req);

                                        match batch_result {
                                            Some(Ok(())) => true,
                                            Some(Err(index)) => {
                                                fail_index = index as u64;
                                                false
                                            }
                                            None => match crate::signature::verify_ed25519_batch(
                                                &req,
                                                Self::MAX_ED25519_BATCH_ENTRIES,
                                            ) {
                                                Ok(()) => true,
                                                Err(err) => {
                                                    fail_index = match err {
                                                        crate::signature::Ed25519BatchError::Empty => 0,
                                                        crate::signature::Ed25519BatchError::TooMany { actual, .. } => {
                                                            actual as u64
                                                        }
                                                        crate::signature::Ed25519BatchError::InvalidEntry {
                                                            index,
                                                        } => index as u64,
                                                        crate::signature::Ed25519BatchError::SignatureFailed {
                                                            index,
                                                        } => index as u64,
                                                    };
                                                    false
                                                }
                                            },
                                        }
                                    }
                                }
                                Err(_) => false,
                            }
                    } else {
                        false
                    };
                    self.registers.set(rd, if ok { 1 } else { 0 });
                    self.registers.set(rs2, fail_index);
                    if self.zk_mode {
                        self.registers.set_tag(rd, false);
                        self.registers.set_tag(rs2, false);
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::ED25519VERIFY => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    if self.zk_mode
                        && (self.registers.tag(rd)
                            || self.registers.tag(rs1)
                            || self.registers.tag(rs2))
                    {
                        return Err(VMError::PrivacyViolation);
                    }
                    let msg_ptr = self.registers.get(rs1);
                    let sig_ptr = self.registers.get(rs2);
                    let pk_ptr = self.registers.get(rd);
                    let ok = if let (Ok(tlv_msg), Ok(tlv_sig), Ok(tlv_pk)) = (
                        self.memory.validate_tlv(msg_ptr),
                        self.memory.validate_tlv(sig_ptr),
                        self.memory.validate_tlv(pk_ptr),
                    ) {
                        let types_ok = tlv_msg.type_id_raw()
                            == crate::pointer_abi::PointerType::Blob as u16
                            && tlv_sig.type_id_raw()
                                == crate::pointer_abi::PointerType::Blob as u16
                            && tlv_pk.type_id_raw() == crate::pointer_abi::PointerType::Blob as u16;
                        types_ok
                            && crate::signature::verify_signature(
                                crate::signature::SignatureScheme::Ed25519,
                                tlv_msg.payload,
                                tlv_sig.payload,
                                tlv_pk.payload,
                            )
                    } else {
                        false
                    };
                    self.registers.set(rd, if ok { 1 } else { 0 });
                    if self.zk_mode {
                        self.registers.set_tag(rd, false);
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::ECDSAVERIFY => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    if self.zk_mode
                        && (self.registers.tag(rd)
                            || self.registers.tag(rs1)
                            || self.registers.tag(rs2))
                    {
                        return Err(VMError::PrivacyViolation);
                    }
                    let msg_ptr = self.registers.get(rs1);
                    let sig_ptr = self.registers.get(rs2);
                    let pk_ptr = self.registers.get(rd);
                    let ok = if let (Ok(tlv_msg), Ok(tlv_sig), Ok(tlv_pk)) = (
                        self.memory.validate_tlv(msg_ptr),
                        self.memory.validate_tlv(sig_ptr),
                        self.memory.validate_tlv(pk_ptr),
                    ) {
                        let types_ok = tlv_msg.type_id_raw()
                            == crate::pointer_abi::PointerType::Blob as u16
                            && tlv_sig.type_id_raw()
                                == crate::pointer_abi::PointerType::Blob as u16
                            && tlv_pk.type_id_raw() == crate::pointer_abi::PointerType::Blob as u16;
                        types_ok
                            && crate::signature::verify_signature(
                                crate::signature::SignatureScheme::Secp256k1,
                                tlv_msg.payload,
                                tlv_sig.payload,
                                tlv_pk.payload,
                            )
                    } else {
                        false
                    };
                    self.registers.set(rd, if ok { 1 } else { 0 });
                    if self.zk_mode {
                        self.registers.set_tag(rd, false);
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::DILITHIUMVERIFY => {
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    if self.zk_mode
                        && (self.registers.tag(rd)
                            || self.registers.tag(rs1)
                            || self.registers.tag(rs2))
                    {
                        return Err(VMError::PrivacyViolation);
                    }
                    let msg_ptr = self.registers.get(rs1);
                    let sig_ptr = self.registers.get(rs2);
                    let pk_ptr = self.registers.get(rd);
                    let ok = if let (Ok(tlv_msg), Ok(tlv_sig), Ok(tlv_pk)) = (
                        self.memory.validate_tlv(msg_ptr),
                        self.memory.validate_tlv(sig_ptr),
                        self.memory.validate_tlv(pk_ptr),
                    ) {
                        let types_ok = tlv_msg.type_id_raw()
                            == crate::pointer_abi::PointerType::Blob as u16
                            && tlv_sig.type_id_raw()
                                == crate::pointer_abi::PointerType::Blob as u16
                            && tlv_pk.type_id_raw() == crate::pointer_abi::PointerType::Blob as u16;
                        types_ok
                            && crate::signature::verify_signature(
                                crate::signature::SignatureScheme::MlDsa,
                                tlv_msg.payload,
                                tlv_sig.payload,
                                tlv_pk.payload,
                            )
                    } else {
                        false
                    };
                    self.registers.set(rd, if ok { 1 } else { 0 });
                    if self.zk_mode {
                        self.registers.set_tag(rd, false);
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::zk::ASSERT => {
                    if unlikely(!self.zk_mode) {
                        return Err(VMError::ZkExtensionDisabled);
                    }
                    let rs = instruction::wide::rs1(instr);
                    self.constraints.record(Constraint::Zero {
                        reg: rs,
                        cycle: self.cycles,
                    });
                    let value = self.registers.get(rs);
                    if value != 0 {
                        self.constraint_failed = true;
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::zk::ASSERT_EQ => {
                    if unlikely(!self.zk_mode) {
                        return Err(VMError::ZkExtensionDisabled);
                    }
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    self.constraints.record(Constraint::Eq {
                        reg1: rs1,
                        reg2: rs2,
                        cycle: self.cycles,
                    });
                    let v1 = self.registers.get(rs1);
                    let v2 = self.registers.get(rs2);
                    if v1 != v2 {
                        self.constraint_failed = true;
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::zk::FADD => {
                    if unlikely(!self.zk_mode) {
                        return Err(VMError::ZkExtensionDisabled);
                    }
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let v1 = self.registers.get(rs1);
                    let v2 = self.registers.get(rs2);
                    let res = crate::field::add(v1, v2);
                    self.registers.set(rd, res);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::zk::FSUB => {
                    if unlikely(!self.zk_mode) {
                        return Err(VMError::ZkExtensionDisabled);
                    }
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let v1 = self.registers.get(rs1);
                    let v2 = self.registers.get(rs2);
                    let res = crate::field::sub(v1, v2);
                    self.registers.set(rd, res);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::zk::FMUL => {
                    if unlikely(!self.zk_mode) {
                        return Err(VMError::ZkExtensionDisabled);
                    }
                    let rd = instruction::wide::rd(instr);
                    let rs1 = instruction::wide::rs1(instr);
                    let rs2 = instruction::wide::rs2(instr);
                    let v1 = self.registers.get(rs1);
                    let v2 = self.registers.get(rs2);
                    let res = crate::field::mul(v1, v2);
                    self.registers.set(rd, res);
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::zk::FINV => {
                    if unlikely(!self.zk_mode) {
                        return Err(VMError::ZkExtensionDisabled);
                    }
                    let rd = instruction::wide::rd(instr);
                    let rs = instruction::wide::rs1(instr);
                    let value = self.registers.get(rs);
                    match crate::field::inv(value) {
                        Some(inv) => self.registers.set(rd, inv),
                        None => {
                            self.constraint_failed = true;
                        }
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::zk::ASSERT_RANGE => {
                    if unlikely(!self.zk_mode) {
                        return Err(VMError::ZkExtensionDisabled);
                    }
                    let rs = instruction::wide::rs1(instr);
                    let imm = instruction::wide::imm8(instr) as u8;
                    self.constraints.record(Constraint::Range {
                        reg: rs,
                        bits: imm,
                        cycle: self.cycles,
                    });
                    if imm <= 64 {
                        let mask = if imm == 64 {
                            u64::MAX
                        } else {
                            (1u64 << imm) - 1
                        };
                        let value = self.registers.get(rs);
                        if value & !mask != 0 {
                            self.constraint_failed = true;
                        }
                    } else {
                        self.constraint_failed = true;
                    }
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                instruction::wide::crypto::PARBEGIN | instruction::wide::crypto::PAREND => {
                    // Parallel sections are markers; no state change required.
                    self.pc = self.pc.wrapping_add(length as u64);
                    self.cycles += 1;
                    continue;
                }
                _ => {
                    if crate::dev_env::debug_invalid_enabled() {
                        eprintln!(
                            "[invalid wide opcode] pc=0x{pc:08x} instr=0x{ins:08x} op_lo=0x{op_lo:02x} op_hi=0x{op_hi:02x}",
                            pc = self.pc,
                            ins = instr,
                            op_lo = opcode,
                            op_hi = wide_op,
                        );
                    }
                    return Err(VMError::InvalidOpcode((instr & 0xFFFF) as u16));
                }
            }
        }

        if !ilp_block.is_empty() {
            self.execute_block_parallel(&ilp_block)?;
            self.cycles += ilp_block.len() as u64;
        }
        // If we exit the loop early, pad the trace so that prover and verifier
        // observe exactly `max_cycles` steps when zero‑knowledge mode is enabled.
        if self.zk_mode && self.max_cycles != 0 && self.cycles < self.max_cycles {
            // Append dummy cycles (treated as NOPs) until the target length is
            // reached. Each padded cycle still costs one unit of gas.
            let remaining = self.max_cycles - self.cycles;
            self.cycles = self.max_cycles;
            // Padding instructions still consume gas like NOPs (cost 1 each)
            if self.gas_remaining < remaining {
                return Err(VMError::OutOfGas);
            } else {
                self.gas_remaining -= remaining;
            }
            self.flush_cycle_logs(&mut last_logged_cycle);
        }
        self.memory.commit();
        if self.constraint_failed {
            Err(VMError::AssertionFailed)
        } else {
            Ok(())
        }
    }

    /// Get the number of cycles executed in the last `run()`.
    #[inline]
    pub fn get_cycle_count(&self) -> u64 {
        self.cycles
    }

    /// Test‑oriented helper: set a GPR directly.
    #[inline]
    pub fn set_reg(&mut self, idx: usize, value: u64) {
        self.registers.set(idx, value);
    }

    /// Convenience wrapper that forwards to the memory subsystem.
    #[inline]
    pub fn store_u8(&mut self, addr: u64, byte: u8) -> Result<(), VMError> {
        self.memory.store_u8(addr, byte)
    }

    #[inline]
    pub fn store_u32(&mut self, addr: u64, value: u32) -> Result<(), VMError> {
        self.memory.store_u32(addr, value)
    }

    #[inline]
    pub fn store_u64(&mut self, addr: u64, value: u64) -> Result<(), VMError> {
        self.memory.store_u64(addr, value)
    }

    #[inline]
    pub fn store_u128(&mut self, addr: u64, value: u128) -> Result<(), VMError> {
        self.memory.store_u128(addr, value)
    }

    #[inline]
    pub fn store_bytes(&mut self, addr: u64, bytes: &[u8]) -> Result<(), VMError> {
        self.memory.store_bytes(addr, bytes)
    }

    #[inline]
    pub fn load_u32(&self, addr: u64) -> Result<u32, VMError> {
        self.memory.load_u32(addr)
    }

    #[inline]
    /// Load a 64-bit value from memory.
    ///
    /// This is a thin wrapper around the underlying memory subsystem, exposed
    /// on `IVM` for convenience by external users (e.g., executor host code)
    /// that interact with VM memory via the VM handle.
    pub fn load_u64(&self, addr: u64) -> Result<u64, VMError> {
        self.memory.load_u64(addr)
    }

    #[inline]
    pub fn load_u128(&self, addr: u64) -> Result<u128, VMError> {
        self.memory.load_u128(addr)
    }

    #[inline]
    pub fn load_bytes(&self, addr: u64, out: &mut [u8]) -> Result<(), VMError> {
        self.memory.load_bytes(addr, out)
    }

    #[inline]
    pub fn preload_input(&mut self, offset: u64, data: &[u8]) -> Result<(), VMError> {
        self.memory.preload_input(offset, data)
    }

    #[inline]
    pub fn alloc_heap(&mut self, size: u64) -> Result<u64, VMError> {
        self.memory.alloc(size)
    }

    #[inline]
    pub fn grow_heap(&mut self, additional: u64) -> Result<u64, VMError> {
        self.memory.grow_heap(additional)
    }

    /// Retrieve a copy of the output region. Intended for use by hosts after
    /// `SYSCALL_COMMIT_OUTPUT`.
    #[inline]
    pub fn read_output(&self) -> &[u8] {
        self.memory.read_output()
    }

    /// Produce a CompactProofBundle for the memory chunk containing `addr` by
    /// invoking the GET_MERKLE_COMPACT syscall via the host. This helper writes
    /// temporary data into the OUTPUT region.
    pub fn get_memory_compact_bundle(
        &mut self,
        addr: u64,
        depth_cap: Option<usize>,
    ) -> Result<crate::merkle_utils::CompactProofBundle, VMError> {
        let out_ptr = Memory::OUTPUT_START;
        let root_out = out_ptr + 8192;
        // Set syscall arguments in registers
        self.set_register(10, addr);
        self.set_register(11, out_ptr);
        self.set_register(12, depth_cap.unwrap_or(0) as u64);
        self.set_register(13, root_out);
        // Gate by policy and invoke host handler directly
        let num = crate::syscalls::SYSCALL_GET_MERKLE_COMPACT;
        if !crate::syscalls::is_syscall_allowed(self.syscall_policy(), num) {
            return Err(VMError::UnknownSyscall(num));
        }
        {
            let mut host = match self.host.take() {
                Some(h) => h,
                None => return Err(VMError::HostUnavailable),
            };
            let _ = host.syscall(num, self)?;
            self.host = Some(host);
        }
        // Parse header and decode typed compact proof
        let mut hdr = [0u8; 1 + 4 + 4];
        self.memory.load_bytes(out_ptr, &mut hdr)?;
        let depth = hdr[0] as usize;
        let total = 1 + 4 + 4 + depth * 32;
        let mut buf = vec![0u8; total];
        self.memory.load_bytes(out_ptr, &mut buf)?;
        let (cp, _) = crate::merkle_utils::decode_compact_proof_bytes(&buf)
            .map_err(|_| VMError::DecodeError)?;
        // Read root
        let mut root = [0u8; 32];
        self.memory.load_bytes(root_out, &mut root)?;
        // Build bundle
        let siblings: Vec<[u8; 32]> = cp
            .siblings()
            .iter()
            .map(|opt| opt.map(|h| *h.as_ref()).unwrap_or([0u8; 32]))
            .collect();
        Ok(crate::merkle_utils::CompactProofBundle {
            depth: cp.depth(),
            dirs: cp.dirs(),
            siblings,
            root,
        })
    }

    /// Produce a CompactProofBundle for the register leaf at `idx` by invoking
    /// the GET_REGISTER_MERKLE_COMPACT syscall via the host. This helper writes
    /// temporary data into the OUTPUT region.
    pub fn get_registers_compact_bundle(
        &mut self,
        idx: usize,
        depth_cap: Option<usize>,
    ) -> Result<crate::merkle_utils::CompactProofBundle, VMError> {
        let out_ptr = Memory::OUTPUT_START;
        let root_out = out_ptr + 12288;
        // Set syscall arguments in registers
        self.set_register(10, idx as u64);
        self.set_register(11, out_ptr);
        self.set_register(12, depth_cap.unwrap_or(0) as u64);
        self.set_register(13, root_out);
        // Gate by policy and invoke host handler directly
        let num = crate::syscalls::SYSCALL_GET_REGISTER_MERKLE_COMPACT;
        if !crate::syscalls::is_syscall_allowed(self.syscall_policy(), num) {
            return Err(VMError::UnknownSyscall(num));
        }
        {
            let mut host = match self.host.take() {
                Some(h) => h,
                None => return Err(VMError::HostUnavailable),
            };
            let _ = host.syscall(num, self)?;
            self.host = Some(host);
        }
        // Parse header and decode typed compact proof
        let mut hdr = [0u8; 1 + 4 + 4];
        self.memory.load_bytes(out_ptr, &mut hdr)?;
        let depth = hdr[0] as usize;
        let total = 1 + 4 + 4 + depth * 32;
        let mut buf = vec![0u8; total];
        self.memory.load_bytes(out_ptr, &mut buf)?;
        let (cp, _) = crate::merkle_utils::decode_compact_proof_bytes(&buf)
            .map_err(|_| VMError::DecodeError)?;
        // Read root
        let mut root = [0u8; 32];
        self.memory.load_bytes(root_out, &mut root)?;
        // Build bundle
        let siblings: Vec<[u8; 32]> = cp
            .siblings()
            .iter()
            .map(|opt| opt.map(|h| *h.as_ref()).unwrap_or([0u8; 32]))
            .collect();
        Ok(crate::merkle_utils::CompactProofBundle {
            depth: cp.depth(),
            dirs: cp.dirs(),
            siblings,
            root,
        })
    }
}

#[cfg(test)]
mod ivm_sched_tests {
    use super::*;

    #[test]
    fn respects_global_scheduler_limits() {
        // Set a small scheduler size and verify IVM::new picks it up.
        crate::parallel::set_default_scheduler_limits(Some(2), Some(2));
        let vm = IVM::new(0);
        assert_eq!(vm.scheduler.thread_count(), 2);
        assert_eq!(vm.core_count, 2);
        // Reset to auto for other tests
        crate::parallel::set_default_scheduler_limits(None, None);
    }
}

/// Try to translate a full 32-bit instruction into a [`SimpleInstruction`].
///
/// Only a subset of arithmetic operations are supported for use with the
/// parallel instruction scheduler. Unsupported instructions return `None` so
/// that the interpreter falls back to sequential execution.
fn to_simple(instr: u32) -> Option<SimpleInstruction> {
    // Prefer the wide encoding when the high byte matches a known opcode.
    {
        use instruction::wide;
        let op = wide::opcode(instr);
        let rd = wide::rd(instr) as u16;
        let rs1 = wide::rs1(instr) as u16;
        let rs2 = wide::rs2(instr) as u16;
        match op {
            wide::arithmetic::ADD => {
                return Some(SimpleInstruction::Add {
                    rd,
                    rs: rs1,
                    rt: rs2,
                });
            }
            wide::arithmetic::SUB => {
                return Some(SimpleInstruction::Sub {
                    rd,
                    rs: rs1,
                    rt: rs2,
                });
            }
            wide::arithmetic::AND => {
                return Some(SimpleInstruction::And {
                    rd,
                    rs: rs1,
                    rt: rs2,
                });
            }
            wide::arithmetic::OR => {
                return Some(SimpleInstruction::Or {
                    rd,
                    rs: rs1,
                    rt: rs2,
                });
            }
            wide::arithmetic::XOR => {
                return Some(SimpleInstruction::Xor {
                    rd,
                    rs: rs1,
                    rt: rs2,
                });
            }
            wide::arithmetic::SLL => {
                return Some(SimpleInstruction::Sll {
                    rd,
                    rs: rs1,
                    rt: rs2,
                });
            }
            wide::arithmetic::SRL => {
                return Some(SimpleInstruction::Srl {
                    rd,
                    rs: rs1,
                    rt: rs2,
                });
            }
            wide::arithmetic::SRA => {
                return Some(SimpleInstruction::Sra {
                    rd,
                    rs: rs1,
                    rt: rs2,
                });
            }
            wide::arithmetic::ADDI => {
                let imm = wide::imm8(instr) as i16;
                if imm >= 0 {
                    return Some(SimpleInstruction::AddImm { rd, rs: rs1, imm });
                } else {
                    return Some(SimpleInstruction::SubImm {
                        rd,
                        rs: rs1,
                        imm: -imm,
                    });
                }
            }
            wide::memory::LOAD64 => {
                let offset = instruction::wide::imm8(instr);
                return Some(SimpleInstruction::Load {
                    rd,
                    addr_reg: rs1,
                    offset,
                });
            }
            wide::memory::STORE64 => {
                let offset = instruction::wide::imm8(instr);
                return Some(SimpleInstruction::Store {
                    rs: rs1,
                    addr_reg: rd,
                    offset,
                });
            }
            wide::crypto::SETVL => {
                return Some(SimpleInstruction::SetVL { new_vl: rs2 });
            }
            wide::crypto::VADD32 => {
                return Some(SimpleInstruction::Vadd {
                    rd,
                    rs: rs1,
                    rt: rs2,
                });
            }
            wide::control::HALT => {
                return None;
            }
            _ => {}
        }
    }

    None
}

/// Metadata describing register and memory accesses of an instruction
#[derive(Default, Clone)]
struct InstrMeta {
    reads: HashSet<u16>,
    writes: HashSet<u16>,
    mem_read: bool,
    mem_write: bool,
}

/// Result of executing a single instruction in parallel
#[derive(Clone, Debug)]
enum ResultUpdate {
    Reg { index: u16, value: u64, tag: bool },
    Mem { addr: u64, value: u64 },
    Vl { value: usize },
}

fn analyse_instruction(
    instr: &SimpleInstruction,
    vl: usize,
    max_vector_lanes: usize,
) -> (InstrMeta, usize) {
    use SimpleInstruction::*;
    let mut meta = InstrMeta::default();
    let mut next_vl = vl;
    match *instr {
        Add { rd, rs, rt }
        | Sub { rd, rs, rt }
        | And { rd, rs, rt }
        | Or { rd, rs, rt }
        | Xor { rd, rs, rt } => {
            meta.reads.insert(rs);
            meta.reads.insert(rt);
            meta.writes.insert(rd);
        }
        Sll { rd, rs, rt } | Srl { rd, rs, rt } | Sra { rd, rs, rt } => {
            meta.reads.insert(rs);
            meta.reads.insert(rt);
            meta.writes.insert(rd);
        }
        AddImm { rd, rs, .. } | SubImm { rd, rs, .. } => {
            meta.reads.insert(rs);
            meta.writes.insert(rd);
        }
        Load {
            rd,
            addr_reg,
            offset: _,
        } => {
            meta.reads.insert(addr_reg);
            meta.writes.insert(rd);
            meta.mem_read = true;
        }
        Store {
            rs,
            addr_reg,
            offset: _,
        } => {
            meta.reads.insert(addr_reg);
            meta.reads.insert(rs);
            meta.mem_write = true;
        }
        Beq { rs, rt, .. } => {
            meta.reads.insert(rs);
            meta.reads.insert(rt);
        }
        Sha256 { dest, .. } => {
            meta.writes.extend([dest, dest + 1, dest + 2, dest + 3]);
            meta.mem_read = true;
        }
        Ed25519Verify { result_reg, .. } => {
            meta.writes.insert(result_reg);
            meta.mem_read = true;
        }
        DilithiumVerify { result_reg, .. } => {
            meta.writes.insert(result_reg);
            meta.mem_read = true;
        }
        SetVL { new_vl } => {
            meta.mem_write = true; // force ordering
            let tmp = if new_vl == 0 { 1 } else { new_vl as usize };
            next_vl = tmp.min(max_vector_lanes);
        }
        Vadd { rd, rs, rt } => {
            let stride = vl as u16;
            let base = crate::IVM::VECTOR_BASE as u16;
            for i in 0..vl as u16 {
                let offset = base + i;
                meta.reads.insert(rs * stride + offset);
                meta.reads.insert(rt * stride + offset);
                meta.writes.insert(rd * stride + offset);
            }
        }
        Jump { .. } | Halt => {}
    }
    (meta, next_vl)
}

fn cost_of(instr: &SimpleInstruction, vl: usize) -> u64 {
    match instr {
        SimpleInstruction::Add { .. }
        | SimpleInstruction::Sub { .. }
        | SimpleInstruction::And { .. }
        | SimpleInstruction::Or { .. }
        | SimpleInstruction::AddImm { .. }
        | SimpleInstruction::SubImm { .. }
        | SimpleInstruction::Xor { .. }
        | SimpleInstruction::Sll { .. }
        | SimpleInstruction::Srl { .. }
        | SimpleInstruction::Sra { .. } => IVM::GAS_ALU,
        SimpleInstruction::Load { .. } | SimpleInstruction::Store { .. } => IVM::GAS_MEM,
        SimpleInstruction::Jump { .. } | SimpleInstruction::Beq { .. } => IVM::GAS_JUMP,
        SimpleInstruction::Sha256 { len, .. } => {
            IVM::GAS_SHA256_BASE + IVM::GAS_SHA256_PER_BYTE * len
        }
        SimpleInstruction::SetVL { .. } => IVM::GAS_ALU,
        SimpleInstruction::Vadd { .. } => {
            let lanes = vl.clamp(1, GAS_VECTOR_BASE_LANES);
            IVM::GAS_ALU * lanes as u64
        }
        SimpleInstruction::Ed25519Verify { .. } => IVM::GAS_ED25519_VERIFY,
        SimpleInstruction::DilithiumVerify { .. } => IVM::GAS_DILITHIUM_VERIFY,
        SimpleInstruction::Halt => 0,
    }
}

fn compute_instruction(
    instr: SimpleInstruction,
    regs: &[u64; 256],
    tags: &[bool; 256],
    mem: &Memory,
    zk: bool,
    vl: usize,
    vector_enabled: bool,
) -> Result<Vec<ResultUpdate>, VMError> {
    use SimpleInstruction::*;
    let mut res = Vec::with_capacity(2);
    match instr {
        Add { rd, rs, rt } => {
            if zk && tags[rs as usize] != tags[rt as usize] {
                return Err(VMError::PrivacyViolation);
            }
            let sum = regs[rs as usize].wrapping_add(regs[rt as usize]);
            let tag = if zk { tags[rs as usize] } else { false };
            res.push(ResultUpdate::Reg {
                index: rd,
                value: sum,
                tag,
            });
        }

        Sub { rd, rs, rt } => {
            if zk && tags[rs as usize] != tags[rt as usize] {
                return Err(VMError::PrivacyViolation);
            }
            let diff = regs[rs as usize].wrapping_sub(regs[rt as usize]);
            let tag = if zk { tags[rs as usize] } else { false };
            res.push(ResultUpdate::Reg {
                index: rd,
                value: diff,
                tag,
            });
        }
        And { rd, rs, rt } => {
            if zk && tags[rs as usize] != tags[rt as usize] {
                return Err(VMError::PrivacyViolation);
            }
            let val = regs[rs as usize] & regs[rt as usize];
            let tag = if zk { tags[rs as usize] } else { false };
            res.push(ResultUpdate::Reg {
                index: rd,
                value: val,
                tag,
            });
        }
        Or { rd, rs, rt } => {
            if zk && tags[rs as usize] != tags[rt as usize] {
                return Err(VMError::PrivacyViolation);
            }
            let val = regs[rs as usize] | regs[rt as usize];
            let tag = if zk { tags[rs as usize] } else { false };
            res.push(ResultUpdate::Reg {
                index: rd,
                value: val,
                tag,
            });
        }
        AddImm { rd, rs, imm } => {
            let val = regs[rs as usize].wrapping_add(imm as i64 as u64);
            let tag = if zk { tags[rs as usize] } else { false };
            res.push(ResultUpdate::Reg {
                index: rd,
                value: val,
                tag,
            });
        }
        SubImm { rd, rs, imm } => {
            let val = regs[rs as usize].wrapping_sub(imm as i64 as u64);
            let tag = if zk { tags[rs as usize] } else { false };
            res.push(ResultUpdate::Reg {
                index: rd,
                value: val,
                tag,
            });
        }
        Xor { rd, rs, rt } => {
            if zk && tags[rs as usize] != tags[rt as usize] {
                return Err(VMError::PrivacyViolation);
            }
            let val = regs[rs as usize] ^ regs[rt as usize];
            let tag = if zk { tags[rs as usize] } else { false };
            res.push(ResultUpdate::Reg {
                index: rd,
                value: val,
                tag,
            });
        }
        Sll { rd, rs, rt } => {
            let sh = regs[rt as usize] & 0x3F;
            let val = regs[rs as usize] << sh;
            if zk && tags[rs as usize] != tags[rt as usize] {
                return Err(VMError::PrivacyViolation);
            }
            let tag = if zk { tags[rs as usize] } else { false };
            res.push(ResultUpdate::Reg {
                index: rd,
                value: val,
                tag,
            });
        }
        Srl { rd, rs, rt } => {
            let sh = regs[rt as usize] & 0x3F;
            let val = regs[rs as usize] >> sh;
            if zk && tags[rs as usize] != tags[rt as usize] {
                return Err(VMError::PrivacyViolation);
            }
            let tag = if zk { tags[rs as usize] } else { false };
            res.push(ResultUpdate::Reg {
                index: rd,
                value: val,
                tag,
            });
        }
        Sra { rd, rs, rt } => {
            let sh = (regs[rt as usize] & 0x3F) as u32;
            let val = ((regs[rs as usize] as i64) >> sh) as u64;
            if zk && tags[rs as usize] != tags[rt as usize] {
                return Err(VMError::PrivacyViolation);
            }
            let tag = if zk { tags[rs as usize] } else { false };
            res.push(ResultUpdate::Reg {
                index: rd,
                value: val,
                tag,
            });
        }
        Load {
            rd,
            addr_reg,
            offset,
        } => {
            if zk && tags[addr_reg as usize] {
                return Err(VMError::PrivacyViolation);
            }
            let base = regs[addr_reg as usize] as i64;
            let addr = base.wrapping_add(offset as i64) as u64;
            let value = mem.load_u64(addr)?;
            res.push(ResultUpdate::Reg {
                index: rd,
                value,
                tag: false,
            });
        }
        Store {
            rs,
            addr_reg,
            offset,
        } => {
            if zk && tags[addr_reg as usize] {
                return Err(VMError::PrivacyViolation);
            }
            let base = regs[addr_reg as usize] as i64;
            let addr = base.wrapping_add(offset as i64) as u64;
            let value = regs[rs as usize];
            res.push(ResultUpdate::Mem { addr, value });
        }
        Sha256 {
            dest,
            src_addr,
            len,
        } => {
            let data = mem.load_region(src_addr, len)?;
            use sha2::{Digest, Sha256};
            let digest = Sha256::digest(data);
            for i in 0..4u16 {
                let mut chunk = [0u8; 8];
                chunk.copy_from_slice(&digest[i as usize * 8..(i as usize + 1) * 8]);
                let val = u64::from_le_bytes(chunk);
                res.push(ResultUpdate::Reg {
                    index: dest + i,
                    value: val,
                    tag: false,
                });
            }
        }
        Ed25519Verify {
            pubkey_addr,
            sig_addr,
            msg_addr,
            msg_len,
            result_reg,
        } => {
            use ed25519_dalek::{Signature, VerifyingKey};
            let pk_slice = mem.load_region(pubkey_addr, 32)?;
            let sig_slice = mem.load_region(sig_addr, 64)?;
            let msg = mem.load_region(msg_addr, msg_len)?;
            let pk_bytes: [u8; 32] = match pk_slice.try_into() {
                Ok(b) => b,
                Err(_) => {
                    res.push(ResultUpdate::Reg {
                        index: result_reg,
                        value: 0,
                        tag: false,
                    });
                    return Ok(res);
                }
            };
            let sig_bytes_arr: [u8; 64] = match sig_slice.try_into() {
                Ok(b) => b,
                Err(_) => {
                    res.push(ResultUpdate::Reg {
                        index: result_reg,
                        value: 0,
                        tag: false,
                    });
                    return Ok(res);
                }
            };
            let pk = match VerifyingKey::from_bytes(&pk_bytes) {
                Ok(k) => k,
                Err(_) => {
                    res.push(ResultUpdate::Reg {
                        index: result_reg,
                        value: 0,
                        tag: false,
                    });
                    return Ok(res);
                }
            };
            let sig = match Signature::from_slice(&sig_bytes_arr) {
                Ok(s) => s,
                Err(_) => {
                    res.push(ResultUpdate::Reg {
                        index: result_reg,
                        value: 0,
                        tag: false,
                    });
                    return Ok(res);
                }
            };
            let valid = pk.verify_strict(msg, &sig).is_ok();
            res.push(ResultUpdate::Reg {
                index: result_reg,
                value: if valid { 1 } else { 0 },
                tag: false,
            });
        }
        DilithiumVerify {
            level,
            pubkey_addr,
            sig_addr,
            msg_addr,
            msg_len,
            result_reg,
        } => {
            use pqcrypto_dilithium::{dilithium2, dilithium3, dilithium5};
            use pqcrypto_traits::sign::{DetachedSignature as _, PublicKey as _};
            let msg = mem.load_region(msg_addr, msg_len)?;
            let valid = match level {
                2 => {
                    let pk_slice =
                        mem.load_region(pubkey_addr, dilithium2::public_key_bytes() as u64)?;
                    let sig_slice =
                        mem.load_region(sig_addr, dilithium2::signature_bytes() as u64)?;
                    let pk = match dilithium2::PublicKey::from_bytes(pk_slice) {
                        Ok(p) => p,
                        Err(_) => {
                            res.push(ResultUpdate::Reg {
                                index: result_reg,
                                value: 0,
                                tag: false,
                            });
                            return Ok(res);
                        }
                    };
                    let sig = match dilithium2::DetachedSignature::from_bytes(sig_slice) {
                        Ok(s) => s,
                        Err(_) => {
                            res.push(ResultUpdate::Reg {
                                index: result_reg,
                                value: 0,
                                tag: false,
                            });
                            return Ok(res);
                        }
                    };
                    dilithium2::verify_detached_signature(&sig, msg, &pk).is_ok()
                }
                3 => {
                    let pk_slice =
                        mem.load_region(pubkey_addr, dilithium3::public_key_bytes() as u64)?;
                    let sig_slice =
                        mem.load_region(sig_addr, dilithium3::signature_bytes() as u64)?;
                    let pk = match dilithium3::PublicKey::from_bytes(pk_slice) {
                        Ok(p) => p,
                        Err(_) => {
                            res.push(ResultUpdate::Reg {
                                index: result_reg,
                                value: 0,
                                tag: false,
                            });
                            return Ok(res);
                        }
                    };
                    let sig = match dilithium3::DetachedSignature::from_bytes(sig_slice) {
                        Ok(s) => s,
                        Err(_) => {
                            res.push(ResultUpdate::Reg {
                                index: result_reg,
                                value: 0,
                                tag: false,
                            });
                            return Ok(res);
                        }
                    };
                    dilithium3::verify_detached_signature(&sig, msg, &pk).is_ok()
                }
                5 => {
                    let pk_slice =
                        mem.load_region(pubkey_addr, dilithium5::public_key_bytes() as u64)?;
                    let sig_slice =
                        mem.load_region(sig_addr, dilithium5::signature_bytes() as u64)?;
                    let pk = match dilithium5::PublicKey::from_bytes(pk_slice) {
                        Ok(p) => p,
                        Err(_) => {
                            res.push(ResultUpdate::Reg {
                                index: result_reg,
                                value: 0,
                                tag: false,
                            });
                            return Ok(res);
                        }
                    };
                    let sig = match dilithium5::DetachedSignature::from_bytes(sig_slice) {
                        Ok(s) => s,
                        Err(_) => {
                            res.push(ResultUpdate::Reg {
                                index: result_reg,
                                value: 0,
                                tag: false,
                            });
                            return Ok(res);
                        }
                    };
                    dilithium5::verify_detached_signature(&sig, msg, &pk).is_ok()
                }
                _ => {
                    res.push(ResultUpdate::Reg {
                        index: result_reg,
                        value: 0,
                        tag: false,
                    });
                    return Ok(res);
                }
            };
            res.push(ResultUpdate::Reg {
                index: result_reg,
                value: if valid { 1 } else { 0 },
                tag: false,
            });
        }
        SetVL { new_vl } => {
            if !vector_enabled {
                return Err(VMError::VectorExtensionDisabled);
            }
            let mut vl = new_vl as usize;
            if vl == 0 {
                vl = 1;
            }
            res.push(ResultUpdate::Vl {
                value: vl.min(LOGICAL_VECTOR_MAX),
            });
        }
        Vadd { rd, rs, rt } => {
            if !vector_enabled {
                return Err(VMError::VectorExtensionDisabled);
            }
            let stride = vl;
            let rd = crate::IVM::VECTOR_BASE + rd as usize * stride;
            let rs = crate::IVM::VECTOR_BASE + rs as usize * stride;
            let rt = crate::IVM::VECTOR_BASE + rt as usize * stride;
            if rd + vl > 256 || rs + vl > 256 || rt + vl > 256 {
                return Err(VMError::RegisterOutOfBounds);
            }
            for i in 0..vl {
                let a = regs[rs + i];
                let b = regs[rt + i];
                if zk && tags[rs + i] != tags[rt + i] {
                    return Err(VMError::PrivacyViolation);
                }
                let tag = if zk { tags[rs + i] } else { false };
                let sum = a.wrapping_add(b);
                res.push(ResultUpdate::Reg {
                    index: (rd + i) as u16,
                    value: sum,
                    tag,
                });
            }
        }
        Beq { .. } | Jump { .. } | Halt => {}
    }
    Ok(res)
}

fn conflict(a: &InstrMeta, b: &InstrMeta) -> bool {
    if !a.writes.is_disjoint(&b.reads) {
        return true;
    }
    if !a.writes.is_disjoint(&b.writes) {
        return true;
    }
    if !b.writes.is_disjoint(&a.reads) {
        return true;
    }
    if (a.mem_write && (b.mem_write || b.mem_read)) || (b.mem_write && (a.mem_write || a.mem_read))
    {
        return true;
    }
    false
}

fn schedule_batches(metas: &[InstrMeta]) -> Vec<Vec<usize>> {
    let mut batches: Vec<Vec<usize>> = Vec::new();
    for (idx, meta) in metas.iter().enumerate() {
        if batches.is_empty() {
            batches.push(Vec::new());
        }
        let cur = batches.last_mut().unwrap();
        let has_conflict = cur.iter().any(|j| conflict(meta, &metas[*j]));
        if has_conflict {
            batches.push(vec![idx]);
        } else {
            cur.push(idx);
        }
    }
    batches
}

impl IVM {
    /// Execute a slice of instructions using simple ILP scheduling.
    pub fn execute_block_parallel(&mut self, block: &[SimpleInstruction]) -> Result<(), VMError> {
        let mut vl = self.vector_length;
        let mut metas = Vec::with_capacity(block.len());
        let mut vls = Vec::with_capacity(block.len());
        for instr in block {
            vls.push(vl);
            let (m, next_vl) = analyse_instruction(instr, vl, LOGICAL_VECTOR_MAX);
            metas.push(m);
            vl = next_vl;
        }
        let batches = schedule_batches(&metas);

        for batch in batches {
            let regs_snapshot = self.registers.snapshot();
            let tags_snapshot = self.registers.snapshot_tags();
            let vector_enabled = self.vector_enabled;

            let results_lock = Mutex::new(Vec::new());

            rayon::scope(|s| {
                for &idx in &batch {
                    let instr = block[idx];
                    let regs = &regs_snapshot;
                    let tags = &tags_snapshot;
                    let mem = &self.memory;
                    let zk = self.zk_mode;
                    let vl = vls[idx];
                    let results = &results_lock;
                    s.spawn(move |_| {
                        let result =
                            compute_instruction(instr, regs, tags, mem, zk, vl, vector_enabled);
                        results
                            .lock()
                            .expect("results mutex poisoned")
                            .push((idx, result));
                    });
                }
            });

            let mut results = results_lock.into_inner().expect("results mutex poisoned");

            results.sort_by_key(|(i, _)| *i);
            for (idx, result) in results {
                let cost = cost_of(&block[idx], vls[idx]);
                if self.gas_remaining < cost {
                    return Err(VMError::OutOfGas);
                }
                self.gas_remaining -= cost;
                let updates = result?;
                for upd in updates {
                    match upd {
                        ResultUpdate::Reg { index, value, tag } => {
                            if index != 0 {
                                self.registers.set(index as usize, value);
                                if self.zk_mode {
                                    self.registers.set_tag(index as usize, tag);
                                }
                            }
                        }
                        ResultUpdate::Mem { addr, value } => {
                            self.memory.store_u64(addr, value)?;
                        }
                        ResultUpdate::Vl { value } => {
                            self.vector_length = value.min(LOGICAL_VECTOR_MAX);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

    use super::*;
    use crate::{instruction, ivm_cache, metadata::LITERAL_SECTION_MAGIC};

    #[test]
    fn builder_respects_deterministic_policy() {
        set_banner_enabled(false);
        let vm = IVM::deterministic_builder(u64::MAX)
            .suppress_startup_banner()
            .build();
        assert!(!vm.uses_cuda());
        assert!(!vm.uses_metal());
        assert_eq!(
            vm.acceleration_policy(),
            AccelerationPolicy::deterministic()
        );
    }

    #[test]
    fn rtm_detection_does_not_panic() {
        let _ = std::panic::catch_unwind(rtm_available);
    }

    #[test]
    fn deterministic_policy_disables_acceleration() {
        let policy = AccelerationPolicy::deterministic();
        assert!(!policy.allow_cuda());
        assert!(!policy.allow_metal());
    }

    fn program_with_imm(imm: i16) -> Vec<u8> {
        let mut bytes = ProgramMetadata::default().encode();
        let imm8 = imm.clamp(-128, 127) as i8;
        let add = crate::encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 1, 0, imm8);
        bytes.extend_from_slice(&add.to_le_bytes());
        bytes.extend_from_slice(&crate::encoding::wide::encode_halt().to_le_bytes());
        bytes
    }

    fn unique_program() -> Vec<u8> {
        static COUNTER: AtomicU32 = AtomicU32::new(1);
        let next = COUNTER.fetch_add(1, AtomicOrdering::Relaxed);
        let imm = ((next % 2047) + 1) as i16;
        program_with_imm(imm)
    }

    fn empty_blob_tlv() -> Vec<u8> {
        use crate::pointer_abi::PointerType;
        let mut tlv = Vec::with_capacity(7 + iroha_crypto::Hash::LENGTH);
        tlv.extend_from_slice(&(PointerType::Blob as u16).to_be_bytes());
        tlv.push(1);
        tlv.extend_from_slice(&0u32.to_be_bytes());
        let hash: [u8; iroha_crypto::Hash::LENGTH] = iroha_crypto::Hash::new([]).into();
        tlv.extend_from_slice(&hash);
        tlv
    }

    fn program_with_literal_prefix() -> (Vec<u8>, usize) {
        let mut bytes = ProgramMetadata::default().encode();
        bytes.extend_from_slice(&LITERAL_SECTION_MAGIC);
        bytes.extend_from_slice(&(1u32).to_le_bytes()); // one literal entry
        bytes.extend_from_slice(&0u32.to_le_bytes()); // no post-pad
        bytes.extend_from_slice(&0u32.to_le_bytes()); // no extra data payload
        bytes.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
        let literal_prefix = bytes.len() - ProgramMetadata::default().encode().len();
        let addi = crate::kotodama::compiler::encode_addi(1, 1, 0).expect("encode addi");
        bytes.extend_from_slice(&addi.to_le_bytes());
        bytes.extend_from_slice(&crate::encoding::encode_halt().to_le_bytes());
        (bytes, literal_prefix)
    }

    #[test]
    fn load_program_predecodes_instructions() {
        set_banner_enabled(false);
        ivm_cache::init_global_with_capacity(64);
        let program = unique_program();
        let mut vm = IVM::new(u64::MAX);
        vm.load_program(&program).expect("program loads");
        assert!(vm.predecoded.is_some());
        assert!(vm.predecoded_index.contains_key(&vm.pc));
        let decoded_len = vm
            .predecoded
            .as_ref()
            .map(|ops| ops.len())
            .unwrap_or_default();
        assert_eq!(vm.predecoded_index.len(), decoded_len);
    }

    #[test]
    fn recompute_input_bump_handles_empty_tlv() {
        set_banner_enabled(false);
        let mut vm = IVM::new(u64::MAX);
        let tlv = empty_blob_tlv();
        vm.memory.preload_input(0, &tlv).expect("preload empty TLV");
        let mut program = ProgramMetadata::default().encode();
        program.extend_from_slice(&crate::encoding::encode_halt().to_le_bytes());
        vm.load_program(&program)
            .expect("load program with TLV scan");
        let new_tlv = empty_blob_tlv();
        let ptr = vm.alloc_input_tlv(&new_tlv).expect("allocate second TLV");
        assert!(ptr > Memory::INPUT_START);
        let mut buf = vec![0u8; tlv.len()];
        vm.memory
            .load_bytes(Memory::INPUT_START, &mut buf)
            .expect("read first TLV");
        assert_eq!(buf, tlv);
    }

    #[test]
    fn run_prefers_predecoded_instructions() {
        set_banner_enabled(false);
        ivm_cache::init_global_with_capacity(64);
        let program = program_with_imm(1);
        let mut vm = IVM::new(u64::MAX);
        vm.load_program(&program).expect("program loads");
        vm.reset_predecode_misses();
        vm.run().expect("run succeeds");
        assert_eq!(vm.predecode_misses(), 0);

        vm.load_program(&program).expect("reload succeeds");
        vm.reset_predecode_misses();
        vm.run().expect("second run succeeds");
        assert_eq!(vm.predecode_misses(), 0);
    }

    #[test]
    fn hardware_capabilities_consistent_with_usage() {
        set_banner_enabled(false);
        let vm = IVM::builder(u64::MAX).suppress_startup_banner().build();
        let caps = vm.hardware_capabilities();
        if vm.uses_cuda() {
            assert!(caps.cuda_available());
        }
        if vm.uses_metal() {
            assert!(caps.metal_available());
        }
    }

    #[test]
    fn builder_can_override_capabilities() {
        set_banner_enabled(false);
        let disabled_caps = HardwareCapabilities::none();
        let vm = IVM::builder(u64::MAX)
            .with_capabilities(disabled_caps)
            .with_acceleration(AccelerationPolicy::new(true, true))
            .suppress_startup_banner()
            .build();
        assert_eq!(vm.hardware_capabilities(), disabled_caps);
        assert!(!vm.uses_cuda());
        assert!(!vm.uses_metal());

        let enabled_cuda = HardwareCapabilities::new(true, false);
        let vm = IVM::builder(u64::MAX)
            .with_capabilities(enabled_cuda)
            .with_acceleration(AccelerationPolicy::new(true, false))
            .suppress_startup_banner()
            .build();
        assert_eq!(vm.hardware_capabilities(), enabled_cuda);
        assert!(vm.uses_cuda());
        assert!(!vm.uses_metal());
    }

    #[test]
    fn builder_config_roundtrip() {
        set_banner_enabled(false);
        let (cfg, builder) = IvmBuilder::deterministic_config_suppressed(2048);
        let builder = builder.map_capabilities_builder(|_| HardwareCapabilities::new(true, false));
        let snapshot = builder.config();
        let expected_cfg = cfg.map_capabilities(|_| HardwareCapabilities::new(true, false));
        assert_eq!(snapshot.gas_limit(), 2048);
        assert_eq!(snapshot.acceleration(), AccelerationPolicy::deterministic());
        assert_eq!(
            snapshot.capabilities(),
            HardwareCapabilities::new(true, false)
        );
        assert_eq!(expected_cfg, snapshot);
        let vm = IVM::new_with_config(snapshot);
        assert_eq!(vm.remaining_gas(), 2048);
        assert_eq!(
            vm.acceleration_policy(),
            AccelerationPolicy::deterministic()
        );
    }

    #[test]
    fn builder_gas_limit_getter_tracks_updates() {
        set_banner_enabled(false);
        let mut builder = IVM::with_config(IvmConfig::adaptive(100)).suppress_startup_banner();
        builder
            .set_acceleration(AccelerationPolicy::adaptive())
            .map_config(|cfg| cfg.with_acceleration(AccelerationPolicy::adaptive()));
        assert_eq!(builder.gas_limit(), 100);
        builder.set_gas_limit(500).map_config(|cfg| {
            IvmConfigBuilder::from_config(cfg)
                .with_gas_limit(500)
                .build()
        });
        assert_eq!(builder.gas_limit(), 500);
        let vm = builder.build();
        assert_eq!(vm.remaining_gas(), 500);
    }

    #[test]
    fn config_builder_conversion_allows_roundtrip_editing() {
        set_banner_enabled(false);
        let builder = IVM::builder(256)
            .with_gas_limit(300)
            .suppress_startup_banner();
        assert_eq!(builder.config().gas_limit(), 300);
        let final_cfg = builder.build_config().builder().with_gas_limit(350).build();
        let (cfg, vm) = IVM::with_config(final_cfg)
            .suppress_startup_banner()
            .build_with_config();
        assert_eq!(vm.remaining_gas(), 350);
        let vm2 = IVM::with_config(cfg).suppress_startup_banner().build();
        assert_eq!(vm2.remaining_gas(), 350);
    }

    #[test]
    fn new_with_config_respects_settings() {
        set_banner_enabled(false);
        let config = IvmConfig::new(256)
            .with_acceleration(AccelerationPolicy::deterministic())
            .with_capabilities(HardwareCapabilities::none());
        let vm = IVM::new_with_config(config);
        assert_eq!(vm.remaining_gas(), 256);
        assert_eq!(
            vm.acceleration_policy(),
            AccelerationPolicy::deterministic()
        );
        assert_eq!(vm.hardware_capabilities(), HardwareCapabilities::none());
        assert!(!vm.uses_cuda());
        assert!(!vm.uses_metal());
    }

    #[test]
    fn config_builder_presets() {
        set_banner_enabled(false);
        let deterministic = IvmConfigBuilder::deterministic(300).build();
        assert_eq!(deterministic.gas_limit(), 300);
        assert_eq!(
            deterministic.acceleration(),
            AccelerationPolicy::deterministic()
        );

        let adaptive = IvmConfigBuilder::adaptive(400)
            .with_capabilities(HardwareCapabilities::none())
            .build();
        assert_eq!(adaptive.gas_limit(), 400);
        assert_eq!(adaptive.acceleration(), AccelerationPolicy::adaptive());
        assert_eq!(adaptive.capabilities(), HardwareCapabilities::none());
    }

    #[test]
    fn stack_limit_for_gas_respects_multiplier_and_budget() {
        set_banner_enabled(false);
        let prev_multiplier = crate::gas_to_stack_multiplier();
        // Keep the budget low enough to observe multiplier effects without hitting the default stack cap.
        let cfg = IvmConfig::adaptive(100_000).with_stack_budget_bytes(2 * 1024 * 1024);
        crate::set_gas_to_stack_multiplier(4);
        let baseline = cfg.stack_limit_for_gas();

        crate::set_gas_to_stack_multiplier(8);
        let raised = cfg.stack_limit_for_gas();
        assert!(
            raised > baseline,
            "increasing the multiplier should raise the derived stack cap"
        );
        assert_eq!(raised, 800_000_u64.clamp(64 * 1024, 2 * 1024 * 1024));

        crate::set_gas_to_stack_multiplier(prev_multiplier);
    }

    #[test]
    fn config_capability_mapping_helper() {
        set_banner_enabled(false);
        let cfg = IvmConfig::adaptive(100).map_capabilities(|caps| caps);
        let builder = IVM::builder(100)
            .map_capabilities_builder(|caps| caps)
            .suppress_startup_banner();
        assert_eq!(cfg.capabilities(), builder.config().capabilities());
    }

    #[test]
    fn config_forced_simd_overrides_detection() {
        set_banner_enabled(false);
        let _simd_guard = vector::forced_simd_test_lock();
        let prev = vector::set_thread_forced_simd(None);
        let cfg = IvmConfig::adaptive(128).with_forced_simd(Some(SimdChoice::Scalar));
        let vm = IVM::with_config(cfg).suppress_startup_banner().build();
        assert_eq!(
            vm.acceleration_policy().forced_simd(),
            Some(SimdChoice::Scalar)
        );
        assert_eq!(vector::simd_choice(), SimdChoice::Scalar);
        match prev {
            Some(choice) => {
                vector::set_thread_forced_simd(Some(choice));
            }
            None => vector::clear_thread_forced_simd(),
        }
    }

    #[test]
    fn adaptive_config_suppressed_helpers() {
        set_banner_enabled(false);
        let (cfg, builder) = IvmBuilder::adaptive_config_suppressed(700);
        assert_eq!(cfg.gas_limit(), 700);
        let (returned_cfg, vm) = builder.build_with_config();
        assert_eq!(returned_cfg.gas_limit(), 700);
        assert_eq!(vm.remaining_gas(), 700);
        let vm2 = IVM::with_config(returned_cfg)
            .suppress_startup_banner()
            .build();
        assert_eq!(vm2.remaining_gas(), 700);
    }

    #[test]
    fn config_into_builder_conversions() {
        set_banner_enabled(false);
        let cfg = IvmConfig::deterministic(123);
        let builder_from_config = IvmBuilder::from(cfg);
        assert_eq!(builder_from_config.config(), cfg);

        let cfg_builder = cfg.builder().with_gas_limit(456);
        let builder_from_builder = IvmBuilder::from(cfg_builder);
        assert_eq!(builder_from_builder.config().gas_limit(), 456);
    }

    #[test]
    fn set_acceleration_policy_updates_policy_snapshot() {
        set_banner_enabled(false);
        let mut vm = IVM::builder(u64::MAX)
            .suppress_startup_banner()
            .with_acceleration(AccelerationPolicy::deterministic())
            .build();
        assert_eq!(
            vm.acceleration_policy(),
            AccelerationPolicy::deterministic()
        );
        let policy = AccelerationPolicy::new(true, true);
        vm.set_acceleration_policy(policy);
        assert_eq!(vm.acceleration_policy(), policy);
    }

    #[test]
    fn second_load_hits_global_cache() {
        set_banner_enabled(false);
        ivm_cache::init_global_with_capacity(64);
        let program = unique_program();
        let before = ivm_cache::global_counters();
        let mut first = IVM::new(u64::MAX);
        first
            .load_program(&program)
            .expect("first load populates cache");
        let after_first = ivm_cache::global_counters();
        // Other tests may update the shared cache concurrently, so ensure we registered at least one miss.
        assert!(after_first.1 - before.1 >= 1);

        let mut second = IVM::new(u64::MAX);
        second
            .load_program(&program)
            .expect("second load retrieves cache");
        let after_second = ivm_cache::global_counters();
        assert!(after_second.0 - after_first.0 >= 1);
    }

    #[test]
    fn reset_preserves_entry_pc_with_literal_prefix() {
        set_banner_enabled(false);
        ivm_cache::init_global_with_capacity(64);
        let (program, literal_prefix) = program_with_literal_prefix();
        let literal_pc = literal_prefix as u64;
        let mut vm = IVM::new(u64::MAX);
        vm.load_program(&program)
            .expect("program with literals loads");
        assert_eq!(vm.pc(), literal_pc);
        assert!(vm.predecoded.is_some());
        assert!(vm.predecoded_index.contains_key(&literal_pc));

        vm.reset();
        assert_eq!(vm.pc(), literal_pc);
        assert!(vm.predecoded_index.contains_key(&literal_pc));
    }
}
