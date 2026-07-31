// Focused adversarial coverage for terminal settlement/publication fencing.

#[test]
fn terminal_handoff_identity_is_chain_and_destination_bound() {
    let outcome_digest = [0x62; 32];
    let first = terminal_handoff_id(
        &ChainId::from("moderation-chain-a"),
        ModerationTerminalHandoffKindV1::Settlement,
        "case-1",
        "round-1",
        outcome_digest,
    );
    assert_eq!(
        first,
        terminal_handoff_id(
            &ChainId::from("moderation-chain-a"),
            ModerationTerminalHandoffKindV1::Settlement,
            "case-1",
            "round-1",
            outcome_digest,
        )
    );
    assert_ne!(
        first,
        terminal_handoff_id(
            &ChainId::from("moderation-chain-b"),
            ModerationTerminalHandoffKindV1::Settlement,
            "case-1",
            "round-1",
            outcome_digest,
        )
    );
    assert_ne!(
        first,
        terminal_handoff_id(
            &ChainId::from("moderation-chain-a"),
            ModerationTerminalHandoffKindV1::Publication,
            "case-1",
            "round-1",
            outcome_digest,
        )
    );
}

#[test]
fn terminal_handoff_policy_drift_after_delivery_is_ambiguous() {
    let inner = Arc::new(MockHandoffSink::default());
    let sink: Arc<dyn ModerationTerminalHandoffSinkV1> = Arc::new(DriftingHandoffSink {
        inner: Arc::clone(&inner),
        qualification_after_delivery: ModerationRuntimeProviderQualificationV1::new(2, [0xB3; 32]),
    });
    let qualified = QualifiedModerationTerminalHandoffSinkV1::try_new(
        HANDOFF_PROVIDER_HANDLE,
        HANDOFF_PROVIDER_QUALIFICATION,
        sink,
    )
    .expect("initially qualified handoff");
    let handoff = ModerationTerminalHandoffV1 {
        handoff_id: [0x61; 32],
        kind: ModerationTerminalHandoffKindV1::Settlement,
        case_id: "case-1".to_owned(),
        round_id: "round-1".to_owned(),
        outcome_digest: [0x62; 32],
        finalized_cursor: ModerationFinalizedCursorV1 {
            height: 11,
            block_hash: [0x63; 32],
        },
    };

    assert_eq!(
        qualified.deliver(&handoff),
        Err(ModerationHandoffFailureV1::Ambiguous)
    );
    assert_eq!(inner.calls(), 1);
    assert_eq!(inner.delivered(), vec![handoff.handoff_id]);
}

#[test]
fn terminal_handoffs_use_exact_finalization_block_across_later_tips() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let finalized = finalized_case_snapshot(
        activated_case_snapshot(2, [2; 32], governance.clone()),
        3,
        [3; 32],
        governance,
    );
    let mut later_tip = finalized.clone();
    later_tip.finalized_height = 4;
    later_tip.finalized_block_hash = [4; 32];
    later_tip.finalized_at_unix_ms = 62;

    let first = ModerationOrchestratorV1::open(
        config(&temp, "terminal-exact-cursor-a.norito"),
        deps(
            Arc::new(MockSnapshotReader::new(finalized)),
            Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
        ),
    )
    .expect("first orchestrator");
    let second = ModerationOrchestratorV1::open(
        config(&temp, "terminal-exact-cursor-b.norito"),
        deps(
            Arc::new(MockSnapshotReader::new(later_tip)),
            Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
        ),
    )
    .expect("second orchestrator");

    let queue = |orchestrator: &ModerationOrchestratorV1| {
        let (snapshot, digest) = orchestrator
            .read_validated_finalized_snapshot()
            .expect("read finalized snapshot");
        let mut state = orchestrator.state.lock().expect("orchestrator state");
        orchestrator
            .install_finalized_snapshot_locked(&mut state, snapshot, digest)
            .expect("queue terminal handoffs");
        state
            .pending_handoffs
            .iter()
            .map(|entry| entry.handoff.clone())
            .collect::<Vec<_>>()
    };
    let first_handoffs = queue(&first);
    let second_handoffs = queue(&second);

    assert_eq!(first_handoffs.len(), 2);
    assert_eq!(
        norito::to_bytes(&first_handoffs).expect("encode first handoffs"),
        norito::to_bytes(&second_handoffs).expect("encode second handoffs")
    );
    assert!(first_handoffs.iter().all(|handoff| {
        handoff.finalized_cursor
            == ModerationFinalizedCursorV1 {
                height: 3,
                block_hash: [3; 32],
            }
    }));
}

#[test]
fn cold_terminal_handoff_rebuild_requires_exact_retained_event() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let mut finalized = finalized_case_snapshot(
        activated_case_snapshot(2, [2; 32], governance.clone()),
        3,
        [3; 32],
        governance.clone(),
    );
    finalized.events[0].event = SorafsModerationLedgerEvent::new(
        SorafsModerationLedgerEventKind::PolicyActivated,
        None,
        None,
        governance,
        61,
    );
    let orchestrator = ModerationOrchestratorV1::open(
        config(&temp, "terminal-missing-event.norito"),
        deps(
            Arc::new(MockSnapshotReader::new(finalized)),
            Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
        ),
    )
    .expect("orchestrator");

    assert!(matches!(
        orchestrator.reconcile(),
        Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(message))
            if message.contains("no retained exact finalization event")
    ));
}

#[test]
fn checkpoint_and_pending_terminal_handoff_are_chain_fenced() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let finalized = finalized_case_snapshot(
        activated_case_snapshot(2, [2; 32], governance.clone()),
        3,
        [3; 32],
        governance,
    );
    let reader = Arc::new(MockSnapshotReader::new(finalized));
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
    let settlement = Arc::new(MockHandoffSink::default());
    let publication = Arc::new(MockHandoffSink::default());
    let checkpoint_store = Arc::new(MockCheckpointStore::default());
    let checkpoint = config(&temp, "terminal-chain-fence.norito");
    let runtime_deps = || ModerationOrchestratorDepsV1 {
        checkpoint_store: checkpoint_store.clone(),
        submitter: submitter.clone(),
        snapshot_reader: reader.clone(),
        settlement_sink: settlement.clone(),
        publication_sink: publication.clone(),
        panel_notification_sink: Arc::new(MockPanelNotificationSink::default()),
    };
    let orchestrator =
        ModerationOrchestratorV1::open(checkpoint.clone(), runtime_deps()).expect("open");
    let (snapshot, digest) = orchestrator
        .read_validated_finalized_snapshot()
        .expect("read finalized snapshot");
    {
        let mut state = orchestrator.state.lock().expect("orchestrator state");
        orchestrator
            .install_finalized_snapshot_locked(&mut state, snapshot, digest)
            .expect("queue terminal handoffs");
    }
    drop(orchestrator);

    let original = std::fs::read(&checkpoint.checkpoint_path).expect("read checkpoint");
    let limits = checkpoint_decode_limits(checkpoint.checkpoint_max_bytes).expect("decode limits");
    let mut transplanted =
        decode_from_bytes_with_limits::<ModerationOrchestratorCheckpointV1>(&original, limits)
            .expect("decode checkpoint");
    transplanted.chain_id = "different-moderation-chain".to_owned();
    std::fs::write(
        &checkpoint.checkpoint_path,
        norito::to_bytes(&transplanted).expect("encode transplanted checkpoint"),
    )
    .expect("write transplanted checkpoint");
    assert!(matches!(
        ModerationOrchestratorV1::open(checkpoint.clone(), runtime_deps()),
        Err(ModerationOrchestratorError::CheckpointCorrupt(message))
            if message.contains("chain binding")
    ));

    let mut substituted =
        decode_from_bytes_with_limits::<ModerationOrchestratorCheckpointV1>(&original, limits)
            .expect("decode checkpoint");
    substituted.pending_handoffs[0].handoff.handoff_id[0] ^= 0x80;
    std::fs::write(
        &checkpoint.checkpoint_path,
        norito::to_bytes(&substituted).expect("encode substituted checkpoint"),
    )
    .expect("write substituted checkpoint");
    assert!(matches!(
        ModerationOrchestratorV1::open(checkpoint, runtime_deps()),
        Err(ModerationOrchestratorError::CheckpointCorrupt(message))
            if message.contains("terminal handoff identity")
    ));
}

#[test]
fn terminal_finalization_source_provenance_mismatch_is_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let mut snapshot = finalized_case_snapshot(
        activated_case_snapshot(2, [2; 32], governance.clone()),
        3,
        [3; 32],
        governance,
    );
    snapshot.events[0].event = SorafsModerationLedgerEvent::new(
        SorafsModerationLedgerEventKind::CaseFinalized,
        Some("case-failover".to_owned()),
        Some("round-1".to_owned()),
        account(98),
        61,
    );
    let orchestrator = ModerationOrchestratorV1::open(
        config(&temp, "terminal-provenance.norito"),
        deps(
            Arc::new(MockSnapshotReader::new(snapshot)),
            Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
        ),
    )
    .expect("orchestrator");
    assert!(matches!(
        orchestrator.reconcile(),
        Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(message))
            if message.contains("finalization event provenance")
    ));
}
