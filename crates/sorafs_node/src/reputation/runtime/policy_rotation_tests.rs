// Policy rotation retains exact source-time authority and requires current source observations.

#[test]
fn policy_rotation_rebinds_only_ready_rows_and_preserves_ambiguous_bytes() {
    let temp = TempDir::new().expect("tempdir");
    let first_policy = journal_authority_policy();
    let first_activation = FINALIZED_AT_MS - 1_000;
    let outbox = Arc::new(
        ReputationJournalProducerOutboxV1::open(
            temp.path(),
            strict_policy(first_policy.clone(), "producer policy"),
        )
        .expect("outbox"),
    );
    let first_cursor = finalized_cursor(10, [0xC1; 32], FINALIZED_AT_MS + 100);
    outbox
        .synchronize_authority_policy(
            authority_record(first_policy.clone(), first_activation),
            first_cursor,
        )
        .expect("initialize policy");
    let producer = por_producer(Arc::clone(&outbox));
    let first_id = por_event_id(&producer, 7, verified_por(0x41), "first");
    let second_id = por_event_id(&producer, 8, verified_por(0x42), "second");
    let post_activation = shifted_por(0x43, 200, true);
    let post_activation_id = por_event_id(
        &producer,
        9,
        post_activation,
        "post-activation source queued before observing rotation",
    );
    let ambiguous = outbox
        .begin_submission(first_id, finalized_id(10, [0xC1; 32]))
        .expect("capture first bytes");
    let mut successor = first_policy.clone();
    successor.revision = 2;
    successor.predecessor_policy_digest =
        Some(first_policy.canonical_digest().expect("first digest"));
    successor.por_recorder_authority = account(0x51);
    let successor_activation = FINALIZED_AT_MS + 150;
    let successor_record = authority_record(successor.clone(), successor_activation);
    assert_eq!(
        outbox
            .synchronize_authority_policy(
                successor_record.clone(),
                finalized_cursor(11, [0xC2; 32], FINALIZED_AT_MS + 200),
            )
            .expect("rotate"),
        ReputationJournalPolicySyncOutcomeV1::Rotated { rebound_ready: 1 }
    );
    let late_historical_id = por_event_id(
        &producer,
        11,
        verified_por(0x45),
        "first-seen historical source after rotation",
    );
    let boundary_source = shifted_por(0x46, successor_activation - FINALIZED_AT_MS, true);
    let boundary_id = por_event_id(
        &producer,
        12,
        boundary_source,
        "successor activation boundary",
    );
    let (late_policy_digest, late_authority, boundary_policy_digest, boundary_authority) = {
        let state = outbox.state.lock().expect("outbox state");
        let late = state
            .checkpoint
            .pending
            .iter()
            .find(|delivery| delivery.entry.event_id == late_historical_id)
            .expect("late historical row");
        let boundary = state
            .checkpoint
            .pending
            .iter()
            .find(|delivery| delivery.entry.event_id == boundary_id)
            .expect("boundary row");
        (
            late.entry.authority_policy_digest,
            late.entry.recorded_by.clone(),
            boundary.entry.authority_policy_digest,
            boundary.entry.recorded_by.clone(),
        )
    };
    assert_eq!(
        late_policy_digest,
        first_policy.canonical_digest().expect("first digest")
    );
    assert_eq!(late_authority, first_policy.por_recorder_authority);
    assert_eq!(boundary_policy_digest, successor_record.policy_digest);
    assert_eq!(boundary_authority, successor.por_recorder_authority);
    let predating_delta = FINALIZED_AT_MS - first_activation + 1;
    let predating_source = shifted_por(0x47, predating_delta, false);
    assert!(matches!(
        producer.enqueue_terminal(provider(13), predating_source),
        Err(ReputationRuntimeError::InvalidAuthorityPolicy)
    ));
    let pending = outbox.pending(8).expect("pending");
    assert!(
        pending.iter().any(|row| {
            row.event_id == ambiguous.event_id
                && row.state == ReputationJournalDeliveryStateV1::Ambiguous
        }),
        "ambiguous exact bytes must remain immutable across rotation"
    );
    assert!(
        pending.iter().any(|row| row.event_id == second_id),
        "source material from before activation must retain its historical policy"
    );
    assert!(
        pending.iter().all(|row| row.event_id != post_activation_id),
        "a never-exposed Ready row sourced after activation must be rebound"
    );
    assert_eq!(
        producer
            .enqueue_terminal(provider(7), verified_por(0x41))
            .expect("retained source replay resolves before current-policy construction"),
        ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id: first_id }
    );
    let mut substituted_source = verified_por(0x41);
    substituted_source.decided_at_unix_ms = substituted_source.decided_at_unix_ms.saturating_add(1);
    assert!(matches!(
        producer.enqueue_terminal(provider(7), substituted_source),
        Err(ReputationRuntimeError::JournalSourceConflict)
    ));
    reconcile_empty(&outbox, 12, [0xC3; 32], FINALIZED_AT_MS.saturating_add(300));
    outbox
        .mark_finalized_absent(first_id, finalized_id(12, [0xC3; 32]), [0xC4; 32])
        .expect("prove old append absent");
    let rebound = outbox
        .begin_submission_against_active_policy(
            first_id,
            successor_record.policy_digest,
            finalized_id(12, [0xC3; 32]),
            FINALIZED_AT_MS.saturating_add(300),
        )
        .expect("retry source-time-valid historical bytes");
    assert_eq!(rebound.event_id, first_id);
    assert_eq!(rebound.authority, first_policy.por_recorder_authority);
    let not_yet_finalized_source = shifted_por(0x44, 400, true);
    let before_stale_source = outbox.state.lock().unwrap().checkpoint.clone();
    let durable_before_stale_source = durable_snapshot(temp.path(), &outbox);
    assert!(matches!(
        producer.enqueue_terminal(provider(10), not_yet_finalized_source.clone()),
        Err(ReputationRuntimeError::FinalizedRollback)
    ));
    assert_eq!(outbox.state.lock().unwrap().checkpoint, before_stale_source);
    assert_eq!(
        durable_snapshot(temp.path(), &outbox),
        durable_before_stale_source
    );
    // The source adapter must observe the same finalized cursor as the
    // reconciled journal. Its source event may still await a later block.
    let current_producer = por_producer_for(
        Arc::clone(&outbox),
        source_query(Ok(source_view_at(
            12,
            [0xC3; 32],
            FINALIZED_AT_MS.saturating_add(300),
            None,
        ))),
    );
    let future_event_id = por_event_id(
        &current_producer,
        10,
        not_yet_finalized_source,
        "retain source awaiting a sufficiently new finalized view",
    );
    assert!(matches!(
        outbox.begin_submission_against_active_policy(
            future_event_id,
            successor_record.policy_digest,
            finalized_id(12, [0xC3; 32]),
            FINALIZED_AT_MS.saturating_add(300),
        ),
        Err(ReputationRuntimeError::JournalSourceNotFinalized)
    ));
    drop(current_producer);
    drop(producer);
    drop(outbox);
    assert_bad_reopen(temp.path(), first_policy.clone(), "stale producer policy");
    let mut substituted_successor = successor.clone();
    substituted_successor.por_recorder_authority = account(0x52);
    assert_bad_reopen(
        temp.path(),
        substituted_successor,
        "substituted producer policy",
    );
    let mut skipped_successor = successor.clone();
    skipped_successor.revision = skipped_successor.revision.saturating_add(1);
    skipped_successor.predecessor_policy_digest =
        Some(successor.canonical_digest().expect("successor digest"));
    assert_bad_reopen(temp.path(), skipped_successor, "skipped producer policy");
    let restored = Arc::new(
        ReputationJournalProducerOutboxV1::open(
            temp.path(),
            strict_policy(successor, "producer policy"),
        )
        .expect("restore rotated producer checkpoint"),
    );
    let restored_producer = por_producer(restored);
    assert_eq!(
        restored_producer
            .enqueue_terminal(provider(7), verified_por(0x41))
            .expect("exact retained replay after restart"),
        ReputationJournalEnqueueOutcomeV1::ExactReplay { event_id: first_id }
    );
}
