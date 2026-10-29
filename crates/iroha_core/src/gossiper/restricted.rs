//! Exact signed-root or native-lane ownership of restricted transaction dissemination.

use std::{collections::BTreeMap, num::NonZeroUsize};

use super::{DROP_REASON_NO_RESTRICTED_TARGETS, GossipRoute, TransactionGossiper};
use crate::{
    queue::GossipBatchEntry,
    state::{WorldReadOnly, live_consensus_key_pop_for_peer_on_lane},
    sumeragi::{
        lanes::{lane_height_config, routing::committed_root_scope},
        schedule::scheduled_committee,
    },
};
use iroha_data_model::block::consensus::SumeragiRootScope;
use iroha_model_base::{
    peer::PeerId,
    topology::{DataSpaceId, LaneId},
};

/// Never put bodies owned by different lane committees in the same message.
pub(super) fn group_by_route(
    entries: Vec<GossipBatchEntry>,
) -> BTreeMap<(DataSpaceId, LaneId), Vec<GossipBatchEntry>> {
    let mut grouped = BTreeMap::<_, Vec<_>>::new();
    for entry in entries {
        grouped
            .entry((entry.routing.dataspace_id, entry.routing.lane_id))
            .or_default()
            .push(entry);
    }
    grouped
}

/// Read the exact owner from one committed view. An independent private root owns
/// lane zero through its authenticated next-height schedule; global-root lanes
/// retain their current incarnation and live pinned keys. Physical catalogs and
/// connected peers never supply dissemination authority.
pub(super) fn committee(
    world: &impl WorldReadOnly,
    applied_height: u64,
    route: GossipRoute,
) -> Result<Vec<PeerId>, &'static str> {
    match committed_root_scope(world).ok_or("missing_root_scope")? {
        SumeragiRootScope::Dataspace { dataspace_id, .. } => {
            if route.lane_id != LaneId::SINGLE || route.dataspace_id != dataspace_id {
                return Err("private_root_route_mismatch");
            }
            let height = applied_height
                .checked_add(1)
                .ok_or("private_root_height_overflow")?;
            return scheduled_committee(world, height)
                .map_err(|_| "unavailable_private_root_committee");
        }
        SumeragiRootScope::Global => {}
    }
    let record = world
        .sumeragi_lanes()
        .lane(route.lane_id)
        .ok_or("missing_native_lane")?;
    if record.dataspace != route.dataspace_id || route.dataspace_id == DataSpaceId::UNIVERSAL {
        return Err("native_lane_dataspace_mismatch");
    }
    if !record.admits_anchor(applied_height) {
        return Err("inactive_native_lane");
    }
    lane_height_config(record).map_err(|_| "invalid_native_lane_committee")?;
    Ok(record
        .committee
        .iter()
        .filter(|member| {
            live_consensus_key_pop_for_peer_on_lane(
                world,
                &member.peer,
                applied_height,
                record.lane,
            )
            .is_some_and(|pop| pop == member.pop)
        })
        .map(|member| member.peer.clone())
        .collect())
}

/// Connectivity and fanout caps may narrow authority, never broaden it.
pub(super) fn targets(
    world: &impl WorldReadOnly,
    applied_height: u64,
    route: GossipRoute,
    online: &std::collections::BTreeSet<PeerId>,
    cap: Option<NonZeroUsize>,
    seed: u64,
) -> Result<Vec<PeerId>, &'static str> {
    let members = committee(world, applied_height, route)?;
    let eligible = members
        .into_iter()
        .filter(|peer| online.contains(peer))
        .collect();
    let (targets, _) = TransactionGossiper::select_targets_with_seed(eligible, cap, seed);
    if targets.is_empty() {
        Err(DROP_REASON_NO_RESTRICTED_TARGETS)
    } else {
        Ok(targets)
    }
}

#[cfg(test)]
pub(super) mod tests;
