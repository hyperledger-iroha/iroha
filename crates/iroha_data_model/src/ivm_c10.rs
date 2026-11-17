//! Public result families of the C10 IVM profile (`iroha.runtime.ivm_c10.v1`).
//!
//! The installed baseline selection executes ABI V1 contracts on the frozen pre-C10 engine and
//! reports through the original wire types (`IvmAdmissionError`, `IvmFaultKindV1`,
//! `NumericFaultV1` and every type that encloses them). Those definitions never change meaning
//! (`specs/runtime/ivm_baseline_and_c10_profiles.md`). The consolidated engine ("C10") changed
//! the admission, fault and numeric contracts, so it reports through the new definitions here,
//! each under its own nominal schema identity, and the frozen types stay byte-identical.
//!
//! The families reproduce the C10 contract exactly: the tags of [`IvmFaultKindC10V1`] and
//! [`NumericFaultC10V1`] are those C10's ABI descriptor binds (`compute_c10_abi_hash`), and
//! [`IvmAdmissionErrorC10V1`] carries C10's header rules (a `max_cycles` trace bound only for
//! ZK artifacts). Unchanged leaves (`PointerAbiFaultV1`, the fault site, the admission detail
//! records, the instruction, query and contract rejections) are shared with the baseline.
//!
//! The rejection parents ([`ValidationFailC10V1`], [`TransactionRejectionReasonC10V1`],
//! [`TransactionStatusC10V1`]) are new definitions for the same reason: each embeds a changed
//! member.
// TODO(phase 5, `specs/runtime/ivm_baseline_and_c10_profiles.md` "Phase 5 record"): the event
// filter, trigger descriptor, manifest and trigger parents, the two registration instructions
// (`RegisterSmartContractCodeC10V1`, `RegisterTriggerC10V1`), their storage and the selected
// dispatch that reports C10 executions through these families are not yet defined.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::{boxed::Box, string::String};

use crate::{
    executor::{
        ArtifactAbiHashMismatchInfo, ContractRejection, DecodedCodeSizeLimitInfo,
        DecodedInstructionLimitInfo, ManifestAbiHashMismatchInfo, ManifestCodeHashMismatchInfo,
        MaxCyclesExceedsUpperBoundInfo, UnsupportedVersionInfo,
        fault::{IvmFaultSiteV1, NumericFaultV1, PointerAbiFaultV1},
        settlement::SettlementReplayV1,
    },
    governance::anonymous_ballot::AnonymousBallotRejectionV1,
    isi::error::InstructionExecutionError,
    nexus::AxtRejectContext,
    query::error::{FindError, QueryExecutionFail},
    transaction::error::{InstructionExecutionFail, TransactionLimitError, TriggerExecutionFail},
};

/// Stable failure codes of the C10 profile's checked numeric operations.
///
/// The baseline codes 1–13 plus [`Self::RegisterIntegerOverflow`], which C10's register
/// arithmetic raises where the baseline engine raised `AssertionFailed`.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[repr(u64)]
#[norito(tag = "kind", content = "value")]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::ivm_c10::NumericFaultC10V1")]
pub enum NumericFaultC10V1 {
    /// The canonical result exceeds the signed 512-bit integer domain.
    #[codec(index = 1)]
    MantissaOverflow = 1,
    /// The canonical exact decimal result requires a scale greater than 28.
    #[codec(index = 2)]
    ScaleOverflow = 2,
    /// Division by zero was requested.
    #[codec(index = 3)]
    DivisionByZero = 3,
    /// An exact quotient has a non-terminating decimal expansion.
    #[codec(index = 4)]
    RepeatingDecimal = 4,
    /// An exact terminating quotient needs more than 28 decimal places.
    #[codec(index = 5)]
    ExactDivisionScaleOverflow = 5,
    /// A requested output scale is outside `0..=28`.
    #[codec(index = 6)]
    InvalidScale = 6,
    /// An exact conversion would discard a fractional component or exceed its target.
    #[codec(index = 7)]
    InexactConversion = 7,
    /// A negative value was converted to the nominal quantity domain.
    #[codec(index = 8)]
    NegativeQuantity = 8,
    /// Quantity subtraction would produce a negative value.
    #[codec(index = 9)]
    QuantityUnderflow = 9,
    /// A rounded operation received an unknown rounding-mode tag.
    #[codec(index = 10)]
    InvalidRoundingMode = 10,
    /// A fallible operation received an unknown failure-mode tag.
    #[codec(index = 11)]
    InvalidFailureMode = 11,
    /// A register required to be zero by the syscall contract was nonzero.
    #[codec(index = 12)]
    ReservedRegisterNonZero = 12,
    /// Integer square root received a negative operand.
    #[codec(index = 13)]
    NegativeSquareRoot = 13,
    /// A signed 64-bit register operation overflowed (`i64::MIN / -1`, `ABS(i64::MIN)` or a
    /// ceiling quotient beyond `i64::MAX`). Distinct from the 512-bit [`Self::MantissaOverflow`];
    /// numeric syscalls never return it.
    #[codec(index = 14)]
    RegisterIntegerOverflow = 14,
}

impl NumericFaultC10V1 {
    /// Decode a stable ABI tag.
    #[must_use]
    pub const fn from_tag(tag: u64) -> Option<Self> {
        Some(match tag {
            1 => Self::MantissaOverflow,
            2 => Self::ScaleOverflow,
            3 => Self::DivisionByZero,
            4 => Self::RepeatingDecimal,
            5 => Self::ExactDivisionScaleOverflow,
            6 => Self::InvalidScale,
            7 => Self::InexactConversion,
            8 => Self::NegativeQuantity,
            9 => Self::QuantityUnderflow,
            10 => Self::InvalidRoundingMode,
            11 => Self::InvalidFailureMode,
            12 => Self::ReservedRegisterNonZero,
            13 => Self::NegativeSquareRoot,
            14 => Self::RegisterIntegerOverflow,
            _ => return None,
        })
    }

    /// Return the stable ABI tag.
    #[must_use]
    pub const fn tag(self) -> u64 {
        self as u64
    }
}

impl From<NumericFaultV1> for NumericFaultC10V1 {
    /// Every numeric code a numeric syscall returns keeps its tag in the C10 family.
    fn from(code: NumericFaultV1) -> Self {
        match code {
            NumericFaultV1::MantissaOverflow => Self::MantissaOverflow,
            NumericFaultV1::ScaleOverflow => Self::ScaleOverflow,
            NumericFaultV1::DivisionByZero => Self::DivisionByZero,
            NumericFaultV1::RepeatingDecimal => Self::RepeatingDecimal,
            NumericFaultV1::ExactDivisionScaleOverflow => Self::ExactDivisionScaleOverflow,
            NumericFaultV1::InvalidScale => Self::InvalidScale,
            NumericFaultV1::InexactConversion => Self::InexactConversion,
            NumericFaultV1::NegativeQuantity => Self::NegativeQuantity,
            NumericFaultV1::QuantityUnderflow => Self::QuantityUnderflow,
            NumericFaultV1::InvalidRoundingMode => Self::InvalidRoundingMode,
            NumericFaultV1::InvalidFailureMode => Self::InvalidFailureMode,
            NumericFaultV1::ReservedRegisterNonZero => Self::ReservedRegisterNonZero,
            NumericFaultV1::NegativeSquareRoot => Self::NegativeSquareRoot,
        }
    }
}

/// Closed consensus fault tags of the C10 profile, independent of presentation text.
///
/// The tags are those C10's ABI descriptor binds: the baseline's vector-length and extension
/// faults are absent, and [`Self::ControlFlowViolation`] is C10's.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(tag = "kind", content = "value")]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::ivm_c10::IvmFaultKindC10V1")]
pub enum IvmFaultKindC10V1 {
    /// The deterministic instruction or syscall gas allowance was exhausted.
    #[codec(index = 0)]
    OutOfGas,
    /// The guest exceeded its deterministic VM memory bounds.
    #[codec(index = 1)]
    MemoryLimitExceeded,
    /// The guest used memory with insufficient region permissions.
    #[codec(index = 2)]
    MemoryAccessViolation,
    /// The guest used an improperly aligned address.
    #[codec(index = 3)]
    MisalignedAccess,
    /// The guest accessed outside the VM memory domain.
    #[codec(index = 4)]
    MemoryOutOfBounds,
    /// Executable instructions or host input could not be decoded.
    #[codec(index = 5)]
    DecodeError,
    /// An instruction opcode was not valid.
    #[codec(index = 6)]
    InvalidOpcode,
    /// The instruction selected an unknown syscall.
    #[codec(index = 7)]
    UnknownSyscall,
    /// The canonical host intentionally rejects this known syscall.
    #[codec(index = 8)]
    UnsupportedSyscall,
    /// A deterministic gas formula overflowed its canonical domain.
    #[codec(index = 9)]
    GasCostOverflow,
    /// A checked numeric operation failed with its exact ABI code.
    #[codec(index = 10)]
    Numeric(NumericFaultC10V1),
    /// A pointer envelope failed with its exact ABI code.
    #[codec(index = 11)]
    PointerAbi(PointerAbiFaultV1),
    /// A guest assertion failed.
    #[codec(index = 12)]
    AssertionFailed,
    /// The deterministic completed-cycle allowance was exhausted.
    #[codec(index = 13)]
    ExceededMaxCycles,
    /// Guest-selected metadata was invalid during execution.
    #[codec(index = 14)]
    InvalidMetadata,
    /// The executable stream ended without a terminating instruction.
    #[codec(index = 15)]
    MissingHalt,
    /// The replicated state already contains the nullifier.
    #[codec(index = 16)]
    NullifierAlreadyUsed,
    /// The running guest attempted an unauthorized operation.
    #[codec(index = 17)]
    PermissionDenied,
    /// The guest violated deterministic privacy-tag rules.
    #[codec(index = 18)]
    PrivacyViolation,
    /// The guest selected a register outside the VM register domain.
    #[codec(index = 19)]
    RegisterOutOfBounds,
    /// A canonical Norito envelope or value was invalid.
    #[codec(index = 20)]
    NoritoInvalid,
    /// The selected pointer type is not allowed by ABI V1.
    #[codec(index = 21)]
    AbiTypeNotAllowed,
    /// The retained effect count exceeded the consensus limit.
    #[codec(index = 22)]
    HostOutputItemsExceeded,
    /// The retained effect bytes exceeded the consensus limit.
    #[codec(index = 23)]
    HostOutputBytesExceeded,
    /// The deterministic static AMX estimate exceeded its governed budget.
    #[codec(index = 24)]
    AmxBudgetExceeded,
    /// A nested call targeted an already active contract address.
    #[codec(index = 25)]
    ReentrantCall,
    /// A nested or protected direct call exceeded the canonical call-depth limit.
    #[codec(index = 26)]
    CallDepthExceeded,
    /// A protected contract returned to an untrusted PC, used an indirect transfer or `HALT`.
    #[codec(index = 27)]
    ControlFlowViolation,
}

/// Bounded deterministic failure of a started C10 invocation.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::ivm_c10::IvmFaultC10V1")]
pub struct IvmFaultC10V1 {
    /// Exact deterministic failure category.
    pub kind: IvmFaultKindC10V1,
    /// Origin retained before nested execution is unwound.
    pub site: IvmFaultSiteV1,
}

impl core::fmt::Display for IvmFaultC10V1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:?} in {} {:?} at {:?}",
            self.kind, self.site.code_hash, self.site.selector, self.site.position
        )
    }
}

/// Structured reasons for a C10 IVM admission or static validation failure.
///
/// C10's header has no logical vector length and binds `max_cycles` only as the trace bound of
/// a ZK artifact, so the baseline's vector-length and fuel reasons are absent and
/// [`Self::MaxCyclesWithoutZk`] is C10's.
#[derive(
    Debug,
    displaydoc::Display,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(tag = "kind", content = "content")]
#[repr(u32)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::ivm_c10::IvmAdmissionErrorC10V1")]
pub enum IvmAdmissionErrorC10V1 {
    /// ZK-mode IVM bytecode omits its non-zero `max_cycles` trace bound
    #[codec(index = 0)]
    MissingMaxCycles,
    /// Unsupported IVM version
    #[codec(index = 1)]
    UnsupportedVersion(UnsupportedVersionInfo),
    /// Unsupported IVM feature bits
    #[codec(index = 2)]
    UnsupportedFeatureBits(u8),
    /// Unsupported IVM ABI version
    #[codec(index = 3)]
    UnsupportedAbiVersion(u8),
    /// Program targets an ABI version that is not active under runtime governance
    #[codec(index = 4)]
    AbiVersionNotActive(u8),
    /// ZK trace bound `max_cycles` exceeds the node upper bound
    #[codec(index = 5)]
    MaxCyclesExceedsUpperBound(MaxCyclesExceedsUpperBoundInfo),
    /// Non-ZK IVM bytecode declares a `max_cycles` trace bound: {0}
    #[codec(index = 6)]
    MaxCyclesWithoutZk(u64),
    /// Decoded instruction count exceeds admission limit
    #[codec(index = 7)]
    DecodedInstructionCountExceeded(DecodedInstructionLimitInfo),
    /// Decoded byte length exceeds admission limit
    #[codec(index = 8)]
    DecodedCodeSizeExceeded(DecodedCodeSizeLimitInfo),
    /// Kotodama bytecode decode failed: {0}
    #[codec(index = 9)]
    BytecodeDecodingFailed(String),
    /// Manifest `code_hash` mismatch
    #[codec(index = 10)]
    ManifestCodeHashMismatch(ManifestCodeHashMismatchInfo),
    /// Manifest `abi_hash` mismatch with node policy
    #[codec(index = 11)]
    ManifestAbiHashMismatch(ManifestAbiHashMismatchInfo),
    /// Contract manifest omits the required `code_hash`
    #[codec(index = 12)]
    ManifestCodeHashMissing,
    /// Contract manifest omits the required `abi_hash`
    #[codec(index = 13)]
    ManifestAbiHashMissing,
    /// Transaction `contract_manifest` metadata is malformed
    #[codec(index = 14)]
    ManifestMalformed,
    /// Artifact-carried ABI hash does not match the runtime descriptor
    #[codec(index = 15)]
    ArtifactAbiHashMismatch(ArtifactAbiHashMismatchInfo),
    /// Generic IVM program invokes a contract-bound syscall
    #[codec(index = 16)]
    GenericSyscallNotAllowed(u32),
}

/// Operation validation failed in a C10 execution.
///
/// The same outcomes as `ValidationFail`; IVM admission and execution faults are C10's.
#[derive(
    Debug,
    displaydoc::Display,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    thiserror::Error,
)]
#[ignore_extra_doc_attributes]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::ivm_c10::ValidationFailC10V1")]
pub enum ValidationFailC10V1 {
    /// Operation is not permitted: {0}
    NotPermitted(String),
    /// C10 IVM admission/static checks failed with a structured reason.
    IvmAdmission(IvmAdmissionErrorC10V1),
    /// Instruction execution failed
    InstructionFailed(#[source] InstructionExecutionError),
    /// Contract rejected the operation: {0}
    ContractRejected(ContractRejection),
    /// C10 IVM execution fault: {0}
    IvmFault(IvmFaultC10V1),
    /// Query execution failed
    QueryFailed(#[source] QueryExecutionFail),
    /// Atomic cross-transaction policy rejected the request.
    AxtReject(AxtRejectContext),
    /// A payer-scoped payment identity already succeeded: {0}
    SettlementReplay(Box<SettlementReplayV1>),
    /// Operation is too complex; consider increasing the IVM runtime configuration parameters
    TooComplex,
    /// Internal error occurred, please contact the support or check the logs if you are the node owner: {0}
    ///
    /// Usually means a bug inside **Runtime Executor** or **Iroha** implementation.
    InternalError(
        /// Contained error message. Norito encodes it, exactly as it encodes the text of the
        /// baseline `ValidationFail::InternalError` (whose `#[codec(skip)]` Norito does not read).
        String,
    ),
}

/// The reason a transaction containing a C10 execution was rejected.
///
/// The same reasons as `TransactionRejectionReason`, with the C10 validation outcome.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    displaydoc::Display,
    Encode,
    Decode,
    IntoSchema,
    thiserror::Error,
)]
#[ignore_extra_doc_attributes]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::ivm_c10::TransactionRejectionReasonC10V1")]
pub enum TransactionRejectionReasonC10V1 {
    /// Account does not exist
    AccountDoesNotExist(#[source] FindError),
    /// Failed to validate transaction limits
    LimitCheck(#[source] TransactionLimitError),
    /// Validation failed
    Validation(#[source] ValidationFailC10V1),
    /// Failure in instruction execution
    InstructionExecution(#[source] InstructionExecutionFail),
    /// Execution of a time trigger or an invoked data trigger failed.
    TriggerExecution(#[source] TriggerExecutionFail),
    /// Anonymous ballot rejected: {0}
    #[codec(index = 5)]
    AnonymousBallot(#[source] AnonymousBallotRejectionV1),
}

/// Pipeline status of a transaction containing a C10 execution.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::ivm_c10::TransactionStatusC10V1")]
pub enum TransactionStatusC10V1 {
    /// Transaction was received and enqueued
    Queued,
    /// Transaction was dropped(not stored in a block)
    Expired,
    /// Transaction was stored in the block as valid
    Approved,
    /// Transaction was stored in the block as invalid
    Rejected(Box<TransactionRejectionReasonC10V1>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::fault::{IvmFaultPositionV1, IvmInvocationSelectorV1};
    use norito::codec::DecodeAll;

    /// The framed and the bare Norito encodings both decode to `value`.
    fn roundtrip<T>(value: &T)
    where
        T: norito::NoritoSerialize
            + for<'de> norito::NoritoDeserialize<'de>
            + Encode
            + DecodeAll
            + PartialEq
            + core::fmt::Debug,
    {
        let bytes = norito::to_bytes(value).expect("encode");
        assert_eq!(
            &norito::decode_from_bytes::<T>(&bytes).expect("decode"),
            value
        );
        let bare = value.encode();
        assert_eq!(
            &T::decode_all(&mut bare.as_slice()).expect("decode the bare encoding"),
            value
        );
    }

    fn json_roundtrip<T>(value: &T)
    where
        T: norito::json::JsonSerialize
            + norito::json::JsonDeserialize
            + PartialEq
            + core::fmt::Debug,
    {
        let json = norito::json::to_json(value).expect("json");
        assert_eq!(
            &norito::json::from_json::<T>(&json).expect("from json"),
            value
        );
    }

    fn every_numeric() -> Vec<NumericFaultC10V1> {
        (1..=14)
            .map(|tag| NumericFaultC10V1::from_tag(tag).expect("defined tag"))
            .collect()
    }

    fn every_kind() -> Vec<IvmFaultKindC10V1> {
        use IvmFaultKindC10V1 as K;
        let mut kinds = vec![
            K::OutOfGas,
            K::MemoryLimitExceeded,
            K::MemoryAccessViolation,
            K::MisalignedAccess,
            K::MemoryOutOfBounds,
            K::DecodeError,
            K::InvalidOpcode,
            K::UnknownSyscall,
            K::UnsupportedSyscall,
            K::GasCostOverflow,
            K::AssertionFailed,
            K::ExceededMaxCycles,
            K::InvalidMetadata,
            K::MissingHalt,
            K::NullifierAlreadyUsed,
            K::PermissionDenied,
            K::PrivacyViolation,
            K::RegisterOutOfBounds,
            K::NoritoInvalid,
            K::AbiTypeNotAllowed,
            K::HostOutputItemsExceeded,
            K::HostOutputBytesExceeded,
            K::AmxBudgetExceeded,
            K::ReentrantCall,
            K::CallDepthExceeded,
            K::ControlFlowViolation,
        ];
        kinds.extend(every_numeric().into_iter().map(K::Numeric));
        kinds.extend(
            (1..=11).map(|tag| K::PointerAbi(PointerAbiFaultV1::from_tag(tag).expect("tag"))),
        );
        kinds
    }

    fn every_admission() -> Vec<IvmAdmissionErrorC10V1> {
        use IvmAdmissionErrorC10V1 as A;
        use iroha_crypto::Hash;
        vec![
            A::MissingMaxCycles,
            A::UnsupportedVersion(UnsupportedVersionInfo { major: 2, minor: 1 }),
            A::UnsupportedFeatureBits(0x80),
            A::UnsupportedAbiVersion(2),
            A::AbiVersionNotActive(2),
            A::MaxCyclesExceedsUpperBound(MaxCyclesExceedsUpperBoundInfo {
                max_cycles: 9,
                upper_bound: 8,
            }),
            A::MaxCyclesWithoutZk(7),
            A::DecodedInstructionCountExceeded(DecodedInstructionLimitInfo {
                decoded_instructions: 5,
                limit: 4,
            }),
            A::DecodedCodeSizeExceeded(DecodedCodeSizeLimitInfo {
                decoded_bytes: 5,
                limit: 4,
            }),
            A::BytecodeDecodingFailed("bad".into()),
            A::ManifestCodeHashMismatch(ManifestCodeHashMismatchInfo {
                expected: Hash::new(b"e"),
                actual: Hash::new(b"a"),
            }),
            A::ManifestAbiHashMismatch(ManifestAbiHashMismatchInfo {
                expected: Hash::new(b"e"),
                actual: Hash::new(b"a"),
            }),
            A::ManifestCodeHashMissing,
            A::ManifestAbiHashMissing,
            A::ManifestMalformed,
            A::ArtifactAbiHashMismatch(ArtifactAbiHashMismatchInfo {
                expected: Hash::new(b"e"),
                actual: Hash::new(b"a"),
            }),
            A::GenericSyscallNotAllowed(0x47),
        ]
    }

    fn sample_fault(kind: IvmFaultKindC10V1) -> IvmFaultC10V1 {
        IvmFaultC10V1 {
            kind,
            site: IvmFaultSiteV1 {
                code_hash: iroha_crypto::Hash::new(b"exact artifact"),
                selector: IvmInvocationSelectorV1::Entrypoint(3),
                position: IvmFaultPositionV1::Execute { pc_offset: 40 },
            },
        }
    }

    #[test]
    fn numeric_faults_roundtrip_with_their_c10_tags() {
        for code in every_numeric() {
            roundtrip(&code);
            json_roundtrip(&code);
            assert_eq!(NumericFaultC10V1::from_tag(code.tag()), Some(code));
        }
        assert_eq!(NumericFaultC10V1::RegisterIntegerOverflow.tag(), 14);
        assert_eq!(NumericFaultC10V1::from_tag(0), None);
        assert_eq!(NumericFaultC10V1::from_tag(15), None);
        // Every baseline code keeps its tag in the C10 family.
        for tag in 1..=13 {
            let baseline = NumericFaultV1::from_tag(tag).expect("baseline tag");
            assert_eq!(NumericFaultC10V1::from(baseline).tag(), tag);
        }
        assert_eq!(
            norito::json::to_json(&IvmFaultKindC10V1::Numeric(
                NumericFaultC10V1::RegisterIntegerOverflow
            ))
            .unwrap(),
            r#"{"kind":"Numeric","value":{"kind":"RegisterIntegerOverflow","value":null}}"#
        );
    }

    #[test]
    fn fault_kinds_and_faults_roundtrip_binary_and_json() {
        for kind in every_kind() {
            roundtrip(&kind);
            json_roundtrip(&kind);
            let fault = sample_fault(kind);
            roundtrip(&fault);
            json_roundtrip(&fault);
            let json = norito::json::to_json(&fault).unwrap();
            let injected = json.replacen('{', "{\"message\":\"untrusted\",", 1);
            assert!(norito::json::from_json::<IvmFaultC10V1>(&injected).is_err());
        }
    }

    #[test]
    fn c10_fault_tags_are_the_ones_the_c10_descriptor_binds() {
        // The kind tags of `fault_layout` in C10's ABI descriptor (`compute_c10_abi_hash`).
        let source = include_str!("ivm_c10.rs");
        let body = source
            .split_once("pub enum IvmFaultKindC10V1 {")
            .unwrap()
            .1
            .split_once("\n}")
            .unwrap()
            .0;
        let lines: Vec<_> = body.lines().map(str::trim).collect();
        let actual: Vec<_> = lines
            .windows(2)
            .filter_map(|pair| {
                let index = pair[0]
                    .strip_prefix("#[codec(index = ")?
                    .strip_suffix(")]")?
                    .parse::<u32>()
                    .unwrap();
                let name = pair[1].split(['(', ',']).next().unwrap();
                Some(format!("{name}{index}"))
            })
            .collect();
        assert_eq!(
            actual.join("|"),
            "OutOfGas0|MemoryLimitExceeded1|MemoryAccessViolation2|MisalignedAccess3|\
             MemoryOutOfBounds4|DecodeError5|InvalidOpcode6|UnknownSyscall7|UnsupportedSyscall8|\
             GasCostOverflow9|Numeric10|PointerAbi11|AssertionFailed12|ExceededMaxCycles13|\
             InvalidMetadata14|MissingHalt15|NullifierAlreadyUsed16|PermissionDenied17|\
             PrivacyViolation18|RegisterOutOfBounds19|NoritoInvalid20|AbiTypeNotAllowed21|\
             HostOutputItemsExceeded22|HostOutputBytesExceeded23|AmxBudgetExceeded24|\
             ReentrantCall25|CallDepthExceeded26|ControlFlowViolation27"
        );
    }

    #[test]
    fn admission_errors_roundtrip_with_their_c10_tags() {
        for (tag, error) in every_admission().into_iter().enumerate() {
            roundtrip(&error);
            json_roundtrip(&error);
            let bytes = error.encode();
            assert_eq!(
                u32::from_le_bytes(bytes[..4].try_into().unwrap()),
                u32::try_from(tag).unwrap(),
                "{error:?} keeps its C10 tag"
            );
        }
        let unknown = 17_u32.encode();
        assert!(
            IvmAdmissionErrorC10V1::decode_all(&mut unknown.as_slice()).is_err(),
            "unknown admission error tags must fail closed"
        );
    }

    #[test]
    fn rejection_parents_roundtrip() {
        let mut failures: Vec<ValidationFailC10V1> = every_admission()
            .into_iter()
            .map(ValidationFailC10V1::IvmAdmission)
            .chain(
                every_kind()
                    .into_iter()
                    .map(|kind| ValidationFailC10V1::IvmFault(sample_fault(kind))),
            )
            .collect();
        failures.extend([
            ValidationFailC10V1::NotPermitted("denied".into()),
            ValidationFailC10V1::ContractRejected(ContractRejection {
                contract: "Seiyaku".into(),
                error_type: "E".into(),
                schema_hash: [7; 32],
                name: "Low".into(),
                code: 18,
                message: Some("too low".into()),
            }),
            ValidationFailC10V1::TooComplex,
        ]);
        for failure in failures {
            roundtrip(&failure);
            let reason = TransactionRejectionReasonC10V1::Validation(failure);
            roundtrip(&reason);
            let status = TransactionStatusC10V1::Rejected(Box::new(reason));
            roundtrip(&status);
        }
        for status in [
            TransactionStatusC10V1::Queued,
            TransactionStatusC10V1::Expired,
            TransactionStatusC10V1::Approved,
        ] {
            roundtrip(&status);
        }
        roundtrip(&TransactionRejectionReasonC10V1::LimitCheck(
            TransactionLimitError {
                reason: "too many".into(),
            },
        ));
        roundtrip(&TransactionRejectionReasonC10V1::TriggerExecution(
            TriggerExecutionFail::MaxDepthExceeded,
        ));
        // The internal error text is encoded, as the baseline `ValidationFail` encodes it.
        roundtrip(&ValidationFailC10V1::InternalError("internal".into()));
        let baseline = norito::codec::Encode::encode(
            &crate::executor::ValidationFail::InternalError("internal".into()),
        );
        let c10 = ValidationFailC10V1::InternalError("internal".into()).encode();
        assert_eq!(
            c10, baseline,
            "an unchanged variant keeps its bare encoding"
        );
    }

    #[test]
    fn c10_families_have_their_own_schema_identities() {
        use norito::schema::identity::frame_hash;
        let pairs = [
            (
                frame_hash::<IvmFaultKindC10V1>(),
                frame_hash::<crate::executor::fault::IvmFaultKindV1>(),
            ),
            (
                frame_hash::<IvmFaultC10V1>(),
                frame_hash::<crate::executor::fault::IvmFaultV1>(),
            ),
            (
                frame_hash::<NumericFaultC10V1>(),
                frame_hash::<NumericFaultV1>(),
            ),
            (
                frame_hash::<ValidationFailC10V1>(),
                frame_hash::<crate::executor::ValidationFail>(),
            ),
            (
                frame_hash::<TransactionRejectionReasonC10V1>(),
                frame_hash::<crate::transaction::error::TransactionRejectionReason>(),
            ),
            (
                frame_hash::<TransactionStatusC10V1>(),
                frame_hash::<crate::events::pipeline::TransactionStatus>(),
            ),
        ];
        for (c10, baseline) in pairs {
            assert_ne!(
                c10, baseline,
                "a C10 family never reuses a baseline identity"
            );
        }
    }
}
