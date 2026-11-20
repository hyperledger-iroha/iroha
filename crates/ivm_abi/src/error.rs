//! Error types and permission flags for the VM.
//!
//! Error variants cover common failure modes including privacy tag violations.
use crate::numeric::{NumericFaultV1, PointerAbiFaultV1};
/// Original local allocation refusal carried by [`crate::error::VmDeferral::AllocationDeferred`].
/// This is the allocation owner's type; re-exporting it does not copy its custody.
pub use iroha_allocation::{AllocationRefusal, AllocatorRefusal};
use iroha_data_model::executor::fault::{IvmFaultKindV1, IvmFaultV1};
/// Original move-only native verification refusal owned by the shared proof layer.
pub use iroha_plonk::verification_attempt::VerificationRefusal;
use std::{error::Error as StdError, fmt};
/// Memory region permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Perm(u8);
impl Perm {
    pub const NONE: Perm = Perm(0);
    pub const READ: Perm = Perm(1);
    pub const WRITE: Perm = Perm(2);
    pub const EXECUTE: Perm = Perm(4);
    pub fn contains(self, other: Perm) -> bool {
        (self.0 & other.0) == other.0
    }
}
// Enable bitwise OR for Perm flags
use std::ops::BitOr;
impl BitOr for Perm {
    type Output = Perm;
    fn bitor(self, rhs: Perm) -> Perm {
        Perm(self.0 | rhs.0)
    }
}
/// High-level trap category captured alongside the raw [`VMError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmTrapKind {
    OutOfGas,
    OutOfMemory,
    MemoryFault,
    DecodeError,
    InvalidOpcode,
    UnknownSyscall,
    NotImplemented,
    SyscallGasQuoteExceeded,
    SyscallMeteringModeMismatch,
    GasCostOverflow,
    NumericFault,
    PointerAbiFault,
    AssertionFailed,
    ContractAbort,
    ExceededMaxCycles,
    InvalidMetadata,
    UnsupportedProgramVersion,
    UnsupportedProgramFeatureBits,
    UnsupportedProgramAbiVersion,
    MissingMaxCycles,
    MaxCyclesWithoutZk,
    /// Baseline-engine header vector length above the ABI V1 maximum.
    ProgramVectorLengthTooLarge,
    ArtifactAbiHashMismatch,
    GenericSyscallNotAllowed,
    /// Baseline-engine `SETVL` or vector width outside the logical lane bound.
    InvalidVectorLength,
    MissingHalt,
    PermissionDenied,
    ReentrantCall,
    CallDepthExceeded,
    ControlFlowViolation,
    PrivacyViolation,
    RegisterOutOfBounds,
    NoritoInvalid,
    AbiTypeNotAllowed,
    HostOutputBudgetExceeded,
    AmxBudgetExceeded,
    Other,
}
/// Consensus-retained host resource whose execution budget was exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOutputResource {
    /// Number of retained effect artifacts.
    Items,
    /// Aggregate encoded size of retained effect artifacts.
    Bytes,
}
/// Source location mapped from compiler-emitted debug metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmSourceLocation<'a> {
    pub function: Option<&'a str>,
    pub path: Option<&'a str>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}
/// Budget-related execution snapshot captured at trap time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmBudgetSnapshot {
    pub gas_limit: u64,
    pub gas_remaining: u64,
    pub gas_used: u64,
    pub cycles: u64,
    pub max_cycles: u64,
    pub stack_limit_bytes: u64,
    pub stack_bytes_used: u64,
}
/// Additional execution context captured at trap time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmExecutionContext<'a> {
    pub entrypoint_pc: Option<u64>,
    pub current_function: Option<&'a str>,
    /// Primary opcode byte of the rejected instruction word, when the trap is an opcode rejection.
    pub opcode: Option<u8>,
    pub syscall: Option<u32>,
    pub predecoded_loaded: bool,
    pub predecoded_hit: Option<bool>,
}
/// Borrowed semantic-trap context; rendering also requires the original returned [`VMError`].
///
/// Text borrows the VM's retained source metadata. This view owns no error or allocation
/// and cannot outlive or permit replacement of that metadata. Local refusals have no view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmExecutionDiagnostic<'a> {
    /// Canonical bounded execution fault, absent for application rejections.
    pub fault: Option<IvmFaultV1>,
    /// Original structured fault when the selected ABI V1 C10 profile ran.
    pub c10_fault: Option<iroha_data_model::ivm_c10::IvmFaultC10V1>,
    pub trap_kind: VmTrapKind,
    pub pc: u64,
    pub source: Option<VmSourceLocation<'a>>,
    pub budget: VmBudgetSnapshot,
    pub context: VmExecutionContext<'a>,
}
/// Local inability to complete an execution attempt. This is never a protocol
/// rejection or a deterministic VM fault and deliberately has no wire codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionDeferral {
    /// Original host custody or an immutable construction plan was inconsistent.
    /// Execution rolls back without fees or rejection; the native owner must halt
    /// for recovery rather than retry, quarantine or certify this local result.
    LocalInvariantViolation,
    /// The host allocator could not supply the requested physical storage.
    AllocationUnavailable,
    /// The local active execution memory admission owner has no capacity.
    ActiveMemoryCapacity,
    /// Governed verifier artifacts are not loaded or require an authenticated reload.
    /// This local availability condition cannot become a transaction rejection.
    VerifierArtifactsUnavailable,
    /// Native verification was cooperatively cancelled before a verdict.
    VerificationCancelled,
    /// Native verification exceeded its own scratch budget; no allocation-pool wait is implied.
    NativeVerificationMemoryUnavailable,
    /// Trusted verifier initialization failed before an incoming-proof verdict was available.
    VerifierInitializationUnavailable,
    /// Original local committed execution history is missing, corrupt, or not readable.
    /// This is never evidence that the signed transaction was rejected.
    CanonicalHistoryUnavailable,
    /// The initial canonical crypto admission policy has not been installed locally.
    /// This readiness condition is neither an incoming signature verdict nor an allocation refusal.
    CryptoAdmissionPolicyUnavailable,
    /// The exact canonical operation requires an implementation absent from this node.
    /// This fixed local cause retains its algorithm and operation, with no resource authority.
    CryptoImplementationUnavailable(iroha_crypto::UnsupportedImplementation),
    /// A bounded native ancestry read exhausted its local source-work or byte allowance.
    /// No allocator wake source is implied by this independent read limit.
    CanonicalHistoryCapacity,
    /// The local trace owner no longer matches its admitted invocation or storage scope.
    /// Retry with a coherent owner; this does not establish a guest or protocol fault.
    TraceOwnerUnavailable,
}
impl fmt::Display for ExecutionDeferral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CryptoImplementationUnavailable(original) => {
                return write!(f, "local crypto implementation unavailable: {original}");
            }
            Self::LocalInvariantViolation => "local original execution custody invariant violated",
            Self::AllocationUnavailable => "local allocation unavailable",
            Self::ActiveMemoryCapacity => "local active execution memory capacity unavailable",
            Self::VerifierArtifactsUnavailable => "local governed verifier artifacts unavailable",
            Self::VerificationCancelled => "native verification cancelled before completion",
            Self::NativeVerificationMemoryUnavailable => {
                "native verification scratch budget unavailable"
            }
            Self::VerifierInitializationUnavailable => {
                "trusted verifier initialization unavailable"
            }
            Self::CanonicalHistoryUnavailable => "local canonical execution history unavailable",
            Self::CryptoAdmissionPolicyUnavailable => {
                "initial committed crypto admission policy unavailable"
            }
            Self::CanonicalHistoryCapacity => {
                "local canonical execution history read capacity unavailable"
            }
            Self::TraceOwnerUnavailable => "local execution trace owner unavailable",
        })
    }
}
impl StdError for ExecutionDeferral {}
/// Original node-local cause of an unfinished invocation. This move-only type has no wire
/// codec and cannot contain a deterministic trap, a metered result or a charged outcome.
#[derive(Debug)]
pub enum VmDeferral {
    /// Local reason without a separately owned resource.
    ExecutionDeferred(ExecutionDeferral),
    /// The pool reservation and its exact wake source.
    AllocationDeferred(AllocationRefusal),
    /// The original bounded decoder and enclosing scope.
    DecodeDeferred(norito::core::DecodeLocalRefusal),
    /// The original physical allocator refusal.
    AllocatorDeferred(AllocatorRefusal),
    /// The original native verifier, including any decoder or scratch refusal.
    VerificationDeferred(VerificationRefusal),
    /// No host was present for this invocation.
    HostUnavailable,
    /// The host's reported actual cost exceeded its pre-execution quote.
    SyscallGasQuoteExceeded {
        /// Original cost quoted before dispatch.
        quoted: u64,
        /// Actual cost returned by the host.
        actual: u64,
    },
    /// The host and VM used different metering conventions for this syscall.
    SyscallMeteringModeMismatch {
        /// Original dispatch selector.
        syscall: u32,
    },
}

fn decoder_reason(kind: norito::core::DecodeLocalRefusalKind) -> ExecutionDeferral {
    use norito::core::DecodeLocalRefusalKind as Kind;
    match kind {
        Kind::Allocator => ExecutionDeferral::AllocationUnavailable,
        Kind::EnclosingLimit => ExecutionDeferral::ActiveMemoryCapacity,
    }
}

fn verifier_reason(original: &VerificationRefusal) -> ExecutionDeferral {
    use iroha_plonk::verification_attempt::VerificationRefusalKind as Kind;
    match original.kind() {
        Kind::Decode(kind) => decoder_reason(kind),
        Kind::NativeCancelled => ExecutionDeferral::VerificationCancelled,
        Kind::NativeMemoryBudget => ExecutionDeferral::NativeVerificationMemoryUnavailable,
        Kind::InitializationUnavailable => ExecutionDeferral::VerifierInitializationUnavailable,
        Kind::LocalInvariantViolation => ExecutionDeferral::LocalInvariantViolation,
    }
}

impl VmDeferral {
    /// Borrow the scheduling category while the original cause remains owned here.
    #[must_use]
    pub fn reason(&self) -> ExecutionDeferral {
        match self {
            Self::ExecutionDeferred(reason) => *reason,
            Self::AllocationDeferred(_) => ExecutionDeferral::ActiveMemoryCapacity,
            Self::AllocatorDeferred(_) => ExecutionDeferral::AllocationUnavailable,
            Self::DecodeDeferred(original) => decoder_reason(original.kind()),
            Self::VerificationDeferred(original) => verifier_reason(original),
            Self::HostUnavailable
            | Self::SyscallGasQuoteExceeded { .. }
            | Self::SyscallMeteringModeMismatch { .. } => {
                ExecutionDeferral::LocalInvariantViolation
            }
        }
    }
}

impl TryFrom<VMError> for VmDeferral {
    type Error = VMError;
    fn try_from(error: VMError) -> Result<Self, Self::Error> {
        match error {
            VMError::Deferred(cause) => Ok(cause),
            completed => Err(completed),
        }
    }
}

impl From<VmDeferral> for VMError {
    fn from(cause: VmDeferral) -> Self {
        Self::Deferred(cause)
    }
}
impl From<VmTrap> for VMError {
    fn from(trap: VmTrap) -> Self {
        Self::Trap(trap)
    }
}

/// The outcome of an unsuccessful local VM call. Only a deterministic trap can carry gas.
pub enum VMError {
    /// A deterministic execution or admission failure.
    Trap(VmTrap),
    /// The node could not complete the attempt and retains the original cause.
    Deferred(VmDeferral),
}

// Baseline execution fixtures retain the original error diagnostic, including
// the field name of every metering wrapper.
struct TrapDebug<'a>(&'a VmTrap);

impl fmt::Debug for TrapDebug<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            VmTrap::Metered { gas, trap } => f
                .debug_struct("Metered")
                .field("gas", gas)
                .field("source", &Self(trap))
                .finish(),
            leaf => fmt::Debug::fmt(leaf, f),
        }
    }
}

impl fmt::Debug for VMError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trap(trap) => fmt::Debug::fmt(&TrapDebug(trap), f),
            Self::Deferred(cause) => fmt::Debug::fmt(cause, f),
        }
    }
}

/// Borrow the underlying failure after peeling deterministic metering.
#[derive(Clone, Copy)]
pub enum VmErrorRef<'a> {
    /// The unmetered deterministic failure.
    Trap(&'a VmTrap),
    /// The original local owner.
    Deferred(&'a VmDeferral),
}

impl fmt::Display for VmErrorRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trap(trap) => fmt::Display::fmt(trap, f),
            Self::Deferred(cause) => fmt::Display::fmt(cause, f),
        }
    }
}

impl core::fmt::Debug for VmErrorRef<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Trap(trap) => core::fmt::Debug::fmt(trap, f),
            Self::Deferred(cause) => core::fmt::Debug::fmt(cause, f),
        }
    }
}

/// Deterministic execution and admission failures, including metered syscall work.
///
/// A metered failure can only own a completed trap.
/// ```compile_fail
/// use ivm_abi::error::{VmDeferral, VmTrap};
/// let refusal = VmDeferral::HostUnavailable;
/// let _ = VmTrap::Metered { gas: 9, trap: Box::new(refusal) };
/// ```
#[derive(Debug)]
pub enum VmTrap {
    /// Host work failed after consuming a deterministic amount of syscall gas.
    ///
    /// The VM executor debits `gas` and then returns `trap` so trap
    /// classification and diagnostics preserve the original failure kind.
    Metered {
        gas: u64,
        trap: Box<VmTrap>,
    },
    OutOfGas,
    OutOfMemory,
    MemoryAccessViolation {
        addr: u32,
        perm: Perm,
    },
    MisalignedAccess {
        addr: u32,
    },
    /// Access outside the configured VM memory bounds.
    MemoryOutOfBounds,
    /// Instruction decoding failed due to invalid data or out-of-bounds fetch.
    DecodeError,
    /// An instruction word is not an admitted opcode.
    ///
    /// The C10 engine reports the primary opcode byte (bits 31..24). The baseline engine keeps
    /// its original value, the low 16 bits of the rejected word.
    InvalidOpcode(u16),
    UnknownSyscall(u32),
    /// Syscall number is reserved/known but not implemented by the current host build.
    NotImplemented {
        syscall: u32,
    },
    /// A deterministic gas formula exceeded the `u64` gas domain.
    GasCostOverflow,
    /// The next staged syscall phase could not be afforded.
    ///
    /// No gas is debited for the named phase; gas charged by earlier phases is retained.
    SyscallOutOfGas {
        /// Syscall executing the staged phase.
        syscall: u32,
        /// Stable staged-metering phase tag.
        phase: u8,
    },
    /// A checked numeric operation failed in trap mode.
    NumericFault(NumericFaultV1),
    /// A C10 signed 64-bit register operation overflowed (`i64::MIN / -1`, `ABS(i64::MIN)` or a
    /// ceiling quotient beyond `i64::MAX`).
    ///
    /// Internal to the C10 engine (numeric tag
    /// [`crate::numeric::C10_REGISTER_INTEGER_OVERFLOW_TAG`]); the baseline engine reports these
    /// operations as `AssertionFailed` and its wire numeric enum has no such tag.
    RegisterIntegerOverflow,
    /// A numeric pointer envelope failed stable ABI validation.
    PointerAbiFault(PointerAbiFaultV1),
    AssertionFailed,
    /// Contract requested an application-level abort with a stable numeric code.
    ContractAbort {
        /// Authenticated seiyaku name where the rejection originated.
        contract: Box<str>,
        /// Authenticated symbolic variant name within the declared error type.
        name: String,
        /// Stable nominal error type identity.
        error_type: String,
        /// Hash of the exact signed variant schema.
        schema_hash: [u8; 32],
        /// Validated nonzero enum-local variant code.
        code: u32,
        /// Static presentation text authenticated by the originating contract interface.
        /// Its immutable storage keeps this rare diagnostic from inflating every VM result.
        message: Option<Box<str>>,
    },
    ExceededMaxCycles,
    InvalidMetadata,
    /// The fixed header declares a version outside the sole first-release 1.1 surface.
    UnsupportedProgramVersion {
        /// Declared major version.
        major: u8,
        /// Declared minor version.
        minor: u8,
    },
    /// The fixed header contains mode bits unknown to ABI V1.
    UnsupportedProgramFeatureBits {
        /// Unknown feature bits only, excluding recognized bits.
        bits: u8,
    },
    /// The fixed header selects an ABI version unavailable in this release.
    UnsupportedProgramAbiVersion {
        /// Declared ABI version.
        version: u8,
    },
    /// A ZK-mode header declares no trace bound (`max_cycles == 0`).
    MissingMaxCycles,
    /// A header without the ZK bit declares a trace bound; gas is the only non-ZK budget.
    MaxCyclesWithoutZk {
        /// Declared, non-zero `max_cycles`.
        max_cycles: u64,
    },
    /// A baseline-engine fixed header requests more logical vector lanes than ABI V1 permits.
    ProgramVectorLengthTooLarge {
        /// Declared vector length.
        vector_length: u8,
        /// Maximum accepted vector length.
        max_allowed: u8,
    },
    /// A self-describing artifact targets a different canonical ABI descriptor.
    ArtifactAbiHashMismatch {
        /// ABI descriptor hash required by the runtime.
        expected: [u8; 32],
        /// Authenticated ABI descriptor hash carried by the artifact.
        actual: [u8; 32],
    },
    /// A contract-bound syscall appeared in an ABI-authenticated generic program.
    GenericSyscallNotAllowed {
        /// Rejected syscall number.
        syscall: u32,
    },
    /// A baseline-engine vector instruction selected an invalid logical vector length.
    InvalidVectorLength {
        /// Rejected logical lane count.
        vector_length: usize,
    },
    /// Program reached the end of the executable region without an explicit terminating syscall or HALT.
    MissingHalt,
    /// A baseline-engine vector or 128-bit memory instruction ran without the header VECTOR bit.
    VectorExtensionDisabled,
    /// A baseline-engine ZK-helper instruction ran outside ZK mode.
    ZkExtensionDisabled,
    NullifierAlreadyUsed,
    PermissionDenied,
    /// A nested invocation targeted an address already active in its ancestry.
    ReentrantCall,
    /// A nested invocation or protected direct call exceeded the canonical call-depth bound.
    CallDepthExceeded,
    /// A protected contract violated call/return integrity: a return to an untrusted PC, an
    /// indirect transfer, or `HALT` inside a deployable contract.
    ControlFlowViolation,
    PrivacyViolation,
    RegisterOutOfBounds,
    /// Malformed Norito TLV envelope or checksum mismatch.
    NoritoInvalid,
    /// Pointer‑ABI type not allowed under the current ABI policy.
    AbiTypeNotAllowed {
        abi: u8,
        type_id: u16,
    },
    /// An execution attempted to retain more effect output than consensus permits.
    HostOutputBudgetExceeded {
        /// Resource whose limit was exceeded.
        resource: HostOutputResource,
        /// Amount the host attempted to retain.
        attempted: u64,
        /// Consensus maximum for the resource.
        limit: u64,
    },
    /// AMX static analysis budget exceeded for the current dataspace.
    AmxBudgetExceeded {
        /// Dataspace whose slice exceeded the configured budget.
        dataspace: iroha_model_base::topology::DataSpaceId,
        /// Stage that exceeded the budget (e.g., commit).
        stage: iroha_data_model::errors::AmxStage,
        /// Estimated elapsed milliseconds for the stage.
        elapsed_ms: u64,
        /// Configured budget in milliseconds.
        budget_ms: u64,
    },
}

impl VmTrap {
    /// Borrow the deterministic fault or admission failure under all metering layers.
    #[must_use]
    pub fn as_unmetered(&self) -> &Self {
        match self {
            Self::Metered { trap, .. } => trap.as_unmetered(),
            leaf => leaf,
        }
    }
    /// Consume metering and return its deterministic cause.
    #[must_use]
    pub fn into_unmetered(self) -> Self {
        match self {
            Self::Metered { trap, .. } => trap.into_unmetered(),
            leaf => leaf,
        }
    }
}
impl VMError {
    /// Attach deterministic gas to a completed failure, preserving a local refusal verbatim.
    #[must_use]
    pub fn metered(gas: u64, source: Self) -> Self {
        match source {
            Self::Deferred(_) => source,
            Self::Trap(VmTrap::Metered {
                gas: existing,
                trap,
            }) => Self::Trap(VmTrap::Metered {
                gas: gas.saturating_add(existing),
                trap,
            }),
            Self::Trap(trap) => Self::Trap(VmTrap::Metered {
                gas,
                trap: Box::new(trap),
            }),
        }
    }
    /// Return the scheduling category of an unfinished attempt.
    #[must_use]
    pub fn execution_deferral(&self) -> Option<ExecutionDeferral> {
        match self {
            Self::Trap(_) => None,
            Self::Deferred(cause) => Some(cause.reason()),
        }
    }
    /// Project only deterministic failures of a started invocation.
    ///
    /// Admission errors, application rejections and local host/resource failures
    /// deliberately have no runtime fault value. The projection belongs to the engine that
    /// raised the failure ([`crate::IvmEngine::fault_kind`]); this accessor has no profile
    /// context and applies the engine of the interface profile, the frozen baseline.
    // TODO: only tests call this accessor; consensus paths project with the engine of the VM
    // that ran. Delete it once those tests name their engine.
    #[must_use]
    pub fn fault_kind(&self) -> Option<IvmFaultKindV1> {
        crate::IvmProfileId::of_policy(crate::SyscallPolicy::AbiV1)
            .engine()
            .fault_kind(self)
    }
    /// Construct a metered `NotImplemented` error for a known syscall.
    #[must_use]
    pub fn metered_not_implemented(gas: u64, syscall: u32) -> Self {
        Self::metered(gas, Self::Trap(VmTrap::NotImplemented { syscall }))
    }

    /// Borrow the original failure and its family, peeling every metered trap.
    #[must_use]
    pub fn as_unmetered(&self) -> VmErrorRef<'_> {
        match self {
            Self::Deferred(cause) => VmErrorRef::Deferred(cause),
            Self::Trap(trap) => VmErrorRef::Trap(trap.as_unmetered()),
        }
    }
    /// Borrow an unmetered deterministic cause for fault projection.
    #[must_use]
    pub fn unmetered_trap(&self) -> Option<&VmTrap> {
        match self {
            Self::Trap(trap) => Some(trap.as_unmetered()),
            Self::Deferred(_) => None,
        }
    }
    /// The reported cost of a metered deterministic failure.
    #[must_use]
    pub fn metered_gas(&self) -> Option<u64> {
        match self {
            Self::Trap(VmTrap::Metered { gas, .. }) => Some(*gas),
            _ => None,
        }
    }
    /// Consume this failure and return its original cause in the same family.
    #[must_use]
    pub fn into_unmetered(self) -> Self {
        match self {
            Self::Trap(trap) => Self::Trap(trap.into_unmetered()),
            local => local,
        }
    }
    /// Extract deterministic syscall gas and return the original failure.
    #[must_use]
    pub fn split_metered(self) -> (Option<u64>, Self) {
        let gas = self.metered_gas();
        (gas, self.into_unmetered())
    }
}
impl fmt::Display for VmTrap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Metered { gas, trap } => {
                write!(f, "metered syscall error after {gas} gas: {trap}")
            }
            Self::OutOfGas => write!(f, "out of gas"),
            Self::OutOfMemory => write!(f, "out of memory"),
            Self::MemoryAccessViolation { addr, perm } => {
                write!(
                    f,
                    "memory access violation at 0x{addr:08x} (needed permission: {perm:?})"
                )
            }
            Self::MisalignedAccess { addr } => {
                write!(f, "misaligned memory access at 0x{addr:08x}")
            }
            Self::MemoryOutOfBounds => write!(f, "memory access out of bounds"),
            Self::DecodeError => write!(f, "instruction decode error"),
            Self::InvalidOpcode(op) => write!(f, "invalid or unknown opcode 0x{op:02x}"),
            Self::UnknownSyscall(num) => write!(f, "unknown syscall number {num}"),
            Self::NotImplemented { syscall } => {
                write!(f, "syscall 0x{syscall:02x} not implemented by host")
            }
            Self::GasCostOverflow => write!(f, "gas cost exceeded the canonical u64 domain"),
            Self::SyscallOutOfGas { syscall, phase } => write!(
                f,
                "syscall 0x{syscall:02x} ran out of gas before metering phase {phase}"
            ),
            Self::NumericFault(fault) => {
                write!(f, "numeric operation failed with ABI fault {}", fault.tag())
            }
            Self::RegisterIntegerOverflow => write!(
                f,
                "numeric operation failed with ABI fault {}",
                crate::numeric::C10_REGISTER_INTEGER_OVERFLOW_TAG
            ),
            Self::PointerAbiFault(fault) => {
                write!(
                    f,
                    "numeric pointer validation failed with ABI fault {}",
                    fault.tag()
                )
            }
            Self::AssertionFailed => write!(f, "assertion failed (constraint violation)"),
            Self::ContractAbort {
                error_type,
                code,
                message,
                ..
            } => {
                write!(f, "seiyaku aborted with {error_type} code {code}")?;
                if let Some(message) = message {
                    write!(f, ": {message}")?;
                }
                Ok(())
            }
            Self::ExceededMaxCycles => write!(f, "execution exceeded max cycles"),
            Self::InvalidMetadata => write!(f, "invalid program metadata"),
            Self::UnsupportedProgramVersion { major, minor } => {
                write!(f, "unsupported IVM program version {major}.{minor}")
            }
            Self::UnsupportedProgramFeatureBits { bits } => {
                write!(f, "unsupported IVM program feature bits 0x{bits:02x}")
            }
            Self::UnsupportedProgramAbiVersion { version } => {
                write!(f, "unsupported IVM program ABI version {version}")
            }
            Self::MissingMaxCycles => {
                write!(f, "ZK-mode IVM program header declares no trace bound")
            }
            Self::MaxCyclesWithoutZk { max_cycles } => write!(
                f,
                "IVM program header declares trace bound {max_cycles} without ZK mode"
            ),
            Self::ProgramVectorLengthTooLarge {
                vector_length,
                max_allowed,
            } => write!(
                f,
                "IVM program vector length {vector_length} exceeds maximum {max_allowed}"
            ),
            Self::ArtifactAbiHashMismatch { expected, actual } => write!(
                f,
                "contract artifact ABI hash mismatch (expected={}, actual={})",
                HexBytes(expected),
                HexBytes(actual)
            ),
            Self::GenericSyscallNotAllowed { syscall } => write!(
                f,
                "syscall 0x{syscall:02x} is not allowed in a generic IVM program"
            ),
            Self::InvalidVectorLength { vector_length } => {
                write!(f, "invalid vector length {vector_length}")
            }
            Self::MissingHalt => write!(f, "program terminated without HALT, EXIT, or ABORT"),
            Self::VectorExtensionDisabled => write!(f, "vector extension not enabled"),
            Self::ZkExtensionDisabled => write!(f, "zk extension not enabled"),
            Self::NullifierAlreadyUsed => write!(f, "nullifier already used"),
            Self::PermissionDenied => write!(f, "permission denied"),
            Self::ReentrantCall => write!(f, "contract address is already active"),
            Self::CallDepthExceeded => write!(f, "contract call depth exceeded"),
            Self::ControlFlowViolation => write!(f, "protected control-flow violation"),
            Self::PrivacyViolation => write!(f, "privacy tag violation"),
            Self::RegisterOutOfBounds => write!(f, "register index out of bounds"),
            Self::NoritoInvalid => write!(f, "invalid Norito TLV envelope"),
            Self::AbiTypeNotAllowed { abi, type_id } => write!(
                f,
                "pointer-ABI type 0x{type_id:04x} not allowed for abi_version={abi}"
            ),
            Self::HostOutputBudgetExceeded {
                resource,
                attempted,
                limit,
            } => write!(
                f,
                "host output budget exceeded (resource={resource:?}, attempted={attempted}, limit={limit})"
            ),
            Self::AmxBudgetExceeded {
                dataspace,
                stage,
                elapsed_ms,
                budget_ms,
            } => write!(
                f,
                "amx budget exceeded (dataspace={}, stage={stage:?}, elapsed_ms={}, budget_ms={})",
                dataspace.as_u64(),
                elapsed_ms,
                budget_ms
            ),
        }
    }
}
impl fmt::Display for VmDeferral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExecutionDeferred(reason) => write!(f, "execution deferred: {reason}"),
            Self::AllocationDeferred(refusal) => {
                write!(f, "execution allocation deferred: {refusal}")
            }
            Self::AllocatorDeferred(original) => {
                write!(f, "physical allocation deferred: {original}")
            }
            Self::DecodeDeferred(original) => {
                write!(f, "execution decode deferred: {original}")
            }
            Self::VerificationDeferred(original) => {
                write!(f, "native verification deferred: {original}")
            }
            Self::HostUnavailable => write!(f, "host unavailable"),
            Self::SyscallGasQuoteExceeded { quoted, actual } => write!(
                f,
                "syscall gas quote exceeded (quoted={quoted}, actual={actual})"
            ),
            Self::SyscallMeteringModeMismatch { syscall } => write!(
                f,
                "syscall 0x{syscall:02x} violated its registered metering mode"
            ),
        }
    }
}
impl fmt::Display for VMError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trap(trap) => trap.fmt(f),
            Self::Deferred(cause) => cause.fmt(f),
        }
    }
}
struct HexBytes<'a>(&'a [u8]);
impl fmt::Display for HexBytes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
impl StdError for VmTrap {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Metered { trap, .. } => Some(trap.as_ref()),
            _ => None,
        }
    }
}
impl StdError for VmDeferral {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::DecodeDeferred(owner) => Some(owner),
            Self::AllocatorDeferred(owner) => Some(owner),
            Self::VerificationDeferred(owner) => Some(owner),
            _ => None,
        }
    }
}
impl StdError for VMError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Trap(trap) => trap.source(),
            Self::Deferred(cause) => cause.source(),
        }
    }
}
/// Preserve a local operational refusal when mapping a deterministic decode fault.
///
/// The complete original error retains its cause and any existing pool/release
/// observation; a decoder scope never invents an allocation-pool wakeup.
/// Converting it to malformed input would make transaction validity depend on
/// local resource pressure. Semantic failures retain the caller's existing map.
pub fn preserve_execution_deferral(error: VMError, malformed: VMError) -> VMError {
    if error.execution_deferral().is_some() {
        error
    } else {
        malformed
    }
}

// Numeric pointer faults have one canonical conversion for all ABI producers and runtime consumers.
impl From<iroha_primitives::numeric_abi::NumericAbiError> for VMError {
    fn from(error: iroha_primitives::numeric_abi::NumericAbiError) -> Self {
        use iroha_primitives::numeric_abi::NumericAbiError;
        let fault = match error {
            NumericAbiError::SchemaMismatch => PointerAbiFaultV1::SchemaMismatch,
            NumericAbiError::NonCanonicalMantissa
            | NumericAbiError::NonCanonicalDecimal
            | NumericAbiError::MantissaOverflow
            | NumericAbiError::InvalidScale
            | NumericAbiError::NegativeQuantity => PointerAbiFaultV1::NonCanonical,
            NumericAbiError::FrameTooLarge => PointerAbiFaultV1::OversizedLength,
            NumericAbiError::FrameTooShort
            | NumericAbiError::InvalidHeader
            | NumericAbiError::CompressionNotAllowed
            | NumericAbiError::LayoutFlagsNotAllowed
            | NumericAbiError::LengthMismatch
            | NumericAbiError::Norito(_) => PointerAbiFaultV1::MalformedFrame,
        };
        VMError::Trap(VmTrap::PointerAbiFault(fault))
    }
}

#[cfg(test)]
mod numeric_abi_error_tests {
    use super::{PointerAbiFaultV1, VMError};
    use crate::error::VmTrap;
    use iroha_primitives::numeric_abi::NumericAbiError;

    #[test]
    fn every_numeric_frame_error_maps_to_its_stable_pointer_fault() {
        use NumericAbiError::*;
        use PointerAbiFaultV1::{MalformedFrame, NonCanonical, OversizedLength};

        let cases = [
            (FrameTooShort, MalformedFrame),
            (FrameTooLarge, OversizedLength),
            (InvalidHeader, MalformedFrame),
            (SchemaMismatch, PointerAbiFaultV1::SchemaMismatch),
            (CompressionNotAllowed, MalformedFrame),
            (LayoutFlagsNotAllowed, MalformedFrame),
            (LengthMismatch, MalformedFrame),
            (MantissaOverflow, NonCanonical),
            (NonCanonicalMantissa, NonCanonical),
            (InvalidScale, NonCanonical),
            (NonCanonicalDecimal, NonCanonical),
            (NegativeQuantity, NonCanonical),
            (Norito("invalid checksum".into()), MalformedFrame),
        ];
        for (error, expected) in cases {
            let actual = VMError::from(error.clone());
            assert!(
                matches!(actual, VMError::Trap(VmTrap::PointerAbiFault(found)) if found == expected),
                "numeric frame error {error:?} changed its pointer fault"
            );
        }
    }
}

#[cfg(test)]
mod execution_deferral_tests {
    use crate::error::{VmDeferral, VmErrorRef, VmTrap};

    #[test]
    fn diagnostic_text_keeps_the_baseline_metering_and_local_cause() {
        let plain = VMError::Trap(VmTrap::OutOfGas);
        let retained = VMError::Trap(VmTrap::Metered {
            gas: 17,
            trap: Box::new(VmTrap::Metered {
                gas: 3,
                trap: Box::new(VmTrap::OutOfGas),
            }),
        });
        assert_eq!(format!("{plain:?}"), "OutOfGas");
        assert_eq!(
            format!("{retained:?}"),
            "Metered { gas: 17, source: Metered { gas: 3, source: OutOfGas } }"
        );
        assert_eq!(format!("{:?}", retained.as_unmetered()), "OutOfGas");
        let local = VMError::Deferred(VmDeferral::HostUnavailable);
        assert_eq!(format!("{local:?}"), "HostUnavailable");
    }
    #[test]
    fn physical_allocator_transport_is_unmetered_and_not_a_fault() {
        use iroha_allocation::{AllocatorRefusal, ChargedBufferError};
        use std::error::Error;
        // Transport-only control. Core tests obtain this variant from a real null allocation.
        let original = AllocatorRefusal::try_from(ChargedBufferError::Allocator {
            requested_bytes: 97,
        })
        .unwrap();
        let error = VMError::Deferred(VmDeferral::AllocatorDeferred(original));
        assert_eq!(
            error.execution_deferral(),
            Some(ExecutionDeferral::AllocationUnavailable)
        );
        assert!(error.fault_kind().is_none());
        assert!(error.source().unwrap().is::<AllocatorRefusal>());
        let error = preserve_execution_deferral(
            VMError::metered(99, error),
            VMError::Trap(VmTrap::NoritoInvalid),
        );
        assert!(error.metered_gas().is_none());
        let VMError::Deferred(VmDeferral::AllocatorDeferred(original)) = error.into_unmetered()
        else {
            panic!("retain exact physical owner")
        };
        assert!(matches!(
            original.into_buffer_error().unwrap(),
            ChargedBufferError::Allocator {
                requested_bytes: 97
            }
        ));
        assert!(
            core::mem::size_of::<VMError>() < 128,
            "the error remains inline"
        );
    }

    use super::{ExecutionDeferral, VMError, preserve_execution_deferral};

    fn assert_reason(error: &VMError, reason: ExecutionDeferral) {
        assert!(
            matches!(error.as_unmetered(), VmErrorRef::Deferred(VmDeferral::ExecutionDeferred(found)) if *found == reason)
        );
        assert_eq!(error.execution_deferral(), Some(reason));
    }

    #[test]
    fn decode_mapping_keeps_complete_operational_errors_and_existing_semantic_faults() {
        let budget = iroha_allocation::AllocationBudget::new(1);
        let occupied = budget.try_reserve_bytes(1).unwrap();
        let refusal = budget.try_reserve_bytes(1).unwrap_err();
        let error = preserve_execution_deferral(
            VMError::Deferred(VmDeferral::AllocationDeferred(refusal.clone())),
            VMError::Trap(VmTrap::NoritoInvalid),
        );
        assert!(
            matches!(error, VMError::Deferred(VmDeferral::AllocationDeferred(ref found)) if *found == refusal)
        );
        for reason in [
            ExecutionDeferral::LocalInvariantViolation,
            ExecutionDeferral::AllocationUnavailable,
            ExecutionDeferral::ActiveMemoryCapacity,
            ExecutionDeferral::VerifierArtifactsUnavailable,
            ExecutionDeferral::VerificationCancelled,
            ExecutionDeferral::NativeVerificationMemoryUnavailable,
            ExecutionDeferral::VerifierInitializationUnavailable,
            ExecutionDeferral::TraceOwnerUnavailable,
        ] {
            assert_reason(
                &preserve_execution_deferral(
                    VMError::Deferred(VmDeferral::ExecutionDeferred(reason)),
                    VMError::Trap(VmTrap::DecodeError),
                ),
                reason,
            );
        }
        let wrapped = VMError::metered(17, error);
        let mapped = preserve_execution_deferral(wrapped, VMError::Trap(VmTrap::DecodeError));
        assert_eq!(mapped.metered_gas(), None);
        assert!(
            matches!(mapped, VMError::Deferred(VmDeferral::AllocationDeferred(found)) if found == refusal)
        );
        assert!(matches!(
            preserve_execution_deferral(
                VMError::Trap(VmTrap::MemoryOutOfBounds),
                VMError::Trap(VmTrap::NoritoInvalid)
            ),
            VMError::Trap(VmTrap::NoritoInvalid)
        ));
        drop(occupied);
    }

    #[test]
    fn contract_abort_preserves_authenticated_text_and_identity_through_metering() {
        // Retain the existing size guard pending measured review of the new inline owner.
        assert!(core::mem::size_of::<VMError>() < 128);
        let rejection = VMError::Trap(VmTrap::ContractAbort {
            contract: "Treasury".into(),
            name: "InsufficientFunds".into(),
            error_type: "treasury::Error".into(),
            schema_hash: [7; 32],
            code: 18,
            message: Some("残高が不足しています".into()),
        });
        assert_eq!(
            rejection.to_string(),
            "seiyaku aborted with treasury::Error code 18: 残高が不足しています"
        );
        fn assert_abort(error: &VMError) {
            let VmErrorRef::Trap(VmTrap::ContractAbort {
                contract,
                name,
                error_type,
                schema_hash,
                code,
                message,
            }) = error.as_unmetered()
            else {
                panic!("lost completed abort");
            };
            assert_eq!(&**contract, "Treasury");
            assert_eq!(&**name, "InsufficientFunds");
            assert_eq!(&**error_type, "treasury::Error");
            assert_eq!(*schema_hash, [7; 32]);
            assert_eq!(*code, 18);
            assert_eq!(message.as_deref(), Some("残高が不足しています"));
        }
        let metered = VMError::metered(123, rejection);
        assert_abort(&metered);
        let (gas, moved) = metered.split_metered();
        assert_eq!(gas, Some(123));
        assert_abort(&moved);
    }

    #[test]
    fn local_refusal_never_acquires_a_deterministic_gas_charge() {
        let reason = ExecutionDeferral::AllocationUnavailable;
        let deferred = VMError::metered(
            123,
            VMError::Deferred(VmDeferral::ExecutionDeferred(reason)),
        );
        assert_reason(&deferred, reason);
        let wrapped = VMError::metered(456, deferred);
        assert_eq!(wrapped.execution_deferral(), Some(reason));
        assert_eq!(wrapped.metered_gas(), None);
        let (gas, moved) = wrapped.split_metered();
        assert_eq!(gas, None);
        assert_reason(&moved, reason);
    }

    #[test]
    fn deterministic_guest_memory_fault_preserves_metering() {
        let fault = VMError::metered(123, VMError::Trap(VmTrap::OutOfMemory));
        assert_eq!(fault.execution_deferral(), None);
        let (gas, moved) = fault.split_metered();
        assert_eq!(gas, Some(123));
        assert!(matches!(moved, VMError::Trap(VmTrap::OutOfMemory)));
    }

    #[test]
    fn original_capacity_release_survives_every_metered_boundary() {
        let budget = iroha_allocation::AllocationBudget::new(8);
        let occupied = budget.try_reserve_bytes(8).unwrap();
        let refusal = budget.try_reserve_bytes(1).unwrap_err();
        let error = VMError::Deferred(VmDeferral::AllocationDeferred(refusal.clone()));
        assert_eq!(
            error.execution_deferral(),
            Some(ExecutionDeferral::ActiveMemoryCapacity)
        );
        let error = VMError::metered(123, error);
        assert!(
            matches!(&error, VMError::Deferred(VmDeferral::AllocationDeferred(found)) if *found == refusal)
        );
        let wrapped = VMError::metered(456, error);
        assert_eq!(wrapped.metered_gas(), None);
        let error = VMError::metered(789, wrapped);
        assert!(
            matches!(&error, VMError::Deferred(VmDeferral::AllocationDeferred(found)) if *found == refusal)
        );
        let (gas, moved) = error.split_metered();
        assert_eq!(gas, None);
        assert!(
            matches!(moved, VMError::Deferred(VmDeferral::AllocationDeferred(found)) if found == refusal)
        );
        drop(occupied);
    }

    #[test]
    fn unavailable_governed_verifier_is_local_and_cannot_be_metered() {
        let reason = ExecutionDeferral::VerifierArtifactsUnavailable;
        assert_eq!(
            reason.to_string(),
            "local governed verifier artifacts unavailable"
        );
        let original = VMError::metered(
            777,
            VMError::Deferred(VmDeferral::ExecutionDeferred(reason)),
        );
        assert_reason(&original, reason);
        let (gas, moved) = original.split_metered();
        assert_eq!(gas, None);
        assert_reason(&moved, reason);
    }

    #[test]
    fn absent_initial_crypto_policy_is_unmetered_local_readiness() {
        let reason = ExecutionDeferral::CryptoAdmissionPolicyUnavailable;
        assert_eq!(
            reason.to_string(),
            "initial committed crypto admission policy unavailable"
        );
        let original = VMError::metered(
            777,
            VMError::Deferred(VmDeferral::ExecutionDeferred(reason)),
        );
        assert_reason(&original, reason);
        let (gas, moved) = original.split_metered();
        assert_eq!(gas, None);
        assert_reason(&moved, reason);
    }

    #[test]
    fn missing_crypto_implementation_preserves_exact_unmetered_cause() {
        let missing = iroha_crypto::UnsupportedImplementation {
            algorithm: iroha_crypto::Algorithm::MlDsa,
            operation: iroha_crypto::CryptoOperation::VerifySignature,
        };
        let reason = ExecutionDeferral::CryptoImplementationUnavailable(missing);
        let original = VMError::metered(
            777,
            VMError::Deferred(VmDeferral::ExecutionDeferred(reason)),
        );
        assert_reason(&original, reason);
        let (gas, moved) = original.split_metered();
        assert_eq!(gas, None);
        assert_reason(&moved, reason);
        assert!(reason.to_string().ends_with(&missing.to_string()));
    }

    #[test]
    fn canonical_history_deferrals_cannot_be_metered_as_vm_faults() {
        for reason in [
            ExecutionDeferral::CanonicalHistoryUnavailable,
            ExecutionDeferral::CanonicalHistoryCapacity,
        ] {
            let original = VMError::metered(
                777,
                VMError::Deferred(VmDeferral::ExecutionDeferred(reason)),
            );
            assert_reason(&original, reason);
            let (gas, moved) = original.split_metered();
            assert_eq!(gas, None);
            assert_reason(&moved, reason);
        }
    }

    #[test]
    fn trace_owner_mismatch_remains_local_through_nested_metering() {
        let reason = ExecutionDeferral::TraceOwnerUnavailable;
        assert_eq!(
            reason.to_string(),
            "local execution trace owner unavailable"
        );
        let nested = VMError::metered(
            17,
            VMError::metered(91, VMError::Deferred(VmDeferral::ExecutionDeferred(reason))),
        );
        assert_eq!(nested.execution_deferral(), Some(reason));
        assert_eq!(nested.metered_gas(), None);
        let nested = preserve_execution_deferral(nested, VMError::Trap(VmTrap::DecodeError));
        assert!(
            matches!(&nested, VMError::Deferred(VmDeferral::ExecutionDeferred(found)) if *found == reason)
        );
        let original = VMError::metered(123, nested);
        assert_reason(&original, reason);
        let (gas, moved) = original.split_metered();
        assert_eq!(gas, None);
        assert_reason(&moved, reason);
    }
}

#[cfg(test)]
mod canonical_decode_deferral_tests {
    use super::*;
    use crate::error::{VmDeferral, VmTrap};

    #[test]
    fn retained_codec_refusal_has_no_metered_gas_or_deterministic_fault() {
        let bytes = norito::encode_canonical(&vec![31_u8; 8]).unwrap();
        let cause = norito::core::with_decode_limits_scope(
            norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
            || {
                norito::decode_canonical_for_admission::<Vec<u8>>(
                    &bytes,
                    norito::canonical_decode_limits(bytes.len()),
                )
            },
        )
        .unwrap_err()
        .into_local_refusal()
        .unwrap();
        let error = VMError::Deferred(VmDeferral::DecodeDeferred(cause.clone()));
        assert_eq!(
            error.execution_deferral(),
            Some(ExecutionDeferral::ActiveMemoryCapacity)
        );
        assert_eq!(error.fault_kind(), None);
        let error = VMError::metered(91, error);
        assert!(
            matches!(&error, VMError::Deferred(VmDeferral::DecodeDeferred(found)) if *found == cause)
        );
        let nested = VMError::metered(27, error);
        assert_eq!(
            nested.execution_deferral(),
            Some(ExecutionDeferral::ActiveMemoryCapacity)
        );
        assert_eq!(nested.fault_kind(), None);
        let (gas, error) = nested.split_metered();
        assert_eq!(gas, None);
        assert!(
            matches!(&error, VMError::Deferred(VmDeferral::DecodeDeferred(found)) if *found == cause)
        );
        let error = preserve_execution_deferral(error, VMError::Trap(VmTrap::NoritoInvalid));
        assert!(
            matches!(&error, VMError::Deferred(VmDeferral::DecodeDeferred(found)) if *found == cause)
        );
        assert_eq!(
            error
                .source()
                .unwrap()
                .downcast_ref::<norito::core::DecodeLocalRefusal>(),
            Some(&cause)
        );
        assert!(error.to_string().starts_with("execution decode deferred: "));
    }

    #[test]
    fn captured_allocator_refusal_retains_bytes_without_a_pool_wakeup() {
        // This synthetic decoder result exercises the established classifier trust
        // boundary; it is not evidence of an actual physical allocation failure.
        let cause = norito::core::classify_decode_attempt(|| {
            Err::<(), _>(norito::Error::AllocationFailed { bytes: 127 })
        })
        .unwrap_err()
        .into_local_refusal()
        .unwrap();
        let error = VMError::Deferred(VmDeferral::DecodeDeferred(cause.clone()));
        assert_eq!(
            error.execution_deferral(),
            Some(ExecutionDeferral::AllocationUnavailable)
        );
        assert_eq!(error.fault_kind(), None);
        let (gas, returned) = VMError::metered(13, error).split_metered();
        assert_eq!(gas, None);
        let VMError::Deferred(VmDeferral::DecodeDeferred(original)) = returned else {
            panic!("lost decoder owner")
        };
        assert_eq!(original, cause);
        assert_eq!(original.allocation_bytes(), Some(127));
    }
}

#[cfg(test)]
mod native_verification_deferral_tests {
    use super::*;
    use crate::error::{VmDeferral, VmTrap};
    use iroha_plonk::{
        pcs::ipa::IpaError,
        verification_attempt::{VerificationAttemptError, native_verification_error},
        verifier::VerifyError,
    };

    #[test]
    fn exact_native_budget_and_cancellation_move_through_nested_metering_without_faults() {
        // This is a genuine native budget check, not a claim that physical allocation failed.
        let budget = iroha_plonk::keys::KeygenConfigV2::pipa_r(Vec::new()).msm_budget;
        let refusal = budget.check(usize::MAX).unwrap_err();
        let cancellation = iroha_pasta::CancellationToken::new();
        assert!(iroha_pasta::CancellationToken::checkpoint(Some(&cancellation)).is_ok());
        cancellation.cancel();
        let cancelled =
            iroha_pasta::CancellationToken::checkpoint(Some(&cancellation)).unwrap_err();
        for (original, reason) in [
            (
                VerifyError::Ipa(IpaError::Msm(refusal.into())),
                ExecutionDeferral::NativeVerificationMemoryUnavailable,
            ),
            (
                VerifyError::from(cancelled),
                ExecutionDeferral::VerificationCancelled,
            ),
        ] {
            let original = Box::new(original);
            let expected_address: *const VerifyError = &*original;
            let attempt = native_verification_error(
                VerifyError::BatchItem {
                    index: 2,
                    error: original,
                },
                |_| -> () {
                    panic!("unfinished native work cannot reject a proof");
                },
            );
            let VerificationAttemptError::Deferred(owner) = attempt else {
                panic!("lost native owner");
            };
            let error = VMError::Deferred(VmDeferral::VerificationDeferred(owner));
            assert_eq!(error.execution_deferral(), Some(reason));
            assert_eq!(error.fault_kind(), None);
            assert!(
                error
                    .source()
                    .unwrap()
                    .downcast_ref::<VerificationRefusal>()
                    .is_some()
            );
            let nested = VMError::metered(19, VMError::metered(41, error));
            assert_eq!(nested.execution_deferral(), Some(reason));
            assert_eq!(nested.fault_kind(), None);
            assert_eq!(nested.metered_gas(), None);
            let mapped = preserve_execution_deferral(nested, VMError::Trap(VmTrap::NoritoInvalid));
            let (gas, moved) = VMError::metered(71, mapped).split_metered();
            assert_eq!(gas, None);
            let VMError::Deferred(VmDeferral::VerificationDeferred(owner)) = moved else {
                panic!("lost original after consuming split");
            };
            assert!(owner.decode_refusal().is_none());
            let Some(VerifyError::BatchItem { index, error }) = owner.native_verification_error()
            else {
                panic!("lost original typed outer failure");
            };
            assert_eq!(*index, 2);
            let retained_address: *const VerifyError = &**error;
            assert_eq!(retained_address, expected_address);
            match reason {
                ExecutionDeferral::NativeVerificationMemoryUnavailable => assert!(
                    matches!(&**error, VerifyError::Ipa(IpaError::Msm(actual)) if *actual == refusal.into())
                ),
                ExecutionDeferral::VerificationCancelled => {
                    assert!(matches!(&**error, VerifyError::Cancelled))
                }
                _ => unreachable!(),
            }
        }
    }
}
