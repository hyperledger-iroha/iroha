use crate::{
    queue::LaneQueueReservationReconciliationGroupV1,
    sumeragi::v2_apply::{
        HistoricalAutonomousLaneRecoveryInstallOutcome, HistoricalAutonomousReservationInstallV1,
        install_historical_autonomous_lane_recovery,
    },
};

#[test]
fn local_native_amx_signer_rejects_conflicting_claim_for_one_leg_phase() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let request = native_request(&adapter, &keys);
    let first = adapter
        .sign_native_request_once(&request, 0)
        .expect("first exact request may be signed");
    let retransmission = adapter
        .sign_native_request_once(&request, 0)
        .expect("an exact retransmission is idempotently signable");
    assert_eq!(first, retransmission);
    assert_eq!(
        adapter
            .native_signing_guard
            .as_ref()
            .expect("validator has durable Native AMX guard")
            .record_count_for_test(),
        1,
        "the exact retransmission must reuse one durable signing decision"
    );
    adapter.local_native_claims.clear();
    let conflicting = native_request_with_entrypoint(
        request.clone(),
        HashOf::from_untyped_unchecked(Hash::new(b"conflicting entrypoint")),
    );
    assert_eq!(conflicting.validate_plan_binding(), Ok(()));
    assert!(
        adapter.sign_native_request_once(&conflicting, 0).is_none(),
        "an honest adapter must not sign a second request for one round/session/leg/phase"
    );
    assert!(
        adapter.local_native_claims.is_empty(),
        "durable rejection must survive loss of the volatile fast-path claim"
    );
    assert_eq!(
        adapter
            .native_signing_guard
            .as_ref()
            .expect("validator has durable Native AMX guard")
            .record_count_for_test(),
        1,
        "a rejected request conflict must precede durable journal mutation"
    );
    let mut commit = request;
    commit.body.phase = NativeAmxPhase::Commit;
    assert_eq!(commit.validate_plan_binding(), Ok(()));
    assert!(
        adapter.sign_native_request_once(&commit, 0).is_some(),
        "Prepare and Commit are distinct durable claims"
    );
    assert_eq!(
        adapter
            .native_signing_guard
            .as_ref()
            .expect("validator has durable Native AMX guard")
            .record_count_for_test(),
        2
    );
}
#[test]
fn native_amx_signing_guard_reopens_same_height_without_losing_claims() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let request = native_request(&adapter, &keys);
    assert_eq!(request.validate_plan_binding(), Ok(()));
    assert!(adapter.native_request_matches_context(&request, 0));
    let first = adapter
        .sign_native_request_once(&request, 0)
        .expect("first full request is durably signable");
    let context = adapter.context.clone();
    let local_peer = adapter.local_peer.clone();
    let key_pair = adapter.key_pair.clone();
    let state = Arc::clone(&adapter.state);
    let kura = Arc::clone(&adapter.kura);
    let limits = adapter.limits;
    drop(adapter);
    let mut reopened = V2LaneWorkAdapter::new_with_output_guard(
        context,
        local_peer,
        key_pair,
        true,
        state,
        kura,
        limits,
        None,
        None,
        ConsensusOutputGuard::isolated(),
    )
    .expect("reopen adapter against the exact durable height context");
    assert_eq!(
        reopened
            .sign_native_request_once(&request, 0)
            .expect("exact full-request durable replay remains signable"),
        first
    );
    reopened.local_native_claims.clear();
    let conflicting = native_request_with_entrypoint(
        request,
        HashOf::from_untyped_unchecked(Hash::new(b"restart-conflicting-entrypoint")),
    );
    assert_eq!(conflicting.validate_plan_binding(), Ok(()));
    assert!(
        reopened.native_request_matches_context(&conflicting, 0),
        "the conflicting request must reach the reopened durable claim guard"
    );
    assert!(
        reopened.sign_native_request_once(&conflicting, 0).is_none(),
        "the reopened production signer must reject a conflicting durable claim"
    );
    assert_eq!(
        reopened
            .native_signing_guard
            .as_ref()
            .expect("reopened validator has durable guard")
            .record_count_for_test(),
        1
    );
}
#[test]
fn unsafe_native_amx_signing_journal_latches_consensus_fail_stop() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let request = native_request(&adapter, &keys);
    adapter
        .sign_native_request_once(&request, 0)
        .expect("seed one durable signing decision");
    adapter.local_native_claims.clear();
    adapter
        .native_signing_guard
        .as_ref()
        .expect("validator has durable guard")
        .remove_one_record_for_test();
    assert!(adapter.sign_native_request_once(&request, 0).is_none());
    assert!(adapter.output_guard.restart_required());
    assert!(
        adapter.sign_native_request_once(&request, 0).is_none(),
        "a poisoned process must never sign again"
    );
}
include!("v2_lane_work_effect_queue.rs");
#[test]
fn planner_view_one_binds_rotated_global_leader_to_fresh_lane_view() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let global_view = 1;
    let leader_index =
        usize::try_from(adapter.context.leader(global_view)).expect("leader index fits usize");
    adapter.local_peer = adapter.context.roster[leader_index].validator.clone();
    adapter.key_pair = keys[leader_index].clone();
    let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, global_view);
    let ownership = &block
        .execution_context()
        .expect("planned block carries its execution context")
        .lane_payload_ownerships[0];
    assert_eq!(ownership.proposal_view, global_view);
    assert_eq!(ownership.lane_block_view, 0);
    assert_eq!(proposal.descriptor.lane_block_view, 0);
    assert_eq!(
        proposal
            .payload_block_hint
            .expect("planned proposal carries its global block hint")
            .proposal_view,
        global_view
    );
    let round = wire::ConsensusRound {
        context_id: adapter.context.id(),
        height: adapter.context.height,
        view: global_view,
    };
    adapter
        .planned_lane_proposals
        .insert(round, vec![proposal.clone()]);
    assert_eq!(
        adapter.bind_local_candidate(round, block.hash()),
        V2LaneIngressOutcome::Inserted,
        "the rotated global leader must bind a fresh lane-local proposal"
    );
    let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
    assert_eq!(
        adapter.mark_global_body_locked(locked_round, locked_subject),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_ne!(
        adapter.bind_locked_global_body(&block),
        V2LaneIngressOutcome::Rejected,
        "the exact locked body keeps its global view independently of the lane-local view"
    );
    let expected_hint = proposal
        .payload_block_hint
        .expect("planned proposal carries its global block hint");
    assert_eq!(
        adapter
            .locally_bound_lane_proposals
            .get(&proposal.proposal_hash),
        Some(&expected_hint)
    );
    let mut tampered = proposal;
    tampered
        .payload_block_hint
        .as_mut()
        .expect("planned proposal carries its global block hint")
        .proposal_view = 0;
    let forged_sender_index =
        usize::try_from(adapter.context.leader(0)).expect("leader index fits usize");
    let forged_sender = adapter.context.roster[forged_sender_index]
        .validator
        .clone();
    assert_eq!(
        adapter.insert_lane_proposal(tampered, Some(&forged_sender), false, global_view),
        V2LaneIngressOutcome::Rejected,
        "an advisory hint must exactly match the authenticated locked-body binding"
    );
}
#[test]
fn enabled_nexus_binds_independent_lane_author_distinct_from_global_leader() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    let lane_validators = enable_multilane_nexus(&mut adapter, &keys, lane_id, dataspace_id);
    let lane_author = lane_validators
        .first()
        .expect("enabled lane committee is non-empty")
        .clone();
    let global_view = (0..u64::try_from(adapter.context.roster.len())
        .expect("roster length fits u64"))
        .find(|view| {
            let leader_index =
                usize::try_from(adapter.context.leader(*view)).expect("leader index fits usize");
            adapter.context.roster[leader_index].validator != lane_author
        })
        .expect("rotating global roster contains a leader distinct from the lane author");
    let global_leader_index =
        usize::try_from(adapter.context.leader(global_view)).expect("leader index fits usize");
    let global_leader = adapter.context.roster[global_leader_index]
        .validator
        .clone();
    adapter.local_peer = global_leader.clone();
    adapter.key_pair = keys[global_leader_index].clone();
    let (block, proposal) = planned_lane_candidate_block_for_route_at_view(
        &adapter,
        &keys,
        global_view,
        lane_id,
        dataspace_id,
    );
    assert_eq!(lane_proposal_author(&proposal), Some(&lane_author));
    assert_ne!(lane_author, global_leader);
    assert_eq!(proposal.descriptor.lane_block_view, 0);
    assert_eq!(
        proposal
            .payload_block_hint
            .expect("planned proposal carries its global block hint")
            .proposal_view,
        global_view
    );
    let round = wire::ConsensusRound {
        context_id: adapter.context.id(),
        height: adapter.context.height,
        view: global_view,
    };
    adapter
        .planned_lane_proposals
        .insert(round, vec![proposal.clone()]);
    assert_eq!(
        adapter.bind_local_candidate(round, block.hash()),
        V2LaneIngressOutcome::Inserted,
        "the global leader may bind work authored by the independent lane rotation"
    );
    let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
    assert_eq!(
        adapter.mark_global_body_locked(locked_round, locked_subject),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_ne!(
        adapter.bind_locked_global_body(&block),
        V2LaneIngressOutcome::Rejected,
        "the exact global lock must not require the independent lane author to be its leader"
    );
    adapter
        .kura
        .store_block(block.clone())
        .expect("persist exact enabled-Nexus recovery body");
    assert!(canonical_v2_lane_payload_matches_kura(
        adapter.state.as_ref(),
        adapter.kura.as_ref(),
        &adapter.context,
        &block,
    ));
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
    let local_peer = parent.local_peer.clone();
    let local_key = parent.key_pair.clone();
    let state = Arc::clone(&parent.state);
    let kura = Arc::clone(&parent.kura);
    let limits = parent.limits;
    drop(parent);
    let mut successor = V2LaneWorkAdapter::new(
        successor_context,
        local_peer,
        local_key,
        true,
        Arc::clone(&state),
        Arc::clone(&kura),
        limits,
        None,
    )
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
    let autonomous_accepted = crate::tx::AcceptedTransaction::new_unchecked_entrypoint(
        std::borrow::Cow::Owned(autonomous_entrypoint.clone()),
    );
    let autonomous_routing_plan = RoutingPlan::single(RoutingDecision::new(
        autonomous_lane_id,
        autonomous_dataspace_id,
    ));
    let mut autonomous_reservation = crate::queue::LaneQueueReservationKeyV2 {
        version: crate::queue::LaneQueueReservationKeyV2::VERSION,
        signed_transaction_hash: autonomous_accepted.hash(),
        entrypoint_hash: autonomous_entrypoint.hash(),
        queue_plan_admission_binding_hash: Hash::new(
            b"mixed-raw-successor-queue-plan-admission-binding",
        ),
        routing_plan_digest: autonomous_routing_plan.digest(),
        coordinator_leg: autonomous_routing_plan.coordinator_leg(),
        lane_id: autonomous_lane_id,
        dataspace_id: autonomous_dataspace_id,
        lane_incarnation: autonomous_proposal.descriptor.lane_incarnation,
        proposal_height: autonomous_proposal.descriptor.proposal_height,
        lane_block_height: autonomous_proposal.descriptor.lane_block_height,
        lane_block_view: autonomous_proposal.descriptor.lane_block_view,
        reservation_owner_hash: Hash::new(b"mixed-raw-successor-reservation-owner"),
        proposal_identity_hash: autonomous_proposal.proposal_hash,
    };
    let autonomous_producer = successor
        .expected_autonomous_lane_author(&autonomous_proposal)
        .expect("deterministic mixed-carrier autonomous producer")
        .clone();
    bind_canonical_autonomous_reservation_identity(
        &successor,
        &autonomous_proposal,
        &autonomous_producer,
        &mut autonomous_reservation,
    );
    let autonomous_producer_key = keys
        .iter()
        .find(|key| key.public_key() == autonomous_producer.public_key())
        .expect("mixed-carrier fixture contains its autonomous producer");
    let autonomous_payload = LaneExecutablePayloadV1::new_signed_with_reservations(
        successor.native_network_id(),
        successor.context.epoch,
        autonomous_proposal.clone(),
        vec![autonomous_entrypoint],
        vec![autonomous_reservation],
        vec![autonomous_routing_plan],
        vec![None],
        autonomous_producer.clone(),
        autonomous_producer_key.private_key(),
    )
    .expect("construct exact autonomous mixed-carrier payload");
    assert_eq!(
        successor.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneExecutablePayload(autonomous_payload.clone()),
                Some(autonomous_producer),
            ),
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
    let (locked_round, locked_subject) = global_lock_for_block(&successor, &successor_block);
    assert_eq!(
        successor.mark_global_body_locked(locked_round, locked_subject),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
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
    let successor_session = CommittedLaneBlockSession {
        proposal: successor_proposal.clone(),
        prepare_qc: lane_qc_for_phase(&successor_proposal, &keys[..3], CertPhase::Prepare),
        commit_qc: lane_qc_for_phase(&successor_proposal, &keys[..3], CertPhase::Commit),
    };
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
        successor
            .kura
            .read_certified_lane_block_artifact(
                successor_proposal.descriptor.lane_id,
                successor_proposal.descriptor.lane_block_height,
            )
            .is_none(),
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
    assert!(effects.iter().any(|effect| {
        matches!(
            effect,
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockProposal(proposal),
                ..
            } if proposal == &parent_proposal
        )
    }));
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
        successor.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneBlockCertificate(Box::new(parent_certificate)),
                Some(PeerId::new(keys[1].public_key().clone())),
            ),
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
            matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneBlockVote(vote),
                    ..
                } if vote.body == successor_proposal.vote_body(CertPhase::Prepare)
            )
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
    let local_peer = first.local_peer.clone();
    let local_key = first.key_pair.clone();
    let state = Arc::clone(&first.state);
    let kura = Arc::clone(&first.kura);
    let mut limits = first.limits;
    limits.session_capacity = NonZeroUsize::new(2).expect("two-link hydration bound");
    drop(first);
    let second = V2LaneWorkAdapter::new(
        second_context,
        local_peer.clone(),
        local_key.clone(),
        true,
        Arc::clone(&state),
        Arc::clone(&kura),
        limits,
        None,
    )
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
    let third = V2LaneWorkAdapter::new(
        third_context,
        local_peer,
        local_key,
        true,
        state,
        kura,
        limits,
        None,
    )
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
#[test]
fn canonical_kura_recovery_accepts_global_view_one_with_fresh_lane_view() {
    let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
    let global_view = 1;
    let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, global_view);
    assert_eq!(block.header().view_change_index(), global_view);
    assert_eq!(proposal.descriptor.lane_block_view, 0);
    adapter
        .kura
        .store_block(block.clone())
        .expect("persist planner-produced canonical recovery body");
    assert!(canonical_v2_lane_payload_matches_kura(
        adapter.state.as_ref(),
        adapter.kura.as_ref(),
        &adapter.context,
        &block,
    ));
    assert!(
        adapter.canonical_anchor_for_proposal(&proposal).is_some(),
        "the exact ownership/header global view must authenticate the lane-local proposal"
    );
}
#[test]
fn canonical_kura_recovery_rejects_nonzero_planner_origin_lane_view() {
    let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
    let (planned, _) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    let mut ownership = planned
        .execution_context()
        .expect("planned block carries its execution context")
        .lane_payload_ownerships[0]
        .clone();
    ownership.lane_block_view = 1;
    let replay = ownership
        .compute_replay_hashes()
        .expect("nonzero lane-view ownership replay material recomputes");
    ownership.subject_hash = replay.subject_hash;
    ownership.payload_ownership_hash = replay.payload_ownership_hash;
    ownership.rbc_instance_hash = replay.rbc_instance_hash;
    ownership.lane_block_descriptor_hash = Some(replay.lane_block_descriptor_hash);
    ownership
        .validate_replay_material()
        .expect("nonzero lane-view fixture must not rely on stale replay hashes");
    let leader_index = usize::try_from(adapter.context.leader(0)).expect("leader index");
    let block = test_block(1, None, Some(ownership), &keys[leader_index]);
    adapter
        .kura
        .store_block(block.clone())
        .expect("persist adversarial nonzero lane-view body");
    assert!(
        !canonical_v2_lane_payload_matches_kura(
            adapter.state.as_ref(),
            adapter.kura.as_ref(),
            &adapter.context,
            &block,
        ),
        "canonical recovery must enforce the planner-origin lane-view invariant"
    );
}
#[test]
fn lane_work_stays_quiescent_until_the_exact_global_prepare_lock() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let later_view =
        u64::try_from(adapter.context.roster.len()).expect("fixture roster length fits u64");
    assert_eq!(
        adapter.context.leader(0),
        adapter.context.leader(later_view)
    );
    let (block_zero, proposal_at_view_zero) =
        planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    let round_zero = wire::ConsensusRound {
        context_id: adapter.context.id(),
        height: adapter.context.height,
        view: 0,
    };
    adapter
        .planned_lane_proposals
        .insert(round_zero, vec![proposal_at_view_zero.clone()]);
    assert_eq!(
        adapter.bind_local_candidate(round_zero, block_zero.hash()),
        V2LaneIngressOutcome::Inserted
    );
    adapter
        .schedule_retransmission()
        .expect("schedule pre-lock retransmission");
    assert!(
        adapter.drain_effects(usize::MAX).is_empty(),
        "local Prepare intent must not leak lane proposals or votes before PrepareQC"
    );
    assert!(adapter.lane_sessions.commit_vote_lock_slots().is_empty());
    let (later_block, proposal_at_later_view) =
        planned_lane_candidate_block_at_view(&adapter, &keys, later_view);
    assert_ne!(
        proposal_at_view_zero.proposal_hash,
        proposal_at_later_view.proposal_hash
    );
    assert_eq!(proposal_at_later_view.descriptor.lane_block_view, 0);
    assert_eq!(
        proposal_at_later_view
            .payload_block_hint
            .expect("replanned proposal carries its global block hint")
            .proposal_view,
        later_view,
        "a full global-leader rotation must not advance the fresh lane-local view"
    );
    let later_round = wire::ConsensusRound {
        context_id: adapter.context.id(),
        height: adapter.context.height,
        view: later_view,
    };
    adapter
        .planned_lane_proposals
        .insert(later_round, vec![proposal_at_later_view.clone()]);
    assert_eq!(
        adapter.bind_local_candidate(later_round, later_block.hash()),
        V2LaneIngressOutcome::Inserted,
        "a later global view must remain free to replan before any PrepareQC lock"
    );
    assert_eq!(
        adapter.bind_locked_global_body(&block_zero),
        V2LaneIngressOutcome::Rejected,
        "a validated body alone is insufficient without the reducer lock"
    );
    let (locked_round, locked_subject) = global_lock_for_block(&adapter, &later_block);
    assert_eq!(
        adapter.mark_global_body_locked(locked_round, locked_subject),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_eq!(
        adapter.bind_locked_global_body(&block_zero),
        V2LaneIngressOutcome::Rejected,
        "a stale body must not satisfy the exact locked subject"
    );
    adapter
        .schedule_retransmission()
        .expect("schedule locked-body retransmission");
    assert!(
        adapter.drain_effects(usize::MAX).is_empty(),
        "the lock without its exact durable body must not release lane work"
    );
    assert_ne!(
        adapter.bind_locked_global_body(&later_block),
        V2LaneIngressOutcome::Rejected
    );
    let effects = adapter.drain_effects(usize::MAX);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        V2LaneWorkEffect::PostLaneBlock {
            message: BlockMessage::LaneBlockProposal(proposal),
            ..
        } if proposal.proposal_hash == proposal_at_later_view.proposal_hash
    )));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        V2LaneWorkEffect::PostLaneBlock {
            message: BlockMessage::LaneBlockVote(vote),
            ..
        } if vote.body.proposal_hash == proposal_at_later_view.proposal_hash
    )));
    assert!(!effects.iter().any(|effect| matches!(
        effect,
        V2LaneWorkEffect::PostLaneBlock {
            message: BlockMessage::LaneBlockProposal(proposal),
            ..
        } if proposal.proposal_hash == proposal_at_view_zero.proposal_hash
    )));
}
#[test]
fn global_body_lock_replacement_requires_higher_prepare_round_and_exact_subject() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let (block, losing_proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
    let (_, subject_a) = global_lock_for_block(&adapter, &block);
    let block_hash = subject_a.block_hash;
    let subject_a = wire::BlockSubject {
        payload_hash: Hash::new(b"global lock payload A"),
        ..subject_a
    };
    let subject_b = wire::BlockSubject {
        payload_hash: Hash::new(b"global lock payload B"),
        ..subject_a
    };
    let context_id = adapter.context.id();
    let height = adapter.context.height;
    let round = |view| wire::ConsensusRound {
        context_id,
        height,
        view,
    };
    assert_eq!(
        adapter
            .lane_sessions
            .insert_proposal(losing_proposal.clone()),
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    );
    assert_eq!(
        adapter.mark_global_body_locked(round(0), subject_a),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_eq!(
        adapter.mark_global_body_locked(round(0), subject_a),
        Ok(GlobalBodyLockOutcome::Duplicate)
    );
    assert!(matches!(
        adapter.mark_global_body_locked(round(0), subject_b),
        Err(V2LaneWorkError::ConflictingGlobalBodyLock)
    ));
    assert_eq!(
        adapter.globally_locked_body,
        Some(GlobalBodyLock {
            round: round(0),
            subject: subject_a,
        })
    );
    adapter
        .pending_local_lane_proposals
        .insert(block_hash, Vec::new());
    adapter.locally_bound_lane_proposals.insert(
        Hash::new(b"losing local lane proposal"),
        LaneBlockProposalPayloadHintV1 {
            proposal_height: adapter.context.height,
            proposal_view: 0,
            proposal_block_hash: block_hash,
        },
    );
    assert_eq!(
        adapter.mark_global_body_locked(round(1), subject_b),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_eq!(
        adapter.globally_locked_body,
        Some(GlobalBodyLock {
            round: round(1),
            subject: subject_b,
        }),
        "same block hash with different payload is a distinct higher lock"
    );
    assert!(adapter.pending_local_lane_proposals.is_empty());
    assert!(adapter.locally_bound_lane_proposals.is_empty());
    assert!(
        !adapter.lane_sessions.contains_proposal(&losing_proposal),
        "uncommitted lane sessions for the superseded carrier must release capacity"
    );
    assert!(matches!(
        adapter.mark_global_body_locked(round(0), subject_a),
        Err(V2LaneWorkError::ConflictingGlobalBodyLock)
    ));
    assert_eq!(
        adapter.globally_locked_body.map(|lock| lock.subject),
        Some(subject_b),
        "a lower lock cannot restore the retired exact subject"
    );
}
#[test]
fn superseded_commit_protected_lane_session_cannot_retransmit() {
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
    assert_eq!(
        adapter.lane_sessions.insert_qc_with_pops(
            lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
            &lane_signer_pops(&keys),
        ),
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    );
    let commit_vote = signed_lane_vote(&proposal, CertPhase::Commit, &keys[0]);
    assert_eq!(
        adapter.lane_sessions.insert_vote(commit_vote, None),
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    );
    let replacement_round = wire::ConsensusRound {
        view: locked_round.view + 1,
        ..locked_round
    };
    let replacement_subject = wire::BlockSubject {
        block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"replacement global carrier block hash",
        )),
        payload_hash: Hash::new(b"replacement global carrier payload"),
        ..locked_subject
    };
    assert_eq!(
        adapter.mark_global_body_locked(replacement_round, replacement_subject),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert!(
        adapter.lane_sessions.contains_proposal(&proposal),
        "Commit evidence remains cached as safety state"
    );
    adapter
        .schedule_retransmission()
        .expect("schedule after replacing the exact global lock");
    let effects = adapter.drain_effects(usize::MAX);
    assert!(
        !effects.iter().any(|effect| matches!(
            effect,
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockProposal(candidate),
                ..
            } if candidate.proposal_hash == proposal.proposal_hash
        ) || matches!(
            effect,
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockVote(vote),
                ..
            } if vote.body.proposal_hash == proposal.proposal_hash
        ) || matches!(
            effect,
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockQc(qc),
                ..
            } if qc.body.proposal_hash == proposal.proposal_hash
        )),
        "safety-retained state for the losing carrier must not remain live traffic"
    );
}
#[test]
fn decision_cleanup_fairly_reconstructs_completed_commit_qc_fanout() {
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
    adapter.limits.effect_capacity = NonZeroUsize::new(1).expect("non-zero capacity");
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
    assert!(adapter.has_pending_committed_output_handoff());
    adapter
        .retain_merge_sidecars_for_global_view(
            locked_round.view,
            Some(locked_subject),
            Some(locked_subject),
        )
        .expect("install decided carrier state");
    let expected = proposal
        .descriptor
        .validator_set
        .iter()
        .filter(|peer| *peer != &adapter.local_peer)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut observed = BTreeSet::new();
    for _ in 0..=expected.len() {
        adapter
            .schedule_retransmission()
            .expect("reconstruct the next final CommitQC destination");
        for effect in adapter.drain_effects(1) {
            match effect {
                V2LaneWorkEffect::PostLaneBlock {
                    peer,
                    message: BlockMessage::LaneBlockQc(qc),
                } => {
                    assert_eq!(qc.body.phase, CertPhase::Commit);
                    assert_eq!(qc.body.proposal_hash, proposal.proposal_hash);
                    assert!(
                        observed.insert(peer),
                        "destination must transfer exactly once"
                    );
                }
                other => panic!("decision cleanup retained non-final lane output: {other:?}"),
            }
        }
        if !adapter.has_pending_committed_output_handoff() {
            break;
        }
    }
    assert_eq!(observed, expected);
    assert!(!adapter.has_pending_committed_output_handoff());
}

#[test]
fn autonomous_payload_and_new_view_ingress_are_exact_and_contiguous() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let (source_block, mut proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    proposal.payload_block_hint = None;
    let entrypoint = source_block
        .external_entrypoints_cloned()
        .next()
        .expect("planned autonomous entrypoint");
    let accepted = crate::tx::AcceptedTransaction::new_unchecked_entrypoint(
        std::borrow::Cow::Owned(entrypoint.clone()),
    );
    let routing_plan = RoutingPlan::single(RoutingDecision::new(
        proposal.descriptor.lane_id,
        proposal.descriptor.dataspace_id,
    ));
    let mut reservation = crate::queue::LaneQueueReservationKeyV2 {
        version: crate::queue::LaneQueueReservationKeyV2::VERSION,
        signed_transaction_hash: accepted.hash(),
        entrypoint_hash: entrypoint.hash(),
        queue_plan_admission_binding_hash: Hash::new(
            b"autonomous-ingress-queue-plan-admission-binding",
        ),
        routing_plan_digest: routing_plan.digest(),
        coordinator_leg: routing_plan.coordinator_leg(),
        lane_id: proposal.descriptor.lane_id,
        dataspace_id: proposal.descriptor.dataspace_id,
        lane_incarnation: proposal.descriptor.lane_incarnation,
        proposal_height: proposal.descriptor.proposal_height,
        lane_block_height: proposal.descriptor.lane_block_height,
        lane_block_view: proposal.descriptor.lane_block_view,
        reservation_owner_hash: Hash::new(b"autonomous-ingress-reservation-owner"),
        proposal_identity_hash: proposal.proposal_hash,
    };
    let producer = adapter
        .expected_autonomous_lane_author(&proposal)
        .expect("deterministic autonomous producer")
        .clone();
    bind_canonical_autonomous_reservation_identity(
        &adapter,
        &proposal,
        &producer,
        &mut reservation,
    );
    let producer_key = keys
        .iter()
        .find(|key| key.public_key() == producer.public_key())
        .expect("producer key");
    let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
        adapter.native_network_id(),
        adapter.context.epoch,
        proposal,
        vec![entrypoint],
        vec![reservation],
        vec![routing_plan],
        vec![None],
        producer.clone(),
        producer_key.private_key(),
    )
    .expect("signed autonomous payload");
    let wrong_sender = keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .find(|peer| peer != &producer)
        .expect("non-producer sender");
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneExecutablePayload(payload.clone()),
                Some(wrong_sender),
            ),
            0,
        ),
        V2LaneIngressOutcome::Rejected
    );
    let mut zero_owner = payload.clone();
    zero_owner.reservation_keys[0].reservation_owner_hash = Hash::prehashed([0; Hash::LENGTH]);
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneExecutablePayload(zero_owner),
                Some(producer.clone()),
            ),
            0,
        ),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneExecutablePayload(payload.clone()),
                Some(producer.clone()),
            ),
            0,
        ),
        V2LaneIngressOutcome::Inserted
    );
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneExecutablePayload(payload.clone()),
                Some(producer),
            ),
            0,
        ),
        V2LaneIngressOutcome::Duplicate
    );
    let envelope = autonomous_lane_payload_envelope(
        &payload,
        adapter.native_network_id(),
        adapter.context.epoch,
    )
    .expect("encode the exact autonomous carrier envelope");
    let header = BlockHeader::new(
        NonZeroU64::new(adapter.context.height).expect("non-zero carrier height"),
        adapter
            .context
            .parent_commit_qc
            .as_ref()
            .map(|qc| qc.subject.block_hash),
        None,
        None,
        adapter.context.height,
        0,
    );
    let mut builder = BlockBuilder::new(header);
    builder.set_execution_context(Some(
        BlockExecutionContextBundle::new(Vec::new())
            .with_autonomous_lane_payloads(vec![envelope]),
    ));
    let leader_index =
        usize::try_from(adapter.context.leader(0)).expect("global leader index fits usize");
    let block = builder
        .build_with_signature(
            u64::try_from(leader_index).expect("global leader index fits u64"),
            keys[leader_index].private_key(),
        )
        .canonical_resultless_proposal();
    let payload = payload
        .attach_global_hint_exact(
            LaneBlockProposalPayloadHintV1 {
                proposal_height: adapter.context.height,
                proposal_view: block.header().view_change_index(),
                proposal_block_hash: block.hash(),
            },
            adapter.native_network_id(),
            adapter.context.epoch,
        )
        .expect("anchor the autonomous payload to its exact canonical carrier");
    let premature_body = crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
        &payload.origin_proposal,
        &payload,
        1,
        adapter.native_network_id(),
        adapter.context.epoch,
    )
    .expect("well-formed pre-lock NewView body");
    let premature_signer = payload.origin_proposal.descriptor.validator_set[0].clone();
    let premature_key = keys
        .iter()
        .find(|key| key.public_key() == premature_signer.public_key())
        .expect("pre-lock NewView signer key");
    let premature_vote = crate::lane_consensus::LaneBlockNewViewVoteV1::new_signed(
        premature_body,
        premature_signer.clone(),
        premature_key.private_key(),
    )
    .expect("signed pre-lock NewView vote");
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneBlockNewViewVote(premature_vote),
                Some(premature_signer),
            ),
            0,
        ),
        V2LaneIngressOutcome::Rejected,
        "hinted bytes cannot advance before their exact protected carrier is durable"
    );
    let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
    assert_eq!(
        adapter.mark_global_body_locked(locked_round, locked_subject),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_ne!(
        adapter.bind_locked_global_body(&block),
        V2LaneIngressOutcome::Rejected,
        "the exact carrier makes payload bytes durable"
    );
    let payload_key = AutonomousLanePayloadKey::from(&payload.origin_proposal);
    assert_eq!(
        adapter
            .autonomous_new_view_started_at
            .get(&payload_key)
            .map(|(view, _)| *view),
        Some(0)
    );
    adapter
        .retain_merge_sidecars_for_global_view(
            locked_round.view,
            Some(locked_subject),
            Some(locked_subject),
        )
        .expect("install exact global Decision before lane NewView");
    adapter.drive_lane_sessions();
    let synthetic_view_one = crate::lane_consensus::retarget_lane_block_proposal_exact_view(
        &payload.origin_proposal,
        1,
    )
    .expect("synthetic view-one cursor");
    let gap_body = crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
        &synthetic_view_one,
        &payload,
        2,
        adapter.native_network_id(),
        adapter.context.epoch,
    )
    .expect("well-formed but premature view-two transition");
    let gap_signer = payload.origin_proposal.descriptor.validator_set[0].clone();
    let gap_key = keys
        .iter()
        .find(|key| key.public_key() == gap_signer.public_key())
        .expect("gap signer key");
    let gap_vote = crate::lane_consensus::LaneBlockNewViewVoteV1::new_signed(
        gap_body,
        gap_signer.clone(),
        gap_key.private_key(),
    )
    .expect("signed premature NewView vote");
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneBlockNewViewVote(gap_vote),
                Some(gap_signer),
            ),
            0,
        ),
        V2LaneIngressOutcome::Rejected,
        "a valid future transition cannot skip the retained view cursor"
    );
    let body = crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
        &payload.origin_proposal,
        &payload,
        1,
        adapter.native_network_id(),
        adapter.context.epoch,
    )
    .expect("view-one transition");
    let quorum = usize::try_from(body.min_quorum).expect("quorum fits usize");
    let started_at = Instant::now();
    adapter
        .autonomous_new_view_started_at
        .insert(payload_key, (0, started_at));
    let _ = adapter.drain_effects(usize::MAX);
    let timeout = Duration::from_secs(1);
    adapter
        .schedule_autonomous_new_view_timeouts(
            started_at + timeout - Duration::from_nanos(1),
            0,
            timeout,
        )
        .expect("pre-deadline NewView tick");
    assert!(
        adapter
            .autonomous_new_view_votes
            .votes_for_signer(&adapter.local_peer)
            .is_empty(),
        "a lane must not emit NewView before its independent deadline"
    );
    assert!(adapter.drain_effects(usize::MAX).is_empty());
    adapter
        .schedule_autonomous_new_view_timeouts(started_at + timeout, 0, timeout)
        .expect("deadline NewView tick");
    let first_fanout = adapter
        .drain_effects(usize::MAX)
        .into_iter()
        .filter_map(|effect| match effect {
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockNewViewVote(vote),
                ..
            } => Some(vote),
            _ => None,
        })
        .collect::<Vec<_>>();
    let local_vote = first_fanout
        .first()
        .cloned()
        .expect("deadline emits the local NewView vote");
    assert!(
        first_fanout.iter().all(|vote| vote == &local_vote),
        "one timeout fanout must retain byte-identical vote evidence"
    );
    adapter
        .schedule_autonomous_new_view_timeouts(
            started_at + timeout + Duration::from_millis(1),
            0,
            timeout,
        )
        .expect("due NewView retransmission tick");
    let repeated_fanout = adapter
        .drain_effects(usize::MAX)
        .into_iter()
        .filter_map(|effect| match effect {
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockNewViewVote(vote),
                ..
            } => Some(vote),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        !repeated_fanout.is_empty() && repeated_fanout.iter().all(|vote| vote == &local_vote),
        "a still-due clock retransmits the exact cached vote without changing identity"
    );
    let mut accepted = 1_usize;
    let mut last_vote = local_vote;
    let local_peer = adapter.local_peer.clone();
    for signer in payload
        .origin_proposal
        .descriptor
        .validator_set
        .iter()
        .filter(|signer| *signer != &local_peer)
    {
        if accepted >= quorum {
            break;
        }
        let signer_key = keys
            .iter()
            .find(|key| key.public_key() == signer.public_key())
            .expect("NewView signer key");
        let vote = crate::lane_consensus::LaneBlockNewViewVoteV1::new_signed(
            body.clone(),
            signer.clone(),
            signer_key.private_key(),
        )
        .expect("signed contiguous NewView vote");
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockNewViewVote(vote.clone()),
                    Some(signer.clone()),
                ),
                0,
            ),
            V2LaneIngressOutcome::Inserted
        );
        last_vote = vote;
        accepted = accepted.saturating_add(1);
    }
    assert_eq!(adapter.autonomous_payload_views.get(&payload_key), Some(&1));
    let (durable_payload, durable_cursor) = adapter
        .kura
        .current_autonomous_lane_payload(
            payload.origin_proposal.descriptor.lane_id,
            payload.origin_proposal.descriptor.lane_block_height,
            adapter.native_network_id(),
            adapter.context.epoch,
        )
        .expect("quorum persists the exact autonomous NewView cursor");
    assert_eq!(durable_payload, payload);
    assert_eq!(durable_cursor.descriptor.lane_block_view, 1);
    let incomplete_lane_sessions = adapter.lane_sessions.proposals_without_commit_qc();
    assert!(
        incomplete_lane_sessions.contains(&payload.origin_proposal)
            && incomplete_lane_sessions
                .iter()
                .all(|proposal| proposal.descriptor.lane_block_view == 0),
        "NewView must not create a second READY/Prepare/Commit subject"
    );
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneBlockNewViewVote(last_vote.clone()),
                Some(last_vote.signer.clone()),
            ),
            0,
        ),
        V2LaneIngressOutcome::Duplicate,
        "a post-seal retransmission remains idempotent after the cursor advances"
    );
    let durable_certificate = adapter
        .kura
        .read_autonomous_lane_block_artifact(
            payload.origin_proposal.descriptor.lane_id,
            payload.origin_proposal.descriptor.lane_block_height,
            adapter.native_network_id(),
            adapter.context.epoch,
        )
        .and_then(|artifact| {
            V2LaneWorkAdapter::latest_durable_autonomous_new_view_certificate(&artifact, 1)
                .cloned()
        })
        .expect("durable view-one certificate");
    let finality = verified_finality_artifact_for_block(&adapter, &keys, &block);
    adapter
        .kura
        .store_block(block.clone())
        .expect("persist canonical autonomous carrier before restart");
    let finality_receipt = adapter
        .kura
        .store_v2_finality_artifact(&finality)
        .expect("persist canonical autonomous carrier finality before restart");
    assert_eq!(finality_receipt.height(), adapter.context.height);
    assert_eq!(finality_receipt.block_hash(), block.hash());
    let committed = ValidBlock::committed_from_replay_signed_block(block.clone());
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
    let historical_epoch = adapter.context.epoch;
    let historical_height = adapter.context.height;
    {
        let mut world = adapter.state.world.block();
        world.vrf_epochs_mut_for_testing().insert(
            historical_epoch,
            iroha_data_model::consensus::VrfEpochRecord {
                epoch: historical_epoch,
                seed: adapter.context.leader_seed,
                epoch_length: 2,
                commit_deadline_offset: 0,
                reveal_deadline_offset: 0,
                roster_len: 0,
                finalized: true,
                updated_at_height: historical_height,
                participants: Vec::new(),
                late_reveals: Vec::new(),
                committed_no_reveal: Vec::new(),
                no_participation: Vec::new(),
                penalties_applied: false,
                penalties_applied_at_height: None,
                validator_election: None,
            },
        );
        world.commit();
    }
    let execution_commitment = finality.commit_qc.execution_commitment;
    let mut historical_install = HistoricalAutonomousReservationInstallV1 {
        version: HistoricalAutonomousReservationInstallV1::VERSION,
        recovery_id: Hash::prehashed([0; Hash::LENGTH]),
        canonical_body: CanonicalExecutedBlockNeedV1 {
            height: historical_height,
            block_hash: block.hash(),
            finality_artifact_hash: HashOf::new(&finality),
            execution_commitment,
            executed_block_wire_len: execution_commitment.executed_block_wire_len,
            executed_block_wire_hash: execution_commitment.executed_block_wire_hash,
        },
        historical_context: adapter.context.clone(),
        historical_context_id: adapter.context.id(),
        historical_context_hash: HashOf::new(&adapter.context),
        carrier_view: block.header().view_change_index(),
        payload: payload.clone(),
        reservation_group: LaneQueueReservationReconciliationGroupV1 {
            identity: LaneQueueReservationGroupIdentityV1::from_key(
                payload
                    .reservation_keys
                    .first()
                    .expect("autonomous recovery reservation group is non-empty"),
            ),
            ordered_keys: payload.reservation_keys.clone(),
        },
    };
    historical_install.recovery_id = historical_install.computed_recovery_id();
    assert_eq!(
        install_historical_autonomous_lane_recovery(
            adapter.state.as_ref(),
            adapter.kura.as_ref(),
            &historical_install,
        )
        .expect("persist exact historical autonomous recovery record"),
        HistoricalAutonomousLaneRecoveryInstallOutcome::Installed,
    );
    let mut context = successor_context_for_parent(&adapter, &block);
    context.epoch = {
        let world = adapter.state.world_view();
        crate::sumeragi::epoch_for_height_from_world(&world, context.height)
    };
    context.epoch_end_height = context.height.saturating_add(1);
    assert_eq!(
        context.epoch,
        historical_epoch.saturating_add(1),
        "fixture must cross an authenticated epoch boundary"
    );
    let local_peer = adapter.local_peer.clone();
    let replacement = PeerId::new(
        KeyPair::try_from_seed(vec![0xF4; 32], Algorithm::BlsNormal)
            .expect("deterministic successor-only validator")
            .public_key()
            .clone(),
    );
    let removed_index = context
        .roster
        .iter()
        .position(|entry| entry.validator == local_peer)
        .expect("fixture local validator belongs to the predecessor roster");
    assert!(
        context
            .roster
            .iter()
            .all(|entry| entry.validator != replacement),
        "successor-only validator must be distinct from the predecessor roster"
    );
    context.roster[removed_index].validator = replacement;
    context
        .roster
        .sort_by(|left, right| left.validator.cmp(&right.validator));
    context.quorum =
        wire::DualQuorum::from_roster(&context.roster).expect("successor equal-vote quorum");
    assert!(
        context
            .roster
            .iter()
            .all(|entry| entry.validator != local_peer),
        "fixture must remove the old lane signer from the successor global roster"
    );
    context
        .validate()
        .expect("roster-changing successor context remains valid");
    let key_pair = adapter.key_pair.clone();
    let state = Arc::clone(&adapter.state);
    let kura = Arc::clone(&adapter.kura);
    let limits = adapter.limits;
    drop(adapter);
    let mut recovered = V2LaneWorkAdapter::new_with_output_guard(
        context,
        local_peer.clone(),
        key_pair,
        true,
        state,
        kura,
        limits,
        None,
        None,
        ConsensusOutputGuard::isolated(),
    )
    .expect("reopen successor adapter after durable NewView publication");
    assert!(
        recovered
            .historical_autonomous_recovery_records
            .get(&payload_key)
            .is_some_and(|record| record.payload == payload),
        "successor startup must hydrate the exact immutable historical recovery record"
    );
    assert!(
        recovered
            .lane_sessions
            .proposals_without_commit_qc()
            .contains(&payload.origin_proposal),
        "successor startup must rebuild the historical lane consensus session"
    );
    let restored_prepare_vote = recovered
        .lane_sessions
        .local_vote_rebroadcast_artifacts_for(&local_peer)
        .into_iter()
        .find_map(|(proposal, vote)| {
            (proposal == payload.origin_proposal && vote.body.phase == CertPhase::Prepare)
                .then_some(vote)
        })
        .expect(
            "a configured validator removed from the successor global roster must keep signing its unfinished historical lane committee",
        );
    assert!(
        recovered.autonomous_payloads.is_empty()
            && recovered.autonomous_payload_views.is_empty(),
        "historical recovery must not populate current-height autonomous payload or cursor maps"
    );
    assert!(
        !recovered
            .autonomous_new_view_certificates
            .contains(&durable_certificate.certificate),
        "historical recovery must not hydrate a current-height NewView certificate cache"
    );
    let recovered_durable_certificate = recovered
        .kura
        .read_autonomous_lane_block_artifact(
            payload.origin_proposal.descriptor.lane_id,
            payload.origin_proposal.descriptor.lane_block_height,
            recovered.native_network_id(),
            historical_epoch,
        )
        .and_then(|artifact| {
            V2LaneWorkAdapter::latest_durable_autonomous_new_view_certificate(&artifact, 1)
                .cloned()
        })
        .expect("historical NewView certificate remains durable in Kura");
    assert_eq!(recovered_durable_certificate, durable_certificate);
    let _ = recovered.drain_effects(usize::MAX);
    recovered
        .schedule_retransmission()
        .expect("retransmit restored historical lane session");
    let retransmissions = recovered.drain_effects(usize::MAX);
    assert!(
        retransmissions.iter().any(|effect| {
            matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneBlockProposal(proposal),
                    ..
                } if proposal == &payload.origin_proposal
            )
        }),
        "historical retransmission must fan out the exact restored proposal"
    );
    assert!(
        retransmissions.iter().any(|effect| {
            matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneBlockVote(vote),
                    ..
                } if vote == &restored_prepare_vote
            )
        }),
        "historical retransmission must fan out the exact restored Prepare vote"
    );
}
