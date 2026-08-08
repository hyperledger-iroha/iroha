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
        adapter.context.chain_id.clone(),
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
