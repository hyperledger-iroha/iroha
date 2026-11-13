//! The one-use direct signed original of an anonymous credential registration (design §3.2).
//!
//! `RegisterParliamentCredentialV1` executes only as the sole instruction of a direct signed
//! input of this network under its candidate-testing selection. The executor arms the original
//! right before the instruction executes, and the handler consumes it once: it yields the
//! locator of the signed input that carries the proof, which the credential record keeps
//! instead of the 160,148-byte proof. Contracts, callbacks, user executors and nested
//! execution never arm it, so the instruction is refused there.

use super::{StateReadOnly, StateTransaction};
use crate::execution_attempt::{ExecutionAttemptError, ExecutionDeferred};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    account::AccountId,
    governance::anonymous_ballot::{
        ParliamentCandidateTestingPolicyV1, ParliamentCredentialSourceV1,
        RegisterParliamentCredentialV1,
    },
    isi::InstructionBox,
    parameter::CustomParameter,
    transaction::{Executable, SignedTransaction, TransactionEntrypoint},
};

mod history;
#[cfg(test)]
pub(crate) use history::assert_registration_history;
pub use history::{CredentialHistoryErrorV1, resolve_credential_registration_v1};

/// Exact original coordinate. No Clone, codec or constructor outside this module.
pub(super) struct DirectRegistrationOriginal {
    signed_hash: HashOf<SignedTransaction>,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    input_index: u64,
    height: u64,
}

/// Original source or its retained refusal, consumed exactly once.
pub(super) type DirectRegistrationCapture =
    Result<DirectRegistrationOriginal, RegistrationSourceError>;

/// A refusal to take the original; local inability is not a verdict.
#[derive(Debug)]
pub(crate) enum RegistrationSourceError {
    /// The registration has no admissible original.
    Rejected(&'static str),
    /// The committed candidate-testing selection does not decode: rejected, or locally
    /// deferred.
    Policy(ExecutionAttemptError<&'static str>),
}

impl RegistrationSourceError {
    /// The original local refusal, or the error itself when it is a verdict.
    ///
    /// # Errors
    /// The error itself when it rejects the registration.
    pub(crate) fn into_deferral(self) -> Result<ExecutionDeferred, Self> {
        match self {
            Self::Policy(ExecutionAttemptError::Deferred(reason)) => Ok(reason),
            other => Err(other),
        }
    }
}

impl core::fmt::Display for RegistrationSourceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Rejected(reason) => f.write_str(reason),
            Self::Policy(error) => write!(f, "registration selection: {error:?}"),
        }
    }
}

/// A committed selected policy must decode exactly; absence never selects a default.
fn require_candidate_policy(
    parameter: Option<&CustomParameter>,
) -> Result<(), RegistrationSourceError> {
    let parameter = parameter.ok_or(RegistrationSourceError::Rejected(
        "candidate registration was not selected by genesis",
    ))?;
    ParliamentCandidateTestingPolicyV1::from_custom_parameter(parameter)
        .map(|_| ())
        .map_err(|error| {
            RegistrationSourceError::Policy(
                super::parliament_candidate_testing::candidate_policy_attempt_error(error),
            )
        })
}

impl StateTransaction<'_, '_> {
    /// Arm the original registration right before the executor runs it: the sole instruction
    /// of a direct signed input of this network, under its candidate-testing selection.
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
            || instruction
                .as_any()
                .downcast_ref::<RegisterParliamentCredentialV1>()
                .is_none()
        {
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
        // One registration per direct original. A batch needs a shared paid signed-source
        // owner, not one source per registration.
        if instruction_index != 0
            || originals.len() != 1
            || originals.first() != Some(instruction)
            || self.current_tx_hash != Some(signed.hash())
            || self.tx_call_hash != Some(Hash::from(signed.hash_as_entrypoint()))
            || self.current_network_entrypoint_hash != Some(signed.hash_as_entrypoint())
        {
            return;
        }
        self.direct_parliament_registration = Some(Ok(DirectRegistrationOriginal {
            signed_hash: signed.hash(),
            entrypoint_hash: signed.hash_as_entrypoint(),
            input_index,
            height: self.block_height(),
        }));
    }

    /// Consume the one-use original of the registration being executed: the locator of the
    /// signed input that carries its proof.
    ///
    /// No caller-provided height, input position or proof can replace the original, and the
    /// candidate-testing selection must still be committed.
    ///
    /// # Errors
    /// [`RegistrationSourceError`] when the registration has no admissible original.
    pub(crate) fn consume_direct_parliament_registration(
        &mut self,
    ) -> Result<ParliamentCredentialSourceV1, RegistrationSourceError> {
        use RegistrationSourceError::Rejected;
        let original = self.direct_parliament_registration.take().ok_or(Rejected(
            "registration needs its one-use direct signed original",
        ))??;
        if self.execution_callback_active()
            || self.current_tx_hash != Some(original.signed_hash)
            || self.tx_call_hash != Some(Hash::from(original.entrypoint_hash))
            || self.current_network_entrypoint_hash != Some(original.entrypoint_hash)
            || self.current_entrypoint_index != Some(original.input_index)
            || self.block_height() != original.height
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
        Ok(ParliamentCredentialSourceV1 {
            height: original.height,
            input_index: original.input_index,
            // The sole instruction of its input.
            instruction_index: 0,
            entrypoint_hash: original.entrypoint_hash,
        })
    }
}

#[cfg(test)]
mod tests;
