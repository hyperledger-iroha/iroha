//! Frozen definitions of the baseline engine ([`crate::IvmEngine::Baseline`]).
//!
//! The installed baseline selection executes artifacts with the header, instruction set,
//! admission precedence, fault classes and wire tags of the pre-C10 anchor `77600a4840`. Every
//! value in this module is reconstructed from that anchor and checked against the phase-1 fixtures
//! (`crates/ivm/tests/fixtures/baseline_v1`); none of it may change meaning while any selection
//! contains the baseline profile (`specs/runtime/ivm_baseline_and_c10_profiles.md`).
//!
//! - [`instruction`] and [`encoding`] are the anchor's wide-instruction tables: an 8-bit opcode
//!   and three 8-bit operand slots, 256 registers, 8-bit immediates and branch offsets, a 16-bit
//!   `JAL`, an 8-bit `SCALL` and a 24-bit `SYSTEM` syscall form, and the vector, parallel,
//!   direct-crypto and ZK-helper families.
//! - The fixed header is 49 bytes: a logical vector length follows the mode byte.
//! - The wire values are the data model's original `IvmFaultKindV1`, `IvmFaultV1` and
//!   `IvmAdmissionError` (with `VectorLengthTooLargeInfo` and `MaxCyclesExceedsFuelInfo`): the
//!   projections here return them unchanged, with their original tags.
//!
//! [`crate::IvmProfileId::of_policy`] binds [`crate::SyscallPolicy::AbiV1`] to this engine
//! ([`crate::IvmProfileId::BASELINE_ABI_V1`]). The Kotodama compiler emits these encodings for
//! its default `abi-v1` target and reproduces the anchor compiler's phase-1 corpus.

use crate::error::VmTrap;
pub mod encoding;
pub mod instruction;

use std::num::NonZeroU64;

use iroha_crypto::Hash;
use iroha_data_model::executor::{
    ArtifactAbiHashMismatchInfo, IvmAdmissionError, MaxCyclesExceedsFuelInfo,
    MaxCyclesExceedsUpperBoundInfo, UnsupportedVersionInfo, VectorLengthTooLargeInfo,
    fault::IvmFaultKindV1,
};

use crate::{
    VMError,
    error::HostOutputResource,
    metadata::{MAGIC, ProgramMetadata},
};

/// Length of the baseline fixed header: 17 bytes of execution metadata and the ABI hash.
pub const HEADER_SIZE: usize = 49;
/// Header offset of the logical vector length byte.
pub const HEADER_VECTOR_LENGTH_OFFSET: usize = 7;
/// Header offset of the little-endian `u64` `max_cycles` field.
pub const HEADER_BUDGET_OFFSET: usize = 8;
/// Header offset of the ABI version byte.
pub const HEADER_ABI_VERSION_OFFSET: usize = 16;
/// Header offset of the authenticated 32-byte ABI descriptor hash.
pub const HEADER_ABI_HASH_OFFSET: usize = 17;
/// Maximum logical vector length a baseline header may declare.
pub const VECTOR_LENGTH_MAX: u8 = 64;
/// Logical vector length a header value of zero selects.
pub const DEFAULT_VECTOR_LENGTH: usize = 4;
/// Embedded contract execution-capability bit of the baseline: deterministic vector mode.
pub const CONTRACT_FEATURE_BIT_VECTOR: u64 = 1 << 1;
/// Every embedded execution-capability bit the baseline accepts.
pub const CONTRACT_FEATURE_KNOWN_BITS: u64 =
    crate::metadata::CONTRACT_FEATURE_BIT_ZK | CONTRACT_FEATURE_BIT_VECTOR;

/// Mode bits of the baseline header.
pub mod mode {
    /// Zero-knowledge mode.
    pub const ZK: u8 = 0x01;
    /// Vector extension (vector, 128-bit memory and direct-crypto lanes).
    pub const VECTOR: u8 = 0x02;
    /// Every mode bit the baseline header accepts.
    pub const KNOWN: u8 = ZK | VECTOR;
}

/// ABI descriptor hash of the baseline profile (`7531b45d…`).
#[must_use]
pub fn abi_hash() -> [u8; 32] {
    crate::syscalls::compute_baseline_abi_hash()
}

/// Encode a baseline fixed header.
///
/// ABI version 1 carries the baseline ABI hash; any other version carries zeros, as the anchor
/// encoder did, so admission rejects it by version first.
#[must_use]
pub fn encode_header(metadata: &ProgramMetadata, vector_length: u8) -> Vec<u8> {
    let mut header = Vec::with_capacity(HEADER_SIZE);
    header.extend_from_slice(MAGIC);
    header.push(metadata.version_major);
    header.push(metadata.version_minor);
    header.push(metadata.mode);
    header.push(vector_length);
    header.extend_from_slice(&metadata.max_cycles.to_le_bytes());
    header.push(metadata.abi_version);
    let hash = match metadata.abi_version {
        1 => abi_hash(),
        _ => [0; 32],
    };
    header.extend_from_slice(&hash);
    header
}

/// Logical vector length a loaded baseline program starts with.
#[must_use]
pub fn initial_vector_length(header_vector_length: u8) -> usize {
    if header_vector_length == 0 {
        DEFAULT_VECTOR_LENGTH
    } else {
        usize::from(header_vector_length)
    }
}

/// Fault projection of the baseline engine: the anchor's `VMError::fault_kind`.
///
/// Only deterministic failures of a started invocation have a fault value. Admission errors,
/// application rejections and local host or resource refusals return `None`.
#[must_use]
pub fn fault_kind(error: &VMError) -> Option<IvmFaultKindV1> {
    use IvmFaultKindV1 as F;
    Some(match error.unmetered_trap()? {
        VmTrap::OutOfGas | VmTrap::SyscallOutOfGas { .. } => F::OutOfGas,
        VmTrap::OutOfMemory => F::MemoryLimitExceeded,
        VmTrap::MemoryAccessViolation { .. } => F::MemoryAccessViolation,
        VmTrap::MisalignedAccess { .. } => F::MisalignedAccess,
        VmTrap::MemoryOutOfBounds => F::MemoryOutOfBounds,
        VmTrap::DecodeError => F::DecodeError,
        VmTrap::InvalidOpcode(_) => F::InvalidOpcode,
        VmTrap::UnknownSyscall(_) => F::UnknownSyscall,
        VmTrap::NotImplemented { .. } => F::UnsupportedSyscall,
        VmTrap::GasCostOverflow => F::GasCostOverflow,
        VmTrap::NumericFault(code) => F::Numeric(*code),
        VmTrap::PointerAbiFault(code) => F::PointerAbi(*code),
        VmTrap::AssertionFailed => F::AssertionFailed,
        VmTrap::ExceededMaxCycles => F::ExceededMaxCycles,
        VmTrap::InvalidMetadata => F::InvalidMetadata,
        VmTrap::InvalidVectorLength { .. } => F::InvalidVectorLength,
        VmTrap::MissingHalt => F::MissingHalt,
        VmTrap::VectorExtensionDisabled => F::VectorExtensionDisabled,
        VmTrap::ZkExtensionDisabled => F::ZkExtensionDisabled,
        VmTrap::NullifierAlreadyUsed => F::NullifierAlreadyUsed,
        VmTrap::PermissionDenied => F::PermissionDenied,
        VmTrap::ReentrantCall => F::ReentrantCall,
        VmTrap::CallDepthExceeded => F::CallDepthExceeded,
        VmTrap::PrivacyViolation => F::PrivacyViolation,
        VmTrap::RegisterOutOfBounds => F::RegisterOutOfBounds,
        VmTrap::NoritoInvalid => F::NoritoInvalid,
        VmTrap::AbiTypeNotAllowed { .. } => F::AbiTypeNotAllowed,
        VmTrap::HostOutputBudgetExceeded {
            resource: HostOutputResource::Items,
            ..
        } => F::HostOutputItemsExceeded,
        VmTrap::HostOutputBudgetExceeded {
            resource: HostOutputResource::Bytes,
            ..
        } => F::HostOutputBytesExceeded,
        VmTrap::AmxBudgetExceeded { .. } => F::AmxBudgetExceeded,
        VmTrap::ContractAbort { .. }
        | VmTrap::UnsupportedProgramVersion { .. }
        | VmTrap::UnsupportedProgramFeatureBits { .. }
        | VmTrap::UnsupportedProgramAbiVersion { .. }
        | VmTrap::ProgramVectorLengthTooLarge { .. }
        | VmTrap::ArtifactAbiHashMismatch { .. }
        | VmTrap::GenericSyscallNotAllowed { .. } => return None,
        // The C10 engine alone raises these; a baseline invocation never produces them.
        VmTrap::MissingMaxCycles
        | VmTrap::MaxCyclesWithoutZk { .. }
        | VmTrap::ControlFlowViolation
        | VmTrap::RegisterIntegerOverflow => return None,
        VmTrap::Metered { .. } => unreachable!("as_unmetered removes metering wrappers"),
    })
}

/// Admission projection of the baseline engine: the anchor Core's
/// `admission_reason_from_vm_error`.
#[must_use]
pub fn admission_reason(error: VMError) -> IvmAdmissionError {
    match error {
        VMError::Trap(VmTrap::UnsupportedProgramVersion { major, minor }) => {
            IvmAdmissionError::UnsupportedVersion(UnsupportedVersionInfo { major, minor })
        }
        VMError::Trap(VmTrap::UnsupportedProgramFeatureBits { bits }) => {
            IvmAdmissionError::UnsupportedFeatureBits(bits)
        }
        VMError::Trap(VmTrap::UnsupportedProgramAbiVersion { version }) => {
            IvmAdmissionError::UnsupportedAbiVersion(version)
        }
        VMError::Trap(VmTrap::ProgramVectorLengthTooLarge {
            vector_length,
            max_allowed,
        }) => IvmAdmissionError::VectorLengthTooLarge(VectorLengthTooLargeInfo {
            vector_length,
            max_allowed,
        }),
        VMError::Trap(VmTrap::ArtifactAbiHashMismatch { expected, actual }) => {
            IvmAdmissionError::ArtifactAbiHashMismatch(ArtifactAbiHashMismatchInfo {
                expected: Hash::prehashed(expected),
                actual: Hash::prehashed(actual),
            })
        }
        VMError::Trap(VmTrap::GenericSyscallNotAllowed { syscall }) => {
            IvmAdmissionError::GenericSyscallNotAllowed(syscall)
        }
        other => IvmAdmissionError::BytecodeDecodingFailed(other.to_string()),
    }
}

/// Header policy of the baseline profile: the anchor Core's `validate_header_policy`.
///
/// # Errors
/// Returns the first violated rule in the anchor's precedence: version, unknown mode bits, ABI
/// version, vector length, missing `max_cycles`.
pub fn validate_header_policy(
    meta: &ProgramMetadata,
    vector_length: u8,
) -> Result<(), IvmAdmissionError> {
    if meta.version_major != 1 || meta.version_minor != 1 {
        return Err(IvmAdmissionError::UnsupportedVersion(
            UnsupportedVersionInfo {
                major: meta.version_major,
                minor: meta.version_minor,
            },
        ));
    }
    if meta.mode & !mode::KNOWN != 0 {
        return Err(IvmAdmissionError::UnsupportedFeatureBits(
            meta.mode & !mode::KNOWN,
        ));
    }
    if meta.abi_version != 1 {
        return Err(IvmAdmissionError::UnsupportedAbiVersion(meta.abi_version));
    }
    if vector_length != 0 && vector_length > VECTOR_LENGTH_MAX {
        return Err(IvmAdmissionError::VectorLengthTooLarge(
            VectorLengthTooLargeInfo {
                vector_length,
                max_allowed: VECTOR_LENGTH_MAX,
            },
        ));
    }
    if meta.max_cycles == 0 {
        return Err(IvmAdmissionError::MissingMaxCycles);
    }
    Ok(())
}

/// Node-ceiling check of the baseline profile: the anchor Core's `validate_cycle_ceiling`.
///
/// # Errors
/// Rejects a missing `max_cycles` or one above `upper_bound`.
pub fn validate_cycle_ceiling(
    meta: &ProgramMetadata,
    upper_bound: NonZeroU64,
) -> Result<NonZeroU64, IvmAdmissionError> {
    let cycles = NonZeroU64::new(meta.max_cycles).ok_or(IvmAdmissionError::MissingMaxCycles)?;
    if cycles > upper_bound {
        return Err(IvmAdmissionError::MaxCyclesExceedsUpperBound(
            MaxCyclesExceedsUpperBoundInfo {
                max_cycles: cycles.get(),
                upper_bound: upper_bound.get(),
            },
        ));
    }
    Ok(cycles)
}

/// Governed cycle limits of the baseline profile: the anchor Core's `validate_cycle_limits`.
///
/// # Errors
/// Rejects a missing `max_cycles`, then one above `fuel`, then one above `upper_bound`.
pub fn validate_cycle_limits(
    meta: &ProgramMetadata,
    upper_bound: NonZeroU64,
    fuel: NonZeroU64,
) -> Result<NonZeroU64, IvmAdmissionError> {
    let cycles = NonZeroU64::new(meta.max_cycles).ok_or(IvmAdmissionError::MissingMaxCycles)?;
    if cycles > fuel {
        return Err(IvmAdmissionError::MaxCyclesExceedsFuel(
            MaxCyclesExceedsFuelInfo {
                max_cycles: cycles.get(),
                fuel_limit: fuel.get(),
            },
        ));
    }
    if cycles > upper_bound {
        return Err(IvmAdmissionError::MaxCyclesExceedsUpperBound(
            MaxCyclesExceedsUpperBoundInfo {
                max_cycles: cycles.get(),
                upper_bound: upper_bound.get(),
            },
        ));
    }
    Ok(cycles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{VmDeferral, VmTrap};
    use crate::numeric::{NumericFaultV1, PointerAbiFaultV1};

    fn bare<T: norito::codec::Encode>(value: &T) -> String {
        hex::encode(norito::codec::Encode::encode(value))
    }

    #[test]
    fn fault_kind_tags_and_bare_encodings_are_the_original_wire_values() {
        // Values written out from the phase-1 oracle (`fault_kind.bare_hex`).
        assert_eq!(bare(&IvmFaultKindV1::AssertionFailed), "0c000000");
        assert_eq!(bare(&IvmFaultKindV1::InvalidVectorLength), "0f000000");
        assert_eq!(bare(&IvmFaultKindV1::VectorExtensionDisabled), "11000000");
        assert_eq!(bare(&IvmFaultKindV1::ZkExtensionDisabled), "12000000");
        assert_eq!(bare(&IvmFaultKindV1::PrivacyViolation), "15000000");
        assert_eq!(
            bare(&IvmFaultKindV1::Numeric(NumericFaultV1::DivisionByZero)),
            "0a0000000403000000"
        );
        assert_eq!(
            bare(&IvmFaultKindV1::PointerAbi(
                PointerAbiFaultV1::InvalidAddress
            )),
            "0b0000000401000000"
        );
        for kind in [
            IvmFaultKindV1::OutOfGas,
            IvmFaultKindV1::MissingHalt,
            IvmFaultKindV1::CallDepthExceeded,
            IvmFaultKindV1::Numeric(NumericFaultV1::MantissaOverflow),
        ] {
            let tag: u32 = match kind {
                IvmFaultKindV1::OutOfGas => 0,
                IvmFaultKindV1::Numeric(_) => 10,
                IvmFaultKindV1::MissingHalt => 16,
                IvmFaultKindV1::CallDepthExceeded => 29,
                _ => unreachable!("only the listed kinds"),
            };
            assert_eq!(norito::codec::Encode::encode(&kind)[..4], tag.to_le_bytes());
        }
    }

    #[test]
    fn fault_projection_is_the_anchor_table() {
        let cases = [
            (
                VMError::Trap(VmTrap::AssertionFailed),
                Some(IvmFaultKindV1::AssertionFailed),
            ),
            (
                VMError::Trap(VmTrap::InvalidVectorLength { vector_length: 65 }),
                Some(IvmFaultKindV1::InvalidVectorLength),
            ),
            (
                VMError::Trap(VmTrap::VectorExtensionDisabled),
                Some(IvmFaultKindV1::VectorExtensionDisabled),
            ),
            (
                VMError::Trap(VmTrap::ZkExtensionDisabled),
                Some(IvmFaultKindV1::ZkExtensionDisabled),
            ),
            (
                VMError::Trap(VmTrap::InvalidOpcode(0x1234)),
                Some(IvmFaultKindV1::InvalidOpcode),
            ),
            (
                VMError::Trap(VmTrap::ProgramVectorLengthTooLarge {
                    vector_length: 65,
                    max_allowed: 64,
                }),
                None,
            ),
            (VMError::Deferred(VmDeferral::HostUnavailable), None),
            (
                VMError::metered(3, VMError::Trap(VmTrap::OutOfGas)),
                Some(IvmFaultKindV1::OutOfGas),
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(fault_kind(&error), expected, "{error:?}");
        }
        assert_eq!(
            fault_kind(&VMError::Trap(VmTrap::RegisterIntegerOverflow)),
            None
        );
        assert_eq!(
            fault_kind(&VMError::Trap(VmTrap::ControlFlowViolation)),
            None
        );
    }

    #[test]
    fn admission_mirror_reproduces_the_oracle_encodings_and_texts() {
        let vector = admission_reason(VMError::Trap(VmTrap::ProgramVectorLengthTooLarge {
            vector_length: 65,
            max_allowed: 64,
        }));
        assert_eq!(bare(&vector), "050000000401410140");
        assert_eq!(vector.to_string(), "Vector length too large");
        let decode = admission_reason(VMError::Trap(VmTrap::InvalidMetadata));
        assert_eq!(
            bare(&decode),
            "0a0000001918696e76616c69642070726f6772616d206d65746164617461"
        );
        assert_eq!(
            decode.to_string(),
            "Kotodama bytecode decode failed: invalid program metadata"
        );
        let generic = admission_reason(VMError::Trap(VmTrap::GenericSyscallNotAllowed {
            syscall: 54,
        }));
        assert_eq!(bare(&generic), "110000000436000000");
        let meta = ProgramMetadata {
            max_cycles: 55_000_001,
            ..ProgramMetadata::default()
        };
        let fuel = validate_cycle_limits(
            &meta,
            NonZeroU64::new(1_000_000).unwrap(),
            NonZeroU64::new(55_000_000).unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            bare(&fuel),
            "070000001208c13b47030000000008c03b470300000000"
        );
        assert_eq!(
            fuel.to_string(),
            "`max_cycles` exceeds configured fuel limit"
        );
    }

    #[test]
    fn header_policy_keeps_the_anchor_precedence() {
        let mut meta = ProgramMetadata {
            version_major: 2,
            mode: 0x80,
            abi_version: 3,
            ..ProgramMetadata::default()
        };
        let tag = |error: IvmAdmissionError| norito::codec::Encode::encode(&error)[0];
        assert_eq!(tag(validate_header_policy(&meta, 65).unwrap_err()), 1);
        meta.version_major = 1;
        assert_eq!(tag(validate_header_policy(&meta, 65).unwrap_err()), 2);
        meta.mode = mode::ZK | mode::VECTOR;
        assert_eq!(tag(validate_header_policy(&meta, 65).unwrap_err()), 3);
        meta.abi_version = 1;
        assert_eq!(tag(validate_header_policy(&meta, 65).unwrap_err()), 5);
        assert_eq!(tag(validate_header_policy(&meta, 64).unwrap_err()), 0);
        meta.max_cycles = 1;
        assert!(validate_header_policy(&meta, 0).is_ok());
        assert_eq!(
            validate_cycle_ceiling(&meta, NonZeroU64::new(1).unwrap()).unwrap(),
            NonZeroU64::new(1).unwrap()
        );
    }

    #[test]
    fn header_encoding_has_the_baseline_layout() {
        let meta = ProgramMetadata {
            mode: mode::VECTOR,
            max_cycles: 0x0102_0304,
            ..ProgramMetadata::default()
        };
        let header = encode_header(&meta, 8);
        assert_eq!(header.len(), HEADER_SIZE);
        assert_eq!(&header[..4], MAGIC);
        assert_eq!(header[6], mode::VECTOR);
        assert_eq!(header[HEADER_VECTOR_LENGTH_OFFSET], 8);
        assert_eq!(
            header[HEADER_BUDGET_OFFSET..HEADER_ABI_VERSION_OFFSET],
            0x0102_0304_u64.to_le_bytes()
        );
        assert_eq!(header[HEADER_ABI_VERSION_OFFSET], 1);
        assert_eq!(header[HEADER_ABI_HASH_OFFSET..], abi_hash());
        assert_eq!(initial_vector_length(0), DEFAULT_VECTOR_LENGTH);
        assert_eq!(initial_vector_length(9), 9);
        let other = encode_header(
            &ProgramMetadata {
                abi_version: 2,
                ..ProgramMetadata::default()
            },
            0,
        );
        assert_eq!(other[HEADER_ABI_HASH_OFFSET..], [0; 32]);
    }
}
