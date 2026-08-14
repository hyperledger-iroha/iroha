// Lane-reservation restart, reconciliation, and fee-capacity regression tests.
//
// Included by `queue::tests` so source-bound libtest names remain stable.
fn live_snapshot_phase_fixture() -> (
    LaneQueueReservationReconciliationSnapshotV1,
    Vec<LaneQueueReservationKeyV2>,
    LaneQueueReservationGroupBindingV1,
) {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Queue::test(config_factory(), &time_source);
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    for _ in 0..3 {
        push_globally_bound_lane_reservation_candidate(
            &queue,
            &state,
            &dir,
            accepted_tx_by_someone(&time_source),
        );
    }
    let keys = queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(&state, b"snapshot-phase-owner", b"snapshot-phase-proposal"),
            nonzero!(3_usize),
        )
        .expect("reserve exact snapshot phase group")
        .iter()
        .map(|reservation| *reservation.key())
        .collect::<Vec<_>>();
    let group = lane_queue_reservation_group_binding_from_ordered_keys(keys.iter())
        .expect("bind exact snapshot phase group");
    let snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture exact live V4/V6 phase snapshot");
    (snapshot, keys, group)
}
struct ReplayedSnapshotRecoveryFixture {
    queue: Queue,
    snapshot: LaneQueueReservationReconciliationSnapshotV1,
    groups: Vec<(
        LaneQueueReservationGroupBindingV1,
        Vec<LaneQueueReservationKeyV2>,
    )>,
    _journal_dir: TempDir,
}
fn replayed_snapshot_recovery_fixture(
    group_sizes: &[usize],
    release_group: Option<(usize, AutonomousLaneRetirementQueueSnapshotPhaseV1)>,
) -> ReplayedSnapshotRecoveryFixture {
    assert!(group_sizes.iter().all(|size| *size != 0));
    assert!(release_group.is_none_or(|(index, _)| index < group_sizes.len()));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let journal_dir = tempdir().expect("snapshot-recovery journal directory");
    let plan_path = journal_dir
        .path()
        .join("queue-plans-for-reservations.norito");
    let (reservation_path, groups) = {
        let writer = Queue::test(config_factory(), &time_source);
        let reservation_path =
            install_globally_certified_test_reservation_journals(&writer, &journal_dir);
        let mut groups = Vec::with_capacity(group_sizes.len());
        for (index, size) in group_sizes.iter().copied().enumerate() {
            for _ in 0..size {
                push_globally_bound_lane_reservation_candidate(
                    &writer,
                    &state,
                    &journal_dir,
                    accepted_tx_by_someone(&time_source),
                );
            }
            let owner = format!("snapshot-planner-owner-{index}");
            let proposal = format!("snapshot-planner-proposal-{index}");
            let keys = writer
                .reserve_transactions_for_lane(
                    &state,
                    lane_reservation_scope(&state, owner.as_bytes(), proposal.as_bytes()),
                    NonZeroUsize::new(size).expect("fixture group is non-empty"),
                )
                .expect("reserve exact snapshot planner group")
                .iter()
                .map(|reservation| *reservation.key())
                .collect::<Vec<_>>();
            let group = lane_queue_reservation_group_binding_from_ordered_keys(keys.iter())
                .expect("bind exact snapshot planner group");
            groups.push((group, keys));
        }
        if let Some((index, phase)) = release_group {
            let barrier = lane_reservation_release_barrier(
                groups[index].1.clone(),
                b"snapshot-planner-prepared-release",
            );
            writer
                .prepare_lane_reservation_release_barrier(&barrier)
                .expect("persist prepared snapshot planner release");
            if phase == AutonomousLaneRetirementQueueSnapshotPhaseV1::Completed {
                let reservations = writer.lane_reservations.lock();
                let ordered_records = barrier
                    .ordered_keys
                    .iter()
                    .map(|key| reservations.live_by_hash[&key.signed_transaction_hash].clone())
                    .collect();
                drop(reservations);
                writer
                    .lane_reservation_journal
                    .lock()
                    .as_mut()
                    .expect("installed reservation journal")
                    .complete_release(LaneQueueReservationReleaseCompletionV5 {
                        version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION,
                        barrier,
                        ordered_records,
                    })
                    .expect("persist completed snapshot planner release");
            }
        }
        (reservation_path, groups)
    };
    let queue = Queue::test(config_factory(), &time_source);
    queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restore exact V6 snapshot planner owners");
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("restore exact V4 snapshot planner owners");
    let expected_replayed = group_sizes.iter().sum::<usize>();
    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("replay exact snapshot planner payloads")
            .replayed,
        expected_replayed
    );
    let snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture exact replayed snapshot planner owners");
    ReplayedSnapshotRecoveryFixture {
        queue,
        snapshot,
        groups,
        _journal_dir: journal_dir,
    }
}
include!("retired_release_snapshot_recovery_tests.rs");
#[test]
fn snapshot_recovery_phase_gate_accepts_only_exact_v4_v6_prefixes_and_order() {
    let (live_snapshot, keys, group) = live_snapshot_phase_fixture();
    let cleanup_gate = LaneQueueCarrierCleanupGate::direct_test(group);
    let live_state = cleanup_gate.cleanup_state(
        IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
        IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
        0,
        0,
        0,
    );
    let live_phases = lane_reservation_recovery_phase_map(&live_snapshot)
        .expect("partition every exact live owner");
    assert_eq!(
        lane_reservation_snapshot_group_phase_agrees(
            &live_snapshot,
            &live_phases,
            group,
            &keys,
            live_state,
        )
        .expect("the exact live signed state must match both Queue journals")
        .len(),
        keys.len()
    );
    let mut nonmatching_state = live_state;
    nonmatching_state.queue.selected_count =
        nonmatching_state.queue.selected_count.saturating_add(1);
    assert_eq!(
        select_unique_lane_reservation_snapshot_recovered_state(
            &live_snapshot,
            &live_phases,
            group,
            &keys,
            live_state,
            Some(nonmatching_state),
        )
        .expect("only the complete before-state matches exact Queue evidence"),
        live_state
    );
    assert!(matches!(
        select_unique_lane_reservation_snapshot_recovered_state(
            &live_snapshot,
            &live_phases,
            group,
            &keys,
            nonmatching_state,
            None,
        ),
        Err(LaneQueueReservationError::LifecycleStateSelectionConflict { matching_states: 0 })
    ));
    assert!(matches!(
        select_unique_lane_reservation_snapshot_recovered_state(
            &live_snapshot,
            &live_phases,
            group,
            &keys,
            live_state,
            Some(live_state),
        ),
        Err(LaneQueueReservationError::LifecycleStateSelectionConflict { matching_states: 2 })
    ));
    let mut reordered_snapshot = live_snapshot.clone();
    reordered_snapshot.ordered_groups[0].ordered_keys.swap(0, 1);
    assert!(
        lane_reservation_snapshot_group_phase_agrees(
            &reordered_snapshot,
            &live_phases,
            group,
            &keys,
            live_state,
        )
        .is_err(),
        "equal membership must not substitute a different durable FIFO order"
    );
    let mut missing_phase = live_snapshot.clone();
    missing_phase.ordered_owner_phases.pop();
    assert!(
        lane_reservation_recovery_phase_map(&missing_phase).is_err(),
        "cross-journal phase coverage must partition every durable owner"
    );
    let mut partial_commit = live_snapshot.clone();
    partial_commit
        .ordered_records
        .retain(|record| record.key != keys[0]);
    partial_commit.ordered_groups[0].ordered_keys.remove(0);
    partial_commit.commit_barriers.push(keys[0]);
    partial_commit
        .ordered_owner_phases
        .iter_mut()
        .find(|phase| phase.key == keys[0])
        .expect("first phase exists")
        .reservation_phase = LaneQueueReservationOwnerPhaseV6::CommitBarrier;
    let partial_phases = lane_reservation_recovery_phase_map(&partial_commit)
        .expect("partition the exact partial Commit cut");
    let partial_state = cleanup_gate.cleanup_state(
        IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
        IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
        1,
        0,
        0,
    );
    assert!(
        lane_reservation_snapshot_group_phase_agrees(
            &partial_commit,
            &partial_phases,
            group,
            &keys,
            partial_state,
        )
        .is_ok(),
        "one exact committed prefix may coexist with its live FIFO suffix"
    );
    let mut committed = partial_commit;
    committed.ordered_records.clear();
    committed.ordered_groups.clear();
    committed.commit_barriers = keys.clone();
    for phase in &mut committed.ordered_owner_phases {
        phase.reservation_phase = LaneQueueReservationOwnerPhaseV6::CommitBarrier;
    }
    let committed_state = cleanup_gate.cleanup_state(
        IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
        IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMITTED,
        group.reservation_count,
        0,
        0,
    );
    // V4 can be one durable append ahead of its V6 marker. Until the marker is durable, the
    // composed PersistPlanTombstone action has not linearized and the signed prefix stays at zero.
    committed
        .ordered_owner_phases
        .iter_mut()
        .find(|phase| phase.key == keys[0])
        .expect("first committed phase exists")
        .queue_plan_phase = QueuePlanReservationPhaseV1::Tombstoned;
    let v4_ahead_phases = lane_reservation_recovery_phase_map(&committed)
        .expect("partition the exact V4-before-V6 crash window");
    assert!(
        lane_reservation_snapshot_group_phase_agrees(
            &committed,
            &v4_ahead_phases,
            group,
            &keys,
            committed_state,
        )
        .is_ok(),
        "the one bounded V4-before-V6 crash window must remain recoverable"
    );
    committed
        .ordered_owner_phases
        .iter_mut()
        .find(|phase| phase.key == keys[0])
        .expect("first tombstoned phase exists")
        .plan_tombstone_marked = true;
    let marked_phases = lane_reservation_recovery_phase_map(&committed)
        .expect("partition the marked first tombstone");
    assert!(
        lane_reservation_snapshot_group_phase_agrees(
            &committed,
            &marked_phases,
            group,
            &keys,
            committed_state,
        )
        .is_err(),
        "a durable V6 marker cannot be hidden behind a stale signed prefix"
    );
    let first_tombstone_state = cleanup_gate.cleanup_state(
        IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
        IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMITTED,
        group.reservation_count,
        1,
        0,
    );
    assert!(
        lane_reservation_snapshot_group_phase_agrees(
            &committed,
            &marked_phases,
            group,
            &keys,
            first_tombstone_state,
        )
        .is_ok(),
        "the signed tombstone prefix must advance exactly with its V6 marker"
    );
    committed
        .ordered_owner_phases
        .iter_mut()
        .find(|phase| phase.key == keys[0])
        .expect("first marked phase exists")
        .plan_tombstone_marked = false;
    let unmarked_phases = lane_reservation_recovery_phase_map(&committed)
        .expect("partition the unmarked first tombstone");
    assert!(
        lane_reservation_snapshot_group_phase_agrees(
            &committed,
            &unmarked_phases,
            group,
            &keys,
            first_tombstone_state,
        )
        .is_err(),
        "a signed completed prefix requires the exact durable V6 marker"
    );
}
#[test]
fn snapshot_recovery_authority_requires_complete_exact_group_coverage_and_is_a_stutter() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("queue-plans-for-reservations.norito");
    let (reservation_path, keys) = {
        let writer = Queue::test(config_factory(), &time_source);
        let reservation_path = install_globally_certified_test_reservation_journals(&writer, &dir);
        for _ in 0..2 {
            push_globally_bound_lane_reservation_candidate(
                &writer,
                &state,
                &dir,
                accepted_tx_by_someone(&time_source),
            );
        }
        let keys = writer
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(
                    &state,
                    b"snapshot-authority-owner",
                    b"snapshot-authority-proposal",
                ),
                nonzero!(2_usize),
            )
            .expect("reserve exact snapshot authority group")
            .iter()
            .map(|reservation| *reservation.key())
            .collect::<Vec<_>>();
        (reservation_path, keys)
    };
    let queue = Queue::test(config_factory(), &time_source);
    queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restore exact V6 reservation snapshot");
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("restore exact V4 QueuePlan journal");
    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("replay exact startup reservation payloads")
            .replayed,
        keys.len()
    );
    let snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture the combined V4/V6 startup snapshot");
    let group = lane_queue_reservation_group_binding_from_ordered_keys(keys.iter())
        .expect("bind exact startup reservation group");
    let recovered_state = LaneQueueCarrierCleanupGate::direct_test(group).cleanup_state(
        IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
        IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
        0,
        0,
        0,
    );
    let height_context_id =
        iroha_data_model::block::consensus_v2::HeightContextId(HashOf::<
            iroha_data_model::block::consensus_v2::HeightContext,
        >::from_untyped_unchecked(
            Hash::new(b"snapshot-authority-height-context"),
        ));
    let validator_set_hash = HashOf::<Vec<PeerId>>::from_untyped_unchecked(Hash::new(
        b"snapshot-authority-validator-set",
    ));
    let lifecycle_projection = |ordered_keys: Vec<LaneQueueReservationKeyV2>,
                                cursor_seed: &'static [u8]| {
        LaneReservationSnapshotLifecycleProjectionV1 {
            height_context_id,
            origin_proposal_hash: Hash::new(b"snapshot-authority-origin-proposal"),
            executable_payload_hash: Hash::new(b"snapshot-authority-executable-payload"),
            cursor_sequence: 1,
            cursor_hash: Hash::new(cursor_seed),
            cursor_phase: AutonomousLifecycleCursorPhaseKindV2::Live,
            owner_generation: 1,
            source_generation: None,
            validator_set_hash_version: 1,
            validator_set_hash,
            validator_count: 1,
            local_validator_index: 0,
            local_actor: 1,
            producer: 1,
            reservation_group: group,
            ordered_keys,
            cursor_before: recovered_state,
            cursor_after: None,
            recovered_state,
        }
    };
    let authorization = queue
        .authorize_lane_reservation_snapshot_recovery(
            checked_startup_reconciliation_receipt(&queue),
            vec![lifecycle_projection(
                keys.clone(),
                b"snapshot-authority-live-cursor",
            )],
            None,
        )
        .expect("authorize exact checked action-25 recovery stutter");
    assert_eq!(authorization.checked_groups.len(), 1);
    let accepted = authorization.checked_groups[0]
        .checked
        .accepted_projection();
    assert_eq!(
        accepted.action,
        IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT
    );
    assert_eq!(accepted.actor, 0);
    assert_eq!(accepted.target, 0);
    assert_eq!(accepted.before, recovered_state);
    assert_eq!(accepted.after, recovered_state);
    let recovered_receipt = authorization
        .into_reconciliation_receipt()
        .expect("consume only exact checked snapshot stutters");
    assert!(
        queue
            .revalidate_lane_reservation_startup_reconciliation_receipt(
                &recovered_receipt,
                &snapshot,
            )
            .expect("revalidate returned combined startup receipt")
    );
    let mut reordered_keys = keys.clone();
    reordered_keys.swap(0, 1);
    assert!(matches!(
        queue.authorize_lane_reservation_snapshot_recovery(
            checked_startup_reconciliation_receipt(&queue),
            vec![lifecycle_projection(
                reordered_keys,
                b"snapshot-authority-reordered-cursor",
            )],
            None,
        ),
        Err(LaneQueueReservationError::InvalidIdentity(_))
    ));
    assert!(matches!(
        queue.authorize_lane_reservation_snapshot_recovery(
            checked_startup_reconciliation_receipt(&queue),
            vec![
                lifecycle_projection(keys.clone(), b"snapshot-authority-overlap-a"),
                lifecycle_projection(keys, b"snapshot-authority-overlap-b"),
            ],
            None,
        ),
        Err(LaneQueueReservationError::InvalidIdentity(_))
    ));
}
#[test]
fn snapshot_recovery_accepts_mixed_lifecycle_and_strict_absence_coverage() {
    let fixture = replayed_snapshot_recovery_fixture(&[1, 1], None);
    let (lifecycle_group, lifecycle_keys) = &fixture.groups[0];
    let (strict_group, strict_keys) = &fixture.groups[1];
    let lifecycle_state = LaneQueueCarrierCleanupGate::direct_test(*lifecycle_group).cleanup_state(
        IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
        IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
        0,
        0,
        0,
    );
    let strict_state = strictly_absent_lane_reservation_snapshot_recovery_state(
        &fixture.snapshot,
        *strict_group,
        strict_keys,
    )
    .expect("derive the exact pre-Kura state from durable Queue admissions");
    let planner_evidence = LaneReservationSnapshotPlannerEvidence::from_parts_for_test(
        fixture.snapshot.clone(),
        vec![(
            *strict_group,
            strict_keys.clone(),
            LaneReservationSnapshotPlannerProjectionKind::StrictlyAbsent {
                recovered_state: strict_state,
            },
        )],
    );
    let lifecycle = snapshot_lifecycle_projection_fixture(
        *lifecycle_group,
        lifecycle_keys.clone(),
        lifecycle_state,
        b"snapshot-planner-mixed-lifecycle",
    );
    let authorization = fixture
        .queue
        .authorize_lane_reservation_snapshot_recovery(
            checked_startup_reconciliation_receipt(&fixture.queue),
            vec![lifecycle],
            Some(planner_evidence),
        )
        .expect("authorize the exact lifecycle and strict-absence owner partition");
    assert_eq!(authorization.checked_groups.len(), 1);
    assert_eq!(authorization.checked_planner_groups.len(), 1);
    let lifecycle_stutter = authorization.checked_groups[0]
        .checked
        .accepted_projection();
    assert_eq!(lifecycle_stutter.before, lifecycle_state);
    assert_eq!(lifecycle_stutter.after, lifecycle_state);
    let planner_stutter = authorization.checked_planner_groups[0]
        .checked
        .accepted_projection();
    assert_eq!(planner_stutter.before, strict_state);
    assert_eq!(planner_stutter.after, strict_state);
    let recovered_receipt = authorization
        .into_reconciliation_receipt()
        .expect("consume both exact action-25 stutters");
    assert!(
        fixture
            .queue
            .revalidate_lane_reservation_startup_reconciliation_receipt(
                &recovered_receipt,
                &fixture.snapshot,
            )
            .expect("revalidate the mixed-coverage startup receipt")
    );
}
#[test]
fn snapshot_recovery_rejects_planner_evidence_for_another_exact_snapshot() {
    let fixture = replayed_snapshot_recovery_fixture(&[1], None);
    let (group, keys) = &fixture.groups[0];
    let strict_state =
        strictly_absent_lane_reservation_snapshot_recovery_state(&fixture.snapshot, *group, keys)
            .expect("derive exact strict-absence state");
    let mut stale_snapshot = fixture.snapshot.clone();
    stale_snapshot.ordered_owner_phases.clear();
    let planner_evidence = LaneReservationSnapshotPlannerEvidence::from_parts_for_test(
        stale_snapshot,
        vec![(
            *group,
            keys.clone(),
            LaneReservationSnapshotPlannerProjectionKind::StrictlyAbsent {
                recovered_state: strict_state,
            },
        )],
    );
    let result = fixture.queue.authorize_lane_reservation_snapshot_recovery(
        checked_startup_reconciliation_receipt(&fixture.queue),
        Vec::new(),
        Some(planner_evidence),
    );
    assert!(matches!(
        result,
        Err(LaneQueueReservationError::InvalidIdentity(ref detail))
            if detail.contains("another exact Queue snapshot")
    ));
    assert_eq!(
        fixture
            .queue
            .lane_reservation_reconciliation_snapshot()
            .expect("recapture owners after stale planner rejection"),
        fixture.snapshot,
        "a stale planner batch must not mutate any durable Queue owner"
    );
}
#[test]
fn snapshot_recovery_rejects_terminal_planner_and_lifecycle_overlap() {
    let fixture = replayed_snapshot_recovery_fixture(&[1], None);
    let (group, keys) = &fixture.groups[0];
    let lifecycle_state = LaneQueueCarrierCleanupGate::direct_test(*group).cleanup_state(
        IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
        IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
        0,
        0,
        0,
    );
    let strict_state =
        strictly_absent_lane_reservation_snapshot_recovery_state(&fixture.snapshot, *group, keys)
            .expect("derive exact strict-absence state");
    let planner_evidence = LaneReservationSnapshotPlannerEvidence::from_parts_for_test(
        fixture.snapshot.clone(),
        vec![(
            *group,
            keys.clone(),
            LaneReservationSnapshotPlannerProjectionKind::StrictlyAbsent {
                recovered_state: strict_state,
            },
        )],
    );
    let lifecycle = snapshot_lifecycle_projection_fixture(
        *group,
        keys.clone(),
        lifecycle_state,
        b"snapshot-planner-overlapping-lifecycle",
    );
    let result = fixture.queue.authorize_lane_reservation_snapshot_recovery(
        checked_startup_reconciliation_receipt(&fixture.queue),
        vec![lifecycle],
        Some(planner_evidence),
    );
    assert!(matches!(
        result,
        Err(LaneQueueReservationError::InvalidIdentity(ref detail))
            if detail.contains("planner-terminal recovery overlaps signed lifecycle")
    ));
    assert_eq!(
        fixture
            .queue
            .lane_reservation_reconciliation_snapshot()
            .expect("recapture owners after overlapping planner rejection"),
        fixture.snapshot
    );
}
#[test]
fn snapshot_recovery_rejects_strict_absence_against_prepared_release_phase() {
    let fixture = replayed_snapshot_recovery_fixture(
        &[2],
        Some((0, AutonomousLaneRetirementQueueSnapshotPhaseV1::Prepared)),
    );
    let (group, keys) = &fixture.groups[0];
    assert_eq!(fixture.snapshot.ordered_owner_phases.len(), keys.len());
    assert!(fixture.snapshot.ordered_owner_phases.iter().all(|phase| {
        phase.reservation_phase == LaneQueueReservationOwnerPhaseV6::ReleasePrepared
    }));
    let strict_state =
        strictly_absent_lane_reservation_snapshot_recovery_state(&fixture.snapshot, *group, keys)
            .expect("prepared owners still retain their exact live admission records");
    let planner_evidence = LaneReservationSnapshotPlannerEvidence::from_parts_for_test(
        fixture.snapshot.clone(),
        vec![(
            *group,
            keys.clone(),
            LaneReservationSnapshotPlannerProjectionKind::StrictlyAbsent {
                recovered_state: strict_state,
            },
        )],
    );
    assert!(matches!(
        fixture.queue.authorize_lane_reservation_snapshot_recovery(
            checked_startup_reconciliation_receipt(&fixture.queue),
            Vec::new(),
            Some(planner_evidence),
        ),
        Err(LaneQueueReservationError::InvalidIdentity(_))
    ));
    assert_eq!(
        fixture
            .queue
            .lane_reservation_reconciliation_snapshot()
            .expect("recapture prepared release after strict-absence rejection"),
        fixture.snapshot,
        "strict absence must neither replace nor release a durable prepared owner"
    );
}
#[test]
fn completed_release_install_and_replay_remain_quarantined_until_explicit_proof() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("completed-release-plans.norito");
    let reservation_path = dir.path().join("completed-release-reservations.norito");
    let barrier = {
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install completed-release queue-plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        let transactions = (0..2)
            .map(|_| accepted_tx_by_someone(&time_source))
            .collect::<Vec<_>>();
        for transaction in &transactions {
            push_globally_bound_lane_reservation_candidate(
                &queue,
                &state,
                &dir,
                transaction.clone(),
            );
        }
        let reserved = queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(
                    &state,
                    b"completed-release-owner",
                    b"completed-release-proposal",
                ),
                nonzero!(2_usize),
            )
            .expect("reserve completed-release batch");
        let keys = reserved.iter().map(|tx| *tx.key()).collect::<Vec<_>>();
        let barrier = lane_reservation_release_barrier(keys, b"completed-release-retirement");
        queue
            .prepare_lane_reservation_release_barrier(&barrier)
            .expect("persist prepared release");
        let reservations = queue.lane_reservations.lock();
        let ordered_records = barrier
            .ordered_keys
            .iter()
            .map(|key| reservations.live_by_hash[&key.signed_transaction_hash].clone())
            .collect();
        drop(reservations);
        queue
            .lane_reservation_journal
            .lock()
            .as_mut()
            .expect("installed reservation journal")
            .complete_release(LaneQueueReservationReleaseCompletionV5 {
                version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION,
                barrier: barrier.clone(),
                ordered_records,
            })
            .expect("persist completed release before simulated restart");
        barrier
    };
    let journal_before = fs::read(&reservation_path).expect("read completed release journal");
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let replay = queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restore completed release without finalizing it");
    assert_eq!(replay.restored, 0);
    assert_eq!(replay.release_barriers, 0);
    assert_eq!(replay.completed_releases, 1);
    assert_eq!(
        queue
            .lane_reservation_snapshot_replay_receipt()
            .expect("completed-owner replay retains its exact receipt")
            .owner_transition_count(),
        2,
        "the replay receipt must cover every exact completed owner"
    );
    assert_eq!(
        queue.lane_reservation_release_barriers(),
        vec![barrier.clone()]
    );
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("reopen completed-release queue-plan journal");
    assert_eq!(
        queue.lane_reservation_release_barriers(),
        vec![barrier.clone()],
        "journal installation must not infer Kura retirement proof"
    );
    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("materialize completed-release payloads under quarantine")
            .replayed,
        2
    );
    assert_eq!(queue.queued_len(), 0);
    assert_eq!(
        queue.lane_reservation_release_barriers(),
        vec![barrier.clone()],
        "payload replay alone must not restore FIFO or forget durable release ownership"
    );
    assert_eq!(
        fs::read(&reservation_path).expect("reread completed release journal"),
        journal_before,
        "install and replay are read-only with respect to the evidence-gated release barrier"
    );
    // Simulate an authenticated Kura proof that the exact entrypoint claims are Released.
    assert_eq!(
        queue
            .finalize_lane_reservation_release_barrier(&barrier)
            .expect("finalize only after simulated external proof"),
        0,
        "the completion frame already performed the durable live-to-released handoff"
    );
    assert!(queue.lane_reservation_release_barriers().is_empty());
    assert_eq!(queue.queued_len(), 2);
}
#[test]
fn reservation_validation_failure_does_not_poison_durability() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Queue::test(config_factory(), &time_source);
    let dir = tempdir().expect("tempdir");
    install_test_reservation_journal(&queue, &dir);
    let mut malformed = lane_reservation_scope(&state, b"owner", b"proposal");
    malformed.lane_block_height = 0;
    let error = match queue.reserve_transactions_for_lane(&state, malformed, nonzero!(1_usize)) {
        Err(error) => error,
        Ok(_) => panic!("zero lane-local height must be rejected before durability changes"),
    };
    assert!(
        matches!(
            &error,
            LaneQueueReservationError::InvalidIdentity(reason)
                if reason == "lane block height must be non-zero"
        ),
        "unexpected validation error: {error:?}"
    );
    assert!(!queue.lane_reservation_durability_faulted());
}
#[test]
fn reservation_restart_restore_blocks_resync_until_explicit_release() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("queue-plans.norito");
    let reservation_path = dir.path().join("lane-reservations.norito");
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.hash();
    let key = {
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, tx);
        queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(&state, b"restart-owner", b"restart-proposal"),
                nonzero!(1_usize),
            )
            .expect("reserve before crash")[0]
            .key()
            .clone()
    };
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let restored = queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restore reservations before transaction replay");
    assert_eq!(restored.restored, 1);
    assert_eq!(restored.awaiting_transaction_replay, 1);
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install plan journal after reservation restore");
    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("replay pending transaction")
            .replayed,
        1
    );
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 0);
    // Force the ordinary resync corridor; the live reservation must never be reinserted.
    let mut global = Vec::new();
    queue.get_transactions_for_block_with_state(&state, nonzero!(1_usize), &mut global);
    assert!(global.is_empty());
    assert!(queue.contains_transaction_hash(hash));
    let reconciliation_receipt = checked_startup_reconciliation_receipt(&queue);
    assert_eq!(
        queue
            .retain_lane_reservation(&key)
            .expect("retain exact durable owner"),
        LaneQueueReservationOutcome::Retained
    );
    assert_eq!(queue.queued_len(), 0);
    assert_eq!(
        queue
            .release_lane_reservations_in_order(core::slice::from_ref(&key))
            .expect("release after the caller's evidence-gated decision"),
        1
    );
    assert!(
        queue.lane_reservation_startup_reconciliation_pending(),
        "restart release must remain quarantined until the State/Kura publication gate"
    );
    queue
        .complete_lane_reservation_startup_reconciliation(reconciliation_receipt)
        .expect("publish completed reservation restart reconciliation");
    assert!(!queue.lane_reservation_startup_reconciliation_pending());
    queue.get_transactions_for_block_with_state(&state, nonzero!(1_usize), &mut global);
    assert_eq!(global[0].as_ref().hash(), hash);
}
#[test]
fn state_committed_live_reservation_replays_quarantined_until_explicit_proof_commit() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("queue-plans-committed-live-owner.norito");
    let reservation_path = dir
        .path()
        .join("lane-reservations-committed-live-owner.norito");
    let transaction = accepted_tx_by_someone(&time_source);
    let hash = transaction.hash();
    let key = {
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install committed-owner plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install committed-owner reservation journal");
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
        *queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(&state, b"committed-live-owner", b"committed-live-proposal"),
                nonzero!(1_usize),
            )
            .expect("reserve transaction before canonical commit")[0]
            .key()
    };
    let block_header = ValidBlock::new_dummy(&checked_random_queue_keypair().into_parts().1)
        .as_ref()
        .header();
    let mut state_block = state.block(block_header);
    state_block
        .transactions
        .insert_block(HashSet::from([hash]), nonzero!(1_usize));
    state_block.commit().expect("commit transaction identity");
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let reservation_replay = queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restore live reservation before plan replay");
    assert_eq!(reservation_replay.restored, 1);
    assert_eq!(reservation_replay.awaiting_transaction_replay, 1);
    assert_eq!(
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install sole payload journal"),
        1
    );
    let replay = queue
        .replay_plan_journal(&state)
        .expect("authenticate and quarantine the sole payload source");
    assert_eq!(
        replay,
        QueuePlanJournalReplaySummary {
            records: 1,
            replayed: 1,
            tombstoned_committed: 0,
            tombstoned_expired: 0,
            tombstoned_conflicting_global_admission: 0,
        }
    );
    assert!(queue.txs.contains_key(&hash));
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 0);
    assert!(queue.tx_hashes.is_empty());
    assert_eq!(queue.live_lane_reservations(), vec![key]);
    assert_eq!(
        queue
            .missing_reservation_payload_count
            .load(Ordering::Relaxed),
        0
    );
    assert_eq!(
        queue
            .plan_journal
            .lock()
            .as_ref()
            .expect("installed sole payload journal")
            .live_record_count()
            .expect("count retained sole payload owner"),
        1,
        "terminal state must not tombstone the only payload source while reservation ownership remains live"
    );
    let mut selected = Vec::new();
    queue.get_transactions_for_block_with_state(&state, nonzero!(1_usize), &mut selected);
    assert!(
        selected.is_empty(),
        "a State-committed replayed reservation stays quarantined outside ordinary FIFO"
    );
    // Simulate the State/Kura reconciliation layer proving this exact carrier committed.
    assert_eq!(
        queue
            .commit_lane_reservation_for_test(&key)
            .expect("consume reservation after simulated external proof"),
        LaneQueueReservationOutcome::Finalized
    );
    assert!(queue.lane_reservation_commit_barriers().is_empty());
    assert!(queue.live_lane_reservations().is_empty());
    assert_eq!(queue.active_len(), 0);
    assert_eq!(
        queue
            .plan_journal
            .lock()
            .as_ref()
            .expect("installed sole payload journal")
            .live_record_count()
            .expect("count post-proof payload owners"),
        0
    );
}
#[test]
fn expired_live_reservation_replays_payload_without_fifo_or_tombstone() {
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("queue-plans-expired-live-owner.norito");
    let reservation_path = dir.path().join("reservations-expired-live-owner.norito");
    let config = Config {
        transaction_time_to_live: Duration::from_millis(1),
        expired_cull_interval: Duration::from_millis(1),
        expired_cull_batch: nonzero!(16_usize),
        ..config_factory()
    };
    let transaction = accepted_tx_by_someone(&time_source);
    let hash = transaction.hash();
    let reserved_key = {
        let queue = Arc::new(Queue::test(config, &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install expired-owner plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install expired-owner reservation journal");
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
        *queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(&state, b"expired-live-owner", b"expired-live-proposal"),
                nonzero!(1_usize),
            )
            .expect("reserve transaction before it expires")[0]
            .key()
    };
    time_handle.advance(Duration::from_millis(2));
    let queue = Arc::new(Queue::test(config, &time_source));
    let reservation_replay = queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restore expired live reservation before plan payload");
    assert_eq!(reservation_replay.restored, 1);
    assert_eq!(reservation_replay.awaiting_transaction_replay, 1);
    assert_eq!(queue.active_len(), 1);
    assert_eq!(
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install expired live-owner plan journal"),
        1
    );
    let summary = queue
        .replay_plan_journal(&state)
        .expect("materialize expired payload under its durable reservation owner");
    assert_eq!(
        summary,
        QueuePlanJournalReplaySummary {
            records: 1,
            replayed: 1,
            tombstoned_committed: 0,
            tombstoned_expired: 0,
            tombstoned_conflicting_global_admission: 0,
        }
    );
    let replayed = queue
        .txs
        .get(&hash)
        .expect("expired reservation payload must be materialized");
    assert!(queue.is_expired(replayed.as_accepted()));
    drop(replayed);
    assert_eq!(
        queue
            .missing_reservation_payload_count
            .load(Ordering::Relaxed),
        0,
        "exact plan replay must clear payload-less owner accounting"
    );
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 0);
    assert!(queue.tx_hashes.is_empty());
    assert_eq!(queue.live_lane_reservations(), vec![reserved_key]);
    assert_eq!(
        queue
            .plan_journal
            .lock()
            .as_ref()
            .expect("installed expired live-owner plan journal")
            .live_record_count()
            .expect("count retained expired live-owner record"),
        1,
        "expiry must not tombstone the sole payload source while reservation ownership is live"
    );
}
#[test]
fn missing_replayed_reservation_owns_capacity_until_exact_payload_replay() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let mut state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("queue-plans-owner-capacity.norito");
    let reservation_path = dir.path().join("lane-reservations-owner-capacity.norito");
    let one_slot_config = || Config {
        capacity: nonzero!(1_usize),
        capacity_per_user: nonzero!(1_usize),
        ..config_factory()
    };
    let transaction = accepted_tx_by_someone(&time_source);
    let transaction_hash = transaction.hash();
    let retained_cost = Queue::retained_byte_cost(Queue::compute_tx_encoded_len(&transaction));
    {
        let queue = Arc::new(Queue::test(one_slot_config(), &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
        queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(&state, b"owner-capacity-owner", b"owner-capacity-proposal"),
                nonzero!(1_usize),
            )
            .expect("reserve before restart");
    }
    let queue = Arc::new(Queue::test(one_slot_config(), &time_source));
    let replay = queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restore the payload-less reservation owner");
    assert_eq!(replay.restored, 1);
    assert_eq!(replay.awaiting_transaction_replay, 1);
    assert!(queue.txs.is_empty());
    assert_eq!(queue.materialized_active_len(), 0);
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.retained_bytes(), TX_RETAINED_OVERHEAD_BYTES);
    let pressure = queue.pressure_snapshot();
    assert_eq!(pressure.tracked_tx_count, 1);
    assert!(pressure.saturated_by_count);
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install the payload journal");
    let unrelated = accepted_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(
        Arc::get_mut(&mut state).expect("unshared owner-capacity test state"),
        &unrelated,
    );
    assert_ne!(unrelated.hash(), transaction_hash);
    let failure = queue
        .push_with_lane_with_state(unrelated, &state)
        .expect_err("startup quarantine must reject unrelated work before payload replay");
    assert!(matches!(
        failure.err,
        Error::PlanJournalDurabilityRejected { ref reason }
            if reason.contains("startup reconciliation")
    ));
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.retained_bytes(), TX_RETAINED_OVERHEAD_BYTES);
    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("the exact payload must replace its reserved capacity")
            .replayed,
        1
    );
    assert_eq!(queue.materialized_active_len(), 1);
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 0);
    assert_eq!(queue.retained_bytes(), retained_cost);
    assert_eq!(
        queue
            .missing_reservation_payload_count
            .load(Ordering::Relaxed),
        0
    );
    assert!(
        queue
            .lane_reservations
            .lock()
            .missing_payload_hashes
            .is_empty()
    );
    let reconciliation_receipt = checked_startup_reconciliation_receipt(&queue);
    queue
        .complete_lane_reservation_startup_reconciliation(reconciliation_receipt)
        .expect("publish completed owner-capacity startup reconciliation");
    let post_replay_unrelated = accepted_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(
        Arc::get_mut(&mut state).expect("unshared owner-capacity test state"),
        &post_replay_unrelated,
    );
    let failure = queue
        .push_with_lane_with_state(post_replay_unrelated, &state)
        .expect_err("the materialized reservation must retain its exact capacity slot");
    assert!(matches!(failure.err, Error::Full));
}
#[test]
fn missing_replayed_reservation_owns_retained_budget_until_exact_payload_replay() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let mut state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("queue-plans-owner-bytes.norito");
    let reservation_path = dir.path().join("lane-reservations-owner-bytes.norito");
    let transaction = accepted_tx_by_someone(&time_source);
    let retained_cost = Queue::retained_byte_cost(Queue::compute_tx_encoded_len(&transaction));
    let bounded_config = || Config {
        capacity: nonzero!(2_usize),
        capacity_per_user: nonzero!(2_usize),
        max_retained_bytes: NonZeroU64::new(retained_cost)
            .expect("transaction retained cost is non-zero"),
        ..config_factory()
    };
    {
        let queue = Arc::new(Queue::test(bounded_config(), &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
        queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(&state, b"owner-bytes-owner", b"owner-bytes-proposal"),
                nonzero!(1_usize),
            )
            .expect("reserve before restart");
    }
    let queue = Arc::new(Queue::test(bounded_config(), &time_source));
    queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restore the payload-less reservation owner");
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.retained_bytes(), TX_RETAINED_OVERHEAD_BYTES);
    let pressure = queue.pressure_snapshot();
    assert!(pressure.saturated_by_count);
    assert!(!pressure.saturated_by_bytes);
    assert!(pressure.is_saturated());
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install the payload journal");
    let unrelated = accepted_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(
        Arc::get_mut(&mut state).expect("unshared owner-bytes test state"),
        &unrelated,
    );
    let failure = queue
        .push_with_lane_with_state(unrelated, &state)
        .expect_err("startup quarantine must reject unrelated work before payload replay");
    assert!(matches!(
        failure.err,
        Error::PlanJournalDurabilityRejected { ref reason }
            if reason.contains("startup reconciliation")
    ));
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.retained_bytes(), TX_RETAINED_OVERHEAD_BYTES);
    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("the exact payload must replace its reserved byte charge")
            .replayed,
        1
    );
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.retained_bytes(), retained_cost);
    assert_eq!(
        queue
            .missing_reservation_payload_count
            .load(Ordering::Relaxed),
        0
    );
    let reconciliation_receipt = checked_startup_reconciliation_receipt(&queue);
    queue
        .complete_lane_reservation_startup_reconciliation(reconciliation_receipt)
        .expect("publish completed owner-bytes startup reconciliation");
    let pressure = queue.pressure_snapshot();
    assert!(!pressure.saturated_by_count);
    assert!(pressure.saturated_by_bytes);
    let post_replay_unrelated = accepted_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(
        Arc::get_mut(&mut state).expect("unshared owner-bytes test state"),
        &post_replay_unrelated,
    );
    let failure = queue
        .push_with_lane_with_state(post_replay_unrelated, &state)
        .expect_err("the materialized payload must retain its exact byte budget");
    assert!(matches!(failure.err, Error::Full));
}
#[test]
fn restart_commit_barrier_stays_quarantined_until_explicit_proof_commit() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let mut state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("queue-plans-commit-window.norito");
    let reservation_path = dir.path().join("lane-reservations-commit-window.norito");
    let transaction = accepted_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(
        Arc::get_mut(&mut state).expect("unshared lane-reservation test state"),
        &transaction,
    );
    let hash = transaction.hash();
    let key = {
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        let binding =
            push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
        assert_eq!(binding.admission_context.proposal_height, 1);
        seed_committed_height_for_queue_test(&state, 4);
        let mut later_scope =
            lane_reservation_scope(&state, b"commit-window-owner", b"commit-window-proposal");
        later_scope.proposal_height = 5;
        later_scope.lane_incarnation = state
            .lane_incarnation_at_height(LaneId::SINGLE, later_scope.proposal_height)
            .expect("the canonical lane remains active at the later reservation height");
        let key = *queue
            .reserve_transactions_for_lane(&state, later_scope, nonzero!(1_usize))
            .expect("reserve transaction")[0]
            .key();
        assert_eq!(key.proposal_height, 5);
        // Model a crash after the reservation Commit fsync and before the independent
        // queue-plan Remove append. The public commit API closes this window with the same
        // durable barrier protocol.
        queue
            .lane_reservation_journal
            .lock()
            .as_mut()
            .expect("reservation journal")
            .commit(key)
            .expect("durable commit barrier");
        key
    };
    let block_header = ValidBlock::new_dummy(&checked_random_queue_keypair().into_parts().1)
        .as_ref()
        .header();
    let mut state_block = state.block(block_header);
    state_block
        .transactions
        .insert_block(HashSet::from([hash]), nonzero!(1_usize));
    state_block.commit().expect("commit transaction identity");
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let restored = queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restore commit barrier before plan journal");
    assert_eq!(restored.restored, 0);
    assert_eq!(restored.commit_barriers, 1);
    assert_eq!(queue.active_len(), 1);
    assert_eq!(
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install the exact payload source without consuming its barrier"),
        1
    );
    assert_eq!(queue.lane_reservation_commit_barriers(), vec![key]);
    let replay = queue
        .replay_plan_journal(&state)
        .expect("materialize the State-committed payload under commit quarantine");
    assert_eq!(replay.replayed, 1);
    assert_eq!(replay.records, 1);
    assert_eq!(queue.lane_reservation_commit_barriers(), vec![key]);
    assert!(queue.txs.contains_key(&hash));
    assert_eq!(queue.queued_len(), 0);
    // Simulate the caller proving the exact global carrier from State and Kura.
    assert_eq!(
        queue
            .commit_lane_reservation_for_test(&key)
            .expect("consume commit barrier after simulated external proof"),
        LaneQueueReservationOutcome::AlreadyFinalized
    );
    assert!(queue.lane_reservation_commit_barriers().is_empty());
    assert_eq!(queue.active_len(), 0);
    assert_eq!(queue.queued_len(), 0);
    assert_eq!(
        queue
            .plan_journal
            .lock()
            .as_ref()
            .expect("installed plan journal")
            .live_record_count()
            .expect("count post-proof plan records"),
        0
    );
}
#[test]
fn stale_reservation_commit_digest_cannot_tombstone_or_forget_live_plan() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("queue-plans-stale-commit.norito");
    let reservation_path = dir.path().join("lane-reservations-stale-commit.norito");
    let hash;
    {
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        let transaction = accepted_tx_by_someone(&time_source);
        hash = transaction.hash();
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
        let mut stale = *queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(&state, b"stale-commit-owner", b"stale-commit-proposal"),
                nonzero!(1_usize),
            )
            .expect("reserve transaction")[0]
            .key();
        stale.routing_plan_digest = Hash::new(b"stale reservation commit plan digest");
        let error = queue
            .remove_plan_journal_for_reservation_commit(&stale)
            .expect_err("a stale commit digest must not append a no-op tombstone");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(queue.plan_journal_durability_faulted());
        assert!(queue.txs.contains_key(&hash));
        assert!(queue.durable_plan_claims.contains_key(&hash));
    }
    let replay_queue = Arc::new(Queue::test(config_factory(), &time_source));
    assert_eq!(
        replay_queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("reopen plan journal after rejected stale tombstone"),
        1,
        "the exact live owner must survive the rejected stale cleanup and restart"
    );
    let replay = replay_queue
        .replay_plan_journal(&state)
        .expect("replay exact plan after rejected stale tombstone");
    assert_eq!(replay.records, 1);
    assert_eq!(replay.replayed, 1);
    assert!(replay_queue.txs.contains_key(&hash));
}
#[test]
fn stale_reservation_commit_binding_cannot_tombstone_or_forget_live_plan() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("queue-plans-stale-commit-binding.norito");
    let reservation_path = dir
        .path()
        .join("lane-reservations-stale-commit-binding.norito");
    let hash;
    {
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        let transaction = accepted_tx_by_someone(&time_source);
        hash = transaction.hash();
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
        let mut stale = *queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(&state, b"stale-binding-owner", b"stale-binding-proposal"),
                nonzero!(1_usize),
            )
            .expect("reserve transaction")[0]
            .key();
        stale.queue_plan_admission_binding_hash =
            Hash::new(b"stale reservation commit admission binding");
        let error = queue
            .remove_plan_journal_for_reservation_commit(&stale)
            .expect_err("a stale binding hash must not append a plan tombstone");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(queue.plan_journal_durability_faulted());
        assert!(queue.txs.contains_key(&hash));
        assert!(queue.durable_plan_claims.contains_key(&hash));
    }
    let replay_queue = Arc::new(Queue::test(config_factory(), &time_source));
    assert_eq!(
        replay_queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("reopen plan journal after rejected stale binding"),
        1,
        "the exact live owner must survive a forged reservation binding"
    );
    let replay = replay_queue
        .replay_plan_journal(&state)
        .expect("replay exact plan after rejected stale binding");
    assert_eq!(replay.records, 1);
    assert_eq!(replay.replayed, 1);
    assert!(replay_queue.txs.contains_key(&hash));
}
#[test]
fn high_volume_commit_barriers_require_explicit_proof_before_consumption() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = test_lane_reservation_plan_path(&dir);
    let reservation_path = dir.path().join("installed-plan-high-volume.norito");
    let make_queue = || {
        Queue::test(
            Config {
                capacity: nonzero!(300_usize),
                capacity_per_user: nonzero!(300_usize),
                ..config_factory()
            },
            &time_source,
        )
    };
    let keys = {
        let queue = Arc::new(make_queue());
        queue
            .install_lane_reservation_journal(&reservation_path, 1)
            .expect("install reservation journal");
        let mut keys = Vec::with_capacity(256);
        for index in 0_u16..256 {
            keys.push(persist_unreconciled_commit_barrier(
                &queue,
                &state,
                &dir,
                accepted_tx_by_someone(&time_source),
                &index.to_le_bytes(),
                &index.wrapping_add(1).to_le_bytes(),
            ));
        }
        keys
    };
    let block_header = ValidBlock::new_dummy(&checked_random_queue_keypair().into_parts().1)
        .as_ref()
        .header();
    let mut state_block = state.block(block_header);
    state_block.transactions.insert_block(
        keys.iter().map(|key| key.signed_transaction_hash).collect(),
        nonzero!(1_usize),
    );
    state_block
        .commit()
        .expect("commit high-volume transaction identities");
    {
        let queue = make_queue();
        let replay = queue
            .install_lane_reservation_journal(&reservation_path, 1)
            .expect("restore high-volume commit barriers before the production plan journal");
        assert_eq!(replay.restored, 0);
        assert_eq!(replay.commit_barriers, 256);
        assert_eq!(queue.lane_reservation_commit_barriers().len(), 256);
        assert_eq!(
            queue
                .install_plan_journal(&plan_path, 1024 * 1024, true)
                .expect("install exact source plan journal without consuming commit barriers"),
            256
        );
        assert_eq!(queue.lane_reservation_commit_barriers().len(), 256);
        assert_eq!(
            queue
                .replay_plan_journal(&state)
                .expect("materialize every committed payload under quarantine"),
            QueuePlanJournalReplaySummary {
                records: 256,
                replayed: 256,
                tombstoned_committed: 0,
                tombstoned_expired: 0,
                tombstoned_conflicting_global_admission: 0,
            }
        );
        assert_eq!(queue.active_len(), 256);
        assert_eq!(queue.queued_len(), 0);
        // Simulate a successful all-groups State/Kura preflight. Consumption remains explicit
        // for every exact reservation identity even at restart scale.
        for key in &keys {
            assert_eq!(
                queue
                    .commit_lane_reservation_for_test(key)
                    .expect("consume one externally proven commit barrier"),
                LaneQueueReservationOutcome::AlreadyFinalized
            );
        }
        assert!(queue.lane_reservation_commit_barriers().is_empty());
        assert_eq!(queue.active_len(), 0);
        assert_eq!(queue.queued_len(), 0);
        assert!(
            reservation_path.metadata().expect("journal metadata").len() < 4096,
            "explicit proof-driven commits must still bound reconciled barrier history"
        );
    }
    let queue = make_queue();
    let replay = queue
        .install_lane_reservation_journal(&reservation_path, 1)
        .expect("restart compacted reservation journal");
    assert_eq!(replay, LaneQueueReservationReplaySummary::default());
    assert_eq!(
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("restart reconciled production plan journal"),
        0
    );
    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("second restart remains empty"),
        QueuePlanJournalReplaySummary::default()
    );
}
#[test]
fn replay_late_forged_commit_barrier_preserves_every_durable_owner() {
    const JOURNAL_LIMIT: u64 = 4 * 1024 * 1024;
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = test_lane_reservation_plan_path(&dir);
    let reservation_path = dir.path().join("late-forged-barrier-reservations.norito");
    let keys = {
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        queue
            .install_lane_reservation_journal(&reservation_path, JOURNAL_LIMIT)
            .expect("install seed reservation journal");
        (0_u8..2)
            .map(|index| {
                persist_unreconciled_commit_barrier(
                    &queue,
                    &state,
                    &dir,
                    accepted_unique_entrypoint_tx_by_someone(&time_source),
                    &[index],
                    &[index.wrapping_add(1)],
                )
            })
            .collect::<Vec<_>>()
    };
    let reservation_journal_before =
        fs::read(&reservation_path).expect("read exact reservation journal");
    let plan_journal_before = fs::read(&plan_path).expect("read exact QueuePlan journal");
    let queue = Queue::test(config_factory(), &time_source);
    let restored = queue
        .install_lane_reservation_journal(&reservation_path, JOURNAL_LIMIT)
        .expect("restore both durable commit barriers");
    assert_eq!(restored.commit_barriers, 2);
    assert_eq!(queue.active_len(), 2);
    assert!(queue.txs.is_empty());
    // Leave the first owner exact and forge only the later owner's immutable plan binding.
    // Replay must preflight the full record set before publishing the valid prefix.
    let mut forged = keys[1];
    forged.routing_plan_digest = Hash::new(b"forged later QueuePlan digest");
    queue.lane_reservations.lock().commit_barriers[1] = forged;
    assert_eq!(
        queue
            .install_plan_journal(&plan_path, JOURNAL_LIMIT, true)
            .expect("install is intentionally read-only with respect to commit barriers"),
        2
    );
    let error = queue
        .replay_plan_journal(&state)
        .expect_err("later forged barrier must reject the complete replay batch");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        error
            .to_string()
            .contains("conflicts with durable reservation ownership"),
        "unexpected late-conflict replay error: {error}"
    );
    assert_eq!(queue.active_len(), 2);
    assert_eq!(queue.queued_len(), 0);
    assert!(queue.txs.is_empty());
    assert!(queue.routing_plans.is_empty());
    assert!(queue.durable_plan_claims.is_empty());
    let mut expected_barriers = vec![keys[0], forged];
    expected_barriers.sort_by_key(LaneQueueReservationKeyV2::digest);
    assert_eq!(queue.lane_reservation_commit_barriers(), expected_barriers);
    assert_eq!(
        queue
            .plan_journal
            .lock()
            .as_ref()
            .expect("installed QueuePlan journal")
            .live_record_count()
            .expect("count retained QueuePlan owners"),
        2
    );
    assert_eq!(
        fs::read(&reservation_path).expect("reread reservation journal"),
        reservation_journal_before,
        "a later conflict must append neither ForgetCommit nor any other reservation mutation"
    );
    assert_eq!(
        fs::read(&plan_path).expect("reread QueuePlan journal"),
        plan_journal_before,
        "a later conflict must not tombstone the already-validated prefix"
    );
}
#[test]
fn restart_commit_barrier_rejects_mismatched_queue_hash_without_tombstone_or_forget() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = test_lane_reservation_plan_path(&dir);
    let reservation_path = dir.path().join("restart-mismatched-queue-hash.norito");
    let key = {
        let queue = Queue::test(config_factory(), &time_source);
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        persist_unreconciled_commit_barrier(
            &queue,
            &state,
            &dir,
            accepted_tx_by_someone(&time_source),
            b"mismatched-hash-owner",
            b"mismatched-hash-proposal",
        )
    };
    let block_header = ValidBlock::new_dummy(&checked_random_queue_keypair().into_parts().1)
        .as_ref()
        .header();
    let mut state_block = state.block(block_header);
    state_block.transactions.insert_block(
        HashSet::from([key.signed_transaction_hash]),
        nonzero!(1_usize),
    );
    state_block
        .commit()
        .expect("commit exact transaction identity");
    {
        let queue = Queue::test(config_factory(), &time_source);
        assert_eq!(
            queue
                .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
                .expect("restore exact commit barrier")
                .commit_barriers,
            1
        );
        queue.lane_reservations.lock().commit_barriers[0].signed_transaction_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"forged compatibility queue hash"));
        assert_eq!(
            queue
                .install_plan_journal(&plan_path, 1024 * 1024, true)
                .expect("install must retain the unverified barrier"),
            1
        );
        let error = queue
            .replay_plan_journal(&state)
            .expect_err("mismatched queue hash must fail before replay publication");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(queue.lane_reservation_commit_barriers().len(), 1);
        assert_eq!(
            queue
                .plan_journal
                .lock()
                .as_mut()
                .expect("installed plan journal remains inspectable")
                .replay()
                .expect("replay rejected cleanup")
                .len(),
            1
        );
    }
    let queue = Queue::test(config_factory(), &time_source);
    assert_eq!(
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("ForgetCommit was not appended")
            .commit_barriers,
        1
    );
    assert_eq!(
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("the original exact barrier remains recoverable"),
        1
    );
    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("authenticate the original exact barrier")
            .replayed,
        1
    );
    assert_eq!(queue.lane_reservation_commit_barriers(), vec![key]);
    assert_eq!(
        queue
            .commit_lane_reservation_for_test(&key)
            .expect("consume only after simulated external proof"),
        LaneQueueReservationOutcome::AlreadyFinalized
    );
    assert!(queue.lane_reservation_commit_barriers().is_empty());
}
#[test]
fn restart_commit_barrier_rejects_retargeted_coordinator_without_tombstone_or_forget() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = test_lane_reservation_plan_path(&dir);
    let reservation_path = dir.path().join("restart-retargeted-coordinator.norito");
    let key = {
        let queue = Queue::test(config_factory(), &time_source);
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        persist_unreconciled_commit_barrier(
            &queue,
            &state,
            &dir,
            accepted_tx_by_someone(&time_source),
            b"retargeted-owner",
            b"retargeted-proposal",
        )
    };
    let block_header = ValidBlock::new_dummy(&checked_random_queue_keypair().into_parts().1)
        .as_ref()
        .header();
    let mut state_block = state.block(block_header);
    state_block.transactions.insert_block(
        HashSet::from([key.signed_transaction_hash]),
        nonzero!(1_usize),
    );
    state_block
        .commit()
        .expect("commit exact transaction identity");
    {
        let queue = Queue::test(config_factory(), &time_source);
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("restore exact commit barrier");
        let retargeted = RoutingDecision::new(LaneId::new(77), DataSpaceId::new(88));
        let mut store = queue.lane_reservations.lock();
        let key = &mut store.commit_barriers[0];
        key.coordinator_leg = RouteLeg::new(retargeted, RouteLegRole::Coordinator);
        key.lane_id = retargeted.lane_id;
        key.dataspace_id = retargeted.dataspace_id;
        key.lane_incarnation = Hash::new(b"retargeted lane incarnation");
        drop(store);
        assert_eq!(
            queue
                .install_plan_journal(&plan_path, 1024 * 1024, true)
                .expect("install must retain the unverified retargeted barrier"),
            1
        );
        let error = queue
            .replay_plan_journal(&state)
            .expect_err("retargeted coordinator must fail before replay publication");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(queue.lane_reservation_commit_barriers().len(), 1);
        assert_eq!(
            queue
                .plan_journal
                .lock()
                .as_mut()
                .expect("installed plan journal")
                .replay()
                .expect("replay rejected retarget")
                .len(),
            1
        );
    }
    let queue = Queue::test(config_factory(), &time_source);
    assert_eq!(
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("retarget rejection appended no ForgetCommit")
            .commit_barriers,
        1
    );
    assert_eq!(
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("the durable exact coordinator remains recoverable"),
        1
    );
    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("authenticate the original coordinator binding")
            .replayed,
        1
    );
    assert_eq!(queue.lane_reservation_commit_barriers(), vec![key]);
    assert_eq!(
        queue
            .commit_lane_reservation_for_test(&key)
            .expect("consume only after simulated external proof"),
        LaneQueueReservationOutcome::AlreadyFinalized
    );
    assert!(queue.lane_reservation_commit_barriers().is_empty());
}
#[test]
fn restart_commit_barrier_rejects_same_plan_binding_aba_without_tombstone_or_forget() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = test_lane_reservation_plan_path(&dir);
    let reservation_path = dir.path().join("restart-same-plan-binding-aba.norito");
    {
        let queue = Queue::test(config_factory(), &time_source);
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        let _ = persist_unreconciled_commit_barrier(
            &queue,
            &state,
            &dir,
            accepted_tx_by_someone(&time_source),
            b"aba-owner",
            b"aba-proposal",
        );
        let mut journal = queue.plan_journal.lock();
        let journal = journal.as_mut().expect("installed plan journal");
        let mut replacement = journal
            .replay()
            .expect("read original exact claim")
            .pop()
            .expect("one original exact claim");
        replacement.enqueue_timestamp_ms = replacement.enqueue_timestamp_ms.saturating_add(1);
        journal
            .replace_strict_durable(replacement)
            .expect("persist same-plan replacement binding");
    }
    {
        let queue = Queue::test(config_factory(), &time_source);
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("restore stale commit barrier");
        assert_eq!(
            queue
                .install_plan_journal(&plan_path, 1024 * 1024, true)
                .expect("installation must not consume the stale barrier"),
            1
        );
        let error = queue
            .replay_plan_journal(&state)
            .expect_err("stale binding must reject replay before publication");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(queue.lane_reservation_commit_barriers().len(), 1);
        assert_eq!(
            queue
                .plan_journal
                .lock()
                .as_mut()
                .expect("installed plan journal")
                .replay()
                .expect("replacement survives stale cleanup")
                .len(),
            1
        );
    }
    let queue = Queue::test(config_factory(), &time_source);
    assert_eq!(
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("stale ABA cleanup appended no ForgetCommit")
            .commit_barriers,
        1
    );
    assert_eq!(
        QueuePlanJournal::open_with_limits(
            &plan_path,
            QueuePlanJournalLimits::new(
                1024 * 1024,
                queue.max_retained_bytes.get().min(u64::from(u32::MAX)),
                (1024_u64 * 1024)
                    .saturating_add(queue.max_retained_bytes.get())
                    .saturating_add(queue.max_retained_bytes.get().min(u64::from(u32::MAX)),),
                queue.capacity.get(),
            ),
            true,
        )
        .expect("open replacement plan journal")
        .replay()
        .expect("replay replacement after second restart")
        .len(),
        1
    );
}
#[test]
fn plan_tombstoned_commit_barrier_replays_absent_until_explicit_proof() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = test_lane_reservation_plan_path(&dir);
    let reservation_path = dir.path().join("restart-after-plan-tombstone.norito");
    let key = {
        let queue = Queue::test(config_factory(), &time_source);
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        let key = persist_unreconciled_commit_barrier(
            &queue,
            &state,
            &dir,
            accepted_tx_by_someone(&time_source),
            b"tombstone-owner",
            b"tombstone-proposal",
        );
        assert_eq!(
            queue
                .plan_journal
                .lock()
                .as_mut()
                .expect("installed plan journal")
                .remove_exact_global_admission_binding_strict_durable(&key)
                .expect("persist plan tombstone before simulated crash"),
            QueuePlanJournalExactRemoveResult::Removed
        );
        key
    };
    let block_header = ValidBlock::new_dummy(&checked_random_queue_keypair().into_parts().1)
        .as_ref()
        .header();
    let mut state_block = state.block(block_header);
    state_block.transactions.insert_block(
        HashSet::from([key.signed_transaction_hash]),
        nonzero!(1_usize),
    );
    state_block
        .commit()
        .expect("commit exact transaction identity");
    let queue = Queue::test(config_factory(), &time_source);
    assert_eq!(
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("restore unforgotten commit barrier")
            .commit_barriers,
        1
    );
    assert_eq!(
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("an already durable plan tombstone is idempotent"),
        0
    );
    assert_eq!(
        queue.lane_reservation_commit_barriers(),
        vec![key],
        "plan installation must retain the barrier until State/Kura evidence is checked"
    );
    assert_eq!(queue.active_len(), 1);
    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("State-committed tombstoned payload may replay absent"),
        QueuePlanJournalReplaySummary::default()
    );
    assert_eq!(queue.lane_reservation_commit_barriers(), vec![key]);
    let snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture unmarked tombstone crash window");
    assert_eq!(
        snapshot.ordered_owner_phases,
        vec![LaneQueueReservationRecoveryPhaseV1 {
            key,
            reservation_phase: LaneQueueReservationOwnerPhaseV6::CommitBarrier,
            queue_plan_phase: QueuePlanReservationPhaseV1::Tombstoned,
            plan_tombstone_marked: false,
        }]
    );
    // Simulate proof of the exact canonical carrier. The absent payload is safe only because
    // State membership and the durable Commit identity were both retained for this check.
    queue.hold_next_lane_reservation_commit_after_plan_marker_for_test();
    assert_eq!(
        queue
            .commit_lane_reservation_for_test(&key)
            .expect("persist V6 marker before simulated publication crash"),
        LaneQueueReservationOutcome::AlreadyFinalized
    );
    assert_eq!(queue.lane_reservation_commit_barriers(), vec![key]);
    assert!(
        queue.lane_reservations.lock().plan_tombstoned.is_empty(),
        "simulated crash must occur before process-local marker publication"
    );
    assert_eq!(queue.active_len(), 1);
    queue
        .plan_journal
        .lock()
        .as_mut()
        .expect("installed plan journal")
        .compact_if_needed()
        .expect("compact away retained V4 tombstone after V6 marker");
    drop(queue);
    let queue = Queue::test(config_factory(), &time_source);
    assert_eq!(
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("restore exact marked commit barrier")
            .plan_tombstoned,
        1
    );
    assert_eq!(
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install compacted plan journal"),
        0
    );
    queue
        .replay_plan_journal(&state)
        .expect("V6 marker proves compacted V4 tombstone");
    assert_eq!(
        queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture marked restart phase")
            .ordered_owner_phases,
        vec![LaneQueueReservationRecoveryPhaseV1 {
            key,
            reservation_phase: LaneQueueReservationOwnerPhaseV6::CommitBarrier,
            queue_plan_phase: QueuePlanReservationPhaseV1::Tombstoned,
            plan_tombstone_marked: true,
        }]
    );
    assert_eq!(
        queue
            .commit_lane_reservation_for_test(&key)
            .expect("forget exactly marked barrier after restart"),
        LaneQueueReservationOutcome::AlreadyFinalized
    );
    assert!(queue.lane_reservation_commit_barriers().is_empty());
}
#[test]
fn unmarked_commit_without_live_or_retained_v4_tombstone_fails_closed() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = test_lane_reservation_plan_path(&dir);
    let reservation_path = dir.path().join("unproven-plan-tombstone.norito");
    let key = {
        let queue = Queue::test(config_factory(), &time_source);
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        let key = persist_unreconciled_commit_barrier(
            &queue,
            &state,
            &dir,
            accepted_tx_by_someone(&time_source),
            b"unproven-tombstone-owner",
            b"unproven-tombstone-proposal",
        );
        let mut plan = queue.plan_journal.lock();
        let plan = plan.as_mut().expect("installed plan journal");
        assert_eq!(
            plan.remove_exact_global_admission_binding_strict_durable(&key)
                .expect("persist exact V4 tombstone"),
            QueuePlanJournalExactRemoveResult::Removed
        );
        plan.compact_if_needed()
            .expect("compact away unmarked V4 tombstone evidence");
        key
    };
    let block_header = ValidBlock::new_dummy(&checked_random_queue_keypair().into_parts().1)
        .as_ref()
        .header();
    let mut state_block = state.block(block_header);
    state_block.transactions.insert_block(
        HashSet::from([key.signed_transaction_hash]),
        nonzero!(1_usize),
    );
    state_block
        .commit()
        .expect("commit exact transaction identity");
    let queue = Queue::test(config_factory(), &time_source);
    let reservation_replay = queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restore unmarked commit barrier");
    assert_eq!(reservation_replay.commit_barriers, 1);
    assert_eq!(reservation_replay.plan_tombstoned, 0);
    assert_eq!(
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install compacted empty plan journal"),
        0
    );
    let error = queue
        .replay_plan_journal(&state)
        .expect_err("unmarked absence without retained V4 evidence must fail closed");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        error
            .to_string()
            .contains("neither live nor exactly tombstoned"),
        "unexpected unproven tombstone rejection: {error}"
    );
    assert_eq!(queue.lane_reservation_commit_barriers(), vec![key]);
    assert!(queue.lane_reservations.lock().plan_tombstoned.is_empty());
}
#[test]
fn commit_barrier_pressure_clears_only_after_explicit_proof_commit() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = test_lane_reservation_plan_path(&dir);
    let reservation_path = dir.path().join("restart-backpressure-refresh.norito");
    let make_queue = || {
        Queue::test(
            Config {
                capacity: nonzero!(1_usize),
                capacity_per_user: nonzero!(1_usize),
                ..config_factory()
            },
            &time_source,
        )
    };
    let key = {
        let queue = make_queue();
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        persist_unreconciled_commit_barrier(
            &queue,
            &state,
            &dir,
            accepted_tx_by_someone(&time_source),
            b"pressure-owner",
            b"pressure-proposal",
        )
    };
    let block_header = ValidBlock::new_dummy(&checked_random_queue_keypair().into_parts().1)
        .as_ref()
        .header();
    let mut state_block = state.block(block_header);
    state_block.transactions.insert_block(
        HashSet::from([key.signed_transaction_hash]),
        nonzero!(1_usize),
    );
    state_block
        .commit()
        .expect("commit exact transaction identity");
    let queue = make_queue();
    assert_eq!(
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("restore capacity-owning commit barrier")
            .commit_barriers,
        1
    );
    assert_eq!(
        queue.active_len(),
        1,
        "the payload-less Commit barrier must still own queue capacity"
    );
    let pressure = queue.backpressure_handle();
    assert!(
        matches!(
            pressure.snapshot(),
            BackpressureState::Saturated {
                queued: 0,
                capacity,
            } if capacity == nonzero!(1_usize)
        ),
        "the restored owner is capacity pressure, not an ordinary queued payload"
    );
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install exact payload source without consuming the barrier");
    assert!(
        pressure.snapshot().is_saturated(),
        "plan installation cannot clear unverified reservation ownership"
    );
    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("materialize committed payload under quarantine")
            .replayed,
        1
    );
    assert_eq!(queue.queued_len(), 0);
    assert!(
        pressure.snapshot().is_saturated(),
        "payload replay cannot clear unverified reservation ownership"
    );
    let reconciliation_receipt = checked_startup_reconciliation_receipt(&queue);
    assert_eq!(
        queue
            .commit_lane_reservation_for_test(&key)
            .expect("consume barrier after simulated external proof"),
        LaneQueueReservationOutcome::AlreadyFinalized
    );
    assert!(
        pressure.snapshot().is_saturated(),
        "consuming the barrier cannot publish before startup reconciliation completes"
    );
    queue
        .complete_lane_reservation_startup_reconciliation(reconciliation_receipt)
        .expect("publish completed commit-barrier startup reconciliation");
    assert_eq!(
        pressure.snapshot(),
        BackpressureState::Healthy {
            queued: 0,
            capacity: nonzero!(1_usize),
        },
        "the explicit proof-driven commit must publish the terminal tracked count"
    );
}
#[test]
fn globally_bound_reservation_survives_expiry_until_canonical_commit() {
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(
        Config {
            transaction_time_to_live: Duration::from_millis(1),
            expired_cull_interval: Duration::from_millis(1),
            expired_cull_batch: nonzero!(16_usize),
            ..config_factory()
        },
        &time_source,
    ));
    let dir = tempdir().expect("tempdir");
    install_test_reservation_journal(&queue, &dir);
    let transaction = accepted_tx_by_someone(&time_source);
    let hash = transaction.hash();
    let binding = push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
    let key = queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(&state, b"expiry-owner", b"expiry-proposal"),
            nonzero!(1_usize),
        )
        .expect("reserve expiring transaction")[0]
        .key()
        .clone();
    time_handle.advance(Duration::from_millis(2));
    assert_eq!(queue.cull_expired_entries(time_source.get_unix_time()), 0);
    assert_eq!(queue.active_len(), 1);
    queue
        .release_lane_reservation(&key)
        .expect("release expired reservation");
    assert_eq!(
        queue.queued_len(),
        1,
        "release must restore the globally certified owner to ordinary FIFO even after TTL expiry"
    );
    assert_eq!(
        queue.cull_expired_entries(time_source.get_unix_time()),
        0,
        "TTL expiry cannot erase an exact globally certified owner after lane release"
    );
    assert_eq!(queue.active_len(), 1);
    assert!(
        queue
            .durable_plan_claims
            .get(&hash)
            .is_some_and(|claim| claim.journal_record_digest == binding.journal_record_digest)
    );
    assert_eq!(
        queue.remove_committed_hashes([hash], None),
        1,
        "canonical execution must terminally consume the durable owner"
    );
    assert_eq!(queue.active_len(), 0);
    assert_eq!(queue.retained_bytes(), 0);
    assert!(!queue.durable_plan_claims.contains_key(&hash));
    assert!(
        queue
            .plan_journal
            .lock()
            .as_ref()
            .expect("installed plan journal")
            .replay()
            .expect("replay post-commit journal")
            .is_empty(),
        "canonical execution must durably tombstone the exact plan claim"
    );
}
#[test]
fn concurrent_lane_reserve_attempts_cannot_duplicate_one_transaction() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_test_reservation_journal(&queue, &dir);
    push_globally_bound_lane_reservation_candidate(
        &queue,
        &state,
        &dir,
        accepted_tx_by_someone(&time_source),
    );
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let mut handles = Vec::new();
    for index in 0_u8..2 {
        let queue = Arc::clone(&queue);
        let state = Arc::clone(&state);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            queue
                .reserve_transactions_for_lane(
                    &state,
                    lane_reservation_scope(&state, &[b'o', index], &[b'p', index]),
                    nonzero!(1_usize),
                )
                .expect("concurrent reservation attempt")
                .len()
        }));
    }
    barrier.wait();
    let selected: usize = handles
        .into_iter()
        .map(|handle| handle.join().expect("join reservation attempt"))
        .sum();
    assert_eq!(selected, 1);
    assert_eq!(queue.live_lane_reservations().len(), 1);
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 0);
}
#[test]
fn stale_lane_incarnation_identity_fails_closed() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_test_reservation_journal(&queue, &dir);
    push_globally_bound_lane_reservation_candidate(
        &queue,
        &state,
        &dir,
        accepted_tx_by_someone(&time_source),
    );
    let mut stale = lane_reservation_scope(&state, b"stale-owner", b"stale-proposal");
    stale.lane_incarnation = Hash::new(b"retired-incarnation");
    assert!(matches!(
        queue.reserve_transactions_for_lane(&state, stale, nonzero!(1_usize)),
        Err(LaneQueueReservationError::StaleLaneIncarnation)
    ));
    assert_eq!(queue.queued_len(), 1);
}
include!("native_amx_reservation_tests.rs");
#[test]
fn opposite_global_and_lane_call_orders_never_select_the_same_hash() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let first = accepted_tx_by_someone(&time_source);
    let second = accepted_tx_by_someone(&time_source);
    let all_hashes = BTreeSet::from([first.hash(), second.hash()]);
    let run = |lane_first: bool, suffix: &str| {
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join(format!("reservations-{suffix}.norito"));
        queue
            .install_plan_journal(
                dir.path().join(format!("queue-plans-{suffix}.norito")),
                1024 * 1024,
                true,
            )
            .expect("install globally certified reservation plan journal");
        queue
            .install_lane_reservation_journal(path, 1024 * 1024)
            .expect("install reservation journal");
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, first.clone());
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, second.clone());
        let mut global = Vec::new();
        let reserved = if lane_first {
            let reserved = queue
                .reserve_transactions_for_lane(
                    &state,
                    lane_reservation_scope(&state, b"order-owner", suffix.as_bytes()),
                    nonzero!(1_usize),
                )
                .expect("lane-first reservation");
            queue.get_transactions_for_block_with_state(&state, nonzero!(1_usize), &mut global);
            reserved
        } else {
            queue.get_transactions_for_block_with_state(&state, nonzero!(1_usize), &mut global);
            queue
                .reserve_transactions_for_lane(
                    &state,
                    lane_reservation_scope(&state, b"order-owner", suffix.as_bytes()),
                    nonzero!(1_usize),
                )
                .expect("global-first reservation")
        };
        let global_hash = global[0].as_ref().hash();
        let reserved_hash = reserved[0].as_accepted().hash();
        assert_ne!(global_hash, reserved_hash);
        assert_eq!(BTreeSet::from([global_hash, reserved_hash]), all_hashes);
    };
    run(true, "lane-first");
    run(false, "global-first");
}
#[test]
fn fee_capacity_reservations_prevent_queue_oversubscription() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let first_hash = accepted_tx_by_someone(&time_source).hash();
    let second_hash = accepted_tx_by_someone(&time_source).hash();
    assert_ne!(
        first_hash, second_hash,
        "fixtures must identify two transactions"
    );
    let (sponsor, _) = gen_account_in("sponsor");
    let (beneficiary, _) = gen_account_in("beneficiary");
    let program_id = FeeSponsorProgramId::new(
        sponsor,
        "queue_capacity".parse().expect("valid program name"),
    );
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("fees", "universal").expect("valid fee domain"),
        "xor".parse().expect("valid asset name"),
    );
    let amount = Quantity::from(6_u32);
    let remaining = Quantity::from(10_u32);
    let reservation = || {
        let block_key = FeeSponsorBudgetCounterKey {
            program_id: program_id.clone(),
            asset_definition_id: asset_definition_id.clone(),
            window: FeeSponsorBudgetWindow::Block(FeeSponsorBlockBudgetWindow { height: 7 }),
        };
        let program_epoch_key = FeeSponsorBudgetCounterKey {
            program_id: program_id.clone(),
            asset_definition_id: asset_definition_id.clone(),
            window: FeeSponsorBudgetWindow::ProgramEpoch(FeeSponsorProgramEpochBudgetWindow {
                epoch: 1,
            }),
        };
        let beneficiary_epoch_key = FeeSponsorBudgetCounterKey {
            program_id: program_id.clone(),
            asset_definition_id: asset_definition_id.clone(),
            window: FeeSponsorBudgetWindow::BeneficiaryEpoch(
                FeeSponsorBeneficiaryEpochBudgetWindow {
                    epoch: 1,
                    beneficiary: beneficiary.clone(),
                },
            ),
        };
        let source = FeeReservationAssetSource::SponsorProgram {
            program_id: program_id.clone(),
            asset_definition_id: asset_definition_id.clone(),
        };
        FeeAdmissionReservation {
            program_revision: Some(1),
            beneficiary: beneficiary.clone(),
            asset_charges: BTreeMap::from([(source.clone(), amount.clone())]),
            window_charges: BTreeMap::from([
                (block_key.clone(), amount.clone()),
                (program_epoch_key.clone(), amount.clone()),
                (beneficiary_epoch_key.clone(), amount.clone()),
            ]),
            relay_lease_charges: BTreeMap::new(),
            asset_remaining: BTreeMap::from([(source, Quantity::from(100_u32))]),
            window_remaining: BTreeMap::from([
                (block_key, remaining.clone()),
                (program_epoch_key, remaining.clone()),
                (beneficiary_epoch_key, remaining.clone()),
            ]),
            relay_lease_remaining: BTreeMap::new(),
        }
    };
    let mut store = FeeAdmissionReservationStore::default();
    store
        .reserve(first_hash, reservation())
        .expect("first transaction reserves the shared capacity");
    let err = store
        .reserve(second_hash, reservation())
        .expect_err("second transaction must not overbook the same snapshot");
    assert!(matches!(
        err,
        Error::NexusFeeAdmissionRejected {
            code: FeeRejectionCode::ProgramBlockBudgetExhausted,
            ..
        }
    ));
    store.release(&first_hash);
    store
        .reserve(second_hash, reservation())
        .expect("released capacity is immediately reusable");
}
#[test]
fn fee_reservation_refresh_moves_carried_transaction_to_current_block_window() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let first_hash = accepted_tx_by_someone(&time_source).hash();
    let second_hash = accepted_tx_by_someone(&time_source).hash();
    let (sponsor, _) = gen_account_in("refresh_fee_reservation");
    let (beneficiary, _) = gen_account_in("refresh_fee_beneficiary");
    let program_id =
        FeeSponsorProgramId::new(sponsor, "rollover".parse().expect("valid program name"));
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("fees", "universal").expect("valid fee domain"),
        "xor".parse().expect("valid asset name"),
    );
    let reservation_at = |height| {
        let key = FeeSponsorBudgetCounterKey {
            program_id: program_id.clone(),
            asset_definition_id: asset_definition_id.clone(),
            window: FeeSponsorBudgetWindow::Block(FeeSponsorBlockBudgetWindow { height }),
        };
        FeeAdmissionReservation {
            program_revision: Some(1),
            beneficiary: beneficiary.clone(),
            asset_charges: BTreeMap::new(),
            window_charges: BTreeMap::from([(key.clone(), Quantity::from(6_u32))]),
            relay_lease_charges: BTreeMap::new(),
            asset_remaining: BTreeMap::new(),
            window_remaining: BTreeMap::from([(key, Quantity::from(10_u32))]),
            relay_lease_remaining: BTreeMap::new(),
        }
    };
    let mut store = FeeAdmissionReservationStore::default();
    store
        .reserve(first_hash, reservation_at(7))
        .expect("transaction reserves its enqueue-height window");
    store
        .refresh(first_hash, Some(reservation_at(8)))
        .expect("pop-time recheck moves the hold to the execution-height window");
    let err = store
        .reserve(second_hash, reservation_at(8))
        .expect_err("a competing transaction must see the refreshed current-height hold");
    assert!(matches!(
        err,
        Error::NexusFeeAdmissionRejected {
            code: FeeRejectionCode::ProgramBlockBudgetExhausted,
            ..
        }
    ));
    store
        .refresh(first_hash, None)
        .expect("disabling fee charging releases the stale hold");
    store
        .reserve(second_hash, reservation_at(8))
        .expect("released current-height capacity is reusable");
}
#[test]
fn unsigned_payload_routing_matches_signed_queue_admission_routing() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let tx = accepted_tx_by_someone(&time_source);
    let payload = tx
        .external()
        .expect("external transaction fixture")
        .payload()
        .clone();
    let state = State::new_for_testing(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let queue = Queue::test(config_factory(), &time_source);
    let signed = queue
        .route_plan_with_state(&tx, &state)
        .expect("signed route");
    let unsigned = queue
        .route_payload_plan_with_state(&payload, &state)
        .expect("unsigned route");
    assert_eq!(unsigned, signed);
}
#[test]
fn receipt_settled_queue_admission_rejects_authority_payer() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let (authority, keypair) = gen_account_in("receipt_fee_admission");
    let domain_id =
        DomainId::try_new("receipt_fee_admission", "universal").expect("receipt fee domain");
    let fee_asset = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "xor".parse().expect("receipt fee asset name"),
    );
    let world = World::with(
        [Domain::new(domain_id).build(&authority)],
        [Account::new(authority.clone()).build(&authority)],
        [AssetDefinition::numeric(
            fee_asset.clone(),
            "receipt fee XOR".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&authority)],
    );
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    {
        let mut nexus = state.nexus.write();
        nexus.enabled = true;
        nexus.fees.settlement_mode =
            iroha_config::parameters::actual::NexusFeeSettlementMode::LaneRelayBurn;
        nexus.fees.fee_asset_id = fee_asset.canonical_address();
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
    }
    let queue = Queue::test(config_factory(), &time_source);
    let transaction = accepted_tx_by(authority, &keypair, &time_source);
    let error = queue
        .push(transaction, state.view())
        .expect_err("receipt-settled queue admission must require a sponsor");
    assert!(matches!(
        error.err,
        Error::NexusFeeAdmissionRejected {
            code: FeeRejectionCode::RelayCapacityUnavailable,
            ref reason,
        } if reason.contains("active fee sponsor program")
            && reason.contains("exact active revision")
    ));
    assert_eq!(queue.active_len(), 0);
}
#[test]
fn authority_fee_reservations_prevent_overbooking_and_release_capacity() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let first_hash = accepted_tx_by_someone(&time_source).hash();
    let second_hash = accepted_tx_by_someone(&time_source).hash();
    let (authority, _) = gen_account_in("authority_fee_reservation");
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("fees", "universal").expect("valid fee domain"),
        "xor".parse().expect("valid asset name"),
    );
    let asset_id = AssetId::new(asset_definition_id, authority.clone());
    let source = FeeReservationAssetSource::Authority(asset_id);
    let reservation = || FeeAdmissionReservation {
        program_revision: None,
        beneficiary: authority.clone(),
        asset_charges: BTreeMap::from([(source.clone(), Quantity::from(6_u32))]),
        window_charges: BTreeMap::new(),
        relay_lease_charges: BTreeMap::new(),
        asset_remaining: BTreeMap::from([(source.clone(), Quantity::from(10_u32))]),
        window_remaining: BTreeMap::new(),
        relay_lease_remaining: BTreeMap::new(),
    };
    let mut store = FeeAdmissionReservationStore::default();
    store
        .reserve(first_hash, reservation())
        .expect("first authority transaction reserves its balance");
    let err = store
        .reserve(second_hash, reservation())
        .expect_err("second authority transaction must not overbook the balance");
    assert!(matches!(
        err,
        Error::NexusFeeAdmissionRejected {
            code: FeeRejectionCode::AuthorityPayerInsufficient,
            ..
        }
    ));
    store.release(&first_hash);
    store
        .reserve(second_hash, reservation())
        .expect("released authority capacity is immediately reusable");
}
#[test]
fn relay_spend_lease_reservations_prevent_overbooking_and_release_capacity() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let first_hash = accepted_tx_by_someone(&time_source).hash();
    let second_hash = accepted_tx_by_someone(&time_source).hash();
    let (beneficiary, _) = gen_account_in("relay_lease_reservation");
    let lease_id = Hash::new(b"exact verified sponsor spend lease");
    let reservation = || FeeAdmissionReservation {
        program_revision: Some(3),
        beneficiary: beneficiary.clone(),
        asset_charges: BTreeMap::new(),
        window_charges: BTreeMap::new(),
        relay_lease_charges: BTreeMap::from([(lease_id, Quantity::from(6_u32))]),
        asset_remaining: BTreeMap::new(),
        window_remaining: BTreeMap::new(),
        relay_lease_remaining: BTreeMap::from([(lease_id, Quantity::from(10_u32))]),
    };
    let mut store = FeeAdmissionReservationStore::default();
    store
        .reserve(first_hash, reservation())
        .expect("first transaction reserves exact lease capacity");
    let err = store
        .reserve(second_hash, reservation())
        .expect_err("second transaction must not overbook the exact lease");
    assert!(matches!(
        err,
        Error::NexusFeeAdmissionRejected {
            code: FeeRejectionCode::RelayCapacityUnavailable,
            ..
        }
    ));
    store.release(&first_hash);
    store
        .reserve(second_hash, reservation())
        .expect("released lease capacity is immediately reusable");
}
#[test]
fn relay_spend_lease_reservation_maps_use_aggregate_per_asset_charges() {
    let (sponsor, _) = gen_account_in("relay_lease_map_sponsor");
    let program_id =
        FeeSponsorProgramId::new(sponsor, "relay_maps".parse().expect("valid program name"));
    let domain_id = DomainId::try_new("relay_lease_maps", "universal").expect("valid fee domain");
    let shared_asset = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "shared".parse().expect("valid shared asset name"),
    );
    let distinct_asset = AssetDefinitionId::derive_from_components(
        domain_id,
        "distinct".parse().expect("valid distinct asset name"),
    );
    let shared_lease = Hash::new(b"queue-shared-asset-spend-lease");
    let distinct_lease = Hash::new(b"queue-distinct-asset-spend-lease");
    let component_charges = [
        FeeChargeBound {
            kind: iroha_data_model::transaction::FeeChargeKind::Nexus,
            asset_definition_id: shared_asset.clone(),
            max_bound: Quantity::from(4_u32),
        },
        FeeChargeBound {
            kind: iroha_data_model::transaction::FeeChargeKind::PipelineGas,
            asset_definition_id: shared_asset.clone(),
            max_bound: Quantity::from(6_u32),
        },
    ];
    let sponsor_charges = sponsored_charge_totals(&component_charges)
        .expect("Nexus and PipelineGas charges aggregate by fee asset");
    assert_eq!(sponsor_charges[&shared_asset], Quantity::from(10_u32));
    let sponsor_charges = BTreeMap::from([
        (shared_asset.clone(), sponsor_charges[&shared_asset].clone()),
        (distinct_asset.clone(), Quantity::from(7_u32)),
    ]);
    let selections = BTreeMap::from([
        (
            shared_asset,
            FeeSponsorRelayLeaseCapacity {
                lease_id: shared_lease,
                remaining: Quantity::from(15_u32),
            },
        ),
        (
            distinct_asset,
            FeeSponsorRelayLeaseCapacity {
                lease_id: distinct_lease,
                remaining: Quantity::from(8_u32),
            },
        ),
    ]);
    let (charges, remaining) =
        relay_lease_reservation_maps(&program_id, &sponsor_charges, selections)
            .expect("each charged asset maps to its exact selected lease");
    assert_eq!(charges[&shared_lease], Quantity::from(10_u32));
    assert_eq!(charges[&distinct_lease], Quantity::from(7_u32));
    assert_eq!(remaining[&shared_lease], Quantity::from(15_u32));
    assert_eq!(remaining[&distinct_lease], Quantity::from(8_u32));
}
