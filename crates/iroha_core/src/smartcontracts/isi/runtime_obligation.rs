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
