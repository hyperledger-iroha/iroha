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
fn native_amx_request_accepts_exact_autonomous_lane_author() {
    let (mut adapter, keys) = autonomous_test_fixture(wire::ConsensusMode::Permissioned, false);
    let participant_lane = LaneId::new(1);
    let participant_dataspace = DataSpaceId::new(7);
    prepare_autonomous_test_lane(&mut adapter, &keys, participant_lane, participant_dataspace);
    let slot = plan_autonomous_lane_reservation_slot(
        adapter.state.as_ref(),
        adapter.kura.as_ref(),
        &adapter.context,
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    )
    .expect("plan the frozen autonomous coordinator slot");
    let mut request = native_request_with_distinct_participant(
        &adapter,
        &keys,
        participant_lane,
        participant_dataspace,
        slot.lane_block_height,
        slot.previous_lane_block_descriptor_hash,
    );
    request.coordinator_proposal.payload_block_hint = None;
    assert!(
        V2LaneWorkAdapter::autonomous_proposal_matches_reservation_slot(
            &request.coordinator_proposal,
            &slot,
        )
    );
    let author = slot.author;
    let view = (0..u64::try_from(adapter.context.roster.len()).expect("roster length fits u64"))
        .find(|view| {
            adapter.autonomous_native_coordinator_for_view(*view)
                == Some((LaneId::SINGLE, DataSpaceId::UNIVERSAL))
                && usize::try_from(adapter.context.leader(*view))
                    .ok()
                    .and_then(|index| adapter.context.roster.get(index))
                    .is_some_and(|entry| entry.validator != author)
        })
        .expect("some owned Native view has a global leader distinct from the lane author");
    request.body.round.view = view;
    assert!(adapter.native_request_matches_context(&request, view));
    assert!(adapter.native_request_sender_authorized(&request, &author));
    let non_owner_view = (view + 1
        ..view + 1 + u64::try_from(adapter.context.roster.len()).unwrap())
        .find(|candidate| {
            adapter.autonomous_native_coordinator_for_view(*candidate)
                != Some((LaneId::SINGLE, DataSpaceId::UNIVERSAL))
        })
        .expect("the deterministic Native coordinator rotates");
    let mut non_owner_request = request.clone();
    non_owner_request.body.round.view = non_owner_view;
    assert!(adapter.native_request_matches_context(&non_owner_request, non_owner_view));
    assert!(
        !adapter.native_request_sender_authorized(&non_owner_request, &author),
        "an exact lane author waits while another coordinator owns this global view"
    );
    let global_leader = usize::try_from(adapter.context.leader(view))
        .ok()
        .and_then(|index| adapter.context.roster.get(index))
        .expect("fixture view has a global leader")
        .validator
        .clone();
    assert_ne!(global_leader, author);
    assert!(
        !adapter.native_request_sender_authorized(&request, &global_leader),
        "a global leader cannot pre-empt the independently frozen lane author"
    );

    let guard_root = tempfile::tempdir().expect("isolated post-Nexus signing guard root");
    adapter.native_signing_guard = Some(
        NativeAmxSigningGuard::open(
            guard_root.path(),
            adapter.context.height,
            adapter.context.id(),
            adapter.context.epoch,
            adapter.context.network_id,
            adapter.local_peer.clone(),
            adapter.limits.native_amx_signing_guard_limits,
        )
        .expect("open guard against the exact post-Nexus height context"),
    );

    let relay = adapter
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .find(|peer| peer != &author)
        .expect("fixture has a distinct physical relay");
    let mut routes = NetworkReplyRouteTestFixture::new(relay);
    let route = routes.mint(author.clone());
    assert_eq!(
        adapter.accept_native_amx(
            author.clone(),
            Some(route),
            NativeAmxMessage::PrepareRequest(request),
            view,
        ),
        V2LaneIngressOutcome::Inserted
    );
    assert!(adapter.drain_effects(usize::MAX).iter().any(|effect| {
        matches!(
            effect,
            V2LaneWorkEffect::PostNativeAmx {
                peer,
                message: NativeAmxMessage::PrepareVote(_),
                ..
            } if peer == &author
        )
    }));
}
#[test]
fn native_amx_request_keeps_global_leader_authority_on_the_ordinary_path() {
    let (adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let request = native_request(&adapter, &keys);
    let leader = usize::try_from(adapter.context.leader(request.body.round.view))
        .ok()
        .and_then(|index| adapter.context.roster.get(index))
        .expect("fixture view has a global leader")
        .validator
        .clone();
    let non_leader = adapter
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .find(|peer| peer != &leader)
        .expect("fixture has a non-leader validator");

    assert!(adapter.native_request_sender_authorized(&request, &leader));
    assert!(!adapter.native_request_sender_authorized(&request, &non_leader));
}
#[test]
fn native_amx_request_respects_the_configured_source_bound() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    adapter.limits.native_source_capacity = NonZeroUsize::new(1).expect("non-zero source cap");
    let mut request = native_request(&adapter, &keys);
    let second_source = [0xA4; Hash::LENGTH];
    let second_entrypoint = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
        b"second bounded Native AMX entrypoint",
    ));
    let entrypoints = vec![
        Hash::from(second_entrypoint),
        Hash::from(request.body.tx_entrypoint_hash),
    ];
    for proposal in [
        &mut request.coordinator_proposal,
        &mut request.participant_proposal,
    ] {
        proposal.descriptor.accepted_candidate_indices = vec![0, 1];
        proposal.descriptor.accepted_transaction_hashes = entrypoints.clone();
        proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
        proposal.proposal_hash = proposal.computed_proposal_hash();
    }
    request.body.coordinator_proposal_hash = request.coordinator_proposal.proposal_hash;
    request.body.participant_proposal_hash = request.participant_proposal.proposal_hash;
    request.participant_settlement = request
        .body
        .computed_grouped_participant_settlement(&[second_source, request.body.source_id])
        .expect("build a canonical two-source settlement");
    request.body.participant_settlement_commitment = Hash::from(
        iroha_data_model::nexus::compute_settlement_hash(&request.participant_settlement)
            .expect("hash the canonical two-source settlement"),
    );
    assert!(request.validate_plan_binding().is_ok());
    assert!(!adapter.native_request_matches_context(&request, request.body.round.view));
}
#[test]
fn native_request_rotation_reaches_every_peer_and_keeps_delayed_votes_authorized() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    adapter.limits.native_request_capacity =
        NonZeroUsize::new(keys.len()).expect("non-zero Native delivery capacity");
    adapter.limits.effect_capacity = NonZeroUsize::new(1).expect("one total effect slot");
    let request = native_request(&adapter, &keys);
    let body = request.body;
    let validators = request
        .participant_proposal
        .descriptor
        .validator_set
        .clone();
    let expected_remotes = validators
        .iter()
        .filter(|peer| *peer != &adapter.local_peer)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut observed = BTreeSet::new();
    let _ = adapter.drain_effects(usize::MAX);

    for _ in 0..validators.len().saturating_mul(2) {
        adapter.ensure_native_prepare_requests(&request, &validators, body.round.view);
        for effect in adapter.drain_effects(usize::MAX) {
            assert!(
                effect.retries_from_native_catalog_after_source_retention(),
                "a declined request is recreated by compact catalog ownership"
            );
            if let V2LaneWorkEffect::PostNativeAmx {
                peer,
                message: NativeAmxMessage::PrepareRequest(_),
                ..
            } = effect
            {
                observed.insert(peer);
            }
        }
        adapter.schedule_native_retransmissions();
        for effect in adapter.drain_effects(usize::MAX) {
            assert!(
                effect.retries_from_native_catalog_after_source_retention(),
                "a rotated retry remains catalog-backed"
            );
            if let V2LaneWorkEffect::PostNativeAmx {
                peer,
                message: NativeAmxMessage::PrepareRequest(_),
                ..
            } = effect
            {
                observed.insert(peer);
            }
        }
    }
    assert_eq!(observed, expected_remotes);
    assert_eq!(
        adapter
            .native_requests
            .get(&body)
            .map(|entry| &entry.expected_peers),
        Some(&expected_remotes)
    );

    for peer in expected_remotes {
        let key = keys
            .iter()
            .find(|key| key.public_key() == peer.public_key())
            .expect("fixture retains every validator key");
        let signature = Signature::try_new(key.private_key(), &body.signature_preimage())
            .expect("sign delayed Native AMX vote");
        let vote = NativeAmxVoteV2 {
            body,
            signer: peer.clone(),
            bls_signature: signature.payload().to_vec(),
        };
        assert_eq!(
            adapter.accept_native_vote(peer, vote, NativeAmxPhase::Prepare, body.round.view),
            V2LaneIngressOutcome::Inserted,
            "authorization must outlive any finite number of bounded delivery rotations"
        );
    }
    assert!(adapter.native_requests.is_empty());
}
#[test]
fn native_vote_requires_an_exact_locally_issued_request() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let request = native_request(&adapter, &keys);
    let body = request.body;
    let remote_key = keys
        .iter()
        .find(|key| key.public_key() != adapter.local_peer.public_key())
        .expect("fixture has a remote validator");
    let remote = PeerId::new(remote_key.public_key().clone());
    let signature = Signature::try_new(remote_key.private_key(), &body.signature_preimage())
        .expect("sign exact remote vote");
    let vote = NativeAmxVoteV2 {
        body,
        signer: remote.clone(),
        bls_signature: signature.payload().to_vec(),
    };
    assert_eq!(
        adapter.accept_native_vote(
            remote.clone(),
            vote.clone(),
            NativeAmxPhase::Prepare,
            body.round.view,
        ),
        V2LaneIngressOutcome::Rejected
    );
    assert!(
        adapter
            .native_sessions
            .sorted_votes_for_body(NativeAmxSessionKey::from_body(&body), &body)
            .is_empty()
    );

    assert!(adapter.register_native_request(
        body,
        remote.clone(),
        NativeAmxMessage::PrepareRequest(request),
    ));
    assert_eq!(
        adapter.accept_native_vote(remote, vote, NativeAmxPhase::Prepare, body.round.view,),
        V2LaneIngressOutcome::Inserted
    );
}
#[test]
fn native_request_claims_reject_recomputed_source_and_slot_bodies_within_view() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let request = native_request(&adapter, &keys);
    let body = request.body;
    assert!(adapter.authorize_native_request_bodies(&[body]));

    let mut changed_source = body;
    changed_source.coordinator_proposal_hash = Hash::new(b"recomputed coordinator proposal");
    assert!(
        !adapter.authorize_native_request_bodies(&[changed_source]),
        "one source cannot publish a recomputed coordinator proposal in the same view"
    );

    let mut changed_slot = body;
    changed_slot.source_id = [0xD4; Hash::LENGTH];
    changed_slot.participant_proposal_hash = Hash::new(b"recomputed participant proposal");
    changed_slot.participant_settlement_commitment =
        Hash::new(b"recomputed participant settlement");
    assert!(
        !adapter.authorize_native_request_bodies(&[changed_slot]),
        "distinct sources cannot race incompatible claims for one participant slot"
    );

    adapter
        .retain_native_amx_for_global_view(body.round.view + 1)
        .expect("certified view supersedes volatile Native claims");
    changed_source.round.view += 1;
    assert!(
        adapter.authorize_native_request_bodies(&[changed_source]),
        "an uncertified Prepare claim is superseded by a numeric certified view"
    );
}
#[test]
fn global_body_lock_retires_and_fences_native_request_ownership() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let request = native_request(&adapter, &keys);
    let body = request.body;
    let remote = adapter
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .find(|peer| peer != &adapter.local_peer)
        .expect("fixture has a remote validator");
    assert!(adapter.register_native_request(
        body,
        remote.clone(),
        NativeAmxMessage::PrepareRequest(request.clone()),
    ));
    assert!(!adapter.native_requests.is_empty());

    let (block, _) = planned_lane_candidate_block_at_view(&adapter, &keys, body.round.view);
    mark_global_body_locked_for_block(&mut adapter, &block);
    assert!(adapter.native_requests.is_empty());
    assert!(adapter.native_request_source_claims.is_empty());
    assert!(adapter.native_request_slot_claims.is_empty());
    assert!(!adapter.native_sessions.has_pending_votes_for_lane(
        body.participant_lane_id,
        body.participant_dataspace_id,
        body.participant_lane_incarnation,
    ));
    assert!(
        adapter
            .effects
            .iter()
            .all(|effect| !matches!(effect, V2LaneWorkEffect::PostNativeAmx { .. }))
    );
    adapter.schedule_native_retransmissions();
    assert!(
        adapter
            .effects
            .iter()
            .all(|effect| !matches!(effect, V2LaneWorkEffect::PostNativeAmx { .. }))
    );
    assert!(!adapter.native_body_matches_context(&body, body.round.view));
    assert!(!adapter.register_native_request(
        body,
        remote,
        NativeAmxMessage::PrepareRequest(request),
    ));
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
    assert_eq!(receipt.network_id, adapter.context.network_id);
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
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
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
