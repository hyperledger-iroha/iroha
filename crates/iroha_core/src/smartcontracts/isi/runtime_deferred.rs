//! Explicit signed registrations that follow current runtime bindings.

use iroha_data_model::{
    account::AccountId,
    isi::{
        error::InstructionExecutionError as Error,
        runtime_deferred::{ProposeCurrentBindingMultisigV1, RegisterCurrentBindingTriggerV1},
    },
};
use iroha_executor_data_model::isi::multisig::{MultisigProposalNonce, MultisigPropose};

use crate::{smartcontracts::Execute, state::StateTransaction};

impl Execute for RegisterCurrentBindingTriggerV1 {
    fn execute(
        self,
        authority: &AccountId,
        tx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        crate::runtime::deferred_consent::require_enabled(tx)?;
        super::triggers::isi::register_trigger_internal_with_consent(
            authority,
            tx,
            self.trigger,
            None,
            true,
        )
    }
}

impl Execute for ProposeCurrentBindingMultisigV1 {
    fn execute(
        self,
        authority: &AccountId,
        tx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        crate::runtime::deferred_consent::require_enabled(tx)?;
        let nonce = MultisigProposalNonce::new(self.proposal_nonce)
            .map_err(|error| Error::InvariantViolation(error.to_string().into()))?;
        super::multisig::execute_propose_current(
            tx,
            authority,
            &MultisigPropose::new(
                self.account,
                nonce,
                self.instructions,
                self.transaction_ttl_ms,
            ),
        )
        .map_err(|error| match error {
            iroha_data_model::ValidationFail::InstructionFailed(error) => error,
            error => Error::InvariantViolation(error.to_string().into()),
        })
    }
}
