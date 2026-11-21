//! Read-only scope binding shared by operator runtime drafts.
//!
//! Drafts grant no authority and never initialize a root. A child draft requires the protected
//! original foundation, exact selected administrator service and committed native root metadata.
//! Actual signed execution additionally authenticates the complete instance and source frame.

use iroha_data_model::{
    NetworkId,
    block::consensus::SumeragiRootScope,
    runtime::lifecycle::{ChangeSetV1, RootScopeV1},
};

use crate::{execution_attempt::ExecutionAttemptError, state::WorldReadOnly};

fn root(
    world: &impl WorldReadOnly,
    network: NetworkId,
) -> Result<RootScopeV1, ExecutionAttemptError<String>> {
    let invalid = |reason: &str| ExecutionAttemptError::Rejected(reason.to_owned());
    let scope = crate::sumeragi::lanes::routing::read_routing_root_scope(world)
        .map_err(ExecutionAttemptError::Deferred)?;
    match scope {
        Some(SumeragiRootScope::Dataspace { dataspace_id, .. }) => {
            super::lifecycle_activation::validate_child_foundation(world, network)?;
            if !super::child_admin::enabled(world.runtime_selection())
                .map_err(ExecutionAttemptError::Rejected)?
            {
                return Err(invalid("child administrator service is not selected"));
            }
            Ok(RootScopeV1::DataSpace(dataspace_id))
        }
        None | Some(SumeragiRootScope::Global) if world.runtime_child_admin_state().is_none() => {
            Ok(RootScopeV1::Universal)
        }
        _ => Err(invalid(
            "child administrator state has no authenticated child root",
        )),
    }
}

/// Bind a selection-only draft to its committed root before computing its public identity.
/// The existing operator APIs expose catalog diagnostics, so original local refusals become
/// diagnostics only at that boundary; this helper retains them until the caller returns.
pub(super) fn bind(
    world: &impl WorldReadOnly,
    network: NetworkId,
    mut change: ChangeSetV1,
) -> Result<ChangeSetV1, ExecutionAttemptError<String>> {
    if change.network != network {
        return Err(ExecutionAttemptError::Rejected(
            "runtime draft belongs to another network".into(),
        ));
    }
    change.root = root(world, network)?;
    Ok(change)
}
