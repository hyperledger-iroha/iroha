//! Kernel registration and completion of obligations: the `iroha.runtime.lifecycle.v1` addition
//! (`specs/live_runtime_upgrades.md` §7.2, [`crate::runtime::obligations::registry`]).

use iroha_data_model::{
    account::AccountId,
    isi::runtime_obligation::{CompleteRuntimeObligationV1, RegisterRuntimeObligationV1},
};

use crate::{
    smartcontracts::{Execute, isi::Error},
    state::StateTransaction,
};

impl Execute for RegisterRuntimeObligationV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        crate::runtime::obligations::registry::register(state_transaction, authority, &self)
    }
}

impl Execute for CompleteRuntimeObligationV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        crate::runtime::obligations::registry::complete(state_transaction, authority, &self)
    }
}

// Native dispatch has no opaque invocation authority. Only the Executor boundary carrying
// the original contract frame can enter the independently selected kernel operation.
impl Execute for iroha_data_model::isi::runtime_obligation::RegisterContractRuntimeObligationV1 {
    fn execute(
        self,
        _authority: &AccountId,
        _state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        Err(Error::InvariantViolation(
            "contract obligation registration requires authenticated invocation context".into(),
        ))
    }
}
