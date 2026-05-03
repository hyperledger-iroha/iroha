//! Error types and permission flags for the VM.
//!
//! Error variants cover common failure modes including privacy tag violations
//! and hardware transactional memory aborts.
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
    AssertionFailed,
    ExceededMaxCycles,
    InvalidMetadata,
    InvalidVectorLength,
    MissingHalt,
    PermissionDenied,
    PrivacyViolation,
    RegisterOutOfBounds,
    HTMAbort,
    NoritoInvalid,
    AbiTypeNotAllowed,
    AmxBudgetExceeded,
    Other,
}

/// Source location mapped from compiler-emitted debug metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmSourceLocation {
    pub function: Option<String>,
    pub path: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

/// Budget-related execution snapshot captured at trap time.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmExecutionContext {
    pub entrypoint_pc: Option<u64>,
    pub current_function: Option<String>,
    pub opcode: Option<u16>,
    pub syscall: Option<u32>,
    pub predecoded_loaded: bool,
    pub predecoded_hit: Option<bool>,
}

/// Structured runtime diagnostic emitted as a side channel when execution traps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmExecutionDiagnostic {
    pub trap_kind: VmTrapKind,
    pub message: String,
    pub pc: u64,
    pub source: Option<VmSourceLocation>,
    pub budget: VmBudgetSnapshot,
    pub context: VmExecutionContext,
}

/// VM errors.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum VMError {
    /// Host work failed after consuming a deterministic amount of syscall gas.
    ///
    /// The VM executor debits `gas` and then returns `source` so trap
    /// classification and diagnostics preserve the original failure kind.
    Metered {
        gas: u64,
        source: Box<VMError>,
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
    /// Access outside the bounds of a memory segment in `segmented_memory`.
    MemoryOutOfBounds,
    /// Unaligned load/store in `segmented_memory`.
    UnalignedAccess,
    /// Attempt to write to a read-only segment.
    MemoryPermissionDenied,
    /// Instruction decoding failed due to invalid data or out-of-bounds fetch.
    DecodeError,
    InvalidOpcode(u16),
    UnknownSyscall(u32),
    /// VM was asked to run or execute a host-dependent operation without a host attached.
    HostUnavailable,
    /// Syscall number is reserved/known but not implemented by the current host build.
    NotImplemented {
        syscall: u32,
    },
    AssertionFailed,
    ExceededMaxCycles,
    InvalidMetadata,
    InvalidVectorLength {
        vector_length: usize,
    },
    /// Program reached the end of the executable region without an explicit terminating syscall or HALT.
    MissingHalt,
    VectorExtensionDisabled,
    ZkExtensionDisabled,
    NullifierAlreadyUsed,
    PermissionDenied,
    PrivacyViolation,
    RegisterOutOfBounds,
    HTMAbort,
    /// Malformed Norito TLV envelope or checksum mismatch.
    NoritoInvalid,
    /// Pointer‑ABI type not allowed under the current ABI policy.
    AbiTypeNotAllowed {
        abi: u8,
        type_id: u16,
    },
    /// AMX static analysis budget exceeded for the current dataspace.
    AmxBudgetExceeded {
        /// Dataspace whose slice exceeded the configured budget.
        dataspace: iroha_data_model::nexus::DataSpaceId,
        /// Stage that exceeded the budget (e.g., commit).
        stage: iroha_data_model::errors::AmxStage,
        /// Estimated elapsed milliseconds for the stage.
        elapsed_ms: u64,
        /// Configured budget in milliseconds.
        budget_ms: u64,
    },
}

impl VMError {
    /// Wrap an error with deterministic gas charged before surfacing it.
    #[must_use]
    pub fn metered(gas: u64, source: VMError) -> Self {
        match source {
            VMError::Metered {
                gas: existing,
                source,
            } => VMError::Metered {
                gas: gas.saturating_add(existing),
                source,
            },
            source => VMError::Metered {
                gas,
                source: Box::new(source),
            },
        }
    }

    /// Construct a metered `NotImplemented` error for a known syscall.
    #[must_use]
    pub fn metered_not_implemented(gas: u64, syscall: u32) -> Self {
        Self::metered(gas, VMError::NotImplemented { syscall })
    }

    /// Return the original error kind, peeling metered wrappers.
    #[must_use]
    pub fn as_unmetered(&self) -> &VMError {
        match self {
            VMError::Metered { source, .. } => source.as_unmetered(),
            error => error,
        }
    }

    /// Return the gas attached to this error, if it is metered.
    #[must_use]
    pub fn metered_gas(&self) -> Option<u64> {
        match self {
            VMError::Metered { gas, .. } => Some(*gas),
            _ => None,
        }
    }

    /// Consume this error and return the original unmetered error kind.
    #[must_use]
    pub fn into_unmetered(self) -> VMError {
        match self {
            VMError::Metered { source, .. } => source.into_unmetered(),
            error => error,
        }
    }

    /// Consume this error and split any attached gas from the original error.
    #[must_use]
    pub fn split_metered(self) -> (Option<u64>, VMError) {
        match self {
            VMError::Metered { gas, source } => (Some(gas), source.into_unmetered()),
            error => (None, error),
        }
    }
}

impl fmt::Display for VMError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VMError::Metered { gas, source } => {
                write!(f, "metered syscall error after {gas} gas: {source}")
            }
            VMError::OutOfGas => write!(f, "out of gas"),
            VMError::OutOfMemory => write!(f, "out of memory"),
            VMError::MemoryAccessViolation { addr, perm } => {
                write!(
                    f,
                    "memory access violation at 0x{addr:08x} (needed permission: {perm:?})"
                )
            }
            VMError::MisalignedAccess { addr } => {
                write!(f, "misaligned memory access at 0x{addr:08x}")
            }
            VMError::MemoryOutOfBounds => write!(f, "memory access out of bounds"),
            VMError::UnalignedAccess => write!(f, "unaligned memory access"),
            VMError::MemoryPermissionDenied => write!(f, "memory permission denied"),
            VMError::DecodeError => write!(f, "instruction decode error"),
            VMError::InvalidOpcode(op) => write!(f, "invalid or unknown opcode 0x{op:02x}"),
            VMError::UnknownSyscall(num) => write!(f, "unknown syscall number {num}"),
            VMError::HostUnavailable => write!(f, "host unavailable"),
            VMError::NotImplemented { syscall } => {
                write!(f, "syscall 0x{syscall:02x} not implemented by host")
            }
            VMError::AssertionFailed => write!(f, "assertion failed (constraint violation)"),
            VMError::ExceededMaxCycles => write!(f, "execution exceeded max cycles"),
            VMError::InvalidMetadata => write!(f, "invalid program metadata"),
            VMError::InvalidVectorLength { vector_length } => {
                write!(f, "invalid vector length {vector_length}")
            }
            VMError::MissingHalt => write!(f, "program terminated without HALT, EXIT, or ABORT"),
            VMError::VectorExtensionDisabled => write!(f, "vector extension not enabled"),
            VMError::ZkExtensionDisabled => write!(f, "zk extension not enabled"),
            VMError::NullifierAlreadyUsed => write!(f, "nullifier already used"),
            VMError::PermissionDenied => write!(f, "permission denied"),
            VMError::PrivacyViolation => write!(f, "privacy tag violation"),
            VMError::RegisterOutOfBounds => write!(f, "register index out of bounds"),
            VMError::HTMAbort => write!(f, "hardware transaction aborted"),
            VMError::NoritoInvalid => write!(f, "invalid Norito TLV envelope"),
            VMError::AbiTypeNotAllowed { abi, type_id } => write!(
                f,
                "pointer-ABI type 0x{type_id:04x} not allowed for abi_version={abi}"
            ),
            VMError::AmxBudgetExceeded {
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

impl StdError for VMError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            VMError::Metered { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
