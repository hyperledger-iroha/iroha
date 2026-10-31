//! Direct native dispatch retains the exact signed source and approved executor path.
//!
//! The deployed contract queue borrows the same canonical verifier and effect owner.
//! Native transaction admission remains closed until its real security construction and
//! complete signed execution requirements are satisfied.
//! No request reference or audit digest is a permission or verification capability.
use super::{
    Executor, InstructionExecutionProfile,
    attachment_custody::{
        AttachmentCustody, CustodyError, Invocation,
        x509::{X509Binding, X509Error},
    },
};
use crate::{smartcontracts::isi::privacy, state::StateTransaction};
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    isi::{
        InstructionBox,
        error::InstructionExecutionError,
        privacy::{BootstrapPrivacyPgcAccountsV1, SubmitPrivacyZkX509CertificateV1},
    },
    transaction::{signed::TransactionSignatureError, signed_attachments::SignedAttachmentErrorV1},
};

/// The actual direct invocation or a move-only queued owner's borrowed original proof.
/// This context is never constructed from an instruction or guest-supplied digest.
pub(super) enum AttachmentExecution<'a, 'owner, 'tx> {
    Direct(&'a mut Invocation<'owner, 'tx>),
    QueuedX509 {
        binding: &'a X509Binding,
        proof: &'a [u8],
    },
}

pub(super) fn x509_instruction_error(
    state: &mut StateTransaction<'_, '_>,
    error: X509Error,
) -> InstructionExecutionError {
    use crate::execution_attempt::ExecutionAttemptError;
    match error {
        X509Error::Execution(original) => original,
        X509Error::AllocationUnavailable => state.attempt_error_to_instruction_error(
            ExecutionAttemptError::Deferred(ivm::ExecutionDeferral::AllocationUnavailable.into()),
        ),
        X509Error::Capacity(original) => state
            .attempt_error_to_instruction_error(ExecutionAttemptError::Deferred(original.into())),
        X509Error::Custody(CustodyError::Declaration(SignedAttachmentErrorV1::LocalResource(
            original,
        )))
        | X509Error::Request(
            iroha_data_model::isi::privacy::PrivacyZkX509RequestErrorV1::Attachment(
                SignedAttachmentErrorV1::LocalResource(original),
            ),
        ) => state
            .attempt_error_to_instruction_error(ExecutionAttemptError::Deferred(original.into())),
        original => InstructionExecutionError::InvalidParameter(
            iroha_data_model::isi::error::InvalidParameterError::SmartContract(format!(
                "original X.509 attachment refused: {original}"
            )),
        ),
    }
}

#[derive(Debug, thiserror::Error)]
enum DirectError {
    #[error(transparent)]
    Custody(#[from] CustodyError),
    #[error(transparent)]
    Execution(#[from] ValidationFail),
}

pub(super) fn custody_validation_error(
    state: &mut StateTransaction<'_, '_>,
    error: CustodyError,
) -> ValidationFail {
    match error {
        CustodyError::Declaration(SignedAttachmentErrorV1::LocalResource(original)) => {
            state.defer_execution(original)
        }
        CustodyError::Signature(TransactionSignatureError::UnsupportedImplementation(original)) => {
            state.defer_execution(
                ivm::error::ExecutionDeferral::CryptoImplementationUnavailable(original),
            )
        }
        original => ValidationFail::NotPermitted(format!(
            "original signed attachment custody refused: {original}"
        )),
    }
}

impl Executor {
    /// Execute one original signed direct item at its executable position.
    /// Both initial and installed executor validation remain before State application.
    pub(crate) fn execute_original_direct_instruction(
        &self,
        state: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: &InstructionBox,
        instruction_index: usize,
        custody: Option<&mut AttachmentCustody<'_>>,
    ) -> Result<(), ValidationFail> {
        let Some(custody) = custody else {
            return self.execute_instruction_with_attachment_context(
                state,
                authority,
                instruction,
                InstructionExecutionProfile::Runtime,
                None,
                None,
            );
        };
        let index = u32::try_from(instruction_index).map_err(|_| {
            ValidationFail::NotPermitted("direct instruction ordinal exceeds u32".into())
        })?;
        custody
            .require_original_direct_instruction(index, instruction)
            .map_err(|error| custody_validation_error(state, error))?;
        let result: Result<(), DirectError> = custody.direct(index, authority, |invocation| {
            self.execute_instruction_with_attachment_context(
                state,
                authority,
                instruction,
                InstructionExecutionProfile::Runtime,
                None,
                Some(AttachmentExecution::Direct(invocation)),
            )
            .map_err(Into::into)
        });
        result.map_err(|error| match error {
            DirectError::Custody(original) => custody_validation_error(state, original),
            DirectError::Execution(original) => original,
        })
    }
}

impl Executor {
    /// Apply one queued X.509 request through the ordinary initial/installed permission owner.
    pub(crate) fn execute_queued_x509_instruction(
        &self,
        state: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: &InstructionBox,
        runtime: Option<&super::ContractRuntimeExecutionContext>,
        binding: &X509Binding,
        proof: &[u8],
    ) -> Result<(), ValidationFail> {
        self.execute_instruction_with_attachment_context(
            state,
            authority,
            instruction,
            InstructionExecutionProfile::Runtime,
            runtime,
            Some(AttachmentExecution::QueuedX509 { binding, proof }),
        )
    }
}

/// The shared application point after either executor has authorized the instruction.
pub(super) fn apply_authorized_instruction(
    instruction: &InstructionBox,
    authority: &AccountId,
    state: &mut StateTransaction<'_, '_>,
    attachment: Option<AttachmentExecution<'_, '_, '_>>,
) -> Result<(), InstructionExecutionError> {
    if let Some(request) = instruction
        .as_any()
        .downcast_ref::<SubmitPrivacyZkX509CertificateV1>()
    {
        return match attachment {
            Some(AttachmentExecution::Direct(invocation)) => {
                use crate::state::StateReadOnly as _;
                let limits = state.world.privacy_consensus_policy.get().current_limits;
                let budget = state.execution_budget();
                let source_byte_limit = state
                    .world
                    .parameters
                    .get()
                    .transaction()
                    .max_tx_bytes()
                    .get();
                invocation
                    .with_x509_action(
                        request,
                        &limits,
                        source_byte_limit,
                        &budget,
                        |binding, proof| {
                            privacy::execute_x509_submission(
                                request, &binding, proof, authority, state,
                            )
                            .map_err(X509Error::from)
                        },
                    )
                    .map_err(|error| x509_instruction_error(state, error))
            }
            Some(AttachmentExecution::QueuedX509 { binding, proof }) => {
                privacy::execute_x509_submission(request, binding, proof, authority, state)
            }
            None => Err(InstructionExecutionError::InvalidParameter(
                iroha_data_model::isi::error::InvalidParameterError::SmartContract(
                    "X.509 request requires its original signed attachment owner".into(),
                ),
            )),
        };
    }
    match attachment {
        Some(AttachmentExecution::Direct(invocation)) => {
            if let Some(bootstrap) = instruction
                .as_any()
                .downcast_ref::<BootstrapPrivacyPgcAccountsV1>()
            {
                return privacy::execute_pgc_bootstrap(bootstrap, authority, state, invocation);
            }
        }
        Some(AttachmentExecution::QueuedX509 { .. }) => {
            return Err(InstructionExecutionError::InvalidParameter(
                iroha_data_model::isi::error::InvalidParameterError::SmartContract(
                    "queued X.509 attachment cannot authorize a different instruction".into(),
                ),
            ));
        }
        None => {}
    }
    crate::smartcontracts::isi::execute_borrowed_instruction(instruction, authority, state)
}
