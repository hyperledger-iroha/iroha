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
fn native_amx_request_rejects_global_hint_without_autonomous_authority() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
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

    assert!(request.coordinator_proposal.payload_block_hint.is_some());
    assert!(!adapter.native_request_sender_authorized(&request, &leader));
    assert!(!adapter.native_request_sender_authorized(&request, &non_leader));
    let mut routes = NetworkReplyRouteTestFixture::new(non_leader);
    let reply_route = routes.mint(leader.clone());
    assert_eq!(
        adapter.accept_native_amx(
            leader,
            Some(reply_route),
            NativeAmxMessage::PrepareRequest(request),
            0,
        ),
        V2LaneIngressOutcome::Rejected,
        "a global carrier hint cannot replace autonomous coordinator authority"
    );
    assert!(adapter.local_native_claims.is_empty());
    assert!(adapter.drain_effects(usize::MAX).is_empty());
    assert_eq!(
        adapter
            .native_signing_guard
            .as_ref()
            .expect("validator signing guard")
            .record_count_for_test(),
        0,
    );
    assert!(!adapter.output_guard.restart_required());
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
        .computed_grouped_participant_settlement(None, &[second_source, request.body.source_id])
        .expect("build a canonical two-source settlement");
    request.body.participant_settlement_commitment = Hash::from(
        request
            .participant_settlement
            .computed_hash()
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
    let (mut adapter, _, lane_id, dataspace_id, previous) =
        native_coordinator_after_applied_participant_fixture();
    let exact = native_coordinator_successor_request(&adapter, lane_id, dataspace_id, &previous);
    assert!(adapter.native_request_matches_context(&exact, 0));
    let mut forged = exact.clone();
    let wrong_hash = Hash::new(b"wrong-coordinator-predecessor-at-height-one");
    for proposal in [
        &mut forged.coordinator_proposal,
        &mut forged.participant_proposal,
    ] {
        let mut ownership = ownership_from_proposal(proposal);
        ownership.previous_lane_block_descriptor_hash = Some(wrong_hash);
        let replay = ownership
            .compute_replay_hashes()
            .expect("a competing predecessor is structurally valid replay material");
        ownership.subject_hash = replay.subject_hash;
        ownership.payload_ownership_hash = replay.payload_ownership_hash;
        ownership.rbc_instance_hash = replay.rbc_instance_hash;
        ownership.lane_block_descriptor_hash = Some(replay.lane_block_descriptor_hash);
        *proposal = proposal_from_ownership(
            &ownership,
            HashOf::from_untyped_unchecked(Hash::new(b"unused competing proposal hint")),
        )
        .expect("reconstruct exact competing proposal");
        proposal.payload_block_hint = None;
    }
    forged.body.coordinator_proposal_hash = forged.coordinator_proposal.proposal_hash;
    forged.body.participant_proposal_hash = forged.participant_proposal.proposal_hash;
    forged.body.participant_previous_block_descriptor_hash = Some(wrong_hash);
    assert_eq!(forged.validate_plan_binding(), Ok(()));
    assert_eq!(
        forged.body.planned_coordinator_block_height,
        exact.body.planned_coordinator_block_height
    );
    assert!(
        adapter
            .native_coordinator_height_is_current(&forged.body)
            .expect("height alone does not authenticate the previous hash")
    );
    assert!(
        !adapter
            .native_coordinator_predecessor_is_current(&forged)
            .expect("valid competing descriptor is not local corruption")
    );
    assert!(!adapter.native_request_matches_context(&forged, 0));
    assert!(adapter.sign_native_request_once(&forged, 0).is_none());
    assert!(adapter.local_native_claims.is_empty());
    assert_eq!(
        adapter
            .native_signing_guard
            .as_ref()
            .unwrap()
            .record_count_for_test(),
        0
    );
    assert!(!adapter.output_guard.restart_required());
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
            .expect("authenticate the current lane frontier")
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
    assert_eq!(
        adapter
            .state
            .lane_incarnation_at_height(lane_id, adapter.context.height),
        Some(retired_incarnation),
        "an alias is display metadata and cannot retire a consensus namespace"
    );
    let recreated_incarnation = Hash::new(b"recreated Native coordinator lane incarnation");
    assert_eq!(
        adapter
            .state
            .set_lane_incarnation_for_test(lane_id, recreated_incarnation),
        Some(retired_incarnation),
    );
    let recreated_entry = adapter
        .state
        .nexus_snapshot()
        .lane_config
        .entry(lane_id)
        .expect("recreated lane storage entry")
        .clone();
    adapter
        .kura
        .install_lane_incarnation_marker_for_test(&recreated_entry, recreated_incarnation, 0)
        .expect("install the explicit recreated consensus namespace");
    assert_ne!(
        adapter
            .state
            .lane_incarnation_at_height(lane_id, adapter.context.height),
        Some(retired_incarnation),
        "lane recreation must retire the historical namespace"
    );
    assert!(
        adapter
            .kura
            .latest_lane_block_artifact(lane_id)
            .expect("authenticate absence in the recreated lane namespace")
            .is_none(),
        "the active Kura marker must hide the retired high artifact"
    );
    let body = native_body(&adapter);
    assert!(
        adapter
            .native_coordinator_height_is_current(&body)
            .expect("authenticate the active coordinator frontier"),
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
            .expect("descriptor-only signing check should not fail")
            .is_none(),
        "configured validator role cannot sign a descriptor which omits the local key"
    );
}

fn native_coordinator_after_applied_participant_fixture() -> (
    V2LaneWorkAdapter,
    Vec<KeyPair>,
    LaneId,
    DataSpaceId,
    NativeBodyRecoveryPayload,
) {
    let (adapter, keys, lane_id, dataspace_id) = native_body_recovery_adapter();
    assert!(
        adapter
            .state
            .native_amx_participant_application_tips_snapshot()
            .expect("empty Native authority is readable")
            .is_empty()
    );
    let payload = native_body_recovery_payload(&adapter, &keys, lane_id, dataspace_id);
    let carrier = native_body_recovery_carrier(&adapter, &keys, &payload);
    let (_, finality) = native_body_recovery_finality(&adapter, &keys, &carrier);
    adapter
        .kura
        .store_block(carrier.clone())
        .expect("store actual Native carrier");
    adapter
        .kura
        .store_v2_finality_artifact(&finality)
        .expect("publish actual Native manifest and complete wire authority");
    commit_test_block_to_state(
        adapter.state.as_ref(),
        &ValidBlock::committed_from_replay_signed_block(carrier.clone()),
        &adapter.context,
    );
    let checkpoint = crate::snapshot::canonical_state_snapshot_hash(adapter.state.as_ref());
    adapter
        .kura
        .store_wsv_checkpoint(carrier.header().height().get(), carrier.hash(), checkpoint)
        .expect("publish exact committed Native checkpoint");
    adapter
        .kura
        .store_commit_manifest(
            crate::kura::CommitManifest::new(
                carrier.header().height().get(),
                carrier.hash(),
                None,
                None,
                checkpoint,
                None,
            )
            .with_authenticated_v2_commit_authority(&finality),
        )
        .expect("publish exact Native commit metadata");
    let pending = adapter
        .state
        .native_amx_participant_frontiers_pending_durable_evidence_snapshot()
        .expect("genuinely missing Native receipt remains recoverable");
    assert_eq!(pending.len(), 1);
    assert_eq!(
        adapter
            .state
            .unapplied_native_amx_participant_control_heights_snapshot()
            .expect("pending Native marker is readable")
            .get(&(lane_id, dataspace_id)),
        Some(&pending[0].lane_block_height)
    );
    adapter
        .kura
        .repair_native_amx_participant_application_evidence_for_markers(&carrier, &pending)
        .expect("publish authentic receipt through the production repair boundary");
    assert!(
        adapter
            .state
            .native_amx_participant_frontiers_pending_durable_evidence_snapshot()
            .expect("read complete Native authority")
            .is_empty()
    );
    assert_eq!(
        adapter
            .state
            .native_amx_participant_application_tips_snapshot()
            .expect("read exact applied Native tip")
            .len(),
        1
    );

    let mut context = adapter.context.clone();
    context.height += 1;
    context.parent_commit_qc = Some(finality.commit_qc.clone());
    context.snapshot_bootstrap = None;
    context.nexus_amx_context_hash =
        super::super::v2_recovery::committed_nexus_amx_context_hash(adapter.state.as_ref());
    let restart = LaneAdapterRestartParts::capture(&adapter);
    drop(adapter);
    let adapter = restart
        .reopen_isolated(context, true)
        .expect("reopen successor under exact signed Native application authority");
    (adapter, keys, lane_id, dataspace_id, payload)
}
fn native_coordinator_successor_request(
    adapter: &V2LaneWorkAdapter,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    previous: &NativeBodyRecoveryPayload,
) -> NativeAmxAttestationRequestV2 {
    let transaction_key = KeyPair::try_from_seed(vec![0xE9; 32], Algorithm::Ed25519)
        .expect("deterministic successor transaction key");
    let transaction = TransactionBuilder::new(
        adapter.context.network_id,
        AccountId::new(transaction_key.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(transaction_key.private_key());
    let entrypoint_hash = transaction.hash_as_entrypoint();
    let mut source_id = [0_u8; Hash::LENGTH];
    source_id.copy_from_slice(transaction.hash().as_ref());
    let route = RoutingDecision::new(lane_id, dataspace_id);
    let plan = prepare_v2_lane_payload_plan(
        adapter.state.as_ref(),
        adapter.kura.as_ref(),
        &adapter.context,
        0,
        &adapter.local_peer,
        &[route],
        &[Hash::from(entrypoint_hash)],
    )
    .expect("actual production planner extends the applied Native participant");
    assert!(plan.unavailable_indices.is_empty());
    assert_eq!(plan.proposals.len(), 1);
    let proposal = plan.proposals[0].clone();
    let descriptor = &proposal.descriptor;
    assert_eq!(descriptor.previous_lane_block_height, 1);
    assert_eq!(descriptor.lane_block_height, 2);
    assert_eq!(
        descriptor.previous_lane_block_descriptor_hash,
        Some(
            previous
                .request
                .participant_proposal
                .descriptor
                .descriptor_hash
        )
    );
    let routing_plan =
        RoutingPlan::native_amx(route, vec![RouteLeg::new(route, RouteLegRole::Participant)]);
    let mut body = native_body(adapter);
    body.source_id = source_id;
    body.tx_entrypoint_hash = entrypoint_hash;
    body.plan_digest = routing_plan.digest();
    body.coordinator_lane_id = lane_id;
    body.coordinator_dataspace_id = dataspace_id;
    body.coordinator_lane_incarnation = descriptor.lane_incarnation;
    body.planned_coordinator_block_height = descriptor.lane_block_height;
    body.coordinator_lane_block_view = descriptor.lane_block_view;
    body.coordinator_proposal_hash = proposal.proposal_hash;
    body.participant_lane_id = lane_id;
    body.participant_dataspace_id = dataspace_id;
    body.participant_lane_incarnation = descriptor.lane_incarnation;
    body.participant_previous_block_height = descriptor.previous_lane_block_height;
    body.participant_previous_block_descriptor_hash =
        descriptor.previous_lane_block_descriptor_hash;
    body.participant_lane_block_height = descriptor.lane_block_height;
    body.participant_lane_block_view = descriptor.lane_block_view;
    body.participant_proposal_hash = proposal.proposal_hash;
    body.participant_validator_set_hash = descriptor.validator_set_hash;
    body.participant_validator_count = descriptor.validator_count;
    body.participant_min_quorum = descriptor.min_quorum;
    let previous_hash = previous
        .request
        .participant_settlement
        .computed_hash()
        .expect("hash actual prior Native settlement");
    let participant_settlement = body
        .computed_grouped_participant_settlement(Some(previous_hash), &[source_id])
        .expect("construct linked successor Native control");
    body.participant_settlement_commitment = Hash::from(
        participant_settlement
            .computed_hash()
            .expect("hash linked successor control"),
    );
    let request = NativeAmxAttestationRequestV2 {
        body,
        plan_legs: routing_plan.legs(),
        coordinator_proposal: proposal.clone(),
        participant_proposal: proposal,
        participant_settlement,
    };
    request
        .validate_plan_binding()
        .expect("exact production planner request binding");
    request
}
#[test]
fn applied_native_participant_becomes_coordinator_at_shared_successor_height() {
    let (mut adapter, _, lane_id, dataspace_id, previous) =
        native_coordinator_after_applied_participant_fixture();
    assert!(
        adapter
            .kura
            .latest_lane_block_artifact(lane_id)
            .expect("raw history is independently readable")
            .is_none(),
        "the Native participant must not be replaced by a fake raw artifact"
    );
    let request = native_coordinator_successor_request(&adapter, lane_id, dataspace_id, &previous);
    assert!(
        adapter
            .native_coordinator_height_is_current(&request.body)
            .expect("authenticate shared coordinator height")
    );
    assert!(
        adapter
            .native_coordinator_predecessor_is_current(&request)
            .expect("authenticate exact shared coordinator predecessor")
    );
    assert!(adapter.native_request_matches_context(&request, 0));
    let slot = plan_autonomous_lane_reservation_slot(
        adapter.state.as_ref(),
        adapter.kura.as_ref(),
        &adapter.context,
        lane_id,
        dataspace_id,
    )
    .expect("actual deterministic coordinator reservation authority");
    assert!(adapter.native_request_sender_authorized(&request, &slot.author));
    let relay = adapter
        .context
        .roster
        .iter()
        .map(|power| power.validator.clone())
        .find(|peer| peer != &slot.author)
        .expect("distinct authenticated physical relay");
    let mut routes = NetworkReplyRouteTestFixture::new(relay);
    let reply_route = routes.mint(slot.author.clone());
    assert_eq!(
        adapter.accept_native_amx(
            slot.author.clone(),
            Some(reply_route),
            NativeAmxMessage::PrepareRequest(request),
            0
        ),
        V2LaneIngressOutcome::Inserted,
        "the exact Native H1 to coordinator H2 request reaches durable signing and reply publication"
    );
    let effects = adapter.drain_effects(usize::MAX);
    let vote = effects
        .iter()
        .find_map(|effect| match effect {
            V2LaneWorkEffect::PostNativeAmx {
                peer,
                message: NativeAmxMessage::PrepareVote(vote),
                ..
            } if peer == &slot.author => Some(vote),
            _ => None,
        })
        .expect("exact signed vote is published to the authenticated coordinator");
    assert_eq!(
        vote.validate_ingress(NativeAmxPhase::Prepare, Some(&adapter.local_peer)),
        Ok(())
    );
    assert_eq!(
        adapter
            .native_signing_guard
            .as_ref()
            .unwrap()
            .record_count_for_test(),
        1
    );
    assert!(!adapter.output_guard.restart_required());
}
#[test]
fn native_coordinator_successor_rejects_wrong_signed_native_history_link() {
    let (mut adapter, _, lane_id, dataspace_id, previous) =
        native_coordinator_after_applied_participant_fixture();
    let mut request =
        native_coordinator_successor_request(&adapter, lane_id, dataspace_id, &previous);
    let wrong = HashOf::from_untyped_unchecked(Hash::new(b"wrong prior Native settlement"));
    request.participant_settlement = request
        .body
        .computed_grouped_participant_settlement(Some(wrong), &[request.body.source_id])
        .expect("a nonzero competing link is structurally valid");
    request.body.participant_settlement_commitment = Hash::from(
        request
            .participant_settlement
            .computed_hash()
            .expect("hash competing link"),
    );
    assert_eq!(request.validate_plan_binding(), Ok(()));
    assert!(
        adapter
            .native_coordinator_predecessor_is_current(&request)
            .expect("ordinary shared predecessor remains exact")
    );
    assert!(!adapter.native_control_predecessor_is_current(&request));
    assert!(!adapter.native_request_matches_context(&request, 0));
    assert!(adapter.sign_native_request_once(&request, 0).is_none());
    assert_eq!(
        adapter
            .native_signing_guard
            .as_ref()
            .unwrap()
            .record_count_for_test(),
        0
    );
    assert!(
        !adapter.output_guard.restart_required(),
        "valid remote competition is not corruption"
    );
}
#[test]
fn native_coordinator_successor_fails_closed_on_corrupt_applied_native_receipt() {
    let (mut adapter, _, lane_id, dataspace_id, previous) =
        native_coordinator_after_applied_participant_fixture();
    let request = native_coordinator_successor_request(&adapter, lane_id, dataspace_id, &previous);
    let receipt_path = adapter
        .state
        .nexus_snapshot()
        .lane_config
        .entry(lane_id)
        .expect("actual participant storage route")
        .blocks_dir(adapter.kura.store_root())
        .join("lane_artifacts/native_amx_receipt_v1_00000000000000000001.norito");
    assert!(
        !std::fs::read(&receipt_path)
            .expect("actual durable receipt")
            .is_empty()
    );
    corrupt_durable_file_for_test(&receipt_path);
    let damaged = std::fs::read(&receipt_path).unwrap();
    assert!(!adapter.output_guard.restart_required());
    assert!(matches!(
        adapter.native_coordinator_height_is_current(&request.body),
        Err(V2LaneWorkError::Persistence(_))
    ));
    assert!(adapter.output_guard.restart_required());
    assert!(
        adapter
            .native_coordinator_predecessor_is_current(&request)
            .is_err()
    );
    assert!(adapter.sign_native_request_once(&request, 0).is_none());
    assert_eq!(
        adapter
            .native_signing_guard
            .as_ref()
            .unwrap()
            .record_count_for_test(),
        0
    );
    assert_eq!(std::fs::read(receipt_path).unwrap(), damaged);
}
