// Stale replicas must recover authoritative state before exact operation replay.

fn pending_signed_bytes(
    orchestrator: &ModerationOrchestratorV1,
    operation_id: [u8; 32],
) -> Vec<u8> {
    orchestrator
        .state
        .lock()
        .expect("checkpoint state")
        .outbox
        .iter()
        .find(|entry| entry.operation_id == operation_id)
        .expect("retained pending operation")
        .signed_transaction_bytes
        .clone()
        .expect("retained exact signed transaction")
}

#[test]
fn duplicate_cross_replica_submission_reuses_one_transaction() {
    orchestrator_fixture!(first; temp = tempfile::tempdir().expect("tempdir"); reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32]))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound { observed_finalized_height: 1 })); => config(&temp, "first.norito"); deps(Arc::clone(&reader), Arc::clone(&submitter)); "first orchestrator");
    let second_checkpoint = config(&temp, "second.norito");
    open_test_orchestrator!(second = second_checkpoint.clone(); deps(reader.clone(), Arc::clone(&submitter)); "second orchestrator");
    let authority = account(1);
    let action = policy_action(policy(1));
    let first_outcome = first
        .submit(authority.clone(), action.clone(), [0x11; 32])
        .expect("first submit");
    let exact_bytes = pending_signed_bytes(&first, first_outcome.operation_id);
    let committed = reader.checkpoint_store.latest();
    assert_eq!(
        second.submit(authority.clone(), action.clone(), [0x22; 32]),
        Err(ModerationOrchestratorError::CheckpointStoreFenced)
    );
    assert_eq!(
        second.reconcile(),
        Err(ModerationOrchestratorError::DurabilityFaulted)
    );
    assert_eq!(reader.checkpoint_store.latest(), committed);
    assert_eq!(submitter.calls(), 1);
    assert_eq!(submitter.sign_calls(), 1);
    drop(second);
    open_test_orchestrator!(second = second_checkpoint; deps(reader, Arc::clone(&submitter)); "recover second from authoritative checkpoint");
    assert_eq!(
        pending_signed_bytes(&second, first_outcome.operation_id),
        exact_bytes
    );
    let second_outcome = second
        .submit(authority, action, [0x22; 32])
        .expect("second submit");
    assert_eq!(submitter.calls(), 1);
    assert_eq!(submitter.sign_calls(), 1);
    assert!(second_outcome.replay);
    assert_eq!(first_outcome.operation_id, second_outcome.operation_id);
    assert_eq!(first_outcome.transaction_id, second_outcome.transaction_id);
    assert_eq!(
        pending_signed_bytes(&second, second_outcome.operation_id),
        exact_bytes
    );
}

#[test]
fn terminal_finalization_converges_after_restart_and_split_peer_replay() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let open_snapshot = activated_case_snapshot(2, [2; 32], governance.clone());
    let finalized_snapshot =
        finalized_case_snapshot(open_snapshot.clone(), 3, [3; 32], governance.clone());
    let reader = Arc::new(MockSnapshotReader::new(open_snapshot));
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
        observed_finalized_height: 2,
    }));
    let settlement_sink = Arc::new(MockHandoffSink::default());
    let publication_sink = Arc::new(MockHandoffSink::default());
    let runtime_deps = || test_runtime_deps!(reader.checkpoint_store.clone(); submitter.clone(); reader.clone(); settlement_sink.clone(); publication_sink.clone(); Arc::new(MockPanelNotificationSink::default()); Arc::new(MockPanelNotificationArchive::default()));
    let first_checkpoint = config(&temp, "terminal-first.norito");
    let second_checkpoint = config(&temp, "terminal-second.norito");
    let action = ModerationNativeActionV1::FinalizeCase(FinalizeSorafsModerationCase::new(
        "case-failover".to_owned(),
        "round-1".to_owned(),
    ));
    open_test_orchestrator!(first = first_checkpoint.clone(); runtime_deps(); "first orchestrator");
    open_test_orchestrator!(second = second_checkpoint.clone(); runtime_deps(); "second orchestrator");
    let first_submit = first
        .submit(governance.clone(), action.clone(), [0x11; 32])
        .expect("first terminal submit");
    let exact_bytes = pending_signed_bytes(&first, first_submit.operation_id);
    let committed = reader.checkpoint_store.latest();
    assert_eq!(
        second.submit(governance.clone(), action.clone(), [0x22; 32]),
        Err(ModerationOrchestratorError::CheckpointStoreFenced)
    );
    assert_eq!(
        second.reconcile(),
        Err(ModerationOrchestratorError::DurabilityFaulted)
    );
    assert_eq!(reader.checkpoint_store.latest(), committed);
    assert_eq!(submitter.calls(), 1);
    assert_eq!(submitter.sign_calls(), 1);
    assert_eq!(settlement_sink.calls(), 0);
    assert_eq!(publication_sink.calls(), 0);
    drop(second);
    open_test_orchestrator!(second = second_checkpoint.clone(); runtime_deps(); "recover split peer from authoritative checkpoint");
    assert_eq!(
        pending_signed_bytes(&second, first_submit.operation_id),
        exact_bytes
    );
    let split_peer_submit = second
        .submit(governance.clone(), action.clone(), [0x22; 32])
        .expect("split-peer terminal replay");
    assert_eq!(first_submit.status, ModerationOperationStatusV1::Pending);
    assert_eq!(
        split_peer_submit.status,
        ModerationOperationStatusV1::Pending
    );
    assert_eq!(first_submit.operation_id, split_peer_submit.operation_id);
    assert_eq!(
        first_submit.transaction_id,
        split_peer_submit.transaction_id
    );
    assert_eq!(submitter.calls(), 1);
    assert_eq!(submitter.sign_calls(), 1);
    assert!(split_peer_submit.replay);
    assert_eq!(
        pending_signed_bytes(&second, first_submit.operation_id),
        exact_bytes
    );
    drop(first);
    reader.replace(finalized_snapshot);
    open_test_orchestrator!(restarted = first_checkpoint.clone(); runtime_deps(); "restarted orchestrator");
    restarted
        .reconcile()
        .expect("restart reconciles finalized case");
    let finalized_record = reader.checkpoint_store.latest();
    assert_eq!(
        second.reconcile(),
        Err(ModerationOrchestratorError::CheckpointStoreFenced)
    );
    assert_eq!(
        second.reconcile(),
        Err(ModerationOrchestratorError::DurabilityFaulted)
    );
    assert_eq!(reader.checkpoint_store.latest(), finalized_record);
    assert_eq!(settlement_sink.calls(), 1);
    assert_eq!(publication_sink.calls(), 1);
    let restarted_replay = restarted
        .submit(governance.clone(), action.clone(), [0x11; 32])
        .expect("restarted finalized replay");
    drop(second);
    open_test_orchestrator!(second = second_checkpoint.clone(); runtime_deps(); "recover terminal split peer");
    let split_peer_replay = second
        .submit(governance, action, [0x22; 32])
        .expect("split-peer finalized replay");
    assert_eq!(
        restarted_replay.status,
        ModerationOperationStatusV1::Finalized
    );
    assert_eq!(
        split_peer_replay.status,
        ModerationOperationStatusV1::Finalized
    );
    assert!(restarted_replay.replay);
    assert!(split_peer_replay.replay);
    assert_eq!(submitter.calls(), 1);
    assert_eq!(submitter.sign_calls(), 1);
    let restarted_case = restarted
        .case("case-failover", "round-1")
        .expect("restarted case projection");
    let split_peer_case = second
        .case("case-failover", "round-1")
        .expect("split-peer case projection");
    assert!(restarted_case.outcome.is_some());
    assert_eq!(
        norito::to_bytes(&restarted_case).expect("encode restarted projection"),
        norito::to_bytes(&split_peer_case).expect("encode split-peer projection")
    );
    assert_eq!(settlement_sink.delivered().len(), 1);
    assert_eq!(publication_sink.delivered().len(), 1);
    drop(restarted);
    open_test_orchestrator!(restarted = first_checkpoint; runtime_deps(); "recover latest terminal checkpoint for idempotent reconcile");
    restarted.reconcile().expect("idempotent restart reconcile");
    drop(second);
    open_test_orchestrator!(second = second_checkpoint; runtime_deps(); "recover latest split-peer checkpoint for idempotent reconcile");
    second.reconcile().expect("idempotent split-peer reconcile");
    assert_eq!(settlement_sink.delivered().len(), 1);
    assert_eq!(publication_sink.delivered().len(), 1);
    assert_eq!(settlement_sink.calls(), 1);
    assert_eq!(publication_sink.calls(), 1);
}
