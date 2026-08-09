#[test]
fn native_amx_request_rejects_inactive_reply_route_before_signing() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let request = native_request(&adapter, &keys);
    let leader = usize::try_from(adapter.context.leader(request.body.round.view))
        .ok()
        .and_then(|index| adapter.context.roster.get(index))
        .expect("fixture view has a leader")
        .validator
        .clone();
    let relay = adapter
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .find(|peer| peer != &leader)
        .expect("fixture has a distinct authenticated relay");
    let mut routes = NetworkReplyRouteTestFixture::new(relay);
    let route = routes.mint(leader.clone());
    assert!(routes.retire(&route));

    assert_eq!(
        adapter.accept_native_amx(
            leader,
            Some(route),
            NativeAmxMessage::PrepareRequest(request),
            0,
        ),
        V2LaneIngressOutcome::Rejected
    );
    assert!(adapter.local_native_claims.is_empty());
    assert!(adapter.drain_effects(usize::MAX).is_empty());
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
fn native_amx_request_rejects_same_next_height_wrong_coordinator_predecessor_hash() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let participant_lane_id = LaneId::new(1);
    let participant_dataspace_id = DataSpaceId::new(7);
    let _participant_validators = enable_multilane_nexus(
        &mut adapter,
        &keys,
        participant_lane_id,
        participant_dataspace_id,
    );
    let coordinator_lane_incarnation = adapter
        .state
        .lane_incarnation_at_height(LaneId::SINGLE, adapter.context.height)
        .expect("fixture coordinator lane incarnation");
    let predecessor = proposal_for_route(
        &adapter,
        &keys,
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
        coordinator_lane_incarnation,
        adapter.context.height,
        1,
    );
    let predecessor = store_canonical_anchor(&adapter, &predecessor, &keys[0]);
    let exact_predecessor_hash = predecessor.descriptor.descriptor_hash;

    let exact = native_request_with_distinct_participant(
        &adapter,
        &keys,
        participant_lane_id,
        participant_dataspace_id,
        2,
        Some(exact_predecessor_hash),
    );
    assert_eq!(exact.validate_plan_binding(), Ok(()));
    assert!(adapter.native_body_matches_context(&exact.body, 0));
    assert!(adapter.native_request_matches_context(&exact, 0));

    let forged = native_request_with_distinct_participant(
        &adapter,
        &keys,
        participant_lane_id,
        participant_dataspace_id,
        2,
        Some(Hash::new(b"wrong-coordinator-predecessor-at-height-one")),
    );
    assert_eq!(forged.validate_plan_binding(), Ok(()));
    assert_eq!(
        forged.body.planned_coordinator_block_height, exact.body.planned_coordinator_block_height,
        "the adversarial request must preserve the exact next height"
    );
    assert_eq!(
        forged
            .coordinator_proposal
            .descriptor
            .previous_lane_block_height,
        predecessor.descriptor.lane_block_height,
        "the adversarial request must preserve the exact predecessor height"
    );
    assert!(
        adapter.native_body_matches_context(&forged.body, 0),
        "the body-only height guard cannot distinguish the forged predecessor hash"
    );
    assert!(!adapter.native_request_matches_context(&forged, 0));
    assert!(
        adapter.sign_native_request_once(&forged, 0).is_none(),
        "the production signing boundary must retain and reject the forged proposal"
    );

    let leader = usize::try_from(adapter.context.leader(forged.body.round.view))
        .ok()
        .and_then(|index| adapter.context.roster.get(index))
        .expect("fixture view has a leader")
        .validator
        .clone();
    let relay = adapter
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .find(|peer| peer != &leader)
        .expect("fixture has a distinct authenticated relay");
    let mut routes = NetworkReplyRouteTestFixture::new(relay);
    let route = routes.mint(leader.clone());
    assert_eq!(
        adapter.accept_native_amx(
            leader,
            Some(route),
            NativeAmxMessage::PrepareRequest(forged),
            0,
        ),
        V2LaneIngressOutcome::Rejected,
        "request admission must use the exact production signing predicate"
    );
    assert!(adapter.local_native_claims.is_empty());
    assert_eq!(
        adapter
            .native_signing_guard
            .as_ref()
            .expect("validator has durable Native AMX guard")
            .record_count_for_test(),
        0,
        "a forged predecessor must be rejected before durable authority is recorded"
    );
}

#[test]
fn native_coordinator_height_ignores_retired_incarnation_artifacts() {
    let (adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let lane_id = LaneId::SINGLE;
    let dataspace_id = DataSpaceId::UNIVERSAL;
    let retired_incarnation = adapter
        .state
        .lane_incarnation_at_height(lane_id, adapter.context.height)
        .expect("fixture lane incarnation");
    let historical = proposal_for_route(
        &adapter,
        &keys,
        lane_id,
        dataspace_id,
        retired_incarnation,
        adapter.context.height,
        100,
    );
    let _ = store_canonical_anchor(&adapter, &historical, &keys[0]);
    assert!(
        adapter
            .kura
            .latest_lane_block_artifact(lane_id)
            .is_some_and(|artifact| artifact.ownership.lane_block_height == 100),
        "fixture must first install a reachable high lane-local artifact"
    );

    let recreated_catalog = LaneCatalog::new(
        NonZeroU32::new(1).expect("non-zero lane count"),
        vec![LaneConfig {
            alias: "recreated-default".to_owned(),
            ..LaneConfig::default()
        }],
    )
    .expect("recreated default-lane catalog");
    {
        let mut nexus = adapter.state.nexus.write();
        nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&recreated_catalog);
        nexus.lane_catalog = recreated_catalog;
    }
    adapter.state.reseed_static_lane_incarnations_for_tests();
    assert_ne!(
        adapter
            .state
            .lane_incarnation_at_height(lane_id, adapter.context.height),
        Some(retired_incarnation),
        "lane recreation must retire the historical namespace"
    );
    assert!(
        adapter.kura.latest_lane_block_artifact(lane_id).is_none(),
        "the active Kura marker must hide the retired high artifact"
    );

    let body = native_body(&adapter);
    assert!(
        adapter.native_coordinator_height_is_current(&body),
        "retired-incarnation history must not advance the active coordinator height"
    );
    assert!(adapter.native_body_matches_context(&body, 0));
}

#[test]
fn full_native_amx_receipt_metadata_is_derived_from_frozen_context_and_proposal() {
    let (adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let proposal = coordinator_proposal(&adapter, &keys);
    let coordinator = RoutingDecision::new(
        proposal.descriptor.lane_id,
        proposal.descriptor.dataspace_id,
    );
    let source_id = [0x5A; Hash::LENGTH];
    let plan_digest = Hash::new(b"full-native-amx-plan");
    let receipt = adapter
        .assemble_native_receipt(source_id, coordinator, plan_digest, &proposal, Vec::new())
        .expect("canonical coordinator proposal builds a full receipt");
    assert_eq!(receipt.version, 2);
    assert_eq!(receipt.source_id, source_id);
    assert_eq!(
        receipt.chain_id_hash,
        Hash::prehashed(*adapter.context.network_id.as_bytes())
    );
    assert_eq!(receipt.plan_digest, plan_digest);
    assert_eq!(receipt.lane_id, proposal.descriptor.lane_id);
    assert_eq!(receipt.dataspace_id, proposal.descriptor.dataspace_id);
    assert_eq!(
        receipt.lane_incarnation,
        proposal.descriptor.lane_incarnation
    );
    assert_eq!(
        receipt.authority_context_height,
        proposal.descriptor.proposal_height
    );
    assert_eq!(
        receipt.lane_block_height,
        proposal.descriptor.lane_block_height
    );
    assert_eq!(receipt.lane_block_view, proposal.descriptor.lane_block_view);
    assert_eq!(receipt.coordinator_proposal_hash, proposal.proposal_hash);

    let mut wrong_height = proposal;
    wrong_height.descriptor.proposal_height = adapter.context.height.saturating_add(1);
    wrong_height.descriptor.descriptor_hash = wrong_height.descriptor.computed_descriptor_hash();
    wrong_height.proposal_hash = wrong_height.computed_proposal_hash();
    assert!(
        adapter
            .assemble_native_receipt(
                source_id,
                coordinator,
                plan_digest,
                &wrong_height,
                Vec::new(),
            )
            .is_none(),
        "receipt assembly must reject a proposal outside the frozen authority height"
    );
}

#[test]
fn lane_signing_boundary_requires_exact_descriptor_membership() {
    let (adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let (_, mut proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    assert!(
        proposal
            .descriptor
            .validator_set
            .contains(&adapter.local_peer),
        "fixture starts with local lane authority"
    );
    let replacement = PeerId::new(
        KeyPair::try_from_seed(vec![0xA9; 32], Algorithm::BlsNormal)
            .expect("derive descriptor-only replacement")
            .public_key()
            .clone(),
    );
    let local_index = proposal
        .descriptor
        .validator_set
        .iter()
        .position(|peer| peer == &adapter.local_peer)
        .expect("local validator belongs to fixture descriptor");
    proposal.descriptor.validator_set[local_index] = replacement;
    proposal.descriptor.validator_set.sort();
    proposal.descriptor.validator_set_hash = HashOf::new(&proposal.descriptor.validator_set);
    proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
    proposal.proposal_hash = proposal.computed_proposal_hash();
    assert!(
        !proposal
            .descriptor
            .validator_set
            .contains(&adapter.local_peer)
    );
    assert!(
        adapter
            .sign_lane_vote(&proposal, CertPhase::Prepare)
            .is_none(),
        "configured validator role cannot sign a descriptor which omits the local key"
    );
}
