#[test]
fn native_amx_context_guard_rejects_replayed_round_epoch_and_future_view() {
    let (adapter, _) = fixture(wire::ConsensusMode::Permissioned);
    let body = native_body(&adapter);
    assert!(adapter.native_body_matches_context(&body, 0));

    let mut wrong_context = body;
    wrong_context.round.context_id =
        wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(b"other-context")));
    assert!(!adapter.native_body_matches_context(&wrong_context, 0));

    let mut wrong_epoch = body;
    wrong_epoch.epoch = wrong_epoch.epoch.saturating_add(1);
    assert!(!adapter.native_body_matches_context(&wrong_epoch, 0));

    let mut future_view = body;
    future_view.round.view = 1;
    assert!(!adapter.native_body_matches_context(&future_view, 0));
    assert!(adapter.native_body_matches_context(&future_view, 1));

    let mut wrong_lane_height = body;
    wrong_lane_height.planned_coordinator_block_height = 2;
    assert!(!adapter.native_body_matches_context(&wrong_lane_height, 0));
}

#[test]
fn native_signing_boundary_rechecks_view_routes_predecessors_and_authority() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let exact = native_request(&adapter, &keys);

    let mut wrong_context = exact.clone();
    wrong_context.body.round.context_id =
        wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(b"other-context")));
    let mut wrong_round_height = exact.clone();
    wrong_round_height.body.round.height = wrong_round_height.body.round.height.saturating_add(1);
    let mut wrong_epoch = exact.clone();
    wrong_epoch.body.epoch = wrong_epoch.body.epoch.saturating_add(1);
    let mut wrong_chain = exact.clone();
    wrong_chain.body.chain_id_hash = Hash::new(b"other-chain");
    let mut wrong_coordinator_dataspace = exact.clone();
    wrong_coordinator_dataspace.body.coordinator_dataspace_id = DataSpaceId::new(70);
    let mut wrong_coordinator_incarnation = exact.clone();
    wrong_coordinator_incarnation
        .body
        .coordinator_lane_incarnation = Hash::new(b"other-coordinator-incarnation");
    let mut wrong_participant_lane = exact.clone();
    wrong_participant_lane.body.participant_lane_id = LaneId::new(20);
    let mut wrong_participant_dataspace = exact.clone();
    wrong_participant_dataspace.body.participant_dataspace_id = DataSpaceId::new(80);
    let mut wrong_participant_incarnation = exact.clone();
    wrong_participant_incarnation
        .body
        .participant_lane_incarnation = Hash::new(b"other-participant-incarnation");
    let mut wrong_coordinator_predecessor = exact.clone();
    wrong_coordinator_predecessor
        .body
        .planned_coordinator_block_height = wrong_coordinator_predecessor
        .body
        .planned_coordinator_block_height
        .saturating_add(1);
    let mut wrong_participant_predecessor = exact.clone();
    wrong_participant_predecessor
        .body
        .participant_previous_block_height = 1;
    wrong_participant_predecessor
        .body
        .participant_previous_block_descriptor_hash =
        Some(Hash::new(b"other-participant-predecessor"));
    wrong_participant_predecessor
        .body
        .participant_lane_block_height = 2;
    let mut wrong_authority = exact.clone();
    wrong_authority.body.authority_context_height = wrong_authority
        .body
        .authority_context_height
        .saturating_add(1);

    for (label, request, active_view) in [
        ("active reducer view", exact.clone(), 1),
        ("height context", wrong_context, 0),
        ("round height", wrong_round_height, 0),
        ("epoch", wrong_epoch, 0),
        ("chain", wrong_chain, 0),
        ("coordinator dataspace", wrong_coordinator_dataspace, 0),
        ("coordinator incarnation", wrong_coordinator_incarnation, 0),
        ("participant lane", wrong_participant_lane, 0),
        ("participant dataspace", wrong_participant_dataspace, 0),
        ("participant incarnation", wrong_participant_incarnation, 0),
        ("coordinator predecessor", wrong_coordinator_predecessor, 0),
        ("participant predecessor", wrong_participant_predecessor, 0),
        ("authority height", wrong_authority, 0),
    ] {
        assert!(
            adapter
                .sign_native_request_once(&request, active_view)
                .is_none(),
            "{label} drift must be rejected at the final signing boundary"
        );
    }
    assert!(
        adapter.local_native_claims.is_empty(),
        "rejected context drift must not become volatile signing authority"
    );
    assert_eq!(
        adapter
            .native_signing_guard
            .as_ref()
            .expect("validator has durable Native AMX guard")
            .record_count_for_test(),
        0,
        "context drift must be rejected before any durable claim is recorded"
    );
    assert!(
        !adapter.output_guard.restart_required(),
        "ordinary stale-context rejection is not a journal ambiguity"
    );

    adapter
        .sign_native_request_once(&exact, 0)
        .expect("the exact request remains signable under its authoritative active view");
    assert_eq!(
        adapter
            .native_signing_guard
            .as_ref()
            .expect("validator has durable Native AMX guard")
            .record_count_for_test(),
        1
    );
}

fn native_request_for_distinct_routes(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    coordinator: RoutingDecision,
    participant: RoutingDecision,
) -> NativeAmxAttestationRequestV2 {
    assert_ne!(coordinator, participant);
    let mut body = native_body(adapter);
    body.coordinator_lane_id = coordinator.lane_id;
    body.coordinator_dataspace_id = coordinator.dataspace_id;
    body.coordinator_lane_incarnation = adapter
        .state
        .lane_incarnation_at_height(coordinator.lane_id, body.authority_context_height)
        .expect("fixture coordinator incarnation");
    body.participant_lane_id = participant.lane_id;
    body.participant_dataspace_id = participant.dataspace_id;
    body.participant_lane_incarnation = adapter
        .state
        .lane_incarnation_at_height(participant.lane_id, body.authority_context_height)
        .expect("fixture participant incarnation");
    let (participant_validators, participant_min_signers) = adapter
        .native_committee_shape_for_route(
            participant.lane_id,
            participant.dataspace_id,
            body.authority_context_height,
        )
        .expect("fixture participant committee");
    body.participant_validator_set_hash = HashOf::new(&participant_validators);
    body.participant_validator_count =
        u32::try_from(participant_validators.len()).expect("fixture validator count");
    body.participant_min_quorum =
        u32::try_from(participant_min_signers).expect("fixture participant quorum");
    let plan = RoutingPlan::native_amx(
        coordinator,
        vec![RouteLeg::new(participant, RouteLegRole::Participant)],
    );
    body.plan_digest = plan.digest();

    let mut ordered_keys = keys.to_vec();
    ordered_keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let coordinator_base = proposal_for_route(
        adapter,
        &ordered_keys,
        coordinator.lane_id,
        coordinator.dataspace_id,
        body.coordinator_lane_incarnation,
        body.authority_context_height,
        1,
    );
    let mut coordinator_ownership = ownership_from_proposal(&coordinator_base);
    coordinator_ownership.accepted_transaction_hashes = vec![Hash::from(body.tx_entrypoint_hash)];
    let coordinator_replay = coordinator_ownership
        .compute_replay_hashes()
        .expect("fixture coordinator replay material");
    coordinator_ownership.subject_hash = coordinator_replay.subject_hash;
    coordinator_ownership.payload_ownership_hash = coordinator_replay.payload_ownership_hash;
    coordinator_ownership.rbc_instance_hash = coordinator_replay.rbc_instance_hash;
    coordinator_ownership.lane_block_descriptor_hash =
        Some(coordinator_replay.lane_block_descriptor_hash);
    let coordinator_proposal = proposal_from_ownership(
        &coordinator_ownership,
        HashOf::from_untyped_unchecked(Hash::new(
            b"native signing distinct coordinator proposal hint",
        )),
    )
    .expect("fixture distinct coordinator proposal");
    body.planned_coordinator_block_height = coordinator_proposal.descriptor.lane_block_height;
    body.coordinator_lane_block_view = coordinator_proposal.descriptor.lane_block_view;
    body.coordinator_proposal_hash = coordinator_proposal.proposal_hash;

    let mut participant_proposal = proposal_for_route(
        adapter,
        &ordered_keys,
        participant.lane_id,
        participant.dataspace_id,
        body.participant_lane_incarnation,
        body.authority_context_height,
        1,
    );
    participant_proposal.payload_block_hint = None;
    let participant_descriptor = &participant_proposal.descriptor;
    body.participant_previous_block_height = participant_descriptor.previous_lane_block_height;
    body.participant_previous_block_descriptor_hash =
        participant_descriptor.previous_lane_block_descriptor_hash;
    body.participant_lane_block_height = participant_descriptor.lane_block_height;
    body.participant_lane_block_view = participant_descriptor.lane_block_view;
    body.participant_proposal_hash = participant_proposal.proposal_hash;
    let participant_settlement = body
        .computed_grouped_participant_settlement(&[body.source_id])
        .expect("fixture distinct participant settlement");
    body.participant_settlement_commitment = Hash::from(
        iroha_data_model::nexus::compute_settlement_hash(&participant_settlement)
            .expect("hash fixture distinct participant settlement"),
    );

    NativeAmxAttestationRequestV2 {
        body,
        plan_legs: plan.legs(),
        coordinator_proposal,
        participant_proposal,
        participant_settlement,
    }
}

fn recreate_native_signing_test_lane(
    adapter: &V2LaneWorkAdapter,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
) -> Hash {
    adapter
        .state
        .apply_lane_lifecycle(&iroha_data_model::nexus::LaneLifecyclePlan {
            additions: vec![LaneConfig {
                id: lane_id,
                dataspace_id,
                alias: "independent-lane".to_owned(),
                ..LaneConfig::default()
            }],
            retire: vec![lane_id],
        })
        .expect("recreate Native signing fixture lane through production lifecycle semantics");
    adapter
        .state
        .lane_incarnation_at_height(lane_id, adapter.context.height)
        .expect("recreated Native signing fixture lane is active")
}

#[test]
fn native_signing_boundary_rejects_plan_valid_participant_predecessor_drift() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let participant = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
    enable_multilane_nexus(
        &mut adapter,
        &keys,
        participant.lane_id,
        participant.dataspace_id,
    );
    let mut request = native_request_with_distinct_participant(
        &adapter,
        &keys,
        participant.lane_id,
        participant.dataspace_id,
        1,
        None,
    );

    let mut ordered_keys = keys.clone();
    ordered_keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let mut participant_proposal = proposal_for_route(
        &adapter,
        &ordered_keys,
        participant.lane_id,
        participant.dataspace_id,
        request.body.participant_lane_incarnation,
        request.body.authority_context_height,
        2,
    );
    participant_proposal.payload_block_hint = None;
    let descriptor = &participant_proposal.descriptor;
    request.body.participant_previous_block_height = descriptor.previous_lane_block_height;
    request.body.participant_previous_block_descriptor_hash =
        descriptor.previous_lane_block_descriptor_hash;
    request.body.participant_lane_block_height = descriptor.lane_block_height;
    request.body.participant_lane_block_view = descriptor.lane_block_view;
    request.body.participant_proposal_hash = participant_proposal.proposal_hash;
    request.participant_proposal = participant_proposal;
    request.participant_settlement = request
        .body
        .computed_grouped_participant_settlement(&[request.body.source_id])
        .expect("plan-valid predecessor-drift settlement");
    request.body.participant_settlement_commitment = Hash::from(
        iroha_data_model::nexus::compute_settlement_hash(&request.participant_settlement)
            .expect("hash plan-valid predecessor-drift settlement"),
    );

    assert_eq!(request.validate_plan_binding(), Ok(()));
    assert!(
        !adapter.native_request_matches_context(&request, 0),
        "an internally coherent request cannot invent a participant predecessor absent from State/Kura"
    );
    assert!(adapter.sign_native_request_once(&request, 0).is_none());
    assert_eq!(
        adapter
            .native_signing_guard
            .as_ref()
            .expect("validator has durable Native AMX guard")
            .record_count_for_test(),
        0,
        "participant predecessor drift must fail before durable authorization"
    );
}

#[test]
fn native_signing_boundary_rejects_delayed_participant_after_same_id_recreation() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let participant = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
    enable_multilane_nexus(
        &mut adapter,
        &keys,
        participant.lane_id,
        participant.dataspace_id,
    );
    let request = native_request_with_distinct_participant(
        &adapter,
        &keys,
        participant.lane_id,
        participant.dataspace_id,
        1,
        None,
    );
    assert_eq!(request.validate_plan_binding(), Ok(()));
    assert!(adapter.native_request_matches_context(&request, 0));
    let incarnation_a = request.body.participant_lane_incarnation;
    let coordinator_incarnation = request.body.coordinator_lane_incarnation;

    let incarnation_b =
        recreate_native_signing_test_lane(&adapter, participant.lane_id, participant.dataspace_id);
    assert_ne!(incarnation_b, incarnation_a);
    assert_eq!(
        adapter.state.lane_incarnation_at_height(
            request.body.coordinator_lane_id,
            request.body.authority_context_height,
        ),
        Some(coordinator_incarnation),
        "participant recreation must leave the coordinator incarnation authoritative"
    );
    assert_eq!(request.validate_plan_binding(), Ok(()));
    assert!(!adapter.native_request_matches_context(&request, 0));
    assert!(adapter.sign_native_request_once(&request, 0).is_none());

    let incarnation_c =
        recreate_native_signing_test_lane(&adapter, participant.lane_id, participant.dataspace_id);
    assert_ne!(incarnation_c, incarnation_a);
    assert_ne!(incarnation_c, incarnation_b);
    assert!(
        adapter.sign_native_request_once(&request, 0).is_none(),
        "a delayed incarnation-A request must remain rejected across repeated same-ID A/B/A route recreation"
    );
    assert_eq!(
        adapter
            .native_signing_guard
            .as_ref()
            .expect("validator has durable Native AMX guard")
            .record_count_for_test(),
        0
    );
}

#[test]
fn native_signing_boundary_rejects_plan_valid_stale_coordinator_incarnation() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let recreated_route = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
    enable_multilane_nexus(
        &mut adapter,
        &keys,
        recreated_route.lane_id,
        recreated_route.dataspace_id,
    );
    let participant = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let request = native_request_for_distinct_routes(&adapter, &keys, recreated_route, participant);
    assert_eq!(request.validate_plan_binding(), Ok(()));
    assert!(adapter.native_request_matches_context(&request, 0));
    let stale_coordinator_incarnation = request.body.coordinator_lane_incarnation;
    let participant_incarnation = request.body.participant_lane_incarnation;

    let active_coordinator_incarnation = recreate_native_signing_test_lane(
        &adapter,
        recreated_route.lane_id,
        recreated_route.dataspace_id,
    );
    assert_ne!(
        active_coordinator_incarnation,
        stale_coordinator_incarnation
    );
    assert_eq!(
            adapter.state.lane_incarnation_at_height(
                participant.lane_id,
                request.body.authority_context_height,
            ),
            Some(participant_incarnation),
            "coordinator recreation must leave the participant incarnation authoritative"
        );
    assert_eq!(request.validate_plan_binding(), Ok(()));
    assert!(!adapter.native_request_matches_context(&request, 0));
    assert!(adapter.sign_native_request_once(&request, 0).is_none());
    assert_eq!(
        adapter
            .native_signing_guard
            .as_ref()
            .expect("validator has durable Native AMX guard")
            .record_count_for_test(),
        0
    );
}

#[test]
fn native_signing_boundary_rechecks_state_after_durable_record_before_signature() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let participant = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
    enable_multilane_nexus(
        &mut adapter,
        &keys,
        participant.lane_id,
        participant.dataspace_id,
    );
    let request = native_request_with_distinct_participant(
        &adapter,
        &keys,
        participant.lane_id,
        participant.dataspace_id,
        1,
        None,
    );
    assert_eq!(request.validate_plan_binding(), Ok(()));
    assert!(adapter.native_request_matches_context(&request, 0));
    let initial_incarnation = request.body.participant_lane_incarnation;
    let context_checks = Cell::new(0_u8);

    let vote = adapter.sign_native_vote_once_with_context(&request, |adapter| {
        let check = context_checks.get().saturating_add(1);
        context_checks.set(check);
        if check == 3 {
            let recreated = recreate_native_signing_test_lane(
                adapter,
                participant.lane_id,
                participant.dataspace_id,
            );
            assert_ne!(recreated, initial_incarnation);
        }
        adapter.native_request_matches_context(&request, 0)
    });

    assert!(
        vote.is_none(),
        "a route change after durable authorization must prevent signature emission"
    );
    assert_eq!(context_checks.get(), 3);
    assert_eq!(
        adapter
            .native_signing_guard
            .as_ref()
            .expect("validator has durable Native AMX guard")
            .record_count_for_test(),
        1,
        "the deterministic state change must occur only after durable authorization"
    );
    assert!(adapter.local_native_claims.is_empty());
    assert!(!adapter.output_guard.restart_required());
}
