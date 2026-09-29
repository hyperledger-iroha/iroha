//! Authority projection for QueuePlan admission.
//!
//! Every transaction executes in the global Sumeragi block; lanes and dataspaces only label
//! routes. Admission binds the proposal height's retained [`crate::sumeragi::schedule::ConsensusSchedule`]
//! entry, in canonical core order, for every route and both consensus modes. Mutable peer
//! registrations and public-lane staking cannot replace that committed voting authority.

use super::*;

/// Resolve one QueuePlan route's exact retained proposal-height global committee.
///
/// # Errors
/// An unknown or inactive route is rejected. Otherwise,
/// [`crate::state::LaneAuthorityError::InvalidAuthoritySource`] when the schedule window is
/// malformed, excludes `proposal_height`, or the selected entry has invalid geometry, ordering,
/// proofs of possession or chain parameters. An empty schedule has no authority and returns
/// [`crate::state::LaneAuthorityError::UndersizedPool`].
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
    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{LaneAuthorityError, derive_validator_key_id},
        sumeragi::schedule::{
            ChainParamsRecord, ConsensusSchedule, RetainedConsensusSchedule, ScheduledConfig,
            ScheduledSlot,
        },
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::parameter::system::SumeragiConsensusMode;
    const PROPOSAL_HEIGHT: u64 = 5;

    // This fixture tests retained authority admission/query projection. It does not claim
    // that the local World rows are a certified State/history snapshot.
    fn state_with_validators(retain: bool) -> State {
        let mut state = State::new(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        if !retain {
            return state;
        }
        let signed = crate::sumeragi::epoch::tests::genesis_fixture(
            SumeragiConsensusMode::Permissioned,
            10,
            false,
        );
        let epoch = crate::sumeragi::epoch::genesis_epoch(&signed).unwrap();
        for member in &epoch.committee {
            state.world.register_validator_pop_for_testing(
                member.validator.public_key().clone(),
                member.proof_of_possession.clone(),
            );
        }
        let params = ChainParamsRecord::from_parameters(state.world.view().parameters().sumeragi());
        let graph = ConsensusSchedule::from_owned_entries(
            (5..=7)
                .map(|height| {
                    ScheduledSlot::Ready(ScheduledConfig {
                        height,
                        epoch: epoch.clone(),
                        params,
                    })
                })
                .collect(),
        )
        .unwrap();
        let retained =
            RetainedConsensusSchedule::admit(&graph, &state.ivm_execution_budget()).unwrap();
        {
            let mut world = state.world.block();
            let mut peers = world.peers_mut_for_testing().transaction();
            for member in epoch.committee {
                peers.push(member.validator);
            }
            peers.apply();
            *world.consensus_schedule.get_mut() = retained;
            world.commit();
        }
        state
    }
    fn expected(state: &State) -> Vec<PeerId> {
        state
            .world
            .view()
            .consensus_schedule()
            .ready(PROPOSAL_HEIGHT)
            .unwrap()
            .epoch
            .committee
            .iter()
            .map(|member| member.validator.clone())
            .collect()
    }

    #[test]
    fn every_route_binds_the_scheduled_global_committee() {
        let state = state_with_validators(true);
        let peers = expected(&state);
        for height in 5..=7 {
            for route in [RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)] {
                assert_eq!(
                    queue_plan_authoritative_peers_in_view_at_height(&state.view(), route, height),
                    Ok(peers.clone())
                );
            }
        }
    }
    #[test]
    fn an_unknown_route_cannot_borrow_global_authority() {
        let state = state_with_validators(true);
        assert!(matches!(
            queue_plan_authoritative_peers_in_view_at_height(
                &state.view(),
                RoutingDecision::new(LaneId::new(7), DataSpaceId::new(9)),
                5
            ),
            Err(LaneAuthorityError::UnknownDataspace { .. })
        ));
    }
    #[test]
    fn a_height_without_live_validators_has_no_authority() {
        let state = state_with_validators(false);
        assert!(matches!(
            queue_plan_authoritative_peers_in_view_at_height(
                &state.view(),
                RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                1
            ),
            Err(LaneAuthorityError::UndersizedPool { actual: 0, .. })
        ));
    }
    #[test]
    fn a_height_outside_the_retained_window_has_no_authority() {
        let state = state_with_validators(true);
        for height in [4, 8] {
            assert!(
                matches!(queue_plan_authoritative_peers_in_view_at_height(&state.view(),RoutingDecision::new(LaneId::SINGLE,DataSpaceId::UNIVERSAL),height),Err(LaneAuthorityError::InvalidAuthoritySource {authority_height,..}) if authority_height==height)
            );
        }
    }
    #[test]
    fn a_pending_boundary_cannot_supply_guessed_incumbent_authority() {
        let state = state_with_validators(true);
        let signed =
            crate::sumeragi::epoch::tests::genesis_fixture(SumeragiConsensusMode::Npos, 10, false);
        let epoch = crate::sumeragi::epoch::genesis_epoch(&signed).unwrap();
        let params = ChainParamsRecord::from_parameters(state.world.view().parameters().sumeragi());
        let graph = ConsensusSchedule::from_owned_entries(vec![
            ScheduledSlot::Ready(ScheduledConfig {
                height: 9,
                epoch: epoch.clone(),
                params,
            }),
            ScheduledSlot::Ready(ScheduledConfig {
                height: 10,
                epoch: epoch.clone(),
                params,
            }),
            ScheduledSlot::PendingBoundary {
                height: 11,
                boundary_height: 10,
                predecessor_context_id: epoch.context_id().unwrap(),
                params,
            },
        ])
        .unwrap();
        let retained =
            RetainedConsensusSchedule::admit(&graph, &state.ivm_execution_budget()).unwrap();
        {
            let mut world = state.world.block();
            *world.consensus_schedule.get_mut() = retained;
            world.commit();
        }
        assert!(matches!(
            queue_plan_authoritative_peers_in_view_at_height(
                &state.view(),
                RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                11
            ),
            Err(LaneAuthorityError::InvalidAuthoritySource {
                authority_height: 11,
                ..
            })
        ));
    }
    #[test]
    fn populated_live_validators_cannot_supply_an_empty_schedule() {
        let state = state_with_validators(true);
        {
            let mut world = state.world.block();
            *world.consensus_schedule.get_mut() = RetainedConsensusSchedule::default();
            world.commit();
        }
        assert_eq!(state.world.view().peers().len(), 4);
        assert!(matches!(
            queue_plan_authoritative_peers_in_view_at_height(
                &state.view(),
                RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                5
            ),
            Err(LaneAuthorityError::UndersizedPool { actual: 0, .. })
        ));
    }
    #[test]
    fn malformed_authority_never_enters_the_retained_owner() {
        let state = state_with_validators(true);
        let original = state
            .world
            .view()
            .consensus_schedule()
            .ready(5)
            .unwrap()
            .clone();
        for mutation in 0..7 {
            let mut config = original.clone();
            config.height = 6;
            match mutation {
                0 => config.epoch.committee[0].proof_of_possession[0] ^= 1,
                1 => {
                    config.epoch.committee[0].proof_of_possession.pop();
                }
                2 => config.epoch.committee.swap(0, 1),
                3 => config.params.payload_retry_interval_ms = 0,
                4 => {
                    config.epoch.committee.pop();
                }
                5 => config.height = u64::MAX,
                _ => config.epoch.leader_seed[0] ^= 1,
            }
            let graph = ConsensusSchedule::from_owned_entries(vec![
                ScheduledSlot::Ready(original.clone()),
                ScheduledSlot::Ready(config),
                ScheduledSlot::Ready(ScheduledConfig {
                    height: 7,
                    ..original.clone()
                }),
            ]);
            assert!(graph.is_err(), "mutation {mutation}");
        }
        assert_eq!(
            queue_plan_authoritative_peers_in_view_at_height(
                &state.view(),
                RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                5
            ),
            Ok(expected(&state))
        );
    }
    #[test]
    fn changed_live_registrations_cannot_replace_the_retained_roster() {
        let mut state = state_with_validators(true);
        let peers = expected(&state);
        let replacement = KeyPair::from_seed(vec![0xD1; 32], Algorithm::BlsNormal);
        {
            let mut world = state.world.block();
            world.peers_mut_for_testing().get_mut().clear();
            world
                .peers_mut_for_testing()
                .get_mut()
                .push(PeerId::new(replacement.public_key().clone()));
            for peer in &peers {
                world
                    .consensus_keys
                    .remove(derive_validator_key_id(peer.public_key()));
            }
            world.commit();
        }
        state.world.register_validator_pop_for_testing(
            replacement.public_key().clone(),
            iroha_crypto::bls_normal_pop_prove(replacement.private_key()).unwrap(),
        );
        assert_eq!(
            queue_plan_authoritative_peers_in_view_at_height(
                &state.view(),
                RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
                5
            ),
            Ok(peers)
        );
    }
}
