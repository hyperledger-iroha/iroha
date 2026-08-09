// Autonomous carrier retry and durable retirement regressions.

#[test]
fn repeated_heartbeat_retries_never_make_autonomous_routes_ordinary_eligible() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    prepare_autonomous_test_lane(&mut adapter, &keys, lane_id, dataspace_id);

    let transaction_key = KeyPair::try_from_seed(vec![0xE1; 32], Algorithm::Ed25519)
        .expect("deterministic autonomous-route transaction key");
    let transaction = TransactionBuilder::new(
        adapter.context.network_id,
        AccountId::new(transaction_key.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(transaction_key.private_key());
    let accepted =
        crate::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(transaction));
    let routing_plan = RoutingPlan::single(RoutingDecision::new(lane_id, dataspace_id));
    let candidate = CandidateDescriptor::new(&accepted, &routing_plan);
    let context = adapter.context.clone();
    let mut provider = &mut adapter;

    for _ in 0..3 {
        let heartbeat = provider
            .prepare(&context, 0, &[])
            .expect("an empty heartbeat requires no ordinary lane ownership");
        assert!(heartbeat.native_amx_receipts.is_empty());

        let unavailable = provider
            .prepare(&context, 0, &[candidate])
            .expect_err("autonomous route must remain unavailable to ordinary execution");
        assert_eq!(unavailable.indices(), &BTreeSet::from([0]));
        assert_eq!(
            unavailable.reason(),
            "waiting for deterministic autonomous lane authors to publish durable FIFO reservations"
        );
    }
}

#[test]
fn losing_autonomous_carrier_is_durably_retired_before_cache_drop() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    prepare_autonomous_test_lane(&mut adapter, &keys, lane_id, dataspace_id);
    select_autonomous_test_role(&mut adapter, &keys, lane_id, dataspace_id, true);

    let journal_dir = tempfile::tempdir().expect("autonomous reservation journal directory");
    let journal_path = journal_dir.path().join("lane-reservations.norito");
    let queue = install_autonomous_test_queue(&mut adapter, lane_id, dataspace_id, &journal_path);
    let expected_entrypoints =
        enqueue_autonomous_test_transactions(&adapter, &queue, lane_id, dataspace_id, 4);
    let original_fifo = queue.fifo_snapshot_for_test();
    adapter
        .schedule_autonomous_lane_production(0, autonomous_test_candidate_limits(6, 6))
        .expect("produce a durably reserved losing autonomous payload");
    let payload = adapter
        .pending_autonomous_anchor_payloads
        .values()
        .find(|payload| {
            payload.origin_proposal.descriptor.lane_id == lane_id
                && payload.origin_proposal.descriptor.dataspace_id == dataspace_id
        })
        .expect("local author publishes a pending autonomous payload")
        .clone();
    let proposal = payload.origin_proposal.clone();
    let producer = payload.producer.clone();
    assert_eq!(payload.entrypoints, expected_entrypoints[..3]);
    let mut expected_live_reservations = payload.reservation_keys.clone();
    expected_live_reservations.sort_by_key(crate::queue::LaneQueueReservationKeyV2::digest);
    assert_eq!(queue.live_lane_reservations(), expected_live_reservations);
    let context = adapter.context.clone();
    let prepared = adapter
        .prepare_certified_execution_carrier(&context, 0, &[])
        .expect("prepare an exact-empty winning execution carrier");
    assert!(prepared.autonomous_lane_payloads.is_empty());
    assert_eq!(
        adapter.pending_autonomous_anchor_payloads.values().next(),
        Some(&payload),
        "the losing payload remains durably owned until the carrier lock chooses a winner"
    );
    let leader_index =
        usize::try_from(adapter.context.leader(0)).expect("winning execution-carrier leader index");
    let winning_header = adapter
        .merge_carrier_context_header(0)
        .expect("exact empty winning-carrier header");
    let winning_block = BlockBuilder::new(winning_header)
        .build_with_signature(
            u64::try_from(leader_index).expect("leader index fits u64"),
            keys[leader_index].private_key(),
        )
        .canonical_resultless_proposal();
    let (round, winning_subject) = global_lock_for_block(&adapter, &winning_block);
    assert_eq!(
        adapter.mark_global_body_locked(round, winning_subject),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_eq!(
        adapter.pending_autonomous_anchor_payloads.values().next(),
        Some(&payload),
        "a lock without its authenticated body must preserve pending ownership"
    );
    assert!(
        adapter
            .kura
            .read_autonomous_lane_slot_retirement(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
                adapter.native_chain_id_hash(),
                adapter.context.epoch,
            )
            .expect("read pre-body losing-slot state")
            .is_none()
    );
    assert_ne!(
        adapter.bind_locked_global_body(&winning_block),
        V2LaneIngressOutcome::Rejected,
        "the authenticated exact-empty body must drive losing-slot retirement"
    );

    assert!(adapter.autonomous_payloads.is_empty());
    assert!(adapter.pending_autonomous_anchor_payloads.is_empty());
    let retirement = adapter
        .kura
        .read_autonomous_lane_slot_retirement(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
            adapter.native_chain_id_hash(),
            adapter.context.epoch,
        )
        .expect("read losing autonomous retirement")
        .expect("losing autonomous slot is durably retired");
    assert_eq!(
        retirement,
        crate::kura::AutonomousLaneSlotRetirementV1::from_payload(&payload)
    );
    assert!(queue.live_lane_reservations().is_empty());
    assert_eq!(queue.fifo_snapshot_for_test(), original_fifo);
    assert!(!adapter.output_guard.restart_required());
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(BlockMessage::LaneExecutablePayload(payload), Some(producer),),
            round.view,
        ),
        V2LaneIngressOutcome::Rejected,
        "a delayed payload from the retired carrier must not reclaim the slot"
    );
}

#[test]
fn canonical_autonomous_carrier_binds_after_direct_four_validator_decision() {
    let (mut adapter, keys) = fixture_at_height_inner(wire::ConsensusMode::Permissioned, 2, true);

    let (source_block, mut proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    proposal.payload_block_hint = None;
    let entrypoint = source_block
        .external_entrypoints_cloned()
        .next()
        .expect("autonomous entrypoint");
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
            b"direct-decision-queue-plan-admission-binding",
        ),
        routing_plan_digest: routing_plan.digest(),
        coordinator_leg: routing_plan.coordinator_leg(),
        lane_id: proposal.descriptor.lane_id,
        dataspace_id: proposal.descriptor.dataspace_id,
        lane_incarnation: proposal.descriptor.lane_incarnation,
        proposal_height: proposal.descriptor.proposal_height,
        lane_block_height: proposal.descriptor.lane_block_height,
        lane_block_view: proposal.descriptor.lane_block_view,
        reservation_owner_hash: Hash::new(b"direct-decision-reservation-owner"),
        proposal_identity_hash: proposal.proposal_hash,
    };
    let producer = adapter
        .expected_lane_author(&proposal)
        .expect("deterministic autonomous producer")
        .clone();
    let producer_key = keys
        .iter()
        .find(|candidate| candidate.public_key() == producer.public_key())
        .expect("autonomous producer key");
    bind_canonical_autonomous_reservation_identity(
        &adapter,
        &proposal,
        &producer,
        &mut reservation,
    );
    let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
        adapter.native_chain_id_hash(),
        adapter.context.epoch,
        proposal.clone(),
        vec![entrypoint],
        vec![reservation],
        vec![routing_plan],
        vec![None],
        producer.clone(),
        producer_key.private_key(),
    )
    .expect("signed hint-free autonomous payload");
    adapter
        .kura
        .persist_lane_executable_payload(
            &payload,
            adapter.native_chain_id_hash(),
            adapter.context.epoch,
        )
        .expect("persist producer payload before carrier selection");
    assert_eq!(
        adapter.accept_lane_message(
            InboundBlockMessage::new(
                BlockMessage::LaneExecutablePayload(payload.clone()),
                Some(producer),
            ),
            0,
        ),
        V2LaneIngressOutcome::Inserted
    );

    let envelope = autonomous_lane_payload_envelope(
        &payload,
        adapter.native_chain_id_hash(),
        adapter.context.epoch,
    )
    .expect("encode autonomous carrier envelope");
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
        BlockExecutionContextBundle::new(Vec::new()).with_autonomous_lane_payloads(vec![envelope]),
    ));
    let leader_index =
        usize::try_from(adapter.context.leader(0)).expect("global leader index fits usize");
    let carrier = builder.build_with_signature(
        u64::try_from(leader_index).expect("global leader index fits u64"),
        keys[leader_index].private_key(),
    );
    adapter
        .kura
        .store_block(carrier.clone())
        .expect("persist canonical autonomous carrier");
    let (locked_round, decided) = global_lock_for_block(&adapter, &carrier);
    let finality = verified_finality_artifact_for_block(&adapter, &keys, &carrier);
    let receipt = KuraV2CommitReceipt::for_test(&finality);
    let stale_lock = wire::BlockSubject {
        parent_block_hash: decided.parent_block_hash,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"stale-four-validator-local-lock")),
        payload_hash: Hash::new(b"stale-four-validator-local-lock-payload"),
    };
    assert_eq!(
        adapter.mark_global_body_locked(locked_round, stale_lock),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    // Model the runner race exactly: Decision is published before another
    // local proposal-scheduling turn can consume the worker's body load,
    // and it supersedes this validator's different local Prepare lock.
    adapter
        .retain_merge_sidecars_for_global_view(locked_round.view, Some(stale_lock), Some(decided))
        .expect("install direct same-view Decision");
    assert_eq!(
        adapter.globally_locked_body.map(|lock| lock.subject),
        Some(stale_lock)
    );
    let committed = ValidBlock::committed_from_replay_signed_block(carrier);
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
    assert!(
        adapter
            .kura
            .read_lane_block_execution_input(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .is_none(),
        "Decision arrived before the live locked-body binding path"
    );

    assert_ne!(
        adapter
            .recover_decided_canonical_lane_body(&receipt, &finality)
            .expect("recover exact receipt-authorized canonical carrier"),
        V2LaneIngressOutcome::Rejected
    );
    assert!(
        adapter
            .kura
            .read_lane_block_execution_input(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .is_some(),
        "receipt-bound recovery must persist execution input before READY"
    );
    let prepare_votes = keys
        .iter()
        .map(|key| signed_autonomous_prepare_vote(&proposal, &payload, key, &keys))
        .collect::<Vec<_>>();
    let prepare_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        proposal.vote_body(CertPhase::Prepare),
        proposal.descriptor.validator_set.clone(),
        &prepare_votes,
    )
    .expect("four-validator READY votes form PrepareQC");
    assert_eq!(
        adapter.insert_lane_qc(prepare_qc, locked_round.view),
        V2LaneIngressOutcome::Inserted
    );
    assert_eq!(
        adapter.insert_lane_qc(
            lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
            locked_round.view,
        ),
        V2LaneIngressOutcome::Inserted
    );
    assert_eq!(
        adapter
            .persist_anchored_sessions()
            .expect("bind canonical carrier and finish four-validator lane consensus"),
        1
    );
    assert!(
        adapter
            .kura
            .read_lane_block_execution_input(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .is_some(),
        "canonical fallback must persist execution input before READY"
    );
    let durable = adapter
        .kura
        .read_certified_lane_block_artifact(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
        )
        .expect("four-validator READY and Commit votes produce a durable certificate");
    assert!(durable.prepare_qc.payload_availability_qc.is_some());
    assert!(
        adapter
            .durable_lane_rollover_authority(&finality)
            .expect("validate four-validator lane rollover")
            .is_some(),
        "durable four-validator availability and Commit certificates release rollover"
    );
}
