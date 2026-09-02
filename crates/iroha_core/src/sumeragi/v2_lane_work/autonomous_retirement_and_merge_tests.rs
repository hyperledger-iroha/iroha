fn assert_locked_external_queue_plan_body_rejects_before_retiring_autonomous_owner() {
    let (mut adapter, keys) = autonomous_test_fixture(wire::ConsensusMode::Permissioned, true);
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    prepare_autonomous_test_lane(&mut adapter, &keys, lane_id, dataspace_id);
    let journal_dir = tempfile::tempdir().expect("autonomous reservation journal directory");
    let queue = install_autonomous_test_queue(
        &mut adapter,
        lane_id,
        dataspace_id,
        &journal_dir.path().join("lane-reservations.norito"),
    );
    enqueue_autonomous_test_transactions(&adapter, &queue, lane_id, dataspace_id, 1);
    adapter
        .schedule_autonomous_lane_production(0, autonomous_test_candidate_limits(2, 2))
        .expect("produce one Queue-owned autonomous payload");

    let pending_before = adapter.pending_autonomous_anchor_payloads.clone();
    let reservations_before = queue.live_lane_reservations();
    let fifo_before = queue.fifo_snapshot_for_test();
    let (forbidden, _) = planned_lane_candidate_block_for_route_at_view_with_intent(
        &adapter,
        &keys,
        0,
        lane_id,
        dataspace_id,
        iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
    );
    mark_global_body_locked_for_block(&mut adapter, &forbidden);

    assert_eq!(
        adapter.bind_locked_global_body(&forbidden),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(adapter.pending_autonomous_anchor_payloads, pending_before);
    assert_eq!(queue.live_lane_reservations(), reservations_before);
    assert_eq!(queue.fifo_snapshot_for_test(), fifo_before);
    for payload in pending_before.values() {
        assert!(
            adapter
                .kura
                .read_autonomous_lane_slot_retirement(
                    payload.origin_proposal.descriptor.lane_id,
                    payload.origin_proposal.descriptor.lane_block_height,
                    adapter.native_network_id(),
                    adapter.context.epoch,
                )
                .expect("read autonomous retirement state")
                .is_none(),
            "role-invalid global ownership must not retire the autonomous attempt"
        );
    }
}

#[test]
fn losing_autonomous_carrier_is_durably_retired_before_cache_drop() {
    assert_locked_external_queue_plan_body_rejects_before_retiring_autonomous_owner();
    let (mut adapter, keys) = autonomous_test_fixture(wire::ConsensusMode::Permissioned, true);
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    prepare_autonomous_test_lane(&mut adapter, &keys, lane_id, dataspace_id);
    assert_autonomous_test_role(&adapter, &keys, lane_id, dataspace_id, true);
    let journal_dir = tempfile::tempdir().expect("autonomous reservation journal directory");
    let journal_path = journal_dir.path().join("lane-reservations.norito");
    let queue = install_autonomous_test_queue(&mut adapter, lane_id, dataspace_id, &journal_path);
    enqueue_autonomous_test_transactions(&adapter, &queue, lane_id, dataspace_id, 1);
    let original_fifo = queue.fifo_snapshot_for_test();
    adapter
        .schedule_autonomous_lane_production(0, autonomous_test_candidate_limits(2, 2))
        .expect("produce one Queue-owned autonomous payload");
    let payload = adapter
        .pending_autonomous_anchor_payloads
        .values()
        .find(|payload| {
            payload.origin_proposal.descriptor.lane_id == lane_id
                && payload.origin_proposal.descriptor.dataspace_id == dataspace_id
        })
        .expect("local producer publishes one pending autonomous payload")
        .clone();
    let producer = payload.producer.clone();
    assert_eq!(payload.origin_proposal.payload_block_hint, None);
    assert_eq!(queue.live_lane_reservations(), payload.reservation_keys);
    assert_eq!(
        adapter
            .kura
            .read_autonomous_lane_block_artifact(
                lane_id,
                payload.origin_proposal.descriptor.lane_block_height,
                adapter.native_network_id(),
                adapter.context.epoch,
            )
            .expect("read durable losing autonomous attempt")
            .executable_payload,
        payload
    );
    let _ = adapter.drain_effects(usize::MAX);
    let envelope = autonomous_lane_payload_envelope(
        &payload,
        adapter.native_network_id(),
        adapter.context.epoch,
    )
    .expect("encode the exact losing autonomous carrier envelope");
    let leader_index =
        usize::try_from(adapter.context.leader(0)).expect("execution-carrier leader index");
    let losing_header = adapter
        .merge_carrier_context_header(0)
        .expect("exact losing-carrier round header");
    let mut losing_builder = BlockBuilder::new(losing_header);
    losing_builder.set_execution_context(Some(
        BlockExecutionContextBundle::new(Vec::new()).with_autonomous_lane_payloads(vec![envelope]),
    ));
    let losing_carrier = losing_builder
        .build_with_signature(
            u64::try_from(leader_index).expect("leader index fits u64"),
            keys[leader_index].private_key(),
        )
        .canonical_resultless_proposal();
    let (losing_round, _losing_subject) =
        mark_global_body_locked_for_block(&mut adapter, &losing_carrier);
    assert!(
        adapter
            .pending_autonomous_anchor_payloads
            .values()
            .any(|pending| pending == &payload),
        "the lock alone cannot release a hint-free Queue owner"
    );
    assert_eq!(queue.live_lane_reservations(), payload.reservation_keys);
    assert_ne!(
        adapter.bind_locked_global_body(&losing_carrier),
        V2LaneIngressOutcome::Rejected,
        "the exact losing body must bind its hint-free autonomous payload"
    );
    let anchored_payload = adapter
        .autonomous_payloads
        .values()
        .find(|anchored| anchored.payload_hash == payload.payload_hash)
        .expect("losing carrier retains its exact anchored payload")
        .clone();
    assert_eq!(
        anchored_payload
            .origin_proposal
            .payload_block_hint
            .expect("bound payload carries the losing global hint")
            .proposal_block_hash,
        losing_carrier.hash()
    );
    assert!(adapter.pending_autonomous_anchor_payloads.is_empty());
    assert_eq!(
        queue.live_lane_reservations(),
        anchored_payload.reservation_keys
    );
    assert!(
        adapter
            .kura
            .read_autonomous_lane_slot_retirement(
                lane_id,
                anchored_payload
                    .origin_proposal
                    .descriptor
                    .lane_block_height,
                adapter.native_network_id(),
                adapter.context.epoch,
            )
            .expect("read pre-supersession autonomous retirement")
            .is_none(),
        "the exact losing carrier remains live until a higher lock supersedes it"
    );
    let retired_descriptor = &anchored_payload.origin_proposal.descriptor;
    let ordinary_winner = proposal_for_route(
        &adapter,
        &keys,
        lane_id,
        dataspace_id,
        retired_descriptor.lane_incarnation,
        retired_descriptor.proposal_height,
        retired_descriptor.lane_block_height,
    );
    assert_ne!(
        ordinary_winner, anchored_payload.origin_proposal,
        "the ordinary winner must not alias the autonomous attempt",
    );
    let ordinary_session = committed_lane_session(&ordinary_winner, &keys);
    let ordinary_pops = adapter.pops_for_lane_session(&ordinary_session);
    let ordinary_authority = crate::state::CertifiedLaneBlockPersistenceAuthority::for_test(
        lane_id,
        dataspace_id,
        retired_descriptor.lane_incarnation,
        None,
    );
    let live_conflict = adapter
        .kura
        .persist_committed_lane_block_session_with_authority(
            &ordinary_session,
            &ordinary_pops,
            &ordinary_authority,
        )
        .expect_err("a live autonomous attempt must exclude an ordinary winner");
    assert!(
        live_conflict
            .to_string()
            .contains("ordinary certification conflicts with a live durable autonomous lane slot"),
        "unexpected live autonomous collision error: {live_conflict}",
    );
    let _ = adapter.drain_effects(usize::MAX);
    let winning_view = losing_round.view.saturating_add(1);
    let winning_leader_index = usize::try_from(adapter.context.leader(winning_view))
        .expect("winning execution-carrier leader index");
    let winning_header = adapter
        .merge_carrier_context_header(winning_view)
        .expect("exact winning-carrier round header");
    let winning_carrier = BlockBuilder::new(winning_header)
        .build_with_signature(
            u64::try_from(winning_leader_index).expect("winning leader index fits u64"),
            keys[winning_leader_index].private_key(),
        )
        .canonical_resultless_proposal();
    let (winning_round, _winning_subject) =
        mark_global_body_locked_for_block(&mut adapter, &winning_carrier);
    assert!(adapter.autonomous_payloads.is_empty());
    let retirement = adapter
        .kura
        .read_autonomous_lane_slot_retirement(
            lane_id,
            anchored_payload
                .origin_proposal
                .descriptor
                .lane_block_height,
            adapter.native_network_id(),
            adapter.context.epoch,
        )
        .expect("read losing autonomous retirement")
        .expect("losing autonomous slot is durably retired");
    assert_eq!(
        retirement,
        crate::kura::AutonomousLaneSlotRetirementV1::from_payload(&anchored_payload)
    );
    assert!(queue.live_lane_reservations().is_empty());
    assert_eq!(queue.fifo_snapshot_for_test(), original_fifo);
    assert!(!adapter.output_guard.restart_required());
    let unauthenticated = adapter
        .kura
        .persist_committed_lane_block_session(&ordinary_session, &ordinary_pops)
        .expect_err("retirement alone must not authorize ordinary slot replacement");
    assert!(
        unauthenticated
            .to_string()
            .contains("lacks State lifecycle authority"),
        "unexpected unauthenticated replacement error: {unauthenticated}",
    );
    adapter
        .kura
        .persist_committed_lane_block_session_with_authority(
            &ordinary_session,
            &ordinary_pops,
            &ordinary_authority,
        )
        .expect("a different State-authorized ordinary winner may follow Complete retirement");
    assert_eq!(
        adapter
            .kura
            .read_certified_lane_block_artifact(lane_id, retired_descriptor.lane_block_height)
            .expect("read the ordinary winner after autonomous terminalization")
            .proposal,
        ordinary_winner,
    );
    assert_eq!(
        accept_lane_message_from(
            &mut adapter,
            BlockMessage::LaneExecutablePayload(anchored_payload),
            producer,
            winning_round.view,
        ),
        V2LaneIngressOutcome::Rejected,
        "a delayed payload from the retired carrier must not reclaim the slot"
    );
}
#[test]
fn losing_pending_autonomous_payload_is_retired_by_fifo_only_replica() {
    let (mut producer_adapter, keys) =
        autonomous_test_fixture(wire::ConsensusMode::Permissioned, true);
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    prepare_autonomous_test_lane(&mut producer_adapter, &keys, lane_id, dataspace_id);
    assert_autonomous_test_role(&producer_adapter, &keys, lane_id, dataspace_id, true);
    let journal_dir = tempfile::tempdir().expect("replica retirement journal directory");
    let journal_path = journal_dir.path().join("lane-reservations.norito");
    let plan_journal_path = journal_path.with_extension("plans.norito");
    let queue =
        install_autonomous_test_queue(&mut producer_adapter, lane_id, dataspace_id, &journal_path);
    enqueue_autonomous_test_transactions(&producer_adapter, &queue, lane_id, dataspace_id, 1);
    let original_fifo = queue.fifo_snapshot_for_test();
    producer_adapter
        .schedule_autonomous_lane_production(0, autonomous_test_candidate_limits(2, 2))
        .expect("produce the authenticated replica-retirement payload");
    let payload = producer_adapter
        .pending_autonomous_anchor_payloads
        .values()
        .find(|payload| {
            payload.origin_proposal.descriptor.lane_id == lane_id
                && payload.origin_proposal.descriptor.dataspace_id == dataspace_id
        })
        .expect("deterministic producer publishes the replica-retirement payload")
        .clone();
    let producer = payload.producer.clone();
    assert_eq!(payload.origin_proposal.payload_block_hint, None);
    assert_eq!(queue.live_lane_reservations(), payload.reservation_keys);
    let queue_plan_binding = producer_adapter
        .state
        .queue_plan_pending_binding_for_entrypoint(payload.reservation_keys[0].entrypoint_hash)
        .expect("read replica-retirement QueuePlan binding")
        .expect("replica-retirement QueuePlan binding remains pending");
    let producer_context = producer_adapter.context.clone();

    // Construct the exact state a non-producer replica has after QueuePlan
    // gossip: the complete transaction and immutable claim remain FIFO-owned,
    // while no local lane reservation owner remains. The producer Kura is
    // deliberately discarded below; only this Queue replica is shared.
    assert_eq!(
        queue
            .release_lane_reservations_in_order(&payload.reservation_keys)
            .expect("restore FIFO-only replica ownership"),
        payload.reservation_keys.len(),
    );
    assert!(queue.live_lane_reservations().is_empty());
    assert_eq!(queue.fifo_snapshot_for_test(), original_fifo);
    let empty_owner_snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture FIFO-only replica ownership");
    assert!(
        empty_owner_snapshot.is_empty(),
        "the follower seam must begin without any Queue reservation owner family",
    );
    let reservation_journal_before =
        std::fs::read(&journal_path).expect("read FIFO-only reservation journal");
    let plan_journal_before =
        std::fs::read(&plan_journal_path).expect("read FIFO-only QueuePlan journal");
    drop(producer_adapter);

    let (mut adapter, replica_keys) =
        autonomous_test_fixture(wire::ConsensusMode::Permissioned, false);
    prepare_autonomous_test_lane(&mut adapter, &replica_keys, lane_id, dataspace_id);
    assert_autonomous_test_role(&adapter, &replica_keys, lane_id, dataspace_id, false);
    assert_eq!(
        adapter.context, producer_context,
        "producer and follower fixtures must share one frozen height context",
    );
    assert_ne!(
        adapter.local_peer, producer,
        "the retirement actor must be a strict non-producer replica",
    );
    install_autonomous_fixture_queue_plan_registry_value(
        adapter.state.as_ref(),
        &queue_plan_binding,
    );
    adapter
        .install_lane_drain_queue(Arc::clone(&queue))
        .expect("install the FIFO-only replica Queue");
    let descriptor = &payload.origin_proposal.descriptor;
    let lane_block_height = descriptor.lane_block_height;
    let proposal_height = descriptor.proposal_height;
    let network_id = adapter.native_network_id();
    let epoch = adapter.context.epoch;
    assert!(
        adapter
            .kura
            .read_autonomous_lane_block_artifact(lane_id, lane_block_height, network_id, epoch)
            .is_none(),
        "the follower must not begin with the producer's Kura attempt",
    );
    assert_eq!(
        accept_lane_message_from(
            &mut adapter,
            BlockMessage::LaneExecutablePayload(payload.clone()),
            producer.clone(),
            0,
        ),
        V2LaneIngressOutcome::Inserted,
        "authenticated hint-free ingress must enter the follower pending cache",
    );
    assert_eq!(adapter.pending_autonomous_anchor_payloads.len(), 1);
    assert!(
        adapter
            .pending_autonomous_anchor_payloads
            .values()
            .any(|pending| pending == &payload),
    );
    assert!(
        adapter
            .kura
            .read_autonomous_lane_block_artifact(lane_id, lane_block_height, network_id, epoch)
            .is_none(),
        "hint-free follower ingress alone must not manufacture durable custody",
    );
    assert_eq!(
        queue
            .lane_reservation_reconciliation_snapshot()
            .expect("recheck pending follower Queue ownership"),
        empty_owner_snapshot,
    );
    assert_eq!(queue.fifo_snapshot_for_test(), original_fifo);

    let leader_index =
        usize::try_from(adapter.context.leader(0)).expect("empty-carrier leader index");
    let carrier_header = adapter
        .merge_carrier_context_header(0)
        .expect("exact empty-carrier round header");
    let carrier = BlockBuilder::new(carrier_header)
        .build_with_signature(
            u64::try_from(leader_index).expect("leader index fits u64"),
            replica_keys[leader_index].private_key(),
        )
        .canonical_resultless_proposal();
    let (locked_round, _locked_subject) = mark_global_body_locked_for_block(&mut adapter, &carrier);
    assert!(
        adapter
            .pending_autonomous_anchor_payloads
            .values()
            .any(|pending| pending == &payload),
        "the lock alone cannot retire a pending follower payload",
    );
    assert_eq!(
        queue
            .lane_reservation_reconciliation_snapshot()
            .expect("recheck locked follower Queue ownership"),
        empty_owner_snapshot,
    );
    assert_eq!(
        adapter.bind_locked_global_body(&carrier),
        V2LaneIngressOutcome::Duplicate,
        "the empty canonical carrier must retire the omitted follower payload",
    );

    let expected_retirement = crate::kura::AutonomousLaneSlotRetirementV1::from_payload(&payload);
    let retired = adapter
        .kura
        .read_autonomous_lane_retired_attempt(
            lane_id,
            lane_block_height,
            proposal_height,
            network_id,
            epoch,
        )
        .expect("read follower retirement custody")
        .expect("the losing follower attempt is durably retired");
    assert_eq!(retired.artifact.executable_payload, payload);
    assert_eq!(retired.retirement, expected_retirement);
    assert!(adapter.pending_autonomous_anchor_payloads.is_empty());
    assert!(adapter.autonomous_payloads.is_empty());
    assert!(adapter.autonomous_payload_order.is_empty());
    assert!(adapter.autonomous_payload_views.is_empty());
    assert_eq!(
        queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture retired follower Queue ownership"),
        empty_owner_snapshot,
        "FIFO-only retirement must not manufacture a Queue owner family",
    );
    assert!(queue.live_lane_reservations().is_empty());
    assert_eq!(queue.fifo_snapshot_for_test(), original_fifo);
    assert_eq!(
        std::fs::read(&journal_path).expect("reread follower reservation journal"),
        reservation_journal_before,
        "FIFO-only retirement must stutter the reservation journal",
    );
    assert_eq!(
        std::fs::read(&plan_journal_path).expect("reread follower QueuePlan journal"),
        plan_journal_before,
        "FIFO-only retirement must stutter the QueuePlan journal",
    );
    assert!(
        adapter
            .kura
            .pending_autonomous_lifecycle_terminal_outcome_inventory()
            .expect("inspect follower terminal outcomes")
            .is_empty(),
        "the live retirement must consume its Pending terminal outcome",
    );
    assert!(!adapter.output_guard.restart_required());
    let ordinary_winner = proposal_for_route(
        &adapter,
        &replica_keys,
        lane_id,
        dataspace_id,
        descriptor.lane_incarnation,
        proposal_height,
        lane_block_height,
    );
    assert_ne!(ordinary_winner, payload.origin_proposal);
    let ordinary_session = committed_lane_session(&ordinary_winner, &replica_keys);
    let ordinary_pops = adapter.pops_for_lane_session(&ordinary_session);
    let ordinary_authority = crate::state::CertifiedLaneBlockPersistenceAuthority::for_test(
        lane_id,
        dataspace_id,
        descriptor.lane_incarnation,
        None,
    );
    adapter
        .kura
        .persist_committed_lane_block_session_with_authority(
            &ordinary_session,
            &ordinary_pops,
            &ordinary_authority,
        )
        .expect(
            "a State-authorized ordinary winner may follow a Complete replica Queue disposition",
        );
    assert_eq!(
        adapter
            .kura
            .read_certified_lane_block_artifact(lane_id, lane_block_height)
            .expect("read ordinary winner after replica terminalization")
            .proposal,
        ordinary_winner,
    );
    assert_eq!(
        accept_lane_message_from(
            &mut adapter,
            BlockMessage::LaneExecutablePayload(payload),
            producer,
            locked_round.view,
        ),
        V2LaneIngressOutcome::Rejected,
        "a delayed payload cannot reclaim the durably retired follower slot",
    );
}
#[test]
fn canonical_kura_anchor_cannot_bypass_route_reset_or_incarnation_guards() {
    let lane_id = LaneId::SINGLE;
    let dataspace_id = DataSpaceId::UNIVERSAL;
    {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 3);
        let incarnation = adapter
            .state
            .lane_incarnation_at_height(lane_id, adapter.context.height)
            .expect("canonical lane incarnation is active");
        let proposal = proposal_for_route(
            &adapter,
            &keys,
            lane_id,
            dataspace_id,
            incarnation,
            adapter.context.height,
            1,
        );
        let canonical = store_canonical_anchor(&adapter, &proposal, &keys[0]);
        assert!(
            adapter.kura.read_lane_block_artifact(lane_id, 1).is_some(),
            "fixture must retain a raw canonical Kura anchor"
        );
        assert!(adapter.canonical_anchor_for_proposal(&canonical).is_some());
        assert!(
            adapter
                .canonical_proposal_for_vote_body(&canonical.vote_body(CertPhase::Prepare))
                .is_some()
        );
        mark_lane_reset(&adapter, lane_id, adapter.context.height);
        assert!(
            adapter.kura.read_lane_block_artifact(lane_id, 1).is_some(),
            "reset validation must be tested with the canonical file still present"
        );
        assert!(
            adapter.canonical_anchor_for_proposal(&canonical).is_none(),
            "a canonical file at the reset watermark is not an admissible anchor"
        );
        assert!(
            adapter
                .canonical_proposal_for_vote_body(&canonical.vote_body(CertPhase::Prepare))
                .is_none(),
            "historical vote recovery must apply the reset guard too"
        );
        assert!(
            !adapter.lane_proposal_authorized(&canonical, None, true, 0),
            "canonical-anchor fast path must not bypass the reset guard"
        );
    }
    {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 2);
        let incarnation = adapter
            .state
            .lane_incarnation_at_height(lane_id, adapter.context.height)
            .expect("canonical lane incarnation is active");
        let wrong_dataspace = DataSpaceId::new(91);
        let proposal = proposal_for_route(
            &adapter,
            &keys,
            lane_id,
            wrong_dataspace,
            incarnation,
            adapter.context.height,
            1,
        );
        if let Some(canonical) = try_store_canonical_anchor(&adapter, &proposal, &keys[0]) {
            assert!(
                adapter.canonical_anchor_for_proposal(&canonical).is_none(),
                "canonical storage must not make an inactive dataspace route authoritative"
            );
            assert!(
                adapter
                    .canonical_proposal_for_vote_body(&canonical.vote_body(CertPhase::Prepare))
                    .is_none()
            );
            assert!(!adapter.lane_proposal_authorized(&canonical, None, true, 0));
        } else {
            assert!(
                adapter.kura.read_lane_block_artifact(lane_id, 1).is_none(),
                "Kura must not expose an artifact rejected for inactive route geometry"
            );
        }
    }
    {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 2);
        let active_incarnation = adapter
            .state
            .lane_incarnation_at_height(lane_id, adapter.context.height)
            .expect("canonical lane incarnation is active");
        let stale_incarnation = Hash::new(b"canonical-but-retired-lane-incarnation");
        assert_ne!(stale_incarnation, active_incarnation);
        let proposal = proposal_for_route(
            &adapter,
            &keys,
            lane_id,
            dataspace_id,
            stale_incarnation,
            adapter.context.height,
            1,
        );
        if let Some(canonical) = try_store_canonical_anchor(&adapter, &proposal, &keys[0]) {
            assert!(
                adapter.canonical_anchor_for_proposal(&canonical).is_none(),
                "canonical storage must not authorize a retired incarnation"
            );
            assert!(
                adapter
                    .canonical_proposal_for_vote_body(&canonical.vote_body(CertPhase::Prepare))
                    .is_none()
            );
            assert!(!adapter.lane_proposal_authorized(&canonical, None, true, 0));
        } else {
            assert!(
                adapter.kura.read_lane_block_artifact(lane_id, 1).is_none(),
                "Kura must not expose an artifact rejected for a retired incarnation"
            );
        }
    }
}
#[test]
fn merge_certificates_require_exact_equal_vote_quorum_and_leader_custody() {
    let (adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let all_signatures = (0_u32..4)
        .map(|signer| (signer, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        exact_merge_certificate_signers(&all_signatures, 3, 3),
        Some(vec![0, 1, 3]),
        "exact QC construction must retain a high-index round leader"
    );
    let signatures_without_leader = (0_u32..3)
        .map(|signer| (signer, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        exact_merge_certificate_signers(&signatures_without_leader, 3, 3),
        None,
        "new QC construction waits for its durable round-leader custodian"
    );
    assert!(!adapter.frozen_certificate_quorum_met(&[0, 1]));
    assert!(adapter.frozen_certificate_quorum_met(&[1, 2, 3]));
    assert!(adapter.frozen_certificate_quorum_met(&[0, 1, 3]));
    assert!(!adapter.frozen_certificate_quorum_met(&[0, 1, 2, 3]));
    let carrier_view = 0;
    let leader = usize::try_from(adapter.context.leader(carrier_view))
        .expect("fixture leader index fits usize");
    let subquorum = missing_sidecar_reference_with_signers(&adapter, &keys, carrier_view, &[1, 2]);
    assert!(matches!(
        authenticate_bounded_merge_sidecar_holders(&adapter.context, &subquorum),
        Err(reason) if reason.contains("signer count mismatch")
    ));
    let without_leader = (0..adapter.context.roster.len())
        .filter(|index| *index != leader)
        .collect::<Vec<_>>();
    let legacy_without_leader =
        missing_sidecar_reference_with_signers(&adapter, &keys, carrier_view, &without_leader);
    let legacy_holders =
        authenticate_bounded_merge_sidecar_holders(&adapter.context, &legacy_without_leader)
            .expect("a pre-fix V1 quorum retains implicit carrier-leader custody");
    assert_eq!(
        legacy_holders.first(),
        Some(&adapter.context.roster[leader].validator),
        "the authenticated carrier leader precedes legacy bitmap signers"
    );
    let quorum = missing_sidecar_reference(&adapter, &keys, carrier_view);
    let holders = authenticate_bounded_merge_sidecar_holders(&adapter.context, &quorum)
        .expect("an exact quorum with leader custody is authenticated");
    assert_eq!(
        holders.first(),
        Some(&adapter.context.roster[leader].validator),
        "the durable round-leader custodian must be first"
    );
    let superset =
        missing_sidecar_reference_with_signers(&adapter, &keys, carrier_view, &[0, 1, 2, 3]);
    assert!(matches!(
        authenticate_bounded_merge_sidecar_holders(&adapter.context, &superset),
        Err(reason) if reason.contains("expected exactly 3, got 4")
    ));
}
#[test]
fn missing_merge_sidecar_contacts_durable_round_leader_before_other_signers() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let local_index = adapter
        .context
        .roster
        .iter()
        .position(|entry| entry.validator == adapter.local_peer)
        .expect("fixture local peer belongs to the frozen roster");
    let carrier_view = (0..u64::try_from(adapter.context.roster.len()).expect("roster fits u64"))
        .find(|view| {
            usize::try_from(adapter.context.leader(*view)).expect("leader index fits usize")
                != local_index
        })
        .expect("four-validator fixture has a remote round leader");
    let leader_index =
        usize::try_from(adapter.context.leader(carrier_view)).expect("leader index fits usize");
    let leader = adapter.context.roster[leader_index].validator.clone();
    let reference = missing_sidecar_reference(&adapter, &keys, carrier_view);
    let round = wire::ConsensusRound {
        context_id: adapter.context.id(),
        height: adapter.context.height,
        view: carrier_view,
    };
    let subject = wire::BlockSubject {
        parent_block_hash: adapter
            .context
            .parent_commit_qc
            .as_ref()
            .map(|qc| qc.subject.block_hash),
        block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"leader-first merge sidecar carrier",
        )),
        payload_hash: Hash::new(b"leader-first merge sidecar payload"),
    };
    assert_eq!(
        adapter
            .defer_missing_merge_sidecar(round, subject, reference)
            .expect("authenticate and register the missing exact sidecar"),
        MergeSidecarDeferralDisposition::Fetching
    );
    let effect = adapter
        .drain_effects(usize::MAX)
        .into_iter()
        .find(|effect| {
            matches!(
                effect,
                V2LaneWorkEffect::PostCertifiedMergeSidecar {
                    message,
                    ..
                } if matches!(message.as_ref(), CertifiedMergeSidecarMessage::Request(_))
            )
        })
        .expect("registration emits one exact sidecar request");
    let V2LaneWorkEffect::PostCertifiedMergeSidecar { peer, message, .. } = effect else {
        unreachable!("selected effect is a sidecar post")
    };
    let CertifiedMergeSidecarMessage::Request(request) = message.as_ref() else {
        unreachable!("selected sidecar post is a request")
    };
    assert_eq!(peer, leader);
    assert_eq!(request.responder, leader);
}
fn merge_candidate_for_persistence_retry(
    adapter: &V2LaneWorkAdapter,
    view: wire::View,
) -> crate::merge::MergeLedgerCandidate {
    let nexus = adapter.state.nexus_snapshot();
    let active_lanes = nexus
        .lane_catalog
        .lanes()
        .iter()
        .map(|lane| iroha_data_model::merge::MergeLaneBinding {
            lane_id: lane.id,
            dataspace_id: lane.dataspace_id,
            lane_config_hash: crate::merge::merge_lane_config_hash(lane),
            incarnation: adapter
                .state
                .lane_incarnation_at_height(lane.id, adapter.context.height)
                .expect("fixture lane incarnation is active"),
            activation_height: 1,
        })
        .collect::<Vec<_>>();
    let incarnation_entries = active_lanes
        .iter()
        .map(
            |binding| iroha_data_model::nexus::LaneLifecycleIncarnationEntry {
                lane_id: binding.lane_id,
                incarnation: binding.incarnation,
            },
        )
        .collect::<Vec<_>>();
    crate::merge::MergeLedgerCandidate {
        version: crate::merge::MergeLedgerCandidate::VERSION,
        epoch_id: 1,
        view,
        carrier_height: adapter.context.height,
        carrier_parent_hash: adapter
            .context
            .parent_commit_qc
            .as_ref()
            .expect("non-genesis fixture parent")
            .subject
            .block_hash,
        lane_catalog_hash: iroha_data_model::nexus::LaneLifecycleParameterV1::catalog_hash(
            &nexus.lane_catalog,
        ),
        active_lanes: active_lanes.clone(),
        incarnation_root: iroha_data_model::nexus::LaneLifecycleParameterV1::incarnation_root(
            &incarnation_entries,
        ),
        activation_root: crate::merge::merge_activation_root(&active_lanes),
        lane_snapshots: Vec::new(),
        lane_drain_certificates: Vec::new(),
        execution_batch: None,
        global_state_root: crate::merge::reduce_merge_hint_roots(&[]),
    }
}
#[test]
fn merge_candidate_selection_preserves_authorized_digest_and_relay_priority() {
    let relay_digest = Hash::new(b"relay candidate");
    let installed_digest = Hash::new(b"installed candidate");
    let digest = |candidate: &(u8, Hash)| candidate.1;
    assert_eq!(
        preferred_merge_candidates(
            None,
            vec![(2, relay_digest)],
            vec![(3, installed_digest)],
            digest,
        ),
        vec![(2, relay_digest)],
        "the deterministic leader candidate takes priority over opportunistic installation"
    );
    assert_eq!(
        preferred_merge_candidates(
            Some(relay_digest),
            vec![(2, relay_digest)],
            vec![(3, installed_digest)],
            digest,
        ),
        vec![(2, relay_digest)],
        "a durable signing decision must survive later candidate installation"
    );
    assert!(
        preferred_merge_candidates(
            Some(Hash::new(b"unavailable authorized candidate")),
            vec![(2, relay_digest)],
            vec![(3, installed_digest)],
            digest,
        )
        .is_empty(),
        "an unavailable durable decision must fail closed instead of selecting another digest"
    );
}
#[test]
fn installed_execution_candidate_with_wrong_carrier_context_never_reaches_local_signing() {
    let (mut adapter, _) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    adapter
        .retain_merge_sidecars_for_global_view(0, None, None)
        .expect("install exact unlocked reducer directive");
    adapter.drain_effects(usize::MAX);
    let mut candidate = merge_candidate_for_persistence_retry(&adapter, 0);
    candidate.execution_batch = Some(iroha_data_model::merge::MergeExecutionBatch {
        version: 1,
        base_state_height: adapter.context.height.saturating_sub(1),
        base_state_hash: HashOf::from_untyped_unchecked(Hash::new(b"retired execution base state")),
        application_block_header: BlockHeader::new(
            NonZeroU64::new(adapter.context.height).expect("non-zero carrier height"),
            Some(candidate.carrier_parent_hash),
            None,
            None,
            1,
            candidate.view,
        ),
        lanes: Vec::new(),
        entrypoint_count: 1,
        entrypoint_merkle_root: HashOf::from_untyped_unchecked(Hash::new(
            b"retired execution entrypoints",
        )),
        result_merkle_root: HashOf::from_untyped_unchecked(Hash::new(b"retired execution results")),
        execution_root: Hash::new(b"retired execution root"),
        application_write_set_root: Hash::new(b"retired execution application writes"),
        write_set_root: Hash::new(b"retired execution writes"),
        expected_post_state_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"retired execution post state",
        )),
        batch_hash: Hash::new(b"retired execution batch"),
    });
    let digest = crate::merge::merge_qc_message_digest(
        &adapter.context.network_id,
        &candidate,
        VALIDATOR_SET_HASH_VERSION_V1,
        adapter.frozen_validator_set_hash(),
    );
    let key = MergeKey {
        epoch_id: candidate.epoch_id,
        view: candidate.view,
        digest,
    };
    adapter.merge_entries.insert(
        key,
        PendingMerge {
            stage: PendingMergeStage::Collecting(candidate),
            signatures: BTreeMap::new(),
        },
    );
    adapter
        .refresh_merge_candidates(0)
        .expect("carrier-mismatched execution candidate fails closed without signing");
    assert!(adapter.merge_entries[&key].signatures.is_empty());
    assert!(
        adapter
            .drain_effects(usize::MAX)
            .iter()
            .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_))),
        "a carrier-mismatched execution-batch candidate must not reach the private key"
    );
}
fn merge_signing_context_for_test(
    adapter: &V2LaneWorkAdapter,
    candidate: &crate::merge::MergeLedgerCandidate,
) -> MergeSigningContextV1 {
    MergeSigningContextV1 {
        epoch_id: candidate.epoch_id,
        view: candidate.view,
        carrier_height: candidate.carrier_height,
        parent_hash: candidate.carrier_parent_hash,
        validator_set_hash: adapter.frozen_validator_set_hash(),
    }
}
fn remote_merge_leader_view(adapter: &V2LaneWorkAdapter) -> wire::View {
    let local = adapter
        .local_validator_index()
        .expect("fixture local validator is in the frozen roster");
    let search_bound = u64::try_from(adapter.context.roster.len())
        .expect("fixture roster length fits u64")
        .saturating_mul(2);
    (0..search_bound)
        .find(|view| adapter.context.leader(*view) != local)
        .expect("rotating leader schedule reaches a remote validator")
}
fn signed_merge_share_for_test(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    candidate: &crate::merge::MergeLedgerCandidate,
    signer: wire::ValidatorIndex,
) -> MergeCommitteeSignature {
    let digest = crate::merge::merge_qc_message_digest(
        &adapter.context.network_id,
        candidate,
        VALIDATOR_SET_HASH_VERSION_V1,
        adapter.frozen_validator_set_hash(),
    );
    let signature = Signature::try_new(
        keys[usize::try_from(signer).expect("fixture signer index fits usize")].private_key(),
        digest.as_ref(),
    )
    .expect("sign merge-share test fixture")
    .payload()
    .to_vec();
    MergeCommitteeSignature {
        version: MERGE_COMMITTEE_SIGNATURE_VERSION_V2,
        epoch_id: candidate.epoch_id,
        view: candidate.view,
        signer,
        message_digest: digest,
        bls_sig: signature,
        leader_candidate_body: (signer == adapter.context.leader(candidate.view))
            .then(|| candidate.canonical_bytes()),
    }
}
fn synthetic_merge_execution_batch_for_test(
    adapter: &V2LaneWorkAdapter,
    application_block_header: BlockHeader,
) -> iroha_data_model::merge::MergeExecutionBatch {
    iroha_data_model::merge::MergeExecutionBatch {
        version: 1,
        base_state_height: adapter.context.height.saturating_sub(1),
        base_state_hash: adapter.state.lane_execution_state_hash(),
        application_block_header,
        lanes: Vec::new(),
        entrypoint_count: 0,
        entrypoint_merkle_root: HashOf::from_untyped_unchecked(Hash::new(
            b"synthetic execution entrypoints",
        )),
        result_merkle_root: HashOf::from_untyped_unchecked(Hash::new(
            b"synthetic execution results",
        )),
        execution_root: Hash::new(b"synthetic execution root"),
        application_write_set_root: Hash::new(b"synthetic execution application writes"),
        write_set_root: Hash::new(b"synthetic execution writes"),
        expected_post_state_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"synthetic execution post state",
        )),
        batch_hash: Hash::new(b"synthetic execution batch"),
    }
}
#[test]
fn authenticated_leader_candidate_recovers_exact_follower_share_after_restart() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let view = remote_merge_leader_view(&adapter);
    let candidate =
        record_production_merge_candidate_for_persistence_retry(&mut adapter, &keys, view);
    let candidate_bytes = candidate.canonical_bytes();
    let leader = adapter.context.leader(view);
    let local = adapter
        .local_validator_index()
        .expect("fixture local validator is in the frozen roster");
    assert_ne!(leader, local);
    adapter
        .retain_merge_sidecars_for_global_view(view, None, None)
        .expect("install exact unlocked follower directive");
    assert!(
        adapter
            .drain_effects(usize::MAX)
            .iter()
            .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_))),
        "a follower must not select or sign a proposer-local candidate"
    );
    let leader_share = signed_merge_share_for_test(&adapter, &keys, &candidate, leader);
    let leader_digest = leader_share.message_digest;
    assert_eq!(
        leader_share.leader_candidate_body.as_deref(),
        Some(candidate_bytes.as_slice())
    );
    assert_eq!(
        adapter
            .accept_merge_signature(leader_share.clone(), view)
            .expect("admit authenticated leader candidate"),
        V2LaneIngressOutcome::Inserted
    );
    let validation_checks_after_admission = adapter.merge_candidate_validation_checks.get();
    assert_eq!(
        adapter
            .accept_merge_signature(leader_share.clone(), view)
            .expect("re-admit exact leader candidate"),
        V2LaneIngressOutcome::Duplicate
    );
    assert_eq!(
        adapter.merge_candidate_validation_checks.get(),
        validation_checks_after_admission,
        "an exact authenticated replay must not reexecute its admitted candidate"
    );
    let mut substituted = leader_share;
    let mut substituted_candidate = candidate.clone();
    substituted_candidate.global_state_root = Hash::new(b"substituted replay body");
    substituted.leader_candidate_body = Some(substituted_candidate.canonical_bytes());
    assert_eq!(
        adapter
            .accept_merge_signature(substituted, view)
            .expect("reject a replay carrying substituted leader bytes"),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(
        adapter.merge_candidate_validation_checks.get(),
        validation_checks_after_admission,
        "a substituted replay body must be rejected before semantic execution"
    );
    let follower_share = adapter
        .drain_effects(usize::MAX)
        .into_iter()
        .find_map(|effect| match effect {
            V2LaneWorkEffect::BroadcastMerge(share) if share.signer == local => Some(share),
            _ => None,
        })
        .expect("leader admission releases one local follower share");
    assert_eq!(follower_share.version, MERGE_COMMITTEE_SIGNATURE_VERSION_V2);
    assert_eq!(follower_share.message_digest, leader_digest);
    assert!(
        follower_share.leader_candidate_body.is_none(),
        "a follower transmission must remain bodyless"
    );
    assert_eq!(
        adapter
            .merge_signing_guard
            .as_ref()
            .expect("voting adapter has merge signing guard")
            .authorized_candidate(&merge_signing_context_for_test(&adapter, &candidate))
            .expect("read exact durable candidate"),
        Some((
            follower_share.message_digest,
            candidate.clone(),
            candidate_bytes.clone(),
        ))
    );
    let context = adapter.context.clone();
    let restart = LaneAdapterRestartParts::capture(&adapter);
    drop(adapter);
    let mut reopened = restart
        .reopen(context, true)
        .expect("reopen adapter with exact pre-QC journal");
    reopened
        .retain_merge_sidecars_for_global_view(view, None, None)
        .expect("reconstruct exact candidate under the follower directive");
    let recovered = reopened
        .drain_effects(usize::MAX)
        .into_iter()
        .find_map(|effect| match effect {
            V2LaneWorkEffect::BroadcastMerge(share) if share.signer == local => Some(share),
            _ => None,
        })
        .expect("restart reconstructs the exact follower share");
    assert_eq!(recovered.message_digest, follower_share.message_digest);
    assert!(recovered.leader_candidate_body.is_none());
    reopened
        .schedule_merge_share_retransmissions(view)
        .expect("schedule exact follower retransmission");
    let retransmitted = reopened
        .drain_effects(usize::MAX)
        .into_iter()
        .find_map(|effect| match effect {
            V2LaneWorkEffect::BroadcastMerge(share) if share.signer == local => Some(share),
            _ => None,
        })
        .expect("retransmit recovered follower share");
    assert_eq!(retransmitted, recovered);
    assert_eq!(
        reopened
            .merge_signing_guard
            .as_ref()
            .expect("voting adapter has merge signing guard")
            .authorized_candidate(&merge_signing_context_for_test(&reopened, &candidate))
            .expect("read restarted exact durable candidate"),
        Some((recovered.message_digest, candidate, candidate_bytes,))
    );
}
#[test]
fn merge_share_transport_rejects_omission_nonleader_body_and_legacy_version() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let view = remote_merge_leader_view(&adapter);
    let candidate =
        record_production_merge_candidate_for_persistence_retry(&mut adapter, &keys, view);
    let leader = adapter.context.leader(view);
    let follower = adapter
        .local_validator_index()
        .expect("fixture local validator is in the frozen roster");
    adapter
        .retain_merge_sidecars_for_global_view(view, None, None)
        .expect("install exact unlocked follower directive");
    adapter.drain_effects(usize::MAX);
    let mut omitted = signed_merge_share_for_test(&adapter, &keys, &candidate, leader);
    omitted.leader_candidate_body = None;
    assert_eq!(
        adapter
            .accept_merge_signature(omitted, view)
            .expect("reject omitted leader body"),
        V2LaneIngressOutcome::Rejected
    );
    let mut legacy = signed_merge_share_for_test(&adapter, &keys, &candidate, leader);
    legacy.version = MERGE_COMMITTEE_SIGNATURE_VERSION_V2.saturating_sub(1);
    assert_eq!(
        adapter
            .accept_merge_signature(legacy, view)
            .expect("reject legacy merge-share version"),
        V2LaneIngressOutcome::Rejected
    );
    let mut follower_with_body = signed_merge_share_for_test(&adapter, &keys, &candidate, follower);
    assert!(follower_with_body.leader_candidate_body.is_none());
    follower_with_body.leader_candidate_body = Some(candidate.canonical_bytes());
    assert_eq!(
        adapter
            .accept_merge_signature(follower_with_body, view)
            .expect("reject nonleader candidate body"),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(
        adapter
            .merge_signing_guard
            .as_ref()
            .expect("voting adapter has merge signing guard")
            .authorized_candidate(&merge_signing_context_for_test(&adapter, &candidate))
            .expect("read untouched signing guard"),
        None
    );
}
#[test]
fn merge_leader_candidate_body_is_canonical_under_ambient_layout() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let view = remote_merge_leader_view(&adapter);
    let candidate =
        record_production_merge_candidate_for_persistence_retry(&mut adapter, &keys, view);
    adapter
        .retain_merge_sidecars_for_global_view(view, None, None)
        .expect("install exact unlocked follower directive");
    adapter.drain_effects(usize::MAX);
    let leader = adapter.context.leader(view);
    let share = signed_merge_share_for_test(&adapter, &keys, &candidate, leader);
    let parent = adapter
        .state
        .latest_block_header_fast()
        .expect("fixture has exact committed parent");
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let decoded = {
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        adapter.decode_and_validate_leader_candidate(&share, view, &parent)
    }
    .expect("canonical leader body remains valid under alternate ambient flags");
    assert_eq!(decoded, candidate);
    let canonical_body =
        norito::encode_canonical(&candidate).expect("encode canonical merge candidate");
    assert_eq!(
        share.leader_candidate_body.as_deref(),
        Some(canonical_body.as_slice())
    );
    let alternate_body = {
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::to_bytes(&candidate).expect("encode alternate-layout merge candidate")
    };
    assert_ne!(alternate_body, canonical_body);
    let mut noncanonical = share;
    noncanonical.leader_candidate_body = Some(alternate_body);
    let reason = {
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        adapter
            .decode_and_validate_leader_candidate(&noncanonical, view, &parent)
            .expect_err("alternate-layout leader body must fail closed")
    };
    assert!(
        reason.contains("not canonical"),
        "unexpected alternate-layout rejection: {reason}"
    );
}
#[test]
fn merge_leader_candidate_rejects_substitution_outer_epoch_and_oversize_before_journal() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let view = remote_merge_leader_view(&adapter);
    let candidate =
        record_production_merge_candidate_for_persistence_retry(&mut adapter, &keys, view);
    let leader = adapter.context.leader(view);
    adapter
        .retain_merge_sidecars_for_global_view(view, None, None)
        .expect("install exact unlocked follower directive");
    adapter.drain_effects(usize::MAX);
    let mut substituted = candidate.clone();
    substituted.global_state_root = Hash::new(b"authenticated substituted merge body");
    let mut substituted_share = signed_merge_share_for_test(&adapter, &keys, &candidate, leader);
    substituted_share.leader_candidate_body = Some(substituted.canonical_bytes());
    assert_eq!(
        adapter
            .accept_merge_signature(substituted_share, view)
            .expect("reject body substitution"),
        V2LaneIngressOutcome::Rejected
    );
    let mut wrong_outer_epoch = signed_merge_share_for_test(&adapter, &keys, &candidate, leader);
    wrong_outer_epoch.epoch_id = wrong_outer_epoch.epoch_id.saturating_add(1);
    assert_eq!(
        adapter
            .accept_merge_signature(wrong_outer_epoch, view)
            .expect("reject outer epoch drift"),
        V2LaneIngressOutcome::Rejected
    );
    adapter.limits.merge_share_frame_capacity =
        iroha_config::parameters::defaults::sumeragi::V2_MERGE_LEADER_BODY_FRAME_HEADROOM_BYTES;
    let oversize = signed_merge_share_for_test(&adapter, &keys, &candidate, leader);
    assert_eq!(
        adapter
            .accept_merge_signature(oversize, view)
            .expect("reject candidate outside configured full-frame partition"),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(
        adapter
            .merge_signing_guard
            .as_ref()
            .expect("voting adapter has merge signing guard")
            .authorized_candidate(&merge_signing_context_for_test(&adapter, &candidate))
            .expect("read untouched signing guard"),
        None
    );
}
#[test]
fn authenticated_execution_candidate_rejects_noncanonical_carrier_context_header() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let view = remote_merge_leader_view(&adapter);
    let mut candidate = merge_candidate_for_persistence_retry(&adapter, view);
    let expected_header = adapter
        .merge_carrier_context_header(view)
        .expect("derive exact deterministic carrier context");
    let wrong_creation_time = u64::try_from(expected_header.creation_time().as_millis())
        .expect("fixture carrier time fits u64")
        .checked_add(1)
        .expect("fixture carrier time can advance");
    let wrong_header = BlockHeader::new(
        expected_header.height(),
        expected_header.prev_block_hash(),
        None,
        None,
        wrong_creation_time,
        expected_header.view_change_index(),
    );
    assert_ne!(wrong_header, expected_header);
    candidate.execution_batch = Some(synthetic_merge_execution_batch_for_test(
        &adapter,
        wrong_header,
    ));
    assert!(
        candidate.lane_snapshots.is_empty(),
        "the carrier-context test must not be preempted by mixed candidate forms"
    );
    let leader = adapter.context.leader(view);
    let share = signed_merge_share_for_test(&adapter, &keys, &candidate, leader);
    adapter
        .retain_merge_sidecars_for_global_view(view, None, None)
        .expect("install exact unlocked follower directive");
    adapter.drain_effects(usize::MAX);
    let parent = adapter
        .state
        .latest_block_header_fast()
        .expect("fixture has exact committed parent");
    let reason = adapter
        .decode_and_validate_leader_candidate(&share, view, &parent)
        .expect_err("wrong-time execution candidate must not obtain a follower share");
    assert!(
        reason.contains("exact deterministic carrier context header"),
        "unexpected carrier-context rejection: {reason}"
    );
    assert_eq!(
        adapter
            .accept_merge_signature(share, view)
            .expect("reject execution candidate for an uncarryable header"),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(
        adapter
            .merge_signing_guard
            .as_ref()
            .expect("voting adapter has merge signing guard")
            .authorized_candidate(&merge_signing_context_for_test(&adapter, &candidate))
            .expect("read untouched signing guard"),
        None
    );
}
#[test]
fn authenticated_relay_candidate_cannot_be_relabelled_as_execution() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let view = remote_merge_leader_view(&adapter);
    let mut candidate =
        record_production_merge_candidate_for_persistence_retry(&mut adapter, &keys, view);
    let exact_header = adapter
        .merge_carrier_context_header(view)
        .expect("derive exact deterministic carrier context");
    candidate.execution_batch = Some(synthetic_merge_execution_batch_for_test(
        &adapter,
        exact_header,
    ));
    let leader = adapter.context.leader(view);
    let share = signed_merge_share_for_test(&adapter, &keys, &candidate, leader);
    adapter
        .retain_merge_sidecars_for_global_view(view, None, None)
        .expect("install exact unlocked follower directive");
    adapter.drain_effects(usize::MAX);
    let parent = adapter
        .state
        .latest_block_header_fast()
        .expect("fixture has exact committed parent");
    let reason = adapter
        .decode_and_validate_leader_candidate(&share, view, &parent)
        .expect_err("relay snapshots cannot be relabeled as autonomous execution");
    assert!(
        reason.contains("execution candidates must not mix relay snapshots"),
        "unexpected authenticated execution rejection: {reason}"
    );
    assert_eq!(
        adapter
            .accept_merge_signature(share, view)
            .expect("reject unmarked autonomous candidate"),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(
        adapter
            .merge_signing_guard
            .as_ref()
            .expect("voting adapter has merge signing guard")
            .authorized_candidate(&merge_signing_context_for_test(&adapter, &candidate))
            .expect("read untouched signing guard"),
        None
    );
}
#[test]
fn durable_local_merge_claim_rejects_same_context_candidate_drift() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let candidate = record_production_merge_candidate_for_persistence_retry(&mut adapter, &keys, 0);
    let signer = adapter
        .local_validator_index()
        .expect("fixture local validator is in the frozen roster");
    let first_digest = crate::merge::merge_qc_message_digest(
        &adapter.context.network_id,
        &candidate,
        VALIDATOR_SET_HASH_VERSION_V1,
        adapter.frozen_validator_set_hash(),
    );
    adapter
        .retain_merge_sidecars_for_global_view(0, None, None)
        .expect("install exact unlocked reducer directive");
    assert_eq!(
        adapter
            .merge_claims
            .get(&(candidate.epoch_id, candidate.view, signer)),
        Some(&first_digest)
    );
    let mut drifted = candidate.clone();
    drifted.global_state_root = Hash::new(b"same-context conflicting merge payload");
    let drifted_digest = crate::merge::merge_qc_message_digest(
        &adapter.context.network_id,
        &drifted,
        VALIDATOR_SET_HASH_VERSION_V1,
        adapter.frozen_validator_set_hash(),
    );
    assert_ne!(first_digest, drifted_digest);
    assert_eq!(
        adapter.authorize_local_merge_claim(&drifted, 0, signer, drifted_digest),
        Err(MergeSidecarError::LocalSigningEquivocation)
    );
    assert_eq!(
        adapter
            .merge_claims
            .get(&(candidate.epoch_id, candidate.view, signer)),
        Some(&first_digest),
        "a conflicting candidate must never overwrite the in-memory decision"
    );
    assert_eq!(
        adapter
            .merge_signing_guard
            .as_ref()
            .expect("voting adapter has merge signing guard")
            .authorized_digest(&merge_signing_context_for_test(&adapter, &candidate))
            .expect("read durable exact-context decision"),
        Some(first_digest),
        "a conflicting candidate must never overwrite the durable decision"
    );
}
#[test]
fn durable_local_merge_claim_rejects_conflict_after_adapter_reopen() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let candidate = record_production_merge_candidate_for_persistence_retry(&mut adapter, &keys, 0);
    let signer = adapter
        .local_validator_index()
        .expect("fixture local validator is in the frozen roster");
    let first_digest = crate::merge::merge_qc_message_digest(
        &adapter.context.network_id,
        &candidate,
        VALIDATOR_SET_HASH_VERSION_V1,
        adapter.frozen_validator_set_hash(),
    );
    adapter
        .retain_merge_sidecars_for_global_view(0, None, None)
        .expect("authorize pre-restart merge decision from the unlocked directive");
    let context = adapter.context.clone();
    let restart = LaneAdapterRestartParts::capture(&adapter);
    drop(adapter);
    let mut reopened = restart
        .reopen(context, true)
        .expect("reopen adapter against the same committed frontier");
    assert!(
        reopened
            .drain_effects(usize::MAX)
            .iter()
            .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_))),
        "constructor must not emit a merge share before reducer recovery"
    );
    reopened
        .retain_merge_sidecars_for_global_view(0, None, None)
        .expect("install reopened exact unlocked directive");
    assert!(
            reopened
                .drain_effects(usize::MAX)
                .iter()
                .any(|effect| matches!(effect, V2LaneWorkEffect::BroadcastMerge(signature) if signature.message_digest == first_digest)),
            "the exact unlocked directive may release the recovered candidate share"
        );
    reopened.merge_claims.clear();
    reopened.merge_entries.clear();
    reopened.purge_queued_merge_broadcasts();
    let mut drifted = candidate.clone();
    drifted.global_state_root = Hash::new(b"post-restart conflicting merge payload");
    let drifted_digest = crate::merge::merge_qc_message_digest(
        &reopened.context.network_id,
        &drifted,
        VALIDATOR_SET_HASH_VERSION_V1,
        reopened.frozen_validator_set_hash(),
    );
    assert_eq!(
        reopened.authorize_local_merge_claim(&drifted, 0, signer, drifted_digest),
        Err(MergeSidecarError::LocalSigningEquivocation)
    );
    assert!(
        reopened
            .merge_claims
            .get(&(candidate.epoch_id, candidate.view, signer))
            .is_none(),
        "restart rejection must not manufacture a conflicting in-memory claim"
    );
    assert_eq!(
        reopened
            .merge_signing_guard
            .as_ref()
            .expect("voting adapter has merge signing guard")
            .authorized_digest(&merge_signing_context_for_test(&reopened, &candidate))
            .expect("read restarted durable decision"),
        Some(first_digest)
    );
}
#[test]
fn locked_later_view_directive_purges_queued_merge_shares_and_disables_retry() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let candidate = record_production_merge_candidate_for_persistence_retry(&mut adapter, &keys, 0);
    adapter
        .retain_merge_sidecars_for_global_view(0, None, None)
        .expect("install initial unlocked directive");
    assert!(adapter.effects.iter().any(
            |effect| matches!(effect, V2LaneWorkEffect::BroadcastMerge(signature) if signature.view == 0)
        ));
    let locked = wire::BlockSubject {
        parent_block_hash: Some(candidate.carrier_parent_hash),
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"locked later-view carrier")),
        payload_hash: Hash::new(b"locked later-view payload"),
    };
    adapter
        .retain_merge_sidecars_for_global_view(1, Some(locked), None)
        .expect("install locked later-view directive");
    assert!(adapter.merge_entries.is_empty());
    assert!(adapter.merge_claims.is_empty());
    assert!(
        adapter
            .effects
            .iter()
            .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_)))
    );
    adapter
        .schedule_retransmission()
        .expect("schedule locked-view retransmission");
    assert!(
        adapter
            .drain_effects(usize::MAX)
            .iter()
            .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_)))
    );
}
fn record_production_merge_candidate_for_persistence_retry(
    adapter: &mut V2LaneWorkAdapter,
    keys: &[KeyPair],
    view: wire::View,
) -> crate::merge::MergeLedgerCandidate {
    let lane_id = LaneId::SINGLE;
    let dataspace_id = DataSpaceId::UNIVERSAL;
    let lane_height = 1;
    let global_height = adapter.context.height;
    let finalized_height = global_height
        .checked_sub(1)
        .expect("persistence-retry fixture has a finalized parent height");
    let finalized_height_index = usize::try_from(finalized_height)
        .ok()
        .and_then(NonZeroUsize::new)
        .expect("finalized parent height fits Kura indexing");
    let block = adapter
        .kura
        .get_block(finalized_height_index)
        .expect("persistence-retry fixture retains its finalized parent block");
    let mut finalized_context = adapter.context.clone();
    finalized_context.height = finalized_height;
    let predecessor_height = finalized_height
        .checked_sub(1)
        .expect("fixture finalized parent has a predecessor");
    let predecessor = adapter
        .kura
        .get_block(
            usize::try_from(predecessor_height)
                .ok()
                .and_then(NonZeroUsize::new)
                .expect("finalized predecessor height fits Kura indexing"),
        )
        .expect("persistence-retry fixture retains the finalized predecessor");
    let predecessor_wire = predecessor
        .encode_wire()
        .expect("encode persistence-retry finalized predecessor");
    let predecessor_qc = finalized_context
        .parent_commit_qc
        .as_mut()
        .expect("non-genesis finalized context has a parent certificate");
    predecessor_qc.round.height = predecessor_height;
    predecessor_qc.proposal_round = predecessor_qc.round;
    predecessor_qc.subject = wire::BlockSubject {
        parent_block_hash: predecessor.header().prev_block_hash(),
        block_hash: predecessor.hash(),
        payload_hash: predecessor
            .canonical_proposal_wire_hash()
            .expect("hash persistence-retry finalized predecessor"),
    };
    predecessor_qc.execution_commitment =
        wire::ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
            Hash::new(b"persistence-retry predecessor parent state"),
            Hash::new(b"persistence-retry predecessor post state"),
            Hash::new(b"persistence-retry predecessor writes"),
            u64::try_from(predecessor_wire.len()).expect("predecessor wire length fits u64"),
            Hash::new(&predecessor_wire),
        );
    finalized_context
        .validate()
        .expect("production-shaped finalized relay context is valid");
    // Relay admission requires committee members to be present in both
    // the exact frozen commit topology and World. The v2 adapter fixture
    // seeds the key registry directly and commits synthetic parent blocks,
    // so complete that production authority tuple before constructing
    // authenticated relay evidence.
    {
        let mut topology = adapter.state.commit_topology.block();
        topology.clear();
        for entry in &adapter.context.roster {
            topology.push(entry.validator.clone());
        }
        topology.commit();
    }
    let mut world_block = adapter.state.world.block();
    {
        let mut peers = world_block.peers_mut_for_testing().transaction();
        for key in keys {
            let peer = PeerId::new(key.public_key().clone());
            if !peers.iter().any(|existing| existing == &peer) {
                peers.push(peer);
            }
        }
        peers.apply();
    }
    world_block.commit();
    let (beacon_key, beacon_pulse) = crate::beacon::signed_persisted_pulse_fixture_for_world(
        adapter.context.network_id,
        global_height - 1,
    );
    let beacon_link = crate::beacon::GlobalThresholdBeaconPulseLinkV1 {
        pulse_id: beacon_pulse.pulse_id,
        seed: beacon_pulse.seed,
        height: beacon_pulse.height,
        round: beacon_pulse.round,
    };
    {
        let mut world = adapter.state.world.block();
        world
            .global_beacon_key_sessions
            .insert(beacon_pulse.session_id, beacon_key);
        world.global_beacon_active_session.insert(
            crate::state::GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY,
            beacon_pulse.session_id,
        );
        world
            .global_beacon_pulses
            .insert(beacon_pulse.pulse_id, beacon_pulse);
        world.global_beacon_pulse_slots.insert(
            (
                iroha_data_model::governance::types::BeaconSessionId::for_network_v1(
                    &beacon_pulse.network_id,
                ),
                beacon_pulse.height,
            ),
            beacon_pulse.pulse_id,
        );
        world.global_beacon_latest_pulse.insert(
            crate::state::GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY,
            beacon_link,
        );
        world.commit();
    }
    // Resolve the production relay committee from the persisted beacon pulse.
    // The fixture has the exact 3f+1 topology, so every live validator is
    // selected in the frozen authority geometry used by merge admission.
    let committee = adapter
        .state
        .resolve_lane_committee_at_height(
            crate::state::LaneAuthorityRoute::new(lane_id, dataspace_id),
            global_height,
        )
        .expect("verified beacon pulse resolves the production relay committee")
        .into_validators();
    assert_eq!(
        committee.len(),
        keys.len(),
        "fixture must provide exact 3f+1 relay committee"
    );
    let parent_state_root = Hash::new(b"v2 merge retry parent state");
    let post_state_root = Hash::new(b"v2 merge retry post state");
    let settlement = LaneBlockCommitment {
        block_height: lane_height,
        lane_id,
        lane_incarnation: adapter
            .state
            .lane_incarnation_at_height(lane_id, lane_height)
            .expect("fixture lane incarnation is active"),
        dataspace_id,
        tx_count: 0,
        total_local_amount: "0".parse().expect("valid settlement quantity"),
        total_xor_due: "0".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0".parse().expect("valid settlement quantity"),
        total_xor_variance: "0".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let mut envelope = LaneRelayEnvelope::new(block.header(), None, settlement, 0)
        .expect("construct production-valid relay envelope")
        .with_lane_block_descriptor_hash(Some(Hash::new(
            b"v2 merge persistence retry lane descriptor",
        )))
        .with_manifest_root(Some([0x44; 32]))
        .with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
            proof_digest: Hash::new(b"v2 merge persistence retry FastPQ proof"),
            verified_at_height: finalized_height,
        }));
    let statement = envelope
        .lane_finality_statement()
        .expect("complete production relay finality statement");
    let statement_tree: iroha_crypto::MerkleTree<iroha_data_model::nexus::LaneFinalityStatement> =
        core::iter::once(HashOf::new(&statement)).collect();
    let statement_commitment = statement_tree
        .commitment()
        .expect("one-leaf relay finality commitment");
    let statement_proof = statement_tree
        .get_proof(0)
        .expect("one-leaf relay finality proof");
    let executed_block_wire = block
        .encode_wire()
        .expect("encode persistence-retry carrier");
    let mut execution_commitment = wire::ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
        parent_state_root,
        post_state_root,
        Hash::new(b"v2 merge retry ordinary writes"),
        u64::try_from(executed_block_wire.len()).expect("carrier wire length fits u64"),
        Hash::new(&executed_block_wire),
    );
    execution_commitment.lane_finality_manifest = Some(statement_commitment);
    execution_commitment
        .validate()
        .expect("valid persistence-retry execution commitment");
    let finality = signed_finality_artifact(
        &finalized_context,
        keys,
        &block,
        execution_commitment,
        (0..crate::sumeragi::network_topology::commit_quorum_from_len(keys.len()).max(1))
            .map(|index| u32::try_from(index).expect("fixture signer index fits u32"))
            .collect(),
        [
            "encode persistence-retry finalized block",
            "derive persistence-retry finality signer preimage",
            "persistence-retry signer index",
            "sign persistence-retry finality vote",
            "aggregate persistence-retry finality votes",
            "derive persistence-retry finality signer PoP",
            "cryptographically valid persistence-retry finality artifact",
        ],
    );
    let _commit_receipt = adapter
        .kura
        .store_v2_finality_artifact(&finality)
        .expect("persist persistence-retry finality");
    envelope.finality_authority = Some(iroha_data_model::nexus::LaneFinalityAuthorityV1 {
        version: 1,
        global_block_height: finality.height,
        finality_artifact_hash: HashOf::new(&finality),
        statement_proof,
    });
    let (envelope, proof_blob) = crate::state::prove_finalized_lane_relay_for_registration(
        envelope,
        parent_state_root.into(),
        post_state_root.into(),
    );
    let registration =
        InstructionBox::from(iroha_data_model::isi::nexus::RegisterVerifiedLaneRelay {
            envelope: envelope.clone(),
            proof_blob,
            effect_proof_blob: None,
        });
    let registration_authority = AccountId::new(keys[0].public_key().clone());
    let mut registration_block = adapter.state.block(block.header());
    {
        let mut registration_transaction = registration_block.transaction();
        crate::smartcontracts::isi::execute_borrowed_instruction(
            &registration,
            &registration_authority,
            &mut registration_transaction,
        )
        .expect("production instruction verifies and stages the persistence-retry relay");
        // Commit only the production-generated World writes. Applying the whole
        // transaction would stage a second copy as current-block metadata, whose
        // normal block commit would advance the already-finalized State frontier.
        registration_transaction.world.apply();
    }
    registration_block
        .commit_world_overlay_for_testing()
        .expect("commit production-verified relay state without advancing the frontier");
    adapter.context.nexus_amx_context_hash =
        super::super::v2_recovery::committed_nexus_amx_context_hash(adapter.state.as_ref());
    adapter.context.execution_policy_hash =
        super::super::v2_recovery::committed_execution_policy_hash(adapter.state.as_ref())
            .expect("derive persistence-retry execution policy");
    adapter
        .state
        .record_lane_relay(&envelope)
        .expect("production relay admission accepts retry fixture");
    let candidates = adapter
        .state
        .merge_entry_candidates_from_lane_relays_for_view(view);
    assert_eq!(
        candidates.len(),
        1,
        "one admitted relay yields one candidate"
    );
    candidates
        .into_iter()
        .next()
        .expect("relay merge candidate")
}
#[test]
fn merge_signing_rejects_wrong_round_context_and_post_apply_state() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let candidate = record_production_merge_candidate_for_persistence_retry(&mut adapter, &keys, 0);
    let signer = adapter
        .local_validator_index()
        .expect("fixture local validator is in the frozen roster");
    adapter
        .retain_merge_sidecars_for_global_view(0, None, None)
        .expect("install exact unlocked directive");
    adapter.drain_effects(usize::MAX);
    let mut wrong_view = candidate.clone();
    wrong_view.view = wrong_view.view.saturating_add(1);
    let mut wrong_height = candidate.clone();
    wrong_height.carrier_height = wrong_height.carrier_height.saturating_add(1);
    let mut wrong_parent = candidate.clone();
    wrong_parent.carrier_parent_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"wrong merge signing parent"));
    for (label, drifted) in [
        ("view", wrong_view),
        ("height", wrong_height),
        ("parent", wrong_parent),
    ] {
        let digest = crate::merge::merge_qc_message_digest(
            &adapter.context.network_id,
            &drifted,
            VALIDATOR_SET_HASH_VERSION_V1,
            adapter.frozen_validator_set_hash(),
        );
        assert_eq!(
            adapter.authorize_local_merge_claim(&drifted, 0, signer, digest),
            Err(MergeSidecarError::LocalSigningEquivocation),
            "wrong {label} must fail before private-key use"
        );
    }
    assert!(
        adapter
            .effects
            .iter()
            .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_)))
    );
    let applied = test_block(
        adapter.context.height,
        Some(candidate.carrier_parent_hash),
        None,
        &keys[0],
    );
    adapter
        .kura
        .store_block(applied.clone())
        .expect("persist exact post-apply carrier");
    let committed = ValidBlock::committed_from_replay_signed_block(applied);
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
    let digest = crate::merge::merge_qc_message_digest(
        &adapter.context.network_id,
        &candidate,
        VALIDATOR_SET_HASH_VERSION_V1,
        adapter.frozen_validator_set_hash(),
    );
    assert_eq!(
        adapter.authorize_local_merge_claim(&candidate, 0, signer, digest),
        Err(MergeSidecarError::LocalSigningEquivocation),
        "post-apply recovery must never authorize another share"
    );
    adapter
        .refresh_merge_candidates(0)
        .expect("post-apply refresh remains signing-silent");
    adapter
        .schedule_retransmission()
        .expect("schedule post-apply retransmission");
    assert!(
        adapter
            .drain_effects(usize::MAX)
            .iter()
            .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_)))
    );
}
#[test]
fn merge_signing_rejects_block_first_kura_ahead_crash_image() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let candidate = record_production_merge_candidate_for_persistence_retry(&mut adapter, &keys, 0);
    let signer = adapter
        .local_validator_index()
        .expect("fixture local validator is in the frozen roster");
    let digest = crate::merge::merge_qc_message_digest(
        &adapter.context.network_id,
        &candidate,
        VALIDATOR_SET_HASH_VERSION_V1,
        adapter.frozen_validator_set_hash(),
    );
    let durable_carrier = test_block(
        adapter.context.height,
        Some(candidate.carrier_parent_hash),
        None,
        &keys[0],
    );
    adapter
        .kura
        .store_block(durable_carrier)
        .expect("persist block-first carrier without advancing State");
    adapter
        .retain_merge_sidecars_for_global_view(candidate.view, None, None)
        .expect("install exact unlocked reducer directive");
    assert!(
        adapter
            .drain_effects(usize::MAX)
            .iter()
            .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_))),
        "a Kura-ahead crash image must not release a private-key operation"
    );
    assert!(matches!(
        adapter.authorize_local_merge_claim(&candidate, candidate.view, signer, digest),
        Err(MergeSidecarError::SigningGuard(message))
            if message.contains("identical committed State and durable Kura frontiers")
    ));
    assert_eq!(
        adapter
            .merge_signing_guard
            .as_ref()
            .expect("voting adapter has merge signing guard")
            .authorized_digest(&merge_signing_context_for_test(&adapter, &candidate))
            .expect("read durable signing guard"),
        None
    );
}
#[test]
fn same_round_merge_claims_survive_successful_kura_staging() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let local_index = adapter
        .local_validator_index()
        .expect("fixture local validator is in the frozen roster");
    let local_leader_view = (0..u64::try_from(adapter.context.roster.len())
        .expect("fixture roster length fits u64"))
        .find(|view| adapter.context.leader(*view) == local_index)
        .expect("rotating leader schedule reaches the local validator");
    let candidate = record_production_merge_candidate_for_persistence_retry(
        &mut adapter,
        &keys,
        local_leader_view,
    );
    let digest = crate::merge::merge_qc_message_digest(
        &adapter.context.network_id,
        &candidate,
        VALIDATOR_SET_HASH_VERSION_V1,
        adapter.frozen_validator_set_hash(),
    );
    let key = MergeKey {
        epoch_id: candidate.epoch_id,
        view: candidate.view,
        digest,
    };
    adapter
        .retain_merge_sidecars_for_global_view(candidate.view, None, None)
        .expect("install exact unlocked reducer directive");
    let initial_local_share = adapter
        .drain_effects(usize::MAX)
        .into_iter()
        .find_map(|effect| match effect {
            V2LaneWorkEffect::BroadcastMerge(share) if share.signer == local_index => Some(share),
            _ => None,
        })
        .expect("local leader must publish its exact candidate share");
    assert_eq!(
        initial_local_share.leader_candidate_body.as_deref(),
        Some(candidate.canonical_bytes().as_slice()),
        "the leader retransmission identity includes its canonical candidate body"
    );
    assert_eq!(
        adapter
            .merge_claims
            .get(&(candidate.epoch_id, candidate.view, local_index)),
        Some(&digest),
        "local claim must be recorded before its signature is produced"
    );
    let mut accepted_remote_signers = Vec::new();
    let mut accepted_remote_share = None;
    for (index, key_pair) in keys.iter().enumerate() {
        let signer = u32::try_from(index).expect("fixture signer index fits u32");
        if signer == local_index {
            continue;
        }
        let signature = Signature::try_new(key_pair.private_key(), digest.as_ref())
            .expect("sign remote merge share")
            .payload()
            .to_vec();
        let share = MergeCommitteeSignature {
            version: MERGE_COMMITTEE_SIGNATURE_VERSION_V2,
            epoch_id: candidate.epoch_id,
            view: candidate.view,
            signer,
            message_digest: digest,
            bls_sig: signature,
            leader_candidate_body: (signer == adapter.context.leader(candidate.view))
                .then(|| candidate.canonical_bytes()),
        };
        assert_eq!(
            adapter
                .accept_merge_signature(share.clone(), candidate.view)
                .expect("persist remote merge signature"),
            V2LaneIngressOutcome::Inserted
        );
        accepted_remote_share = Some(share);
        accepted_remote_signers.push(signer);
        if matches!(
            adapter
                .merge_entries
                .get(&key)
                .map(|pending| &pending.stage),
            Some(PendingMergeStage::Persisted(_))
        ) {
            break;
        }
    }
    assert!(matches!(
        adapter
            .merge_entries
            .get(&key)
            .map(|pending| &pending.stage),
        Some(PendingMergeStage::Persisted(_))
    ));
    for signer in std::iter::once(local_index).chain(accepted_remote_signers) {
        assert_eq!(
            adapter
                .merge_claims
                .get(&(candidate.epoch_id, candidate.view, signer)),
            Some(&digest),
            "Kura staging must not reopen any same-round signer decision"
        );
    }
    let (_, staged) = adapter
        .kura
        .select_pending_certified_merge_entry()
        .expect("read pending certified merge entry")
        .expect("quorum must stage one exact merge entry");
    assert_eq!(staged.merge_qc.message_digest, digest);
    let validation_checks_after_persistence = adapter.merge_candidate_validation_checks.get();
    adapter
        .schedule_retransmission()
        .expect("retransmit the already-persisted merge quorum");
    let retransmitted_local_share = adapter
        .drain_effects(usize::MAX)
        .into_iter()
        .find_map(|effect| match effect {
            V2LaneWorkEffect::BroadcastMerge(share) if share.signer == local_index => Some(share),
            _ => None,
        })
        .expect("persisted quorum must retransmit the local leader share");
    assert_eq!(retransmitted_local_share, initial_local_share);
    assert_eq!(
        adapter.merge_candidate_validation_checks.get(),
        validation_checks_after_persistence,
        "retransmission must not reexecute an already-persisted candidate"
    );
    assert_eq!(
        adapter
            .accept_merge_signature(initial_local_share, candidate.view)
            .expect("classify an exact persisted leader replay"),
        V2LaneIngressOutcome::Duplicate
    );
    assert_eq!(
        adapter.merge_candidate_validation_checks.get(),
        validation_checks_after_persistence,
        "persisted leader replay must not reexecute the certified candidate"
    );
    assert_eq!(
        adapter
            .accept_merge_signature(
                accepted_remote_share.expect("quorum includes one remote share"),
                candidate.view,
            )
            .expect("classify an exact post-persistence replay"),
        V2LaneIngressOutcome::Duplicate
    );
    assert_eq!(
        adapter.merge_candidate_validation_checks.get(),
        validation_checks_after_persistence,
        "post-persistence replay must not reexecute the certified candidate"
    );
}
#[test]
fn quorate_merge_persistence_failure_latches_restart_required() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let candidate = record_production_merge_candidate_for_persistence_retry(&mut adapter, &keys, 0);
    adapter
        .retain_merge_sidecars_for_global_view(candidate.view, None, None)
        .expect("install exact unlocked reducer directive");
    let digest = crate::merge::merge_qc_message_digest(
        &adapter.context.network_id,
        &candidate,
        VALIDATOR_SET_HASH_VERSION_V1,
        adapter.frozen_validator_set_hash(),
    );
    let key = MergeKey {
        epoch_id: candidate.epoch_id,
        view: candidate.view,
        digest,
    };
    let signatures = keys
        .iter()
        .enumerate()
        .map(|(index, key_pair)| {
            (
                u32::try_from(index).expect("fixture signer index fits u32"),
                Signature::try_new(key_pair.private_key(), digest.as_ref())
                    .expect("sign retry candidate")
                    .payload()
                    .to_vec(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let expected_signers = exact_merge_certificate_signers(
        &signatures,
        adapter.context.quorum.min_signers as usize,
        adapter.context.leader(candidate.view),
    )
    .expect("complete fixture shares include the round leader");
    adapter.merge_entries.insert(
        key,
        PendingMerge {
            stage: PendingMergeStage::Collecting(candidate.clone()),
            signatures,
        },
    );
    let pending_dir = adapter.kura.store_root().join("pending_merge_entries");
    if pending_dir.is_dir() {
        std::fs::remove_dir(&pending_dir)
            .expect("remove empty pending sidecar directory before obstruction");
    }
    std::fs::write(&pending_dir, b"temporarily block pending sidecar directory")
        .expect("install transient Kura obstruction");
    let output_guard = Arc::clone(&adapter.output_guard);
    let publication = {
        let operation = output_guard
            .begin_fail_stop_operation()
            .expect("fixture output admission remains open");
        match adapter.try_commit_merge(key) {
            Ok(()) => {
                operation.complete();
                Ok(())
            }
            Err(error) => Err(error),
        }
    };
    assert!(matches!(publication, Err(V2LaneWorkError::Persistence(_))));
    assert!(
        adapter.merge_entries.contains_key(&key),
        "failed Kura publication must retain the complete quorum"
    );
    let certified_entry = match &adapter.merge_entries[&key].stage {
        PendingMergeStage::Certified(entry) => entry.clone(),
        PendingMergeStage::Collecting(_) => {
            panic!("production quorum must advance to Certified before Kura publication")
        }
        PendingMergeStage::Persisted(_) => {
            panic!("injected Kura failure must not reach the Persisted stage")
        }
    };
    assert_eq!(certified_entry.merge_qc.message_digest, key.digest);
    let certified_signers = certified_entry
        .merge_qc
        .signer_proofs
        .iter()
        .map(|proof| proof.signer)
        .collect::<Vec<_>>();
    assert_eq!(certified_signers, expected_signers);
    assert!(
        certified_signers.contains(&adapter.context.leader(candidate.view)),
        "the exact-cardinality QC must retain its durable round-leader custodian"
    );
    let expected_bitmap = certified_signers
        .iter()
        .fold(0_u8, |bitmap, signer| bitmap | (1_u8 << *signer));
    assert_eq!(
        certified_entry.merge_qc.signers_bitmap,
        vec![expected_bitmap]
    );
    assert_eq!(certified_entry.epoch_id, candidate.epoch_id);
    assert_eq!(certified_entry.lane_snapshots, candidate.lane_snapshots);
    assert_eq!(certified_entry.active_lanes, candidate.active_lanes);
    let certified_hash = crate::merge::merge_ledger_entry_hash(&certified_entry);
    std::fs::remove_file(&pending_dir).expect("remove transient Kura obstruction");
    assert!(
        adapter.output_guard.restart_required(),
        "failed durable publication must poison this process before it can sign again"
    );
    assert!(matches!(
        adapter.schedule_retransmission(),
        Err(V2LaneWorkError::RestartRequired)
    ));
    assert_eq!(
        adapter
            .kura
            .merge_entry_by_hash(certified_hash)
            .expect("read exact unpublished merge entry"),
        None,
        "a poisoned process must not retry durable publication"
    );
}
#[test]
fn merge_signature_state_is_bound_to_the_active_global_view() {
    let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
    let stale_digest = Hash::new(b"stale merge claim");
    adapter.merge_claims.insert((7, 0, 0), stale_digest);
    adapter
        .retain_merge_sidecars_for_global_view(1, None, None)
        .expect("install next unlocked reducer view");
    assert!(
        adapter.merge_claims.is_empty(),
        "advancing the reducer view must retire old-view signing claims"
    );
    let stale = MergeCommitteeSignature {
        version: MERGE_COMMITTEE_SIGNATURE_VERSION_V2,
        epoch_id: 7,
        view: 0,
        signer: 0,
        message_digest: stale_digest,
        bls_sig: vec![0xA5; 96],
        leader_candidate_body: None,
    };
    assert_eq!(
        adapter
            .accept_merge_signature(stale, 1)
            .expect("reject stale remote signature without local durability work"),
        V2LaneIngressOutcome::Rejected
    );
    assert!(adapter.merge_claims.is_empty());
    assert!(adapter.merge_entries.is_empty());
}
