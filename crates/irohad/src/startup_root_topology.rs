//! Bind startup execution geometry to the independently authenticated original genesis scope.

use iroha_config::parameters::actual::{LaneConfig, Nexus};
use iroha_data_model::{
    block::consensus::{ConsensusMode, SumeragiRootScope},
    nexus::{
        AUTOSCALE_META_COMMITTEE, AUTOSCALE_META_CREATED_HEIGHT, AUTOSCALE_META_DRAIN_STATE,
        AUTOSCALE_META_MANAGED, LaneStorageProfile, LaneVisibility,
    },
};
use iroha_model_base::topology::LaneId;

/// Check geometry only after the caller has authenticated the original signed genesis context.
pub(super) fn validate(
    nexus: &Nexus,
    mode: ConsensusMode,
    scope: SumeragiRootScope,
) -> Result<(), &'static str> {
    scope.validate().map_err(|_| "invalid signed root scope")?;
    let SumeragiRootScope::Dataspace { dataspace_id, .. } = scope else {
        return if nexus.uses_multilane_catalogs() && mode != ConsensusMode::Npos {
            Err("custom Nexus lane topology requires the authenticated consensus mode to be NPoS")
        } else {
            Ok(())
        };
    };
    if mode != ConsensusMode::Permissioned {
        return Err("private root requires the authenticated consensus mode to be Permissioned");
    }
    if nexus.lane_catalog != nexus.configured_lane_catalog
        || nexus.dataspace_catalog != nexus.configured_dataspace_catalog
        || nexus.lane_config != LaneConfig::from_catalog(&nexus.lane_catalog)
    {
        return Err("private root requires unchanged configured execution geometry");
    }
    if !matches!(nexus.dataspace_catalog.entries(), [entry] if entry.id == dataspace_id) {
        return Err("private root requires exactly its signed dataspace in the physical catalog");
    }
    let [lane] = nexus.lane_catalog.lanes() else {
        return Err("private root requires exactly one physical lane zero");
    };
    if nexus.lane_catalog.lane_count().get() != 1
        || lane.id != LaneId::SINGLE
        || lane.dataspace_id != dataspace_id
        || lane.visibility != LaneVisibility::Restricted
        || lane.storage != LaneStorageProfile::FullReplica
    {
        return Err(
            "private root requires its own restricted, fully replicated physical lane zero",
        );
    }
    let routing = &nexus.routing_policy;
    if routing.default_lane != LaneId::SINGLE
        || routing.default_dataspace != dataspace_id
        || routing.rules.iter().any(|rule| {
            rule.lane != LaneId::SINGLE
                || rule.dataspace.is_some_and(|target| target != dataspace_id)
        })
    {
        return Err("private root routing must remain inside its signed dataspace and lane zero");
    }
    if nexus.autoscale.enabled
        || nexus.autoscale.last_transition_height != 0
        || [
            AUTOSCALE_META_MANAGED,
            AUTOSCALE_META_CREATED_HEIGHT,
            AUTOSCALE_META_DRAIN_STATE,
            AUTOSCALE_META_COMMITTEE,
        ]
        .iter()
        .any(|key| lane.metadata.contains_key(*key))
    {
        return Err("private root cannot contain autoscale or elastic-lane state");
    }
    Ok(())
}

#[cfg(test)]
mod tests;
