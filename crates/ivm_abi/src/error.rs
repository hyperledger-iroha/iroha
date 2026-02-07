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

/// VM errors.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum VMError {
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

impl fmt::Display for VMError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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

impl StdError for VMError {}
