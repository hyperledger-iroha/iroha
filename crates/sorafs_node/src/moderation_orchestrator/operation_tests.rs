#[test]
fn duplicate_cross_replica_submission_reuses_one_transaction() {
    orchestrator_fixture!(first; temp = tempfile::tempdir().expect("tempdir"); reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32]))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound { observed_finalized_height: 1 })); => config(&temp, "first.norito"); deps(Arc::clone(&reader), Arc::clone(&submitter)); "first orchestrator");
    open_test_orchestrator!(second = config(&temp, "second.norito"); deps(reader, Arc::clone(&submitter)); "second orchestrator");
    let authority = account(1);
    let action = policy_action(policy(1));
    let first_outcome = first
        .submit(authority.clone(), action.clone(), [0x11; 32])
        .expect("first submit");
    let second_outcome = second
        .submit(authority, action, [0x22; 32])
        .expect("second submit");
    assert_eq!(submitter.calls(), 1);
    assert_eq!(first_outcome.operation_id, second_outcome.operation_id);
    assert_eq!(first_outcome.transaction_id, second_outcome.transaction_id);
}
#[test]
fn finalize_sortition_rejects_non_policy_authority_without_mutation() {
    let governance = account(90);
    let imposter = account(91);
    let action =
        ModerationNativeActionV1::FinalizeSortition(FinalizeSorafsModerationSortition::new(
            "case-authority".to_owned(),
            "round-1".to_owned(),
            [0x31; 32],
            [0x32; 32],
            Vec::new(),
            Vec::new(),
        ));
    assert_finalized_authority_rejection_has_no_native_mutation(
        snapshot_with_policy(1, [1; 32], policy(1), governance.clone()),
        imposter,
        &governance,
        action,
    );
}
#[test]
fn selection_governed_actions_reject_non_selected_authority_without_mutation() {
    let governance = account(90);
    let imposter = account(91);
    let (awaiting, sortition_digest) = awaiting_acceptance_snapshot(2, [2; 32], governance.clone());
    assert_finalized_authority_rejection_has_no_native_mutation(
        awaiting,
        imposter.clone(),
        &governance,
        ModerationNativeActionV1::ActivateCase(ActivateSorafsModerationCase::new(
            "case-failover".to_owned(),
            "round-1".to_owned(),
            sortition_digest,
        )),
    );
    for action in [
        ModerationNativeActionV1::ResolveChallenge(ResolveSorafsModerationChallenge::new(
            "case-failover".to_owned(),
            "round-1".to_owned(),
            "challenge-authority".to_owned(),
            ModerationChallengeDecisionV1::Rejected,
        )),
        ModerationNativeActionV1::FinalizeCase(FinalizeSorafsModerationCase::new(
            "case-failover".to_owned(),
            "round-1".to_owned(),
        )),
    ] {
        assert_finalized_authority_rejection_has_no_native_mutation(
            activated_case_snapshot(2, [2; 32], governance.clone()),
            imposter.clone(),
            &governance,
            action,
        );
    }
}
#[test]
fn historical_operation_replay_precedes_rotated_finalized_authority() {
    let temp = tempfile::tempdir().expect("tempdir");
    let original_governance = account(90);
    let rotated_governance = account(91);
    orchestrator_fixture!(orchestrator; reader = Arc::new(MockSnapshotReader::new(snapshot_with_policy(1, [1; 32], policy(1), original_governance.clone()))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound { observed_finalized_height: 1 })); => config(&temp, "authority-replay.norito"); deps(Arc::clone(&reader), Arc::clone(&submitter)); "orchestrator");
    let action =
        ModerationNativeActionV1::FinalizeSortition(FinalizeSorafsModerationSortition::new(
            "case-authority-replay".to_owned(),
            "round-1".to_owned(),
            [0x41; 32],
            [0x42; 32],
            Vec::new(),
            Vec::new(),
        ));
    let first = orchestrator
        .submit(original_governance.clone(), action.clone(), [0xA1; 32])
        .expect("initial submission");
    assert!(!first.replay);
    assert_eq!(submitter.calls(), 1);
    let mut rotated_snapshot = snapshot_with_policy(2, [2; 32], policy(2), rotated_governance);
    rotated_snapshot.events[0].sequence = 2;
    reader.replace(rotated_snapshot);
    let replay = orchestrator
        .submit(original_governance, action, [0xA1; 32])
        .expect("retained historical replay");
    assert!(replay.replay);
    assert_eq!(replay.operation_id, first.operation_id);
    assert_eq!(submitter.calls(), 1);
}
#[test]
fn same_semantic_identity_with_different_action_is_rejected() {
    orchestrator_fixture!(orchestrator; temp = tempfile::tempdir().expect("tempdir"); reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32]))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound { observed_finalized_height: 1 })); => config(&temp, "checkpoint.norito"); deps(reader, submitter); "orchestrator");
    let authority = account(1);
    orchestrator
        .submit(authority.clone(), policy_action(policy(1)), [0x11; 32])
        .expect("first submit");
    let mut conflicting = policy(1);
    conflicting.missing_commit_penalty_points = 11;
    let error = orchestrator
        .submit(authority, policy_action(conflicting), [0x22; 32])
        .expect_err("conflicting replay must fail");
    assert!(matches!(
        error,
        ModerationOrchestratorError::IdempotencyConflict { .. }
    ));
}
#[test]
fn stale_and_equivocating_finalized_cursors_fail_closed() {
    orchestrator_fixture!(orchestrator; temp = tempfile::tempdir().expect("tempdir"); reader = Arc::new(MockSnapshotReader::new(empty_snapshot(2, [2; 32]))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound { observed_finalized_height: 2 })); => config(&temp, "checkpoint.norito"); deps(Arc::clone(&reader), submitter); "orchestrator");
    orchestrator.reconcile().expect("initial reconcile");
    reader.replace(empty_snapshot(1, [1; 32]));
    assert!(matches!(
        orchestrator.reconcile(),
        Err(ModerationOrchestratorError::StaleFinalizedCursor { .. })
    ));
    reader.replace(empty_snapshot(2, [9; 32]));
    assert!(matches!(
        orchestrator.reconcile(),
        Err(ModerationOrchestratorError::FinalizedEquivocation { .. })
    ));
}
#[test]
fn every_external_collaborator_is_reentrant_without_holding_the_state_mutex() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
        observed_finalized_height: 1,
    }));
    let settlement = Arc::new(MockHandoffSink::default());
    let publication = Arc::new(MockHandoffSink::default());
    let probe = Arc::new(ReentrantLockProbe::default());
    let orchestrator = Arc::new(
        open_test_orchestrator!(config(&temp, "reentrant-collaborators.norito"); test_runtime_deps!(Arc::new(MockCheckpointStore::default()); Arc::new(ProbedSubmitter { inner: Arc::clone(&submitter), probe: Arc::clone(&probe) }); Arc::new(ProbedSnapshotReader { inner: Arc::clone(&reader), probe: Arc::clone(&probe) }); Arc::new(ProbedHandoffSink { inner: Arc::clone(&settlement), probe: Arc::clone(&probe) }); Arc::new(ProbedHandoffSink { inner: Arc::clone(&publication), probe: Arc::clone(&probe) }); Arc::new(MockPanelNotificationSink::default()); Arc::new(MockPanelNotificationArchive::default())); "orchestrator"),
    );
    probe.attach(&orchestrator);
    orchestrator
        .submit(account(1), policy_action(policy(1)), [0x45; 32])
        .expect("sign and submit outside the mutex");
    orchestrator.reconcile().expect("lookup outside the mutex");
    let governance = account(99);
    let open = activated_case_snapshot(2, [2; 32], governance.clone());
    reader.replace(finalized_case_snapshot(open, 3, [3; 32], governance));
    orchestrator
        .reconcile()
        .expect("terminal sinks outside the mutex");
    assert!(probe.checks() >= 6);
    assert_eq!(settlement.delivered().len(), 1);
    assert_eq!(publication.delivered().len(), 1);
}
#[test]
fn blocking_signer_claim_allows_concurrent_duplicate_worker_to_exit() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
    let inner = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
        observed_finalized_height: 1,
    }));
    let (entered_tx, entered_rx) = mpsc::channel();
    let blocking = Arc::new(BlockingSignSubmitter::new(Arc::clone(&inner), entered_tx));
    let orchestrator = Arc::new(
        open_test_orchestrator!(config(&temp, "blocking-duplicate-workers.norito"); test_runtime_deps!(Arc::new(MockCheckpointStore::default()); blocking.clone(); reader; Arc::new(MockHandoffSink::default()); Arc::new(MockHandoffSink::default()); Arc::new(MockPanelNotificationSink::default()); Arc::new(MockPanelNotificationArchive::default())); "orchestrator"),
    );
    seed_default_operation!(orchestrator; [0x46; 32]);
    let first = {
        let orchestrator = Arc::clone(&orchestrator);
        thread::spawn(move || orchestrator.drive_external_work())
    };
    entered_rx
        .recv_timeout(core::time::Duration::from_secs(5))
        .expect("signer entered");
    let lock_was_free = orchestrator.state.try_lock().is_ok();
    let (duplicate_tx, duplicate_rx) = mpsc::channel();
    let duplicate = {
        let orchestrator = Arc::clone(&orchestrator);
        thread::spawn(move || {
            let result = orchestrator.drive_external_work();
            duplicate_tx.send(result).expect("signal duplicate worker");
        })
    };
    let duplicate_result = duplicate_rx.recv_timeout(core::time::Duration::from_secs(5));
    blocking.release();
    first
        .join()
        .expect("first worker thread")
        .expect("first worker finishes");
    duplicate.join().expect("duplicate worker thread");
    assert!(lock_was_free);
    duplicate_result
        .expect("duplicate worker exits while signer is blocked")
        .expect("duplicate worker exits without duplicate work");
    assert_eq!(inner.sign_calls(), 1);
    assert_eq!(inner.calls(), 1);
}
#[test]
fn generic_signed_envelope_contract_rejects_network_ttl_nonce_metadata_and_action_substitution() {
    let network_id = test_network_id();
    let authority = account(1);
    let action = policy_action(policy(1));
    let request = ModerationTransactionRequestV1::new(
        network_id,
        1,
        authority.clone(),
        action.clone(),
        [0x71; 32],
        7,
        [0x72; 32],
    )
    .expect("canonical generic request");
    let other_network_request = ModerationTransactionRequestV1::new(
        iroha_data_model::NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"other-moderation-genesis")),
        ),
        1,
        authority.clone(),
        action.clone(),
        [0x71; 32],
        7,
        [0x72; 32],
    )
    .expect("canonical cross-network request");
    assert_ne!(request.operation_id, other_network_request.operation_id);
    let next_generation_request = ModerationTransactionRequestV1::new(
        network_id,
        2,
        authority.clone(),
        action.clone(),
        [0x71; 32],
        8,
        [0x73; 32],
    )
    .expect("canonical next-generation request");
    assert_eq!(request.operation_id, next_generation_request.operation_id);
    let mut zero_generation_request = request.clone();
    zero_generation_request.envelope_generation = 0;
    assert!(matches!(
        zero_generation_request.validate(),
        Err(ModerationOrchestratorError::InvalidAction(message))
            if message.contains("generation")
    ));
    let signer = key_for_authority(&authority);
    let exact_builder = || {
        TransactionBuilder::new(
            request.network_id,
            authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
    };
    let sign_exact = |mut builder: TransactionBuilder, instruction: InstructionBox| {
        builder.set_ttl(core::time::Duration::from_millis(
            MODERATION_TRANSACTION_TTL_MS_V1,
        ));
        builder
            .with_instructions([instruction])
            .sign(signer.private_key())
    };
    let exact = sign_exact(exact_builder(), action.instruction());
    ModerationSignedTransactionV1::from_signed_transaction(&request, &exact)
        .expect("exact generic signed envelope");
    let wrong_network = sign_exact(
        TransactionBuilder::new(
            iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                iroha_data_model::block::BlockHeader,
            >::from_untyped_unchecked(
                iroha_crypto::Hash::new(b"other-moderation-network"),
            )),
            authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        ),
        action.instruction(),
    );
    assert_eq!(
        ModerationSignedTransactionV1::from_signed_transaction(&request, &wrong_network),
        Err(ModerationSubmissionFailureV1::PermanentRejection),
    );
    let mut wrong_ttl_builder = exact_builder();
    wrong_ttl_builder.set_ttl(core::time::Duration::from_millis(
        MODERATION_TRANSACTION_TTL_MS_V1 + 1,
    ));
    let wrong_ttl = wrong_ttl_builder
        .with_instructions([action.instruction()])
        .sign(signer.private_key());
    assert_eq!(
        ModerationSignedTransactionV1::from_signed_transaction(&request, &wrong_ttl),
        Err(ModerationSubmissionFailureV1::PermanentRejection),
    );
    let mut nonce_builder = exact_builder();
    nonce_builder.set_ttl(core::time::Duration::from_millis(
        MODERATION_TRANSACTION_TTL_MS_V1,
    ));
    nonce_builder.set_nonce(NonZeroU32::new(9).expect("non-zero nonce"));
    let with_nonce = nonce_builder
        .with_instructions([action.instruction()])
        .sign(signer.private_key());
    assert_eq!(
        ModerationSignedTransactionV1::from_signed_transaction(&request, &with_nonce),
        Err(ModerationSubmissionFailureV1::PermanentRejection),
    );
    let mut metadata = Metadata::default();
    metadata.insert(
        "moderation_action_hint"
            .parse()
            .expect("valid metadata key"),
        Json::new("set_policy".to_owned()),
    );
    let with_metadata = sign_exact(
        exact_builder().with_metadata(metadata),
        action.instruction(),
    );
    assert_eq!(
        ModerationSignedTransactionV1::from_signed_transaction(&request, &with_metadata),
        Err(ModerationSubmissionFailureV1::PermanentRejection),
    );
    let substituted_action = ModerationNativeActionV1::FinalizeCase(
        FinalizeSorafsModerationCase::new("case-substitute".to_owned(), "round-1".to_owned()),
    );
    let substituted = sign_exact(exact_builder(), substituted_action.instruction());
    assert_eq!(
        ModerationSignedTransactionV1::from_signed_transaction(&request, &substituted),
        Err(ModerationSubmissionFailureV1::PermanentRejection),
    );
}
#[test]
fn expired_finalized_not_found_renews_one_generation_and_preserves_history() {
    orchestrator_fixture!(orchestrator; temp = tempfile::tempdir().expect("tempdir"); reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32]))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound { observed_finalized_height: 1 })); => config(&temp, "renew-expired.norito"); deps(Arc::clone(&reader), Arc::clone(&submitter)); "orchestrator");
    orchestrator
        .submit(account(1), policy_action(policy(1)), [0x81; 32])
        .expect("initial submission");
    let (operation_id, generation, first_signed, first_timing, state) =
        retained_envelope(&orchestrator);
    assert_eq!(generation, 1);
    assert_eq!(state, StoredOutboxStateV1::Submitted);
    submitter.set_lookup(
        operation_id,
        first_signed.transaction_id,
        ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 2,
        },
    );
    reader.replace(empty_snapshot_at(
        2,
        [2; 32],
        first_timing.expires_at_unix_ms,
    ));
    orchestrator
        .reconcile()
        .expect("renew after exact finalized absence");
    let (_, generation, second_signed, _, state) = retained_envelope(&orchestrator);
    assert_eq!(generation, 2);
    assert_eq!(state, StoredOutboxStateV1::Submitted);
    assert_ne!(second_signed.transaction_id, first_signed.transaction_id);
    assert_ne!(
        second_signed.canonical_bytes_digest,
        first_signed.canonical_bytes_digest
    );
    assert_eq!(submitter.sign_calls(), 2);
    assert_eq!(submitter.calls(), 2);
    single_outbox_entry!(state, entry = orchestrator; "orchestrator state"; "one renewed outbox entry");
    let [retired] = entry.retired_envelopes.as_slice() else {
        panic!("one retired envelope");
    };
    assert_eq!(retired.generation, 1);
    assert_eq!(retired.transaction_id, first_signed.transaction_id);
    assert_eq!(
        retired.signed_transaction_digest,
        first_signed.canonical_bytes_digest
    );
    assert_eq!(
        retired.disposition,
        StoredRetiredEnvelopeDispositionV1::NotFound
    );
    assert_eq!(
        retired.record_digest,
        retired_envelope_record_digest(operation_id, retired)
    );
}
#[test]
fn expired_envelope_does_not_renew_for_positive_unknown_rejected_or_stale_absence() {
    let scenarios = [
        (
            "pending",
            ModerationSubmissionLookupV1::Pending {
                transaction_id: [0; 32],
            },
        ),
        (
            "applied",
            ModerationSubmissionLookupV1::Applied {
                transaction_id: [0; 32],
            },
        ),
        ("unknown", ModerationSubmissionLookupV1::Unknown),
        (
            "rejected",
            ModerationSubmissionLookupV1::Rejected {
                transaction_id: Some([0; 32]),
                observed_finalized_height: 2,
            },
        ),
        (
            "stale-not-found",
            ModerationSubmissionLookupV1::NotFound {
                observed_finalized_height: 2,
            },
        ),
    ];
    for (index, (label, lookup)) in scenarios.into_iter().enumerate() {
        orchestrator_fixture!(orchestrator; temp = tempfile::tempdir().expect("tempdir"); reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32]))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)); => config(&temp, &format!("no-renew-{label}.norito")); deps(Arc::clone(&reader), Arc::clone(&submitter)); "orchestrator");
        orchestrator
            .submit(
                account(1),
                policy_action(policy(1)),
                [u8::try_from(index).unwrap_or(0).saturating_add(1); 32],
            )
            .expect("initial submission");
        let (operation_id, _, signed, timing, _) = retained_envelope(&orchestrator);
        let exact_lookup = match lookup {
            ModerationSubmissionLookupV1::Pending { .. } => ModerationSubmissionLookupV1::Pending {
                transaction_id: signed.transaction_id,
            },
            ModerationSubmissionLookupV1::Applied { .. } => ModerationSubmissionLookupV1::Applied {
                transaction_id: signed.transaction_id,
            },
            ModerationSubmissionLookupV1::Rejected {
                observed_finalized_height,
                ..
            } => ModerationSubmissionLookupV1::Rejected {
                transaction_id: Some(signed.transaction_id),
                observed_finalized_height,
            },
            other => other,
        };
        submitter.set_lookup(operation_id, signed.transaction_id, exact_lookup);
        let finalized_height = if label == "stale-not-found" { 3 } else { 2 };
        reader.replace(empty_snapshot_at(
            finalized_height,
            [u8::try_from(finalized_height).unwrap_or(9); 32],
            timing.expires_at_unix_ms.saturating_add(1),
        ));
        orchestrator
            .reconcile()
            .expect("expired non-renewal reconciliation");
        assert_eq!(submitter.sign_calls(), 1, "{label}");
        let state = orchestrator.state.lock().expect("orchestrator state");
        if label == "rejected" {
            assert!(state.outbox.is_empty(), "{label}");
            assert_eq!(
                state.operations[0].status,
                StoredOperationStatusV1::Rejected
            );
        } else {
            let [entry] = state.outbox.as_slice() else {
                panic!("{label}: one retained envelope");
            };
            assert_eq!(entry.envelope_generation, 1, "{label}");
            assert!(entry.retired_envelopes.is_empty(), "{label}");
            if matches!(label, "unknown" | "stale-not-found") {
                assert_eq!(entry.state, StoredOutboxStateV1::Ambiguous, "{label}");
            }
        }
    }
}
#[test]
fn ambiguous_submission_never_renews_without_exact_finalized_absence() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
    submitter.set_failure(Some(ModerationSubmissionFailureV1::Ambiguous));
    open_test_orchestrator!(orchestrator = config(&temp, "ambiguous-no-renew.norito"); deps(Arc::clone(&reader), Arc::clone(&submitter)); "orchestrator");
    orchestrator
        .submit(account(1), policy_action(policy(1)), [0x82; 32])
        .expect("ambiguous submission retained");
    let (_, _, _, timing, state) = retained_envelope(&orchestrator);
    assert_eq!(state, StoredOutboxStateV1::Ambiguous);
    reader.replace(empty_snapshot_at(
        2,
        [2; 32],
        timing.expires_at_unix_ms.saturating_add(1),
    ));
    orchestrator
        .reconcile()
        .expect("unknown lookup remains ambiguous");
    let (_, generation, _, _, state) = retained_envelope(&orchestrator);
    assert_eq!(generation, 1);
    assert_eq!(state, StoredOutboxStateV1::Ambiguous);
    assert_eq!(submitter.sign_calls(), 1);
}
#[test]
fn late_applied_retired_envelope_fences_new_generation_until_semantic_finality() {
    let temp = tempfile::tempdir().expect("tempdir");
    let authority = account(1);
    let active_policy = policy(1);
    orchestrator_fixture!(orchestrator; reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32]))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound { observed_finalized_height: 1 })); => config(&temp, "late-old-envelope.norito"); deps(Arc::clone(&reader), Arc::clone(&submitter)); "orchestrator");
    orchestrator
        .submit(
            authority.clone(),
            policy_action(active_policy.clone()),
            [0x83; 32],
        )
        .expect("initial submission");
    let (operation_id, _, first_signed, first_timing, _) = retained_envelope(&orchestrator);
    submitter.set_lookup(
        operation_id,
        first_signed.transaction_id,
        ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 2,
        },
    );
    reader.replace(empty_snapshot_at(
        2,
        [2; 32],
        first_timing.expires_at_unix_ms,
    ));
    orchestrator.reconcile().expect("renew envelope");
    let (_, generation, second_signed, second_timing, _) = retained_envelope(&orchestrator);
    assert_eq!(generation, 2);
    submitter.set_lookup(
        operation_id,
        first_signed.transaction_id,
        ModerationSubmissionLookupV1::Pending {
            transaction_id: first_signed.transaction_id,
        },
    );
    submitter.set_lookup(
        operation_id,
        second_signed.transaction_id,
        ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 3,
        },
    );
    reader.replace(empty_snapshot_at(
        3,
        [3; 32],
        second_timing.expires_at_unix_ms.saturating_add(1),
    ));
    orchestrator
        .reconcile()
        .expect("late old pending result fences replacement");
    {
        single_outbox_entry!(state, entry = orchestrator; "orchestrator state"; "fenced renewed entry");
        assert_eq!(entry.envelope_generation, 2);
        assert_eq!(
            entry.retired_envelopes[0].disposition,
            StoredRetiredEnvelopeDispositionV1::Pending
        );
        assert_eq!(
            state.operations[0].transaction_id,
            Some(first_signed.transaction_id)
        );
    }
    assert_eq!(submitter.sign_calls(), 2);
    assert_eq!(submitter.calls(), 2);
    submitter.set_lookup(
        operation_id,
        first_signed.transaction_id,
        ModerationSubmissionLookupV1::Applied {
            transaction_id: first_signed.transaction_id,
        },
    );
    reader.replace(empty_snapshot_at(
        4,
        [4; 32],
        second_timing
            .expires_at_unix_ms
            .saturating_add(MODERATION_TRANSACTION_TTL_MS_V1),
    ));
    orchestrator
        .reconcile()
        .expect("late old application makes the history fence terminal");
    {
        let state = orchestrator.state.lock().expect("orchestrator state");
        assert_eq!(
            state.outbox[0].retired_envelopes[0].disposition,
            StoredRetiredEnvelopeDispositionV1::Applied
        );
    }
    submitter.set_lookup(
        operation_id,
        first_signed.transaction_id,
        ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 5,
        },
    );
    reader.replace(empty_snapshot_at(
        5,
        [5; 32],
        second_timing
            .expires_at_unix_ms
            .saturating_add(MODERATION_TRANSACTION_TTL_MS_V1.saturating_mul(2)),
    ));
    orchestrator
        .reconcile()
        .expect("applied history fence is sticky");
    assert_eq!(retained_envelope(&orchestrator).1, 2);
    assert_eq!(submitter.sign_calls(), 2);
    let mut finalized = snapshot_with_policy(6, [6; 32], active_policy, authority);
    finalized.finalized_at_unix_ms = second_timing
        .expires_at_unix_ms
        .saturating_add(MODERATION_TRANSACTION_TTL_MS_V1.saturating_mul(3));
    reader.replace(finalized);
    orchestrator
        .reconcile()
        .expect("authoritative semantic effect finalizes operation");
    let state = orchestrator.state.lock().expect("orchestrator state");
    assert!(state.outbox.is_empty());
    assert_eq!(
        state.operations[0].status,
        StoredOperationStatusV1::Finalized
    );
    assert_eq!(
        state.operations[0].transaction_id,
        Some(first_signed.transaction_id)
    );
}
#[test]
fn restart_recovers_retired_generation_after_signer_outage() {
    orchestrator_fixture!(orchestrator; temp = tempfile::tempdir().expect("tempdir"); reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32]))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound { observed_finalized_height: 1 })); checkpoint = config(&temp, "retired-before-resign.norito"); => checkpoint.clone(); deps(Arc::clone(&reader), Arc::clone(&submitter)); "orchestrator");
    orchestrator
        .submit(account(1), policy_action(policy(1)), [0x84; 32])
        .expect("initial submission");
    let (operation_id, _, first_signed, first_timing, _) = retained_envelope(&orchestrator);
    submitter.set_lookup(
        operation_id,
        first_signed.transaction_id,
        ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 2,
        },
    );
    submitter.set_sign_failure(Some(ModerationSubmissionFailureV1::RuntimeUnavailable));
    reader.replace(empty_snapshot_at(
        2,
        [2; 32],
        first_timing.expires_at_unix_ms,
    ));
    orchestrator
        .reconcile()
        .expect("persist retired generation despite signer outage");
    {
        single_outbox_entry!(state, entry = orchestrator; "orchestrator state"; "one retired ready entry");
        assert_eq!(entry.envelope_generation, 2);
        assert_eq!(entry.state, StoredOutboxStateV1::Ready);
        assert_eq!(entry.retired_envelopes.len(), 1);
        assert!(entry.transaction_id.is_none());
    }
    drop(orchestrator);
    submitter.set_sign_failure(None);
    open_test_orchestrator!(restarted = checkpoint; deps(reader, Arc::clone(&submitter)); "restart from retired ready generation");
    restarted
        .reconcile()
        .expect("sign and submit the next generation after restart");
    let (_, generation, second_signed, _, state) = retained_envelope(&restarted);
    assert_eq!(generation, 2);
    assert_eq!(state, StoredOutboxStateV1::Submitted);
    assert_ne!(second_signed.transaction_id, first_signed.transaction_id);
    assert_eq!(submitter.sign_calls(), 3);
}
#[test]
fn renewed_envelope_restart_replays_byte_identical_bytes_without_resigning() {
    orchestrator_fixture!(orchestrator; temp = tempfile::tempdir().expect("tempdir"); reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32]))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound { observed_finalized_height: 1 })); checkpoint = config(&temp, "renewed-byte-identical.norito"); => checkpoint.clone(); deps(Arc::clone(&reader), Arc::clone(&submitter)); "orchestrator");
    orchestrator
        .submit(account(1), policy_action(policy(1)), [0x85; 32])
        .expect("initial submission");
    let (operation_id, _, first_signed, first_timing, _) = retained_envelope(&orchestrator);
    submitter.set_lookup(
        operation_id,
        first_signed.transaction_id,
        ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 2,
        },
    );
    submitter.set_failure(Some(ModerationSubmissionFailureV1::NotSubmittedUnavailable));
    reader.replace(empty_snapshot_at(
        2,
        [2; 32],
        first_timing.expires_at_unix_ms,
    ));
    orchestrator
        .reconcile()
        .expect("retain renewed envelope after definite non-submission");
    let (_, generation, retained, _, state) = retained_envelope(&orchestrator);
    assert_eq!(generation, 2);
    assert_eq!(state, StoredOutboxStateV1::Signed);
    assert_eq!(submitter.sign_calls(), 2);
    drop(orchestrator);
    submitter.set_failure(None);
    open_test_orchestrator!(restarted = checkpoint; deps(reader, Arc::clone(&submitter)); "restart with renewed retained bytes");
    restarted
        .reconcile()
        .expect("replay exact renewed envelope");
    let (_, generation, replayed, _, state) = retained_envelope(&restarted);
    assert_eq!(generation, 2);
    assert_eq!(state, StoredOutboxStateV1::Submitted);
    assert_eq!(replayed, retained);
    assert_eq!(submitter.sign_calls(), 2);
}
#[test]
fn tampered_retired_envelope_history_fails_closed_on_restart() {
    orchestrator_fixture!(orchestrator; temp = tempfile::tempdir().expect("tempdir"); reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32]))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound { observed_finalized_height: 1 })); checkpoint = config(&temp, "tampered-retired-history.norito"); => checkpoint.clone(); deps(Arc::clone(&reader), Arc::clone(&submitter)); "orchestrator");
    orchestrator
        .submit(account(1), policy_action(policy(1)), [0x86; 32])
        .expect("initial submission");
    let (operation_id, _, first_signed, first_timing, _) = retained_envelope(&orchestrator);
    submitter.set_lookup(
        operation_id,
        first_signed.transaction_id,
        ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 2,
        },
    );
    reader.replace(empty_snapshot_at(
        2,
        [2; 32],
        first_timing.expires_at_unix_ms,
    ));
    orchestrator.reconcile().expect("renew envelope");
    drop(orchestrator);
    let original = fs::read(&checkpoint.checkpoint_path).expect("read checkpoint");
    for tamper in 0_u8..3 {
        let mut state: ModerationOrchestratorCheckpointV1 =
            norito::decode_from_bytes(&original).expect("decode checkpoint");
        let retired = state.outbox[0]
            .retired_envelopes
            .first_mut()
            .expect("retired history");
        match tamper {
            0 => retired.transaction_id[0] ^= 0x80,
            1 => retired.signed_transaction_digest[0] ^= 0x80,
            2 => retired.record_digest[0] ^= 0x80,
            _ => unreachable!(),
        }
        write_atomic(
            &checkpoint.checkpoint_path,
            &norito::to_bytes(&state).expect("encode tampered history"),
        )
        .expect("write tampered history");
        assert!(matches!(
            ModerationOrchestratorV1::open(
                checkpoint.clone(),
                deps(Arc::clone(&reader), Arc::clone(&submitter))
            ),
            Err(ModerationOrchestratorError::CheckpointCorrupt(_))
        ));
        write_atomic(&checkpoint.checkpoint_path, &original).expect("restore checkpoint");
    }
}
#[test]
fn envelope_generation_increment_fails_closed_on_overflow() {
    assert_eq!(
        next_envelope_generation(u32::MAX),
        Err(ModerationOrchestratorError::GenerationOverflow)
    );
}
macro_rules! ingress_crash_recovery_tests {
    ($( $name:ident: $checkpoint_name:literal, $nonce:expr, $effect:ident, $reconcile:literal; )+) => {$(
        #[test]
        fn $name() {
            orchestrator_fixture!(orchestrator; temp = tempfile::tempdir().expect("tempdir"); reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32]))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound { observed_finalized_height: 2 })); checkpoint = config(&temp, $checkpoint_name); => checkpoint.clone(); deps(Arc::clone(&reader), Arc::clone(&submitter)); "orchestrator");
            let operation_id = seed_default_operation!(orchestrator; $nonce);
            execute_one_prepared_sign(&orchestrator, operation_id);
            let interrupted = prepare_one_submit(&orchestrator, operation_id);
            let retained = ingress_crash_recovery_tests!(@retained $effect, interrupted, submitter);
            drop(interrupted);
            drop(orchestrator);
            let mut after_lease = empty_snapshot(2, [2; 32]);
            after_lease.finalized_at_unix_ms = MODERATION_EXTERNAL_WORK_LEASE_MS_V1 + 2;
            reader.replace(after_lease);
            open_test_orchestrator!(restarted = checkpoint; deps(reader, Arc::clone(&submitter)); "restart");
            restarted.reconcile().expect($reconcile);
            find_outbox_entry!(state, entry = restarted, operation_id; "restarted state"; "submitted entry");
            assert_eq!(entry.state, StoredOutboxStateV1::Submitted);
            assert_eq!(
                moderation_signed_transaction(entry).expect("retained exact envelope"),
                retained
            );
            assert_eq!(submitter.sign_calls(), 1);
            assert_eq!(submitter.calls(), 1);
        }
    )+};
    (@retained before, $interrupted:ident, $submitter:ident) => {
        match &$interrupted {
            PreparedExternalWorkV1::Submit { signed, .. } => signed.clone(),
            _ => unreachable!("submit claim"),
        }
    };
    (@retained after, $interrupted:ident, $submitter:ident) => {
        match &$interrupted {
            PreparedExternalWorkV1::Submit { request, signed, .. } => {
                $submitter
                    .submit_signed(request, signed)
                    .expect("ingress effect before crash");
                signed.clone()
            }
            _ => unreachable!("submit claim"),
        }
    };
}
ingress_crash_recovery_tests! {
    restart_reconciles_crash_before_ingress_without_replacing_signed_bytes:
        "crash-before-ingress.norito", [0x47; 32], before,
        "lookup proves no ingress before exact retry";
    restart_reconciles_crash_after_ingress_effect_without_duplicate_submit:
        "crash-after-ingress.norito", [0x48; 32], after,
        "lookup finds the pre-crash ingress effect";
}
#[test]
fn expired_work_lease_rejects_stale_signer_completion() {
    orchestrator_fixture!(orchestrator; temp = tempfile::tempdir().expect("tempdir"); reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32]))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound { observed_finalized_height: 1 })); => config(&temp, "stale-signer-lease.norito"); deps(Arc::clone(&reader), Arc::clone(&submitter)); "orchestrator");
    let operation_id = seed_default_operation!(orchestrator; [0x49; 32]);
    let stale = {
        let mut state = orchestrator.state.lock().expect("orchestrator state");
        orchestrator
            .prepare_next_external_work_locked(&mut state, &BTreeSet::new(), &BTreeSet::new())
            .expect("prepare stale signer")
            .expect("stale signer claim")
    };
    assert!(matches!(
        &stale,
        PreparedExternalWorkV1::Sign { identity, claim, .. }
            if identity.identity == operation_id && claim.generation == 1
    ));
    reader.replace(empty_snapshot_at(
        2,
        [2; 32],
        1 + MODERATION_EXTERNAL_WORK_LEASE_MS_V1,
    ));
    orchestrator
        .reconcile()
        .expect("new generation reclaims expired signer lease");
    let before_stale_completion =
        fs::read(&orchestrator.config.checkpoint_path).expect("checkpoint before stale result");
    orchestrator
        .execute_external_work(stale)
        .expect("stale completion is ignored");
    let after_stale_completion =
        fs::read(&orchestrator.config.checkpoint_path).expect("checkpoint after stale result");
    assert_eq!(before_stale_completion, after_stale_completion);
    assert_eq!(submitter.sign_calls(), 2);
    assert_eq!(submitter.calls(), 1);
    find_outbox_entry!(state, entry = orchestrator, operation_id; "orchestrator state"; "submitted entry");
    assert_eq!(entry.state, StoredOutboxStateV1::Submitted);
    assert!(entry.work_generation >= 3);
    assert!(entry.work_claim.is_none());
}
#[test]
fn expired_ingress_lease_fences_stale_receipt_and_duplicate_effect() {
    orchestrator_fixture!(orchestrator; temp = tempfile::tempdir().expect("tempdir"); reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32]))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound { observed_finalized_height: 2 })); => config(&temp, "stale-ingress-lease.norito"); deps(Arc::clone(&reader), Arc::clone(&submitter)); "orchestrator");
    let operation_id = seed_default_operation!(orchestrator; [0x4B; 32]);
    execute_one_prepared_sign(&orchestrator, operation_id);
    let stale = prepare_one_submit(&orchestrator, operation_id);
    assert!(matches!(
        &stale,
        PreparedExternalWorkV1::Submit { claim, .. }
            if claim.generation == 2
    ));
    reader.replace(empty_snapshot_at(
        2,
        [2; 32],
        1 + MODERATION_EXTERNAL_WORK_LEASE_MS_V1,
    ));
    orchestrator
        .reconcile()
        .expect("lookup and exact retry reclaim expired ingress lease");
    let before_stale_completion =
        fs::read(&orchestrator.config.checkpoint_path).expect("checkpoint before stale receipt");
    orchestrator
        .execute_external_work(stale)
        .expect("stale ingress receipt is ignored");
    let after_stale_completion =
        fs::read(&orchestrator.config.checkpoint_path).expect("checkpoint after stale receipt");
    assert_eq!(before_stale_completion, after_stale_completion);
    assert_eq!(submitter.sign_calls(), 1);
    assert_eq!(submitter.calls(), 1);
    find_outbox_entry!(state, entry = orchestrator, operation_id; "orchestrator state"; "submitted entry");
    assert_eq!(entry.state, StoredOutboxStateV1::Submitted);
    assert_eq!(entry.attempts, 2);
    assert!(entry.work_claim.is_none());
}
#[test]
fn tampered_external_work_claim_fails_closed_on_restart() {
    orchestrator_fixture!(orchestrator; temp = tempfile::tempdir().expect("tempdir"); reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32]))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)); checkpoint = config(&temp, "tampered-external-claim.norito"); => checkpoint.clone(); deps(Arc::clone(&reader), Arc::clone(&submitter)); "orchestrator");
    let operation_id = seed_default_operation!(orchestrator; [0x4A; 32]);
    let claimed = {
        let mut state = orchestrator.state.lock().expect("orchestrator state");
        orchestrator
            .prepare_next_external_work_locked(&mut state, &BTreeSet::new(), &BTreeSet::new())
            .expect("prepare signer claim")
            .expect("one signer claim")
    };
    assert!(matches!(
        claimed,
        PreparedExternalWorkV1::Sign { identity, .. }
            if identity.identity == operation_id
    ));
    drop(orchestrator);
    let bytes = fs::read(&checkpoint.checkpoint_path).expect("read claimed checkpoint");
    let mut state: ModerationOrchestratorCheckpointV1 =
        norito::decode_from_bytes(&bytes).expect("decode claimed checkpoint");
    state.outbox[0]
        .work_claim
        .as_mut()
        .expect("retained work claim")
        .lease_token[0] ^= 0x80;
    write_atomic(
        &checkpoint.checkpoint_path,
        &norito::to_bytes(&state).expect("encode tampered claim"),
    )
    .expect("write tampered claim");
    assert!(matches!(
        ModerationOrchestratorV1::open(checkpoint, deps(reader, submitter)),
        Err(ModerationOrchestratorError::CheckpointCorrupt(message))
            if message.contains("external-work claim")
    ));
}
#[test]
fn restart_submits_the_exact_envelope_persisted_before_ingress() {
    orchestrator_fixture!(orchestrator; temp = tempfile::tempdir().expect("tempdir"); reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32]))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound { observed_finalized_height: 1 })); checkpoint = config(&temp, "signed-before-ingress.norito"); => checkpoint.clone(); deps(Arc::clone(&reader), Arc::clone(&submitter)); "orchestrator");
    let operation_id = seed_default_operation!(orchestrator; [0x51; 32]);
    execute_one_prepared_sign(&orchestrator, operation_id);
    let (retained_id, retained_digest, retained_bytes) = {
        find_outbox_entry!(state, entry = orchestrator, operation_id; "orchestrator state"; "signed entry");
        assert_eq!(entry.state, StoredOutboxStateV1::Signed);
        (
            entry.transaction_id.expect("transaction id"),
            entry.signed_transaction_digest.expect("transaction digest"),
            entry
                .signed_transaction_bytes
                .clone()
                .expect("signed transaction bytes"),
        )
    };
    assert_eq!(submitter.sign_calls(), 1);
    assert_eq!(submitter.calls(), 0);
    drop(orchestrator);
    open_test_orchestrator!(restarted = checkpoint; deps(reader, Arc::clone(&submitter)); "restart from signed checkpoint");
    restarted
        .reconcile()
        .expect("submit retained envelope after restart");
    find_outbox_entry!(state, entry = restarted, operation_id; "restarted state"; "submitted entry");
    assert_eq!(entry.state, StoredOutboxStateV1::Submitted);
    assert_eq!(entry.transaction_id, Some(retained_id));
    assert_eq!(entry.signed_transaction_digest, Some(retained_digest));
    assert_eq!(
        entry.signed_transaction_bytes.as_deref(),
        Some(retained_bytes.as_slice())
    );
    assert_eq!(submitter.sign_calls(), 1);
    assert_eq!(submitter.calls(), 1);
}
#[test]
fn restart_preserves_unexpired_signing_claim_without_overlap() {
    orchestrator_fixture!(orchestrator; temp = tempfile::tempdir().expect("tempdir"); reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32]))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)); checkpoint = config(&temp, "interrupted-signing.norito"); => checkpoint.clone(); deps(Arc::clone(&reader), Arc::clone(&submitter)); "orchestrator");
    let operation_id = seed_default_operation!(orchestrator; [0x52; 32]);
    let interrupted = {
        let mut state = orchestrator.state.lock().expect("orchestrator state");
        orchestrator
            .prepare_next_external_work_locked(&mut state, &BTreeSet::new(), &BTreeSet::new())
            .expect("prepare interrupted signer work")
            .expect("one interrupted signer claim")
    };
    assert!(matches!(
        interrupted,
        PreparedExternalWorkV1::Sign { identity, .. }
            if identity.identity == operation_id
    ));
    drop(orchestrator);
    open_test_orchestrator!(restarted = checkpoint; deps(reader, submitter); "retain signer-only crash state");
    find_outbox_entry!(state, entry = restarted, operation_id; "restarted state"; "recovered entry");
    assert_eq!(entry.state, StoredOutboxStateV1::Signing);
    assert_eq!(entry.baseline_finalized_height, 1);
    assert_eq!(entry.baseline_finalized_block_hash, [1; 32]);
    assert!(entry.transaction_id.is_none());
    assert!(entry.signed_transaction_digest.is_none());
    assert!(entry.signed_transaction_bytes.is_none());
    assert_eq!(entry.attempts, 0);
    assert!(entry.work_claim.as_ref().is_some_and(|claim| {
        claim.kind == StoredExternalWorkKindV1::Sign
            && claim.lease_expires_at_unix_ms == 1 + MODERATION_EXTERNAL_WORK_LEASE_MS_V1
    }));
}
#[test]
fn tampered_retained_transaction_bytes_digest_and_hash_fail_closed() {
    orchestrator_fixture!(orchestrator; temp = tempfile::tempdir().expect("tempdir"); reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32]))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)); checkpoint = config(&temp, "tampered-signed.norito"); => checkpoint.clone(); deps(Arc::clone(&reader), Arc::clone(&submitter)); "orchestrator");
    let operation_id = seed_default_operation!(orchestrator; [0x53; 32]);
    execute_one_prepared_sign(&orchestrator, operation_id);
    drop(orchestrator);
    let original = fs::read(&checkpoint.checkpoint_path).expect("read canonical signed checkpoint");
    for tamper in 0_u8..3 {
        let mut state: ModerationOrchestratorCheckpointV1 =
            norito::decode_from_bytes(&original).expect("decode checkpoint");
        let entry = state.outbox.first_mut().expect("signed outbox entry");
        match tamper {
            0 => {
                let bytes = entry
                    .signed_transaction_bytes
                    .as_mut()
                    .expect("signed bytes");
                let last = bytes.last_mut().expect("non-empty signed bytes");
                *last ^= 0x80;
                let digest = signed_transaction_digest(bytes);
                entry.signed_transaction_digest = Some(digest);
            }
            1 => {
                entry
                    .signed_transaction_digest
                    .as_mut()
                    .expect("signed digest")[0] ^= 0x80;
            }
            2 => {
                entry.transaction_id.as_mut().expect("transaction id")[0] ^= 0x80;
                state.operations[0]
                    .transaction_id
                    .as_mut()
                    .expect("operation transaction id")[0] ^= 0x80;
            }
            _ => unreachable!(),
        }
        write_atomic(
            &checkpoint.checkpoint_path,
            &norito::to_bytes(&state).expect("encode tampered checkpoint"),
        )
        .expect("write tampered checkpoint");
        assert!(matches!(
            ModerationOrchestratorV1::open(
                checkpoint.clone(),
                deps(Arc::clone(&reader), Arc::clone(&submitter))
            ),
            Err(ModerationOrchestratorError::CheckpointCorrupt(_))
        ));
        write_atomic(&checkpoint.checkpoint_path, &original).expect("restore checkpoint");
    }
}
#[test]
fn definitely_not_submitted_reuses_retained_envelope_without_resigning() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
        observed_finalized_height: 1,
    }));
    submitter.set_failure(Some(ModerationSubmissionFailureV1::NotSubmittedUnavailable));
    open_test_orchestrator!(orchestrator = config(&temp, "not-submitted.norito"); deps(reader, Arc::clone(&submitter)); "orchestrator");
    orchestrator
        .submit(account(1), policy_action(policy(1)), [0x54; 32])
        .expect("retain exact envelope after pre-ingress failure");
    let retained = {
        single_outbox_entry!(state, entry = orchestrator; "orchestrator state"; "one retained outbox entry");
        assert_eq!(entry.state, StoredOutboxStateV1::Signed);
        moderation_signed_transaction(entry).expect("retained envelope")
    };
    assert_eq!(submitter.sign_calls(), 1);
    assert_eq!(submitter.calls(), 1);
    submitter.set_failure(None);
    orchestrator
        .reconcile()
        .expect("retry the exact retained envelope");
    single_outbox_entry!(state, entry = orchestrator; "orchestrator state"; "one submitted outbox entry");
    assert_eq!(entry.state, StoredOutboxStateV1::Submitted);
    assert_eq!(
        moderation_signed_transaction(entry).expect("submitted retained envelope"),
        retained
    );
    assert_eq!(submitter.sign_calls(), 1);
    assert_eq!(submitter.calls(), 2);
}
#[test]
fn ambiguous_submission_is_reconciled_after_restart_without_resubmit() {
    let temp = tempfile::tempdir().expect("tempdir");
    let authority = account(1);
    let active_policy = policy(1);
    let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
    let submitter = Arc::new(MockSubmitter::ambiguous_applied(
        ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 1,
        },
    ));
    let checkpoint = config(&temp, "checkpoint.norito");
    let retained_transaction_id;
    let retained_transaction_digest;
    {
        open_test_orchestrator!(orchestrator = checkpoint.clone(); deps(Arc::clone(&reader), Arc::clone(&submitter)); "orchestrator");
        orchestrator
            .submit(
                authority.clone(),
                policy_action(active_policy.clone()),
                [0x11; 32],
            )
            .expect("ambiguous submit remains pending");
        single_outbox_entry!(state, entry = orchestrator; "orchestrator state"; "one ambiguous outbox entry must remain");
        assert_eq!(entry.state, StoredOutboxStateV1::Ambiguous);
        retained_transaction_id = entry.transaction_id.expect("retained transaction id");
        retained_transaction_digest = entry
            .signed_transaction_digest
            .expect("retained transaction digest");
        let retained_bytes = entry
            .signed_transaction_bytes
            .as_deref()
            .expect("retained signed bytes");
        assert_eq!(
            signed_transaction_digest(retained_bytes),
            retained_transaction_digest
        );
    }
    reader.replace(empty_snapshot(2, [2; 32]));
    open_test_orchestrator!(restarted = checkpoint.clone(); deps(Arc::clone(&reader), Arc::clone(&submitter)); "restart with retained exact transaction");
    restarted
        .reconcile()
        .expect("exact transaction lookup after restart");
    {
        single_outbox_entry!(state, entry = restarted; "restarted state"; "one submitted outbox entry must remain");
        assert_eq!(entry.state, StoredOutboxStateV1::Submitted);
        assert_eq!(entry.transaction_id, Some(retained_transaction_id));
        assert_eq!(
            entry.signed_transaction_digest,
            Some(retained_transaction_digest)
        );
    }
    assert_eq!(submitter.calls(), 1);
    assert_eq!(submitter.sign_calls(), 1);
    reader.replace(snapshot_with_policy(
        3,
        [3; 32],
        active_policy.clone(),
        authority.clone(),
    ));
    restarted.reconcile().expect("finalized reconciliation");
    let replay = restarted
        .submit(authority, policy_action(active_policy), [0x11; 32])
        .expect("finalized replay");
    assert_eq!(submitter.calls(), 1);
    assert_eq!(submitter.sign_calls(), 1);
    assert_eq!(replay.status, ModerationOperationStatusV1::Finalized);
    assert!(replay.replay);
}
#[test]
fn terminal_handoff_crash_after_effect_retries_same_id_after_restart() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let finalized = finalized_case_snapshot(
        activated_case_snapshot(2, [2; 32], governance.clone()),
        3,
        [3; 32],
        governance,
    );
    let mut lease_expired = finalized.clone();
    lease_expired.finalized_height = 4;
    lease_expired.finalized_block_hash = [4; 32];
    lease_expired.finalized_at_unix_ms = finalized
        .finalized_at_unix_ms
        .saturating_add(MODERATION_EXTERNAL_WORK_LEASE_MS_V1)
        .saturating_add(1);
    let reader = Arc::new(MockSnapshotReader::new(finalized));
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
        observed_finalized_height: 3,
    }));
    let settlement = Arc::new(MockHandoffSink::default());
    let publication = Arc::new(MockHandoffSink::default());
    let checkpoint = config(&temp, "handoff-crash-after-effect.norito");
    let runtime_deps = || test_runtime_deps!(reader.checkpoint_store.clone(); submitter.clone(); reader.clone(); settlement.clone(); publication.clone(); Arc::new(MockPanelNotificationSink::default()); Arc::new(MockPanelNotificationArchive::default()));
    open_test_orchestrator!(orchestrator = checkpoint.clone(); runtime_deps(); "orchestrator");
    let (snapshot, digest) = orchestrator
        .read_validated_finalized_snapshot()
        .expect("read finalized snapshot");
    {
        let mut state = orchestrator.state.lock().expect("orchestrator state");
        orchestrator
            .install_finalized_snapshot_locked(&mut state, snapshot, digest)
            .expect("queue terminal handoffs");
    }
    let interrupted = {
        let mut state = orchestrator.state.lock().expect("orchestrator state");
        orchestrator
            .prepare_next_external_work_locked(&mut state, &BTreeSet::new(), &BTreeSet::new())
            .expect("prepare terminal handoff")
            .expect("one terminal handoff claim")
    };
    let handoff = match &interrupted {
        PreparedExternalWorkV1::Handoff { handoff, .. } => handoff.clone(),
        _ => unreachable!("terminal handoff claim"),
    };
    assert_eq!(handoff.kind, ModerationTerminalHandoffKindV1::Settlement);
    settlement
        .deliver(&handoff)
        .expect("sink effect before checkpoint finalization");
    drop(interrupted);
    drop(orchestrator);
    open_test_orchestrator!(restarted = checkpoint; runtime_deps(); "restart");
    restarted
        .reconcile()
        .expect("preserve the unexpired terminal-handoff claim after restart");
    assert_eq!(
        settlement.calls(),
        1,
        "restart must not overlap a live lease"
    );
    assert_eq!(publication.calls(), 1);
    {
        let state = restarted.state.lock().expect("restarted state");
        assert_eq!(state.pending_handoffs.len(), 1);
        assert_eq!(state.completed_handoffs.len(), 1);
    }
    reader.replace(lease_expired);
    restarted
        .reconcile()
        .expect("retry identical handoff after sealed finalized time expires the lease");
    assert_eq!(settlement.calls(), 2);
    assert_eq!(settlement.delivered(), vec![handoff.handoff_id]);
    assert_eq!(publication.calls(), 1);
    assert_eq!(publication.delivered().len(), 1);
    let state = restarted.state.lock().expect("restarted state");
    assert!(state.pending_handoffs.is_empty());
    assert_eq!(state.completed_handoffs.len(), 2);
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
    open_test_orchestrator!(second = second_checkpoint; runtime_deps(); "second orchestrator");
    let first_submit = first
        .submit(governance.clone(), action.clone(), [0x11; 32])
        .expect("first terminal submit");
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
    drop(first);
    reader.replace(finalized_snapshot);
    open_test_orchestrator!(restarted = first_checkpoint; runtime_deps(); "restarted orchestrator");
    restarted
        .reconcile()
        .expect("restart reconciles finalized case");
    second
        .reconcile()
        .expect("split peer reconciles finalized case");
    let restarted_replay = restarted
        .submit(governance.clone(), action.clone(), [0x11; 32])
        .expect("restarted finalized replay");
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
    restarted.reconcile().expect("idempotent restart reconcile");
    second.reconcile().expect("idempotent split-peer reconcile");
    assert_eq!(settlement_sink.delivered().len(), 1);
    assert_eq!(publication_sink.delivered().len(), 1);
}
#[test]
fn outbox_capacity_exhaustion_is_fail_closed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
    let mut bounds = config(&temp, "checkpoint.norito");
    bounds.max_outbox_entries = 1;
    open_test_orchestrator!(orchestrator = bounds; deps(reader, submitter); "orchestrator");
    let authority = account(1);
    orchestrator
        .submit(authority.clone(), policy_action(policy(1)), [0x11; 32])
        .expect("first pending operation");
    let error = orchestrator
        .submit(authority, policy_action(policy(2)), [0x22; 32])
        .expect_err("second pending operation must exceed the bound");
    assert!(matches!(
        error,
        ModerationOrchestratorError::ResourceExhausted {
            resource: "native transaction outbox",
            limit: 1
        }
    ));
}
#[test]
fn no_show_failover_uses_one_stable_native_activation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let (snapshot, expected_sortition_digest) =
        awaiting_acceptance_snapshot(2, [2; 32], governance.clone());
    orchestrator_fixture!(orchestrator; reader = Arc::new(MockSnapshotReader::new(snapshot)); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound { observed_finalized_height: 2 })); => config(&temp, "checkpoint.norito"); deps(reader, Arc::clone(&submitter)); "orchestrator");
    let first = orchestrator
        .run_maintenance(governance.clone(), 1)
        .expect("first failover scan");
    let replay = orchestrator
        .run_maintenance(governance, 1)
        .expect("replayed failover scan");
    assert_eq!(first.len(), 1);
    assert_eq!(replay.len(), 1);
    assert_eq!(first[0].operation_id, replay[0].operation_id);
    assert!(replay[0].replay);
    assert_eq!(submitter.calls(), 1);
    let actions = submitter.actions();
    let [ModerationNativeActionV1::ActivateCase(activation)] = actions.as_slice() else {
        panic!("expected one native activation action");
    };
    assert_eq!(activation.case_id(), "case-failover");
    assert_eq!(activation.round_id(), "round-1");
    assert_eq!(*activation.sortition_digest(), expected_sortition_digest);
}
#[test]
fn sortition_maintenance_uses_the_pinned_anchor_not_the_latest_finalized_hash() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let finalized_block_hash = [0xF3; 32];
    let (snapshot, pinned_anchor) =
        registering_snapshot_with_pinned_anchor(finalized_block_hash, governance.clone());
    assert_ne!(pinned_anchor, finalized_block_hash);
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
        observed_finalized_height: snapshot.finalized_height,
    }));
    open_test_orchestrator!(orchestrator = config(&temp, "pinned-anchor.norito"); deps(Arc::new(MockSnapshotReader::new(snapshot)), Arc::clone(&submitter)); "orchestrator");

    let outcomes = orchestrator
        .run_maintenance(governance, 1)
        .expect("derive one anchored sortition action");
    assert_eq!(outcomes.len(), 1);
    let actions = submitter.actions();
    let [ModerationNativeActionV1::FinalizeSortition(sortition)] = actions.as_slice() else {
        panic!("expected one native sortition action");
    };
    assert_eq!(*sortition.randomness_anchor(), pinned_anchor);
    assert_ne!(*sortition.randomness_anchor(), finalized_block_hash);
}
#[test]
fn post_registration_snapshot_without_a_pinned_anchor_fails_closed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let (mut snapshot, _) = registering_snapshot_with_pinned_anchor([0xF4; 32], governance.clone());
    snapshot.appeals[0].appeal.sortition_anchor = None;
    orchestrator_fixture!(orchestrator; reader = Arc::new(MockSnapshotReader::new(snapshot)); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)); => config(&temp, "missing-anchor.norito"); deps(reader, submitter); "orchestrator");

    let error = orchestrator
        .run_maintenance(governance, 1)
        .expect_err("post-registration finalized state without an anchor must fail");
    assert!(matches!(
        error,
        ModerationOrchestratorError::InvalidFinalizedSnapshot(ref message)
            if message.contains("missing its sortition anchor")
    ));
}
#[test]
fn same_finalized_tip_produces_byte_identical_maintenance_actions_across_replicas() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(100);
    let (snapshot, _) = awaiting_acceptance_snapshot(2, [2; 32], governance.clone());
    let first_submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
        observed_finalized_height: 2,
    }));
    let second_submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
        observed_finalized_height: 2,
    }));
    open_test_orchestrator!(first = config(&temp, "replica-a.norito"); deps(Arc::new(MockSnapshotReader::new(snapshot.clone())), Arc::clone(&first_submitter)); "first replica");
    open_test_orchestrator!(second = config(&temp, "replica-b.norito"); deps(Arc::new(MockSnapshotReader::new(snapshot)), Arc::clone(&second_submitter)); "second replica");
    let first_outcomes = first
        .run_maintenance(governance.clone(), 1)
        .expect("first replica maintenance");
    let second_outcomes = second
        .run_maintenance(governance, 1)
        .expect("second replica maintenance");
    let first_actions = first_submitter.actions();
    let second_actions = second_submitter.actions();
    assert_eq!(first_outcomes.len(), 1);
    assert_eq!(second_outcomes.len(), 1);
    assert_eq!(
        first_outcomes[0].operation_id,
        second_outcomes[0].operation_id
    );
    assert_eq!(first_actions, second_actions);
    assert_eq!(
        norito::to_bytes(&first_actions).expect("encode first actions"),
        norito::to_bytes(&second_actions).expect("encode second actions")
    );
}
#[test]
fn finalized_panel_notifications_are_operation_bound_payload_free_and_byte_identical() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let (snapshot, _) = awaiting_acceptance_snapshot(2, [0x22; 32], governance.clone());
    let selection = snapshot.appeals[0]
        .appeal
        .selection
        .as_ref()
        .expect("selection")
        .clone();
    let expected_source_operation =
        ModerationNativeActionV1::FinalizeSortition(FinalizeSorafsModerationSortition::new(
            "case-failover".to_owned(),
            "round-1".to_owned(),
            snapshot.appeals[0].appeal.pop_snapshot_digest,
            selection.randomness_anchor,
            selection.jurors.clone(),
            selection.waitlist.clone(),
        ))
        .operation_id(&test_network_id(), &governance)
        .expect("source operation");
    let first = isolated_orchestrator!(config(&temp, "panel-replica-a.norito"); snapshot.clone(); MockSubmitter::new(ModerationSubmissionLookupV1::Unknown); "first orchestrator");
    let second = isolated_orchestrator!(config(&temp, "panel-replica-b.norito"); snapshot; MockSubmitter::new(ModerationSubmissionLookupV1::Unknown); "second orchestrator");
    first.reconcile().expect("first reconciliation");
    second.reconcile().expect("second reconciliation");
    let first_entries = first
        .state
        .lock()
        .expect("first state")
        .panel_notifications
        .clone();
    let second_entries = second
        .state
        .lock()
        .expect("second state")
        .panel_notifications
        .clone();
    assert_eq!(first_entries.len(), 3);
    assert_eq!(
        first_entries
            .iter()
            .filter(|entry| {
                entry.notification.kind == ModerationPanelNotificationKindV1::PrimaryAssignment
            })
            .count(),
        2
    );
    assert_eq!(
        first_entries
            .iter()
            .filter(|entry| {
                entry.notification.kind == ModerationPanelNotificationKindV1::WaitlistStandby
            })
            .count(),
        1
    );
    assert!(first_entries.iter().all(|entry| {
        entry.notification.source_operation_id == expected_source_operation
            && entry.notification.finalized_event_cursor.sequence == 5
            && entry.notification.source_occurred_at_unix_ms == 21
    }));
    let first_bytes = norito::to_bytes(&first_entries).expect("encode first notifications");
    let second_bytes = norito::to_bytes(&second_entries).expect("encode second notifications");
    assert_eq!(first_bytes, second_bytes);
    assert_eq!(
        std::fs::read(&first.config().checkpoint_path).expect("read first checkpoint"),
        std::fs::read(&second.config().checkpoint_path).expect("read second checkpoint")
    );
    for forbidden in [b"case-failover".as_slice(), b"round-1", b"ipfs://"] {
        assert!(
            !first_bytes
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "payload-free checkpoint leaked {}",
            String::from_utf8_lossy(forbidden)
        );
    }
    for forbidden_digest in [[0x41; 32], [0x43; 32]] {
        assert!(
            !first_bytes
                .windows(forbidden_digest.len())
                .any(|window| window == forbidden_digest.as_slice()),
            "payload-free checkpoint retained a private intake digest"
        );
    }
}
#[test]
fn qualified_notification_sink_delivers_and_checkpoints_the_due_batch() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let (snapshot, _) = awaiting_acceptance_snapshot(2, [0x26; 32], governance);
    let sink = Arc::new(MockPanelNotificationSink::default());
    open_test_orchestrator!(orchestrator = config(&temp, "panel-qualified-sink.norito"); test_runtime_deps!(Arc::new(MockCheckpointStore::default()); Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)); Arc::new(MockSnapshotReader::new(snapshot)); Arc::new(MockHandoffSink::default()); Arc::new(MockHandoffSink::default()); sink.clone(); Arc::new(MockPanelNotificationArchive::default())); "orchestrator");
    orchestrator.reconcile().expect("queue notifications");
    assert_eq!(
        orchestrator
            .deliver_due_panel_notifications(1_000, 3)
            .expect("deliver qualified notification batch"),
        3
    );
    assert_eq!(sink.calls(), 3);
    assert_eq!(sink.unique_deliveries(), 3);
    assert_eq!(
        orchestrator
            .deliver_due_panel_notifications(1_001, 3)
            .expect("delivered notifications are not re-claimed"),
        0
    );
    assert!(
        orchestrator
            .state
            .lock()
            .expect("state")
            .panel_notifications
            .iter()
            .all(|entry| entry.state == StoredPanelNotificationStateV1::Delivered)
    );
}
#[test]
fn finalized_activation_notifies_only_the_authoritative_ballot_roster() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let snapshot = activated_case_snapshot(3, [0x27; 32], governance.clone());
    let selection = snapshot.appeals[0]
        .appeal
        .selection
        .as_ref()
        .expect("selection");
    let expected_source_operation =
        ModerationNativeActionV1::ActivateCase(ActivateSorafsModerationCase::new(
            "case-failover".to_owned(),
            "round-1".to_owned(),
            selection.sortition_digest,
        ))
        .operation_id(&test_network_id(), &governance)
        .expect("activation operation");
    let expected_recipients = snapshot.cases[0]
        .case
        .spec
        .jurors
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let orchestrator = isolated_orchestrator!(config(&temp, "panel-activation.norito"); snapshot; MockSubmitter::new(ModerationSubmissionLookupV1::Unknown); "orchestrator");
    orchestrator.reconcile().expect("queue activation notices");
    let state = orchestrator.state.lock().expect("state");
    let actual_recipients = state
        .panel_notifications
        .iter()
        .map(|entry| entry.notification.recipient.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_recipients, expected_recipients);
    assert_eq!(state.panel_notifications.len(), 2);
    assert!(state.panel_notifications.iter().all(|entry| {
        entry.notification.kind == ModerationPanelNotificationKindV1::BallotActivated
            && entry.notification.source_operation_id == expected_source_operation
            && entry.notification.finalized_event_cursor.sequence == 6
    }));
}
#[test]
fn signed_native_redrive_preserves_incident_and_splits_a_new_unresolved_failure() {
    orchestrator_fixture!(orchestrator; temp = tempfile::tempdir().expect("tempdir"); reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32]))); submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)); => config(&temp, "native-dead-letter-resolution.norito"); deps(Arc::clone(&reader), submitter); "orchestrator");
    let operation_id = seed_default_operation!(orchestrator; [0xD1; 32]);
    {
        let mut state = orchestrator.state.lock().expect("orchestrator state");
        orchestrator
            .dead_letter_submission_locked(
                &mut state,
                0,
                StoredDeadLetterReasonV1::PermanentRejection,
            )
            .expect("first native incident");
    }
    let redrive = orchestrator
        .prepare_dead_letter_resolution(
            operation_id,
            ModerationDeadLetterKindV1::NativeSubmission,
            ModerationDeadLetterResolutionActionV1::Redrive,
            1,
        )
        .expect("prepare exact native redrive");
    let mut foreign = redrive.clone();
    foreign.network_id = iroha_data_model::NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"same-label-foreign-moderation-resolution-genesis",
        )),
    );
    let before_foreign = orchestrator
        .state
        .lock()
        .expect("state before foreign")
        .clone();
    assert!(matches!(
        orchestrator
            .apply_dead_letter_resolution(foreign.clone(), sign_dead_letter_resolution(&foreign),),
        Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid)
    ));
    assert_eq!(
        *orchestrator.state.lock().expect("state after foreign"),
        before_foreign,
        "a foreign genesis must be rejected before durable mutation"
    );
    let redrive_signature = sign_dead_letter_resolution(&redrive);
    orchestrator
        .apply_dead_letter_resolution(redrive.clone(), redrive_signature)
        .expect("apply signed native redrive");
    let after_redrive = orchestrator
        .state
        .lock()
        .expect("state after redrive")
        .clone();
    assert!(
        orchestrator
            .apply_dead_letter_resolution(redrive.clone(), redrive_signature)
            .is_err()
    );
    assert_eq!(
        *orchestrator.state.lock().expect("state after replay"),
        after_redrive,
        "a consumed resolution must not mutate state on replay"
    );
    {
        let mut state = orchestrator.state.lock().expect("orchestrator state");
        assert_eq!(state.outbox.len(), 1);
        assert_eq!(state.outbox[0].operation_id, operation_id);
        assert_eq!(state.outbox[0].state, StoredOutboxStateV1::Ready);
        orchestrator
            .dead_letter_submission_locked(
                &mut state,
                0,
                StoredDeadLetterReasonV1::PermanentRejection,
            )
            .expect("second native incident");
    }
    {
        let state = orchestrator.state.lock().expect("orchestrator state");
        assert_eq!(state.dead_letters.len(), 2);
        assert_eq!(state.dead_letters[0].incident_sequence, 1);
        assert!(state.dead_letters[0].resolution.is_some());
        assert_eq!(state.dead_letters[1].incident_sequence, 2);
        assert!(state.dead_letters[1].resolution.is_none());
        let records = collect_terminal_archive_records(&state)
            .expect("collect only the resolved native incident");
        let [
            ModerationTerminalArchiveRecordV1::DurableDeadLetter {
                incident_sequence,
                resolution,
                operation_source_record_digest,
                ..
            },
        ] = records.as_slice()
        else {
            panic!("only the first resolved native incident is archive eligible");
        };
        assert_eq!(*incident_sequence, 1);
        assert_eq!(
            resolution.action,
            ModerationDeadLetterResolutionActionV1::Redrive
        );
        assert!(operation_source_record_digest.is_none());
    }
    assert_eq!(
        orchestrator
            .durable_health()
            .expect("unresolved native incident health")
            .durable_dead_letters,
        1
    );
    let acknowledge = orchestrator
        .prepare_dead_letter_resolution(
            operation_id,
            ModerationDeadLetterKindV1::NativeSubmission,
            ModerationDeadLetterResolutionActionV1::Acknowledge,
            1,
        )
        .expect("prepare latest native acknowledgement");
    orchestrator
        .apply_dead_letter_resolution(
            acknowledge.clone(),
            sign_dead_letter_resolution(&acknowledge),
        )
        .expect("apply latest native acknowledgement");
    let state = orchestrator.state.lock().expect("orchestrator state");
    let records =
        collect_terminal_archive_records(&state).expect("collect both resolved native incidents");
    assert_eq!(records.len(), 2);
    for record in records {
        let ModerationTerminalArchiveRecordV1::DurableDeadLetter {
            incident_sequence,
            operation_source_record_digest,
            ..
        } = record
        else {
            panic!("native resolution must archive as a durable dead letter");
        };
        if incident_sequence == 1 {
            assert!(operation_source_record_digest.is_none());
        } else {
            assert_eq!(incident_sequence, 2);
            assert!(operation_source_record_digest.is_some());
        }
    }
}
#[test]
fn signed_panel_and_terminal_resolutions_apply_exact_dispositions() {
    let panel_temp = tempfile::tempdir().expect("panel tempdir");
    let governance = account(99);
    let (panel_snapshot, _) = awaiting_acceptance_snapshot(2, [0x31; 32], governance.clone());
    let panel = isolated_orchestrator!(config(&panel_temp, "panel-dead-letter-resolution.norito"); panel_snapshot; MockSubmitter::new(ModerationSubmissionLookupV1::Unknown); "panel orchestrator");
    panel.reconcile().expect("queue panel notifications");
    let claim = panel
        .claim_panel_notifications([0xD2; 32], 30, 1)
        .expect("claim panel notification")
        .into_iter()
        .next()
        .expect("one panel notification");
    panel
        .release_panel_notification_claim(
            claim.notification.notification_id,
            claim.worker_id,
            claim.lease_token,
            ModerationPanelNotificationFailureV1::Permanent,
            31,
        )
        .expect("dead-letter panel notification");
    let panel_acknowledgement = panel
        .prepare_dead_letter_resolution(
            claim.notification.notification_id,
            ModerationDeadLetterKindV1::PanelNotification,
            ModerationDeadLetterResolutionActionV1::Acknowledge,
            31,
        )
        .expect("prepare exact panel acknowledgement");
    panel
        .apply_dead_letter_resolution(
            panel_acknowledgement.clone(),
            sign_dead_letter_resolution(&panel_acknowledgement),
        )
        .expect("apply signed panel acknowledgement");
    assert_eq!(
        panel
            .panel_notification_status(claim.notification.notification_id)
            .expect("resolved panel status"),
        None
    );
    let panel_state = panel.state.lock().expect("panel state");
    assert!(matches!(
        collect_terminal_archive_records(&panel_state)
            .expect("collect resolved panel incident")
            .as_slice(),
        [ModerationTerminalArchiveRecordV1::ResolvedPanelDeadLetter {
            resolution,
            ..
        }] if resolution.action == ModerationDeadLetterResolutionActionV1::Acknowledge
    ));
    drop(panel_state);
    let terminal_temp = tempfile::tempdir().expect("terminal tempdir");
    let finalized = finalized_case_snapshot(
        activated_case_snapshot(2, [2; 32], governance.clone()),
        3,
        [3; 32],
        governance,
    );
    let terminal = isolated_orchestrator!(config(&terminal_temp, "terminal-dead-letter-resolution.norito"); finalized; MockSubmitter::new(ModerationSubmissionLookupV1::Unknown); "terminal orchestrator");
    let (snapshot, digest) = terminal
        .read_validated_finalized_snapshot()
        .expect("read terminal finalized snapshot");
    {
        let mut state = terminal.state.lock().expect("terminal state");
        terminal
            .install_finalized_snapshot_locked(&mut state, snapshot, digest)
            .expect("queue terminal handoffs");
    }
    let (handoff, claim) = {
        let mut state = terminal.state.lock().expect("terminal state");
        match terminal
            .prepare_next_external_work_locked(&mut state, &BTreeSet::new(), &BTreeSet::new())
            .expect("prepare terminal handoff")
            .expect("one terminal handoff")
        {
            PreparedExternalWorkV1::Handoff { handoff, claim, .. } => (handoff, claim),
            _ => panic!("terminal handoff must be selected"),
        }
    };
    terminal
        .finalize_handoff_work(&claim, Err(ModerationHandoffFailureV1::Permanent))
        .expect("dead-letter terminal handoff");
    let terminal_redrive = terminal
        .prepare_dead_letter_resolution(
            handoff.handoff_id,
            ModerationDeadLetterKindV1::TerminalHandoff,
            ModerationDeadLetterResolutionActionV1::Redrive,
            61,
        )
        .expect("prepare exact terminal redrive");
    terminal
        .apply_dead_letter_resolution(
            terminal_redrive.clone(),
            sign_dead_letter_resolution(&terminal_redrive),
        )
        .expect("apply signed terminal redrive");
    let terminal_state = terminal.state.lock().expect("terminal state");
    assert!(
        terminal_state
            .pending_handoffs
            .iter()
            .any(|entry| entry.handoff == handoff)
    );
    assert!(
        collect_terminal_archive_records(&terminal_state)
            .expect("collect resolved terminal incident")
            .iter()
            .any(|record| matches!(
                record,
                ModerationTerminalArchiveRecordV1::DurableDeadLetter {
                    identity,
                    resolution,
                    handoff_kind: Some(kind),
                    handoff_outcome_digest: Some(outcome_digest),
                    handoff_finalized_cursor: Some(cursor),
                    ..
                } if *identity == handoff.handoff_id
                    && resolution.action == ModerationDeadLetterResolutionActionV1::Redrive
                    && *kind == handoff.kind
                    && *outcome_digest == handoff.outcome_digest
                    && *cursor == handoff.finalized_cursor
            ))
    );
}
struct SaturatedPanelNotificationFixture {
    bounds: ModerationOrchestratorConfigV1,
    governance: AccountId,
    reader: Arc<MockSnapshotReader>,
    checkpoint_store: Arc<MockCheckpointStore>,
    archive: Arc<MockPanelNotificationArchive>,
    publication_sink: Arc<MockHandoffSink>,
    orchestrator: ModerationOrchestratorV1,
}
