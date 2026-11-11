//! Ratification of the adoption anchor and raising of the startup floor: the
//! `iroha.runtime.lifecycle.v1` addition (`specs/live_runtime_upgrades.md` §7.3, §13,
//! [`crate::runtime::checkpoint`]).

use iroha_data_model::{
    account::AccountId,
    isi::runtime_boundary::{RaiseRuntimeStartupFloorV1, RatifyRuntimeAdoptionAnchorV1},
};

use crate::{
    smartcontracts::{Execute, isi::Error},
    state::StateTransaction,
};

impl Execute for RatifyRuntimeAdoptionAnchorV1 {
    /// Any account relays an anchor: the validators' statements it carries are its authority.
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        crate::runtime::checkpoint::ratify_anchor(state_transaction, self.anchor)
    }
}

impl Execute for RaiseRuntimeStartupFloorV1 {
    /// Any account relays a floor: the validators' statements it carries are its authority.
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        crate::runtime::checkpoint::raise_floor(state_transaction, self.floor, self.attestations)
    }
}
