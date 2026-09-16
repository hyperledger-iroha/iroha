// Initial-author loss counterexample for the autonomous producer boundary.
//
// These tests use the actual State committee, durable QueuePlan admission and
// production producer/NewView methods. The negative test deliberately records
// the current failure: it is not evidence that lane liveness is implemented.
// Global views below are arguments at this adapter boundary, not fabricated
// timeout certificates; the real four-peer network regression owns that proof.
// TODO: Replace the recorded absence of progress with certified successor-view
// production once the shared lane reducer owns initial author replacement.

fn initial_author_loss_fixture(local_validator_index: usize) -> (V2LaneWorkAdapter, Vec<KeyPair>) {
    let (mut observer, keys) = fixture_at_height_inner_with_kura_and_local_index(
        wire::ConsensusMode::Permissioned,
        9,
        true,
        locked_lane_work_test_kura(iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY),
        Some(local_validator_index),
        false,
    );
    enable_multilane_nexus(&mut observer, &keys, LaneId::new(1), DataSpaceId::new(7));
    let context = observer.context.clone();
    let restart = LaneAdapterRestartParts::capture(&observer);
    drop(observer);
    let adapter = restart
        .reopen_isolated(context, true)
        .expect("open each survivor's voting journals under its final lane context");
    (adapter, keys)
}

#[test]
fn autonomous_initial_author_loss_counterexample_retains_work_without_preproposal_progress() {
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    // The same deterministic four-key committee is reconstructed in each
    // isolated replica fixture. Its index-zero author never runs a producer.
    for local_validator_index in 1..4 {
        let (mut adapter, keys) = initial_author_loss_fixture(local_validator_index);
        assert_autonomous_test_role(&adapter, &keys, lane_id, dataspace_id, false);
        let slot = plan_autonomous_lane_reservation_slot(
            adapter.state.as_ref(),
            adapter.kura.as_ref(),
            &adapter.context,
            lane_id,
            dataspace_id,
        )
        .expect("plan the exact initial autonomous slot from State and Kura");
        assert_eq!(slot.validator_set.len(), 4);
        assert_eq!(
            crate::sumeragi::network_topology::commit_quorum_from_len(slot.validator_set.len()),
            3
        );
        assert_eq!(slot.author, slot.validator_set[0]);
        assert_eq!(
            adapter.local_peer,
            slot.validator_set[local_validator_index]
        );
        assert_eq!(slot.lane_block_height, 1);
        assert_eq!(slot.lane_block_view, 0);
        let journal_dir = tempfile::tempdir().expect("survivor queue journal directory");
        let queue = install_autonomous_test_queue(
            &mut adapter,
            lane_id,
            dataspace_id,
            &journal_dir.path().join("lane-reservations.norito"),
        );
        let admitted =
            enqueue_autonomous_test_transactions(&adapter, &queue, lane_id, dataspace_id, 1);
        let original_fifo = queue.fifo_snapshot_for_test();
        assert_eq!(admitted.len(), 1);
        assert_eq!(original_fifo.len(), 1);
        assert!(queue.lane_has_pending_work(lane_id, dataspace_id, slot.lane_incarnation));

        for active_view in [0, 1, 3, 7] {
            // Expire only the wall-clock throttle; do not manufacture any
            // reservation, payload, vote, certificate or lifecycle ownership.
            adapter.next_autonomous_producer_tick = Instant::now();
            adapter
                .schedule_autonomous_lane_production(
                    active_view,
                    autonomous_test_candidate_limits(2, 2),
                )
                .expect("run the real survivor producer with admitted work");
            adapter
                .schedule_autonomous_new_view_timeouts(
                    Instant::now() + Duration::from_secs(60),
                    active_view,
                    Duration::from_millis(1),
                )
                .expect("service all existing lane NewView clocks");

            assert_eq!(queue.fifo_snapshot_for_test(), original_fifo);
            assert_eq!(queue.active_len(), 1);
            assert!(queue.live_lane_reservations().is_empty());
            assert!(adapter.pending_autonomous_reservation_batches.is_empty());
            assert!(adapter.pending_autonomous_anchor_payloads.is_empty());
            assert!(adapter.autonomous_payloads.is_empty());
            assert!(adapter.autonomous_new_view_started_at.is_empty());
            assert!(
                adapter
                    .autonomous_new_view_votes
                    .votes_for_signer(&adapter.local_peer)
                    .is_empty()
            );
            assert!(
                adapter
                    .drain_effects(usize::MAX)
                    .into_iter()
                    .all(|effect| !matches!(
                        effect,
                        V2LaneWorkEffect::PostLaneBlock {
                            message: BlockMessage::LaneBlockNewViewVote(_)
                                | BlockMessage::LaneBlockNewViewCertificate(_),
                            ..
                        }
                    ))
            );
            assert!(
                adapter
                    .kura
                    .read_autonomous_lane_block_artifact(
                        lane_id,
                        slot.lane_block_height,
                        adapter.native_network_id(),
                        adapter.context.epoch,
                    )
                    .is_none()
            );
            assert!(!adapter.output_guard.restart_required());
        }
    }
}

#[test]
fn autonomous_initial_author_loss_control_live_author_takes_real_queue_ownership() {
    let (mut adapter, keys) = initial_author_loss_fixture(0);
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    assert_autonomous_test_role(&adapter, &keys, lane_id, dataspace_id, true);
    let journal_dir = tempfile::tempdir().expect("author queue journal directory");
    let queue = install_autonomous_test_queue(
        &mut adapter,
        lane_id,
        dataspace_id,
        &journal_dir.path().join("lane-reservations.norito"),
    );
    let admitted = enqueue_autonomous_test_transactions(&adapter, &queue, lane_id, dataspace_id, 1);
    adapter
        .schedule_autonomous_lane_production(0, autonomous_test_candidate_limits(2, 2))
        .expect("the live elected author must use the same production selection path");
    assert_eq!(queue.queued_len(), 0);
    assert_eq!(queue.live_lane_reservations().len(), 1);
    let payload = adapter
        .pending_autonomous_anchor_payloads
        .values()
        .next()
        .expect("the live author durably publishes its selected executable payload");
    assert_eq!(payload.entrypoints, admitted);
    assert_eq!(payload.producer, adapter.local_peer);
    assert!(!adapter.output_guard.restart_required());
}
