//! The execution profile and original VM failure of a single mutable attempt.
use iroha_data_model::{
    ValidationFail,
    executor::{IvmAdmissionError, fault::IvmFaultPositionV1},
    ivm_c10::{
        IvmAdmissionErrorC10V1, TransactionRejectionReasonC10V1, TransactionResultC10V1,
        ValidationFailC10V1,
    },
    transaction::{TransactionResult, error::TransactionRejectionReason},
};

use super::StateTransaction;
use crate::smartcontracts::ivm::projection::c10_admission_error_v1;

/// Ephemeral evidence captured only after the execution root grants this profile.
#[derive(Default)]
pub(crate) struct C10Execution {
    vm_failure: Option<ValidationFailC10V1>,
    baseline_failure: bool,
}

impl StateTransaction<'_, '_> {
    /// Mark the actual admitted C10 operation before it can produce effects.
    pub(crate) fn mark_c10_execution(&mut self) {
        self.c10_execution.get_or_insert_with(C10Execution::default);
    }

    /// Preserve a deterministic C10 preparation failure before generic ISI reporting.
    pub(crate) fn record_c10_admission_error(&mut self, error: &ivm::VMError) {
        if error.execution_deferral().is_none()
            && let Some(capture) = self.c10_execution.as_mut()
        {
            if !capture.baseline_failure {
                capture.vm_failure.get_or_insert_with(|| {
                    ValidationFailC10V1::IvmAdmission(c10_admission_error_v1(error))
                });
            }
        }
    }

    /// Capture a completed VM failure at the boundary that still owns its authenticated site.
    pub(crate) fn record_vm_execution(&mut self, vm: &ivm::IVM, error: &ivm::VMError) {
        if error.execution_deferral().is_some() {
            return;
        }
        let Some(capture) = self.c10_execution.as_mut() else {
            return;
        };
        if capture.vm_failure.is_some() || capture.baseline_failure {
            return;
        }
        if vm.profile_id() != ivm::IvmProfileId::C10_ABI_V1 {
            capture.baseline_failure = true;
        } else if let Some(fault) =
            vm.c10_execution_fault(error, IvmFaultPositionV1::ReturnValidation)
        {
            capture.vm_failure = Some(ValidationFailC10V1::IvmFault(fault));
        } else if !matches!(error.as_unmetered(), ivm::VMError::ContractAbort { .. }) {
            self.record_c10_admission_error(error);
        }
    }

    /// Transfer this attempt's evidence before applying or dropping its journal.
    pub(crate) fn take_c10_execution(&mut self) -> Option<C10Execution> {
        self.c10_execution.take()
    }
}

impl C10Execution {
    /// A later fee, penalty or quota determination replaced the original rejection.
    pub(super) fn replace_failure(&mut self) {
        self.vm_failure = None;
        self.baseline_failure = false;
    }

    /// Retain actual receipts, effects and the profile of the operation that failed.
    /// The error branch carries an original result belonging to another profile.
    pub(crate) fn result(
        self,
        result: TransactionResult,
    ) -> Result<TransactionResultC10V1, TransactionResult> {
        if self.baseline_failure && result.is_err() {
            return Err(result);
        }
        let TransactionResult(outcome, transfers, fee, events) = result;
        let converted = match outcome {
            Ok(steps) => Ok(steps),
            Err(reason) => match self.rejection(reason) {
                Ok(c10) => Err(c10),
                Err(original) => {
                    return Err(TransactionResult(Err(original), transfers, fee, events));
                }
            },
        };
        Ok(TransactionResultC10V1(converted, transfers, fee, events))
    }

    fn rejection(
        self,
        reason: TransactionRejectionReason,
    ) -> Result<TransactionRejectionReasonC10V1, TransactionRejectionReason> {
        use TransactionRejectionReason as B;
        use TransactionRejectionReasonC10V1 as C;
        Ok(match reason {
            B::Validation(reason) => C::Validation(match (self.vm_failure, reason) {
                (
                    Some(original),
                    ValidationFail::IvmFault(_)
                    | ValidationFail::IvmAdmission(_)
                    | ValidationFail::InstructionFailed(_),
                ) => original,
                (_, reason) => shared_validation(reason).map_err(B::Validation)?,
            }),
            B::AccountDoesNotExist(e) => C::AccountDoesNotExist(e),
            B::LimitCheck(e) => C::LimitCheck(e),
            B::InstructionExecution(e) => C::InstructionExecution(e),
            B::TriggerExecution(e) => C::TriggerExecution(e),
            original @ B::AnonymousBallot(_) => return Err(original),
        })
    }
}

fn shared_validation(reason: ValidationFail) -> Result<ValidationFailC10V1, ValidationFail> {
    use ValidationFail as B;
    use ValidationFailC10V1 as C;
    Ok(match reason {
        B::IvmAdmission(e) => C::IvmAdmission(shared_admission(e).map_err(B::IvmAdmission)?),
        original @ B::IvmFault(_) => return Err(original),
        B::NotPermitted(e) => C::NotPermitted(e),
        B::InstructionFailed(e) => C::InstructionFailed(e),
        B::ContractRejected(e) => C::ContractRejected(e),
        B::QueryFailed(e) => C::QueryFailed(e),
        B::AxtReject(e) => C::AxtReject(e),
        B::SettlementReplay(e) => C::SettlementReplay(e),
        B::InternalError(e) => C::InternalError(e),
        B::TooComplex => C::TooComplex,
    })
}

fn shared_admission(
    reason: IvmAdmissionError,
) -> Result<IvmAdmissionErrorC10V1, IvmAdmissionError> {
    use IvmAdmissionError as B;
    use IvmAdmissionErrorC10V1 as C;
    Ok(match reason {
        original @ (B::MaxCyclesExceedsFuel(_) | B::VectorLengthTooLarge(_)) => {
            return Err(original);
        }
        B::UnsupportedVersion(e) => C::UnsupportedVersion(e),
        B::UnsupportedFeatureBits(e) => C::UnsupportedFeatureBits(e),
        B::UnsupportedAbiVersion(e) => C::UnsupportedAbiVersion(e),
        B::AbiVersionNotActive(e) => C::AbiVersionNotActive(e),
        B::MaxCyclesExceedsUpperBound(e) => C::MaxCyclesExceedsUpperBound(e),
        B::DecodedInstructionCountExceeded(e) => C::DecodedInstructionCountExceeded(e),
        B::DecodedCodeSizeExceeded(e) => C::DecodedCodeSizeExceeded(e),
        B::BytecodeDecodingFailed(e) => C::BytecodeDecodingFailed(e),
        B::ManifestCodeHashMismatch(e) => C::ManifestCodeHashMismatch(e),
        B::ManifestAbiHashMismatch(e) => C::ManifestAbiHashMismatch(e),
        B::ArtifactAbiHashMismatch(e) => C::ArtifactAbiHashMismatch(e),
        B::GenericSyscallNotAllowed(e) => C::GenericSyscallNotAllowed(e),
        B::MissingMaxCycles => C::MissingMaxCycles,
        B::ManifestCodeHashMissing => C::ManifestCodeHashMissing,
        B::ManifestAbiHashMissing => C::ManifestAbiHashMissing,
        B::ManifestMalformed => C::ManifestMalformed,
    })
}
