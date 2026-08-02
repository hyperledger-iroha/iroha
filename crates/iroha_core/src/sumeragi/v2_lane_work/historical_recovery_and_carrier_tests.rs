#[test]
fn historical_certificate_payload_corruption_is_fail_stop_and_retains_owner() {
    let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
    let lane_incarnation = adapter
        .state
        .lane_incarnation_at_height(LaneId::SINGLE, 1)
        .expect("default lane incarnation");
    let proposal = proposal_for_route(
        &adapter,
        &keys,
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
        lane_incarnation,
        1,
        1,
    );
    let proposal = store_canonical_anchor(&adapter, &proposal, &keys[0]);
    let parent_block = adapter
        .kura
        .get_block(NonZeroUsize::new(1).expect("non-zero parent height"))
        .expect("canonical corrupt-payload carrier")
        .as_ref()
        .clone();
    let committed_parent = ValidBlock::committed_from_replay_signed_block(parent_block.clone());
    commit_test_block_to_state(adapter.state.as_ref(), &committed_parent, &adapter.context);
    let certificate = LaneBlockCertificateV1 {
        proposal: proposal.clone(),
        prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
        commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
    };
    let successor_context = successor_context_for_parent(&adapter, &parent_block);
    let local_peer = adapter.local_peer.clone();
    let local_key = adapter.key_pair.clone();
    let state = Arc::clone(&adapter.state);
    let kura = Arc::clone(&adapter.kura);
    let limits = adapter.limits;
    drop(adapter);
    let mut successor = V2LaneWorkAdapter::new(
        successor_context,
        local_peer,
        local_key,
        true,
        state,
        kura,
        limits,
        None,
    )
    .expect("open successor before the historical certificate arrives");
    assert_eq!(
        successor.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneBlockCertificate(Box::new(certificate)),
                Some(PeerId::new(keys[0].public_key().clone())),
            ),
            0,
        ),
        V2LaneIngressOutcome::Inserted
    );

    let error = successor
        .service_next_historical_recovery()
        .expect_err("an immutable missing entrypoint must fail closed");
    assert!(
        error.to_string().contains("MissingEntrypoint"),
        "unexpected historical corruption error: {error}"
    );
    assert_eq!(
        successor.historical_recovery_sessions.len(),
        1,
        "fail-stop diagnosis must retain the exact recovery owner"
    );
    assert!(successor.output_guard.restart_required());
}

#[test]
fn committed_output_source_remains_hard_bounded_after_persistence() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
    assert_eq!(
        adapter.mark_global_body_locked(locked_round, locked_subject),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_ne!(
        adapter.bind_locked_global_body(&block),
        V2LaneIngressOutcome::Rejected
    );
    let _ = adapter.drain_effects(usize::MAX);

    let prepare_qc = lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare);
    let commit_qc = lane_qc_for_phase(&proposal, &keys, CertPhase::Commit);
    let completed = CommittedLaneBlockSession {
        proposal: proposal.clone(),
        prepare_qc: prepare_qc.clone(),
        commit_qc: commit_qc.clone(),
    };
    adapter.limits.session_capacity = NonZeroUsize::new(1).expect("non-zero capacity");
    adapter
        .committed_lane_outputs
        .push_back(PendingCommittedLaneOutput {
            next_validator: commit_qc.validator_set.len(),
            session: completed,
        });

    assert_eq!(
        adapter.insert_lane_qc(prepare_qc, locked_round.view),
        V2LaneIngressOutcome::Inserted
    );
    assert_eq!(
        adapter.insert_lane_qc(commit_qc, locked_round.view),
        V2LaneIngressOutcome::Inserted
    );
    adapter.drive_lane_sessions();
    assert_eq!(adapter.committed_lane_outputs.len(), 1);
    assert!(
        adapter.pending_committed_lanes.is_empty(),
        "persisting the first source must not free its bounded reconstruction slot"
    );

    adapter.committed_lane_outputs.clear();
    adapter.collect_committed_lane_sessions();
    assert_eq!(adapter.committed_lane_outputs.len(), 1);
    assert_eq!(adapter.pending_committed_lanes.len(), 1);
}

#[test]
fn carrier_replacement_filters_persistence_and_output_sources_together() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let (losing_block, losing_proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    let (winning_block, winning_proposal) =
        planned_lane_candidate_block_at_view(&adapter, &keys, 1);
    let (_, winning_subject) = global_lock_for_block(&adapter, &winning_block);
    assert_ne!(
        global_lock_for_block(&adapter, &losing_block).1,
        winning_subject,
        "carrier replacement fixture must use distinct global subjects"
    );

    let sessions = [losing_proposal, winning_proposal].map(|proposal| CommittedLaneBlockSession {
        prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
        commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
        proposal,
    });
    adapter.limits.session_capacity = NonZeroUsize::new(1).expect("non-zero capacity");
    for session in &sessions {
        adapter.pending_committed_lanes.push_back(session.clone());
        adapter
            .committed_lane_outputs
            .push_back(PendingCommittedLaneOutput {
                next_validator: session.commit_qc.validator_set.len(),
                session: session.clone(),
            });
    }

    adapter.retain_committed_lane_outputs_for_subject(winning_subject);
    assert_eq!(adapter.pending_committed_lanes.len(), 1);
    assert_eq!(adapter.committed_lane_outputs.len(), 1);
    assert!(adapter.pending_committed_lanes.iter().all(|session| {
        session
            .proposal
            .payload_block_hint
            .as_ref()
            .is_some_and(|hint| hint.proposal_block_hash == winning_subject.block_hash)
    }));
    assert!(adapter.committed_lane_outputs.iter().all(|output| {
        output
            .session
            .proposal
            .payload_block_hint
            .as_ref()
            .is_some_and(|hint| hint.proposal_block_hash == winning_subject.block_hash)
    }));

    adapter.pending_committed_lanes.clear();
    adapter
        .committed_lane_outputs
        .push_back(PendingCommittedLaneOutput {
            next_validator: sessions[0].commit_qc.validator_set.len(),
            session: sessions[0].clone(),
        });
    let revision_before_output_only_prune = adapter.committed_lane_status_revision;
    adapter.retain_committed_lane_outputs_for_subject(winning_subject);
    assert_ne!(
        adapter.committed_lane_status_revision, revision_before_output_only_prune,
        "removing only an output-owner copy must invalidate the status projection"
    );
}

#[test]
fn completed_commit_qc_round_robin_does_not_restart_ahead_of_pending_source() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let (_, first_proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    let (_, second_proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 1);
    let first_session = CommittedLaneBlockSession {
        prepare_qc: lane_qc_for_phase(&first_proposal, &keys, CertPhase::Prepare),
        commit_qc: lane_qc_for_phase(&first_proposal, &keys, CertPhase::Commit),
        proposal: first_proposal.clone(),
    };
    let second_session = CommittedLaneBlockSession {
        prepare_qc: lane_qc_for_phase(&second_proposal, &keys, CertPhase::Prepare),
        commit_qc: lane_qc_for_phase(&second_proposal, &keys, CertPhase::Commit),
        proposal: second_proposal.clone(),
    };
    adapter.effects.clear();
    adapter.effect_keys.clear();
    adapter.limits.effect_capacity = NonZeroUsize::new(1).expect("non-zero capacity");
    adapter
        .committed_lane_outputs
        .push_back(PendingCommittedLaneOutput {
            next_validator: first_session.commit_qc.validator_set.len(),
            session: first_session,
        });
    adapter
        .committed_lane_outputs
        .push_back(PendingCommittedLaneOutput {
            next_validator: 0,
            session: second_session,
        });

    adapter
        .schedule_lane_artifact_retransmissions()
        .expect("lane artifact retransmission should remain authorized");
    let effect = adapter
        .drain_effects(1)
        .pop()
        .expect("pending source must receive the only effect slot");
    assert!(matches!(
        effect,
        V2LaneWorkEffect::PostLaneBlock {
            message: BlockMessage::LaneBlockQc(qc),
            ..
        } if qc.body.phase == CertPhase::Commit
            && qc.body.proposal_hash == second_proposal.proposal_hash
    ));
}

#[test]
fn completed_commit_qc_retransmits_after_volatile_peer_handoff() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
    assert_eq!(
        adapter.mark_global_body_locked(locked_round, locked_subject),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_ne!(
        adapter.bind_locked_global_body(&block),
        V2LaneIngressOutcome::Rejected
    );
    let _ = adapter.drain_effects(usize::MAX);

    assert_eq!(
        adapter.insert_lane_qc(
            lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
            locked_round.view,
        ),
        V2LaneIngressOutcome::Inserted
    );
    assert_eq!(
        adapter.insert_lane_qc(
            lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
            locked_round.view,
        ),
        V2LaneIngressOutcome::Inserted
    );
    adapter.drive_lane_sessions();
    let _ = adapter.drain_effects(usize::MAX);
    assert!(
        !adapter.has_pending_committed_output_handoff(),
        "the first complete fanout must have transferred to the volatile peer corridor"
    );

    adapter
        .schedule_retransmission()
        .expect("durable completed certificate starts another fanout round");
    let observed = adapter
        .drain_effects(usize::MAX)
        .into_iter()
        .filter_map(|effect| match effect {
            V2LaneWorkEffect::PostLaneBlock {
                peer,
                message: BlockMessage::LaneBlockQc(qc),
            } if qc.body.phase == CertPhase::Commit
                && qc.body.proposal_hash == proposal.proposal_hash =>
            {
                Some(peer)
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let expected = proposal
        .descriptor
        .validator_set
        .iter()
        .filter(|peer| *peer != &adapter.local_peer)
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(observed, expected);
}

#[test]
fn fixed_view_zero_genesis_binds_under_a_later_proposal_lock() {
    let (mut adapter, _keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
    let genesis_key = KeyPair::try_from_seed(vec![0xE1; 32], Algorithm::Ed25519)
        .expect("deterministic genesis key");
    let genesis_transaction = TransactionBuilder::new(
        ChainId::from("fixed-view-zero-genesis"),
        AccountId::new(genesis_key.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(genesis_key.private_key());
    let staged_genesis = SignedBlock::genesis(
        vec![genesis_transaction.clone()],
        genesis_key.private_key(),
        None,
        None,
    );
    let proposal = staged_genesis.canonical_resultless_proposal();
    let canonical_wire = proposal.encode_wire().expect("encode genesis proposal");
    let subject = wire::BlockSubject {
        parent_block_hash: proposal.header().prev_block_hash(),
        block_hash: proposal.hash(),
        payload_hash: Hash::new(&canonical_wire),
    };
    let later_round = wire::ConsensusRound {
        context_id: adapter.context.id(),
        height: 1,
        view: 3,
    };
    assert_eq!(
        adapter.mark_global_body_locked(later_round, subject),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_eq!(
        adapter.bind_locked_global_body(&proposal),
        V2LaneIngressOutcome::Rejected,
        "the ordinary binding path must keep exact proposal-view semantics"
    );

    let wrong_key = KeyPair::try_from_seed(vec![0xE2; 32], Algorithm::Ed25519)
        .expect("different deterministic genesis key");
    let wrong_genesis = SignedBlock::genesis(
        vec![genesis_transaction],
        wrong_key.private_key(),
        None,
        None,
    );
    assert_eq!(
        adapter.bind_locked_genesis_body(&proposal, &wrong_genesis),
        V2LaneIngressOutcome::Rejected,
        "the fixed-view exception must match the authenticated staged genesis bytes"
    );
    assert_ne!(
        adapter.bind_locked_genesis_body(&proposal, &staged_genesis),
        V2LaneIngressOutcome::Rejected,
        "the exact authenticated view-zero genesis remains recoverable after a certified view change"
    );
}

#[test]
fn higher_same_subject_lock_retains_unchanged_body_binding() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let (block, _) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    let (original_round, subject) = global_lock_for_block(&adapter, &block);
    assert_eq!(
        adapter.mark_global_body_locked(original_round, subject),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_ne!(
        adapter.bind_locked_global_body(&block),
        V2LaneIngressOutcome::Rejected
    );

    let higher_round = wire::ConsensusRound {
        view: original_round.view + 1,
        ..original_round
    };
    assert_eq!(
        adapter.mark_global_body_locked(higher_round, subject),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_ne!(
        adapter.bind_locked_global_body(&block),
        V2LaneIngressOutcome::Rejected,
        "a higher same-subject lock must retain the immutable earlier-view body"
    );
    assert_eq!(
        adapter.bind_locked_genesis_body(&block, &block),
        V2LaneIngressOutcome::Rejected,
        "the fixed-view genesis path cannot weaken a successor-height lock"
    );

    let (mut future_adapter, future_keys) = fixture(wire::ConsensusMode::Permissioned);
    let (future_block, _) = planned_lane_candidate_block_at_view(&future_adapter, &future_keys, 1);
    let (_, future_subject) = global_lock_for_block(&future_adapter, &future_block);
    let premature_lock = wire::ConsensusRound {
        context_id: future_adapter.context.id(),
        height: future_adapter.context.height,
        view: 0,
    };
    assert_eq!(
        future_adapter.mark_global_body_locked(premature_lock, future_subject),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_eq!(
        future_adapter.bind_locked_global_body(&future_block),
        V2LaneIngressOutcome::Rejected,
        "a body originating after the installed lock cannot borrow its authority"
    );
}

#[test]
fn locked_body_protected_session_conflict_keeps_kura_sidecars_state_inert() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let entry = pending_sidecar_entry(&adapter, &keys, 1);
    let entry_hash = adapter
        .kura
        .persist_pending_certified_merge_entry(&entry)
        .expect("persist losing sidecar before locked-body failure");
    let lane_id = LaneId::SINGLE;
    let dataspace_id = DataSpaceId::UNIVERSAL;
    let incarnation = adapter
        .state
        .lane_incarnation_at_height(lane_id, adapter.context.height)
        .expect("fixture lane is active");
    let (block, locked_proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    let conflict = proposal_for_route_at_view(
        &adapter,
        &keys,
        lane_id,
        dataspace_id,
        incarnation,
        adapter.context.height + 1,
        locked_proposal.descriptor.lane_block_height,
        locked_proposal.descriptor.lane_block_view,
    );
    assert_ne!(conflict.proposal_hash, locked_proposal.proposal_hash);
    assert_eq!(
        adapter.lane_sessions.insert_proposal(conflict.clone()),
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    );
    assert_eq!(
        adapter
            .lane_sessions
            .insert_qc_with_pops(lane_qc(&conflict, &keys), &lane_signer_pops(&keys)),
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    );
    let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
    assert_eq!(
        adapter.mark_global_body_locked(locked_round, locked_subject),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_eq!(
        adapter.bind_locked_global_body(&block),
        V2LaneIngressOutcome::Rejected,
        "a lane-local QC for a conflicting payload must remain safety-protected"
    );
    assert_eq!(
        adapter
            .kura
            .merge_entry_by_hash(entry_hash)
            .expect("read sidecar after rejected locked body"),
        Some(entry),
        "rejected in-memory binding must not destructively prune Kura"
    );
}

#[test]
fn locked_body_replaces_uncommitted_same_slot_conflict() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let (block, locked_proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    let descriptor = &locked_proposal.descriptor;
    let conflict = proposal_for_route_at_view(
        &adapter,
        &keys,
        descriptor.lane_id,
        descriptor.dataspace_id,
        descriptor.lane_incarnation,
        adapter.context.height + 1,
        descriptor.lane_block_height,
        descriptor.lane_block_view,
    );
    assert_ne!(conflict.proposal_hash, locked_proposal.proposal_hash);
    assert_eq!(
        adapter.lane_sessions.insert_proposal(conflict.clone()),
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    );

    let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
    assert_eq!(
        adapter.mark_global_body_locked(locked_round, locked_subject),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_eq!(
        adapter.bind_locked_global_body(&block),
        V2LaneIngressOutcome::Inserted,
        "the globally locked payload must displace an uncertified attacker shell"
    );
    assert!(adapter.lane_sessions.contains_proposal(&locked_proposal));
    assert!(!adapter.lane_sessions.contains_proposal(&conflict));
}

#[test]
fn lane_route_reset_watermark_is_global_proposal_height_not_lane_local_height() {
    let lane_id = LaneId::SINGLE;
    let dataspace_id = DataSpaceId::UNIVERSAL;
    let reset_height = 8;

    let (fresh_adapter, fresh_keys) =
        fixture_at_height(wire::ConsensusMode::Permissioned, reset_height + 1);
    mark_lane_reset(&fresh_adapter, lane_id, reset_height);
    let fresh_incarnation = fresh_adapter
        .state
        .lane_incarnation_at_height(lane_id, reset_height + 1)
        .expect("canonical lane incarnation is active after the reset height");
    let fresh_lane_one = proposal_for_route(
        &fresh_adapter,
        &fresh_keys,
        lane_id,
        dataspace_id,
        fresh_incarnation,
        reset_height + 1,
        1,
    );
    assert!(
        fresh_adapter.lane_route_active(
            lane_id,
            dataspace_id,
            fresh_incarnation,
            fresh_lane_one.descriptor.proposal_height,
        ),
        "a newly recreated lane-local height 1 must become active at global reset + 1"
    );
    assert!(
        fresh_adapter.lane_proposal_authorized(&fresh_lane_one, None, true, 0),
        "the fresh lane-local height 1 proposal must pass the complete proposal guard"
    );

    let (stale_adapter, stale_keys) =
        fixture_at_height(wire::ConsensusMode::Permissioned, reset_height);
    mark_lane_reset(&stale_adapter, lane_id, reset_height);
    let stale_incarnation = stale_adapter
        .state
        .lane_incarnation(lane_id)
        .expect("canonical lane incarnation remains identifiable at the reset boundary");
    assert_eq!(
        stale_adapter
            .state
            .lane_incarnation_at_height(lane_id, reset_height),
        None,
        "the reset carrier height must fail closed before proposal construction"
    );
    let stale_high_lane_height = proposal_for_route(
        &stale_adapter,
        &stale_keys,
        lane_id,
        dataspace_id,
        stale_incarnation,
        reset_height,
        100,
    );
    assert!(
        !stale_adapter.lane_route_active(
            lane_id,
            dataspace_id,
            stale_incarnation,
            stale_high_lane_height.descriptor.proposal_height,
        ),
        "a high lane-local height must not outrun the global reset watermark"
    );
    assert!(
        !stale_adapter.lane_proposal_authorized(&stale_high_lane_height, None, true, 0),
        "the complete proposal guard must reject evidence at the reset boundary"
    );
}

#[test]
fn lane_proposal_vote_and_qc_reject_non_authoritative_incarnation() {
    let (adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let lane_id = LaneId::SINGLE;
    let dataspace_id = DataSpaceId::UNIVERSAL;
    let proposal_height = adapter.context.height;
    let active_incarnation = adapter
        .state
        .lane_incarnation_at_height(lane_id, proposal_height)
        .expect("canonical lane incarnation is active");
    let active = proposal_for_route(
        &adapter,
        &keys,
        lane_id,
        dataspace_id,
        active_incarnation,
        proposal_height,
        1,
    );
    let active_vote = signed_lane_vote(&active, CertPhase::Prepare, &keys[0]);
    let active_qc = lane_qc(&active, &keys);
    assert!(adapter.lane_proposal_authorized(&active, None, true, 0));
    assert!(adapter.lane_vote_authorized(&active_vote, 0));
    assert!(adapter.lane_qc_authorized(&active_qc, 0));

    let stale_incarnation = Hash::new(b"retired-v2-lane-work-incarnation");
    assert_ne!(stale_incarnation, active_incarnation);
    let stale = proposal_for_route(
        &adapter,
        &keys,
        lane_id,
        dataspace_id,
        stale_incarnation,
        proposal_height,
        1,
    );
    let stale_vote = signed_lane_vote(&stale, CertPhase::Prepare, &keys[0]);
    let stale_qc = lane_qc(&stale, &keys);
    assert!(
        !adapter.lane_route_active(lane_id, dataspace_id, stale_incarnation, proposal_height,),
        "route admission must bind the exact active incarnation"
    );
    assert!(
        !adapter.lane_proposal_authorized(&stale, None, true, 0),
        "a well-formed, correctly authored proposal from a retired incarnation must fail"
    );
    assert!(
        !adapter.lane_vote_authorized(&stale_vote, 0),
        "a validly signed vote cannot revive a retired incarnation"
    );
    assert!(
        !adapter.lane_qc_authorized(&stale_qc, 0),
        "a cryptographically valid QC cannot revive a retired incarnation"
    );
}
