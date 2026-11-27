//! Exact current and predecessor child administration custody at snapshot restoration.
//!
//! Decoding validates shape and retained foundation only. The existing certified-history
//! restoration owner remains responsible for authenticating the snapshot and its native tip.

use super::*;

pub(super) fn validate_world(
    world: &World,
    budget: &iroha_allocation::AllocationBudget,
) -> Result<(), StateRestoreError> {
    fn validate(cut: &impl WorldReadOnly, field: &str) -> Result<(), StateRestoreError> {
        let Some(authority) = cut.runtime_child_admin_state() else {
            use iroha_data_model::{
                block::consensus::SumeragiRootScope,
                runtime::child_admin::foundation::ChildRuntimeFoundationV1,
            };
            // Selecting empty addition storage grants no authority. A signed child recipe,
            // however, cannot survive the disappearance of its founded administrator cell.
            // An ordinary unselected marker on a global root remains inert.
            if cut
                .parameters()
                .custom()
                .contains_key(ChildRuntimeFoundationV1::parameter_id())
                && matches!(
                    crate::sumeragi::lanes::routing::read_routing_root_scope(cut)
                        .map_err(StateRestoreError::ExecutionDeferred)?,
                    Some(SumeragiRootScope::Dataspace { .. })
                )
            {
                return Err(json::Error::InvalidField {
                    field: field.to_owned(),
                    message: "signed child foundation has no administrator state".into(),
                }
                .into());
            }
            return Ok(());
        };
        crate::runtime::lifecycle_activation::validate_child_foundation(
            cut,
            authority.root.network,
        )
        .map_err(|error| {
            error.map_rejection(|message| json::Error::InvalidField {
                field: field.to_owned(),
                message,
            })
        })?;
        Ok(())
    }
    validate(&world.view(), "world.runtime_child_admin_state.blocks")?;
    let previous = world.try_block_and_revert(budget)?;
    validate(&previous, "world.runtime_child_admin_state.revert")
}

pub(super) fn validate_identity(state: &State) -> Result<(), MergeLedgerCommitError> {
    fn validate(
        cut: &impl WorldReadOnly,
        state: &State,
        field: &str,
    ) -> Result<(), MergeLedgerCommitError> {
        let Some(authority) = cut.runtime_child_admin_state() else {
            return Ok(());
        };
        let invalid = |reason| {
            MergeLedgerCommitError::ExecutionStatePublication(format!("restored {field}: {reason}"))
        };
        let scope = crate::sumeragi::lanes::routing::read_routing_root_scope(cut)
            .map_err(MergeLedgerCommitError::ExecutionDeferred)?
            .ok_or_else(|| invalid("missing immutable signed root scope"))?;
        if authority.root.network != state.network_id || authority.root.scope() != scope {
            return Err(invalid(
                "administrator identity differs from the restored root",
            ));
        }
        let instance = scope
            .instance_id(
                &crate::sumeragi::crypto::BlsCrypto::new(),
                state.network_id,
                state.chain_id_ref().as_str(),
            )
            .map_err(|_| invalid("cannot derive the restored child instance"))?;
        if authority.root.instance != instance.0 {
            return Err(invalid(
                "administrator instance differs from the restored root",
            ));
        }
        Ok(())
    }
    validate(
        &state.world.view(),
        state,
        "child administrator current state",
    )?;
    let previous = state
        .world
        .try_block_and_revert(&state.ivm_execution_budget())
        .map_err(StateStorageAdmissionError::World)?;
    validate(&previous, state, "child administrator predecessor state")
}

#[cfg(test)]
#[path = "child_runtime_snapshot_tests.rs"]
mod tests;
