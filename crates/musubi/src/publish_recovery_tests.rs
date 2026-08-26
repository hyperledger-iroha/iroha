// Publication recovery, retry, readback, and finalization tests.
#[test]
fn finalized_chain_time_must_strictly_pass_the_exact_registration_deadline() {
    let (request, broker) = request();
    let operation_id = request.operation_id();
    let receipt = signed_receipt(&request.receipt_binding(), &broker);
    let intent = registration_intent(operation_id, &request, receipt);
    let valid_until_ms =
        archive_registration_intent_valid_until_ms(&intent).expect("exact registration deadline");
    let mut at_deadline = archive_absence_evidence(&request, 60);
    at_deadline.finalized_time_ms = valid_until_ms;
    let terminal = PublicationArchiveRegistrationTerminalV1::finalized_validity_window_elapsed(
        &intent,
        at_deadline,
    );
    assert!(terminal.validate_for(&request, &intent).is_err());
    let mut after_deadline = archive_absence_evidence(&request, 61);
    after_deadline.finalized_time_ms = valid_until_ms + 1;
    let terminal = PublicationArchiveRegistrationTerminalV1::finalized_validity_window_elapsed(
        &intent,
        after_deadline,
    );
    terminal
        .validate_for(&request, &intent)
        .expect("finalized time after the exact deadline is terminal");
    let mut substituted = terminal;
    substituted.reason =
        PublicationArchiveRegistrationTerminalReasonV1::FinalizedValidityWindowElapsed {
            finalized_time_ms: valid_until_ms + 2,
        };
    assert!(substituted.validate_for(&request, &intent).is_err());
}
#[cfg(unix)]
#[test]
fn retry_and_receipt_substitution_never_advance_the_journal() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let engine = PublicationEngine::new(&store);
    let (request, broker) = request();
    let source = BytesSource(b"runtime-only-car-secret".to_vec());
    let plan_bytes = source
        .car_plan(&request.archive_commitment)
        .expect("fixture wire plan")
        .canonical_bytes()
        .expect("canonical fixture plan");
    let operation_id = engine
        .begin_detached(request)
        .expect("persist detached operation");
    let journal_bytes = fs::read(
        temp.path()
            .join(JOURNAL_DIRECTORY)
            .join(format!("{operation_id}.{JOURNAL_EXTENSION}")),
    )
    .expect("read journal");
    assert!(
        !journal_bytes
            .windows(b"runtime-only-car-secret".len())
            .any(|window| window == b"runtime-only-car-secret")
    );
    assert!(
        !journal_bytes
            .windows(plan_bytes.len())
            .any(|window| window == plan_bytes.as_slice())
    );
    let mut backend = EarlyBackend {
        broker,
        fail_validation_once: true,
        substitute_receipt: true,
        now_ms: 1_500,
        receipt_window: None,
        prepare_calls: 0,
    };
    let error = engine
        .advance_once(operation_id, &source, &mut backend)
        .expect_err("first validation is retryable");
    let PublicationError::Backend(error) = error else {
        panic!("expected backend failure");
    };
    assert_eq!(error.class(), PublicationBackendFailureClass::Retryable);
    let unchanged = store.load(operation_id).expect("unchanged journal");
    assert_eq!(unchanged.phase, PublicationPhaseV1::Validation);
    assert_eq!(unchanged.revision, 1);
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("retry validation"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::SeedIngress)
    );
    let error = engine
        .advance_once(operation_id, &source, &mut backend)
        .expect_err("substituted receipt must fail");
    assert!(matches!(
        error,
        PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::SeedIngress,
            ..
        }
    ));
    let unchanged = store
        .load(operation_id)
        .expect("unchanged after substitution");
    assert_eq!(unchanged.phase, PublicationPhaseV1::SeedIngress);
    assert_eq!(unchanged.revision, 2);
    assert!(unchanged.staging_receipt.is_none());
}
#[cfg(unix)]
#[test]
fn future_issued_receipt_waits_within_service_skew_before_registration() {
    let within = tempdir().expect("within-skew state root");
    let within_store =
        PublicationJournalStore::open(within.path()).expect("within-skew journal store");
    let within_engine = PublicationEngine::new(&within_store);
    let (within_request, within_broker) = request();
    let within_operation = within_engine
        .begin_detached(within_request)
        .expect("persist within-skew operation");
    let source = BytesSource(b"canonical-car".to_vec());
    let now_ms = 1_000;
    let issued_at_ms = now_ms + MUSUBI_PUBLICATION_SERVICE_MAX_CLOCK_SKEW_MS_V1;
    let mut within_backend = EarlyBackend {
        broker: within_broker,
        fail_validation_once: false,
        substitute_receipt: false,
        now_ms,
        receipt_window: Some((issued_at_ms, issued_at_ms + 100)),
        prepare_calls: 0,
    };
    assert_eq!(
        within_engine
            .advance_once(within_operation, &source, &mut within_backend)
            .expect("validate within-skew operation"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::SeedIngress)
    );
    assert_eq!(
        within_engine
            .advance_once(within_operation, &source, &mut within_backend)
            .expect("accept bounded future-issued receipt"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
    );
    let waiting = within_store
        .load(within_operation)
        .expect("future-issued receipt journal");
    assert_eq!(
        within_engine
            .advance_once(within_operation, &source, &mut within_backend)
            .expect("wait for receipt issue time"),
        PublicationAdvanceV1::Pending(PublicationPhaseV1::ArchiveRegistration)
    );
    assert_eq!(
        within_store
            .load(within_operation)
            .expect("unchanged waiting journal"),
        waiting
    );
    assert_eq!(within_backend.prepare_calls, 0);
    within_backend.now_ms = issued_at_ms;
    assert_eq!(
        within_engine
            .advance_once(within_operation, &source, &mut within_backend)
            .expect("prepare at the inclusive issue time"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
    );
    assert_eq!(within_backend.prepare_calls, 1);
    assert_eq!(
        within_store
            .load(within_operation)
            .expect("prepared registration journal")
            .archive_registration_attempts
            .len(),
        1
    );
}
#[cfg(unix)]
#[test]
fn future_issued_receipt_beyond_service_skew_is_rejected_before_persistence() {
    let beyond = tempdir().expect("beyond-skew state root");
    let beyond_store =
        PublicationJournalStore::open(beyond.path()).expect("beyond-skew journal store");
    let beyond_engine = PublicationEngine::new(&beyond_store);
    let (beyond_request, beyond_broker) = request();
    let beyond_operation = beyond_engine
        .begin_detached(beyond_request)
        .expect("persist beyond-skew operation");
    let source = BytesSource(b"canonical-car".to_vec());
    let now_ms = 1_000;
    let beyond_issue = now_ms + MUSUBI_PUBLICATION_SERVICE_MAX_CLOCK_SKEW_MS_V1 + 1;
    let mut beyond_backend = EarlyBackend {
        broker: beyond_broker,
        fail_validation_once: false,
        substitute_receipt: false,
        now_ms,
        receipt_window: Some((beyond_issue, beyond_issue + 100)),
        prepare_calls: 0,
    };
    beyond_engine
        .advance_once(beyond_operation, &source, &mut beyond_backend)
        .expect("validate beyond-skew operation");
    assert!(matches!(
        beyond_engine.advance_once(beyond_operation, &source, &mut beyond_backend),
        Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::SeedIngress,
            ..
        })
    ));
    let rejected = beyond_store
        .load(beyond_operation)
        .expect("unchanged beyond-skew journal");
    assert_eq!(rejected.phase, PublicationPhaseV1::SeedIngress);
    assert!(rejected.staging_receipt.is_none());
    assert!(rejected.archive_registration_attempts.is_empty());
    assert_eq!(beyond_backend.prepare_calls, 0);
}
#[cfg(unix)]
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the test preserves and checks both crash boundaries around one exact transaction"
)]
fn registration_intent_recovers_a_dropped_commit_response_after_expiry_and_restart() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let engine = PublicationEngine::new(&store);
    let (request, broker) = request();
    let operation_id = engine
        .begin_detached(request)
        .expect("persist detached operation");
    let source = BytesSource(b"canonical-car".to_vec());
    let mut backend = ArchiveRecoveryBackend {
        broker,
        now_ms: 1_000,
        staged_receipts: Vec::new(),
        prepare_calls: 0,
        registration_calls: 0,
        pin_calls: 0,
        archive_committed: false,
        drop_commit_response_once: true,
        return_conflicting_archive: false,
        registration_mode: ArchiveRecoveryMode::Commit,
    };
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("validate"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::SeedIngress)
    );
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("stage"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
    );
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("persist exact registration intent"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
    );
    let intent_journal = store.load(operation_id).expect("intent journal");
    let durable_intent = intent_journal
        .archive_registration_attempts
        .last()
        .expect("exact signed transaction attempt")
        .intent
        .clone();
    assert!(intent_journal.registered_archive.is_none());
    assert_eq!(backend.prepare_calls, 1);
    assert_eq!(backend.registration_calls, 0);
    let reopened = PublicationJournalStore::open(temp.path()).expect("reopen journal store");
    let resumed = PublicationEngine::new(&reopened);
    let error = resumed
        .advance_once(operation_id, &source, &mut backend)
        .expect_err("simulate response loss after finalized archive commit");
    assert!(matches!(
        error,
        PublicationError::Backend(ref backend_error)
            if backend_error.code() == "ARCHIVE_COMMIT_RESPONSE_DROPPED"
    ));
    let interrupted = reopened.load(operation_id).expect("interrupted journal");
    assert_eq!(
        interrupted
            .archive_registration_attempts
            .last()
            .map(|attempt| &attempt.intent),
        Some(&durable_intent)
    );
    assert!(interrupted.registered_archive.is_none());
    assert!(backend.now_ms > durable_intent.staging_receipt.payload.expires_at_ms);
    assert_eq!(
        resumed
            .advance_once(operation_id, &source, &mut backend)
            .expect("recover authoritative archive after receipt expiry"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
    );
    let recovered = reopened
        .load(operation_id)
        .expect("recovered archive journal");
    assert_eq!(
        recovered
            .registered_archive
            .as_ref()
            .expect("authoritative archive")
            .finalized_transaction_hash,
        durable_intent.transaction_hash
    );
    assert!(recovered.archive_location_attempts.is_empty());
    assert_eq!(backend.staged_receipts.len(), 1);
    assert_eq!(backend.prepare_calls, 1);
    assert_eq!(backend.registration_calls, 2);
    assert_eq!(backend.pin_calls, 0);
    let pin_store = PublicationJournalStore::open(temp.path()).expect("reopen before pin");
    let pin_resume = PublicationEngine::new(&pin_store);
    assert_eq!(
        pin_resume
            .advance_once(operation_id, &source, &mut backend)
            .expect("coordinate storage after durable archive recovery"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
    );
    assert_eq!(backend.pin_calls, 1);
    assert_eq!(
        pin_resume
            .advance_once(operation_id, &source, &mut backend)
            .expect("finalize exact journaled location transaction"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::Replication)
    );
}
#[cfg(unix)]
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the test covers every durable cut of one location replacement"
)]
fn archive_location_generation_recovers_prepared_submitted_applied_and_retired_cuts() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let engine = PublicationEngine::new(&store);
    let (request, broker) = request();
    let operation_id = engine
        .begin_detached(request)
        .expect("persist detached operation");
    let source = BytesSource(b"canonical-car".to_vec());
    let mut backend = LocationRecoveryBackend::new(broker, [LocationPollV1::Retired]);
    backend.drop_location_response_once = true;
    for step in 0..5 {
        engine
            .advance_once(operation_id, &source, &mut backend)
            .unwrap_or_else(|error| panic!("prepare location step {step} failed: {error}"));
    }
    let prepared = store.load(operation_id).expect("prepared location journal");
    assert_eq!(prepared.phase, PublicationPhaseV1::ArchiveRegistration);
    assert_eq!(prepared.archive_location_attempts.len(), 1);
    assert!(prepared.archive_location_attempts[0].registration.is_none());
    assert!(prepared.archive_location_attempts[0].terminal.is_none());
    let first_intent = prepared.archive_location_attempts[0].intent.clone();
    let submitted_store =
        PublicationJournalStore::open(temp.path()).expect("reopen after preparation");
    let submitted_engine = PublicationEngine::new(&submitted_store);
    let error = submitted_engine
        .advance_once(operation_id, &source, &mut backend)
        .expect_err("simulate loss after the exact transaction applied");
    assert!(matches!(
        error,
        PublicationError::Backend(ref error)
            if error.code() == "ARCHIVE_LOCATION_COMMIT_RESPONSE_DROPPED"
    ));
    assert_eq!(
        submitted_store
            .load(operation_id)
            .expect("unchanged submitted journal"),
        prepared
    );
    assert_eq!(backend.applied_generations, vec![1]);
    let applied_store =
        PublicationJournalStore::open(temp.path()).expect("reopen after applied cut");
    let applied_engine = PublicationEngine::new(&applied_store);
    assert_eq!(
        applied_engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("recover exact finalized application"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::Replication)
    );
    let applied = applied_store
        .load(operation_id)
        .expect("applied location journal");
    let applied_attempt = applied.archive_location_attempts[0].clone();
    assert_eq!(applied_attempt.intent, first_intent);
    assert!(applied_attempt.registration.is_some());
    assert!(applied_attempt.terminal.is_none());
    assert_eq!(
        applied_engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("persist authoritative retirement"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
    );
    let retired = applied_store
        .load(operation_id)
        .expect("retired location journal");
    assert_eq!(retired.archive_location_attempts.len(), 1);
    assert_eq!(
        retired.archive_location_attempts[0].intent,
        applied_attempt.intent
    );
    assert_eq!(
        retired.archive_location_attempts[0].registration,
        applied_attempt.registration
    );
    assert!(retired.archive_location_attempts[0].terminal.is_some());
    assert!(retired.replication.is_none());
    assert!(retired.readbacks.is_empty());
    let replacement_store =
        PublicationJournalStore::open(temp.path()).expect("reopen after retirement");
    let replacement_engine = PublicationEngine::new(&replacement_store);
    assert_eq!(
        replacement_engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("persist replacement exact intent"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
    );
    let replacement_prepared = replacement_store
        .load(operation_id)
        .expect("replacement intent journal");
    assert_eq!(replacement_prepared.archive_location_attempts.len(), 2);
    assert_eq!(
        replacement_prepared.archive_location_attempts[0],
        retired.archive_location_attempts[0]
    );
    let second = &replacement_prepared.archive_location_attempts[1];
    assert_eq!(second.generation, 2);
    assert_ne!(second.intent.location_id, first_intent.location_id);
    assert_ne!(
        second.intent.transaction_hash,
        first_intent.transaction_hash
    );
    assert!(second.registration.is_none());
    assert!(second.terminal.is_none());
    assert_eq!(
        backend.prepared_generations,
        vec![(1, Vec::new()), (2, vec![first_intent.location_id])]
    );
    assert_eq!(
        replacement_engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("finalize replacement location"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::Replication)
    );
    let replacement = replacement_store
        .load(operation_id)
        .expect("replacement finalized journal");
    assert_eq!(
        replacement.archive_location_attempts[0],
        retired.archive_location_attempts[0]
    );
    assert!(
        replacement.archive_location_attempts[1]
            .registration
            .is_some()
    );
    assert!(replacement.archive_location_attempts[1].terminal.is_none());
}
#[cfg(unix)]
#[test]
fn retirement_is_rechecked_before_replication_and_readback() {
    for (script, expected_phase) in [
        (
            vec![LocationPollV1::Retired],
            PublicationPhaseV1::Replication,
        ),
        (
            vec![LocationPollV1::Healthy, LocationPollV1::Retired],
            PublicationPhaseV1::Readback,
        ),
    ] {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = engine
            .begin_detached(request)
            .expect("persist detached operation");
        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = LocationRecoveryBackend::new(broker, script);
        for step in 0..6 {
            engine
                .advance_once(operation_id, &source, &mut backend)
                .unwrap_or_else(|error| panic!("reach replication step {step}: {error}"));
        }
        while store.load(operation_id).expect("phase journal").phase != expected_phase {
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("advance to guarded phase");
        }
        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("authoritative retirement rotates the location"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
        );
        let retired = store.load(operation_id).expect("retired journal");
        assert!(retired.archive_location_attempts[0].terminal.is_some());
        assert!(retired.replication.is_none());
        assert!(retired.readbacks.is_empty());
        assert!(retired.submission.is_none());
    }
}
#[cfg(unix)]
#[test]
fn selected_location_renewal_requires_terminal_rotation_and_fresh_readbacks() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let engine = PublicationEngine::new(&store);
    let (request, broker) = request();
    let operation_id = engine
        .begin_detached(request)
        .expect("persist detached operation");
    let source = BytesSource(b"canonical-car".to_vec());
    let mut backend = LocationRecoveryBackend::new(
        broker,
        [
            LocationPollV1::HealthyRevisionOffset(1),
            LocationPollV1::Healthy,
            LocationPollV1::HealthyRevisionOffset(1),
            LocationPollV1::HealthyRevisionOffset(2),
        ],
    );
    for step in 0..6 {
        engine
            .advance_once(operation_id, &source, &mut backend)
            .unwrap_or_else(|error| panic!("reach replication step {step}: {error}"));
    }
    assert_eq!(
        store.load(operation_id).expect("replication journal").phase,
        PublicationPhaseV1::Replication
    );
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("persist renewed healthy location"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::Readback)
    );
    let renewed = store.load(operation_id).expect("renewed journal");
    assert_eq!(
        renewed
            .replication
            .as_ref()
            .expect("renewed replication")
            .finalized_page
            .items[0]
            .revision,
        3
    );
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("stale finalized poll remains retryable"),
        PublicationAdvanceV1::Pending(PublicationPhaseV1::Readback)
    );
    assert_eq!(
        store.load(operation_id).expect("unchanged stale journal"),
        renewed
    );
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("exact journaled revision resumes readback"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::ReleaseSubmission)
    );
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("changed selected location keeps the stale-readback intent unsent"),
        PublicationAdvanceV1::Pending(PublicationPhaseV1::ReleaseSubmission)
    );
    let guarded = store.load(operation_id).expect("guarded journal");
    assert_eq!(
        guarded.release_submission_attempts[0]
            .intent
            .preparation
            .replication
            .finalized_page
            .items[0]
            .revision,
        3
    );
    assert_eq!(backend.release_preparations, 1);
    assert_eq!(backend.release_submissions, 0);
}
#[cfg(unix)]
#[test]
fn stale_pre_send_poll_preserves_the_live_intent_and_replays_identical_bytes() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let engine = PublicationEngine::new(&store);
    let (request, broker) = request();
    let operation_id = engine
        .begin_detached(request)
        .expect("persist detached operation");
    let source = BytesSource(b"canonical-car".to_vec());
    let mut backend = LocationRecoveryBackend::new(
        broker,
        [
            LocationPollV1::HealthyRevisionOffset(2),
            LocationPollV1::HealthyRevisionOffset(2),
            LocationPollV1::HealthyRevisionOffset(1),
            LocationPollV1::HealthyRevisionOffset(2),
        ],
    );
    for step in 0..8 {
        engine
            .advance_once(operation_id, &source, &mut backend)
            .unwrap_or_else(|error| panic!("reach release submission step {step}: {error}"));
    }
    let guarded = store.load(operation_id).expect("release journal");
    assert_eq!(guarded.phase, PublicationPhaseV1::ReleaseSubmission);
    assert_eq!(guarded.readbacks.len(), 2);
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("stale healthy page remains retryable"),
        PublicationAdvanceV1::Pending(PublicationPhaseV1::ReleaseSubmission)
    );
    assert_eq!(
        store.load(operation_id).expect("unchanged journal"),
        guarded
    );
    assert_eq!(backend.release_submissions, 0);
    assert_eq!(backend.release_preparations, 1);
    assert_eq!(backend.release_intents.len(), 1);
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("exact checkpoint permits submission"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::FinalVerification)
    );
    assert_eq!(backend.release_submissions, 1);
    assert_eq!(backend.release_preparations, 1);
    assert_eq!(backend.release_intents.len(), 2);
    assert_eq!(backend.release_intents[0], backend.release_intents[1]);
}
#[cfg(unix)]
#[test]
fn authoritative_pending_status_never_resigns_or_replaces_the_live_transaction() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let engine = PublicationEngine::new(&store);
    let (request, broker) = request();
    let operation_id = engine
        .begin_detached(request)
        .expect("persist detached operation");
    let source = BytesSource(b"canonical-car".to_vec());
    let mut backend = LocationRecoveryBackend::new(
        broker,
        [
            LocationPollV1::Healthy,
            LocationPollV1::Healthy,
            LocationPollV1::Healthy,
            LocationPollV1::Healthy,
            LocationPollV1::Healthy,
        ],
    );
    backend.release_pending_responses = 2;
    for step in 0..8 {
        engine
            .advance_once(operation_id, &source, &mut backend)
            .unwrap_or_else(|error| panic!("reach release submission step {step}: {error}"));
    }
    let live = store.load(operation_id).expect("live release journal");
    let digest = live.release_submission_attempts[0]
        .intent
        .signed_transaction_digest;
    for _ in 0..2 {
        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("pending exact status remains retryable"),
            PublicationAdvanceV1::Pending(PublicationPhaseV1::ReleaseSubmission)
        );
        assert_eq!(store.load(operation_id).expect("unchanged journal"), live);
    }
    assert_eq!(backend.release_preparations, 1);
    assert_eq!(backend.release_submissions, 0);
    assert_eq!(backend.release_intents, vec![digest, digest]);
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("the same exact transaction is eventually applied"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::FinalVerification)
    );
    assert_eq!(backend.release_preparations, 1);
    assert_eq!(backend.release_submissions, 1);
    assert_eq!(backend.release_intents, vec![digest, digest, digest]);
}
#[cfg(unix)]
#[test]
fn lost_release_response_restarts_from_the_same_journaled_transaction() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let engine = PublicationEngine::new(&store);
    let (request, broker) = request();
    let operation_id = engine
        .begin_detached(request)
        .expect("persist detached operation");
    let source = BytesSource(b"canonical-car".to_vec());
    let mut backend = LocationRecoveryBackend::new(
        broker,
        [
            LocationPollV1::Healthy,
            LocationPollV1::Healthy,
            LocationPollV1::HealthyDirectoryAdvance,
            LocationPollV1::HealthyDirectoryAdvance,
        ],
    );
    for step in 0..8 {
        engine
            .advance_once(operation_id, &source, &mut backend)
            .unwrap_or_else(|error| panic!("reach release submission step {step}: {error}"));
    }
    let before = store.load(operation_id).expect("release journal");
    assert_eq!(before.phase, PublicationPhaseV1::ReleaseSubmission);
    let exact_digest = before.release_submission_attempts[0]
        .intent
        .signed_transaction_digest;
    backend.drop_release_response_once = true;
    let error = engine
        .advance_once(operation_id, &source, &mut backend)
        .expect_err("a lost response leaves the exact live intent durable");
    assert!(matches!(
        error,
        PublicationError::Backend(ref error)
            if error.code() == "RELEASE_COMMIT_RESPONSE_DROPPED"
    ));
    assert_eq!(store.load(operation_id).expect("unchanged journal"), before);
    assert_eq!(backend.release_preparations, 1);
    assert_eq!(backend.release_submissions, 1);
    let reopened_store =
        PublicationJournalStore::open(temp.path()).expect("reopen publication journal");
    let reopened_engine = PublicationEngine::new(&reopened_store);
    assert_eq!(
        reopened_engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("status-first recovery observes the exact applied transaction"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::FinalVerification)
    );
    assert_eq!(backend.release_submissions, 1);
    assert_eq!(backend.release_preparations, 1);
    assert_eq!(backend.release_intents, vec![exact_digest, exact_digest]);
}
#[cfg(unix)]
#[test]
fn stale_retirement_is_pending_in_readback() {
    for (script, guarded_phase) in [(
        vec![
            LocationPollV1::HealthyRevisionOffset(2),
            LocationPollV1::Retired,
            LocationPollV1::RetiredRevisionOffset(2),
        ],
        PublicationPhaseV1::Readback,
    )] {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = engine
            .begin_detached(request)
            .expect("persist detached operation");
        let source = BytesSource(b"canonical-car".to_vec());
        let mut backend = LocationRecoveryBackend::new(broker, script);
        for step in 0..6 {
            engine
                .advance_once(operation_id, &source, &mut backend)
                .unwrap_or_else(|error| panic!("reach replication step {step}: {error}"));
        }
        while store.load(operation_id).expect("phase journal").phase != guarded_phase {
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("advance to guarded phase");
        }
        let guarded = store.load(operation_id).expect("guarded journal");
        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("stale retirement remains retryable"),
            PublicationAdvanceV1::Pending(guarded_phase)
        );
        assert_eq!(
            store.load(operation_id).expect("unchanged journal"),
            guarded
        );
        assert_eq!(
            engine
                .advance_once(operation_id, &source, &mut backend)
                .expect("strictly later retirement permits rotation"),
            PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
        );
        let retired = store.load(operation_id).expect("retired journal");
        let attempt = &retired.archive_location_attempts[0];
        assert!(attempt.terminal.is_some());
        assert!(matches!(
            &attempt.terminal_floor,
            Some(PublicationArchiveLocationTerminalFloorV1::Replication(_))
        ));
        let reopened = PublicationJournalStore::open(temp.path())
            .expect("reopen journal store")
            .load(operation_id)
            .expect("revalidate terminal against durable replication floor");
        assert_eq!(reopened, retired);
        let mut regressed = retired;
        let registration = regressed.archive_location_attempts[0]
            .registration
            .as_ref()
            .expect("finalized registration");
        let stale_terminal = retired_location_terminal(registration);
        regressed.archive_location_attempts[0]
            .terminal
            .as_mut()
            .expect("persisted terminal")
            .finalized_page = stale_terminal.finalized_page;
        assert!(matches!(
            regressed.validate(),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Replication,
                ..
            })
        ));
    }
}
#[cfg(unix)]
#[test]
fn stale_post_rejection_retirement_preserves_the_latest_checkpoint() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let engine = PublicationEngine::new(&store);
    let (request, broker) = request();
    let operation_id = engine
        .begin_detached(request)
        .expect("persist detached operation");
    let source = BytesSource(b"canonical-car".to_vec());
    let mut backend = LocationRecoveryBackend::new(
        broker,
        [
            LocationPollV1::HealthyRevisionOffset(2),
            LocationPollV1::HealthyRevisionOffset(2),
            LocationPollV1::HealthyRevisionOffset(2),
            LocationPollV1::Retired,
            LocationPollV1::HealthyRevisionOffset(2),
            LocationPollV1::RetiredRevisionOffset(2),
        ],
    );
    backend.reject_release = true;
    for step in 0..8 {
        engine
            .advance_once(operation_id, &source, &mut backend)
            .unwrap_or_else(|error| panic!("reach release submission step {step}: {error}"));
    }
    let guarded = store.load(operation_id).expect("release journal");
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("persist the exact rejected transaction outcome"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::ReleaseSubmission)
    );
    let terminal = store.load(operation_id).expect("terminal release journal");
    assert_ne!(terminal, guarded);
    assert!(matches!(
        terminal.release_submission_attempts[0].outcome,
        Some(PublicationReleaseSubmissionOutcomeV1::Terminal(_))
    ));
    assert_eq!(backend.release_submissions, 1);
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("stale post-rejection retirement remains retryable"),
        PublicationAdvanceV1::Pending(PublicationPhaseV1::ReleaseSubmission)
    );
    assert_eq!(
        store.load(operation_id).expect("unchanged journal"),
        terminal
    );
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("unchanged location cannot authorize a new signature"),
        PublicationAdvanceV1::Pending(PublicationPhaseV1::ReleaseSubmission)
    );
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("later post-rejection retirement permits rotation"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
    );
    assert_eq!(backend.release_submissions, 1);
}
#[cfg(unix)]
#[test]
fn rejected_release_never_resigns_against_stale_or_unchanged_location_state() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let engine = PublicationEngine::new(&store);
    let (request, broker) = request();
    let operation_id = engine
        .begin_detached(request)
        .expect("persist detached operation");
    let source = BytesSource(b"canonical-car".to_vec());
    let mut backend = LocationRecoveryBackend::new(
        broker,
        [
            LocationPollV1::HealthyRevisionOffset(2),
            LocationPollV1::HealthyRevisionOffset(2),
            LocationPollV1::HealthyRevisionOffset(2),
            LocationPollV1::HealthyRevisionOffset(1),
            LocationPollV1::HealthyRevisionOffset(2),
            LocationPollV1::HealthyRevisionOffset(2),
        ],
    );
    backend.reject_release = true;
    for step in 0..8 {
        engine
            .advance_once(operation_id, &source, &mut backend)
            .unwrap_or_else(|error| panic!("reach release submission step {step}: {error}"));
    }
    let guarded = store.load(operation_id).expect("release journal");
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("persist the authoritative rejection and exact absence"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::ReleaseSubmission)
    );
    let terminal = store.load(operation_id).expect("terminal release journal");
    assert_ne!(terminal, guarded);
    assert_eq!(backend.release_submissions, 1);
    assert_eq!(backend.release_preparations, 1);
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("stale post-rejection page remains retryable"),
        PublicationAdvanceV1::Pending(PublicationPhaseV1::ReleaseSubmission)
    );
    assert_eq!(
        store.load(operation_id).expect("unchanged journal"),
        terminal
    );
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("unchanged current location cannot authorize a successor"),
        PublicationAdvanceV1::Pending(PublicationPhaseV1::ReleaseSubmission)
    );
    assert_eq!(backend.release_submissions, 1);
    assert_eq!(backend.release_preparations, 1);
}
#[test]
fn checkpoint_allows_higher_target_revision_at_equal_location_height_on_a_newer_page() {
    let (request, broker) = request();
    let registration = registration(&request, &broker);
    let previous = replication_checkpoint(&request, &registration, 3);
    let mut current = replication_checkpoint_with_revision_offset(&request, &registration, 1);
    current
        .finalized_page
        .items
        .iter_mut()
        .find(|location| location.location_id == registration.location_id())
        .expect("registered fixture location")
        .finalized_height = previous
        .location(&registration)
        .expect("previous fixture location")
        .finalized_height;
    assert_eq!(
        replication_checkpoint_progress(&request, &registration, &previous, &current)
            .expect("newer full page authenticates the higher local revision"),
        PublicationLocationProgressV1::Current
    );
}
#[cfg(unix)]
#[test]
fn rejected_release_rotates_only_after_post_rejection_retirement_evidence() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let engine = PublicationEngine::new(&store);
    let (request, broker) = request();
    let operation_id = engine
        .begin_detached(request)
        .expect("persist detached operation");
    let source = BytesSource(b"canonical-car".to_vec());
    let mut backend = LocationRecoveryBackend::new(
        broker,
        [
            LocationPollV1::Healthy,
            LocationPollV1::Healthy,
            LocationPollV1::Healthy,
            LocationPollV1::Retired,
        ],
    );
    backend.reject_release = true;
    for step in 0..8 {
        engine
            .advance_once(operation_id, &source, &mut backend)
            .unwrap_or_else(|error| panic!("reach release submission step {step}: {error}"));
    }
    assert_eq!(
        store.load(operation_id).expect("release journal").phase,
        PublicationPhaseV1::ReleaseSubmission
    );
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("persist exact rejection before any location rotation"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::ReleaseSubmission)
    );
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("post-rejection retirement permits rotation"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
    );
    let retired = store.load(operation_id).expect("post-rejection journal");
    assert!(retired.archive_location_attempts[0].terminal.is_some());
    assert!(retired.submission.is_none());
    assert!(retired.readbacks.is_empty());
}
#[cfg(unix)]
#[test]
fn expired_receipt_is_refreshed_only_before_registration_intent() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let engine = PublicationEngine::new(&store);
    let (request, broker) = request();
    let operation_id = engine
        .begin_detached(request)
        .expect("persist detached operation");
    let source = BytesSource(b"canonical-car".to_vec());
    let mut backend = ArchiveRecoveryBackend {
        broker,
        now_ms: 1_000,
        staged_receipts: Vec::new(),
        prepare_calls: 0,
        registration_calls: 0,
        pin_calls: 0,
        archive_committed: false,
        drop_commit_response_once: false,
        return_conflicting_archive: false,
        registration_mode: ArchiveRecoveryMode::Commit,
    };
    engine
        .advance_once(operation_id, &source, &mut backend)
        .expect("validate");
    engine
        .advance_once(operation_id, &source, &mut backend)
        .expect("stage first receipt");
    backend.now_ms = 1_101;
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("discard expired receipt before intent"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::SeedIngress)
    );
    let reset = store.load(operation_id).expect("receipt reset journal");
    assert!(reset.staging_receipt.is_none());
    assert!(reset.archive_registration_attempts.is_empty());
    engine
        .advance_once(operation_id, &source, &mut backend)
        .expect("stage fresh receipt");
    engine
        .advance_once(operation_id, &source, &mut backend)
        .expect("persist intent for fresh receipt");
    assert_eq!(backend.staged_receipts.len(), 2);
    assert_eq!(backend.prepare_calls, 1);
}
#[cfg(unix)]
#[test]
fn expired_unsubmitted_intent_rotates_only_after_authoritative_terminal_absence() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let engine = PublicationEngine::new(&store);
    let (request, broker) = request();
    let operation_id = engine
        .begin_detached(request)
        .expect("persist detached operation");
    let source = BytesSource(b"canonical-car".to_vec());
    let mut backend = ArchiveRecoveryBackend {
        broker,
        now_ms: 1_000,
        staged_receipts: Vec::new(),
        prepare_calls: 0,
        registration_calls: 0,
        pin_calls: 0,
        archive_committed: false,
        drop_commit_response_once: false,
        return_conflicting_archive: false,
        registration_mode: ArchiveRecoveryMode::ExpiredAbsent,
    };
    engine
        .advance_once(operation_id, &source, &mut backend)
        .expect("validate");
    engine
        .advance_once(operation_id, &source, &mut backend)
        .expect("stage first receipt");
    engine
        .advance_once(operation_id, &source, &mut backend)
        .expect("persist first exact intent");
    let before_crash = store.load(operation_id).expect("first intent journal");
    let first_attempt = before_crash.archive_registration_attempts[0].clone();
    assert_eq!(backend.registration_calls, 0);
    backend.now_ms = first_attempt.intent.staging_receipt.payload.expires_at_ms + 1;
    let reopened = PublicationJournalStore::open(temp.path()).expect("reopen journal store");
    let resumed = PublicationEngine::new(&reopened);
    assert_eq!(
        resumed
            .advance_once(operation_id, &source, &mut backend)
            .expect("persist authoritative expiration and absence"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::SeedIngress)
    );
    let terminal = reopened
        .load(operation_id)
        .expect("terminal first generation");
    assert_eq!(terminal.archive_registration_attempts.len(), 1);
    assert_eq!(
        terminal.archive_registration_attempts[0].intent,
        first_attempt.intent
    );
    assert!(terminal.archive_registration_attempts[0].terminal.is_some());
    assert!(terminal.staging_receipt.is_none());
    resumed
        .advance_once(operation_id, &source, &mut backend)
        .expect("stage replacement receipt");
    resumed
        .advance_once(operation_id, &source, &mut backend)
        .expect("append replacement exact intent");
    let replacement = reopened
        .load(operation_id)
        .expect("replacement generation journal");
    assert_eq!(replacement.archive_registration_attempts.len(), 2);
    assert_eq!(
        replacement.archive_registration_attempts[0],
        terminal.archive_registration_attempts[0]
    );
    assert!(
        replacement.archive_registration_attempts[1]
            .terminal
            .is_none()
    );
    assert_ne!(
        replacement.archive_registration_attempts[0]
            .intent
            .transaction_hash,
        replacement.archive_registration_attempts[1]
            .intent
            .transaction_hash
    );
    assert_eq!(backend.staged_receipts.len(), 2);
    assert_eq!(backend.prepare_calls, 2);
}
#[cfg(unix)]
#[test]
fn unknown_or_pending_application_state_never_rotates_the_exact_intent() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let engine = PublicationEngine::new(&store);
    let (request, broker) = request();
    let operation_id = engine
        .begin_detached(request)
        .expect("persist detached operation");
    let source = BytesSource(b"canonical-car".to_vec());
    let mut backend = ArchiveRecoveryBackend {
        broker,
        now_ms: 1_000,
        staged_receipts: Vec::new(),
        prepare_calls: 0,
        registration_calls: 0,
        pin_calls: 0,
        archive_committed: false,
        drop_commit_response_once: false,
        return_conflicting_archive: false,
        registration_mode: ArchiveRecoveryMode::Pending,
    };
    engine
        .advance_once(operation_id, &source, &mut backend)
        .expect("validate");
    engine
        .advance_once(operation_id, &source, &mut backend)
        .expect("stage");
    engine
        .advance_once(operation_id, &source, &mut backend)
        .expect("persist exact intent");
    let before = store.load(operation_id).expect("intent journal");
    backend.now_ms = 10_000;
    assert_eq!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("unknown application state remains pending"),
        PublicationAdvanceV1::Pending(PublicationPhaseV1::ArchiveRegistration)
    );
    assert_eq!(store.load(operation_id).expect("unchanged journal"), before);
    assert_eq!(backend.staged_receipts.len(), 1);
    assert_eq!(backend.prepare_calls, 1);
}
#[cfg(unix)]
#[test]
fn archive_registration_attempt_generation_is_strictly_bounded() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let (request, broker) = request();
    let operation_id = request.operation_id();
    let mut journal = PublicationJournalV1::new(request.clone()).expect("publication journal");
    journal.validation = Some(validation_evidence(&request));
    journal.phase = PublicationPhaseV1::SeedIngress;
    for generation in 1..=MUSUBI_MAX_ARCHIVE_REGISTRATION_ATTEMPTS_V1 {
        let generation_u64 = u64::try_from(generation).expect("generation fits u64");
        let issued_at_ms = 1_000 + generation_u64 * 1_000;
        let receipt = signed_receipt_at(
            &request.receipt_binding(),
            &broker,
            issued_at_ms,
            issued_at_ms + 100,
        );
        let intent = registration_intent(operation_id, &request, receipt);
        let finalized_height = 60 + generation_u64;
        let terminal = PublicationArchiveRegistrationTerminalV1::registry_expired(
            &intent,
            Some(finalized_height),
            archive_absence_evidence(&request, finalized_height),
        );
        journal
            .archive_registration_attempts
            .push(PublicationArchiveRegistrationAttemptV1 {
                generation: u8::try_from(generation).expect("generation fits u8"),
                intent,
                terminal: Some(terminal),
            });
    }
    journal
        .validate()
        .expect("maximum attempt generation is valid");
    store
        .write(&journal)
        .expect("persist maximum-generation journal");
    let source = BytesSource(b"canonical-car".to_vec());
    let mut backend = ArchiveRecoveryBackend {
        broker,
        now_ms: 100_000,
        staged_receipts: Vec::new(),
        prepare_calls: 0,
        registration_calls: 0,
        pin_calls: 0,
        archive_committed: false,
        drop_commit_response_once: false,
        return_conflicting_archive: false,
        registration_mode: ArchiveRecoveryMode::Commit,
    };
    let error = PublicationEngine::new(&store)
        .advance_once(operation_id, &source, &mut backend)
        .expect_err("a ninth generation must not be staged");
    assert!(matches!(
        error,
        PublicationError::Backend(ref error)
            if error.code() == "ARCHIVE_REGISTRATION_ATTEMPT_LIMIT_REACHED"
                && error.class() == PublicationBackendFailureClass::Permanent
    ));
    assert!(backend.staged_receipts.is_empty());
    let mut oversized = journal;
    let previous = oversized
        .archive_registration_attempts
        .last()
        .expect("maximum generation")
        .clone();
    let mut ninth = previous;
    ninth.generation = u8::try_from(MUSUBI_MAX_ARCHIVE_REGISTRATION_ATTEMPTS_V1 + 1)
        .expect("ninth generation fits u8");
    oversized.archive_registration_attempts.push(ninth);
    assert!(matches!(
        oversized.validate(),
        Err(PublicationError::InvalidJournal(ref reason))
            if reason.contains("attempt bound")
    ));
}
#[cfg(unix)]
#[test]
fn archive_location_attempt_generation_is_bounded_and_encoded_below_journal_limit() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let (request, broker) = request();
    let operation_id = request.operation_id();
    let receipt = signed_receipt(&request.receipt_binding(), &broker);
    let archive_intent = registration_intent(operation_id, &request, receipt.clone());
    let registered = registered_archive(&request, &broker, &archive_intent);
    let mut journal = PublicationJournalV1::new(request.clone()).expect("publication journal");
    journal.validation = Some(validation_evidence(&request));
    journal.staging_receipt = Some(receipt);
    journal
        .archive_registration_attempts
        .push(PublicationArchiveRegistrationAttemptV1::new(
            1,
            archive_intent,
        ));
    journal.registered_archive = Some(registered.clone());
    journal.phase = PublicationPhaseV1::ArchiveRegistration;
    for generation in 1..=MUSUBI_MAX_ARCHIVE_LOCATION_ATTEMPTS_V1 {
        let generation = u8::try_from(generation).expect("generation fits u8");
        let registration =
            location_registration_generation(operation_id, &request, &registered, generation);
        let replication = replication_checkpoint(&request, &registration, 3);
        let terminal = retired_location_terminal(&registration);
        journal
            .archive_location_attempts
            .push(PublicationArchiveLocationAttemptV1 {
                generation,
                intent: registration.intent.clone(),
                registration: Some(registration),
                terminal: Some(terminal),
                terminal_floor: Some(PublicationArchiveLocationTerminalFloorV1::Replication(
                    replication,
                )),
            });
    }
    journal
        .validate()
        .expect("maximum location history is valid");
    let encoded = norito::encode_canonical(&journal).expect("encode bounded journal");
    assert!(encoded.len() <= MAX_JOURNAL_BYTES_USIZE);
    store.write(&journal).expect("persist bounded journal");
    let persisted_bytes = fs::metadata(temp.path().join(journal_relative_path(operation_id)))
        .expect("bounded journal metadata")
        .len();
    assert!(persisted_bytes <= MAX_JOURNAL_BYTES);
    assert_eq!(
        store.load(operation_id).expect("reload bounded journal"),
        journal
    );
    let mut rewritten = journal.clone();
    rewritten.archive_location_attempts[0]
        .terminal
        .as_mut()
        .expect("first terminal")
        .transaction_hash = [0xee; 32];
    assert!(!archive_location_attempts_are_append_only(
        &journal.archive_location_attempts,
        &rewritten.archive_location_attempts,
    ));
    let generation =
        u8::try_from(MUSUBI_MAX_ARCHIVE_LOCATION_ATTEMPTS_V1 + 1).expect("ninth fits u8");
    let registration =
        location_registration_generation(operation_id, &request, &registered, generation);
    let replication = replication_checkpoint(&request, &registration, 3);
    let terminal = retired_location_terminal(&registration);
    let mut oversized = journal;
    oversized
        .archive_location_attempts
        .push(PublicationArchiveLocationAttemptV1 {
            generation,
            intent: registration.intent.clone(),
            registration: Some(registration),
            terminal: Some(terminal),
            terminal_floor: Some(PublicationArchiveLocationTerminalFloorV1::Replication(
                replication,
            )),
        });
    assert!(matches!(
        oversized.validate(),
        Err(PublicationError::InvalidJournal(ref reason))
            if reason.contains("archive-location attempt bound")
    ));
}
#[cfg(unix)]
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the test covers substitution at each terminal-to-replacement snapshot boundary"
)]
fn terminal_and_replacement_pages_reject_same_snapshot_or_revision_substitution() {
    let (request, broker) = request();
    let operation_id = request.operation_id();
    let receipt = signed_receipt(&request.receipt_binding(), &broker);
    let archive_intent = registration_intent(operation_id, &request, receipt.clone());
    let registered = registered_archive(&request, &broker, &archive_intent);
    let first = location_registration_generation(operation_id, &request, &registered, 1);
    let first_terminal = retired_location_terminal(&first);
    let second = location_registration_generation(operation_id, &request, &registered, 2);
    let second_attempt = PublicationArchiveLocationAttemptV1 {
        generation: 2,
        intent: second.intent.clone(),
        registration: None,
        terminal: None,
        terminal_floor: None,
    };
    let prior_location_ids = [first.location_id()];
    let active_second_attempt = PublicationArchiveLocationAttemptV1 {
        generation: 2,
        intent: second.intent.clone(),
        registration: Some(second.clone()),
        terminal: None,
        terminal_floor: None,
    };
    let mut equal_index_retirement = retired_location_terminal(&second);
    equal_index_retirement
        .finalized_page
        .snapshot
        .index_revision = second.finalized_page.snapshot.index_revision;
    equal_index_retirement
        .validate_for(
            operation_id,
            &request,
            &registered,
            &active_second_attempt,
            &prior_location_ids,
            &PublicationArchiveLocationTerminalFloorV1::Registered,
        )
        .expect("retirement may preserve the resolver index revision");
    let mut lower_index_retirement = equal_index_retirement;
    lower_index_retirement
        .finalized_page
        .snapshot
        .index_revision -= 1;
    assert!(
        lower_index_retirement
            .validate_for(
                operation_id,
                &request,
                &registered,
                &active_second_attempt,
                &prior_location_ids,
                &PublicationArchiveLocationTerminalFloorV1::Registered,
            )
            .is_err()
    );
    let exact_expiry = PublicationArchiveLocationTerminalV1 {
        transaction_hash: second.intent.transaction_hash,
        reason: PublicationArchiveLocationTerminalReasonV1::RegistryExpired { block_height: None },
        finalized_page: second.intent.prepared_page.clone(),
    };
    exact_expiry
        .validate_for(
            operation_id,
            &request,
            &registered,
            &second_attempt,
            &prior_location_ids,
            &PublicationArchiveLocationTerminalFloorV1::Prepared,
        )
        .expect("unchanged prepared snapshot proves exact expiry absence");
    let mut same_snapshot_substituted = exact_expiry.clone();
    let mut unrelated = second.location().expect("second location fixture").clone();
    unrelated.location_id = MusubiArchiveLocationIdV1::new([0xe5; 32]);
    unrelated.finalized_height = same_snapshot_substituted
        .finalized_page
        .snapshot
        .finalized_height;
    unrelated.revision = same_snapshot_substituted
        .finalized_page
        .archive
        .location_revision
        + 1;
    same_snapshot_substituted
        .finalized_page
        .archive
        .location_revision += 1;
    same_snapshot_substituted
        .finalized_page
        .archive
        .location_ids = vec![unrelated.location_id];
    same_snapshot_substituted.finalized_page.items = vec![unrelated.clone()];
    assert!(
        same_snapshot_substituted
            .validate_for(
                operation_id,
                &request,
                &registered,
                &second_attempt,
                &prior_location_ids,
                &PublicationArchiveLocationTerminalFloorV1::Prepared,
            )
            .is_err()
    );
    let mut same_revision_substituted = exact_expiry;
    same_revision_substituted
        .finalized_page
        .snapshot
        .finalized_height += 1;
    same_revision_substituted
        .finalized_page
        .snapshot
        .finalized_block_hash = [0xe6; 32];
    unrelated.revision = same_revision_substituted
        .finalized_page
        .archive
        .location_revision;
    unrelated.finalized_height = same_revision_substituted
        .finalized_page
        .snapshot
        .finalized_height;
    same_revision_substituted
        .finalized_page
        .archive
        .location_ids = vec![unrelated.location_id];
    same_revision_substituted.finalized_page.items = vec![unrelated.clone()];
    assert!(
        same_revision_substituted
            .validate_for(
                operation_id,
                &request,
                &registered,
                &second_attempt,
                &prior_location_ids,
                &PublicationArchiveLocationTerminalFloorV1::Prepared,
            )
            .is_err()
    );
    let mut journal = PublicationJournalV1::new(request.clone()).expect("publication journal");
    journal.validation = Some(validation_evidence(&request));
    journal.staging_receipt = Some(receipt);
    journal
        .archive_registration_attempts
        .push(PublicationArchiveRegistrationAttemptV1::new(
            1,
            archive_intent,
        ));
    journal.registered_archive = Some(registered);
    journal.archive_location_attempts = vec![
        PublicationArchiveLocationAttemptV1 {
            generation: 1,
            intent: first.intent.clone(),
            registration: Some(first),
            terminal: Some(first_terminal),
            terminal_floor: Some(PublicationArchiveLocationTerminalFloorV1::Registered),
        },
        second_attempt,
    ];
    journal.phase = PublicationPhaseV1::ArchiveRegistration;
    journal
        .validate()
        .expect("exact terminal-to-prepared checkpoint");
    let mut replacement_substituted = journal;
    let prepared = &mut replacement_substituted.archive_location_attempts[1]
        .intent
        .prepared_page;
    prepared.snapshot.finalized_height += 1;
    prepared.snapshot.finalized_block_hash = [0xe7; 32];
    unrelated.revision = prepared.archive.location_revision;
    unrelated.finalized_height = prepared.snapshot.finalized_height;
    prepared.archive.location_ids = vec![unrelated.location_id];
    prepared.items = vec![unrelated];
    assert!(matches!(
        replacement_substituted.validate(),
        Err(PublicationError::InvalidJournal(ref reason))
            if reason.contains("regressed prior terminal finality")
    ));
    let encoded =
        norito::encode_canonical(&replacement_substituted).expect("encode substituted journal");
    let temp = tempdir().expect("substituted journal root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    store
        .root
        .replace(&journal_relative_path(operation_id), &encoded)
        .expect("persist substituted restart image");
    assert!(matches!(
        store.load(operation_id),
        Err(PublicationError::InvalidJournal(ref reason))
            if reason.contains("regressed prior terminal finality")
    ));
}
#[cfg(unix)]
#[test]
fn conflicting_authoritative_archive_never_reaches_pin_coordination() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let engine = PublicationEngine::new(&store);
    let (request, broker) = request();
    let operation_id = engine
        .begin_detached(request)
        .expect("persist detached operation");
    let source = BytesSource(b"canonical-car".to_vec());
    let mut backend = ArchiveRecoveryBackend {
        broker,
        now_ms: 1_000,
        staged_receipts: Vec::new(),
        prepare_calls: 0,
        registration_calls: 0,
        pin_calls: 0,
        archive_committed: false,
        drop_commit_response_once: false,
        return_conflicting_archive: true,
        registration_mode: ArchiveRecoveryMode::Commit,
    };
    engine
        .advance_once(operation_id, &source, &mut backend)
        .expect("validate");
    engine
        .advance_once(operation_id, &source, &mut backend)
        .expect("stage");
    engine
        .advance_once(operation_id, &source, &mut backend)
        .expect("persist intent");
    assert!(matches!(
        engine.advance_once(operation_id, &source, &mut backend),
        Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::ArchiveRegistration,
            ..
        })
    ));
    let unchanged = store.load(operation_id).expect("unchanged intent journal");
    assert_eq!(unchanged.archive_registration_attempts.len(), 1);
    assert!(
        unchanged.archive_registration_attempts[0]
            .terminal
            .is_none()
    );
    assert!(unchanged.registered_archive.is_none());
    assert_eq!(backend.pin_calls, 0);
}
#[cfg(unix)]
#[test]
fn detached_resume_crosses_all_seven_phases_and_reuses_amx_submission() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let engine = PublicationEngine::new(&store);
    let (request, broker) = request();
    let operation_id = request.operation_id();
    let source = BytesSource(b"canonical-car".to_vec());
    let mut backend = CompleteBackend {
        broker,
        replication_pending_once: true,
        finality_pending_once: true,
        substitute_readback: false,
        substitute_all_readbacks: false,
        readback_backend_failure: None,
        readback_providers: Vec::new(),
        submissions: 0,
    };
    assert_eq!(
        engine
            .publish(request, &source, &mut backend)
            .expect("start publication"),
        PublicationAdvanceV1::Pending(PublicationPhaseV1::Replication)
    );
    let replication_wait = store.load(operation_id).expect("replication journal");
    assert_eq!(replication_wait.phase, PublicationPhaseV1::Replication);
    assert_eq!(replication_wait.revision, 7);
    assert_eq!(
        engine
            .resume(operation_id, &source, &mut backend)
            .expect("resume through AMX"),
        PublicationAdvanceV1::Pending(PublicationPhaseV1::FinalVerification)
    );
    assert_eq!(backend.submissions, 1);
    let finality_wait = store.load(operation_id).expect("finality journal");
    assert_eq!(finality_wait.phase, PublicationPhaseV1::FinalVerification);
    assert_eq!(finality_wait.revision, 10);
    let completed = engine
        .resume(operation_id, &source, &mut backend)
        .expect("complete final verification");
    let PublicationAdvanceV1::Complete(result) = completed else {
        panic!("publication should be complete");
    };
    assert_eq!(result.operation_id, operation_id);
    assert_eq!(backend.submissions, 1);
    assert!(matches!(
        engine
            .resume(operation_id, &source, &mut backend)
            .expect("idempotent completed resume"),
        PublicationAdvanceV1::Complete(_)
    ));
    assert_eq!(backend.submissions, 1);
}
#[cfg(unix)]
#[test]
fn trait_backed_readback_skips_corrupt_provider_and_uses_later_quorum() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let engine = PublicationEngine::new(&store);
    let (request, broker) = request();
    let operation_id = request.operation_id();
    let source = BytesSource(b"canonical-car".to_vec());
    let mut backend = CompleteBackend {
        broker,
        replication_pending_once: false,
        finality_pending_once: false,
        substitute_readback: true,
        substitute_all_readbacks: false,
        readback_backend_failure: None,
        readback_providers: Vec::new(),
        submissions: 0,
    };
    assert!(matches!(
        engine
            .publish(request, &source, &mut backend)
            .expect("later providers satisfy the readback floor"),
        PublicationAdvanceV1::Complete(_)
    ));
    assert_eq!(backend.submissions, 1);
    assert_eq!(
        backend.readback_providers,
        vec![
            ProviderId::new([1; 32]),
            ProviderId::new([2; 32]),
            ProviderId::new([3; 32]),
        ]
    );
    let journal = store.load(operation_id).expect("completed journal");
    assert_eq!(
        journal
            .readbacks
            .iter()
            .map(|readback| readback.provider)
            .collect::<Vec<_>>(),
        vec![ProviderId::new([2; 32]), ProviderId::new([3; 32])]
    );
    journal.validate().expect("fallback journal remains valid");
}
#[cfg(unix)]
#[test]
fn trait_backed_invalid_readback_quorum_stops_before_amx_without_journal_mutation() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let engine = PublicationEngine::new(&store);
    let (request, broker) = request();
    let operation_id = request.operation_id();
    let source = BytesSource(b"canonical-car".to_vec());
    let mut backend = CompleteBackend {
        broker,
        replication_pending_once: false,
        finality_pending_once: false,
        substitute_readback: false,
        substitute_all_readbacks: true,
        readback_backend_failure: None,
        readback_providers: Vec::new(),
        submissions: 0,
    };
    let error = engine
        .publish(request, &source, &mut backend)
        .expect_err("invalid providers cannot authorize AMX submission");
    let PublicationError::InvalidEvidence { phase, reason } = error else {
        panic!("substituted provider evidence must retain its integrity classification");
    };
    assert_eq!(phase, PublicationPhaseV1::Readback);
    assert_eq!(reason, "provider readback evidence was substituted");
    assert_eq!(backend.submissions, 0);
    assert_eq!(
        backend.readback_providers,
        vec![
            ProviderId::new([1; 32]),
            ProviderId::new([2; 32]),
            ProviderId::new([3; 32]),
        ]
    );
    let unchanged = store.load(operation_id).expect("readback journal");
    assert_eq!(unchanged.phase, PublicationPhaseV1::Readback);
    assert!(unchanged.readbacks.is_empty());
    assert!(unchanged.release_submission_attempts.is_empty());
    assert!(unchanged.submission.is_none());
    unchanged
        .validate()
        .expect("failed readbacks leave a valid journal");
    let error = engine
        .resume(operation_id, &source, &mut backend)
        .expect_err("retry still lacks two valid providers");
    assert!(matches!(
        error,
        PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::Readback,
            ..
        }
    ));
    assert_eq!(
        store.load(operation_id).expect("retried readback journal"),
        unchanged
    );
    assert_eq!(backend.submissions, 0);
}
#[cfg(unix)]
#[test]
fn trait_backed_readback_exhaustion_preserves_backend_failure_class_and_code() {
    for (class, code) in [
        (
            PublicationBackendFailureClass::Permanent,
            "READBACK_AUTHENTICATION_FAILED",
        ),
        (
            PublicationBackendFailureClass::Retryable,
            "READBACK_PROVIDER_TIMEOUT",
        ),
    ] {
        let temp = tempdir().expect("state root");
        let store = PublicationJournalStore::open(temp.path()).expect("journal store");
        let engine = PublicationEngine::new(&store);
        let (request, broker) = request();
        let operation_id = request.operation_id();
        let source = BytesSource(b"canonical-car".to_vec());
        let failure = match class {
            PublicationBackendFailureClass::Retryable => PublicationBackendError::retryable(code),
            PublicationBackendFailureClass::Permanent => PublicationBackendError::permanent(code),
        };
        let mut backend = CompleteBackend {
            broker,
            replication_pending_once: false,
            finality_pending_once: false,
            substitute_readback: false,
            substitute_all_readbacks: true,
            readback_backend_failure: Some((ProviderId::new([1; 32]), failure)),
            readback_providers: Vec::new(),
            submissions: 0,
        };
        let error = engine
            .publish(request, &source, &mut backend)
            .expect_err("one backend failure plus invalid evidence cannot authorize AMX");
        let PublicationError::Backend(error) = error else {
            panic!("backend failure must retain its redacted classification");
        };
        assert_eq!(error.class(), class);
        assert_eq!(error.code(), code);
        assert_eq!(backend.submissions, 0);
        assert_eq!(
            backend.readback_providers,
            vec![
                ProviderId::new([1; 32]),
                ProviderId::new([2; 32]),
                ProviderId::new([3; 32]),
            ]
        );
        let unchanged = store.load(operation_id).expect("readback journal");
        assert_eq!(unchanged.phase, PublicationPhaseV1::Readback);
        assert!(unchanged.readbacks.is_empty());
        assert!(unchanged.release_submission_attempts.is_empty());
        assert!(unchanged.submission.is_none());
        unchanged
            .validate()
            .expect("failed readbacks leave a valid journal");
    }
}
#[cfg(unix)]
#[test]
fn journal_rejects_missing_phase_evidence_and_tampered_receipt_signature() {
    let temp = tempdir().expect("state root");
    let store = PublicationJournalStore::open(temp.path()).expect("journal store");
    let engine = PublicationEngine::new(&store);
    let (request, broker) = request();
    let operation_id = engine
        .begin_detached(request)
        .expect("persist detached operation");
    let mut missing = store.load(operation_id).expect("load journal");
    missing.phase = PublicationPhaseV1::Replication;
    assert!(matches!(
        missing.validate(),
        Err(PublicationError::InvalidJournal(_))
    ));
    let source = BytesSource(b"car".to_vec());
    let mut backend = EarlyBackend {
        broker,
        fail_validation_once: false,
        substitute_receipt: false,
        now_ms: 1_500,
        receipt_window: None,
        prepare_calls: 0,
    };
    assert!(matches!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("validate"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::SeedIngress)
    ));
    assert!(matches!(
        engine
            .advance_once(operation_id, &source, &mut backend)
            .expect("stage"),
        PublicationAdvanceV1::Progressed(PublicationPhaseV1::ArchiveRegistration)
    ));
    let mut tampered = store.load(operation_id).expect("load staged journal");
    let (_, attacker) = account(99);
    tampered
        .staging_receipt
        .as_mut()
        .expect("receipt")
        .approvals[0]
        .public_key = attacker.public_key().clone();
    let tampered = norito::encode_canonical(&tampered).expect("encode tampered journal");
    store
        .root
        .replace(&journal_relative_path(operation_id), &tampered)
        .expect("simulate durable disk substitution");
    assert!(matches!(
        store.load(operation_id),
        Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::SeedIngress,
            ..
        })
    ));
}
#[test]
fn journal_rejects_archive_registration_receipt_replay_from_another_nonce() {
    let (request, broker) = request();
    let registration = registration(&request, &broker);
    let expected_receipt = registration
        .intent
        .prepared_page
        .archive
        .staging_receipt
        .clone();
    let intent = registration_intent(request.operation_id(), &request, expected_receipt.clone());
    let registered = PublicationRegisteredArchiveV1 {
        finalized_transaction_hash: intent.transaction_hash,
        network_id: request.network_id,
        snapshot: MusubiRegistrySnapshotV1 {
            finalized_height: 60,
            finalized_block_hash: [0x3C; 32],
            index_revision: 2,
        },
        archive: registration.intent.prepared_page.archive.clone(),
    };
    let mut journal = PublicationJournalV1::new(request.clone()).expect("publication journal");
    journal.validation = Some(validation_evidence(&request));
    journal.staging_receipt = Some(expected_receipt);
    journal
        .archive_registration_attempts
        .push(PublicationArchiveRegistrationAttemptV1::new(1, intent));
    journal.registered_archive = Some(registered);
    journal
        .archive_location_attempts
        .push(PublicationArchiveLocationAttemptV1 {
            generation: 1,
            intent: registration.intent.clone(),
            registration: Some(registration),
            terminal: None,
            terminal_floor: None,
        });
    journal.phase = PublicationPhaseV1::Replication;
    journal
        .validate()
        .expect("registration must retain the exact staged receipt");
    let mut replayed_binding = request.receipt_binding();
    replayed_binding.nonce = [0xEE; 32];
    journal
        .archive_location_attempts
        .last_mut()
        .and_then(|attempt| attempt.registration.as_mut())
        .expect("archive registration")
        .finalized_page
        .archive
        .staging_receipt = signed_receipt(&replayed_binding, &broker);
    assert!(matches!(
        journal.validate(),
        Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::ArchiveRegistration,
            ..
        })
    ));
}
#[test]
fn journal_rejects_a_refreshed_receipt_after_archive_registration() {
    let (request, broker) = request();
    let registration = registration(&request, &broker);
    let registered_receipt = registration
        .intent
        .prepared_page
        .archive
        .staging_receipt
        .clone();
    let refreshed_receipt = signed_receipt_at(
        &registered_receipt.payload.binding,
        &broker,
        registered_receipt.payload.expires_at_ms + 1,
        registered_receipt.payload.expires_at_ms + 1_001,
    );
    assert_ne!(registered_receipt, refreshed_receipt);
    let mut journal = PublicationJournalV1::new(request).expect("publication journal");
    journal.validation = Some(validation_evidence(&journal.request));
    journal.staging_receipt = Some(refreshed_receipt);
    let intent = registration_intent(journal.operation_id, &journal.request, registered_receipt);
    journal.registered_archive = Some(PublicationRegisteredArchiveV1 {
        finalized_transaction_hash: intent.transaction_hash,
        network_id: journal.request.network_id,
        snapshot: MusubiRegistrySnapshotV1 {
            finalized_height: 60,
            finalized_block_hash: [0x3C; 32],
            index_revision: 2,
        },
        archive: registration.intent.prepared_page.archive.clone(),
    });
    journal
        .archive_registration_attempts
        .push(PublicationArchiveRegistrationAttemptV1::new(1, intent));
    journal
        .archive_location_attempts
        .push(PublicationArchiveLocationAttemptV1 {
            generation: 1,
            intent: registration.intent.clone(),
            registration: Some(registration),
            terminal: None,
            terminal_floor: None,
        });
    journal.phase = PublicationPhaseV1::Replication;
    assert!(matches!(
        journal.validate(),
        Err(PublicationError::InvalidJournal(ref reason))
            if reason.contains("exact staging receipt")
    ));
}
#[test]
fn replication_requires_three_exact_finalized_providers() {
    let (request, broker) = request();
    let registration = registration(&request, &broker);
    let intent = registration_intent(
        request.operation_id(),
        &request,
        registration
            .intent
            .prepared_page
            .archive
            .staging_receipt
            .clone(),
    );
    let registered = PublicationRegisteredArchiveV1 {
        finalized_transaction_hash: intent.transaction_hash,
        network_id: request.network_id,
        snapshot: MusubiRegistrySnapshotV1 {
            finalized_height: 60,
            finalized_block_hash: [0x3C; 32],
            index_revision: 2,
        },
        archive: registration.intent.prepared_page.archive.clone(),
    };
    registration
        .validate_for(request.operation_id(), &request, &registered, &[])
        .expect("valid archive registration");
    let below_quorum = location(&request, &registration, 2);
    assert!(matches!(
        validate_replication(&request, &registration, &below_quorum),
        Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::Replication,
            ..
        })
    ));
    let exact = location(&request, &registration, 3);
    validate_replication(&request, &registration, &exact).expect("three-provider quorum");
    let mut stale = exact.clone();
    stale.revision -= 1;
    assert!(matches!(
        validate_replication(&request, &registration, &stale),
        Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::Replication,
            ..
        })
    ));
    let mut equal_revision_substitution = exact.clone();
    equal_revision_substitution.renew_after_epoch += 1;
    assert!(matches!(
        validate_replication(&request, &registration, &equal_revision_substitution),
        Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::Replication,
            ..
        })
    ));
    let mut renewed_registration = registration.clone();
    renewed_registration.intent.pin_manifest = ManifestDigest::new([0xD1; 32]);
    renewed_registration.intent.replication_order = ReplicationOrderId::new([0x52; 32]);
    renewed_registration.intent.renew_after_epoch = 15;
    renewed_registration.intent.expires_at_epoch = 30;
    let mut renewed = location(&request, &renewed_registration, 3);
    renewed.revision = 3;
    renewed.finalized_height = 71;
    validate_replication(&request, &registration, &renewed)
        .expect("same stable location may carry an authenticated finalized renewal");
    assert_eq!(
        location_progress(&renewed, &renewed).expect("exact renewal is current"),
        PublicationLocationProgressV1::Current
    );
    let mut newer = renewed.clone();
    newer.revision += 1;
    newer.finalized_height += 1;
    assert_eq!(
        location_progress(&renewed, &newer).expect("newer renewal is current"),
        PublicationLocationProgressV1::Current
    );
    validate_replication(&request, &registration, &newer)
        .expect("newer authenticated renewal remains selectable");
    let mut higher_revision_lower_height = newer;
    higher_revision_lower_height.finalized_height = renewed.finalized_height - 1;
    assert!(matches!(
        location_progress(&renewed, &higher_revision_lower_height),
        Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::Replication,
            ..
        })
    ));
    let mut substituted = exact;
    substituted.provider_attestation_set_digest =
        MusubiProviderBundleAttestationSetDigestV1::new([0xEE; 32]);
    assert!(matches!(
        validate_replication(&request, &registration, &substituted),
        Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::Replication,
            ..
        })
    ));
}
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the test checks the complete revision and snapshot substitution matrix for location checkpoints"
)]
fn archive_location_checkpoints_reject_revision_and_snapshot_substitution() {
    let (request, broker) = request();
    let registration = registration(&request, &broker);
    let archive_intent = registration_intent(
        request.operation_id(),
        &request,
        registration
            .intent
            .prepared_page
            .archive
            .staging_receipt
            .clone(),
    );
    let registered = PublicationRegisteredArchiveV1 {
        finalized_transaction_hash: archive_intent.transaction_hash,
        network_id: request.network_id,
        snapshot: registration.intent.prepared_page.snapshot,
        archive: registration.intent.prepared_page.archive.clone(),
    };
    registration
        .validate_for(request.operation_id(), &request, &registered, &[])
        .expect("baseline archive-location application");
    registration
        .validate_polled_page(&request, &registration.finalized_page)
        .expect("baseline finalized location page");
    let mut target_revision_regressed = registration.clone();
    target_revision_regressed.finalized_page.items[0].revision =
        registration.intent.expected_location_revision;
    assert!(
        target_revision_regressed
            .validate_for(request.operation_id(), &request, &registered, &[])
            .is_err()
    );
    let mut first_application_substituted = registration.clone();
    first_application_substituted.finalized_page.items[0].pin_manifest =
        ManifestDigest::new([0xe1; 32]);
    assert!(
        first_application_substituted
            .validate_for(request.operation_id(), &request, &registered, &[])
            .is_err()
    );
    let mut first_application_not_healthy = registration.clone();
    first_application_not_healthy.finalized_page.items[0].state =
        MusubiArchiveLocationStateV1::Degraded;
    assert!(
        first_application_not_healthy
            .validate_for(request.operation_id(), &request, &registered, &[])
            .is_err()
    );
    let mut first_application_wrong_height = registration.clone();
    first_application_wrong_height.applied_height -= 1;
    assert!(
        first_application_wrong_height
            .validate_for(request.operation_id(), &request, &registered, &[])
            .is_err()
    );
    let mut archive_revision_regressed = registration.finalized_page.clone();
    archive_revision_regressed.snapshot.finalized_height += 1;
    archive_revision_regressed.snapshot.finalized_block_hash = [0xe2; 32];
    archive_revision_regressed.archive.location_revision -= 1;
    archive_revision_regressed.archive.location_ids.clear();
    archive_revision_regressed.items.clear();
    assert!(
        registration
            .validate_polled_page(&request, &archive_revision_regressed)
            .is_err()
    );
    let mut equal_archive_revision_substituted = registration.finalized_page.clone();
    equal_archive_revision_substituted.snapshot.finalized_height += 1;
    equal_archive_revision_substituted
        .snapshot
        .finalized_block_hash = [0xe3; 32];
    equal_archive_revision_substituted
        .archive
        .location_ids
        .clear();
    equal_archive_revision_substituted.items.clear();
    assert!(
        registration
            .validate_polled_page(&request, &equal_archive_revision_substituted)
            .is_err()
    );
    let mut same_snapshot_higher_revision = registration.finalized_page.clone();
    same_snapshot_higher_revision.archive.location_revision += 1;
    same_snapshot_higher_revision.items[0].revision += 1;
    assert!(
        registration
            .validate_polled_page(&request, &same_snapshot_higher_revision)
            .is_err()
    );
    let mut item_ahead_of_archive = registration.finalized_page.clone();
    item_ahead_of_archive.items[0].revision += 1;
    assert!(
        registration
            .validate_polled_page(&request, &item_ahead_of_archive)
            .is_err()
    );
    let mut same_snapshot_archive_substitution = registration.intent.prepared_page.clone();
    same_snapshot_archive_substitution.archive.location_revision += 1;
    assert!(
        validate_archive_location_page(&request, &registered, &same_snapshot_archive_substitution,)
            .is_err()
    );
    let mut registered_current = registered;
    registered_current.snapshot = registration.finalized_page.snapshot;
    registered_current.archive = registration.finalized_page.archive.clone();
    let mut later_archive_revision_regression = registration.finalized_page.clone();
    later_archive_revision_regression.snapshot.finalized_height += 1;
    later_archive_revision_regression
        .snapshot
        .finalized_block_hash = [0xe4; 32];
    later_archive_revision_regression.archive.location_revision -= 1;
    later_archive_revision_regression
        .archive
        .location_ids
        .clear();
    later_archive_revision_regression.items.clear();
    assert!(
        validate_archive_location_page(
            &request,
            &registered_current,
            &later_archive_revision_regression,
        )
        .is_err()
    );
}
#[test]
fn readback_rejects_provider_or_commitment_substitution() {
    let (request, broker) = request();
    let registration = registration(&request, &broker);
    let location = location(&request, &registration, 3);
    let provider = location.providers[0];
    let exact = PublicationReadbackEvidenceV1 {
        provider,
        location_id: location.location_id,
        replication_order: location.replication_order,
        commitment: request.archive_commitment.clone(),
        semantic_release_digest: request.publication.manifest.semantic_digest(),
        verification_lock_digest: request.publication.manifest.verification_lock_digest,
    };
    exact
        .validate_for(&request, &location, provider)
        .expect("exact readback");
    let mut wrong_provider = exact.clone();
    wrong_provider.provider = location.providers[1];
    assert!(
        wrong_provider
            .validate_for(&request, &location, provider)
            .is_err()
    );
    let mut wrong_car = exact;
    wrong_car.commitment.car_digest = MusubiContentDigestV1::new([0xEF; 32]);
    assert!(
        wrong_car
            .validate_for(&request, &location, provider)
            .is_err()
    );
}
#[test]
fn release_preparation_requires_a_sorted_distinct_location_provider_subset() {
    let (request, broker) = request();
    let registration = registration(&request, &broker);
    let replication = replication_checkpoint(&request, &registration, 3);
    let location = replication
        .location(&registration)
        .expect("fixture location");
    let readback_for = |provider| PublicationReadbackEvidenceV1 {
        provider,
        location_id: location.location_id,
        replication_order: location.replication_order,
        commitment: request.archive_commitment.clone(),
        semantic_release_digest: request.publication.manifest.semantic_digest(),
        verification_lock_digest: request.publication.manifest.verification_lock_digest,
    };
    let later_subset = vec![
        readback_for(location.providers[1]),
        readback_for(location.providers[2]),
    ];
    PublicationReleasePreparationFloorV1::try_new(
        registration.intent.generation,
        replication.clone(),
        later_subset.clone(),
        &request,
        &registration,
    )
    .expect("any sorted two-provider location subset is valid");
    let assert_rejected = |readbacks| {
        assert!(matches!(
            PublicationReleasePreparationFloorV1::try_new(
                registration.intent.generation,
                replication.clone(),
                readbacks,
                &request,
                &registration,
            ),
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Readback,
                ref reason,
            }) if reason
                == "provider readbacks were not a strictly ordered distinct location-provider subset"
        ));
    };
    let mut duplicate = later_subset.clone();
    duplicate[1] = duplicate[0].clone();
    assert_rejected(duplicate);
    let mut unsorted = later_subset.clone();
    unsorted.swap(0, 1);
    assert_rejected(unsorted);
    let mut nonmember = later_subset;
    nonmember[1].provider = ProviderId::new([0xFE; 32]);
    assert_rejected(nonmember);
}
#[test]
fn amx_and_final_index_evidence_bind_the_exact_release() {
    let (request, _) = request();
    let operation_id = request.operation_id();
    let instruction = request.publish_instruction();
    let exact_submission =
        PublicationAmxSubmissionV1::new(operation_id, &instruction, [0x71; 32], 80);
    exact_submission
        .validate_for(operation_id, &instruction)
        .expect("exact AMX submission");
    let mut substituted_submission = exact_submission;
    substituted_submission.instruction_digest = [0x72; 32];
    assert!(
        substituted_submission
            .validate_for(operation_id, &instruction)
            .is_err()
    );
    let mut heightless_submission = exact_submission;
    heightless_submission.applied_height = 0;
    assert!(
        heightless_submission
            .validate_for(operation_id, &instruction)
            .is_err()
    );
    let exact_final = final_evidence(&request);
    exact_final
        .validate_for(&request, &exact_submission)
        .expect("exact finalized home and universal records");
    let mut later_unrelated_snapshot = exact_final.clone();
    later_unrelated_snapshot.snapshot.finalized_height += 1;
    later_unrelated_snapshot.snapshot.finalized_block_hash = [0x75; 32];
    later_unrelated_snapshot.snapshot.index_revision += 1;
    later_unrelated_snapshot
        .validate_for(&request, &exact_submission)
        .expect("an unrelated later registry revision must not invalidate the exact row");
    let mut older_storage_projection = exact_final.clone();
    older_storage_projection
        .universal_release
        .selection
        .storage
        .index_revision -= 1;
    older_storage_projection
        .validate_for(&request, &exact_submission)
        .expect("a row may retain an older valid storage projection");
    let mut future_storage_projection = exact_final.clone();
    future_storage_projection
        .universal_release
        .selection
        .storage
        .index_revision += 1;
    assert!(
        future_storage_projection
            .validate_for(&request, &exact_submission)
            .is_err()
    );
    let mut mismatched_tip_storage = exact_final.clone();
    mismatched_tip_storage
        .universal_release
        .selection
        .storage
        .finalized_height = mismatched_tip_storage.snapshot.finalized_height;
    mismatched_tip_storage
        .universal_release
        .selection
        .storage
        .finalized_block_hash = [0x77; 32];
    assert!(
        mismatched_tip_storage
            .validate_for(&request, &exact_submission)
            .is_err()
    );
    let mut pre_application_snapshot = exact_final.clone();
    pre_application_snapshot.snapshot.finalized_height = exact_submission.applied_height - 1;
    pre_application_snapshot.snapshot.finalized_block_hash = [0x76; 32];
    assert!(
        pre_application_snapshot
            .validate_for(&request, &exact_submission)
            .is_err()
    );
    let mut wrong_network = exact_final.clone();
    // Another deployment may reuse the same human-facing ChainName, but its
    // distinct genesis-derived identity is never valid evidence for this request.
    wrong_network.network_id = publication_test_network_id(0x75);
    assert!(
        wrong_network
            .validate_for(&request, &exact_submission)
            .is_err()
    );
    let mut substituted_index = exact_final;
    substituted_index.universal_release.source_digest = MusubiContentDigestV1::new([0x73; 32]);
    assert!(
        substituted_index
            .validate_for(&request, &exact_submission)
            .is_err()
    );
}
#[test]
fn detached_operation_ids_are_canonical_nonzero_lowercase_hex() {
    let (request, _) = request();
    let operation_id = request.operation_id();
    assert_eq!(operation_id.to_string().parse(), Ok(operation_id));
    assert!("00".repeat(32).parse::<PublicationOperationIdV1>().is_err());
    let canonical = operation_id.to_string();
    assert!(
        format!("A{}", &canonical[1..])
            .parse::<PublicationOperationIdV1>()
            .is_err()
    );
}
