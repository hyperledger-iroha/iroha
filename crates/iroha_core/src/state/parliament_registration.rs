//! Private preparation of an anonymous-ballot credential from a direct signed original.
//!
//! This module does not register an executable instruction, accept an invitation, write
//! a credential into State, seal a roster or qualify a ballot profile. Its opaque input
//! remains inactive. The prepared value is candidate-only, borrows the original State
//! cut, and has neither a wire codec nor an enactment/commit operation.
//!
//! TODO(E2): the successor purpose-bound attempt/storage and funded verifier service
//! must consume this preparation, check commitment uniqueness across primaries and
//! alternates, and retain the original history locator before any production dispatch.

use super::{StateReadOnly, StateTransaction};
use crate::execution_attempt::{
    ExecutionAttemptError, ExecutionDeferred, canonical_decode_control_attempt_error,
    canonical_policy_decode, norito_decode_attempt_error,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    account::AccountId,
    governance::{
        anonymous_ballot::{
            BallotBodyContextV1, CredentialCommitmentV1, ParliamentCandidateTestingPolicyV1,
            RegisterParliamentCredentialV1, RegistrationAuthorizationV1,
        },
        types::AssignmentId,
    },
    isi::InstructionBox,
    parameter::CustomParameter,
    transaction::{Executable, SignedTransaction, TransactionEntrypoint},
};
use iroha_parliament_ballot::registration::{RegistrationErrorV1, verify_registration_v1};
use mv::storage::StorageReadOnly;

/// Exact original coordinate. No Clone, codec or constructor outside this module.
pub(super) struct DirectRegistrationOriginal {
    signed_hash: HashOf<SignedTransaction>,
    signed_bytes: HashOf<SignedTransaction>,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    input_index: u64,
    instruction_index: usize,
    height: u64,
}

/// Original source or its retained local/semantic refusal, consumed exactly once.
pub(super) type DirectRegistrationCapture =
    Result<DirectRegistrationOriginal, RegistrationPreparationError>;

/// A refusal to prepare; local inability is not a consensus-invalid proof.
#[derive(Debug)]
pub(crate) enum RegistrationPreparationError {
    Rejected(&'static str),
    Decode(ExecutionAttemptError<&'static str>),
    Allocation(ExecutionDeferred),
    Proof(RegistrationErrorV1),
}

impl RegistrationPreparationError {
    /// Local capacity/codec/backend refusal never becomes an invalid registration.
    #[allow(
        dead_code,
        reason = "TODO(E2): successor dispatch retains this typed distinction"
    )]
    pub(crate) fn is_local(&self) -> bool {
        match self {
            Self::Allocation(_) | Self::Decode(ExecutionAttemptError::Deferred(_)) => true,
            Self::Proof(error) => error.is_local(),
            Self::Rejected(_) | Self::Decode(ExecutionAttemptError::Rejected(_)) => false,
        }
    }
}

impl core::fmt::Display for RegistrationPreparationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Rejected(reason) => f.write_str(reason),
            Self::Decode(error) => write!(f, "registration decode: {error:?}"),
            Self::Allocation(original) => write!(f, "registration allocation: {original:?}"),
            Self::Proof(original) => core::fmt::Display::fmt(original, f),
        }
    }
}

/// Compact candidate verification result, usable only while its State cut is borrowed.
///
/// The 160,148-byte proof stays in the original signed input; no copy of it survives
/// preparation. This value cannot be serialized as a credential or certificate.
#[allow(
    dead_code,
    reason = "TODO(E2): successor registration storage is not selected"
)]
pub(crate) struct PreparedCandidateRegistration<'state> {
    original: DirectRegistrationOriginal,
    body: BallotBodyContextV1,
    assignment: AssignmentId,
    commitment: CredentialCommitmentV1,
    authorization: RegistrationAuthorizationV1,
    state: core::marker::PhantomData<&'state ()>,
}

/// A committed selected policy must decode exactly; absence never selects a default.
fn require_candidate_policy(
    parameter: Option<&CustomParameter>,
) -> Result<(), RegistrationPreparationError> {
    let parameter = parameter.ok_or(RegistrationPreparationError::Rejected(
        "candidate registration was not selected by genesis",
    ))?;
    ParliamentCandidateTestingPolicyV1::from_custom_parameter(parameter)
        .map(|_| ())
        .map_err(|error| {
            RegistrationPreparationError::Decode(
                super::parliament_candidate_testing::candidate_policy_attempt_error(error),
            )
        })
}

impl StateTransaction<'_, '_> {
    /// Called only around the exact native direct signed-instruction frame.
    pub(crate) fn arm_direct_parliament_registration(
        &mut self,
        signed: &SignedTransaction,
        authority: &AccountId,
        instruction: &InstructionBox,
        instruction_index: usize,
        direct: bool,
    ) {
        self.direct_parliament_registration = None;
        if !direct
            || self.execution_callback_active()
            || signed.authority() != authority
            || signed.network_id() != Some(self.network_id())
        {
            return;
        }
        let Some(envelope) = instruction
            .as_opaque()
            .filter(|original| original.wire_id() == RegisterParliamentCredentialV1::WIRE_ID)
        else {
            return;
        };
        if envelope.frame().len() > RegisterParliamentCredentialV1::MAX_FRAME_BYTES {
            self.direct_parliament_registration =
                Some(Err(RegistrationPreparationError::Rejected(
                    "registration original exceeds its fixed input bound",
                )));
            return;
        }
        if let Err(error) = require_candidate_policy(
            self.world
                .parameters
                .get()
                .custom()
                .get(ParliamentCandidateTestingPolicyV1::parameter_id()),
        ) {
            self.direct_parliament_registration = Some(Err(error));
            return;
        }
        let Some(input_index) = self.current_entrypoint_index else {
            return;
        };
        let Executable::Instructions(originals) = signed.instructions() else {
            return;
        };
        // This unselected first slice has one registration per direct original. A
        // batch needs a shared paid signed-source owner, not N full-envelope hashes.
        if instruction_index != 0
            || originals.len() != 1
            || originals.first() != Some(instruction)
            || self.current_tx_hash != Some(signed.hash())
            || self.tx_call_hash != Some(Hash::from(signed.hash_as_entrypoint()))
            || self.current_network_entrypoint_hash != Some(signed.hash_as_entrypoint())
        {
            return;
        }
        self.direct_parliament_registration = Some(
            HashOf::try_new(signed)
                .map_err(|error| {
                    RegistrationPreparationError::Decode(norito_decode_attempt_error(
                        error,
                        |_| "registration original encoding failed",
                    ))
                })
                .map(|signed_bytes| DirectRegistrationOriginal {
                    signed_hash: signed.hash(),
                    signed_bytes,
                    entrypoint_hash: signed.hash_as_entrypoint(),
                    input_index,
                    instruction_index,
                    height: self.block_height(),
                }),
        );
    }

    /// Consume the original one-use frame and verify without mutating governance State.
    ///
    /// No caller-provided body, authority, assignment, height or proof can replace the
    /// original. Candidate selection still has to be present from native genesis.
    #[allow(
        dead_code,
        reason = "TODO(E2): no registration dispatch before successor selection"
    )]
    pub(crate) fn prepare_candidate_parliament_registration<'state>(
        &'state mut self,
        signed: &SignedTransaction,
    ) -> Result<PreparedCandidateRegistration<'state>, RegistrationPreparationError> {
        use RegistrationPreparationError::Rejected;
        let original = self.direct_parliament_registration.take().ok_or(Rejected(
            "registration needs its one-use direct signed original",
        ))??;
        let signed_bytes = HashOf::try_new(signed).map_err(|error| {
            RegistrationPreparationError::Decode(norito_decode_attempt_error(
                error,
                |_| "registration original encoding failed",
            ))
        })?;
        if self.execution_callback_active()
            || self.current_tx_hash != Some(original.signed_hash)
            || self.tx_call_hash != Some(Hash::from(original.entrypoint_hash))
            || self.current_network_entrypoint_hash != Some(original.entrypoint_hash)
            || self.current_entrypoint_index != Some(original.input_index)
            || self.block_height() != original.height
            || signed_bytes != original.signed_bytes
            || signed.hash() != original.signed_hash
            || signed.hash_as_entrypoint() != original.entrypoint_hash
        {
            return Err(Rejected("registration original source changed"));
        }
        require_candidate_policy(
            self.world
                .parameters
                .get()
                .custom()
                .get(ParliamentCandidateTestingPolicyV1::parameter_id()),
        )?;
        let Executable::Instructions(instructions) = signed.instructions() else {
            return Err(Rejected(
                "registration needs explicit original instructions",
            ));
        };
        let envelope = instructions
            .get(original.instruction_index)
            .and_then(InstructionBox::as_opaque)
            .filter(|value| value.wire_id() == RegisterParliamentCredentialV1::WIRE_ID)
            .ok_or(Rejected("registration original instruction changed"))?;
        if envelope.frame().len() > RegisterParliamentCredentialV1::MAX_FRAME_BYTES {
            return Err(Rejected(
                "registration original exceeds its fixed input bound",
            ));
        }
        let limit = RegisterParliamentCredentialV1::MAX_FRAME_BYTES;
        // Keep the decoded graph and canonical-comparison scratch paid until after
        // the proof has been consumed; this is not the future verifier-service budget.
        let _decoded_graph = self
            .world
            .sccp_committee_generations
            .allocation_budget()
            .try_reserve_bytes(limit * 4)
            .map_err(|error| RegistrationPreparationError::Allocation(error.into()))?;
        let input: RegisterParliamentCredentialV1 = canonical_policy_decode(
            Some(&self.world),
            envelope.frame(),
            norito::DecodeLimits::new(limit, limit, limit, limit * 4, 32),
        )
        .map_err(|error| {
            RegistrationPreparationError::Decode(canonical_decode_control_attempt_error(
                error,
                |_| "registration frame is not canonical",
            ))
        })?;
        let attempt = self
            .world
            .parliament_attempts
            .get(&input.governance_attempt_id)
            .ok_or(Rejected("unknown registration governance attempt"))?;
        let assignment = attempt
            .check_invitation_response(
                input.governance_attempt_id,
                input.election_attempt_id,
                signed.authority(),
                original.height,
            )
            .map_err(|_| Rejected("registration invitation is not currently admissible"))?;
        let election = attempt
            .election(&input.election_attempt_id)
            .ok_or(Rejected("unknown registration election"))?;
        let body = BallotBodyContextV1 {
            network_id: *self.network_id(),
            governance_attempt_id: attempt.attempt().id,
            election_attempt_id: election.attempt().id,
            body_role: election.attempt().request.body,
        };
        body.validate()
            .map_err(|_| Rejected("registration requires a binding jury"))?;
        verify_registration_v1(&body, signed.authority(), &assignment, &input.registration)
            .map_err(RegistrationPreparationError::Proof)?;
        Ok(PreparedCandidateRegistration {
            original,
            body,
            assignment,
            commitment: input.registration.commitment,
            authorization: input.registration.authorization,
            state: core::marker::PhantomData,
        })
    }
}

#[cfg(test)]
mod tests;
