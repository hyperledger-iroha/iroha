#[test]
fn queue_plan_pending_resolution_decrements_only_exact_bound_route_counts() {
    let (state, validator_keypairs, _, _) = configured_two_lane_merge_state();
    let participant_lane = LaneId::new(1);
    let native_plan = crate::queue::RoutingPlan::native_amx(
        crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        vec![crate::queue::RouteLeg::new(
            crate::queue::RoutingDecision::new(participant_lane, DataSpaceId::UNIVERSAL),
            crate::queue::RouteLegRole::Participant,
        )],
    );
    let (native_binding, native_certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        native_plan,
        &validator_keypairs,
        1,
        0x6C,
    );
    let single_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let (single_binding, single_certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        single_plan,
        &validator_keypairs,
        1,
        0x6D,
    );
    let native_obligation = queue_plan_pending_obligation_for_test(&state, &native_certificate);
    let single_obligation = queue_plan_pending_obligation_for_test(&state, &single_certificate);
    let coordinator_route = single_obligation.routes[0];
    let participant_route = *native_obligation
        .routes
        .iter()
        .find(|route| route.lane_id == participant_lane)
        .expect("fixture participant route");
    let native_coordinator_member_key = State::queue_plan_pending_route_member_marker_key(
        coordinator_route,
        State::queue_plan_pending_route_member_identity(&native_obligation, coordinator_route)
            .expect("fixture Native coordinator member identity"),
    )
    .expect("fixture Native coordinator member key");
    let native_participant_member_key = State::queue_plan_pending_route_member_marker_key(
        participant_route,
        State::queue_plan_pending_route_member_identity(&native_obligation, participant_route)
            .expect("fixture Native participant member identity"),
    )
    .expect("fixture Native participant member key");
    let single_coordinator_member_key = State::queue_plan_pending_route_member_marker_key(
        coordinator_route,
        State::queue_plan_pending_route_member_identity(&single_obligation, coordinator_route)
            .expect("fixture single coordinator member identity"),
    )
    .expect("fixture single coordinator member key");
    let native_obligation_key = State::queue_plan_pending_obligation_marker_key(
        native_binding.network_id_digest,
        native_binding.entrypoint_hash.clone(),
    )
    .expect("fixture Native pending-obligation key");
    let single_obligation_key = State::queue_plan_pending_obligation_marker_key(
        single_binding.network_id_digest,
        single_binding.entrypoint_hash.clone(),
    )
    .expect("fixture single-route pending-obligation key");
    seed_exact_queue_plan_admission_state_for_test(&state, &native_certificate);
    seed_exact_queue_plan_admission_state_for_test(&state, &single_certificate);
    let world = state.world.view();
    assert_eq!(
        State::queue_plan_pending_route_obligation_count_from_world(&world, coordinator_route,)
            .expect("shared coordinator count"),
        2
    );
    assert_eq!(
        State::queue_plan_pending_route_obligation_count_from_world(&world, participant_route,)
            .expect("participant count"),
        1
    );
    drop(world);
    {
        let mut world = state.world.block();
        assert!(
            State::resolve_queue_plan_pending_obligation_in_storage(
                &mut world.smart_contract_state,
                native_binding.network_id_digest,
                native_binding.entrypoint_hash.clone(),
            )
            .expect("resolve exact Native QueuePlan obligation")
        );
        assert_eq!(
            State::queue_plan_pending_route_obligation_count_from_world(&world, coordinator_route,)
                .expect("decremented coordinator count"),
            1
        );
        assert_eq!(
            State::queue_plan_pending_route_obligation_count_from_world(&world, participant_route,)
                .expect("removed participant count"),
            0
        );
        assert!(
            world
                .smart_contract_state
                .get(&native_obligation_key)
                .is_none(),
            "resolution must remove only the exact Native obligation"
        );
        assert!(
            world
                .smart_contract_state
                .get(&single_obligation_key)
                .is_some(),
            "the unrelated single-route obligation must remain pending"
        );
        assert!(
            world
                .smart_contract_state
                .get(&native_coordinator_member_key)
                .is_none()
                && world
                    .smart_contract_state
                    .get(&native_participant_member_key)
                    .is_none(),
            "resolution must remove every exact member owned by the Native obligation"
        );
        assert!(
            world
                .smart_contract_state
                .get(&single_coordinator_member_key)
                .is_some(),
            "nonterminal resolution must retain the other coordinator member"
        );
        world.commit();
    }
    assert!(state.lane_has_drain_blocking_evidence(
        coordinator_route.lane_id,
        coordinator_route.dataspace_id,
        coordinator_route.lane_incarnation,
    ));
    assert!(
        !state.lane_has_drain_blocking_evidence(
            participant_route.lane_id,
            participant_route.dataspace_id,
            participant_route.lane_incarnation,
        ),
        "resolving the only participant-bound obligation must unblock that route"
    );
    {
        let mut world = state.world.block();
        assert!(
            State::resolve_queue_plan_pending_obligation_in_storage(
                &mut world.smart_contract_state,
                single_binding.network_id_digest,
                single_binding.entrypoint_hash,
            )
            .expect("resolve exact single-route QueuePlan obligation")
        );
        assert_eq!(
            State::queue_plan_pending_route_obligation_count_from_world(&world, coordinator_route,)
                .expect("removed coordinator count"),
            0
        );
        assert!(
            world
                .smart_contract_state
                .get(&single_obligation_key)
                .is_none(),
            "resolution must remove the final exact pending obligation"
        );
        assert!(
            world
                .smart_contract_state
                .get(&single_coordinator_member_key)
                .is_none(),
            "final resolution must remove the final exact route member"
        );
        world.commit();
    }
    assert!(
        !state.lane_has_drain_blocking_evidence(
            coordinator_route.lane_id,
            coordinator_route.dataspace_id,
            coordinator_route.lane_incarnation,
        ),
        "resolving the final coordinator-bound obligation must unblock that route"
    );
}
