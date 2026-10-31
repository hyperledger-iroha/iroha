//! Bind the canonical X.509 request to one original signed invocation and proof.

use super::{ActionRequest, CustodyError, Invocation, InvocationGuard};
use crate::state::{StateReadOnly, StateTransaction};
use iroha_allocation::{AllocationBudget, AllocationRefusal, AllocationReservation};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    isi::{
        error::InstructionExecutionError,
        privacy::{PrivacyZkX509RequestErrorV1, SubmitPrivacyZkX509CertificateV1},
    },
    privacy::{IrohaZkX509StarkP256StatementV1, PrivacyConsensusLimitsV1},
    transaction::{
        SignedTransaction,
        signed_attachments::{
            ActionAttachmentRefV1, ActionOperationV1, ActionRoleV1, AttachmentContentHashV1,
            AttachmentKindV1, AttachmentRoleV1,
        },
    },
};

/// Typed original causes; resource refusals are never converted to proof invalidity.
#[derive(Debug, thiserror::Error)]
pub enum X509Error {
    #[error(transparent)]
    Custody(#[from] CustodyError),
    #[error(transparent)]
    Request(#[from] PrivacyZkX509RequestErrorV1),
    #[error(transparent)]
    Capacity(#[from] AllocationRefusal),
    #[error("admitted X.509 attachment backing could not be allocated")]
    AllocationUnavailable,
    #[error("X.509 attachment must contain one bounded native proof")]
    ProofShape,
    #[error("X.509 attachment request is not the original source or authority")]
    Source,
    #[error(transparent)]
    Execution(#[from] InstructionExecutionError),
}

/// Move-only materialized statement, exact signed proof identity and original source.
///
/// This is custody, not proof acceptance. The statement's bounded retained clone
/// keeps its conservative original-pool allowance until after its destruction.
/// No method extracts or clones this owner.
pub struct X509Binding {
    statement: IrohaZkX509StarkP256StatementV1,
    source: HashOf<SignedTransaction>,
    call: Hash,
    network: NetworkId,
    reference: ActionAttachmentRefV1,
    proof_hash: AttachmentContentHashV1,
    proof_len: usize,
    _statement_allowance: AllocationReservation,
}
impl X509Binding {
    pub(crate) fn statement(&self) -> &IrohaZkX509StarkP256StatementV1 {
        &self.statement
    }
    pub(crate) fn require_original_execution(
        &self,
        request: &SubmitPrivacyZkX509CertificateV1,
        proof: &[u8],
        authority: &AccountId,
        state: &StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        if state.execution_callback_active()
            || state.current_tx_hash != Some(self.source)
            || state.tx_call_hash != Some(self.call)
            || *state.network_id() != self.network
            || authority != &self.statement.wallet_account
            || request.proof_ref != self.reference
            || proof.len() != self.proof_len
            || AttachmentContentHashV1::of(proof) != self.proof_hash
        {
            return Err(InstructionExecutionError::InvalidParameter(
                iroha_data_model::isi::error::InvalidParameterError::SmartContract(
                    "X.509 attachment lost its original signed source or authority".into(),
                ),
            ));
        }
        // The model's request/materialized equality check follows before native verification.
        Ok(())
    }
}

fn original_encoding_error(original: norito::core::DecodeAttemptError) -> X509Error {
    use iroha_data_model::transaction::signed_attachments::SignedAttachmentErrorV1;
    let original = match original.into_local_refusal() {
        Ok(refusal) => SignedAttachmentErrorV1::LocalResource(refusal),
        Err(_) => SignedAttachmentErrorV1::CanonicalEncodingFailed,
    };
    X509Error::Custody(CustodyError::Declaration(original))
}

impl Invocation<'_, '_> {
    /// Consume once, perform the caller's direct native work or funded queue transfer,
    /// then complete custody. Any failure/unwind poisons this original attempt.
    pub(crate) fn with_x509_action<T>(
        &mut self,
        request: &SubmitPrivacyZkX509CertificateV1,
        limits: &PrivacyConsensusLimitsV1,
        source_byte_limit: u64,
        budget: &AllocationBudget,
        visit: impl FnOnce(X509Binding, &[u8]) -> Result<T, X509Error>,
    ) -> Result<T, X509Error> {
        let mut guard = InvocationGuard {
            frame: self,
            completed: false,
        };
        // Reserve from a type-owned fixed ceiling before validation can clone
        // the statement or canonical public-input hashing can allocate scratch.
        // No allocating size measurement precedes this original-pool admission.
        let frame_ceiling = SubmitPrivacyZkX509CertificateV1::canonical_frame_byte_limit_v1();
        let allowance = norito::canonical_decode_limits(frame_ceiling)
            .max_total_allocated_bytes()
            .checked_add(frame_ceiling)
            .ok_or(AllocationRefusal::DemandOverflow)?;
        let statement_allowance = budget.try_reserve_bytes(allowance)?;
        let source_ceiling =
            usize::try_from(source_byte_limit).map_err(|_| AllocationRefusal::DemandOverflow)?;
        let source_workspace = norito::canonical_decode_limits(source_ceiling)
            .max_total_allocated_bytes()
            .checked_add(
                source_ceiling
                    .checked_mul(2)
                    .ok_or(AllocationRefusal::DemandOverflow)?,
            )
            .ok_or(AllocationRefusal::DemandOverflow)?;
        // The original ordered intent normalizes the whole executable and all
        // independent transaction fields, not merely this request. The committed
        // complete signed-transaction byte ceiling funds that temporary graph/encoding.
        let source_allowance = budget.try_reserve_bytes(source_workspace)?;
        let source_len = norito::core::classify_decode_attempt(|| {
            norito::canonical_frame_len(guard.frame.owner.source)
        })
        .map_err(original_encoding_error)?;
        if source_len > source_ceiling {
            return Err(X509Error::Source);
        }
        request.validate_proof_free_v1(limits)?;
        let operation = ActionOperationV1::NativeInstruction(
            SubmitPrivacyZkX509CertificateV1::WIRE_ID.to_owned(),
        );
        let action_request = ActionRequest {
            index: request.proof_ref.action_index,
            operation: &operation,
            role: ActionRoleV1::ProofEffect,
            public_inputs_digest: request.public_inputs_digest_v1()?,
        };
        let action = guard.frame.action(action_request)?;
        let [reference] = action.attachments.as_slice() else {
            return Err(X509Error::ProofShape);
        };
        let proof_len = usize::try_from(reference.length).map_err(|_| X509Error::ProofShape)?;
        if request.proof_ref.slot_index != 0
            || proof_len == 0
            || reference.length > u64::from(limits.max_proof_bytes_per_action)
            || reference.kind != AttachmentKindV1::Proof
            || reference.role != AttachmentRoleV1::ActionProof
        {
            return Err(X509Error::ProofShape);
        }
        let proof_hash = reference.content_hash;
        let intent = guard
            .frame
            .owner
            .source
            .ordered_intent_digest_v1()
            .map_err(CustodyError::from)?;
        let statement = request.materialize_statement_v1(action, intent, limits)?;
        if statement.wallet_account != *guard.frame.actor
            || guard.frame.owner.source.network_id() != Some(&statement.context.network_id)
        {
            return Err(X509Error::Source);
        }
        let call = norito::core::classify_decode_attempt(|| {
            guard.frame.owner.source.try_hash_as_entrypoint()
        })
        .map_err(original_encoding_error)?;
        // Source normalization/hash scratch has been destroyed; only the bounded
        // selected statement is retained by the binding below.
        drop(source_allowance);
        let binding = X509Binding {
            source: HashOf::from_untyped_unchecked(Hash::from(call)),
            call: Hash::from(call),
            network: statement.context.network_id,
            statement,
            reference: request.proof_ref,
            proof_hash,
            proof_len,
            _statement_allowance: statement_allowance,
        };
        let handle = guard.frame.open(
            action_request,
            0,
            AttachmentKindV1::Proof,
            AttachmentRoleV1::ActionProof,
        )?;
        let proof = guard.frame.consume(
            action_request,
            handle,
            AttachmentKindV1::Proof,
            AttachmentRoleV1::ActionProof,
        )?;
        let value = visit(binding, proof)?;
        guard.frame.finish_action(action_request)?;
        guard.completed = true;
        Ok(value)
    }
}

#[cfg(test)]
pub mod tests;
