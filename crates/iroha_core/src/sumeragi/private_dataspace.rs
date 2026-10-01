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
        PrivateDataspaceAdmissionPolicy, PrivateDataspaceAnchor, PrivateDataspaceRegistration,
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
) -> Result<(AccountId, u64), Error> {
    let selector = crate::sns::selector_for_dataspace_alias(alias).map_err(invalid)?;
    if selector.normalized_label() != alias {
        return Err(invalid("dataspace alias must be canonical"));
    }
    // A private child is identified by the paid name's native selector. A
    // preserved physical catalog binding or historical SNS metadata never
    // selects that independent root's identity.
    if dataspace == DataSpaceId::UNIVERSAL
        || DataSpaceId::from_hash(&selector.name_hash()) != dataspace
    {
        return Err(invalid(
            "active alias differs from registered private dataspace",
        ));
    }
    let now = state.block_unix_timestamp_ms();
    crate::sns::active_dataspace_owner_and_generation_by_alias(&state.world, alias, now)
        .map_err(invalid)?
        .ok_or_else(|| invalid("dataspace alias has no active owner"))
}

/// Reject physical parent execution policies that would claim an already external private root.
pub(crate) fn ensure_parent_execution_separate(
    world: &impl WorldReadOnly,
    dataspaces: impl IntoIterator<Item = DataSpaceId>,
) -> Result<(), String> {
    let runtime =
        crate::state::runtime_catalog_from_world(world).map_err(|error| error.to_string())?;
    if dataspaces.into_iter().any(|id| {
        world.private_dataspaces().get(id).is_some()
            || runtime.as_ref().is_some_and(|catalog| {
                catalog
                    .retired_dataspaces
                    .iter()
                    .any(|entry| entry.retirement.dataspace_id == id)
            })
    }) {
        return Err("physical parent execution cannot claim a registered private root".into());
    }
    Ok(())
}

/// Retired physical lane identities remain reserved by their exact historical ledgers.
pub(crate) fn ensure_native_policy_preserves_retired_storage(
    world: &impl WorldReadOnly,
    policy: &iroha_data_model::sumeragi_lanes::SumeragiLanePolicy,
) -> Result<(), String> {
    let Some(runtime) =
        crate::state::runtime_catalog_from_world(world).map_err(|error| error.to_string())?
    else {
        return Ok(());
    };
    if runtime.retired_lanes.iter().any(|entry| {
        policy.fixed_lane(entry.lane.id).is_some()
            || policy
                .routes
                .iter()
                .any(|route| route.lane == entry.lane.id)
            || policy.is_elastic(entry.lane.id)
    }) {
        return Err("native fixed, routing or autoscale policy cannot reuse a retired physical storage identity".into());
    }
    Ok(())
}

impl Execute for RegisterPrivateDataspace {
    fn execute(
        self,
        authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        require_parent(state, authority)?;
        let registration =
            PrivateDataspaceRegistration::decode(&self.registration).map_err(invalid)?;
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
        if state.nexus.dataspace_catalog.by_id(dataspace_id).is_some()
            || state
                .nexus
                .dataspace_catalog
                .by_alias(&self.alias)
                .is_some()
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
        if crate::state::runtime_catalog_from_world(&state.world)
            .map_err(invalid)?
            .is_some_and(|catalog| {
                let retirement = catalog.retired_dataspaces.iter().find(|record| {
                    record.retirement.dataspace_id == dataspace_id
                        || record.retirement.alias == self.alias
                });
                retirement.is_some_and(|record| record.retirement_height >= state.block_height())
                    || (retirement.is_none()
                        && catalog
                            .dataspaces
                            .iter()
                            .any(|entry| entry.descriptor.id == dataspace_id))
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
        let (owner, generation) = active_owner(state, &self.alias, dataspace_id)?;
        if &owner != authority || generation != self.expected_ownership_generation {
            return Err(invalid(
                "authority or expected SNS ownership generation does not match",
            ));
        }
        let policy = state
            .world
            .parameters()
            .custom()
            .get(&PrivateDataspaceAdmissionPolicy::parameter_id())
            .map(PrivateDataspaceAdmissionPolicy::from_custom_parameter)
            .transpose()
            .map_err(invalid)?
            .unwrap_or_default();
        state
            .world
            .private_dataspaces
            .get_mut()
            .register_authorized(policy, self.alias, owner, generation, registration)
            .map_err(invalid)?;
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
        let anchor = PrivateDataspaceAnchor::decode(&self.anchor).map_err(invalid)?;
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
        let (owner, generation) = active_owner(state, &record.alias, self.dataspace_id)?;
        state
            .world
            .private_dataspaces
            .get_mut()
            .apply_authorized(self.dataspace_id, &owner, generation, &anchor)
            .map_err(invalid)?;
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
