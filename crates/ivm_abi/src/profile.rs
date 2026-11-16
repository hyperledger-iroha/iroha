//! Execution profiles: the engine and the named host policy an artifact is admitted and run under.
//!
//! A caller resolves a profile from authenticated context (the selected registration interface,
//! a deployed binding or a captured execution context) before any header byte is parsed. Nothing
//! in this module inspects an artifact to choose an engine, and no caller may try a second decoder
//! after one refused.
//!
//! Internal failures ([`VMError`]) are separate from the wire fault types of
//! `iroha_data_model`. The engine that raised a failure owns its projection
//! ([`IvmEngine::fault_kind`]); a new engine adds its own projection and never widens another
//! engine's wire values.
//!
//! Two engines exist. [`IvmEngine::Baseline`] is the frozen pre-C10 engine ([`crate::baseline`])
//! of the installed baseline selection; [`IvmProfileId::of_policy`] binds the interface policy
//! [`SyscallPolicy::AbiV1`] to it, so every registration, execution, trigger, replay and nested
//! call of an ABI V1 artifact runs it. [`IvmEngine::C10`] executes the named runtime-lifecycle
//! policy and the explicit [`IvmProfileId::C10_ABI_V1`] identity; no root selects C10's own ABI V1
//! surface until a release publishes it as a separate host-profile definition.

use crate::{
    SyscallPolicy, VMError,
    error::HostOutputResource,
    metadata::{
        HEADER_ABI_HASH_OFFSET, HEADER_ABI_VERSION_OFFSET, HEADER_BUDGET_OFFSET, HEADER_SIZE,
    },
    numeric::NumericFaultV1,
};
use iroha_data_model::executor::fault::IvmFaultKindV1;
use std::sync::OnceLock;

/// Instruction-set engine behind the one `IVM` facade.
///
/// An engine owns one fixed-header layout, one instruction set, one admission and fault
/// precedence and one metering rule set. Every dispatch on this type is an exhaustive `match`
/// without a wildcard arm, so adding an engine fails to compile at each site that must decide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IvmEngine {
    /// The consolidated interpreter: 48-byte header, 32 registers, caller-owned gas as the only
    /// execution budget, structured numeric and control-flow faults.
    C10,
    /// The frozen pre-C10 interpreter of the installed baseline selection: 49-byte header with a
    /// logical vector length, 256 registers, 8-bit operands, vector/parallel/direct-crypto/ZK-helper
    /// opcodes, `AssertionFailed` arithmetic traps, header-derived cycle limits and shared nested
    /// cycle allowances ([`crate::baseline`]).
    Baseline,
}

/// Fixed-header geometry of one engine.
///
/// The offsets belong to the engine, never to the workspace: a caller that reads a declared ABI
/// hash names the engine whose layout it reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderLayout {
    /// Length of the fixed header in artifact bytes.
    pub size: usize,
    /// Offset of the little-endian `u64` budget field.
    pub budget_offset: usize,
    /// Offset of the ABI version byte.
    pub abi_version_offset: usize,
    /// Offset of the 32-byte ABI descriptor hash, which ends the header.
    pub abi_hash_offset: usize,
}

impl HeaderLayout {
    /// The ABI descriptor hash an artifact declares in this layout.
    ///
    /// This reads a fixed position and validates nothing. It selects among the named profiles of
    /// one engine; the engine's header parser still authenticates every field in its own order.
    #[must_use]
    pub fn advertised_abi_hash<'a>(&self, artifact: &'a [u8]) -> Option<&'a [u8; 32]> {
        artifact
            .get(self.abi_hash_offset..self.size)
            .and_then(|hash| <&[u8; 32]>::try_from(hash).ok())
    }
}

impl IvmEngine {
    /// Every engine this release implements.
    pub const ALL: [Self; 2] = [Self::C10, Self::Baseline];

    /// Fixed-header geometry of this engine.
    #[must_use]
    pub const fn header_layout(self) -> HeaderLayout {
        match self {
            Self::C10 => HeaderLayout {
                size: HEADER_SIZE,
                budget_offset: HEADER_BUDGET_OFFSET,
                abi_version_offset: HEADER_ABI_VERSION_OFFSET,
                abi_hash_offset: HEADER_ABI_HASH_OFFSET,
            },
            Self::Baseline => HeaderLayout {
                size: crate::baseline::HEADER_SIZE,
                budget_offset: crate::baseline::HEADER_BUDGET_OFFSET,
                abi_version_offset: crate::baseline::HEADER_ABI_VERSION_OFFSET,
                abi_hash_offset: crate::baseline::HEADER_ABI_HASH_OFFSET,
            },
        }
    }

    /// Project a failure this engine raised onto the wire fault kind.
    ///
    /// Only deterministic failures of a started invocation have a fault value. Admission errors,
    /// application rejections and local host or resource refusals return `None`.
    ///
    /// The wire enum is the baseline's (`IvmFaultKindV1`, original tags): the baseline projection
    /// is exact. C10 faults the baseline enum cannot name (`ControlFlowViolation`, the register
    /// integer overflow) have no wire value yet.
    // TODO: phase 5 publishes C10's own fault family (`*C10V1`) and projects every C10 fault
    // onto it; until then those C10 faults return `None`.
    #[must_use]
    pub fn fault_kind(self, error: &VMError) -> Option<IvmFaultKindV1> {
        match self {
            Self::C10 => c10_fault_kind(error),
            Self::Baseline => crate::baseline::fault_kind(error),
        }
    }
}

/// Operand decoding shared by the static analyses of both engines.
///
/// Both instruction sets assign the same values to every opcode they share; only operand layouts
/// differ. Analyses that run on either engine's stream (control-flow and reachability indexes,
/// syscall scans) decode operands through these accessors instead of one engine's field helpers.
impl IvmEngine {
    /// The syscall number of a syscall instruction word, `None` for any other word.
    ///
    /// C10 has one 24-bit `SCALL`; the baseline has an 8-bit `SCALL` and a 24-bit `SYSTEM`.
    #[must_use]
    pub fn syscall_number(self, word: u32) -> Option<u32> {
        match self {
            Self::C10 => {
                if crate::instruction::wide::opcode(word) == crate::instruction::wide::system::SCALL
                {
                    Some(crate::instruction::wide::syscall_number(word))
                } else {
                    None
                }
            }
            Self::Baseline => {
                use crate::baseline::instruction::wide;
                match wide::opcode(word) {
                    wide::system::SCALL => Some(word & 0xFF),
                    wide::system::SYSTEM => {
                        Some(crate::baseline::encoding::wide::decode_syscallx(word))
                    }
                    _ => None,
                }
            }
        }
    }

    /// Signed word offset of a direct branch, `JAL`, `JMP` or `JALS`; `None` for any other word.
    #[must_use]
    pub fn direct_transfer_offset(self, word: u32) -> Option<i64> {
        match self {
            Self::C10 => {
                use crate::instruction::wide;
                Some(match wide::opcode(word) {
                    wide::control::BEQ
                    | wide::control::BNE
                    | wide::control::BLT
                    | wide::control::BGE
                    | wide::control::BLTU
                    | wide::control::BGEU => i64::from(wide::imm14(word)),
                    wide::control::JAL => i64::from(wide::jal_offset(word)),
                    wide::control::JMP | wide::control::JALS => i64::from(wide::imm24(word)),
                    _ => return None,
                })
            }
            Self::Baseline => {
                use crate::baseline::instruction::wide;
                Some(match wide::opcode(word) {
                    wide::control::BEQ
                    | wide::control::BNE
                    | wide::control::BLT
                    | wide::control::BGE
                    | wide::control::BLTU
                    | wide::control::BGEU => i64::from(wide::imm8(word)),
                    wide::control::JAL => i64::from(wide::imm16(word)),
                    wide::control::JMP | wide::control::JALS => i64::from(wide::imm24(word)),
                    _ => return None,
                })
            }
        }
    }

    /// Destination register of a word in this engine's layout (`rd`, the link register of `JAL`).
    #[must_use]
    pub fn destination_register(self, word: u32) -> usize {
        match self {
            Self::C10 => crate::instruction::wide::rd(word),
            Self::Baseline => crate::baseline::instruction::wide::rd(word),
        }
    }

    /// Whether `opcode` transfers control to a register-computed target.
    ///
    /// C10 has only `JALR`; the baseline also has `JR`.
    #[must_use]
    pub const fn is_indirect_transfer(self, opcode: u8) -> bool {
        match self {
            Self::C10 => opcode == crate::instruction::wide::control::JALR,
            Self::Baseline => matches!(
                opcode,
                crate::baseline::instruction::wide::control::JALR
                    | crate::baseline::instruction::wide::control::JR
            ),
        }
    }
}

/// The wire numeric fault of a numeric failure the C10 engine raised.
///
/// The C10 engine reports arithmetic failures with the wire tags themselves, so its projection
/// is the identity. An engine with other numeric semantics supplies its own projection.
const fn c10_numeric_fault(code: NumericFaultV1) -> NumericFaultV1 {
    code
}

/// Fault projection of the C10 engine. The match is exhaustive: a new [`VMError`] variant must be
/// classified here.
fn c10_fault_kind(error: &VMError) -> Option<IvmFaultKindV1> {
    use IvmFaultKindV1 as F;
    Some(match error.as_unmetered() {
        VMError::OutOfGas | VMError::SyscallOutOfGas { .. } => F::OutOfGas,
        VMError::OutOfMemory => F::MemoryLimitExceeded,
        VMError::MemoryAccessViolation { .. } => F::MemoryAccessViolation,
        VMError::MisalignedAccess { .. } => F::MisalignedAccess,
        VMError::MemoryOutOfBounds => F::MemoryOutOfBounds,
        VMError::DecodeError => F::DecodeError,
        VMError::InvalidOpcode(_) => F::InvalidOpcode,
        VMError::UnknownSyscall(_) => F::UnknownSyscall,
        VMError::NotImplemented { .. } => F::UnsupportedSyscall,
        VMError::GasCostOverflow => F::GasCostOverflow,
        VMError::NumericFault(code) => F::Numeric(c10_numeric_fault(*code)),
        VMError::PointerAbiFault(code) => F::PointerAbi(*code),
        VMError::AssertionFailed => F::AssertionFailed,
        VMError::ExceededMaxCycles => F::ExceededMaxCycles,
        VMError::InvalidMetadata => F::InvalidMetadata,
        VMError::MissingHalt => F::MissingHalt,
        VMError::NullifierAlreadyUsed => F::NullifierAlreadyUsed,
        VMError::PermissionDenied => F::PermissionDenied,
        VMError::ReentrantCall => F::ReentrantCall,
        VMError::CallDepthExceeded => F::CallDepthExceeded,
        // C10-only faults the baseline wire enum cannot name (see `IvmEngine::fault_kind`).
        VMError::ControlFlowViolation | VMError::RegisterIntegerOverflow => return None,
        VMError::PrivacyViolation => F::PrivacyViolation,
        VMError::RegisterOutOfBounds => F::RegisterOutOfBounds,
        VMError::NoritoInvalid => F::NoritoInvalid,
        VMError::AbiTypeNotAllowed { .. } => F::AbiTypeNotAllowed,
        VMError::HostOutputBudgetExceeded {
            resource: HostOutputResource::Items,
            ..
        } => F::HostOutputItemsExceeded,
        VMError::HostOutputBudgetExceeded {
            resource: HostOutputResource::Bytes,
            ..
        } => F::HostOutputBytesExceeded,
        VMError::AmxBudgetExceeded { .. } => F::AmxBudgetExceeded,
        VMError::ExecutionDeferred(_)
        | VMError::AllocationDeferred(_)
        | VMError::AllocatorDeferred(_)
        | VMError::DecodeDeferred(_)
        | VMError::VerificationDeferred(_)
        | VMError::HostUnavailable
        | VMError::SyscallGasQuoteExceeded { .. }
        | VMError::SyscallMeteringModeMismatch { .. }
        | VMError::ContractAbort { .. }
        | VMError::UnsupportedProgramVersion { .. }
        | VMError::UnsupportedProgramFeatureBits { .. }
        | VMError::UnsupportedProgramAbiVersion { .. }
        | VMError::MissingMaxCycles
        | VMError::MaxCyclesWithoutZk { .. }
        | VMError::ArtifactAbiHashMismatch { .. }
        | VMError::GenericSyscallNotAllowed { .. } => return None,
        // The baseline engine alone raises these; a C10 invocation never produces them.
        VMError::ProgramVectorLengthTooLarge { .. }
        | VMError::InvalidVectorLength { .. }
        | VMError::VectorExtensionDisabled
        | VMError::ZkExtensionDisabled => return None,
        VMError::Metered { .. } => unreachable!("as_unmetered removes metering wrappers"),
    })
}

/// Compiled identity of one implemented execution profile: a named host policy on its engine.
///
/// Node-local caches bind this identity into their keys, so an object prepared for one profile
/// is never served to another. It is not a wire value and has no codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IvmProfileId {
    engine: IvmEngine,
    policy: SyscallPolicy,
}

impl IvmProfileId {
    /// The profile of every named host policy, in a fixed order: the profiles a root can select.
    pub const IMPLEMENTED: [Self; 2] = [
        Self::of_policy(SyscallPolicy::AbiV1),
        Self::of_policy(SyscallPolicy::AbiV1RuntimeLifecycle),
    ];

    /// The frozen baseline profile: ABI V1 on the baseline engine (ABI hash `7531b45d…`), the
    /// profile of [`SyscallPolicy::AbiV1`] and of the installed baseline selection.
    pub const BASELINE_ABI_V1: Self = Self {
        engine: IvmEngine::Baseline,
        policy: SyscallPolicy::AbiV1,
    };

    /// C10's own ABI V1 surface on the C10 engine.
    ///
    /// No named policy binds to it and no published host-profile definition activates it, so no
    /// root resolves it. Tools and tests that target the C10 engine explicitly (the Kotodama
    /// C10 target, raw code, the engine's own suites) name it.
    // TODO: phase 5 publishes this surface as the host profile of `iroha.runtime.ivm_c10.v1`.
    pub const C10_ABI_V1: Self = Self {
        engine: IvmEngine::C10,
        policy: SyscallPolicy::AbiV1,
    };

    /// The named runtime-lifecycle policy on the C10 engine.
    pub const C10_RUNTIME_LIFECYCLE: Self = Self {
        engine: IvmEngine::C10,
        policy: SyscallPolicy::AbiV1RuntimeLifecycle,
    };

    /// The implemented profiles of `engine`, in a fixed order.
    #[must_use]
    pub fn of_engine(engine: IvmEngine) -> &'static [Self] {
        const C10: [IvmProfileId; 2] = [
            IvmProfileId::C10_ABI_V1,
            IvmProfileId::C10_RUNTIME_LIFECYCLE,
        ];
        const BASELINE: [IvmProfileId; 1] = [IvmProfileId::BASELINE_ABI_V1];
        match engine {
            IvmEngine::C10 => &C10,
            IvmEngine::Baseline => &BASELINE,
        }
    }

    /// Bind a named host policy to the engine that executes it.
    ///
    /// This is the only policy-to-engine binding in the workspace. A new engine is introduced
    /// by a new [`IvmEngine`] variant and its arm here. The interface policy runs the frozen
    /// baseline engine; the runtime-lifecycle policy stays on C10.
    #[must_use]
    pub const fn of_policy(policy: SyscallPolicy) -> Self {
        let engine = match policy {
            SyscallPolicy::AbiV1 => IvmEngine::Baseline,
            SyscallPolicy::AbiV1RuntimeLifecycle => IvmEngine::C10,
        };
        Self { engine, policy }
    }

    /// The implemented profile of `policy` on `engine`, if that engine implements the policy.
    ///
    /// C10 implements both named policies (its own ABI V1 surface is
    /// [`Self::C10_ABI_V1`]); the baseline engine implements only [`SyscallPolicy::AbiV1`].
    #[must_use]
    pub const fn of_engine_policy(engine: IvmEngine, policy: SyscallPolicy) -> Option<Self> {
        match (engine, policy) {
            (IvmEngine::C10, policy) => Some(Self {
                engine: IvmEngine::C10,
                policy,
            }),
            (IvmEngine::Baseline, SyscallPolicy::AbiV1) => Some(Self::BASELINE_ABI_V1),
            (IvmEngine::Baseline, SyscallPolicy::AbiV1RuntimeLifecycle) => None,
        }
    }

    /// Engine that admits and executes this profile.
    #[must_use]
    pub const fn engine(self) -> IvmEngine {
        self.engine
    }

    /// Named host policy of this profile.
    #[must_use]
    pub const fn policy(self) -> SyscallPolicy {
        self.policy
    }

    /// ABI descriptor hash of this profile, computed once per process.
    #[must_use]
    pub fn abi_hash(self) -> [u8; 32] {
        static ABI_V1: OnceLock<[u8; 32]> = OnceLock::new();
        static RUNTIME_LIFECYCLE: OnceLock<[u8; 32]> = OnceLock::new();
        let cached = match (self.engine, self.policy) {
            (IvmEngine::Baseline, _) => return crate::baseline::abi_hash(),
            (IvmEngine::C10, SyscallPolicy::AbiV1) => &ABI_V1,
            (IvmEngine::C10, SyscallPolicy::AbiV1RuntimeLifecycle) => &RUNTIME_LIFECYCLE,
        };
        *cached.get_or_init(|| crate::syscalls::compute_c10_abi_hash(self.policy))
    }

    /// The implemented profile of `engine` whose ABI descriptor hash is exactly `hash`.
    ///
    /// An unknown hash selects nothing; there is no fallback profile.
    #[must_use]
    pub fn for_abi_hash(engine: IvmEngine, hash: &[u8; 32]) -> Option<Self> {
        Self::of_engine(engine)
            .iter()
            .copied()
            .find(|profile| profile.abi_hash() == *hash)
    }

    /// The implemented profile, over every engine, whose ABI descriptor hash is exactly `hash`.
    ///
    /// The engine is part of the answer: the same policy has different descriptors on different
    /// engines. An unknown hash selects nothing.
    #[must_use]
    pub fn of_abi_hash(hash: &[u8; 32]) -> Option<Self> {
        IvmEngine::ALL
            .into_iter()
            .find_map(|engine| Self::for_abi_hash(engine, hash))
    }

    /// The implemented profile an offline artifact declares, over every engine.
    ///
    /// Each candidate profile's ABI hash is compared at the fixed-header position of its own
    /// engine; the hashes are distinct, so at most one candidate matches. This is artifact
    /// inspection for offline tools that name no execution root: it parses no header, authorizes
    /// nothing and does not establish that any root selects the profile.
    #[must_use]
    pub fn declared(artifact: &[u8]) -> Option<Self> {
        IvmEngine::ALL
            .into_iter()
            .flat_map(|engine| Self::of_engine(engine).iter().copied())
            .find(|profile| {
                profile
                    .engine()
                    .header_layout()
                    .advertised_abi_hash(artifact)
                    == Some(&profile.abi_hash())
            })
    }

    /// The implemented profile an artifact declares in the fixed header of `engine`.
    ///
    /// The caller names the engine. This is artifact inspection: it authorizes nothing and does
    /// not establish that any root selects the profile.
    #[must_use]
    pub fn declared_in(engine: IvmEngine, artifact: &[u8]) -> Option<Self> {
        engine
            .header_layout()
            .advertised_abi_hash(artifact)
            .and_then(|hash| Self::for_abi_hash(engine, hash))
    }
}

impl From<SyscallPolicy> for IvmProfileId {
    fn from(policy: SyscallPolicy) -> Self {
        Self::of_policy(policy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        error::{ExecutionDeferral, Perm, VerificationRefusal},
        metadata::ProgramMetadata,
        numeric::PointerAbiFaultV1,
    };
    use iroha_data_model::executor::fault::IvmFaultKindV1 as F;

    /// Position of a variant in the declaration of [`VMError`]. The exhaustive match makes a new
    /// variant fail to compile here until the projection table below names it.
    fn variant_index(error: &VMError) -> usize {
        match error {
            VMError::ExecutionDeferred(_) => 0,
            VMError::AllocationDeferred(_) => 1,
            VMError::DecodeDeferred(_) => 2,
            VMError::AllocatorDeferred(_) => 3,
            VMError::VerificationDeferred(_) => 4,
            VMError::Metered { .. } => 5,
            VMError::OutOfGas => 6,
            VMError::OutOfMemory => 7,
            VMError::MemoryAccessViolation { .. } => 8,
            VMError::MisalignedAccess { .. } => 9,
            VMError::MemoryOutOfBounds => 10,
            VMError::DecodeError => 11,
            VMError::InvalidOpcode(_) => 12,
            VMError::UnknownSyscall(_) => 13,
            VMError::HostUnavailable => 14,
            VMError::NotImplemented { .. } => 15,
            VMError::SyscallGasQuoteExceeded { .. } => 16,
            VMError::SyscallMeteringModeMismatch { .. } => 17,
            VMError::GasCostOverflow => 18,
            VMError::SyscallOutOfGas { .. } => 19,
            VMError::NumericFault(_) => 20,
            VMError::PointerAbiFault(_) => 21,
            VMError::AssertionFailed => 22,
            VMError::ContractAbort { .. } => 23,
            VMError::ExceededMaxCycles => 24,
            VMError::InvalidMetadata => 25,
            VMError::UnsupportedProgramVersion { .. } => 26,
            VMError::UnsupportedProgramFeatureBits { .. } => 27,
            VMError::UnsupportedProgramAbiVersion { .. } => 28,
            VMError::MissingMaxCycles => 29,
            VMError::MaxCyclesWithoutZk { .. } => 30,
            VMError::ArtifactAbiHashMismatch { .. } => 31,
            VMError::GenericSyscallNotAllowed { .. } => 32,
            VMError::MissingHalt => 33,
            VMError::NullifierAlreadyUsed => 34,
            VMError::PermissionDenied => 35,
            VMError::ReentrantCall => 36,
            VMError::CallDepthExceeded => 37,
            VMError::ControlFlowViolation => 38,
            VMError::PrivacyViolation => 39,
            VMError::RegisterOutOfBounds => 40,
            VMError::NoritoInvalid => 41,
            VMError::AbiTypeNotAllowed { .. } => 42,
            VMError::HostOutputBudgetExceeded { .. } => 43,
            VMError::AmxBudgetExceeded { .. } => 44,
            VMError::ProgramVectorLengthTooLarge { .. } => 45,
            VMError::InvalidVectorLength { .. } => 46,
            VMError::VectorExtensionDisabled => 47,
            VMError::ZkExtensionDisabled => 48,
            VMError::RegisterIntegerOverflow => 49,
        }
    }
    const VARIANTS: usize = 50;

    /// One or more values of every [`VMError`] variant with the wire fault kind the C10 engine
    /// reports for it. The table is written out and does not call the projection.
    fn projection_table() -> Vec<(VMError, Option<IvmFaultKindV1>)> {
        let budget = iroha_allocation::AllocationBudget::new(1);
        let occupied = budget
            .try_reserve_bytes(1)
            .expect("reserve the whole budget");
        let refusal = budget
            .try_reserve_bytes(1)
            .expect_err("an exhausted budget refuses");
        drop(occupied);
        let bytes = norito::encode_canonical(&vec![7_u8; 16]).expect("encode sample");
        let decoded = norito::core::with_decode_limits_scope(
            norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
            || {
                norito::decode_canonical_for_admission::<Vec<u8>>(
                    &bytes,
                    norito::canonical_decode_limits(bytes.len()),
                )
            },
        )
        .expect_err("a zero allocation limit refuses locally")
        .into_local_refusal()
        .expect("local refusal");
        vec![
            (
                VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable),
                None,
            ),
            (VMError::AllocationDeferred(refusal), None),
            (VMError::DecodeDeferred(decoded.clone()), None),
            (
                VMError::AllocatorDeferred(
                    iroha_allocation::AllocatorRefusal::try_from(
                        iroha_allocation::ChargedBufferError::Allocator { requested_bytes: 1 },
                    )
                    .expect("allocator refusal"),
                ),
                None,
            ),
            (
                VMError::VerificationDeferred(VerificationRefusal::from_decode_refusal(decoded)),
                None,
            ),
            (
                VMError::Metered {
                    gas: 1,
                    source: Box::new(VMError::OutOfGas),
                },
                Some(F::OutOfGas),
            ),
            (
                VMError::Metered {
                    gas: 1,
                    source: Box::new(VMError::HostUnavailable),
                },
                None,
            ),
            (VMError::OutOfGas, Some(F::OutOfGas)),
            (VMError::OutOfMemory, Some(F::MemoryLimitExceeded)),
            (
                VMError::MemoryAccessViolation {
                    addr: 0,
                    perm: Perm::READ,
                },
                Some(F::MemoryAccessViolation),
            ),
            (
                VMError::MisalignedAccess { addr: 1 },
                Some(F::MisalignedAccess),
            ),
            (VMError::MemoryOutOfBounds, Some(F::MemoryOutOfBounds)),
            (VMError::DecodeError, Some(F::DecodeError)),
            (VMError::InvalidOpcode(0), Some(F::InvalidOpcode)),
            (VMError::UnknownSyscall(0), Some(F::UnknownSyscall)),
            (VMError::HostUnavailable, None),
            (
                VMError::NotImplemented { syscall: 0 },
                Some(F::UnsupportedSyscall),
            ),
            (
                VMError::SyscallGasQuoteExceeded {
                    quoted: 1,
                    actual: 2,
                },
                None,
            ),
            (VMError::SyscallMeteringModeMismatch { syscall: 0 }, None),
            (VMError::GasCostOverflow, Some(F::GasCostOverflow)),
            (
                VMError::SyscallOutOfGas {
                    syscall: 0,
                    phase: 0,
                },
                Some(F::OutOfGas),
            ),
            (
                VMError::NumericFault(NumericFaultV1::DivisionByZero),
                Some(F::Numeric(NumericFaultV1::DivisionByZero)),
            ),
            (
                VMError::PointerAbiFault(PointerAbiFaultV1::WrongType),
                Some(F::PointerAbi(PointerAbiFaultV1::WrongType)),
            ),
            (VMError::AssertionFailed, Some(F::AssertionFailed)),
            (
                VMError::ContractAbort {
                    contract: "c".into(),
                    name: "n".into(),
                    error_type: "e".into(),
                    schema_hash: [0; 32],
                    code: 1,
                    message: None,
                },
                None,
            ),
            (VMError::ExceededMaxCycles, Some(F::ExceededMaxCycles)),
            (VMError::InvalidMetadata, Some(F::InvalidMetadata)),
            (
                VMError::UnsupportedProgramVersion { major: 9, minor: 9 },
                None,
            ),
            (VMError::UnsupportedProgramFeatureBits { bits: 0x80 }, None),
            (VMError::UnsupportedProgramAbiVersion { version: 9 }, None),
            (VMError::MissingMaxCycles, None),
            (VMError::MaxCyclesWithoutZk { max_cycles: 1 }, None),
            // Baseline-engine failures: the C10 engine never raises them.
            (
                VMError::ProgramVectorLengthTooLarge {
                    vector_length: 65,
                    max_allowed: 64,
                },
                None,
            ),
            (VMError::InvalidVectorLength { vector_length: 65 }, None),
            (VMError::VectorExtensionDisabled, None),
            (VMError::ZkExtensionDisabled, None),
            (
                VMError::ArtifactAbiHashMismatch {
                    expected: [0; 32],
                    actual: [1; 32],
                },
                None,
            ),
            (VMError::GenericSyscallNotAllowed { syscall: 0 }, None),
            (VMError::MissingHalt, Some(F::MissingHalt)),
            (VMError::NullifierAlreadyUsed, Some(F::NullifierAlreadyUsed)),
            (VMError::PermissionDenied, Some(F::PermissionDenied)),
            (VMError::ReentrantCall, Some(F::ReentrantCall)),
            (VMError::CallDepthExceeded, Some(F::CallDepthExceeded)),
            // C10-only faults the restored baseline wire enum cannot name.
            (VMError::ControlFlowViolation, None),
            (VMError::RegisterIntegerOverflow, None),
            (VMError::PrivacyViolation, Some(F::PrivacyViolation)),
            (VMError::RegisterOutOfBounds, Some(F::RegisterOutOfBounds)),
            (VMError::NoritoInvalid, Some(F::NoritoInvalid)),
            (
                VMError::AbiTypeNotAllowed { abi: 1, type_id: 0 },
                Some(F::AbiTypeNotAllowed),
            ),
            (
                VMError::HostOutputBudgetExceeded {
                    resource: HostOutputResource::Items,
                    attempted: 2,
                    limit: 1,
                },
                Some(F::HostOutputItemsExceeded),
            ),
            (
                VMError::HostOutputBudgetExceeded {
                    resource: HostOutputResource::Bytes,
                    attempted: 2,
                    limit: 1,
                },
                Some(F::HostOutputBytesExceeded),
            ),
            (
                VMError::AmxBudgetExceeded {
                    dataspace: iroha_model_base::topology::DataSpaceId::UNIVERSAL,
                    stage: iroha_data_model::errors::AmxStage::Commit,
                    elapsed_ms: 2,
                    budget_ms: 1,
                },
                Some(F::AmxBudgetExceeded),
            ),
        ]
    }

    #[test]
    fn every_named_policy_is_bound_to_its_engine() {
        assert_eq!(IvmEngine::ALL, [IvmEngine::C10, IvmEngine::Baseline]);
        assert_eq!(
            IvmProfileId::IMPLEMENTED,
            [
                IvmProfileId::BASELINE_ABI_V1,
                IvmProfileId::C10_RUNTIME_LIFECYCLE
            ]
        );
        assert_eq!(
            IvmProfileId::of_policy(SyscallPolicy::AbiV1),
            IvmProfileId::BASELINE_ABI_V1,
            "the interface policy runs the frozen baseline engine"
        );
        assert_eq!(
            IvmProfileId::of_policy(SyscallPolicy::AbiV1RuntimeLifecycle).engine(),
            IvmEngine::C10
        );
        assert_eq!(
            IvmProfileId::of_engine(IvmEngine::C10),
            &[
                IvmProfileId::C10_ABI_V1,
                IvmProfileId::C10_RUNTIME_LIFECYCLE
            ]
        );
        assert_eq!(
            IvmProfileId::of_engine(IvmEngine::Baseline),
            &[IvmProfileId::BASELINE_ABI_V1]
        );
        for policy in [SyscallPolicy::AbiV1, SyscallPolicy::AbiV1RuntimeLifecycle] {
            let profile = IvmProfileId::of_policy(policy);
            assert_eq!(profile, IvmProfileId::from(policy));
            assert_eq!(profile.policy(), policy);
            assert_eq!(
                profile.abi_hash(),
                crate::syscalls::compute_abi_hash(policy)
            );
            assert_eq!(
                IvmProfileId::for_abi_hash(profile.engine(), &profile.abi_hash()),
                Some(profile)
            );
            assert_eq!(
                crate::syscalls::policy_for_abi_hash(&profile.abi_hash()),
                Some(policy)
            );
        }
        let c10 = IvmProfileId::C10_ABI_V1;
        assert!(!IvmProfileId::IMPLEMENTED.contains(&c10));
        assert_eq!(
            crate::syscalls::policy_for_abi_hash(&c10.abi_hash()),
            None,
            "no named policy binds C10's own ABI V1 surface"
        );
        assert_ne!(c10.abi_hash(), IvmProfileId::BASELINE_ABI_V1.abi_hash());
        assert_eq!(
            IvmProfileId::for_abi_hash(IvmEngine::C10, &c10.abi_hash()),
            Some(c10)
        );
        assert_eq!(IvmProfileId::for_abi_hash(IvmEngine::C10, &[0; 32]), None);
        assert_eq!(
            IvmProfileId::of_engine_policy(IvmEngine::C10, SyscallPolicy::AbiV1),
            Some(IvmProfileId::C10_ABI_V1)
        );
        assert_eq!(
            IvmProfileId::of_engine_policy(IvmEngine::C10, SyscallPolicy::AbiV1RuntimeLifecycle),
            Some(IvmProfileId::C10_RUNTIME_LIFECYCLE)
        );
        assert_eq!(
            IvmProfileId::of_engine_policy(IvmEngine::Baseline, SyscallPolicy::AbiV1),
            Some(IvmProfileId::BASELINE_ABI_V1)
        );
        assert_eq!(
            IvmProfileId::of_engine_policy(
                IvmEngine::Baseline,
                SyscallPolicy::AbiV1RuntimeLifecycle
            ),
            None
        );
    }

    #[test]
    fn declared_profile_reads_each_engine_at_its_own_layout() {
        let meta = ProgramMetadata {
            max_cycles: 1,
            ..ProgramMetadata::default()
        };
        let baseline = crate::baseline::encode_header(&meta, 0);
        assert_eq!(
            IvmProfileId::declared(&baseline),
            Some(IvmProfileId::BASELINE_ABI_V1)
        );
        for &profile in IvmProfileId::of_engine(IvmEngine::C10) {
            let header = ProgramMetadata::default().encode_for_policy(profile.policy());
            assert_eq!(IvmProfileId::declared(&header), Some(profile));
        }
        let mut unknown = baseline.clone();
        unknown[crate::baseline::HEADER_ABI_HASH_OFFSET] ^= 1;
        assert_eq!(IvmProfileId::declared(&unknown), None);
        assert_eq!(IvmProfileId::declared(&baseline[..48]), None);
        for profile in [
            IvmProfileId::BASELINE_ABI_V1,
            IvmProfileId::C10_ABI_V1,
            IvmProfileId::C10_RUNTIME_LIFECYCLE,
        ] {
            assert_eq!(
                IvmProfileId::of_abi_hash(&profile.abi_hash()),
                Some(profile)
            );
        }
        assert_eq!(IvmProfileId::of_abi_hash(&[0; 32]), None);
        assert_eq!(IvmProfileId::declared(&[]), None);
    }

    #[test]
    fn interface_policy_is_the_baseline_profile() {
        let baseline = IvmProfileId::BASELINE_ABI_V1;
        assert_eq!(baseline.engine(), IvmEngine::Baseline);
        assert_eq!(baseline.policy(), SyscallPolicy::AbiV1);
        assert_eq!(baseline.abi_hash(), crate::baseline::abi_hash());
        assert_eq!(
            hex::encode(baseline.abi_hash()),
            "7531b45dd9d79aabc3a0f64ab30a053af847dfe248078cff839ab2e14e3261a7"
        );
        assert_eq!(
            IvmProfileId::for_abi_hash(IvmEngine::Baseline, &baseline.abi_hash()),
            Some(baseline)
        );
        assert_eq!(
            IvmProfileId::for_abi_hash(IvmEngine::Baseline, &IvmProfileId::C10_ABI_V1.abi_hash()),
            None
        );
        assert_eq!(
            IvmProfileId::for_abi_hash(IvmEngine::C10, &baseline.abi_hash()),
            None
        );
        let layout = IvmEngine::Baseline.header_layout();
        assert_eq!(
            layout,
            HeaderLayout {
                size: 49,
                budget_offset: 8,
                abi_version_offset: 16,
                abi_hash_offset: 17,
            }
        );
        let header = crate::baseline::encode_header(&ProgramMetadata::default(), 0);
        assert_eq!(
            IvmProfileId::declared_in(IvmEngine::Baseline, &header),
            Some(baseline)
        );
        assert_eq!(IvmProfileId::declared_in(IvmEngine::C10, &header), None);
    }

    #[test]
    fn operand_decoding_follows_each_engine_layout() {
        use crate::{baseline::encoding::wide as base, encoding::wide as c10};
        let c10_scall = c10::encode_scall(0x0001_0024);
        assert_eq!(IvmEngine::C10.syscall_number(c10_scall), Some(0x0001_0024));
        assert_eq!(IvmEngine::C10.syscall_number(c10::encode_halt()), None);
        let base_scall = base::encode_sys(crate::baseline::instruction::wide::system::SCALL, 0x22);
        assert_eq!(IvmEngine::Baseline.syscall_number(base_scall), Some(0x22));
        assert_eq!(
            IvmEngine::Baseline.syscall_number(base::encode_syscallx(0x0001_0024)),
            Some(0x0001_0024)
        );
        let branch =
            base::encode_branch(crate::baseline::instruction::wide::control::BEQ, 1, 2, -3);
        assert_eq!(IvmEngine::Baseline.direct_transfer_offset(branch), Some(-3));
        let jal = base::encode_jump(crate::baseline::instruction::wide::control::JAL, 1, -300);
        assert_eq!(IvmEngine::Baseline.direct_transfer_offset(jal), Some(-300));
        assert_eq!(IvmEngine::Baseline.destination_register(jal), 1);
        assert_eq!(
            IvmEngine::Baseline.direct_transfer_offset(base::encode_halt()),
            None
        );
        let c10_jal = c10::encode_jump(crate::instruction::wide::control::JAL, 1, -300);
        assert_eq!(IvmEngine::C10.direct_transfer_offset(c10_jal), Some(-300));
        assert_eq!(IvmEngine::C10.destination_register(c10_jal), 1);
        let jr = crate::baseline::instruction::wide::control::JR;
        assert!(IvmEngine::Baseline.is_indirect_transfer(jr));
        assert!(!IvmEngine::C10.is_indirect_transfer(jr));
        assert!(IvmEngine::C10.is_indirect_transfer(crate::instruction::wide::control::JALR));
    }

    #[test]
    fn baseline_engine_projection_is_its_frozen_table() {
        assert_eq!(
            IvmEngine::Baseline.fault_kind(&VMError::AssertionFailed),
            Some(F::AssertionFailed)
        );
        assert_eq!(
            IvmEngine::Baseline.fault_kind(&VMError::VectorExtensionDisabled),
            Some(F::VectorExtensionDisabled),
            "the restored wire enum names every baseline fault"
        );
        assert_eq!(
            IvmEngine::Baseline.fault_kind(&VMError::InvalidVectorLength { vector_length: 65 }),
            Some(F::InvalidVectorLength)
        );
        assert_eq!(
            IvmEngine::Baseline.fault_kind(&VMError::ControlFlowViolation),
            None
        );
        assert_eq!(
            IvmEngine::C10.fault_kind(&VMError::ZkExtensionDisabled),
            None
        );
    }

    #[test]
    fn header_layout_is_the_engine_owner_of_the_fixed_offsets() {
        let layout = IvmEngine::C10.header_layout();
        assert_eq!(
            layout,
            HeaderLayout {
                size: 48,
                budget_offset: 7,
                abi_version_offset: 15,
                abi_hash_offset: 16,
            }
        );
        assert_eq!(layout.size, HEADER_SIZE);
        assert_eq!(layout.abi_hash_offset + 32, layout.size);
        for &profile in IvmProfileId::of_engine(IvmEngine::C10) {
            let header = ProgramMetadata::default().encode_for_policy(profile.policy());
            assert_eq!(header.len(), layout.size);
            assert_eq!(
                layout.advertised_abi_hash(&header),
                Some(&profile.abi_hash())
            );
            assert_eq!(
                IvmProfileId::declared_in(IvmEngine::C10, &header),
                Some(profile)
            );
            assert_eq!(
                header[layout.abi_version_offset], 1,
                "both profiles declare ABI version 1"
            );
        }
        let mut unknown = ProgramMetadata::default().encode();
        unknown[layout.abi_hash_offset] ^= 1;
        assert_eq!(IvmProfileId::declared_in(IvmEngine::C10, &unknown), None);
        assert_eq!(
            layout.advertised_abi_hash(&unknown[..layout.size - 1]),
            None
        );
        assert_eq!(layout.advertised_abi_hash(&[]), None);
    }

    #[test]
    fn engine_projection_reproduces_the_wire_fault_kind_of_every_error() {
        let table = projection_table();
        let covered: std::collections::BTreeSet<usize> = table
            .iter()
            .map(|(error, _)| variant_index(error))
            .collect();
        assert_eq!(
            covered,
            (0..VARIANTS).collect(),
            "one sample of every VMError variant"
        );
        for (error, expected) in &table {
            assert_eq!(
                IvmEngine::C10.fault_kind(error),
                *expected,
                "projection of {error:?}"
            );
        }
        for (error, expected) in table {
            if error.execution_deferral().is_some() {
                assert_eq!(expected, None, "a local refusal is never a fault");
            }
            let wrapped = VMError::Metered {
                gas: 17,
                source: Box::new(error),
            };
            assert_eq!(
                IvmEngine::C10.fault_kind(&wrapped),
                expected,
                "metering keeps the projection"
            );
        }
    }

    #[test]
    fn engine_projection_keeps_every_numeric_and_pointer_fault_tag() {
        for tag in 1..=13 {
            let code = NumericFaultV1::from_tag(tag).expect("numeric fault tag");
            assert_eq!(c10_numeric_fault(code), code);
            for engine in IvmEngine::ALL {
                assert_eq!(
                    engine.fault_kind(&VMError::NumericFault(code)),
                    Some(F::Numeric(code))
                );
            }
        }
        assert_eq!(
            NumericFaultV1::from_tag(crate::numeric::C10_REGISTER_INTEGER_OVERFLOW_TAG),
            None,
            "the register integer overflow is internal to C10"
        );
        for tag in 1..=11 {
            let code = PointerAbiFaultV1::from_tag(tag).expect("pointer fault tag");
            assert_eq!(
                IvmEngine::C10.fault_kind(&VMError::PointerAbiFault(code)),
                Some(F::PointerAbi(code))
            );
        }
        assert_eq!(PointerAbiFaultV1::from_tag(12), None);
    }
}
