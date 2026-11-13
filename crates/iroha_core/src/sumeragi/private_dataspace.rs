//! Parent admission and certified anchoring of independent owner-private execution roots.
//!
//! The parent stores no private genesis, body or artifact. Registration binds the active paid SNS
//! lease and its ownership generation to an exact child authority. Compact anchors extend that
//! authority's contiguous cursor; transfers, expiry and suspension cannot silently replace it.

use iroha_data_model::{
    account::AccountId,
    block::consensus::SumeragiRootScope,
    isi::{
        error::InstructionExecutionError as Error,
        private_dataspace::{AnchorPrivateDataspace, RegisterPrivateDataspace},
    },
    private_dataspace::{
        PrivateDataspaceAdmissionPolicy, PrivateDataspaceAnchor, PrivateDataspaceAnchorError,
        PrivateDataspaceRegistration,
    },
};
use iroha_model_base::topology::DataSpaceId;
use mv::storage::StorageReadOnly;

use crate::{
    smartcontracts::Execute,
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};

fn invalid(error: impl std::fmt::Display) -> Error {
    Error::InvariantViolation(format!("private dataspace: {error}").into())
}

// The model has already captured whether a decoder refusal belongs to this local
// attempt. Preserve it before crossing the model-owned instruction error interface.
fn profile_error(
    error: PrivateDataspaceAnchorError,
    state: &mut StateTransaction<'_, '_>,
) -> Error {
    let attempt = match error {
        PrivateDataspaceAnchorError::Invalid(message) => {
            crate::execution_attempt::ExecutionAttemptError::Rejected(invalid(message))
        }
        PrivateDataspaceAnchorError::Resource(resource) => {
            crate::execution_attempt::canonical_decode_attempt_error(resource, invalid)
        }
    };
    state.attempt_error_to_instruction_error(attempt)
}

fn require_parent(state: &StateTransaction<'_, '_>, authority: &AccountId) -> Result<(), Error> {
    if crate::sumeragi::lanes::routing::committed_root_scope(&state.world)
        != Some(SumeragiRootScope::Global)
    {
        return Err(invalid(
            "registration and anchoring require a committed global root",
        ));
    }
    if state.world.accounts().get(authority).is_none() {
        return Err(invalid("parent transaction authority is not registered"));
    }
    Ok(())
}

fn active_owner(
    state: &StateTransaction<'_, '_>,
    alias: &str,
    dataspace: DataSpaceId,
) -> Result<(AccountId, u64), crate::execution_attempt::ExecutionAttemptError<Error>> {
    let selector = crate::sns::selector_for_dataspace_alias(alias).map_err(invalid)?;
    if selector.normalized_label() != alias {
        return Err(invalid("dataspace alias must be canonical").into());
    }
    let now = state.block_unix_timestamp_ms();
    let resolved = crate::sns::resolve_active_dataspace_id_by_alias(
        &state.world,
        &state.nexus.dataspace_catalog,
        alias,
        now,
    )
    .map_err(|error| error.into_attempt_error(invalid))?;
    if resolved != dataspace
        || dataspace == DataSpaceId::UNIVERSAL
        || DataSpaceId::from_hash(&selector.name_hash()) != dataspace
    {
        return Err(invalid("active alias differs from registered private dataspace").into());
    }
    Ok(
        crate::sns::active_dataspace_owner_and_generation_by_alias(&state.world, alias, now)
            .map_err(|error| error.into_attempt_error(invalid))?
            .ok_or_else(|| invalid("dataspace alias has no active owner"))?,
    )
}

/// Reject physical parent execution policies that would claim an already external private root
/// or a dataspace retired from parent execution.
pub(crate) fn ensure_parent_execution_separate(
    world: &impl WorldReadOnly,
    dataspaces: impl IntoIterator<Item = DataSpaceId>,
) -> Result<(), String> {
    let dataspaces: Vec<_> = dataspaces.into_iter().collect();
    if dataspaces
        .iter()
        .any(|id| world.private_dataspaces().get(*id).is_some())
    {
        return Err("physical parent execution cannot claim a registered private root".into());
    }
    crate::state::dataspace_retirement::ensure_not_retired(world, dataspaces)
}

impl Execute for RegisterPrivateDataspace {
    fn execute(
        self,
        authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_parent(state, authority)?;
        let registration = PrivateDataspaceRegistration::decode(&self.registration)
            .map_err(|error| profile_error(error, state))?;
        let SumeragiRootScope::Dataspace {
            parent_network_id,
            dataspace_id,
        } = registration.scope
        else {
            return Err(invalid("registered child scope is not private"));
        };
        if &parent_network_id != state.network_id() {
            return Err(invalid("child is bound to another parent network"));
        }
        // Existing locally executed public dataspaces cannot be reinterpreted as external roots.
        // An owner-retired catalog identity keeps its descriptor but no parent execution; only
        // its retiring SNS owner and generation may claim it, checked after ownership below.
        let retirement =
            crate::state::dataspace_retirement::retired_dataspace(&state.world, dataspace_id)
                .map_err(|error| {
                    state.attempt_error_to_instruction_error(error.map_rejection(invalid))
                })?;
        if (retirement.is_none() && state.nexus.dataspace_catalog.by_id(dataspace_id).is_some())
            || state
                .world
                .sumeragi_lanes()
                .lanes
                .iter()
                .any(|lane| lane.dataspace == dataspace_id)
        {
            return Err(invalid(
                "private child collides with a locally configured dataspace",
            ));
        }
        if retirement.is_none()
            && crate::state::runtime_catalog_from_world(&state.world)
                .map_err(invalid)?
                .is_some_and(|catalog| {
                    catalog
                        .dataspaces
                        .iter()
                        .any(|entry| entry.descriptor.id == dataspace_id)
                })
        {
            return Err(invalid(
                "private child collides with a staged physical dataspace",
            ));
        }
        if let Some(parameter) = state
            .world
            .parameters()
            .custom()
            .get(&iroha_data_model::sumeragi_lanes::SumeragiLanePolicy::parameter_id())
        {
            let policy =
                iroha_data_model::sumeragi_lanes::SumeragiLanePolicy::from_custom_parameter(
                    parameter,
                )
                .ok_or_else(|| invalid("wrong native lane policy parameter"))?
                .map_err(invalid)?;
            if policy
                .fixed
                .iter()
                .any(|lane| lane.dataspace == dataspace_id)
                || policy
                    .autoscale
                    .is_some_and(|autoscale| autoscale.dataspace == dataspace_id)
            {
                return Err(invalid(
                    "private child collides with reserved physical lane execution",
                ));
            }
        }
        let (owner, generation) =
            active_owner(state, &self.alias, dataspace_id).map_err(|error| match error {
                crate::execution_attempt::ExecutionAttemptError::Rejected(error) => error,
                crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
                    crate::sns::SnsError::Deferred(reason).retain_in_instruction(state)
                }
            })?;
        if &owner != authority || generation != self.expected_ownership_generation {
            return Err(invalid(
                "authority or expected SNS ownership generation does not match",
            ));
        }
        if let Some(retirement) = retirement
            && (retirement.alias != self.alias
                || retirement.owner != owner
                || retirement.ownership_generation != generation
                || retirement.retirement_height >= state.block_height())
        {
            return Err(invalid(
                "a retired dataspace is re-admitted only by its retiring SNS owner and generation in a later block",
            ));
        }
        let policy = state
            .world
            .parameters()
            .custom()
            .get(&PrivateDataspaceAdmissionPolicy::parameter_id())
            .map(PrivateDataspaceAdmissionPolicy::from_custom_parameter)
            .transpose()
            .map_err(|error| profile_error(error, state))?
            .unwrap_or_default();
        state
            .world
            .private_dataspaces
            .get_mut()
            .register_authorized(policy, self.alias, owner, generation, registration)
            .map_err(|error| profile_error(error, state))?;
        let record = state
            .world
            .private_dataspaces()
            .get(dataspace_id)
            .expect("just registered");
        crate::exec_witness::record_write_private_dataspace(record).map_err(invalid)
    }
}

impl Execute for AnchorPrivateDataspace {
    fn execute(
        self,
        authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_parent(state, authority)?;
        let anchor = PrivateDataspaceAnchor::decode(&self.anchor)
            .map_err(|error| profile_error(error, state))?;
        let record = state
            .world
            .private_dataspaces()
            .get(self.dataspace_id)
            .ok_or_else(|| invalid("private root is not registered"))?;
        let SumeragiRootScope::Dataspace {
            parent_network_id, ..
        } = record.anchor.registration().scope
        else {
            return Err(invalid("registered child scope is not private"));
        };
        if &parent_network_id != state.network_id() {
            return Err(invalid(
                "registered child belongs to another parent network",
            ));
        }
        let (owner, generation) =
            active_owner(state, &record.alias, self.dataspace_id).map_err(|error| match error {
                crate::execution_attempt::ExecutionAttemptError::Rejected(error) => error,
                crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
                    crate::sns::SnsError::Deferred(reason).retain_in_instruction(state)
                }
            })?;
        state
            .world
            .private_dataspaces
            .get_mut()
            .apply_authorized(self.dataspace_id, &owner, generation, &anchor)
            .map_err(|error| profile_error(error, state))?;
        let record = state
            .world
            .private_dataspaces()
            .get(self.dataspace_id)
            .expect("registered root retained");
        crate::exec_witness::record_write_private_dataspace(record).map_err(invalid)
    }
}

#[cfg(test)]
mod tests;
