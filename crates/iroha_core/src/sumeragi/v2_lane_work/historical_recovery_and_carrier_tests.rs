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

/// Build a successor with one retained historical certificate waiting for
/// an absent canonical Kura body and no ingress event needed to service it.
pub(in crate::sumeragi) fn quiet_historical_recovery_fixture() -> V2LaneWorkAdapter {
    let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
    let (parent_block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
    let committed_parent = ValidBlock::committed_from_replay_signed_block(parent_block.clone());
    commit_test_block_to_state(adapter.state.as_ref(), &committed_parent, &adapter.context);
    assert!(
        adapter
            .kura
            .get_durable_block_hash(NonZeroUsize::new(1).expect("non-zero height"))
            .is_none(),
        "quiet recovery fixture must retain the canonical Kura publication gap"
    );
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
    .expect("open quiet historical-recovery successor");
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
    assert!(successor.has_pending_historical_recovery());
    successor
}

#[test]
fn historical_recovery_diagnostics_are_typed_bounded_and_payload_free() {
    let identity = |seed: u8| HistoricalRecoveryIdentity {
        lane_id: LaneId::new(u32::from(seed)),
        dataspace_id: DataSpaceId::new(u64::from(seed)),
        lane_incarnation: Hash::new([seed, 0x11]),
        lane_block_height: u64::from(seed),
        proposal_height: u64::from(seed).saturating_add(1),
        proposal_hash: Hash::new([seed, 0x22]),
        descriptor_hash: Hash::new([seed, 0x33]),
        proposal_block_hash: HashOf::from_untyped_unchecked(Hash::new([seed, 0x44])),
    };
    let reasons = [
        (
            HistoricalRecoveryWaitReason::StateCommitPending,
            HistoricalRecoveryStage::CanonicalAnchor,
            HistoricalRecoveryRetry::LocalState,
        ),
        (
            HistoricalRecoveryWaitReason::CanonicalBlockPending,
            HistoricalRecoveryStage::CanonicalAnchor,
            HistoricalRecoveryRetry::AuthenticatedBlockSync,
        ),
        (
            HistoricalRecoveryWaitReason::AutonomousPayloadPending,
            HistoricalRecoveryStage::ExecutablePayload,
            HistoricalRecoveryRetry::AuthenticatedLanePayload,
        ),
        (
            HistoricalRecoveryWaitReason::PredecessorApplicationPending,
            HistoricalRecoveryStage::PredecessorApplication,
            HistoricalRecoveryRetry::LocalState,
        ),
        (
            HistoricalRecoveryWaitReason::CanonicalResultsPending,
            HistoricalRecoveryStage::CanonicalApplication,
            HistoricalRecoveryRetry::AuthenticatedBlockSync,
        ),
    ];
    let diagnostics = |capacity| {
        HistoricalRecoveryDiagnostics::new(
            capacity,
            iroha_config::parameters::defaults::sumeragi::V2_HISTORICAL_RECOVERY_STUCK_ATTEMPTS,
            iroha_config::parameters::defaults::sumeragi::V2_HISTORICAL_RECOVERY_RETRY_TIER_ATTEMPTS,
            iroha_config::parameters::defaults::sumeragi::V2_HISTORICAL_RECOVERY_MAX_RETRY_TIER,
        )
    };
    let mut all_reasons = diagnostics(reasons.len());
    for (index, (reason, stage, retry)) in reasons.into_iter().enumerate() {
        let observation = all_reasons.observe(identity(index as u8 + 1), reason);
        assert_eq!(observation.reason(), reason);
        assert_eq!(observation.stage(), stage);
        assert_eq!(observation.retry(), retry);
        assert!(observation.first_observation());
        assert!(
            observation.retry_delay(Duration::from_millis(10), Duration::from_secs(1))
                <= Duration::from_secs(1)
        );
    }

    let retry_identity = identity(250);
    let retry_wait = diagnostics(1).observe(
        retry_identity,
        HistoricalRecoveryWaitReason::CanonicalBlockPending,
    );
    let retry_floor = Duration::from_millis(10);
    let retry_ceiling = Duration::from_millis(50);
    for (attempt, expected) in [
        (1, Duration::from_millis(10)),
        (4, Duration::from_millis(10)),
        (5, Duration::from_millis(20)),
        (9, Duration::from_millis(40)),
        (13, Duration::from_millis(50)),
        (u32::MAX, Duration::from_millis(50)),
    ] {
        assert_eq!(
            HistoricalRecoveryWait {
                consecutive_attempts: attempt,
                ..retry_wait
            }
            .retry_delay(retry_floor, retry_ceiling),
            expected,
            "retry attempt {attempt} must select the exact bounded tier"
        );
    }
    assert_eq!(
        retry_wait.retry_delay(retry_floor, Duration::from_millis(1)),
        retry_floor,
        "a ceiling below the floor must normalize to the floor"
    );

    let now = Instant::now();
    let first_cadence = HistoricalRecoveryRequestCadence::immediate(
        HistoricalRecoveryWaitReason::CanonicalBlockPending,
        now,
    )
    .after_retained_attempt(retry_wait, now, retry_floor, retry_ceiling)
    .expect("bounded retry deadline fits the monotonic clock");
    let second_identity = identity(251);
    let second_cadence = HistoricalRecoveryRequestCadence::immediate(
        HistoricalRecoveryWaitReason::CanonicalBlockPending,
        now,
    );
    let per_identity = BTreeMap::from([
        (retry_identity, first_cadence),
        (second_identity, second_cadence),
    ]);
    assert!(!per_identity[&retry_identity].due(now));
    assert!(per_identity[&second_identity].due(now));
    assert!(first_cadence.due(first_cadence.next_retry_at));
    let restarted = HistoricalRecoveryRequestCadence::immediate(
        HistoricalRecoveryWaitReason::CanonicalBlockPending,
        now,
    );
    assert_eq!(restarted.retained_attempts, 0);
    assert!(restarted.due(now));
    let reset = HistoricalRecoveryRequestCadence::immediate(
        HistoricalRecoveryWaitReason::CanonicalResultsPending,
        now,
    );
    assert_eq!(reset.retained_attempts, 0);
    assert!(reset.due(now));
    assert_ne!(reset.reason, first_cadence.reason);

    let secret = "raw-transaction-secret-must-never-enter-recovery-diagnostics";
    let rendered = format!("{:?}", all_reasons.snapshot());
    assert!(!rendered.contains(secret));
    assert!(
        rendered.len() < 8 * 1024,
        "bounded typed observations must remain compact"
    );

    let mut bounded = diagnostics(2);
    let first = identity(1);
    let second = identity(2);
    let third = identity(3);
    bounded.observe(first, HistoricalRecoveryWaitReason::CanonicalBlockPending);
    bounded.observe(
        second,
        HistoricalRecoveryWaitReason::CanonicalResultsPending,
    );
    bounded.observe(first, HistoricalRecoveryWaitReason::CanonicalResultsPending);
    bounded.observe(
        third,
        HistoricalRecoveryWaitReason::AutonomousPayloadPending,
    );
    let snapshot = bounded.snapshot();
    assert_eq!(snapshot.len(), 2);
    assert!(
        snapshot
            .iter()
            .all(|observation| observation.identity() != first),
        "updating an observation must not nondeterministically refresh FIFO eviction order"
    );
    assert!(
        snapshot
            .iter()
            .any(|observation| observation.identity() == second)
    );
    assert!(
        snapshot
            .iter()
            .any(|observation| observation.identity() == third)
    );

    let mut stuck = diagnostics(1);
    let mut stuck_reports = 0;
    for _ in 0
        ..iroha_config::parameters::defaults::sumeragi::V2_HISTORICAL_RECOVERY_STUCK_ATTEMPTS
            .get()
            .saturating_mul(2)
    {
        let observation =
            stuck.observe(first, HistoricalRecoveryWaitReason::CanonicalBlockPending);
        if observation.became_stuck {
            stuck_reports += 1;
        }
    }
    assert_eq!(
        stuck_reports, 1,
        "one identity/reason may emit at most one stuck transition"
    );
}

fn retain_exact_remote_finality_quorum(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    finality: &mut wire::finality::V2FinalityArtifact,
) {
    let local_signer = adapter.context.leader(0);
    finality
        .commit_qc
        .signers
        .retain(|signer| *signer != local_signer);
    assert_eq!(
        u32::try_from(finality.commit_qc.signers.len()).expect("signer count fits u32"),
        finality.height_context.quorum.min_signers,
        "the non-local validators form the exact commit quorum"
    );
    let first_signer = *finality
        .commit_qc
        .signers
        .first()
        .expect("non-local finality quorum has one signer");
    let preimage = finality
        .commit_qc
        .signer_preimage(&adapter.context, first_signer)
        .expect("derive non-local finality signer preimage");
    let signatures = finality
        .commit_qc
        .signers
        .iter()
        .map(|signer| {
            Signature::try_new(
                keys[usize::try_from(*signer).expect("signer index fits usize")].private_key(),
                &preimage,
            )
            .expect("sign non-local finality vote")
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
    finality.commit_qc.aggregate_signature =
        iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
            .expect("aggregate non-local finality votes");
    finality
        .verify()
        .expect("cryptographically valid non-local finality quorum");
}

#[test]
fn historical_missing_canonical_block_schedules_authenticated_retry_then_completes() {
    let mut unbound = quiet_historical_recovery_fixture();
    assert!(matches!(
        unbound
            .service_next_historical_recovery_at(Instant::now())
            .expect("State hash without finality remains a typed local wait"),
        HistoricalRecoveryServiceOutcome::Waiting(wait)
            if wait.reason() == HistoricalRecoveryWaitReason::CanonicalBlockPending
    ));
    assert!(
        unbound.drain_effects(usize::MAX).is_empty(),
        "State's block hash alone must never authorize an unbound body request"
    );
    assert!(unbound.historical_recovery_requests.is_empty());

    let (mut adapter, keys) = fixture_at_height_inner_with_kura(
        wire::ConsensusMode::Permissioned,
        2,
        true,
        locked_lane_work_test_kura(
            NonZeroUsize::new(1).expect("retain one canonical recovery body"),
        ),
    );
    let (parent_block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
    adapter
        .kura
        .store_block(parent_block.clone())
        .expect("persist the canonical historical carrier");
    let mut finality = verified_finality_artifact_for_block(&adapter, &keys, &parent_block);
    retain_exact_remote_finality_quorum(&adapter, &keys, &mut finality);
    let finality_receipt = adapter
        .kura
        .store_v2_finality_artifact(&finality)
        .expect("persist the immutable historical body authority");
    assert_eq!(finality_receipt.block_hash(), parent_block.hash());
    assert_eq!(finality_receipt.artifact_hash(), HashOf::new(&finality));
    let committed_parent = ValidBlock::committed_from_replay_signed_block(parent_block.clone());
    commit_test_block_to_state(adapter.state.as_ref(), &committed_parent, &adapter.context);
    evict_canonical_executed_block_fixture(&adapter, &keys, &parent_block);
    assert!(
        adapter
            .kura
            .get_block_without_merge_sidecar(
                NonZeroUsize::new(2).expect("non-zero historical height"),
            )
            .is_none(),
        "fixture must retain finality while the canonical body is remote-only"
    );
    let certificate = LaneBlockCertificateV1 {
        proposal: proposal.clone(),
        prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
        commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
    };
    adapter.context = successor_context_for_parent(&adapter, &parent_block);
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneBlockCertificate(Box::new(certificate)),
                Some(PeerId::new(keys[0].public_key().clone())),
            ),
            0,
        ),
        V2LaneIngressOutcome::Inserted
    );
    let first_attempt_at = Instant::now();
    let wait = match adapter
        .service_next_historical_recovery_at(first_attempt_at)
        .expect("missing canonical body is a typed retry")
    {
        HistoricalRecoveryServiceOutcome::Waiting(wait) => wait,
        outcome => panic!("expected canonical-body wait, got {outcome:?}"),
    };
    assert_eq!(
        wait.reason(),
        HistoricalRecoveryWaitReason::CanonicalBlockPending
    );
    assert_eq!(
        wait.retry(),
        HistoricalRecoveryRetry::AuthenticatedBlockSync
    );
    assert_eq!(adapter.historical_recovery_waits_snapshot(), vec![wait]);
    let first_requests = adapter.drain_effects(usize::MAX);
    let request_frames = |effects: &[V2LaneWorkEffect]| {
        effects
            .iter()
            .filter_map(|effect| match effect {
                V2LaneWorkEffect::PostLaneBlock {
                    peer,
                    message: message @ BlockMessage::LaneHistoricalRecoveryRequest(_),
                } => Some((peer.clone(), message.encode())),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let first_request_frames = request_frames(&first_requests);
    assert!(
        !first_requests.is_empty()
            && first_request_frames.len() == first_requests.len(),
        "finality-bound recovery must emit only authenticated historical requests"
    );
    let identity = wait.identity();
    let first_cadence = adapter
        .historical_recovery_requests
        .get(&identity)
        .expect("first request retains one exact retry owner")
        .cadence;
    assert_eq!(first_cadence.retained_attempts, 1);
    assert_eq!(
        first_cadence.next_retry_at,
        first_attempt_at
            .checked_add(adapter.limits.historical_recovery_retry_floor)
            .expect("bounded first retry deadline")
    );

    let before_deadline = first_cadence
        .next_retry_at
        .checked_sub(Duration::from_nanos(1))
        .expect("first deadline follows its sampled service time");
    assert!(matches!(
        adapter
            .service_next_historical_recovery_at(before_deadline)
            .expect("local recovery checks continue before the network deadline"),
        HistoricalRecoveryServiceOutcome::Waiting(_)
    ));
    assert!(
        adapter.drain_effects(usize::MAX).is_empty(),
        "a same-reason service turn before the deadline must not fan out"
    );
    assert_eq!(
        adapter
            .historical_recovery_requests
            .get(&identity)
            .expect("suppressed retry retains its owner")
            .cadence
            .retained_attempts,
        1,
        "local observations must not advance network retry tiers"
    );

    let late_retry_at = first_cadence
        .next_retry_at
        .checked_add(
            adapter
                .limits
                .historical_recovery_retry_floor
                .saturating_mul(5),
        )
        .expect("delayed retry remains inside the monotonic clock");
    let effect_capacity = adapter.limits.effect_capacity;
    adapter.limits.effect_capacity = NonZeroUsize::new(1).expect("one blocking effect");
    let blocker = match &first_requests[0] {
        V2LaneWorkEffect::PostLaneBlock { message, .. } => {
            V2LaneWorkEffect::PostLaneBlock {
                peer: adapter.local_peer.clone(),
                message: message.clone(),
            }
        }
        _ => unreachable!("historical request fixture emitted only lane posts"),
    };
    assert!(adapter.push_effect(blocker));
    assert!(matches!(
        adapter
            .service_next_historical_recovery_at(late_retry_at)
            .expect("backpressure retains a due historical retry"),
        HistoricalRecoveryServiceOutcome::Waiting(_)
    ));
    assert_eq!(
        adapter
            .historical_recovery_requests
            .get(&identity)
            .expect("backpressured retry retains its owner")
            .cadence
            .retained_attempts,
        1,
        "a full effect queue must not advance the retry deadline"
    );
    assert_eq!(adapter.drain_effects(usize::MAX).len(), 1);
    adapter.limits.effect_capacity = effect_capacity;

    assert!(matches!(
        adapter
            .service_next_historical_recovery_at(late_retry_at)
            .expect("free capacity permits the still-due bounded retry"),
        HistoricalRecoveryServiceOutcome::Waiting(_)
    ));
    let second_requests = adapter.drain_effects(usize::MAX);
    assert!(
        !second_requests.is_empty(),
        "a due retry must re-emit the authenticated request"
    );
    assert_eq!(
        request_frames(&second_requests),
        first_request_frames,
        "retry must preserve the exact peer order and request bytes"
    );
    let second_cadence = adapter
        .historical_recovery_requests
        .get(&identity)
        .expect("second request retains the exact owner")
        .cadence;
    assert_eq!(second_cadence.retained_attempts, 2);
    assert_eq!(
        second_cadence.next_retry_at,
        late_retry_at
            .checked_add(adapter.limits.historical_recovery_retry_floor)
            .expect("bounded second retry deadline"),
        "the next deadline is anchored at the service turn, not the prior schedule"
    );

    adapter
        .kura
        .cache_block_body(&parent_block)
        .expect("simulate authenticated canonical body recovery");
    let local_completion_at = second_cadence
        .next_retry_at
        .checked_sub(Duration::from_nanos(1))
        .expect("second deadline follows the body recovery turn");
    assert!(matches!(
        adapter
            .service_next_historical_recovery_at(local_completion_at)
            .expect("local completion is never gated by the network deadline"),
        HistoricalRecoveryServiceOutcome::Complete(_)
    ));
    assert!(adapter.drain_effects(usize::MAX).is_empty());
    assert!(adapter.historical_recovery_waits_snapshot().is_empty());
    assert!(adapter.historical_recovery_requests.is_empty());
    assert!(adapter.historical_recovery_request_owners.is_empty());
    assert!(adapter.kura.lane_block_application_receipt_available(&proposal));
}

#[test]
fn retained_sidecar_handoff_rejects_foreign_owner_and_wrong_successor() {
    let CertifiedSidecarServerFixture {
        mut adapter,
        validators,
        kura,
        context,
        ..
    } = certified_sidecar_server_fixture();
    let (_, transport_owner) = durable_exact_output_handoff_owner_pair();
    adapter.exact_output_handoff_owner = transport_owner;
    let (foreign_service_owner, _) = durable_exact_output_handoff_owner_pair();
    let foreign_service = service_for_history_context_with_handoff_owner(
        Arc::clone(&kura),
        context.clone(),
        &validators,
        foreign_service_owner,
    );
    let (foreign_receipt, foreign_artifact) =
        durable_finality_fixture(&foreign_service, &validators);
    let foreign_lane_authority = DurableLaneRolloverAuthority::missing_winning_witness_for_test(
        &foreign_artifact,
        Hash::new(b"foreign exact-output owner lane witness"),
    );
    let foreign_handoff = foreign_service
        .seal_applied_height_output_handoff(
            &foreign_receipt,
            &foreign_artifact,
            &foreign_lane_authority,
        )
        .expect("foreign service can seal only its own empty corridor");
    let foreign_successor = immediate_successor_context(&foreign_artifact, None);
    let foreign_failure_guard = Arc::clone(&adapter.output_guard);
    assert!(matches!(
        adapter.into_retained_merge_sidecars(
            foreign_handoff,
            &foreign_artifact,
            &foreign_successor,
        ),
        Err(V2LaneWorkError::InvalidContext(ref reason))
            if reason.contains("another service/transport owner")
    ));
    assert!(
        foreign_failure_guard.restart_required(),
        "a post-finality owner mismatch must fail closed rather than remain recoverable"
    );

    let CertifiedSidecarServerFixture {
        mut adapter,
        validators,
        kura,
        context,
        ..
    } = certified_sidecar_server_fixture();
    let (service_owner, transport_owner) = durable_exact_output_handoff_owner_pair();
    adapter.exact_output_handoff_owner = transport_owner;
    let service = service_for_history_context_with_handoff_owner(
        kura,
        context,
        &validators,
        service_owner,
    );
    let (receipt, artifact) = durable_finality_fixture(&service, &validators);
    let lane_authority = DurableLaneRolloverAuthority::missing_winning_witness_for_test(
        &artifact,
        Hash::new(b"wrong successor context lane witness"),
    );
    let handoff = service
        .seal_applied_height_output_handoff(&receipt, &artifact, &lane_authority)
        .expect("matching service seals its empty corridor");
    let successor = immediate_successor_context(&artifact, None);
    let reply_source_capacity = adapter.limits.reply_source_capacity.get();
    let sidecar_limits = adapter.limits.merge_sidecar_limits;
    let successor_roster = successor
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let retained = adapter
        .into_retained_merge_sidecars(handoff, &artifact, &successor)
        .expect("matching owner binds the exact successor");
    let mut wrong_successor = successor.clone();
    wrong_successor.leader_seed[0] ^= 0x01;
    wrong_successor
        .validate()
        .expect("wrong-context fixture remains structurally valid");
    assert!(matches!(
        retained.rehydrate_for_successor(
            &wrong_successor,
            reply_source_capacity,
            sidecar_limits,
            successor_roster.len(),
            canonical_merge_sidecar_roster_digest(&successor_roster),
            Instant::now(),
        ),
        Err(V2LaneWorkError::InvalidContext(ref reason))
            if reason.contains("another successor context")
    ));
}

#[test]
fn sidecar_server_allocations_require_roster_requester_but_not_roster_relay() {
    let CertifiedSidecarServerFixture {
        mut adapter,
        requester,
        request,
        ..
    } = certified_sidecar_server_fixture();
    let outsider = PeerId::new(
        KeyPair::try_from_seed(vec![0xE3; 32], Algorithm::BlsNormal)
            .expect("deterministic outsider key")
            .public_key()
            .clone(),
    );
    assert!(!adapter.frozen_roster_contains(&outsider));
    let hub = PeerId::new(
        KeyPair::try_from_seed(vec![0xE4; 32], Algorithm::BlsNormal)
            .expect("deterministic non-roster hub key")
            .public_key()
            .clone(),
    );
    assert!(!adapter.frozen_roster_contains(&hub));
    let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(
        hub.clone(),
        adapter.limits.reply_source_capacity.get(),
    );

    let mut outsider_request = request.clone();
    outsider_request.requester = outsider.clone();
    outsider_request.request_id = outsider_request.canonical_request_id();
    let outsider_route = routes.mint_via(outsider.clone(), hub.clone());
    assert_eq!(
        adapter
            .accept_certified_merge_sidecar_for_test(
                outsider.clone(),
                outsider_route.clone(),
                outsider_request,
            )
            .expect("outsider request is rejected without local failure"),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(adapter.merge_sidecars.server_stream_count_for_test(), 0);
    assert_eq!(
        adapter.merge_sidecars.server_request_gate_count_for_test(),
        0
    );
    assert_eq!(
        adapter
            .merge_sidecars
            .server_request_attempt_count_for_test(),
        0
    );
    assert_eq!(
        adapter
            .merge_sidecars
            .retained_outbound_attempt_count_for_test(),
        0
    );
    assert_eq!(adapter.merge_sidecars.retained_outbound_bytes_for_test(), 0);

    let mut outsider_close = CertifiedMergeSidecarCloseV1 {
        version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
        service_generation: request.service_generation,
        stream_epoch: request.stream_epoch,
        closed_through: request.semantic_sequence.get(),
        close_id: Hash::prehashed([0; Hash::LENGTH]),
        requester: outsider.clone(),
        responder: adapter.local_peer.clone(),
    };
    outsider_close.close_id = outsider_close.canonical_close_id();
    assert_eq!(
        adapter
            .accept_certified_merge_sidecar_close(
                outsider,
                Some(outsider_route),
                outsider_close,
            )
            .expect("outsider close is rejected without local failure"),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(adapter.merge_sidecars.server_stream_count_for_test(), 0);
    assert_eq!(
        adapter.merge_sidecars.server_request_gate_count_for_test(),
        0
    );
    assert_eq!(
        adapter
            .merge_sidecars
            .server_request_attempt_count_for_test(),
        0
    );

    let requester_route = routes.mint_via(requester.clone(), hub);
    assert_eq!(
        adapter
            .accept_certified_merge_sidecar_for_test(
                requester.clone(),
                requester_route,
                request,
            )
            .expect("roster requester via a non-roster hub is serviceable"),
        V2LaneIngressOutcome::Inserted
    );
    assert_eq!(adapter.merge_sidecars.server_stream_count_for_test(), 1);
    assert_eq!(
        adapter.merge_sidecars.server_request_gate_count_for_test(),
        1
    );
    assert_eq!(
        adapter
            .merge_sidecars
            .server_request_attempt_count_for_test(),
        1
    );
    assert_eq!(
        adapter
            .merge_sidecars
            .retained_outbound_attempt_count_for_test(),
        1
    );
    assert!(adapter.merge_sidecars.retained_outbound_bytes_for_test() > 0);
}

#[test]
fn sidecar_ingress_materializes_the_fair_scheduler_job_not_the_newest_request() {
    let CertifiedSidecarServerFixture {
        mut adapter,
        requester: first_requester,
        request: first_request,
        ..
    } = certified_sidecar_server_fixture();
    let second_requester = adapter
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .find(|peer| peer != &adapter.local_peer && peer != &first_requester)
        .expect("fixture has a second remote roster requester");
    let mut second_request = first_request.clone();
    second_request.requester = second_requester.clone();
    second_request.request_id = second_request.canonical_request_id();
    let hub = PeerId::new(
        KeyPair::try_from_seed(vec![0xE5; 32], Algorithm::BlsNormal)
            .expect("deterministic scheduler hub key")
            .public_key()
            .clone(),
    );
    let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(
        hub.clone(),
        adapter.limits.reply_source_capacity.get(),
    );
    let local_peer = adapter.local_peer.clone();
    let first_route = routes.mint_via(first_requester.clone(), hub.clone());
    assert!(matches!(
        adapter
            .merge_sidecars
            .admit_server_request(
                &first_requester,
                &first_request,
                Some(&first_route),
                &local_peer,
                Instant::now(),
            )
            .expect("first requester acquires fair lookup authority"),
        ServerRequestAdmission::Materialize
    ));
    assert!(adapter.sidecar_effects.is_empty());

    let second_route = routes.mint_via(second_requester.clone(), hub);
    assert_eq!(
        adapter
            .accept_certified_merge_sidecar_for_test(
                second_requester.clone(),
                second_route,
                second_request.clone(),
            )
            .expect("newer ingress services the scheduler-owned older request"),
        V2LaneIngressOutcome::Inserted
    );
    assert!(adapter.sidecar_effects.iter().any(|effect| {
        matches!(
            effect,
            V2LaneWorkEffect::PostCertifiedMergeSidecar { message, .. }
                if matches!(
                    message.as_ref(),
                    CertifiedMergeSidecarMessage::Chunk(chunk)
                        if chunk.requester == first_requester
                )
        )
    }));
    assert!(!adapter.sidecar_effects.iter().any(|effect| {
        matches!(
            effect,
            V2LaneWorkEffect::PostCertifiedMergeSidecar { message, .. }
                if matches!(
                    message.as_ref(),
                    CertifiedMergeSidecarMessage::Chunk(chunk)
                        if chunk.requester == second_requester
                )
        )
    }));
    assert!(
        adapter
            .merge_sidecars
            .has_server_request_gate_for_test(&second_requester, &second_request),
        "the newer request remains retryable after serving the fair scheduler head"
    );
}

#[derive(Clone, Copy)]
enum HistoricalSidecarFinality {
    Exact,
    Missing,
    WrongChain,
    WrongRoster,
}

struct HistoricalSidecarServerFixture {
    adapter: V2LaneWorkAdapter,
    requester: PeerId,
    request: crate::merge_sidecar::CertifiedMergeSidecarRequestV1,
    finality: wire::finality::V2FinalityArtifact,
    carrier_height: u64,
}

fn merge_entry_from_reference(
    reference: &CertifiedMergeLedgerReference,
    state_root: &[u8],
) -> MergeLedgerEntry {
    MergeLedgerEntry {
        version: MergeLedgerEntry::VERSION,
        epoch_id: reference.epoch_id,
        lane_catalog_hash: Hash::new(b"historical sidecar catalog"),
        active_lanes: Vec::new(),
        incarnation_root: Hash::new(b"historical sidecar incarnations"),
        activation_root: Hash::new(b"historical sidecar activations"),
        lane_snapshots: Vec::new(),
        lane_drain_certificates: Vec::new(),
        queue_plan_admissions: Vec::new(),
        execution_batch: None,
        global_state_root: Hash::new(state_root),
        merge_qc: reference.merge_qc.clone(),
    }
}

fn merge_sidecar_carrier_block(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    entry: &MergeLedgerEntry,
) -> SignedBlock {
    let qc = &entry.merge_qc;
    let leader = usize::try_from(adapter.context.leader(qc.view))
        .expect("historical carrier leader index fits usize");
    let header = BlockHeader::new(
        NonZeroU64::new(qc.carrier_height).expect("historical carrier height is non-zero"),
        Some(qc.carrier_parent_hash),
        None,
        None,
        qc.carrier_height,
        qc.view,
    );
    let mut builder = BlockBuilder::new(header);
    builder.set_execution_context(Some(
        BlockExecutionContextBundle::new(Vec::new())
            .with_merge_entry(CertifiedMergeLedgerReference::new(entry)),
    ));
    builder.build_with_signature(
        u64::try_from(leader).expect("historical carrier leader index fits u64"),
        keys[leader].private_key(),
    )
}

fn verified_finality_for_context(
    context: &wire::HeightContext,
    keys: &[KeyPair],
    block: &SignedBlock,
) -> wire::finality::V2FinalityArtifact {
    let subject = wire::BlockSubject {
        parent_block_hash: block.header().prev_block_hash(),
        block_hash: block.hash(),
        payload_hash: block
            .canonical_proposal_wire_hash()
            .expect("encode historical sidecar carrier"),
    };
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: block.header().view_change_index(),
    };
    let mut commit_qc = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"historical sidecar parent state"),
            Hash::new(b"historical sidecar post state"),
            Hash::new(b"historical sidecar writes"),
            u64::try_from(
                block
                    .encode_wire()
                    .expect("historical request block wire")
                    .len(),
            )
            .expect("historical request block wire length fits u64"),
            block
                .executed_block_wire_hash()
                .expect("encode historical sidecar executed block"),
        ),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    };
    let preimage = commit_qc
        .signer_preimage(context, 0)
        .expect("derive historical sidecar finality preimage");
    let signatures = commit_qc
        .signers
        .iter()
        .map(|index| {
            Signature::try_new(
                keys[usize::try_from(*index).expect("historical signer index")].private_key(),
                &preimage,
            )
            .expect("sign historical sidecar finality")
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
    commit_qc.aggregate_signature =
        iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
            .expect("aggregate historical sidecar finality");
    let artifact = wire::finality::V2FinalityArtifact::new(
        context.clone(),
        subject,
        commit_qc,
        keys.iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("derive historical sidecar finality PoP")
            })
            .collect(),
    );
    artifact
        .verify()
        .expect("historical sidecar finality is cryptographically valid");
    artifact
}

#[allow(clippy::too_many_lines)]
fn historical_sidecar_server_fixture(
    finality_kind: HistoricalSidecarFinality,
    holder_indices: Option<&[usize]>,
    request_noncanonical_entry: bool,
) -> HistoricalSidecarServerFixture {
    let (adapter, keys) = fixture_at_height_inner(wire::ConsensusMode::Permissioned, 2, true);
    let canonical_reference = holder_indices.map_or_else(
        || missing_sidecar_reference(&adapter, &keys, 1),
        |indices| missing_sidecar_reference_with_signers(&adapter, &keys, 1, indices),
    );
    let canonical_entry =
        merge_entry_from_reference(&canonical_reference, b"historical canonical sidecar");
    let canonical_entry_hash = adapter
        .kura
        .persist_pending_certified_merge_entry(&canonical_entry)
        .expect("persist historical canonical merge entry");

    let requested_entry = request_noncanonical_entry.then(|| {
        let reference = missing_sidecar_reference(&adapter, &keys, 1);
        merge_entry_from_reference(&reference, b"historical noncanonical sidecar")
    });
    let requested_entry = requested_entry.as_ref().unwrap_or(&canonical_entry);
    let requested_reference = CertifiedMergeLedgerReference::new(requested_entry);

    let block = merge_sidecar_carrier_block(&adapter, &keys, &canonical_entry);
    adapter
        .kura
        .store_block(block.clone())
        .expect("persist historical merge carrier");
    let mut finality_context = adapter.context.clone();
    let mut finality_keys = keys;
    match finality_kind {
        HistoricalSidecarFinality::Exact | HistoricalSidecarFinality::Missing => {}
        HistoricalSidecarFinality::WrongChain => {
            finality_context.chain_id = "wrong-historical-sidecar-chain".into();
        }
        HistoricalSidecarFinality::WrongRoster => {
            finality_keys = (11_u8..=14)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic wrong historical roster key")
                })
                .collect();
            finality_keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            finality_context.roster = finality_keys
                .iter()
                .map(|key| wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                })
                .collect();
            finality_context.quorum = wire::DualQuorum::from_roster(&finality_context.roster)
                .expect("wrong historical roster has a valid quorum");
        }
    }
    let finality = verified_finality_for_context(&finality_context, &finality_keys, &block);
    if !matches!(finality_kind, HistoricalSidecarFinality::Missing) {
        let _ = adapter
            .kura
            .store_v2_finality_artifact(&finality)
            .expect("persist historical sidecar finality");
    }

    let committed = ValidBlock::committed_from_replay_signed_block(block.clone());
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
    let successor_context = successor_context_for_parent(&adapter, &block);
    let local_peer = adapter.local_peer.clone();
    let local_key = adapter.key_pair.clone();
    let state = Arc::clone(&adapter.state);
    let kura = Arc::clone(&adapter.kura);
    let limits = adapter.limits;
    let carrier_height = adapter.context.height;
    let requester = adapter
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .find(|peer| peer != &local_peer)
        .expect("historical sidecar fixture has a remote requester");
    drop(adapter);
    let successor = V2LaneWorkAdapter::new(
        successor_context,
        local_peer.clone(),
        local_key,
        true,
        state,
        kura,
        limits,
        None,
    )
    .expect("open advanced historical sidecar responder");
    let requested_entry_hash = if request_noncanonical_entry {
        successor
            .kura
            .persist_pending_certified_merge_entry(requested_entry)
            .expect("persist noncanonical historical merge entry after rollover pruning")
    } else {
        canonical_entry_hash
    };
    let mut request = crate::merge_sidecar::CertifiedMergeSidecarRequestV1 {
        version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
        service_generation: CertifiedMergeSidecarServiceGenerationV1::INITIAL,
        stream_epoch: CertifiedMergeSidecarStreamEpochV1(
            NonZeroU64::new(1).expect("historical sidecar stream epoch is non-zero"),
        ),
        semantic_sequence: semantic_sequence(1),
        closed_through: 0,
        request_id: Hash::new(b"historical certified sidecar request"),
        entry_hash: requested_entry_hash,
        encoded_len: requested_reference.encoded_len,
        epoch_id: requested_reference.epoch_id,
        reference_digest: certified_merge_reference_digest(&requested_reference),
        requester: requester.clone(),
        responder: local_peer,
    };
    request.request_id = request.canonical_request_id();
    HistoricalSidecarServerFixture {
        adapter: successor,
        requester,
        request,
        finality,
        carrier_height,
    }
}

fn dispatch_historical_sidecar_request(
    fixture: &mut HistoricalSidecarServerFixture,
) -> V2LaneIngressOutcome {
    let hub = PeerId::new(KeyPair::random().public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(
        hub.clone(),
        fixture.adapter.limits.reply_source_capacity.get(),
    );
    let reply_route = routes.mint_via(fixture.requester.clone(), hub);
    fixture
        .adapter
        .accept_certified_merge_sidecar_for_test(
            fixture.requester.clone(),
            reply_route,
            fixture.request.clone(),
        )
        .expect("historical sidecar request handling remains operational")
}

#[test]
fn advanced_responder_serves_exact_finalized_historical_merge_sidecar() {
    let mut fixture =
        historical_sidecar_server_fixture(HistoricalSidecarFinality::Exact, None, false);
    assert_eq!(
        dispatch_historical_sidecar_request(&mut fixture),
        V2LaneIngressOutcome::Inserted
    );
    assert!(fixture.adapter.sidecar_effects.iter().any(|effect| {
        matches!(
            effect,
            V2LaneWorkEffect::PostCertifiedMergeSidecar { message, .. }
                if matches!(
                    message.as_ref(),
                    CertifiedMergeSidecarMessage::Chunk(chunk)
                        if chunk.entry_hash == fixture.request.entry_hash
                )
        )
    }));
}

#[test]
fn current_height_sidecar_service_rejects_a_different_carrier_parent() {
    let CertifiedSidecarServerFixture {
        mut adapter,
        requester,
        mut request,
        ..
    } = certified_sidecar_server_fixture();
    let mut entry = adapter
        .kura
        .merge_entry_by_hash(request.entry_hash)
        .expect("read current-height sidecar entry")
        .expect("current-height sidecar entry exists");
    entry.merge_qc.carrier_parent_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"wrong current carrier parent"));
    request.entry_hash = adapter
        .kura
        .persist_pending_certified_merge_entry(&entry)
        .expect("persist wrong-parent current-height sidecar");
    let reference = CertifiedMergeLedgerReference::new(&entry);
    request.encoded_len = reference.encoded_len;
    request.epoch_id = reference.epoch_id;
    request.reference_digest = certified_merge_reference_digest(&reference);
    request.request_id = request.canonical_request_id();
    let hub = PeerId::new(KeyPair::random().public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(
        hub.clone(),
        adapter.limits.reply_source_capacity.get(),
    );
    let reply_route = routes.mint_via(requester.clone(), hub);
    assert_eq!(
        adapter
            .accept_certified_merge_sidecar_for_test(requester, reply_route, request)
            .expect("wrong-parent current request is handled"),
        V2LaneIngressOutcome::Rejected
    );
    assert!(adapter.sidecar_effects.is_empty());
}
