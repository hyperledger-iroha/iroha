struct LaneAdapterRestartParts {
    local_peer: PeerId,
    key_pair: KeyPair,
    state: Arc<State>,
    kura: Arc<Kura>,
    limits: V2LaneWorkLimits,
}

impl LaneAdapterRestartParts {
    fn capture(adapter: &V2LaneWorkAdapter) -> Self {
        Self {
            local_peer: adapter.local_peer.clone(),
            key_pair: adapter.key_pair.clone(),
            state: Arc::clone(&adapter.state),
            kura: Arc::clone(&adapter.kura),
            limits: adapter.limits,
        }
    }

    fn reopen(
        &self,
        context: wire::HeightContext,
        voting_enabled: bool,
    ) -> Result<V2LaneWorkAdapter, V2LaneWorkError> {
        V2LaneWorkAdapter::new(
            context,
            self.local_peer.clone(),
            self.key_pair.clone(),
            voting_enabled,
            Arc::clone(&self.state),
            Arc::clone(&self.kura),
            self.limits,
            None,
        )
    }

    fn reopen_isolated(
        &self,
        context: wire::HeightContext,
        voting_enabled: bool,
    ) -> Result<V2LaneWorkAdapter, V2LaneWorkError> {
        V2LaneWorkAdapter::new_with_output_guard(
            context,
            self.local_peer.clone(),
            self.key_pair.clone(),
            voting_enabled,
            Arc::clone(&self.state),
            Arc::clone(&self.kura),
            self.limits,
            None,
            None,
            ConsensusOutputGuard::isolated(),
        )
    }
}

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
    let restart = LaneAdapterRestartParts::capture(&adapter);
    drop(adapter);
    let mut successor = restart
        .reopen(successor_context, true)
        .expect("open successor before the historical certificate arrives");
    assert_eq!(
        accept_lane_message_from(
            &mut successor,
            BlockMessage::LaneBlockCertificate(Box::new(certificate)),
            PeerId::new(keys[0].public_key().clone()),
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
    let (locked_round, _locked_subject) = mark_global_body_locked_for_block(&mut adapter, &block);
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
    let sessions = [losing_proposal, winning_proposal]
        .map(|proposal| committed_lane_session(&proposal, &keys));
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
}
#[test]
fn completed_commit_qc_round_robin_does_not_restart_ahead_of_pending_source() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let (_, first_proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    let (_, second_proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 1);
    let first_session = committed_lane_session(&first_proposal, &keys);
    let second_session = committed_lane_session(&second_proposal, &keys);
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
    let (locked_round, _locked_subject) = mark_global_body_locked_for_block(&mut adapter, &block);
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
        crate::sumeragi::synthetic_network_id("fixed-view-zero-genesis"),
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
    let (original_round, subject) = mark_global_body_locked_for_block(&mut adapter, &block);
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
    let (_locked_round, _locked_subject) = mark_global_body_locked_for_block(&mut adapter, &block);
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
    let (_locked_round, _locked_subject) = mark_global_body_locked_for_block(&mut adapter, &block);
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
    let restart = LaneAdapterRestartParts::capture(&adapter);
    drop(adapter);
    let mut successor = restart
        .reopen(successor_context, true)
        .expect("open quiet historical-recovery successor");
    assert_eq!(
        accept_lane_message_from(
            &mut successor,
            BlockMessage::LaneBlockCertificate(Box::new(certificate)),
            PeerId::new(keys[0].public_key().clone()),
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
    for _ in 0..iroha_config::parameters::defaults::sumeragi::V2_HISTORICAL_RECOVERY_STUCK_ATTEMPTS
        .get()
        .saturating_mul(2)
    {
        let observation = stuck.observe(first, HistoricalRecoveryWaitReason::CanonicalBlockPending);
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
fn canonical_executed_block_dispatch_drains_old_output_before_current_request() {
    let (adapter, keys, canonical_block, finality) = canonical_executed_block_recovery_fixture();
    let need = canonical_executed_block_need(&canonical_block, &finality);
    let requester = finality
        .commit_qc
        .signers
        .iter()
        .filter_map(|index| {
            usize::try_from(*index)
                .ok()
                .and_then(|index| finality.height_context.roster.get(index))
                .map(|entry| entry.validator.clone())
        })
        .next()
        .expect("fixture has a canonical recovery requester");
    let request = canonical_executed_block_request(requester.clone(), need, 0);
    evict_canonical_executed_block_fixture(&adapter, &keys, &canonical_block);
    let context = adapter.context.clone();
    let state = Arc::clone(&adapter.state);
    let kura = Arc::clone(&adapter.kura);
    let limits = adapter.limits;
    drop(adapter);
    let requester_index = context
        .roster
        .iter()
        .position(|entry| entry.validator == requester)
        .and_then(|index| wire::ValidatorIndex::try_from(index).ok())
        .expect("requester belongs to the frozen context");
    let mut services = service_for_history_context_with_local_validator(
        Arc::clone(&kura),
        context.clone(),
        &keys,
        requester_index,
    );
    services
        .set_exact_output_shared_unit_capacity_for_test(1)
        .expect("install one shared exact-output slot");
    let blocked = Arc::new(AtomicBool::new(true));
    let blocked_for_hook = Arc::clone(&blocked);
    let admitted = Arc::new(AtomicUsize::new(0));
    let admitted_for_hook = Arc::clone(&admitted);
    services.set_exact_output_admission_hook(move |post, ticket| {
        if blocked_for_hook.load(Ordering::Acquire) {
            return Err(
                iroha_p2p::network::NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket,
                    rank: 1,
                },
            );
        }
        admitted_for_hook.fetch_add(1, Ordering::Relaxed);
        Ok(())
    });
    let output_guard = services.lifecycle_output_guard();
    let mut recovery = CanonicalExecutedBlockRecovery::new(
        context,
        requester,
        state,
        kura,
        output_guard,
        limits,
        vec![need],
    )
    .expect("install exact-output retry fixture");
    assert!(recovery.service_next().expect("queue responder A request"));
    let first_peer = match recovery.effects.front() {
        Some(V2LaneWorkEffect::PostLaneBlock { peer, .. }) => peer.clone(),
        other => panic!("canonical recovery must queue a lane request: {other:?}"),
    };
    let (_, first_dispatched) =
        dispatch_canonical_executed_block_recovery_effects_for_test(&mut recovery, &services, 1)
            .expect("handoff responder A request into exact output");
    assert!(first_dispatched);
    assert_eq!(recovery.effect_count(), 0);
    assert!(
        services
            .has_pending_exact_output()
            .expect("inspect backpressured responder A request")
    );
    assert!(
        recovery
            .service_next()
            .expect("queue exact responder A retry")
    );
    let (_, retry_dispatched) =
        dispatch_canonical_executed_block_recovery_effects_for_test(&mut recovery, &services, 1)
            .expect("coalesce responder A retry under exact ownership");
    assert!(retry_dispatched);
    assert!(recovery.service_next().expect("rotate to responder B"));
    let second_peer = match recovery.effects.front() {
        Some(V2LaneWorkEffect::PostLaneBlock { peer, .. }) => peer.clone(),
        other => panic!("rotated recovery must queue a lane request: {other:?}"),
    };
    assert_ne!(second_peer, first_peer);
    let mut filler_count = 0_usize;
    for nonce in 0_u8..8 {
        if !services
            .can_retain_lane_work_effect(
                recovery
                    .effects
                    .front()
                    .expect("responder B request stays source-owned"),
            )
            .expect("inspect responder B reservation")
        {
            break;
        }
        let mut filler = request.clone();
        filler.version = filler
            .version
            .saturating_add(u16::from(nonce).saturating_add(1));
        services
            .post_lane_block(
                second_peer.clone(),
                BlockMessage::LaneHistoricalRecoveryRequest(Box::new(filler)),
            )
            .expect("retain exact-output capacity filler");
        filler_count = filler_count.saturating_add(1);
    }
    assert_ne!(
        filler_count, 0,
        "the fixture must consume responder B capacity"
    );
    assert!(
        !services
            .can_retain_lane_work_effect(
                recovery
                    .effects
                    .front()
                    .expect("responder B request stays source-owned"),
            )
            .expect("inspect saturated responder B reservation"),
        "the current request must remain at source until old output drains"
    );
    blocked.store(false, Ordering::Release);
    let (_, second_dispatched) =
        dispatch_canonical_executed_block_recovery_effects_for_test(&mut recovery, &services, 1)
            .expect("drain old output before reserving responder B");
    assert!(second_dispatched);
    assert_eq!(recovery.effect_count(), 0);
    assert!(
        !services
            .has_pending_exact_output()
            .expect("all canonical exact output reaches transport")
    );
    assert_eq!(
        admitted.load(Ordering::Relaxed),
        filler_count.saturating_add(3),
        "both responder-A attempts, every filler, and responder B cross transport exactly once"
    );
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
        accept_lane_message_from(
            &mut adapter,
            BlockMessage::LaneBlockCertificate(Box::new(certificate)),
            PeerId::new(keys[0].public_key().clone()),
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
        !first_requests.is_empty() && first_request_frames.len() == first_requests.len(),
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
        V2LaneWorkEffect::PostLaneBlock { message, .. } => V2LaneWorkEffect::PostLaneBlock {
            peer: adapter.local_peer.clone(),
            message: message.clone(),
        },
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
    let second_requests = adapter.effects.iter().cloned().collect::<Vec<_>>();
    assert!(
        !second_requests.is_empty(),
        "a due retry must re-emit the authenticated request"
    );
    assert_eq!(
        request_frames(&second_requests),
        first_request_frames,
        "retry must preserve the exact peer order and request bytes"
    );
    let second_owner = adapter
        .historical_recovery_requests
        .get(&identity)
        .expect("second request retains the exact owner");
    let second_cadence = second_owner.cadence;
    let retired_request_hash = second_owner.request_hash.clone();
    assert_eq!(second_cadence.retained_attempts, 2);
    assert_eq!(
        second_cadence.next_retry_at,
        late_retry_at
            .checked_add(adapter.limits.historical_recovery_retry_floor)
            .expect("bounded second retry deadline"),
        "the next deadline is anchored at the service turn, not the prior schedule"
    );
    let local_validator = adapter
        .context
        .roster
        .iter()
        .position(|entry| entry.validator == adapter.local_peer)
        .and_then(|index| wire::ValidatorIndex::try_from(index).ok())
        .expect("historical recovery requester belongs to the frozen roster");
    let mut services = service_for_history_context_with_local_validator(
        Arc::clone(&adapter.kura),
        adapter.context.clone(),
        &keys,
        local_validator,
    );
    let block_actor = Arc::new(AtomicBool::new(true));
    let block_actor_for_hook = Arc::clone(&block_actor);
    let admitted = Arc::new(AtomicUsize::new(0));
    let admitted_for_hook = Arc::clone(&admitted);
    services.set_exact_output_admission_hook(move |post, ticket| {
        if block_actor_for_hook.load(Ordering::Acquire) {
            return Err(
                iroha_p2p::network::NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket,
                    rank: 29,
                },
            );
        }
        admitted_for_hook.fetch_add(1, Ordering::Relaxed);
        Ok(())
    });
    let transferred = adapter
        .drain_effects(1)
        .pop()
        .expect("transfer one retry into exact-output ownership");
    let V2LaneWorkEffect::PostLaneBlock { peer, message } = transferred else {
        panic!("historical retry uses lane transport");
    };
    services
        .post_lane_block(peer, message)
        .expect("actor-backpressured retry remains service-owned");
    assert!(
        services
            .has_pending_exact_output()
            .expect("inspect service-owned historical retry")
    );
    let unrelated = second_requests
        .iter()
        .find_map(|effect| match effect {
            V2LaneWorkEffect::PostLaneBlock {
                peer,
                message: BlockMessage::LaneHistoricalRecoveryRequest(request),
            } => {
                let mut request = request.as_ref().clone();
                request.version = request.version.saturating_add(1);
                Some(V2LaneWorkEffect::PostLaneBlock {
                    peer: peer.clone(),
                    message: BlockMessage::LaneHistoricalRecoveryRequest(Box::new(request)),
                })
            }
            _ => None,
        })
        .expect("the retry owns one request effect");
    let unrelated_key = lane_work_effect_key(&unrelated);
    assert!(
        adapter.push_effect(unrelated),
        "an unrelated queued request remains independently owned"
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
    assert_eq!(
        adapter.retired_historical_recovery_request_hashes,
        BTreeSet::from([retired_request_hash]),
        "completion publishes one exact cancellation identity"
    );
    assert_eq!(
        apply_retired_historical_recovery_requests(&mut adapter, &services)
            .expect("cancel service-owned retry before reopening actor admission"),
        1
    );
    block_actor.store(false, Ordering::Release);
    assert!(
        !services
            .retry_pending_exact_output()
            .expect("a retired request leaves no retryable exact output")
    );
    assert_eq!(
        admitted.load(Ordering::Relaxed),
        0,
        "a completed historical request must never reach transport"
    );
    let remaining = adapter.drain_effects(usize::MAX);
    assert_eq!(
        remaining.len(),
        1,
        "completion retires every exact fanout route but preserves unrelated work"
    );
    assert_eq!(lane_work_effect_key(&remaining[0]), unrelated_key);
    assert!(adapter.historical_recovery_waits_snapshot().is_empty());
    assert!(adapter.historical_recovery_requests.is_empty());
    assert!(adapter.historical_recovery_request_owners.is_empty());
    assert!(
        adapter
            .kura
            .lane_block_application_receipt_available(&proposal)
    );
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
    let service =
        service_for_history_context_with_handoff_owner(kura, context, &validators, service_owner);
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
            .accept_certified_merge_sidecar_close(outsider, Some(outsider_route), outsider_close,)
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
            .accept_certified_merge_sidecar_for_test(requester.clone(), requester_route, request,)
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
        posted_sidecar_chunk(effect).is_some_and(|chunk| chunk.requester == first_requester)
    }));
    assert!(!adapter.sidecar_effects.iter().any(|effect| {
        posted_sidecar_chunk(effect).is_some_and(|chunk| chunk.requester == second_requester)
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
    WrongNetwork,
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
    signed_finality_artifact(
        context,
        keys,
        block,
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
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
        vec![0, 1, 2],
        [
            "encode historical sidecar carrier",
            "derive historical sidecar finality preimage",
            "historical signer index",
            "sign historical sidecar finality",
            "aggregate historical sidecar finality",
            "derive historical sidecar finality PoP",
            "historical sidecar finality is cryptographically valid",
        ],
    )
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
        HistoricalSidecarFinality::WrongNetwork => {
            finality_context.network_id =
                crate::sumeragi::synthetic_network_id("wrong-historical-sidecar-network");
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
    let restart = LaneAdapterRestartParts::capture(&adapter);
    let carrier_height = adapter.context.height;
    let requester = adapter
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .find(|peer| peer != &restart.local_peer)
        .expect("historical sidecar fixture has a remote requester");
    drop(adapter);
    let successor = restart
        .reopen(successor_context, true)
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
        responder: restart.local_peer,
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
        posted_sidecar_chunk(effect)
            .is_some_and(|chunk| chunk.entry_hash == fixture.request.entry_hash)
    }));
}
#[test]
fn disjoint_successor_roster_serves_only_exact_historical_requester() {
    let mut fixture =
        historical_sidecar_server_fixture(HistoricalSidecarFinality::Exact, None, false);
    let historical_roster = fixture
        .finality
        .height_context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let mut successor_roster = (0..historical_roster.len())
        .map(|index| {
            let seed = u8::try_from(index)
                .expect("small successor roster index")
                .saturating_add(0xC0);
            wire::ValidatorPower {
                validator: PeerId::new(
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic disjoint sidecar successor")
                        .public_key()
                        .clone(),
                ),
                power: 1,
            }
        })
        .collect::<Vec<_>>();
    successor_roster.sort_by(|left, right| left.validator.cmp(&right.validator));
    assert!(successor_roster.iter().all(|successor| {
        historical_roster
            .iter()
            .all(|historical| historical != &successor.validator)
    }));
    let successor_peers = successor_roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    fixture
        .adapter
        .transition_merge_sidecar_responder_roster_for_test(&successor_peers)
        .expect("persist the disjoint responder-generation fence");
    fixture.adapter.context.roster = successor_roster;
    fixture.adapter.context.quorum = wire::DualQuorum::from_roster(&fixture.adapter.context.roster)
        .expect("disjoint successor has valid equal-vote geometry");
    fixture
        .adapter
        .context
        .validate()
        .expect("disjoint successor context remains valid");
    assert!(!fixture.adapter.frozen_roster_contains(&fixture.requester));
    assert!(
        !fixture
            .adapter
            .frozen_roster_contains(&fixture.adapter.local_peer)
    );
    let current_generation = fixture
        .adapter
        .merge_sidecars
        .server_service_generation_for_test();
    assert!(fixture.request.service_generation < current_generation);
    fixture
        .adapter
        .kura
        .reset_merge_query_read_counters_for_test();
    assert_eq!(
        dispatch_historical_sidecar_request(&mut fixture),
        V2LaneIngressOutcome::Inserted,
        "an exact stale historical request must receive the successor fence"
    );
    assert_eq!(
        fixture.adapter.kura.merge_query_read_counters_for_test(),
        (0, 0, 0),
        "a stale generation is rate-gated and answered without a merge-entry lookup"
    );
    let stale_effects = fixture.adapter.drain_effects(usize::MAX);
    assert!(stale_effects.iter().any(|effect| {
        matches!(
            effect,
            V2LaneWorkEffect::PostCertifiedMergeSidecar { message, .. }
                if matches!(
                    message.as_ref(),
                    CertifiedMergeSidecarMessage::GenerationHint(hint)
                        if hint.current_generation == current_generation
                )
        )
    }));
    assert_eq!(
        fixture
            .adapter
            .merge_sidecars
            .server_stream_count_for_test(),
        0
    );
    assert_eq!(
        fixture
            .adapter
            .merge_sidecars
            .server_request_gate_count_for_test(),
        0,
        "a stale predecessor request must not recreate predecessor ownership"
    );
    for index in 0..wire::MAX_VALIDATORS_PER_HEIGHT {
        let seed = 0xD0_u8
            .checked_add(u8::try_from(index).expect("bounded outsider index"))
            .expect("outsider seed range");
        let outsider = PeerId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic historical sidecar outsider")
                .public_key()
                .clone(),
        );
        assert!(!historical_roster.contains(&outsider));
        assert!(!fixture.adapter.frozen_roster_contains(&outsider));
        let mut outsider_request = fixture.request.clone();
        outsider_request.service_generation = current_generation;
        outsider_request.requester = outsider.clone();
        outsider_request.request_id = outsider_request.canonical_request_id();
        let outsider_hub = PeerId::new(KeyPair::random().public_key().clone());
        let mut outsider_routes = NetworkReplyRouteTestFixture::with_source_capacity(
            outsider_hub.clone(),
            fixture.adapter.limits.reply_source_capacity.get(),
        );
        let outsider_route = outsider_routes.mint_via(outsider.clone(), outsider_hub);
        assert_eq!(
            fixture
                .adapter
                .accept_certified_merge_sidecar_for_test(
                    outsider,
                    outsider_route,
                    outsider_request,
                )
                .expect("an exact-entry outsider is rejected without local failure"),
            V2LaneIngressOutcome::Rejected
        );
    }
    let expected_predecessor_requesters =
        historical_roster.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        fixture.adapter.predecessor_sidecar_requesters.as_ref(),
        Some(&expected_predecessor_requesters),
        "the allocation corridor is bound to the exact durable predecessor"
    );
    assert_eq!(
        fixture
            .adapter
            .merge_sidecars
            .server_stream_count_for_test(),
        0
    );
    assert_eq!(
        fixture
            .adapter
            .merge_sidecars
            .server_request_gate_count_for_test(),
        0
    );
    fixture.request.service_generation = current_generation;
    fixture.request.request_id = fixture.request.canonical_request_id();
    assert_eq!(
        dispatch_historical_sidecar_request(&mut fixture),
        V2LaneIngressOutcome::Inserted,
        "the retried exact request must use the sole successor writer"
    );
    assert_eq!(
        fixture
            .adapter
            .merge_sidecars
            .server_stream_count_for_test(),
        1
    );
    assert_eq!(
        fixture
            .adapter
            .merge_sidecars
            .server_request_gate_count_for_test(),
        1
    );
    assert!(fixture.adapter.sidecar_effects.iter().any(|effect| {
        posted_sidecar_chunk(effect)
            .is_some_and(|chunk| chunk.entry_hash == fixture.request.entry_hash)
    }));
    let mut close = CertifiedMergeSidecarCloseV1 {
        version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
        service_generation: current_generation,
        stream_epoch: fixture.request.stream_epoch,
        closed_through: fixture.request.semantic_sequence.get(),
        close_id: Hash::prehashed([0; Hash::LENGTH]),
        requester: fixture.requester.clone(),
        responder: fixture.adapter.local_peer.clone(),
    };
    close.close_id = close.canonical_close_id();
    let close_hub = PeerId::new(KeyPair::random().public_key().clone());
    let mut close_routes = NetworkReplyRouteTestFixture::with_source_capacity(
        close_hub.clone(),
        fixture.adapter.limits.reply_source_capacity.get(),
    );
    let close_route = close_routes.mint_via(fixture.requester.clone(), close_hub);
    assert_eq!(
        fixture
            .adapter
            .accept_certified_merge_sidecar_close(
                fixture.requester.clone(),
                Some(close_route),
                close,
            )
            .expect("the admitted historical stream can close in the successor generation"),
        V2LaneIngressOutcome::Inserted
    );
    assert_eq!(
        fixture
            .adapter
            .merge_sidecars
            .server_request_gate_count_for_test(),
        0
    );
    assert_eq!(
        fixture
            .adapter
            .merge_sidecars
            .retained_outbound_bytes_for_test(),
        0,
        "the authenticated close releases the bounded response payload"
    );
    // Fill the rest of the complete predecessor committee first, then
    // prove that every disjoint current-roster identity still owns a
    // reserved responder slot. The former roster-sized transport table
    // rejected the live roster at this boundary.
    let mut reservation_routes = NetworkReplyRouteTestFixture::with_source_capacity(
        PeerId::new(KeyPair::random().public_key().clone()),
        fixture.adapter.limits.reply_source_capacity.get(),
    );
    for historical in historical_roster
        .iter()
        .filter(|historical| *historical != &fixture.requester)
    {
        let mut request = fixture.request.clone();
        request.requester = historical.clone();
        request.request_id = request.canonical_request_id();
        let route = reservation_routes.mint(historical.clone());
        let before = fixture
            .adapter
            .merge_sidecars
            .server_stream_count_for_test();
        let outcome = fixture
            .adapter
            .accept_certified_merge_sidecar_for_test(historical.clone(), route, request)
            .expect("another exact predecessor requester remains serviceable");
        assert_ne!(outcome, V2LaneIngressOutcome::Rejected);
        assert_eq!(
            fixture
                .adapter
                .merge_sidecars
                .server_stream_count_for_test(),
            before + 1
        );
    }
    let current_context = &fixture.adapter.context;
    assert_eq!(
        fixture
            .adapter
            .merge_sidecars
            .server_stream_count_matching(|requester| {
                !current_context
                    .roster
                    .iter()
                    .any(|entry| &entry.validator == requester)
            }),
        historical_roster.len()
    );
    for current in &successor_peers {
        let mut request = fixture.request.clone();
        request.requester = current.clone();
        request.request_id = request.canonical_request_id();
        let route = reservation_routes.mint(current.clone());
        let before = fixture
            .adapter
            .merge_sidecars
            .server_stream_count_for_test();
        let outcome = fixture
            .adapter
            .accept_certified_merge_sidecar_for_test(current.clone(), route, request)
            .expect("current-roster reservation remains serviceable");
        assert_ne!(outcome, V2LaneIngressOutcome::Rejected);
        assert_eq!(
            fixture
                .adapter
                .merge_sidecars
                .server_stream_count_for_test(),
            before + 1
        );
    }
    assert_eq!(
        fixture
            .adapter
            .merge_sidecars
            .server_stream_count_for_test(),
        historical_roster.len() + successor_peers.len(),
        "a complete predecessor and disjoint successor fit simultaneously"
    );
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
#[test]
fn decided_mixed_carrier_accepts_canonical_successor_while_local_sidecars_lag() {
    let (mut parent, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
    let autonomous_lane_id = LaneId::new(1);
    let autonomous_dataspace_id = DataSpaceId::new(7);
    enable_multilane_nexus(
        &mut parent,
        &keys,
        autonomous_lane_id,
        autonomous_dataspace_id,
    );
    let autonomous_lane_entry = parent
        .state
        .nexus_snapshot()
        .lane_config
        .entry(autonomous_lane_id)
        .expect("autonomous mixed-carrier lane storage entry")
        .clone();
    parent
        .kura
        .reconcile_lane_segments_for_testing(&[&autonomous_lane_entry], &[], &[])
        .expect("provision autonomous mixed-carrier lane storage");
    let autonomous_lane_incarnation = parent
        .state
        .lane_incarnation_at_height(autonomous_lane_id, parent.context.height)
        .expect("autonomous mixed-carrier lane incarnation");
    parent
        .kura
        .install_lane_incarnation_marker_for_test(
            &autonomous_lane_entry,
            autonomous_lane_incarnation,
            0,
        )
        .expect("install autonomous mixed-carrier lane incarnation marker");
    let (parent_block, parent_proposal) = globally_anchored_lane_block_fixture(&parent, &keys);
    parent
        .kura
        .store_block(parent_block.clone())
        .expect("persist exact raw lane predecessor");
    let parent_finality = verified_finality_artifact_for_block(&parent, &keys, &parent_block);
    let _ = parent
        .kura
        .store_v2_finality_artifact(&parent_finality)
        .expect("persist raw predecessor finality authority");
    let committed_parent = ValidBlock::committed_from_replay_signed_block(parent_block.clone());
    commit_test_block_to_state(parent.state.as_ref(), &committed_parent, &parent.context);
    assert_eq!(
        parent
            .state
            .unapplied_lane_block_artifact_heights_snapshot_cached()
            .get(&(
                parent_proposal.descriptor.lane_id,
                parent_proposal.descriptor.dataspace_id,
            )),
        Some(&parent_proposal.descriptor.lane_block_height),
        "the regression requires a canonical predecessor whose independent lane sidecars are still pending"
    );
    assert!(
        !parent
            .kura
            .lane_block_application_receipt_available(&parent_proposal),
        "the raw canonical predecessor must not already have an application receipt"
    );
    let successor_context = successor_context_for_parent(&parent, &parent_block);
    let restart = LaneAdapterRestartParts::capture(&parent);
    let state = Arc::clone(&restart.state);
    let kura = Arc::clone(&restart.kura);
    drop(parent);
    let mut successor = restart
        .reopen(successor_context, true)
        .expect("open successor while predecessor sidecars remain pending");
    let (autonomous_source_block, mut autonomous_proposal) =
        planned_lane_candidate_block_for_route_at_view(
            &successor,
            &keys,
            0,
            autonomous_lane_id,
            autonomous_dataspace_id,
        );
    autonomous_proposal.payload_block_hint = None;
    let autonomous_entrypoint = autonomous_source_block
        .external_entrypoints_cloned()
        .next()
        .expect("autonomous mixed-carrier entrypoint");
    let (autonomous_payload, autonomous_producer) = signed_autonomous_payload_for_entrypoint(
        &successor,
        &keys,
        &autonomous_proposal,
        autonomous_entrypoint,
        AutonomousAuthorRule::Autonomous,
        b"mixed-raw-successor-queue-plan-admission-binding",
        b"mixed-raw-successor-reservation-owner",
        "deterministic mixed-carrier autonomous producer",
        "mixed-carrier fixture contains its autonomous producer",
        "construct exact autonomous mixed-carrier payload",
    );
    assert_eq!(
        accept_lane_message_from(
            &mut successor,
            BlockMessage::LaneExecutablePayload(autonomous_payload.clone()),
            autonomous_producer,
            0,
        ),
        V2LaneIngressOutcome::Inserted
    );
    let autonomous_envelope = autonomous_lane_payload_envelope(
        &autonomous_payload,
        successor.native_network_id(),
        successor.context.epoch,
    )
    .expect("encode exact autonomous mixed-carrier envelope");
    let mut malformed_autonomous_envelope = autonomous_envelope.clone();
    malformed_autonomous_envelope.proposal_hash =
        Hash::new(b"malformed mixed-carrier autonomous proposal");
    assert!(
        decode_autonomous_lane_payload_envelope(
            &malformed_autonomous_envelope,
            successor.native_network_id(),
            successor.context.epoch,
        )
        .is_err(),
        "the unchanged autonomous envelope validator must reject a malformed mixed-carrier member"
    );
    let transaction_key = KeyPair::try_from_seed(vec![0xD4; 32], Algorithm::Ed25519)
        .expect("deterministic successor transaction key");
    let transaction = TransactionBuilder::new(
        successor.context.network_id,
        AccountId::new(transaction_key.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(transaction_key.private_key());
    let entrypoint_hash = transaction.hash_as_entrypoint();
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let global_view = 0;
    let leader_index = usize::try_from(successor.context.leader(global_view))
        .expect("successor leader index fits usize");
    let global_leader = &successor.context.roster[leader_index].validator;
    let strict = prepare_v2_lane_payload_plan(
        state.as_ref(),
        kura.as_ref(),
        &successor.context,
        global_view,
        global_leader,
        std::slice::from_ref(&route),
        std::slice::from_ref(&Hash::from(entrypoint_hash)),
    )
    .expect("strict producer planning remains deterministic");
    assert_eq!(
        strict.unavailable_indices,
        BTreeSet::from([0]),
        "fresh local production must remain blocked on the missing predecessor sidecars"
    );
    let recovered = prepare_v2_lane_payload_validation_plan(
        state.as_ref(),
        kura.as_ref(),
        &successor.context,
        global_view,
        global_leader,
        std::slice::from_ref(&route),
        std::slice::from_ref(&Hash::from(entrypoint_hash)),
    )
    .expect("derive received ownership from the exact canonical predecessor");
    assert!(recovered.unavailable_indices.is_empty());
    assert_eq!(recovered.ownerships.len(), 1);
    assert_eq!(
        recovered.ownerships[0].previous_lane_block_height,
        parent_proposal.descriptor.lane_block_height
    );
    assert_eq!(
        recovered.ownerships[0].previous_lane_block_descriptor_hash,
        Some(parent_proposal.descriptor.descriptor_hash)
    );
    let header = BlockHeader::new(
        NonZeroU64::new(successor.context.height).expect("non-zero successor height"),
        Some(parent_block.hash()),
        None,
        None,
        successor.context.height,
        global_view,
    );
    let mut builder = BlockBuilder::new(header);
    builder.push_transaction(transaction);
    builder.set_execution_context(Some(
        BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
            entrypoint_hash,
            route.lane_id,
            route.dataspace_id,
        )])
        .with_lane_payload_ownerships(recovered.ownerships)
        .with_autonomous_lane_payloads(vec![autonomous_envelope]),
    ));
    let successor_block = builder
        .build_with_signature(
            u64::try_from(leader_index).expect("successor leader index fits u64"),
            keys[leader_index].private_key(),
        )
        .canonical_resultless_proposal();
    let successor_ownership = successor_block
        .execution_context()
        .expect("successor execution context")
        .lane_payload_ownerships[0]
        .clone();
    let successor_proposal = proposal_from_ownership(&successor_ownership, successor_block.hash())
        .expect("reconstruct exact raw successor proposal");
    let mut wrong_raw_successor = successor_proposal.clone();
    wrong_raw_successor
        .descriptor
        .previous_lane_block_descriptor_hash =
        Some(Hash::new(b"wrong canonical raw predecessor descriptor"));
    assert!(
        !canonical_raw_lane_predecessor_matches_proposal(
            successor.state.as_ref(),
            successor.kura.as_ref(),
            &wrong_raw_successor,
        ),
        "raw fallback must reject a mismatched predecessor descriptor"
    );
    let (locked_round, locked_subject) =
        mark_global_body_locked_for_block(&mut successor, &successor_block);
    assert_ne!(
        successor.bind_locked_global_body(&successor_block),
        V2LaneIngressOutcome::Rejected,
        "an exact PrepareQC-locked successor must not kill a validator merely because its independent predecessor sidecars are catching up"
    );
    assert!(!successor.proposal_can_progress(&successor_proposal));
    assert!(
        successor
            .lane_sessions
            .local_vote_rebroadcast_artifacts_for(&successor.local_peer)
            .iter()
            .all(|(proposal, _)| proposal != &successor_proposal),
        "raw predecessor authentication must not authorize a local successor vote"
    );
    let _ = successor.drain_effects(usize::MAX);
    let mut executed_successor_block = successor_block.clone();
    executed_successor_block
        .set_transaction_results(
            Vec::new(),
            &[entrypoint_hash],
            vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
        )
        .expect("attach canonical successor transaction result");
    assert_eq!(
        executed_successor_block.canonical_resultless_proposal(),
        successor_block,
        "execution results must preserve the exact locked successor proposal"
    );
    successor
        .kura
        .store_block(executed_successor_block.clone())
        .expect("persist exact raw successor carrier");
    let successor_finality =
        verified_finality_artifact_for_block(&successor, &keys, &executed_successor_block);
    let successor_receipt = KuraV2CommitReceipt::for_test(&successor_finality);
    let committed_successor =
        ValidBlock::committed_from_replay_signed_block(executed_successor_block.clone());
    commit_test_block_to_state(
        successor.state.as_ref(),
        &committed_successor,
        &successor.context,
    );
    assert!(
        !canonical_v2_lane_payload_matches_kura(
            successor.state.as_ref(),
            successor.kura.as_ref(),
            &successor.context,
            &successor_block,
        ),
        "the strict canonical matcher must retain applied-predecessor semantics"
    );
    assert!(canonical_v2_lane_payload_matches_kura_inner(
        successor.state.as_ref(),
        successor.kura.as_ref(),
        &successor.context,
        &successor_block,
        true,
    ));
    successor
        .retain_merge_sidecars_for_global_view(
            locked_round.view,
            Some(locked_subject),
            Some(locked_subject),
        )
        .expect("install exact raw-successor Decision");
    assert_ne!(
        successor
            .recover_decided_canonical_lane_body(&successor_receipt, &successor_finality,)
            .expect("recover decided carrier over exact raw predecessor"),
        V2LaneIngressOutcome::Rejected
    );
    assert!(
        successor
            .lane_sessions
            .proposals_without_commit_qc()
            .contains(&parent_proposal),
        "decided recovery hydrates the oldest exact raw predecessor"
    );
    assert!(
        successor
            .lane_sessions
            .proposals_without_commit_qc()
            .contains(&successor_proposal),
        "decided recovery retains the exact raw successor"
    );
    let successor_session = committed_lane_session(&successor_proposal, &keys[..3]);
    successor
        .pending_committed_lanes
        .push_back(successor_session.clone());
    assert_eq!(
        successor
            .persist_anchored_sessions()
            .expect("defer successor certificate behind raw predecessor"),
        0
    );
    assert!(
        certified_artifact(&successor, &successor_proposal).is_none(),
        "the successor certificate must not become durable before its predecessor receipt"
    );
    assert!(
        !successor
            .kura
            .lane_block_application_receipt_available(&successor_proposal)
    );
    successor
        .schedule_retransmission()
        .expect("solicit raw predecessor certificate without consensus progress");
    let effects = successor.drain_effects(usize::MAX);
    assert!(
        effects
            .iter()
            .any(|effect| posted_lane_proposal(effect) == Some(&parent_proposal))
    );
    assert!(effects.iter().all(|effect| match effect {
        V2LaneWorkEffect::PostLaneBlock {
            message: BlockMessage::LaneBlockVote(vote),
            ..
        } => vote.body.proposal_hash != successor_proposal.proposal_hash,
        V2LaneWorkEffect::PostLaneBlock {
            message: BlockMessage::LaneBlockQc(qc),
            ..
        } => qc.body.proposal_hash != successor_proposal.proposal_hash,
        V2LaneWorkEffect::PostDurableLaneCertificate { certificate, .. } => {
            certificate.proposal.proposal_hash != successor_proposal.proposal_hash
        }
        _ => true,
    }));
    let parent_certificate = LaneBlockCertificateV1 {
        proposal: parent_proposal.clone(),
        prepare_qc: lane_qc_for_phase(&parent_proposal, &keys[..3], CertPhase::Prepare),
        commit_qc: lane_qc_for_phase(&parent_proposal, &keys[..3], CertPhase::Commit),
    };
    assert_eq!(
        accept_lane_message_from(
            &mut successor,
            BlockMessage::LaneBlockCertificate(Box::new(parent_certificate)),
            PeerId::new(keys[1].public_key().clone()),
            locked_round.view,
        ),
        V2LaneIngressOutcome::Inserted
    );
    assert!(matches!(
        successor
            .service_next_historical_recovery()
            .expect("persist exact predecessor certificate and receipt"),
        HistoricalRecoveryServiceOutcome::Complete(_)
    ));
    assert!(
        successor
            .kura
            .lane_block_application_receipt_available(&parent_proposal)
    );
    assert!(successor.proposal_can_progress(&successor_proposal));
    assert!(
        successor
            .lane_sessions
            .local_vote_rebroadcast_artifacts_for(&successor.local_peer)
            .iter()
            .any(|(proposal, _)| proposal == &successor_proposal),
        "predecessor receipt completion wakes the retained successor session"
    );
    let resumed_effects = successor.drain_effects(usize::MAX);
    assert!(
        resumed_effects.iter().any(|effect| {
            posted_lane_vote(effect)
                .is_some_and(|vote| vote.body == successor_proposal.vote_body(CertPhase::Prepare))
        }),
        "the unblocked exact H2 Prepare vote must cross the decided-carrier fanout gate"
    );
    assert_eq!(
        successor
            .persist_anchored_sessions()
            .expect("persist unblocked successor certificate and receipt"),
        1
    );
    assert!(
        successor
            .kura
            .lane_block_application_receipt_available(&successor_proposal)
    );
}
#[test]
fn cold_restart_hydrates_two_link_raw_lane_chain_without_receipts() {
    let (first, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
    let (first_block, first_proposal) = globally_anchored_lane_block_fixture(&first, &keys);
    first
        .kura
        .store_block(first_block.clone())
        .expect("persist first raw lane artifact");
    let committed_first = ValidBlock::committed_from_replay_signed_block(first_block.clone());
    commit_test_block_to_state(first.state.as_ref(), &committed_first, &first.context);
    assert!(
        first
            .kura
            .read_lane_block_application_receipt_without_sidecar_repair(
                first_proposal.descriptor.lane_id,
                first_proposal.descriptor.lane_block_height,
            )
            .is_none(),
        "the first raw artifact must not gain an application receipt"
    );
    let second_context = successor_context_for_parent(&first, &first_block);
    let mut restart = LaneAdapterRestartParts::capture(&first);
    restart.limits.session_capacity = NonZeroUsize::new(2).expect("two-link hydration bound");
    let state = Arc::clone(&restart.state);
    let kura = Arc::clone(&restart.kura);
    drop(first);
    let second = restart
        .reopen(second_context, true)
        .expect("open second height over an unreceipted raw predecessor");
    let transaction_key = KeyPair::try_from_seed(vec![0xD5; 32], Algorithm::Ed25519)
        .expect("deterministic second-link transaction key");
    let transaction = TransactionBuilder::new(
        second.context.network_id,
        AccountId::new(transaction_key.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(transaction_key.private_key());
    let entrypoint_hash = transaction.hash_as_entrypoint();
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let global_view = 0;
    let leader_index = usize::try_from(second.context.leader(global_view))
        .expect("second-link leader index fits usize");
    let global_leader = &second.context.roster[leader_index].validator;
    let strict = prepare_v2_lane_payload_plan(
        state.as_ref(),
        kura.as_ref(),
        &second.context,
        global_view,
        global_leader,
        std::slice::from_ref(&route),
        std::slice::from_ref(&Hash::from(entrypoint_hash)),
    )
    .expect("strict second-link producer planning remains deterministic");
    assert_eq!(strict.unavailable_indices, BTreeSet::from([0]));
    let recovered = prepare_v2_lane_payload_validation_plan(
        state.as_ref(),
        kura.as_ref(),
        &second.context,
        global_view,
        global_leader,
        std::slice::from_ref(&route),
        std::slice::from_ref(&Hash::from(entrypoint_hash)),
    )
    .expect("recover second-link ownership from the exact raw predecessor");
    assert!(recovered.unavailable_indices.is_empty());
    assert_eq!(recovered.ownerships.len(), 1);
    assert_eq!(
        recovered.ownerships[0].previous_lane_block_height,
        first_proposal.descriptor.lane_block_height
    );
    assert_eq!(
        recovered.ownerships[0].previous_lane_block_descriptor_hash,
        Some(first_proposal.descriptor.descriptor_hash)
    );
    let header = BlockHeader::new(
        NonZeroU64::new(second.context.height).expect("non-zero second-link height"),
        Some(first_block.hash()),
        None,
        None,
        second.context.height,
        global_view,
    );
    let mut builder = BlockBuilder::new(header);
    builder.push_transaction(transaction);
    builder.set_execution_context(Some(
        BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
            entrypoint_hash,
            route.lane_id,
            route.dataspace_id,
        )])
        .with_lane_payload_ownerships(recovered.ownerships),
    ));
    let second_block = builder
        .build_with_signature(
            u64::try_from(leader_index).expect("second-link leader index fits u64"),
            keys[leader_index].private_key(),
        )
        .canonical_resultless_proposal();
    let second_ownership = second_block
        .execution_context()
        .expect("second-link block carries execution context")
        .lane_payload_ownerships[0]
        .clone();
    let second_proposal = proposal_from_ownership(&second_ownership, second_block.hash())
        .expect("reconstruct exact second raw proposal");
    second
        .kura
        .store_block(second_block.clone())
        .expect("persist second raw lane artifact");
    let committed_second = ValidBlock::committed_from_replay_signed_block(second_block.clone());
    commit_test_block_to_state(second.state.as_ref(), &committed_second, &second.context);
    assert!(canonical_v2_lane_payload_matches_kura_inner(
        second.state.as_ref(),
        second.kura.as_ref(),
        &second.context,
        &second_block,
        true,
    ));
    assert!(canonical_raw_lane_predecessor_matches_proposal(
        second.state.as_ref(),
        second.kura.as_ref(),
        &second_proposal,
    ));
    assert_eq!(
        second
            .state
            .unapplied_lane_block_artifact_heights_snapshot_cached()
            .get(&(route.lane_id, route.dataspace_id)),
        Some(&second_proposal.descriptor.lane_block_height)
    );
    for proposal in [&first_proposal, &second_proposal] {
        assert!(
            second
                .kura
                .read_lane_block_application_receipt_without_sidecar_repair(
                    proposal.descriptor.lane_id,
                    proposal.descriptor.lane_block_height,
                )
                .is_none(),
            "neither raw link may gain an application receipt before restart"
        );
    }
    let third_context = successor_context_for_parent(&second, &second_block);
    drop(second);
    let third = restart
        .reopen(third_context, true)
        .expect("cold-open the third height over a two-link raw chain");
    assert!(
        !third.output_guard.restart_required() && third.output_guard.acquire().is_some(),
        "bounded raw-chain hydration must leave authoritative admission healthy"
    );
    assert_eq!(
        third.lane_sessions.proposals_without_commit_qc(),
        vec![first_proposal.clone(), second_proposal.clone()],
        "cold hydration must reconstruct the exact raw chain oldest first"
    );
    for proposal in [&first_proposal, &second_proposal] {
        assert!(third.canonical_anchor_for_proposal(proposal).is_some());
        assert!(third.historical_raw_proposal_can_solicit_certificate(proposal));
        assert!(!third.proposal_can_progress(proposal));
        assert!(
            third
                .kura
                .read_lane_block_application_receipt_without_sidecar_repair(
                    proposal.descriptor.lane_id,
                    proposal.descriptor.lane_block_height,
                )
                .is_none()
        );
    }
    assert!(
        third
            .lane_sessions
            .local_vote_rebroadcast_artifacts_for(&third.local_peer)
            .iter()
            .all(|(proposal, _)| { proposal != &first_proposal && proposal != &second_proposal }),
        "cold hydration must not mint votes for either unreceipted raw link"
    );
}
