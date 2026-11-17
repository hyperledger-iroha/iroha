//! Projection of internal VM failures onto the wire error types.
//!
//! `ivm::VMError` is an internal value. The wire types (`IvmAdmissionError` here, the fault
//! kinds through [`ivm::IvmEngine::fault_kind`]) belong to published definitions, and the
//! engine that raised a failure owns its projection onto them. A caller names that engine: the
//! engine of its resolved profile, or of the VM that ran. An engine added later supplies its
//! own projection onto its own wire types and never widens the values of this one.

use iroha_crypto::Hash;
use iroha_data_model::{
    executor::{ArtifactAbiHashMismatchInfo, IvmAdmissionError, UnsupportedVersionInfo},
    ivm_c10::IvmAdmissionErrorC10V1,
};
use ivm::IvmEngine;

/// Project a deterministic preparation failure raised by `engine` onto its admission error.
#[must_use]
pub(crate) fn admission_error(engine: IvmEngine, error: ivm::VMError) -> IvmAdmissionError {
    match engine {
        IvmEngine::C10 => c10_admission_error(error),
        IvmEngine::Baseline => baseline_admission_error(error),
    }
}

/// Admission projection of the frozen baseline engine: the anchor Core's
/// `admission_reason_from_vm_error`, onto the restored original enum and tags.
fn baseline_admission_error(error: ivm::VMError) -> IvmAdmissionError {
    ivm_abi::baseline::admission_reason(error)
}

/// Admission projection of the consolidated (C10) engine onto the baseline wire enum.
///
/// The wire enum is the baseline's. A C10 header rule it cannot name (a trace bound without the
/// ZK bit) is carried by its text. C10's own family is [`c10_admission_error_v1`].
// TODO(phase 5 dispatch): C10 admissions report through `c10_admission_error_v1` once their
// results are dispatched to the C10 families; the lifecycle profile keeps this projection.
fn c10_admission_error(error: ivm::VMError) -> IvmAdmissionError {
    match error {
        ivm::VMError::UnsupportedProgramVersion { major, minor } => {
            IvmAdmissionError::UnsupportedVersion(UnsupportedVersionInfo { major, minor })
        }
        ivm::VMError::UnsupportedProgramFeatureBits { bits } => {
            IvmAdmissionError::UnsupportedFeatureBits(bits)
        }
        ivm::VMError::UnsupportedProgramAbiVersion { version } => {
            IvmAdmissionError::UnsupportedAbiVersion(version)
        }
        ivm::VMError::MissingMaxCycles => IvmAdmissionError::MissingMaxCycles,
        ivm::VMError::ArtifactAbiHashMismatch { expected, actual } => {
            IvmAdmissionError::ArtifactAbiHashMismatch(ArtifactAbiHashMismatchInfo {
                expected: Hash::prehashed(expected),
                actual: Hash::prehashed(actual),
            })
        }
        ivm::VMError::GenericSyscallNotAllowed { syscall } => {
            IvmAdmissionError::GenericSyscallNotAllowed(syscall)
        }
        other => IvmAdmissionError::BytecodeDecodingFailed(other.to_string()),
    }
}

/// Project a deterministic preparation failure of the C10 engine onto C10's own admission
/// family ([`IvmAdmissionErrorC10V1`], the family of the `iroha.runtime.ivm_c10.v1` addition).
///
/// This is C10's admission rule exactly as C10 defined it: a trace bound without the ZK bit has
/// its own structured reason, and every other failure keeps its display text.
#[must_use]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "C10 admissions are dispatched to the C10 families with the addition's services"
    )
)]
pub(crate) fn c10_admission_error_v1(error: ivm::VMError) -> IvmAdmissionErrorC10V1 {
    match error {
        ivm::VMError::UnsupportedProgramVersion { major, minor } => {
            IvmAdmissionErrorC10V1::UnsupportedVersion(UnsupportedVersionInfo { major, minor })
        }
        ivm::VMError::UnsupportedProgramFeatureBits { bits } => {
            IvmAdmissionErrorC10V1::UnsupportedFeatureBits(bits)
        }
        ivm::VMError::UnsupportedProgramAbiVersion { version } => {
            IvmAdmissionErrorC10V1::UnsupportedAbiVersion(version)
        }
        ivm::VMError::MissingMaxCycles => IvmAdmissionErrorC10V1::MissingMaxCycles,
        ivm::VMError::MaxCyclesWithoutZk { max_cycles } => {
            IvmAdmissionErrorC10V1::MaxCyclesWithoutZk(max_cycles)
        }
        ivm::VMError::ArtifactAbiHashMismatch { expected, actual } => {
            IvmAdmissionErrorC10V1::ArtifactAbiHashMismatch(ArtifactAbiHashMismatchInfo {
                expected: Hash::prehashed(expected),
                actual: Hash::prehashed(actual),
            })
        }
        ivm::VMError::GenericSyscallNotAllowed { syscall } => {
            IvmAdmissionErrorC10V1::GenericSyscallNotAllowed(syscall)
        }
        other => IvmAdmissionErrorC10V1::BytecodeDecodingFailed(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::executor::{MaxCyclesExceedsFuelInfo, VectorLengthTooLargeInfo};

    /// Every preparation failure with its own admission reason in both engines, written out
    /// independently of the projection.
    fn structured_cases() -> Vec<(ivm::VMError, IvmAdmissionError)> {
        vec![
            (
                ivm::VMError::UnsupportedProgramVersion { major: 9, minor: 8 },
                IvmAdmissionError::UnsupportedVersion(UnsupportedVersionInfo {
                    major: 9,
                    minor: 8,
                }),
            ),
            (
                ivm::VMError::UnsupportedProgramFeatureBits { bits: 0x80 },
                IvmAdmissionError::UnsupportedFeatureBits(0x80),
            ),
            (
                ivm::VMError::UnsupportedProgramAbiVersion { version: 7 },
                IvmAdmissionError::UnsupportedAbiVersion(7),
            ),
            (
                ivm::VMError::ArtifactAbiHashMismatch {
                    expected: [3; 32],
                    actual: [4; 32],
                },
                IvmAdmissionError::ArtifactAbiHashMismatch(ArtifactAbiHashMismatchInfo {
                    expected: Hash::prehashed([3; 32]),
                    actual: Hash::prehashed([4; 32]),
                }),
            ),
            (
                ivm::VMError::GenericSyscallNotAllowed { syscall: 0x51 },
                IvmAdmissionError::GenericSyscallNotAllowed(0x51),
            ),
        ]
    }

    /// Failures without a structured admission reason: they keep their display text.
    fn unstructured_cases() -> Vec<ivm::VMError> {
        vec![
            ivm::VMError::InvalidMetadata,
            ivm::VMError::DecodeError,
            ivm::VMError::InvalidOpcode(0xFE),
            ivm::VMError::UnknownSyscall(0xF2),
            ivm::VMError::MemoryOutOfBounds,
            ivm::VMError::OutOfMemory,
            ivm::VMError::MissingHalt,
            ivm::VMError::ControlFlowViolation,
            ivm::VMError::AbiTypeNotAllowed { abi: 1, type_id: 9 },
            ivm::VMError::NoritoInvalid,
        ]
    }

    #[test]
    fn c10_projection_reproduces_every_admission_reason() {
        for (error, expected) in structured_cases() {
            assert_eq!(admission_error(IvmEngine::C10, error), expected);
        }
        assert_eq!(
            admission_error(IvmEngine::C10, ivm::VMError::MissingMaxCycles),
            IvmAdmissionError::MissingMaxCycles
        );
        for error in unstructured_cases() {
            let text = error.to_string();
            assert_eq!(
                admission_error(IvmEngine::C10, error),
                IvmAdmissionError::BytecodeDecodingFailed(text)
            );
        }
        // The baseline wire enum has no `MaxCyclesWithoutZk`; C10 carries it by its text.
        let error = ivm::VMError::MaxCyclesWithoutZk { max_cycles: 5 };
        let text = error.to_string();
        assert_eq!(
            admission_error(IvmEngine::C10, error),
            IvmAdmissionError::BytecodeDecodingFailed(text)
        );
    }

    #[test]
    fn c10_family_projection_is_the_c10_admission_rule() {
        use iroha_data_model::ivm_c10::IvmAdmissionErrorC10V1 as C10;
        // The shared structured reasons keep their meaning in the C10 family.
        for (error, expected) in structured_cases() {
            let expected = match expected {
                IvmAdmissionError::UnsupportedVersion(info) => C10::UnsupportedVersion(info),
                IvmAdmissionError::UnsupportedFeatureBits(bits) => {
                    C10::UnsupportedFeatureBits(bits)
                }
                IvmAdmissionError::UnsupportedAbiVersion(version) => {
                    C10::UnsupportedAbiVersion(version)
                }
                IvmAdmissionError::ArtifactAbiHashMismatch(info) => {
                    C10::ArtifactAbiHashMismatch(info)
                }
                IvmAdmissionError::GenericSyscallNotAllowed(number) => {
                    C10::GenericSyscallNotAllowed(number)
                }
                other => panic!("unexpected structured case {other:?}"),
            };
            assert_eq!(c10_admission_error_v1(error), expected);
        }
        assert_eq!(
            c10_admission_error_v1(ivm::VMError::MissingMaxCycles),
            C10::MissingMaxCycles
        );
        // The trace-bound rule the baseline family cannot name is structured here.
        assert_eq!(
            c10_admission_error_v1(ivm::VMError::MaxCyclesWithoutZk { max_cycles: 5 }),
            C10::MaxCyclesWithoutZk(5)
        );
        for error in unstructured_cases() {
            let text = error.to_string();
            assert_eq!(
                c10_admission_error_v1(error),
                C10::BytecodeDecodingFailed(text)
            );
        }
        // Its wire tag is C10's (6), distinct from the baseline's tag 6
        // (`MaxCyclesExceedsUpperBound`).
        assert_eq!(
            norito::codec::Encode::encode(&C10::MaxCyclesWithoutZk(5))[0],
            6
        );
    }

    #[test]
    fn baseline_projection_is_the_anchor_admission_reason() {
        for (error, expected) in structured_cases() {
            assert_eq!(admission_error(IvmEngine::Baseline, error), expected);
        }
        assert_eq!(
            admission_error(
                IvmEngine::Baseline,
                ivm::VMError::ProgramVectorLengthTooLarge {
                    vector_length: 65,
                    max_allowed: 64,
                }
            ),
            IvmAdmissionError::VectorLengthTooLarge(VectorLengthTooLargeInfo {
                vector_length: 65,
                max_allowed: 64,
            })
        );
        assert_eq!(
            admission_error(IvmEngine::Baseline, ivm::VMError::InvalidMetadata),
            IvmAdmissionError::BytecodeDecodingFailed("invalid program metadata".to_owned())
        );
        // The anchor had no structured `MissingMaxCycles` VM error; its text is carried.
        assert!(matches!(
            admission_error(IvmEngine::Baseline, ivm::VMError::MissingMaxCycles),
            IvmAdmissionError::BytecodeDecodingFailed(_)
        ));
        // The fuel rule is a Core header rule, not a VM error; its wire value is restored.
        let fuel = IvmAdmissionError::MaxCyclesExceedsFuel(MaxCyclesExceedsFuelInfo {
            max_cycles: 2,
            fuel_limit: 1,
        });
        assert_eq!(norito::codec::Encode::encode(&fuel)[0], 7);
    }

    #[test]
    fn retained_unkeyed_wrappers_are_the_interface_engine_projection() {
        let interface = super::super::profile::ResolvedIvmProfile::interface_default().engine();
        assert_eq!(interface, IvmEngine::Baseline);
        for (error, expected) in structured_cases() {
            assert_eq!(
                super::super::admission_reason_from_vm_error(error),
                expected
            );
        }
        for (error, expected) in structured_cases() {
            assert_eq!(
                super::super::program_admission_error(error),
                iroha_data_model::ValidationFail::IvmAdmission(expected)
            );
        }
        for (keyed, unkeyed) in unstructured_cases().into_iter().zip(unstructured_cases()) {
            assert_eq!(
                admission_error(interface, keyed),
                super::super::admission_reason_from_vm_error(unkeyed)
            );
        }
    }

    #[test]
    fn execution_failures_are_projected_by_the_engine_of_the_vm_that_ran() {
        use iroha_data_model::{ValidationFail, executor::fault::IvmFaultKindV1};
        let vm = ivm::IVM::try_new(100).expect("allocate a VM");
        assert_eq!(vm.engine(), IvmEngine::C10);
        for (error, kind) in [
            (ivm::VMError::OutOfGas, IvmFaultKindV1::OutOfGas),
            (
                ivm::VMError::AssertionFailed,
                IvmFaultKindV1::AssertionFailed,
            ),
        ] {
            assert_eq!(vm.engine().fault_kind(&error), Some(kind));
            let rejection = crate::execution_attempt::expect_completed_rejection(
                super::super::map_vm_error_with_context_to_validation(&vm, error),
            );
            assert!(
                matches!(rejection, ValidationFail::IvmFault(fault) if fault.kind == kind),
                "the wire fault keeps the engine's kind"
            );
        }
        // A C10 fault the baseline wire enum cannot name has no fault value yet.
        assert_eq!(
            vm.engine().fault_kind(&ivm::VMError::ControlFlowViolation),
            None
        );
        // A preparation failure that reaches the execution mapping has no fault value and
        // takes the same engine's admission projection.
        let rejection = crate::execution_attempt::expect_completed_rejection(
            super::super::map_vm_error_with_context_to_validation(
                &vm,
                ivm::VMError::MissingMaxCycles,
            ),
        );
        assert_eq!(
            rejection,
            ValidationFail::IvmAdmission(IvmAdmissionError::MissingMaxCycles)
        );
    }
}
