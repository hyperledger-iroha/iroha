fn strict_current_lane_owner_fixture() -> (
    V2LaneWorkAdapter,
    Vec<KeyPair>,
    SignedBlock,
    LaneBlockProposalV1,
) {
    let (mut adapter, keys) = fixture_at_height_inner(wire::ConsensusMode::Permissioned, 2, true);
    let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    mark_global_body_locked_for_block(&mut adapter, &block);
    assert_ne!(
        adapter.bind_locked_global_body(&block),
        V2LaneIngressOutcome::Rejected
    );
    adapter
        .kura
        .store_block(block.clone())
        .expect("persist the protected carrier");
    let finality = verified_finality_artifact_for_block(&adapter, &keys, &block);
    adapter
        .kura
        .store_v2_finality_artifact(&finality)
        .expect("publish complete-wire carrier authority");
    assert!(
        adapter
            .proposal_body_available(&proposal)
            .expect("authenticate initial body")
    );
    (adapter, keys, block, proposal)
}

#[test]
fn corrupted_raw_anchor_retains_session_instead_of_using_local_body_hint() {
    let (mut adapter, keys, _, proposal) = strict_current_lane_owner_fixture();
    assert_eq!(
        adapter
            .locally_bound_lane_proposals
            .get(&proposal.proposal_hash),
        proposal.payload_block_hint.as_ref()
    );
    let proposals_before = adapter.lane_sessions.rollover_proposal_hashes();
    let qcs_before = adapter.lane_sessions.qcs_for_incomplete_sessions();
    let source = adapter
        .state
        .nexus_snapshot()
        .lane_config
        .entry(proposal.descriptor.lane_id)
        .expect("configured lane")
        .blocks_dir(adapter.kura.store_root());
    corrupt_durable_file_for_test(&source.join("lane_artifacts/ownerships.norito"));
    let certificate = LaneBlockCertificateV1 {
        proposal: proposal.clone(),
        prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
        commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
    };
    let inbound = fair_v2_ingress_admit_for_test(InboundBlockMessage::from_authenticated_peer(
        BlockMessage::LaneBlockCertificate(Box::new(certificate)),
        PeerId::new(keys[1].public_key().clone()),
    ));
    assert_eq!(
        adapter.accept_lane_message_with_ingress_ownership(inbound, 0),
        Err(V2LaneWorkError::RestartRequired)
    );
    assert_eq!(
        adapter.lane_sessions.rollover_proposal_hashes(),
        proposals_before
    );
    assert_eq!(
        adapter.lane_sessions.qcs_for_incomplete_sessions(),
        qcs_before
    );
    assert!(adapter.output_guard.restart_required());
}

#[test]
fn corrupted_decided_carrier_retains_exact_durable_certificate_reply_owner() {
    let (mut adapter, keys, block, proposal) = strict_current_lane_owner_fixture();
    let session = committed_lane_session(&proposal, &keys);
    adapter
        .kura
        .persist_committed_lane_block_session(&session, &adapter.pops_for_lane_session(&session))
        .expect("persist the exact certificate response source");
    let (round, decided) = global_lock_for_block(&adapter, &block);
    adapter
        .retain_merge_sidecars_for_global_view(round.view, Some(decided), Some(decided))
        .expect("install the authenticated decided subject");
    adapter.drain_effects(usize::MAX);
    let requester = session
        .commit_qc
        .validator_set
        .iter()
        .find(|peer| *peer != &adapter.local_peer)
        .cloned()
        .expect("remote certificate requester");
    let relay = PeerId::new(KeyPair::random().public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::new(relay.clone());
    let route = routes.mint(requester.clone());
    let inbound = fair_v2_ingress_admit_for_test(
        InboundBlockMessage::try_from_transport_with_reply_route(
            BlockMessage::LaneBlockProposal(proposal.clone()),
            requester,
            relay,
            route.clone(),
        )
        .expect("live exact reply route"),
    );
    assert_eq!(
        adapter
            .accept_lane_message_with_ingress_ownership(inbound, 0)
            .expect("admit certificate request"),
        V2LaneIngressOutcome::Inserted
    );
    assert!(adapter.effects.iter().any(|effect| matches!(effect,
        V2LaneWorkEffect::PostDurableLaneCertificate { ingress_ownership: Some(owner), reply_routes: Some(routes), .. }
            if owner.validate_exact() && routes.iter().any(|candidate| candidate.same_delivery(&route))
    )));
    let effect_keys = adapter
        .effects
        .iter()
        .map(lane_work_effect_key)
        .collect::<Vec<_>>();
    // Evict only optional volatile body witnesses; the actual decided subject,
    // durable certificate, exact request route, and fair-ingress owner remain.
    adapter.globally_locked_body = None;
    adapter.locally_bound_lane_proposals.clear();
    assert!(
        adapter
            .proposal_is_bound_to_decided_carrier(&proposal)
            .expect("authenticate durable decided binding")
    );
    let blocks = adapter
        .state
        .nexus_snapshot()
        .lane_config
        .primary()
        .blocks_dir(adapter.kura.store_root());
    corrupt_durable_file_for_test(&blocks.join("blocks.data"));
    assert!(
        adapter
            .purge_queued_global_body_effects_except_committed_outputs()
            .is_err()
    );
    assert_eq!(
        adapter
            .effects
            .iter()
            .map(lane_work_effect_key)
            .collect::<Vec<_>>(),
        effect_keys
    );
    assert!(adapter.effects.iter().any(|effect| matches!(effect,
        V2LaneWorkEffect::PostDurableLaneCertificate { ingress_ownership: Some(owner), reply_routes: Some(routes), .. }
            if owner.validate_exact() && routes.iter().any(|candidate| candidate.same_delivery(&route))
    )));
    assert!(adapter.output_guard.restart_required());
}

#[test]
fn corrupted_decided_carrier_retains_autonomous_new_view_clock() {
    let (mut adapter, keys) = fixture_at_height_inner(wire::ConsensusMode::Permissioned, 2, true);
    let (source, mut proposal) =
        planned_autonomous_lane_candidate_block_at_view(&adapter, &keys, 0);
    proposal.payload_block_hint = None;
    let entrypoint = source
        .external_entrypoints_cloned()
        .next()
        .expect("autonomous entrypoint");
    let (payload, producer) = signed_autonomous_payload_for_entrypoint(
        &adapter,
        &keys,
        &proposal,
        entrypoint,
        b"strict-clock-admission",
        b"strict-clock-reservation",
        "producer",
        "producer key",
        "signed autonomous payload",
    );
    assert_eq!(
        accept_lane_message_from(
            &mut adapter,
            BlockMessage::LaneExecutablePayload(payload.clone()),
            producer,
            0
        ),
        V2LaneIngressOutcome::Inserted
    );
    let carrier = autonomous_carrier_block(&adapter, &keys, &payload);
    let (round, decided) = mark_global_body_locked_for_block(&mut adapter, &carrier);
    assert_ne!(
        adapter.bind_locked_global_body(&carrier.canonical_resultless_proposal()),
        V2LaneIngressOutcome::Rejected
    );
    adapter
        .kura
        .store_block(carrier.clone())
        .expect("persist autonomous carrier");
    let finality = verified_finality_artifact_for_block(&adapter, &keys, &carrier);
    adapter
        .kura
        .store_v2_finality_artifact(&finality)
        .expect("publish autonomous complete-wire authority");
    adapter
        .retain_merge_sidecars_for_global_view(round.view, Some(decided), Some(decided))
        .expect("protect the decided autonomous owner");
    assert!(
        !adapter.autonomous_new_view_started_at.is_empty(),
        "the production bind owns a live clock"
    );
    let clocks = adapter.autonomous_new_view_started_at.clone();
    let payloads = adapter.autonomous_payloads.clone();
    adapter.globally_locked_body = None;
    adapter.locally_bound_lane_proposals.clear();
    let blocks = adapter
        .state
        .nexus_snapshot()
        .lane_config
        .primary()
        .blocks_dir(adapter.kura.store_root());
    corrupt_durable_file_for_test(&blocks.join("blocks.data"));
    assert!(
        adapter
            .schedule_autonomous_new_view_timeouts(
                Instant::now(),
                round.view,
                Duration::from_secs(1)
            )
            .is_err()
    );
    assert_eq!(adapter.autonomous_new_view_started_at, clocks);
    assert_eq!(adapter.autonomous_payloads, payloads);
    assert!(adapter.output_guard.restart_required());
}

#[test]
fn corrupted_progress_authority_retains_committed_output_owner_and_cursor() {
    for effect_preflight in [false, true] {
        let mut fixture = nonmember_canonical_replica_pre_qc_fixture();
        let payload = attach_finalized_nonmember_public_payload(
            &fixture.adapter,
            &fixture.proposal,
            fixture.payload.clone(),
            &fixture.locked_round,
            &fixture.decided,
        );
        let proposal = payload.origin_proposal.clone();
        let votes = fixture.lane_keys[..3]
            .iter()
            .map(|key| signed_autonomous_prepare_vote(&proposal, &payload, key, &fixture.lane_keys))
            .collect::<Vec<_>>();
        let prepare_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
            proposal.vote_body(CertPhase::Prepare),
            proposal.descriptor.validator_set.clone(),
            &votes,
        )
        .expect("actual committee READY quorum");
        let certificate = LaneBlockCertificateV1 {
            proposal: proposal.clone(),
            prepare_qc,
            commit_qc: lane_qc_for_phase(&proposal, &fixture.lane_keys[..3], CertPhase::Commit),
        };
        assert_eq!(
            accept_lane_message_from(
                &mut fixture.adapter,
                BlockMessage::LaneBlockCertificate(Box::new(certificate)),
                PeerId::new(fixture.global_keys[0].public_key().clone()),
                fixture.locked_round.view,
            ),
            V2LaneIngressOutcome::Inserted
        );
        let adapter = &mut fixture.adapter;
        assert!(
            adapter
                .proposal_predecessor_is_ready_for_progress(&proposal)
                .expect("authenticate the output's initial progress authority")
        );
        assert!(
            adapter
                .committed_lane_outputs
                .iter()
                .any(|output| output.session.proposal == proposal),
            "production certificate admission must transfer the exact output owner"
        );
        let retry = adapter
            .effects
            .iter()
            .find(|effect| matches!(effect,
                V2LaneWorkEffect::PostLaneBlock { message: BlockMessage::LaneBlockQc(qc), .. }
                    if qc.body.phase == CertPhase::Commit && qc.body.proposal_hash == proposal.proposal_hash
            ))
            .expect("production scheduling retains an exact CommitQC occurrence")
            .clone();
        let outputs = adapter
            .committed_lane_outputs
            .iter()
            .map(|output| (output.session.clone(), output.next_validator))
            .collect::<Vec<_>>();
        let cursor = adapter.committed_lane_output_cursor;
        let queued = adapter
            .effects
            .iter()
            .map(lane_work_effect_key)
            .collect::<Vec<_>>();
        let blocks = adapter
            .state
            .nexus_snapshot()
            .lane_config
            .primary()
            .blocks_dir(adapter.kura.store_root());
        corrupt_durable_file_for_test(&blocks.join("blocks.data"));
        if effect_preflight {
            assert!(matches!(
                adapter.preflight_effect_insertion(&retry),
                Err(LaneWorkEffectInsertionOutcome::Rejected)
            ));
        } else {
            assert!(adapter.schedule_committed_lane_outputs().is_err());
        }
        assert!(adapter.output_guard.restart_required());
        assert_eq!(adapter.committed_lane_output_cursor, cursor);
        assert_eq!(
            adapter
                .committed_lane_outputs
                .iter()
                .map(|output| (output.session.clone(), output.next_validator))
                .collect::<Vec<_>>(),
            outputs
        );
        assert_eq!(
            adapter
                .effects
                .iter()
                .map(lane_work_effect_key)
                .collect::<Vec<_>>(),
            queued
        );
        assert_eq!(
            adapter.effect_keys,
            queued.into_iter().collect::<BTreeSet<_>>()
        );
    }
}

#[test]
fn late_durable_certificate_read_failure_retains_exact_runner_source() {
    let (mut adapter, keys, _, proposal) = strict_current_lane_owner_fixture();
    let session = committed_lane_session(&proposal, &keys);
    adapter
        .kura
        .persist_committed_lane_block_session(&session, &adapter.pops_for_lane_session(&session))
        .expect("publish the exact certified response source");
    adapter.drain_effects(usize::MAX);
    let requester = session
        .commit_qc
        .validator_set
        .iter()
        .find(|peer| *peer != &adapter.local_peer)
        .cloned()
        .expect("remote requester");
    let relay = PeerId::new(KeyPair::random().public_key().clone());
    let mut route_fixture = NetworkReplyRouteTestFixture::new(relay.clone());
    let route = route_fixture.mint(requester.clone());
    let inbound = fair_v2_ingress_admit_for_test(
        InboundBlockMessage::try_from_transport_with_reply_route(
            BlockMessage::LaneBlockProposal(proposal.clone()),
            requester,
            relay,
            route.clone(),
        )
        .expect("exact live request route"),
    );
    assert_eq!(
        adapter
            .accept_lane_message_with_ingress_ownership(inbound, 0)
            .expect("retain the production response occurrence"),
        V2LaneIngressOutcome::Inserted
    );
    adapter
        .effects
        .retain(|effect| matches!(effect, V2LaneWorkEffect::PostDurableLaneCertificate { .. }));
    adapter.effect_keys = adapter.effects.iter().map(lane_work_effect_key).collect();
    assert_eq!(adapter.effects.len(), 1);
    let source = adapter
        .effects
        .front()
        .expect("one actual certificate response");
    let V2LaneWorkEffect::PostDurableLaneCertificate {
        ingress_ownership: Some(original_owner),
        ..
    } = source
    else {
        panic!("production response must retain fair-ingress ownership");
    };
    let original_identity = (
        original_owner.first.physical_admission_ordinal,
        original_owner.latest.physical_admission_ordinal,
        original_owner.admission_count,
        original_owner.occurrence_count,
        original_owner.attempts_hash,
    );
    let local_validator = adapter
        .context
        .roster
        .iter()
        .position(|entry| entry.validator == adapter.local_peer)
        .and_then(|index| wire::ValidatorIndex::try_from(index).ok())
        .expect("local context member");
    let services = service_for_history_context_with_local_validator(
        Arc::clone(&adapter.kura),
        adapter.context.clone(),
        &keys,
        local_validator,
    );
    adapter.output_guard = services.lifecycle_output_guard();
    let blocks = adapter
        .state
        .nexus_snapshot()
        .lane_config
        .entry(proposal.descriptor.lane_id)
        .expect("active source lane")
        .blocks_dir(adapter.kura.store_root());
    corrupt_durable_file_for_test(&blocks.join("lane_artifacts/certified_blocks.norito"));
    assert!(
        services
            .can_retain_lane_work_effect_from_snapshot(source, None)
            .expect("capacity preflight does not consume or authenticate the durable source")
    );
    let keys_before = adapter.effect_keys.clone();
    assert!(
        crate::sumeragi::v2_runner::dispatch_lane_work_effects(&mut adapter, &services, 1).is_err()
    );
    assert!(adapter.output_guard.restart_required());
    assert_eq!(adapter.effects.len(), 1);
    assert_eq!(adapter.effect_keys, keys_before);
    let V2LaneWorkEffect::PostDurableLaneCertificate {
        ingress_ownership: Some(owner),
        reply_routes: Some(routes),
        ..
    } = adapter
        .effects
        .front()
        .expect("late failure retains original response")
    else {
        panic!("late failure must retain both physical owners");
    };
    assert_eq!(
        (
            owner.first.physical_admission_ordinal,
            owner.latest.physical_admission_ordinal,
            owner.admission_count,
            owner.occurrence_count,
            owner.attempts_hash,
        ),
        original_identity
    );
    assert!(owner.validate_exact() && owner.matches_reply_routes(Some(routes)));
    assert!(routes.iter().any(|retained| retained.same_delivery(&route)));
    assert!(
        !services
            .has_pending_exact_output()
            .expect("source validation failed before worker ownership")
    );
}

#[test]
fn evicted_session_corrupt_raw_anchor_preserves_every_committed_output_cursor() {
    let second_lane = LaneId::new(1);
    let second_dataspace = DataSpaceId::new(7);
    let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
    enable_multilane_nexus(&mut adapter, &keys, second_lane, second_dataspace);
    let (first_block, first_proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    let (_, second_proposal) = planned_lane_candidate_block_for_route_at_view(
        &adapter,
        &keys,
        0,
        second_lane,
        second_dataspace,
    );
    adapter
        .kura
        .store_block(first_block)
        .expect("publish the actual ordinary raw ownership slot");
    adapter.lane_sessions = LaneBlockSessionCache::new(1);
    let first_session = committed_lane_session(&first_proposal, &keys);
    let second_session = committed_lane_session(&second_proposal, &keys);
    for session in [&first_session, &second_session] {
        adapter
            .lane_sessions
            .insert_proposal(session.proposal.clone())
            .expect("admit exact proposal");
        let pops = adapter.pops_for_lane_session(session);
        adapter
            .lane_sessions
            .insert_qc_with_pops(session.prepare_qc.clone(), &pops)
            .expect("verify actual PrepareQC");
        adapter
            .lane_sessions
            .insert_qc_with_pops(session.commit_qc.clone(), &pops)
            .expect("verify actual CommitQC");
        let drained = adapter.lane_sessions.drain_committed_sessions();
        assert_eq!(drained.as_slice(), std::slice::from_ref(session));
        adapter
            .committed_lane_outputs
            .push_back(PendingCommittedLaneOutput {
                session: drained
                    .into_iter()
                    .next()
                    .expect("one authenticated completed source"),
                next_validator: 0,
            });
    }
    assert!(
        adapter
            .lane_sessions
            .proposal_for_vote_body(&first_session.commit_qc.body)
            .is_none()
    );
    assert!(adapter.local_can_own_autonomous_payload(&first_proposal));
    adapter.effects.clear();
    adapter.effect_keys.clear();
    let outputs = adapter
        .committed_lane_outputs
        .iter()
        .map(|output| (output.session.clone(), output.next_validator))
        .collect::<Vec<_>>();
    let cursor = adapter.committed_lane_output_cursor;
    let blocks = adapter
        .state
        .nexus_snapshot()
        .lane_config
        .entry(first_proposal.descriptor.lane_id)
        .expect("active ordinary lane")
        .blocks_dir(adapter.kura.store_root());
    corrupt_durable_file_for_test(&blocks.join("lane_artifacts/ownerships.norito"));
    assert!(
        adapter
            .proposal_predecessor_is_ready_for_progress(&first_proposal)
            .expect("ordinary economic predecessor alone does not inspect the corrupt raw owner")
    );
    assert!(adapter.schedule_committed_lane_outputs().is_err());
    assert!(adapter.output_guard.restart_required());
    assert_eq!(adapter.committed_lane_output_cursor, cursor);
    assert_eq!(
        adapter
            .committed_lane_outputs
            .iter()
            .map(|output| (output.session.clone(), output.next_validator))
            .collect::<Vec<_>>(),
        outputs
    );
    assert!(adapter.effects.is_empty() && adapter.effect_keys.is_empty());
}

#[test]
fn corrupt_predecessor_receipt_preserves_autonomous_new_view_clock_and_payload_owner() {
    let (parent, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
    let (parent_block, predecessor) = globally_anchored_lane_block_fixture(&parent, &keys);
    parent
        .kura
        .store_block(parent_block.clone())
        .expect("persist the executed predecessor");
    let finality = verified_finality_artifact_for_block(&parent, &keys, &parent_block);
    parent
        .kura
        .store_v2_finality_artifact(&finality)
        .expect("publish exact predecessor wire authority");
    let session = committed_lane_session(&predecessor, &keys);
    parent
        .kura
        .persist_committed_lane_block_session(&session, &parent.pops_for_lane_session(&session))
        .expect("publish the exact predecessor certificate");
    parent
        .kura
        .persist_lane_block_application_receipt(&predecessor)
        .expect("publish the canonical Current predecessor receipt");
    let committed = ValidBlock::committed_from_replay_signed_block(parent_block.clone());
    commit_test_block_to_state(parent.state.as_ref(), &committed, &parent.context);
    let context = successor_context_for_parent(&parent, &parent_block, &keys);
    let restart = LaneAdapterRestartParts::capture(&parent);
    drop(parent);
    let mut adapter = restart
        .reopen(context, true)
        .expect("open the actual successor context");
    let (source, mut proposal) =
        planned_autonomous_lane_candidate_block_at_view(&adapter, &keys, 0);
    proposal.payload_block_hint = None;
    assert_eq!(proposal.descriptor.previous_lane_block_height, 1);
    assert_eq!(
        proposal.descriptor.previous_lane_block_descriptor_hash,
        Some(predecessor.descriptor.descriptor_hash)
    );
    assert!(
        adapter
            .state
            .world
            .smart_contract_state
            .view()
            .iter()
            .all(|(key, _)| !key.to_string().starts_with("merge_lane_frontier_v1_")),
        "the replicated frontier must remain behind this ordinary predecessor"
    );
    assert!(
        adapter
            .state
            .certified_autonomous_lane_block_predecessor_is_globally_applied(&proposal)
            .expect("the real Current receipt independently authorizes the autonomous successor")
    );
    let entrypoint = source
        .external_entrypoints_cloned()
        .next()
        .expect("autonomous successor input");
    let (payload, producer) = signed_autonomous_payload_for_entrypoint(
        &adapter,
        &keys,
        &proposal,
        entrypoint,
        b"strict-predecessor-clock-admission",
        b"strict-predecessor-clock-reservation",
        "autonomous successor producer",
        "producer key",
        "exact autonomous successor payload",
    );
    assert_eq!(
        accept_lane_message_from(
            &mut adapter,
            BlockMessage::LaneExecutablePayload(payload.clone()),
            producer,
            0
        ),
        V2LaneIngressOutcome::Inserted
    );
    let carrier = autonomous_carrier_block(&adapter, &keys, &payload);
    let (round, _) = mark_global_body_locked_for_block(&mut adapter, &carrier);
    assert_ne!(
        adapter.bind_locked_global_body(&carrier.canonical_resultless_proposal()),
        V2LaneIngressOutcome::Rejected
    );
    assert!(!adapter.decision_pending());
    assert!(
        !adapter.autonomous_new_view_started_at.is_empty(),
        "production protected-carrier binding owns the clock"
    );
    let anchored = adapter
        .autonomous_payloads
        .values()
        .next()
        .expect("protected autonomous owner")
        .origin_proposal
        .clone();
    let clocks = adapter.autonomous_new_view_started_at.clone();
    let payloads = adapter.autonomous_payloads.clone();
    let views = adapter.autonomous_payload_views.clone();
    let proposals = adapter.lane_sessions.rollover_proposal_hashes();
    let qcs = adapter.lane_sessions.qcs_for_incomplete_sessions();
    let effects = adapter
        .effects
        .iter()
        .map(lane_work_effect_key)
        .collect::<Vec<_>>();
    let receipts = adapter
        .state
        .nexus_snapshot()
        .lane_config
        .entry(predecessor.descriptor.lane_id)
        .expect("actual predecessor lane")
        .blocks_dir(adapter.kura.store_root())
        .join("lane_artifacts");
    let data_path = receipts.join("application_receipts.norito");
    let index_path = receipts.join("application_receipts.index");
    let mut damaged =
        std::fs::read(&data_path).expect("read the actual occupied predecessor receipt");
    let index = std::fs::read(&index_path).expect("read the unchanged receipt index");
    assert!(!damaged.is_empty());
    damaged[0] ^= 0xff;
    std::fs::write(&data_path, &damaged)
        .expect("damage only the exact predecessor record while preserving file geometry");
    assert!(
        !adapter
            .lane_application_receipt_available(&anchored)
            .expect("the absent successor slot remains provably absent")
    );
    assert!(
        !adapter.output_guard.restart_required(),
        "the existing applied-slot preflight does not detect the predecessor fault"
    );
    assert!(matches!(
        adapter.schedule_autonomous_new_view_timeouts(
            Instant::now(),
            round.view,
            Duration::from_secs(1)
        ),
        Err(V2LaneWorkError::Persistence(_))
    ));
    assert!(adapter.output_guard.restart_required());
    assert_eq!(adapter.autonomous_new_view_started_at, clocks);
    assert_eq!(adapter.autonomous_payloads, payloads);
    assert_eq!(adapter.autonomous_payload_views, views);
    assert_eq!(adapter.lane_sessions.rollover_proposal_hashes(), proposals);
    assert_eq!(adapter.lane_sessions.qcs_for_incomplete_sessions(), qcs);
    assert_eq!(
        adapter
            .effects
            .iter()
            .map(lane_work_effect_key)
            .collect::<Vec<_>>(),
        effects
    );
    assert_eq!(std::fs::read(data_path).unwrap(), damaged);
    assert_eq!(std::fs::read(index_path).unwrap(), index);
}

#[test]
fn corrupt_historical_application_anchor_preserves_pending_committed_cache_owner() {
    let (parent, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
    let (block, proposal) = globally_anchored_lane_block_fixture(&parent, &keys);
    parent
        .kura
        .store_block(block.clone())
        .expect("store the exact historical carrier");
    let finality = verified_finality_artifact_for_block(&parent, &keys, &block);
    parent
        .kura
        .store_v2_finality_artifact(&finality)
        .expect("publish full-wire historical finality");
    let committed = ValidBlock::committed_from_replay_signed_block(block.clone());
    commit_test_block_to_state(parent.state.as_ref(), &committed, &parent.context);
    let context = successor_context_for_parent(&parent, &block, &keys);
    let restart = LaneAdapterRestartParts::capture(&parent);
    drop(parent);
    let mut adapter = restart
        .reopen(context, true)
        .expect("open the canonical successor");
    let session = committed_lane_session(&proposal, &keys);
    let pops = adapter.pops_for_lane_session(&session);
    adapter
        .lane_sessions
        .insert_recovered_proposal_replacing_uncommitted_conflict(proposal.clone())
        .expect("retain the exact historical proposal");
    adapter
        .lane_sessions
        .insert_qc_with_pops(session.prepare_qc.clone(), &pops)
        .expect("retain the exact PrepareQC");
    adapter
        .lane_sessions
        .insert_qc_with_pops(session.commit_qc.clone(), &pops)
        .expect("retain the exact CommitQC");
    assert_eq!(
        adapter.lane_sessions.pending_committed_sessions(),
        vec![session.clone()]
    );
    assert!(
        !adapter
            .state
            .certified_lane_block_session_is_applied_or_snapshot_anchored(&session)
            .expect("canonical metadata alone cannot impersonate lane application")
    );
    assert!(
        !adapter
            .proposal_can_progress(&proposal)
            .expect("ordinary historical work is outside live signing progression")
    );
    let cache_before = adapter.lane_sessions.clone();
    let pending_before = adapter.pending_committed_lanes.clone();
    let historical_before = adapter.historical_recovery_sessions.clone();
    let outputs_before = adapter
        .committed_lane_outputs
        .iter()
        .map(|output| (output.session.clone(), output.next_validator))
        .collect::<Vec<_>>();
    let effects_before = adapter
        .effects
        .iter()
        .map(lane_work_effect_key)
        .collect::<Vec<_>>();
    let ownerships = adapter
        .state
        .nexus_snapshot()
        .lane_config
        .entry(proposal.descriptor.lane_id)
        .expect("actual historical lane")
        .blocks_dir(adapter.kura.store_root())
        .join("lane_artifacts/ownerships.norito");
    corrupt_durable_file_for_test(&ownerships);
    let damaged = std::fs::read(&ownerships).expect("read the actual damaged raw artifact");
    assert!(
        adapter
            .lane_application_receipt_at_proposal_slot(&proposal)
            .expect("receipt absence is independent of occupied raw ownership damage")
            .is_none()
    );
    assert!(!adapter.output_guard.restart_required());
    assert!(matches!(
        adapter.collect_committed_lane_sessions(),
        Err(V2LaneWorkError::Persistence(_))
    ));
    assert!(adapter.output_guard.restart_required());
    assert_eq!(
        adapter.lane_sessions, cache_before,
        "the pending drain bit and exact quorum owners must survive"
    );
    assert_eq!(
        adapter.lane_sessions.pending_committed_sessions(),
        vec![session]
    );
    assert_eq!(adapter.pending_committed_lanes, pending_before);
    assert_eq!(adapter.historical_recovery_sessions, historical_before);
    assert_eq!(
        adapter
            .committed_lane_outputs
            .iter()
            .map(|output| (output.session.clone(), output.next_validator))
            .collect::<Vec<_>>(),
        outputs_before
    );
    assert_eq!(
        adapter
            .effects
            .iter()
            .map(lane_work_effect_key)
            .collect::<Vec<_>>(),
        effects_before
    );
    assert_eq!(std::fs::read(ownerships).unwrap(), damaged);
}

#[test]
fn corrupt_planner_frontier_retains_exact_pending_producer_reservations() {
    let (mut adapter, keys) = autonomous_test_fixture(wire::ConsensusMode::Permissioned, true);
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    assert_autonomous_test_role(&adapter, &keys, lane_id, dataspace_id, true);
    let directory = tempfile::tempdir().expect("real producer reservation journals");
    let journal_path = directory.path().join("lane-reservations.norito");
    let queue = install_autonomous_test_queue(&mut adapter, lane_id, dataspace_id, &journal_path);
    enqueue_autonomous_test_transactions(&adapter, &queue, lane_id, dataspace_id, 1);
    let slot = plan_autonomous_lane_reservation_slot(
        adapter.state.as_ref(),
        adapter.kura.as_ref(),
        &adapter.context,
        lane_id,
        dataspace_id,
    )
    .expect("derive an actual eligible producer slot");
    let reservations = queue
        .reserve_transactions_for_lane_bounded(
            adapter.state.as_ref(),
            slot.selection_authorization()
                .expect("exact producer selection authority"),
            LaneQueueReservationSelectionLimits {
                max_transactions: NonZeroUsize::new(1).unwrap(),
                max_scan: NonZeroUsize::new(1).unwrap(),
                max_encoded_bytes: NonZeroU64::new(u64::MAX).unwrap(),
                max_gas: NonZeroU64::new(u64::MAX).unwrap(),
            },
            &BTreeSet::new(),
            LaneQueueReservationRoutingMode::AnyCoordinatorPlan,
        )
        .expect("durably reserve the real selected queue entry");
    assert_eq!(reservations.len(), 1);
    let batch = PendingAutonomousReservationBatch {
        slot,
        reservations,
        envelope_byte_limit: 4 * 1024 * 1024,
    };
    let original_allocation = batch.reservations.as_ptr();
    let ordered_keys = batch
        .reservations
        .iter()
        .map(|reservation| *reservation.key())
        .collect::<Vec<_>>();
    adapter
        .pending_autonomous_reservation_batches
        .insert((lane_id, dataspace_id), batch);
    let live_before = queue.live_lane_reservations();
    let fifo_before = queue.fifo_snapshot_for_test();
    let journal_before =
        std::fs::read(&journal_path).expect("read the real live reservation journal");
    let attempted_before = adapter.autonomous_production_attempted_routes.clone();
    let (block, _) =
        planned_lane_candidate_block_for_route_at_view(&adapter, &keys, 0, lane_id, dataspace_id);
    adapter
        .kura
        .store_block(block.clone())
        .expect("publish actual current-height lane ownership");
    let finality = verified_finality_artifact_for_block(&adapter, &keys, &block);
    adapter
        .kura
        .store_v2_finality_artifact(&finality)
        .expect("authenticate the entire stored carrier");
    let ownerships = adapter
        .state
        .nexus_snapshot()
        .lane_config
        .entry(lane_id)
        .expect("actual producer lane")
        .blocks_dir(adapter.kura.store_root())
        .join("lane_artifacts/ownerships.norito");
    corrupt_durable_file_for_test(&ownerships);
    let damaged = std::fs::read(&ownerships).expect("read the damaged occupied planner input");
    adapter.next_autonomous_producer_tick = Instant::now();
    assert!(matches!(
        adapter.schedule_autonomous_lane_production(0, autonomous_test_candidate_limits(2, 2)),
        Err(V2LaneWorkError::Persistence(_))
    ));
    assert!(adapter.output_guard.restart_required());
    let retained = adapter
        .pending_autonomous_reservation_batches
        .get(&(lane_id, dataspace_id))
        .expect("the failed driver must restore its original pending batch");
    assert_eq!(
        retained.reservations.as_ptr(),
        original_allocation,
        "retain the exact move-only batch without cloning"
    );
    assert_eq!(
        retained
            .reservations
            .iter()
            .map(|reservation| *reservation.key())
            .collect::<Vec<_>>(),
        ordered_keys
    );
    assert_eq!(queue.live_lane_reservations(), live_before);
    assert_eq!(queue.fifo_snapshot_for_test(), fifo_before);
    assert_eq!(std::fs::read(journal_path).unwrap(), journal_before);
    assert_eq!(
        adapter.autonomous_production_attempted_routes,
        attempted_before
    );
    assert_eq!(std::fs::read(ownerships).unwrap(), damaged);
}

#[test]
fn corrupt_native_application_receipt_retains_exact_pending_producer_reservations() {
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
    let slot = plan_autonomous_lane_reservation_slot(
        adapter.state.as_ref(),
        adapter.kura.as_ref(),
        &context,
        lane_id,
        dataspace_id,
    )
    .expect("derive successor from actual applied Native receipt");
    let mut restart = LaneAdapterRestartParts::capture(&adapter);
    restart.local_peer = slot.author.clone();
    restart.key_pair = keys
        .iter()
        .find(|key| key.public_key() == slot.author.public_key())
        .expect("actual deterministic producer belongs to four-validator fixture")
        .clone();
    drop(adapter);
    let mut adapter = restart
        .reopen_isolated(context, true)
        .expect("open successor journals under complete Native application authority");
    let directory = tempfile::tempdir().expect("real producer journals");
    let journal_path = directory.path().join("lane-reservations.norito");
    let queue = install_autonomous_test_queue(&mut adapter, lane_id, dataspace_id, &journal_path);
    enqueue_autonomous_test_transactions(&adapter, &queue, lane_id, dataspace_id, 1);
    let reservations = queue
        .reserve_transactions_for_lane_bounded(
            adapter.state.as_ref(),
            slot.selection_authorization()
                .expect("exact selection authority"),
            LaneQueueReservationSelectionLimits {
                max_transactions: NonZeroUsize::new(1).unwrap(),
                max_scan: NonZeroUsize::new(1).unwrap(),
                max_encoded_bytes: NonZeroU64::new(u64::MAX).unwrap(),
                max_gas: NonZeroU64::new(u64::MAX).unwrap(),
            },
            &BTreeSet::new(),
            LaneQueueReservationRoutingMode::AnyCoordinatorPlan,
        )
        .expect("durably reserve exact queue entry");
    assert_eq!(reservations.len(), 1);
    let batch = PendingAutonomousReservationBatch {
        slot,
        reservations,
        envelope_byte_limit: 4 * 1024 * 1024,
    };
    let original_allocation = batch.reservations.as_ptr();
    let ordered_keys = batch
        .reservations
        .iter()
        .map(|reservation| *reservation.key())
        .collect::<Vec<_>>();
    let candidates = batch
        .reservations
        .iter()
        .map(|reservation| Hash::from(reservation.clone_accepted().hash_as_entrypoint()))
        .collect::<Vec<_>>();
    let healthy_plan = prepare_v2_lane_payload_plan(
        adapter.state.as_ref(),
        adapter.kura.as_ref(),
        &adapter.context,
        0,
        &adapter.local_peer,
        &[RoutingDecision::new(lane_id, dataspace_id)],
        &candidates,
    )
    .expect("healthy Native evidence admits this exact producer batch");
    assert!(healthy_plan.unavailable_indices.is_empty());
    assert_eq!(healthy_plan.proposals.len(), 1);
    assert!(
        V2LaneWorkAdapter::autonomous_proposal_matches_reservation_slot(
            &healthy_plan.proposals[0],
            &batch.slot
        )
    );
    adapter
        .pending_autonomous_reservation_batches
        .insert((lane_id, dataspace_id), batch);
    let live_before = queue.live_lane_reservations();
    let fifo_before = queue.fifo_snapshot_for_test();
    let journal_before = std::fs::read(&journal_path).expect("read real live reservation journal");
    let plans_before = std::fs::read(journal_path.with_extension("plans.norito"))
        .expect("read exact admission ownership journal");
    let attempted_before = adapter.autonomous_production_attempted_routes.clone();
    let receipt_path = adapter
        .state
        .nexus_snapshot()
        .lane_config
        .entry(lane_id)
        .expect("actual Native participant storage")
        .blocks_dir(adapter.kura.store_root())
        .join(format!(
            "lane_artifacts/native_amx_receipt_v1_{:020}.norito",
            pending[0].lane_block_height
        ));
    assert!(
        !std::fs::read(&receipt_path)
            .expect("real populated receipt")
            .is_empty()
    );
    corrupt_durable_file_for_test(&receipt_path);
    let damaged = std::fs::read(&receipt_path).expect("read damaged occupied receipt");
    assert!(
        !adapter.output_guard.restart_required(),
        "the actual producer driver must discover the storage failure"
    );
    adapter.next_autonomous_producer_tick = Instant::now();
    assert!(matches!(
        adapter.schedule_autonomous_lane_production(0, autonomous_test_candidate_limits(2, 2)),
        Err(V2LaneWorkError::Persistence(_))
    ));
    assert!(adapter.output_guard.restart_required());
    assert!(
        adapter
            .state
            .native_amx_participant_application_tips_snapshot()
            .is_err()
    );
    assert!(
        adapter
            .state
            .native_amx_participant_frontiers_pending_durable_evidence_snapshot()
            .is_err()
    );
    assert!(
        adapter
            .state
            .unapplied_native_amx_participant_control_heights_snapshot()
            .is_err()
    );
    let plan_error = prepare_v2_lane_payload_plan(
        adapter.state.as_ref(),
        adapter.kura.as_ref(),
        &adapter.context,
        0,
        &adapter.local_peer,
        &[RoutingDecision::new(lane_id, dataspace_id)],
        &candidates,
    )
    .expect_err("unreadable Native authority cannot become ordinary unavailable indices");
    assert!(plan_error.is_storage_error());
    let retained = adapter
        .pending_autonomous_reservation_batches
        .get(&(lane_id, dataspace_id))
        .expect("failed driver retains original pending batch");
    assert_eq!(retained.reservations.as_ptr(), original_allocation);
    assert_eq!(
        retained
            .reservations
            .iter()
            .map(|reservation| *reservation.key())
            .collect::<Vec<_>>(),
        ordered_keys
    );
    assert_eq!(queue.live_lane_reservations(), live_before);
    assert_eq!(queue.fifo_snapshot_for_test(), fifo_before);
    assert_eq!(std::fs::read(&journal_path).unwrap(), journal_before);
    assert_eq!(
        std::fs::read(journal_path.with_extension("plans.norito")).unwrap(),
        plans_before
    );
    assert_eq!(
        adapter.autonomous_production_attempted_routes,
        attempted_before
    );
    assert_eq!(std::fs::read(receipt_path).unwrap(), damaged);
}
