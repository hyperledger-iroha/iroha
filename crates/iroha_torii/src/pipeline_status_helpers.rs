impl PipelineStatusKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Approved => "Approved",
            Self::Committed => "Committed",
            Self::Applied => "Applied",
            Self::Rejected => "Rejected",
            Self::Expired => "Expired",
        }
    }
    fn rank(self) -> u8 {
        match self {
            Self::Queued => 0,
            Self::Approved => 1,
            Self::Expired => 2,
            Self::Rejected => 3,
            Self::Committed => 4,
            Self::Applied => 5,
        }
    }
    fn is_terminal(self) -> bool {
        matches!(self, Self::Applied | Self::Rejected | Self::Expired)
    }
}
/// Stable machine-readable class of a transaction rejection (event streams).
fn transaction_rejection_code(
    reason: &iroha_data_model::transaction::error::TransactionRejectionReason,
) -> &'static str {
    use iroha_data_model::transaction::error::TransactionRejectionReason;
    match reason {
        TransactionRejectionReason::AccountDoesNotExist(_) => "account_does_not_exist",
        TransactionRejectionReason::LimitCheck(_) => "limit_check",
        TransactionRejectionReason::Validation(iroha_data_model::ValidationFail::IvmFault(_)) => {
            "ivm_fault"
        }
        TransactionRejectionReason::Validation(_) => "validation",
        TransactionRejectionReason::InstructionExecution(_) => "instruction_execution",
        TransactionRejectionReason::TriggerExecution(_) => "trigger_execution",
        TransactionRejectionReason::AnonymousBallot(_) => "anonymous_ballot",
    }
}
fn pipeline_rejection_summary<'a>(
    reason: impl Into<iroha_data_model::block::execution_output::TransactionRejectionRef<'a>>,
) -> &'static str {
    use iroha_data_model::{
        block::execution_output::TransactionRejectionRef::{Baseline, C10},
        ivm_c10::{TransactionRejectionReasonC10V1 as C, ValidationFailC10V1},
        transaction::error::TransactionRejectionReason as B,
    };
    match reason.into() {
        Baseline(B::AccountDoesNotExist(_)) | C10(C::AccountDoesNotExist(_)) => "Account does not exist.",
        Baseline(B::LimitCheck(_)) | C10(C::LimitCheck(_)) => "Transaction limits were exceeded.",
        Baseline(B::Validation(iroha_data_model::ValidationFail::IvmFault(_))) | C10(C::Validation(ValidationFailC10V1::IvmFault(_))) => "IVM execution failed.",
        Baseline(B::Validation(_)) | C10(C::Validation(_)) => "Transaction validation failed.",
        Baseline(B::InstructionExecution(_)) | C10(C::InstructionExecution(_)) => "Instruction execution failed.",
        Baseline(B::TriggerExecution(_)) | C10(C::TriggerExecution(_)) => "Trigger execution failed.",
        Baseline(B::AnonymousBallot(reason)) | C10(C::AnonymousBallot(reason)) => match reason {
            iroha_data_model::governance::anonymous_ballot::AnonymousBallotRejectionV1::OutsideCastingWindow => "Anonymous ballot is outside its casting window.",
            iroha_data_model::governance::anonymous_ballot::AnonymousBallotRejectionV1::DuplicateNullifier => "Anonymous ballot nullifier was already accepted.",
        },
    }
}
