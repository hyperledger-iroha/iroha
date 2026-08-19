//! Exact lane-route authority projection for QueuePlan admission.

use super::*;

/// Resolve one QueuePlan route's exact proposal-height committee.
pub(crate) fn queue_plan_authoritative_peers_in_view_at_height(
    state_view: &impl StateReadOnly,
    route: RoutingDecision,
    proposal_height: u64,
) -> Result<Vec<PeerId>, crate::state::LaneAuthorityError> {
    state_view
        .resolve_lane_committee_at_height(
            crate::state::LaneAuthorityRoute::new(route.lane_id, route.dataspace_id),
            proposal_height,
        )
        .map(crate::state::LaneAuthorityCommittee::into_validators)
}
