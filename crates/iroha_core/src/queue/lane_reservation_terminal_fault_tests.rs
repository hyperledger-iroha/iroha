// Durable terminal-boundary and compaction-fault regression tests.
#[test]
fn ambiguous_terminal_reservation_appends_fail_closed_for_diagnostics_and_drain() {
    #[derive(Clone, Copy, Debug)]
    enum TerminalAppend {
        Release,
        OrderedRelease,
        PrepareRelease,
        CompleteRelease,
        Commit,
    }
    for terminal in [
        TerminalAppend::Release,
        TerminalAppend::OrderedRelease,
        TerminalAppend::PrepareRelease,
        TerminalAppend::CompleteRelease,
        TerminalAppend::Commit,
    ] {
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let state = lane_reservation_test_state();
        let dir = tempdir().expect("tempdir");
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        install_test_reservation_journal(&queue, &dir);
        for _ in 0..2 {
            push_globally_bound_lane_reservation_candidate(
                &queue,
                &state,
                &dir,
                accepted_queue_plan_tx_by_someone(&time_source),
            );
        }
        let scope = lane_reservation_scope(
            &state,
            format!("terminal-fault-owner-{terminal:?}").as_bytes(),
            format!("terminal-fault-proposal-{terminal:?}").as_bytes(),
        );
        let reserved = queue
            .reserve_transactions_for_lane(&state, scope, nonzero!(2_usize))
            .expect("reserve terminal-fault batch");
        let keys = reserved.iter().map(|tx| *tx.key()).collect::<Vec<_>>();
        let barrier = lane_reservation_release_barrier(
            keys.clone(),
            format!("terminal-fault-retirement-{terminal:?}").as_bytes(),
        );
        let unrelated_lane = LaneId::new(93);
        let unrelated_dataspace = DataSpaceId::new(93);
        let unrelated_incarnation =
            Hash::new(format!("terminal-fault-unrelated-{terminal:?}").as_bytes());
        assert!(
            !queue.lane_has_pending_work(
                unrelated_lane,
                unrelated_dataspace,
                unrelated_incarnation,
            ),
            "{terminal:?}: healthy unrelated route must initially be drainable"
        );
        let pressure_rx = queue.backpressure_handle().subscribe();
        assert!(
            !pressure_rx.borrow().is_saturated(),
            "{terminal:?}: pressure must be healthy before fault injection"
        );
        let inject_ambiguous_append = || {
            queue
                .lane_reservation_journal
                .lock()
                .as_mut()
                .expect("installed reservation journal")
                .inject_next_append_fault(ReservationJournalAppendFault::SyncAfterFullWrite);
        };
        let result = match terminal {
            TerminalAppend::Release => {
                inject_ambiguous_append();
                queue.release_lane_reservation(&keys[0]).map(|_| ())
            }
            TerminalAppend::OrderedRelease => {
                inject_ambiguous_append();
                queue.release_lane_reservations_in_order(&keys).map(|_| ())
            }
            TerminalAppend::PrepareRelease => {
                inject_ambiguous_append();
                queue
                    .prepare_lane_reservation_release_barrier(&barrier)
                    .map(|_| ())
            }
            TerminalAppend::CompleteRelease => {
                queue
                    .prepare_lane_reservation_release_barrier(&barrier)
                    .expect("prepare completion-fault release");
                inject_ambiguous_append();
                queue
                    .finalize_lane_reservation_release_barrier(&barrier)
                    .map(|_| ())
            }
            TerminalAppend::Commit => {
                inject_ambiguous_append();
                queue.commit_lane_reservation_for_test(&keys[0]).map(|_| ())
            }
        };
        assert!(
            matches!(result, Err(LaneQueueReservationError::Journal(_))),
            "{terminal:?}: ambiguous terminal append must report a journal error"
        );
        assert!(
            queue.lane_reservation_durability_faulted(),
            "{terminal:?}: terminal ambiguity must latch the process fault"
        );
        assert!(
            pressure_rx.borrow().is_saturated(),
            "{terminal:?}: terminal ambiguity must publish saturated pressure to existing observers"
        );
        assert!(
            !queue.lane_reservation_group_is_finalized_for_diagnostics(&keys),
            "{terminal:?}: diagnostics must fail closed after terminal ambiguity"
        );
        assert!(
            queue
                .lane_has_pending_work(unrelated_lane, unrelated_dataspace, unrelated_incarnation,),
            "{terminal:?}: terminal ambiguity must block even unrelated lane drain"
        );
    }
    #[derive(Clone, Copy, Debug)]
    enum StartupTerminalAppend {
        ForgetCommitAfterProof,
        ForgetReleaseAfterProof,
    }
    for terminal in [
        StartupTerminalAppend::ForgetCommitAfterProof,
        StartupTerminalAppend::ForgetReleaseAfterProof,
    ] {
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let state = lane_reservation_test_state();
        let dir = tempdir().expect("tempdir");
        let reservation_path = dir
            .path()
            .join(format!("startup-terminal-fault-{terminal:?}.norito"));
        let plan_path = test_lane_reservation_plan_path(&dir);
        let (keys, barrier) = {
            let queue = Arc::new(Queue::test(config_factory(), &time_source));
            queue
                .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
                .expect("install startup-terminal reservation journal");
            let transaction_count = match terminal {
                StartupTerminalAppend::ForgetCommitAfterProof => 1,
                StartupTerminalAppend::ForgetReleaseAfterProof => 2,
            };
            for _ in 0..transaction_count {
                push_globally_bound_lane_reservation_candidate(
                    &queue,
                    &state,
                    &dir,
                    accepted_queue_plan_tx_by_someone(&time_source),
                );
            }
            let scope = lane_reservation_scope(
                &state,
                format!("startup-terminal-owner-{terminal:?}").as_bytes(),
                format!("startup-terminal-proposal-{terminal:?}").as_bytes(),
            );
            let reserved = queue
                .reserve_transactions_for_lane(
                    &state,
                    scope,
                    NonZeroUsize::new(transaction_count)
                        .expect("startup terminal fixture is nonempty"),
                )
                .expect("reserve startup-terminal batch");
            let keys = reserved.iter().map(|tx| *tx.key()).collect::<Vec<_>>();
            let barrier = lane_reservation_release_barrier(
                keys.clone(),
                format!("startup-terminal-retirement-{terminal:?}").as_bytes(),
            );
            match terminal {
                StartupTerminalAppend::ForgetCommitAfterProof => {
                    queue
                        .lane_reservation_journal
                        .lock()
                        .as_mut()
                        .expect("installed reservation journal")
                        .commit(keys[0])
                        .expect("persist commit before startup-boundary crash");
                }
                StartupTerminalAppend::ForgetReleaseAfterProof => {
                    queue
                        .prepare_lane_reservation_release_barrier(&barrier)
                        .expect("persist prepared startup release");
                    let store = queue.lane_reservations.lock();
                    let ordered_records = barrier
                        .ordered_keys
                        .iter()
                        .map(|key| store.live_by_entrypoint[&key.entrypoint_hash].clone())
                        .collect();
                    drop(store);
                    queue
                        .lane_reservation_journal
                        .lock()
                        .as_mut()
                        .expect("installed reservation journal")
                        .complete_release(LaneQueueReservationReleaseCompletionV1 {
                            version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION,
                            barrier: barrier.clone(),
                            ordered_records,
                        })
                        .expect("persist release completion before startup-boundary crash");
                }
            }
            (keys, barrier)
        };
        if matches!(terminal, StartupTerminalAppend::ForgetCommitAfterProof) {
            let block_header =
                ValidBlock::new_dummy(&checked_random_queue_keypair().into_parts().1)
                    .as_ref()
                    .header();
            let mut state_block = state.block(block_header);
            state_block
                .transactions
                .insert_block(HashSet::from([keys[0].entrypoint_hash]), nonzero!(1_usize));
            state_block
                .commit()
                .expect("commit exact startup-terminal identity");
        }
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        let replay = queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("restore startup-terminal boundary before its plan journal");
        match terminal {
            StartupTerminalAppend::ForgetCommitAfterProof => {
                assert_eq!(keys.len(), 1);
                assert_eq!(replay.restored, 0);
                assert_eq!(replay.commit_barriers, 1);
                assert_eq!(replay.completed_releases, 0);
                assert!(queue.live_lane_reservations().is_empty());
                assert_eq!(
                    queue.lane_reservation_commit_barriers(),
                    vec![keys[0]],
                    "restart must reconstruct the exact commit identity that still needs a plan tombstone"
                );
            }
            StartupTerminalAppend::ForgetReleaseAfterProof => {
                assert_eq!(keys.len(), 2);
                assert_eq!(replay.restored, 0);
                assert_eq!(replay.commit_barriers, 0);
                assert_eq!(replay.completed_releases, 1);
                assert!(queue.live_lane_reservations().is_empty());
                assert_eq!(
                    queue.lane_reservation_release_barriers(),
                    vec![barrier.clone()],
                    "restart must reconstruct the exact completed release barrier"
                );
                let store = queue.lane_reservations.lock();
                assert_eq!(store.completed_releases.len(), 1);
                assert_eq!(store.completed_releases[0].barrier, barrier);
                assert_eq!(
                    store.completed_releases[0]
                        .ordered_records
                        .iter()
                        .map(|record| record.key)
                        .collect::<Vec<_>>(),
                    keys,
                    "completed release replay must retain the exact ordered reservation records"
                );
            }
        }
        let unrelated_lane = LaneId::new(95);
        let unrelated_dataspace = DataSpaceId::new(95);
        let unrelated_incarnation =
            Hash::new(format!("startup-terminal-unrelated-{terminal:?}").as_bytes());
        assert!(!queue.lane_has_pending_work(
            unrelated_lane,
            unrelated_dataspace,
            unrelated_incarnation,
        ));
        let pressure_rx = queue.backpressure_handle().subscribe();
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install exact production plan journal without finalizing barriers");
        queue
            .replay_plan_journal(&state)
            .expect("materialize exact payloads under durable quarantine");
        match terminal {
            StartupTerminalAppend::ForgetCommitAfterProof => assert_eq!(
                queue.lane_reservation_commit_barriers(),
                vec![keys[0]],
                "install and replay must retain the unproven Commit barrier"
            ),
            StartupTerminalAppend::ForgetReleaseAfterProof => assert_eq!(
                queue.lane_reservation_release_barriers(),
                vec![barrier.clone()],
                "install and replay must retain the unproven completed-release barrier"
            ),
        }
        queue
            .lane_reservation_journal
            .lock()
            .as_mut()
            .expect("restored reservation journal")
            .inject_next_append_fault(ReservationJournalAppendFault::SyncAfterFullWrite);
        let error = match terminal {
            StartupTerminalAppend::ForgetCommitAfterProof => queue
                .commit_lane_reservation_for_test(&keys[0])
                .map(|_| ())
                .expect_err("ambiguous ForgetCommit must fail the explicit proof path"),
            StartupTerminalAppend::ForgetReleaseAfterProof => queue
                .finalize_lane_reservation_release_barrier(&barrier)
                .map(|_| ())
                .expect_err("ambiguous ForgetRelease must fail the explicit proof path"),
        };
        assert!(
            error.to_string().contains("lane queue reservation journal"),
            "{terminal:?}: startup error must identify the ambiguous reservation boundary: {error}"
        );
        assert!(
            queue.lane_reservation_durability_faulted(),
            "{terminal:?}: ambiguous startup Forget must latch the process fault"
        );
        assert!(
            pressure_rx.borrow().is_saturated(),
            "{terminal:?}: ambiguous startup Forget must publish saturated pressure"
        );
        assert!(
            !queue.lane_reservation_group_is_finalized_for_diagnostics(&keys),
            "{terminal:?}: diagnostics must fail closed after startup Forget ambiguity"
        );
        assert!(
            queue
                .lane_has_pending_work(unrelated_lane, unrelated_dataspace, unrelated_incarnation,),
            "{terminal:?}: startup Forget ambiguity must block unrelated lane drain"
        );
        match terminal {
            StartupTerminalAppend::ForgetCommitAfterProof => assert_eq!(
                queue.lane_reservation_commit_barriers(),
                vec![keys[0]],
                "ambiguous ForgetCommit must retain the exact affected identity"
            ),
            StartupTerminalAppend::ForgetReleaseAfterProof => {
                let store = queue.lane_reservations.lock();
                assert_eq!(store.completed_releases.len(), 1);
                assert_eq!(store.completed_releases[0].barrier, barrier);
                assert_eq!(
                    store.completed_releases[0]
                        .ordered_records
                        .iter()
                        .map(|record| record.key)
                        .collect::<Vec<_>>(),
                    keys,
                    "ambiguous ForgetRelease must retain the exact affected ordered group"
                );
            }
        }
    }
}
#[test]
fn ambiguous_reservation_compaction_fails_closed_after_terminal_application() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("ambiguous-compaction.norito");
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    queue
        .install_plan_journal(
            dir.path().join("ambiguous-compaction-plans.norito"),
            1024 * 1024,
            true,
        )
        .expect("install globally certified reservation plan journal");
    queue
        .install_lane_reservation_journal(&path, 1)
        .expect("install aggressively compacting reservation journal");
    push_globally_bound_lane_reservation_candidate(
        &queue,
        &state,
        &dir,
        accepted_queue_plan_tx_by_someone(&time_source),
    );
    let key = *queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(
                &state,
                b"compaction-fault-owner",
                b"compaction-fault-proposal",
            ),
            nonzero!(1_usize),
        )
        .expect("reserve compaction-fault transaction")[0]
        .key();
    let unrelated_lane = LaneId::new(94);
    let unrelated_dataspace = DataSpaceId::new(94);
    let unrelated_incarnation = Hash::new(b"compaction-fault-unrelated-incarnation");
    assert!(!queue.lane_has_pending_work(
        unrelated_lane,
        unrelated_dataspace,
        unrelated_incarnation,
    ));
    let pressure_rx = queue.backpressure_handle().subscribe();
    assert!(
        !pressure_rx.borrow().is_saturated(),
        "pressure must be healthy before compaction fault injection"
    );
    queue
        .lane_reservation_journal
        .lock()
        .as_mut()
        .expect("installed reservation journal")
        .inject_next_compaction_fault(
            ReservationJournalCompactionFault::AfterRenameBeforeParentSync,
        );
    assert_eq!(
        queue
            .release_lane_reservation(&key)
            .expect("terminal release remains durably applied before compaction"),
        LaneQueueReservationOutcome::Finalized
    );
    assert!(queue.live_lane_reservations().is_empty());
    assert!(queue.lane_reservation_commit_barriers().is_empty());
    assert!(queue.lane_reservation_release_barriers().is_empty());
    assert!(
        queue.lane_reservation_durability_faulted(),
        "rename-before-parent-sync ambiguity must latch the process fault"
    );
    assert!(
        pressure_rx.borrow().is_saturated(),
        "compaction ambiguity must publish saturated pressure to existing observers"
    );
    assert!(
        !queue.lane_reservation_group_is_finalized_for_diagnostics(&[key]),
        "a fully applied terminal transition must still fail diagnostics closed while its compacted replacement is ambiguous"
    );
    assert!(
        queue.lane_has_pending_work(unrelated_lane, unrelated_dataspace, unrelated_incarnation,),
        "compaction ambiguity must block even unrelated lane drain"
    );
    let mut selected = Vec::new();
    queue.get_transactions_for_block_with_state(&state, nonzero!(1_usize), &mut selected);
    assert!(
        selected.is_empty(),
        "the FIFO-restored transaction must remain nonselectable until restart recovery"
    );
}
