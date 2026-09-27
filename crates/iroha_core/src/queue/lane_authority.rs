//! Authority projection for QueuePlan admission.
//!
//! Every transaction executes in the global Sumeragi block; lanes and dataspaces only label
//! routes. The authority an admission context binds for an active route is therefore the global
//! committee that the lag-2 schedule names for the proposal height: the same roster for every
//! route, in the core's canonical order, in both consensus modes, and the same authority Torii
//! routes by ([`crate::state::State::resolve_route_authority_at_height`]). It never depends on
//! public-lane staking, which only NPoS chains have.

use super::*;

/// Resolve one QueuePlan route's exact proposal-height committee: the global committee
/// scheduled for `proposal_height`.
///
/// # Errors
/// The route is unknown or inactive at `proposal_height`, a live validator key is not
/// BLS-normal, or no validator is live there (see [`crate::state::LaneAuthorityError`]).
pub(crate) fn queue_plan_authoritative_peers_in_view_at_height(
    state_view: &impl StateReadOnly,
    route: RoutingDecision,
    proposal_height: u64,
) -> Result<Vec<PeerId>, crate::state::LaneAuthorityError> {
    crate::state::resolve_global_route(
        state_view.world(),
        crate::state::LaneAuthorityRoute::new(route.lane_id, route.dataspace_id),
        state_view.nexus(),
        proposal_height,
    )
    .map(crate::state::LaneAuthorityCommittee::into_validators)
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;
    use crate::{kura::Kura, query::store::LiveQueryStore, state::LaneAuthorityError};

    fn state_with_validators(seeds: &[u8]) -> State {
        let mut state = State::new(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let keys = seeds
            .iter()
            .map(|seed| KeyPair::from_seed(vec![*seed; 32], Algorithm::BlsNormal))
            .collect::<Vec<_>>();
        {
            let mut world = state.world.block();
            let mut peers = world.peers_mut_for_testing().transaction();
            for key in &keys {
                peers.push(PeerId::new(key.public_key().clone()));
            }
            peers.apply();
            world.commit();
        }
        for key in &keys {
            let pop = iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("PoP");
            state
                .world
                .register_validator_pop_for_testing(key.public_key().clone(), pop);
        }
        state
    }

    #[test]
    fn every_route_binds_the_scheduled_global_committee() {
        let state = state_with_validators(&[0xC1, 0xC2, 0xC3, 0xC4]);
        let view = state.view();
        let expected = crate::sumeragi::schedule::scheduled_committee(view.world(), 5)
            .expect("scheduled committee");
        assert_eq!(expected.len(), 4);
        assert_eq!(
            queue_plan_authoritative_peers_in_view_at_height(
                &view,
                RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                5
            ),
            Ok(expected),
            "a lane is a routing label; the authority is the global committee",
        );
        assert!(matches!(
            queue_plan_authoritative_peers_in_view_at_height(
                &view,
                RoutingDecision::new(LaneId::new(7), DataSpaceId::new(9)),
                5
            ),
            Err(LaneAuthorityError::UnknownDataspace { .. })
        ));
    }

    #[test]
    fn a_height_without_live_validators_has_no_authority() {
        let state = state_with_validators(&[]);
        let view = state.view();
        let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
        assert!(matches!(
            queue_plan_authoritative_peers_in_view_at_height(&view, route, 1),
            Err(LaneAuthorityError::UndersizedPool { actual: 0, .. })
        ));
    }
}
