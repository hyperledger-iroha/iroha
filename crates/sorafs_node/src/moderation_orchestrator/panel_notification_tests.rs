fn saturated_delivered_panel_notifications(
    temp: &TempDir,
    checkpoint_name: &str,
) -> SaturatedPanelNotificationFixture {
    saturated_delivered_panel_notifications_with_probe(temp, checkpoint_name, None)
}
fn saturated_delivered_panel_notifications_with_probe(
    temp: &TempDir,
    checkpoint_name: &str,
    probe: Option<Arc<ReentrantLockProbe>>,
) -> SaturatedPanelNotificationFixture {
    let governance = account(99);
    let (awaiting, _) = awaiting_acceptance_snapshot(2, [0x29; 32], governance.clone());
    let reader = Arc::new(MockSnapshotReader::new(awaiting));
    let mut bounds = config(temp, checkpoint_name);
    bounds.max_handoffs = 3;
    let checkpoint_store = Arc::new(MockCheckpointStore::default());
    let archive = Arc::new(MockPanelNotificationArchive::default());
    let publication_sink = Arc::new(MockHandoffSink::default());
    let archive_dependency: Arc<dyn ModerationPanelNotificationArchiveV1> = match probe {
        Some(probe) => Arc::new(ProbedPanelNotificationArchive {
            inner: archive.clone(),
            probe,
        }),
        None => archive.clone(),
    };
    let orchestrator = ModerationOrchestratorV1::open(
        bounds.clone(),
        ModerationOrchestratorDepsV1 {
            checkpoint_store: checkpoint_store.clone(),
            submitter: Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
            snapshot_reader: reader.clone(),
            settlement_sink: Arc::new(MockHandoffSink::default()),
            publication_sink: publication_sink.clone(),
            panel_notification_sink: Arc::new(MockPanelNotificationSink::default()),
            panel_notification_archive: archive_dependency,
        },
    )
    .expect("orchestrator");
    orchestrator.reconcile().expect("queue assignments");
    let sink = MockPanelNotificationSink::default();
    let assignments = orchestrator
        .claim_panel_notifications([0xA1; 32], 1_000, 3)
        .expect("claim assignments");
    assert_eq!(assignments.len(), 3);
    for claim in &assignments {
        let receipt = sink.deliver(claim, 1_001);
        orchestrator
            .finalize_panel_notification_delivery(
                claim.worker_id,
                claim.lease_token,
                receipt,
                1_001,
            )
            .expect("finalize assignment");
    }
    {
        let state = orchestrator.state.lock().expect("state");
        assert_eq!(state.panel_notifications.len(), 3);
        assert!(
            state
                .panel_notifications
                .iter()
                .all(|entry| entry.state == StoredPanelNotificationStateV1::Delivered)
        );
    }
    SaturatedPanelNotificationFixture {
        bounds,
        governance,
        reader,
        checkpoint_store,
        archive,
        publication_sink,
        orchestrator,
    }
}
fn audited_two_generation_panel_notification_archive(
    temp: &TempDir,
    checkpoint_name: &str,
) -> (
    SaturatedPanelNotificationFixture,
    ModerationPanelNotificationArchiveHeadV1,
    ModerationPanelNotificationArchiveHeadV1,
) {
    let fixture = saturated_delivered_panel_notifications(temp, checkpoint_name);
    let first = fixture
        .orchestrator
        .compact_panel_notification_receipts(2)
        .expect("first archive compaction")
        .expect("first archive head");
    assert!(
        fixture
            .orchestrator
            .reconcile_panel_notification_archive_publication()
            .expect("publish first archive head")
    );
    let second = fixture
        .orchestrator
        .compact_panel_notification_receipts(1)
        .expect("second archive compaction")
        .expect("second archive head");
    assert!(
        fixture
            .orchestrator
            .reconcile_panel_notification_archive_publication()
            .expect("publish second archive head")
    );
    let trusted = fixture
        .orchestrator
        .audit_panel_notification_archive(MODERATION_PANEL_NOTIFICATION_ARCHIVE_AUDIT_PAGE_MAX_V1)
        .expect("establish trusted archive progress");
    assert_eq!(trusted.verified_heads, 2);
    assert_eq!(trusted.target_generation, 2);
    assert_eq!(trusted.last_completed_generation, 2);
    assert!(trusted.cycle_complete);
    assert!(
        fixture
            .orchestrator
            .durable_health()
            .expect("trusted archive health")
            .archive_is_fresh()
    );
    (fixture, first, second)
}
#[test]
fn panel_notification_capacity_recovers_only_after_exact_signed_archive_readback() {
    let temp = tempfile::tempdir().expect("tempdir");
    let SaturatedPanelNotificationFixture {
        governance,
        reader,
        checkpoint_store,
        archive,
        orchestrator,
        ..
    } = saturated_delivered_panel_notifications(&temp, "panel-capacity-archive.norito");
    reader.replace(activated_case_snapshot(3, [0x2A; 32], governance));
    assert!(matches!(
        orchestrator.reconcile(),
        Err(ModerationOrchestratorError::ResourceExhausted {
            resource: "panel notifications",
            limit: 3
        })
    ));
    let before = orchestrator.state.lock().expect("state").clone();
    archive.fail_next_read(1);
    let first_attempt = orchestrator.compact_panel_notification_receipts(2);
    let after_first_attempt = orchestrator.state.lock().expect("state").clone();
    assert!(
        after_first_attempt
            .panel_notification_archive_compaction_reservation
            .is_some(),
        "write-ahead reservation was not retained before {first_attempt:?}"
    );
    assert!(
        checkpoint_store.latest().checkpoint_generation > before.generation,
        "write-ahead reservation was not sealed before {first_attempt:?}"
    );
    assert_eq!(
        checkpoint_store.attestation_calls(),
        1,
        "terminal-set attestation did not complete before {first_attempt:?}"
    );
    assert_eq!(
        archive.artifact_count(),
        1,
        "archive install did not complete before {first_attempt:?}"
    );
    assert_eq!(
        first_attempt,
        Err(ModerationOrchestratorError::PanelNotificationArchiveUnavailable)
    );
    assert!(after_first_attempt.generation > before.generation);
    assert_eq!(
        after_first_attempt
            .panel_notification_archive_compaction_reservation
            .as_ref()
            .expect("sealed archive reservation")
            .records
            .len(),
        2
    );
    let head = orchestrator
        .compact_panel_notification_receipts(2)
        .expect("retry exact archived batch")
        .expect("archive head");
    assert_eq!(head.generation, 1);
    assert_eq!(head.terminal_record_count, 2);
    assert_ne!(head.archive_signature, [0; 64]);
    assert_eq!(archive.read_calls(), 2);
    assert_eq!(
        orchestrator
            .state
            .lock()
            .expect("state")
            .panel_notifications
            .len(),
        1
    );
    assert!(
        orchestrator
            .reconcile_panel_notification_archive_publication()
            .expect("publish sealed archive head")
    );
    orchestrator
        .reconcile()
        .expect("capacity recovers after authenticated archive readback");
    assert_eq!(
        orchestrator
            .state
            .lock()
            .expect("state")
            .panel_notifications
            .len(),
        3
    );
    assert_eq!(
        orchestrator
            .panel_notification_archive_head()
            .expect("authenticated archive head"),
        Some(head)
    );
}
#[test]
fn panel_notification_archive_publishes_audits_and_rotates_signers() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let (awaiting, _) = awaiting_acceptance_snapshot(2, [0x2B; 32], governance);
    let reader = Arc::new(MockSnapshotReader::new(awaiting));
    let checkpoint_store = Arc::new(MockCheckpointStore::default());
    let archive = Arc::new(MockPanelNotificationArchive::default());
    let publication_sink = Arc::new(MockHandoffSink::default());
    let mut bounds = config(&temp, "panel-archive-rotation.norito");
    bounds.max_handoffs = 3;
    let runtime_deps = || ModerationOrchestratorDepsV1 {
        checkpoint_store: checkpoint_store.clone(),
        submitter: Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
        snapshot_reader: reader.clone(),
        settlement_sink: Arc::new(MockHandoffSink::default()),
        publication_sink: publication_sink.clone(),
        panel_notification_sink: Arc::new(MockPanelNotificationSink::default()),
        panel_notification_archive: archive.clone(),
    };
    let orchestrator =
        ModerationOrchestratorV1::open(bounds.clone(), runtime_deps()).expect("orchestrator");
    orchestrator.reconcile().expect("queue assignments");
    let mut claims = orchestrator
        .claim_panel_notifications([0xA1; 32], 1_000, 3)
        .expect("claim assignments");
    assert_eq!(claims.len(), 3);
    claims.sort_by_key(|claim| claim.notification.notification_id);
    let delivery_sink = MockPanelNotificationSink::default();
    for claim in &claims {
        let receipt = delivery_sink.deliver(claim, 1_001);
        orchestrator
            .finalize_panel_notification_delivery(
                claim.worker_id,
                claim.lease_token,
                receipt,
                1_001,
            )
            .expect("terminal delivery receipt");
    }
    let first = orchestrator
        .compact_panel_notification_receipts(2)
        .expect("first archive compaction")
        .expect("first archive head");
    let first_artifact_bytes = archive.artifact(first.operation_id);
    let first_artifact = verify_panel_notification_archive_artifact(
        &bounds,
        &orchestrator.network_id,
        &first_artifact_bytes,
    )
    .expect("strict first archive artifact");
    assert_eq!(first_artifact.payload.records.len(), 2);
    assert!(first_artifact.payload.records.iter().all(|record| {
        matches!(
            record,
            ModerationTerminalArchiveRecordV1::PanelNotification(
                ModerationPanelNotificationArchiveRecordV1 {
                    notification_id,
                    terminal_status:
                        ModerationPanelNotificationArchiveTerminalStatusV1::Delivered { .. },
                    source_record_digest,
                }
            ) if *notification_id != [0; 32] && *source_record_digest != [0; 32]
        )
    }));
    let unpublished = orchestrator
        .durable_health()
        .expect("unpublished archive health");
    assert_eq!(unpublished.panel_notification_archive_generation, 1);
    assert_eq!(
        unpublished.panel_notification_archive_published_generation,
        0
    );
    assert!(!unpublished.archive_is_fresh());
    assert!(
        orchestrator
            .reconcile_panel_notification_archive_publication()
            .expect("publish first head")
    );
    assert!(
        !orchestrator
            .reconcile_panel_notification_archive_publication()
            .expect("idempotent empty publication replay")
    );
    assert_eq!(publication_sink.published_archive_head_count(), 1);
    let first_audit = orchestrator
        .audit_panel_notification_archive(MODERATION_PANEL_NOTIFICATION_ARCHIVE_AUDIT_PAGE_MAX_V1)
        .expect("complete first audit sweep");
    assert_eq!(first_audit.verified_heads, 1);
    assert_eq!(first_audit.last_completed_generation, 1);
    assert!(first_audit.cycle_complete);
    assert!(
        orchestrator
            .durable_health()
            .expect("fresh first archive health")
            .archive_is_fresh()
    );
    let previous_epoch = orchestrator
        .panel_notification_archive_signer_epochs()
        .expect("bootstrap signer epoch")
        .into_iter()
        .next()
        .expect("bootstrap epoch");
    let network_id = orchestrator.network_id;
    drop(orchestrator);
    let predecessor_key = SigningKey::from_bytes(&PANEL_NOTIFICATION_ARCHIVE_SIGNING_SEED);
    let rotated_key = SigningKey::from_bytes(&PANEL_NOTIFICATION_ARCHIVE_ROTATED_SIGNING_SEED);
    let mut proposed_epoch = ModerationPanelNotificationArchiveSignerEpochV1 {
        version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
        epoch: 2,
        activated_at_generation: 2,
        archive_id: PANEL_NOTIFICATION_ARCHIVE_ID,
        archive_handle: PANEL_NOTIFICATION_ARCHIVE_HANDLE.to_owned(),
        archive_revision: PANEL_NOTIFICATION_ARCHIVE_ROTATED_QUALIFICATION.revision(),
        archive_policy_digest: PANEL_NOTIFICATION_ARCHIVE_ROTATED_QUALIFICATION.policy_digest(),
        archive_public_key: rotated_key.verifying_key().to_bytes(),
        predecessor_epoch_digest: Some(previous_epoch.epoch_digest),
        predecessor_revocation_generation: Some(1),
        predecessor_authorization_signature: None,
        new_key_possession_signature: None,
        epoch_digest: [0; 32],
    };
    let authorization_message = proposed_epoch
        .rotation_authorization_message(&network_id)
        .expect("canonical predecessor authorization message");
    let possession_message = proposed_epoch
        .new_key_possession_message(&network_id)
        .expect("canonical new-key possession message");
    let predecessor_authorization_signature =
        predecessor_key.sign(&authorization_message).to_bytes();
    let new_key_possession_signature = rotated_key.sign(&possession_message).to_bytes();
    proposed_epoch.predecessor_authorization_signature = Some(predecessor_authorization_signature);
    proposed_epoch.new_key_possession_signature = Some(new_key_possession_signature);
    bounds.expected_panel_notification_archive_qualification =
        PANEL_NOTIFICATION_ARCHIVE_ROTATED_QUALIFICATION;
    bounds.panel_notification_archive_public_key = rotated_key.verifying_key().to_bytes();
    bounds.panel_notification_archive_predecessor_revocation_generation = Some(1);
    bounds.panel_notification_archive_predecessor_authorization_signature =
        Some(predecessor_authorization_signature);
    bounds.panel_notification_archive_new_key_possession_signature =
        Some(new_key_possession_signature);
    archive
        .provider
        .set_qualification(PANEL_NOTIFICATION_ARCHIVE_ROTATED_QUALIFICATION);
    archive.rotate_signing_key(PANEL_NOTIFICATION_ARCHIVE_ROTATED_SIGNING_SEED);
    let mut substituted = bounds.clone();
    substituted
        .panel_notification_archive_predecessor_authorization_signature
        .as_mut()
        .expect("configured authorization")[0] ^= 1;
    assert!(matches!(
        ModerationOrchestratorV1::open(substituted, runtime_deps()),
        Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid)
    ));
    let rotated =
        ModerationOrchestratorV1::open(bounds.clone(), runtime_deps()).expect("rotated signer");
    let epochs = rotated
        .panel_notification_archive_signer_epochs()
        .expect("authenticated rotated signer log");
    assert_eq!(epochs.len(), 2);
    assert_eq!(
        epochs[1].archive_public_key,
        rotated_key.verifying_key().to_bytes()
    );
    assert_eq!(epochs[1].predecessor_revocation_generation, Some(1));
    assert_eq!(
        epochs[1].predecessor_authorization_signature,
        proposed_epoch.predecessor_authorization_signature
    );
    assert_eq!(
        epochs[1].new_key_possession_signature,
        proposed_epoch.new_key_possession_signature
    );
    let second = rotated
        .compact_panel_notification_receipts(2)
        .expect("post-rotation archive compaction")
        .expect("second archive head");
    assert_eq!(second.generation, 2);
    assert_eq!(second.archive_signer_epoch, 2);
    assert_eq!(
        second.archive_public_key,
        rotated_key.verifying_key().to_bytes()
    );
    assert_eq!(second.predecessor_operation_id, Some(first.operation_id));
    assert!(
        rotated
            .reconcile_panel_notification_archive_publication()
            .expect("publish rotated head")
    );
    assert_eq!(publication_sink.published_archive_head_count(), 2);
    let second_audit = rotated
        .audit_panel_notification_archive(MODERATION_PANEL_NOTIFICATION_ARCHIVE_AUDIT_PAGE_MAX_V1)
        .expect("audit both signer epochs");
    assert_eq!(second_audit.verified_heads, 2);
    assert_eq!(second_audit.last_completed_generation, 2);
    assert!(second_audit.cycle_complete);
    assert!(
        rotated
            .durable_health()
            .expect("fresh rotated archive health")
            .archive_is_fresh()
    );
    let mut corrupt_predecessor = first_artifact_bytes;
    corrupt_predecessor.push(0);
    archive.replace_artifact(first.operation_id, corrupt_predecessor);
    assert_eq!(
        rotated.audit_panel_notification_archive_full_history(
            MODERATION_PANEL_NOTIFICATION_ARCHIVE_AUDIT_PAGE_MAX_V1,
        ),
        Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid)
    );
}
#[test]
fn panel_notification_archive_full_history_audit_resumes_after_restart() {
    let temp = tempfile::tempdir().expect("tempdir");
    let (fixture, _, _) = audited_two_generation_panel_notification_archive(
        &temp,
        "panel-archive-full-history-restart.norito",
    );
    let first_page = fixture
        .orchestrator
        .audit_panel_notification_archive_full_history(1)
        .expect("seal and audit the first full-history page");
    assert_eq!(first_page.verified_heads, 1);
    assert_eq!(first_page.target_generation, 2);
    assert_eq!(first_page.last_completed_generation, 0);
    assert!(!first_page.cycle_complete);
    let incomplete_health = fixture
        .orchestrator
        .durable_health()
        .expect("incomplete full-history health");
    assert_eq!(incomplete_health.panel_notification_archive_generation, 2);
    assert_eq!(
        incomplete_health.panel_notification_archive_published_generation,
        2
    );
    assert_eq!(
        incomplete_health.panel_notification_archive_audited_generation,
        0
    );
    assert!(!incomplete_health.archive_is_fresh());
    let SaturatedPanelNotificationFixture {
        bounds,
        reader,
        checkpoint_store,
        archive,
        publication_sink,
        orchestrator,
        ..
    } = fixture;
    drop(orchestrator);
    let restarted = ModerationOrchestratorV1::open(
        bounds,
        ModerationOrchestratorDepsV1 {
            checkpoint_store,
            submitter: Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
            snapshot_reader: reader,
            settlement_sink: Arc::new(MockHandoffSink::default()),
            publication_sink,
            panel_notification_sink: Arc::new(MockPanelNotificationSink::default()),
            panel_notification_archive: archive,
        },
    )
    .expect("restart with the sealed full-history cursor");
    assert_eq!(
        restarted
            .durable_health()
            .expect("restarted incomplete full-history health"),
        incomplete_health
    );
    let completed = restarted
        .audit_panel_notification_archive(1)
        .expect("resume and complete the sealed full-history audit");
    assert_eq!(completed.verified_heads, 1);
    assert_eq!(completed.target_generation, 2);
    assert_eq!(completed.last_completed_generation, 2);
    assert!(completed.cycle_complete);
    let completed_health = restarted
        .durable_health()
        .expect("completed full-history health");
    assert_eq!(
        completed_health.panel_notification_archive_audited_generation,
        2
    );
    assert!(completed_health.archive_is_fresh());
}
#[test]
fn panel_notification_archive_full_history_corrupt_predecessor_fails_closed_after_restart() {
    let temp = tempfile::tempdir().expect("tempdir");
    let (fixture, first, _) = audited_two_generation_panel_notification_archive(
        &temp,
        "panel-archive-full-history-corrupt-restart.norito",
    );
    let first_page = fixture
        .orchestrator
        .audit_panel_notification_archive_full_history(1)
        .expect("seal and audit the current archive head");
    assert_eq!(first_page.verified_heads, 1);
    assert_eq!(first_page.last_completed_generation, 0);
    assert!(!first_page.cycle_complete);
    let mut corrupt_predecessor = fixture.archive.artifact(first.operation_id);
    corrupt_predecessor.push(0);
    fixture
        .archive
        .replace_artifact(first.operation_id, corrupt_predecessor.clone());
    let sealed_incomplete_record = fixture.checkpoint_store.latest();
    let SaturatedPanelNotificationFixture {
        bounds,
        reader,
        checkpoint_store,
        archive,
        publication_sink,
        orchestrator,
        ..
    } = fixture;
    drop(orchestrator);
    assert!(matches!(
        ModerationOrchestratorV1::open(
            bounds,
            ModerationOrchestratorDepsV1 {
                checkpoint_store: checkpoint_store.clone(),
                submitter: Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown,)),
                snapshot_reader: reader,
                settlement_sink: Arc::new(MockHandoffSink::default()),
                publication_sink,
                panel_notification_sink: Arc::new(MockPanelNotificationSink::default()),
                panel_notification_archive: archive.clone(),
            },
        ),
        Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid)
    ));
    assert_eq!(checkpoint_store.latest(), sealed_incomplete_record);
    assert_eq!(archive.artifact(first.operation_id), corrupt_predecessor);
}
#[test]
fn panel_notification_archive_full_history_invalid_bounds_preserve_trusted_progress() {
    let temp = tempfile::tempdir().expect("tempdir");
    let (fixture, _, _) = audited_two_generation_panel_notification_archive(
        &temp,
        "panel-archive-full-history-bounds.norito",
    );
    let trusted_health = fixture
        .orchestrator
        .durable_health()
        .expect("trusted archive health");
    let trusted_record = fixture.checkpoint_store.latest();
    let trusted_cursor = fixture
        .orchestrator
        .state
        .lock()
        .expect("orchestrator state")
        .panel_notification_archive_audit_cursor
        .clone();
    for maximum_heads in [0, 17] {
        assert!(matches!(
            fixture
                .orchestrator
                .audit_panel_notification_archive_full_history(maximum_heads),
            Err(ModerationOrchestratorError::ResourceExhausted {
                resource: "panel notification archive audit page",
                limit: 16,
            })
        ));
        assert_eq!(
            fixture
                .orchestrator
                .durable_health()
                .expect("trusted health after rejected bound"),
            trusted_health
        );
        assert_eq!(fixture.checkpoint_store.latest(), trusted_record);
        assert_eq!(
            fixture
                .orchestrator
                .state
                .lock()
                .expect("orchestrator state")
                .panel_notification_archive_audit_cursor,
            trusted_cursor
        );
    }
}
#[test]
fn panel_notification_archive_broker_fixture_is_canonical_and_source_bound() {
    let fixture = moderation_panel_notification_archive_broker_fixture_v1()
        .expect("deterministic broker fixture");
    let expectation = fixture.expectation();
    assert_eq!(
        validate_moderation_panel_notification_source_attestation_for_broker_v1(
            &fixture.source_attestation,
            &fixture.network_id,
            &fixture.checkpoint_handle,
            fixture.checkpoint_qualification,
            fixture.checkpoint_attestation_public_key,
            &fixture.current_checkpoint_record,
        )
        .expect("strict source statement"),
        fixture.validation.source_attestation_digest
    );
    assert_eq!(
        validate_moderation_panel_notification_archive_artifact_for_broker_v1(
            &fixture.canonical_artifact,
            &expectation,
        )
        .expect("strict unsigned archive artifact"),
        fixture.validation
    );
    let (signed_head, head_validation) =
        validate_moderation_panel_notification_archive_head_for_broker_v1(
            &fixture.canonical_signed_head,
            &expectation,
        )
        .expect("strict signed archive head");
    assert_eq!(head_validation, fixture.validation);
    assert_eq!(signed_head.archive_signature, fixture.archive_signature);
    let mut trailing_artifact = fixture.canonical_artifact.clone();
    trailing_artifact.push(0);
    assert_eq!(
        validate_moderation_panel_notification_archive_artifact_for_broker_v1(
            &trailing_artifact,
            &expectation,
        ),
        Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid)
    );
    for substituted_source in [
        {
            let mut statement = fixture.source_attestation.clone();
            statement.terminal_record_count = 1;
            statement
        },
        {
            let mut statement = fixture.source_attestation.clone();
            statement.terminal_record_count = 3;
            statement
        },
        {
            let mut statement = fixture.source_attestation.clone();
            statement.terminal_set_digest[0] ^= 0x80;
            statement
        },
        {
            let mut statement = fixture.source_attestation.clone();
            statement.first_notification_id[0] ^= 0x80;
            statement
        },
    ] {
        assert_eq!(
            validate_moderation_panel_notification_source_attestation_for_broker_v1(
                &substituted_source,
                &fixture.network_id,
                &fixture.checkpoint_handle,
                fixture.checkpoint_qualification,
                fixture.checkpoint_attestation_public_key,
                &fixture.current_checkpoint_record,
            ),
            Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid)
        );
    }
}
#[test]
fn panel_notification_archive_callbacks_run_without_the_state_mutex() {
    let temp = tempfile::tempdir().expect("tempdir");
    let probe = Arc::new(ReentrantLockProbe::default());
    let SaturatedPanelNotificationFixture { orchestrator, .. } =
        saturated_delivered_panel_notifications_with_probe(
            &temp,
            "panel-archive-reentrant.norito",
            Some(probe.clone()),
        );
    let orchestrator = Arc::new(orchestrator);
    probe.attach(&orchestrator);
    let head = orchestrator
        .compact_panel_notification_receipts(1)
        .expect("archive outside state mutex")
        .expect("archive head");
    assert!(
        orchestrator
            .reconcile_panel_notification_archive_publication()
            .expect("publication outside state mutex")
    );
    assert_eq!(
        orchestrator
            .panel_notification_archive_head()
            .expect("read archive head outside state mutex"),
        Some(head)
    );
    assert!(probe.checks() >= 12);
}
#[test]
fn panel_notification_archive_provider_is_mandatory_and_exactly_qualified() {
    let temp = tempfile::tempdir().expect("tempdir");
    let config = config(&temp, "missing/panel-archive-provider.norito");
    let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
    let missing_parent = config
        .checkpoint_path
        .parent()
        .expect("checkpoint parent")
        .to_path_buf();
    for archive in [
        Arc::new(MockPanelNotificationArchive::default()),
        Arc::new(MockPanelNotificationArchive::with_handle(
            "object-lock:prod-moderation-receipts-secondary",
        )),
        Arc::new(MockPanelNotificationArchive::with_handle(
            "object-lock:test-moderation-receipts",
        )),
    ] {
        if archive.handle() == PANEL_NOTIFICATION_ARCHIVE_HANDLE {
            archive
                .provider
                .set_readiness(ModerationRuntimeProviderReadinessErrorV1::Unavailable);
        }
        let mut runtime_deps = deps(
            reader.clone(),
            Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
        );
        runtime_deps.panel_notification_archive = archive;
        assert!(matches!(
            ModerationOrchestratorV1::open(config.clone(), runtime_deps),
            Err(ModerationOrchestratorError::InvalidConfiguration(message))
                if message.contains("runtime provider binding")
        ));
        assert!(!missing_parent.exists());
    }
}
#[test]
fn panel_notification_archive_rejects_corrupt_signature_rollback_and_predecessor_substitution() {
    let temp = tempfile::tempdir().expect("tempdir");
    let SaturatedPanelNotificationFixture {
        archive,
        orchestrator,
        ..
    } = saturated_delivered_panel_notifications(&temp, "panel-archive-adversarial.norito");
    let first = orchestrator
        .compact_panel_notification_receipts(1)
        .expect("first compaction")
        .expect("first head");
    let first_bytes = archive.artifact(first.operation_id);
    orchestrator
        .reconcile_panel_notification_archive_publication()
        .expect("publish first archive head");
    let second = orchestrator
        .compact_panel_notification_receipts(1)
        .expect("second compaction")
        .expect("second head");
    orchestrator
        .reconcile_panel_notification_archive_publication()
        .expect("publish second archive head");
    let second_bytes = archive.artifact(second.operation_id);
    assert_eq!(second.generation, 2);
    assert_eq!(second.predecessor_head_digest, Some(first.head_digest));
    assert_eq!(second.predecessor_operation_id, Some(first.operation_id));
    for behavior in [2, 3, 4] {
        archive.fail_next_read(behavior);
        assert_eq!(
            orchestrator.panel_notification_archive_head(),
            Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid)
        );
    }
    archive.replace_artifact(second.operation_id, first_bytes.clone());
    assert_eq!(
        orchestrator.panel_notification_archive_head(),
        Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid)
    );
    archive.replace_artifact(second.operation_id, second_bytes.clone());
    archive.replace_artifact(first.operation_id, second_bytes);
    assert_eq!(
        orchestrator.panel_notification_archive_head(),
        Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid)
    );
    archive.replace_artifact(first.operation_id, first_bytes);
    assert_eq!(
        orchestrator
            .panel_notification_archive_head()
            .expect("restored exact archive lineage"),
        Some(second)
    );
}
#[test]
fn panel_notification_archive_crash_boundary_replays_exact_batch_after_restart_with_smaller_hint() {
    let temp = tempfile::tempdir().expect("tempdir");
    let SaturatedPanelNotificationFixture {
        bounds,
        reader,
        checkpoint_store,
        archive,
        orchestrator,
        ..
    } = saturated_delivered_panel_notifications(&temp, "panel-archive-crash.norito");
    archive.fail_next_install(2);
    checkpoint_store.fail_cas_after_one_success();
    assert_eq!(
        orchestrator.compact_panel_notification_receipts(2),
        Err(ModerationOrchestratorError::CheckpointStoreFenced)
    );
    assert_eq!(archive.artifact_count(), 1);
    drop(orchestrator);
    let restarted = ModerationOrchestratorV1::open(
        bounds,
        ModerationOrchestratorDepsV1 {
            checkpoint_store,
            submitter: Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
            snapshot_reader: reader,
            settlement_sink: Arc::new(MockHandoffSink::default()),
            publication_sink: Arc::new(MockHandoffSink::default()),
            panel_notification_sink: Arc::new(MockPanelNotificationSink::default()),
            panel_notification_archive: archive.clone(),
        },
    )
    .expect("restart from pre-prune sealed checkpoint");
    let recovered = restarted
        .compact_panel_notification_receipts(1)
        .expect("replay exact archived batch despite a smaller page-size hint")
        .expect("recovered archive head");
    assert_eq!(recovered.generation, 1);
    assert_eq!(recovered.terminal_record_count, 2);
    assert_eq!(archive.artifact_count(), 1);
    assert_eq!(archive.install_calls(), 2);
}
#[test]
fn panel_notification_archive_conflicting_replica_is_fenced_by_sealed_checkpoint_cas() {
    let temp = tempfile::tempdir().expect("tempdir");
    let SaturatedPanelNotificationFixture {
        mut bounds,
        reader,
        checkpoint_store,
        archive,
        orchestrator: first,
        ..
    } = saturated_delivered_panel_notifications(&temp, "panel-archive-replica-a.norito");
    bounds.checkpoint_path = temp
        .path()
        .canonicalize()
        .expect("canonical tempdir")
        .join("panel-archive-replica-b.norito");
    let second = ModerationOrchestratorV1::open(
        bounds,
        ModerationOrchestratorDepsV1 {
            checkpoint_store,
            submitter: Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
            snapshot_reader: reader,
            settlement_sink: Arc::new(MockHandoffSink::default()),
            publication_sink: Arc::new(MockHandoffSink::default()),
            panel_notification_sink: Arc::new(MockPanelNotificationSink::default()),
            panel_notification_archive: archive.clone(),
        },
    )
    .expect("open second replica at the same sealed source checkpoint");
    let committed = first
        .compact_panel_notification_receipts(1)
        .expect("first replica compaction")
        .expect("first replica head");
    assert_eq!(
        second.compact_panel_notification_receipts(2),
        Err(ModerationOrchestratorError::CheckpointStoreFenced)
    );
    assert_eq!(committed.generation, 1);
    assert_eq!(archive.artifact_count(), 1);
    assert_eq!(archive.install_calls(), 1);
}
#[test]
fn panel_notification_claims_recover_crashes_and_finalize_one_stable_receipt() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let (snapshot, _) = awaiting_acceptance_snapshot(2, [0x23; 32], governance);
    let reader = Arc::new(MockSnapshotReader::new(snapshot));
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
    let checkpoint = config(&temp, "panel-crash-recovery.norito");
    let orchestrator = ModerationOrchestratorV1::open(
        checkpoint.clone(),
        deps(Arc::clone(&reader), Arc::clone(&submitter)),
    )
    .expect("orchestrator");
    orchestrator.reconcile().expect("queue notifications");
    let first_claims = orchestrator
        .claim_panel_notifications([0xA1; 32], 1_000, 3)
        .expect("first worker claims all notifications");
    assert_eq!(first_claims.len(), 3);
    assert!(first_claims.iter().all(|claim| claim.attempt_limit == 3));
    assert!(
        orchestrator
            .claim_panel_notifications([0xB2; 32], 1_000, 3)
            .expect("duplicate worker scan")
            .is_empty()
    );
    let sink = MockPanelNotificationSink::default();
    let first_receipt = sink.deliver(&first_claims[0], 1_001);
    let target_id = first_claims[0].notification.notification_id;
    drop(orchestrator);
    let restarted = ModerationOrchestratorV1::open(checkpoint, deps(reader, submitter))
        .expect("restart with durable claims");
    assert!(
        restarted
            .claim_panel_notifications([0xB2; 32], 30_999, 3)
            .expect("leases remain exclusive before expiry")
            .is_empty()
    );
    assert!(
        restarted
            .claim_panel_notifications([0xB2; 32], 31_000, 3)
            .expect("expiry begins deterministic backoff")
            .is_empty()
    );
    let second_claims = restarted
        .claim_panel_notifications([0xB2; 32], 32_000, 3)
        .expect("expired claims are reclaimed after backoff");
    assert_eq!(second_claims.len(), 3);
    assert!(second_claims.iter().all(|claim| claim.attempt == 2));
    let second_claim = second_claims
        .iter()
        .find(|claim| claim.notification.notification_id == target_id)
        .expect("same notification reclaimed");
    let deduplicated_receipt = sink.deliver(second_claim, 32_001);
    assert_eq!(deduplicated_receipt, first_receipt);
    assert_eq!(sink.calls(), 2);
    assert_eq!(sink.unique_deliveries(), 1);
    assert!(matches!(
        restarted.finalize_panel_notification_delivery(
            first_claims[0].worker_id,
            first_claims[0].lease_token,
            first_receipt,
            32_001,
        ),
        Err(ModerationOrchestratorError::PanelNotificationClaimConflict {
            notification_id
        }) if notification_id == target_id
    ));
    assert_eq!(
        restarted
            .finalize_panel_notification_delivery(
                second_claim.worker_id,
                second_claim.lease_token,
                deduplicated_receipt,
                32_001,
            )
            .expect("reclaimed receipt finalization"),
        ModerationPanelNotificationFinalizeOutcomeV1::Delivered
    );
    assert_eq!(
        restarted
            .finalize_panel_notification_delivery(
                second_claim.worker_id,
                second_claim.lease_token,
                deduplicated_receipt,
                32_002,
            )
            .expect("idempotent receipt replay"),
        ModerationPanelNotificationFinalizeOutcomeV1::AlreadyDelivered
    );
    let mut substituted = deduplicated_receipt;
    substituted.receipt_digest = [0xEE; 32];
    assert!(matches!(
        restarted.finalize_panel_notification_delivery(
            second_claim.worker_id,
            second_claim.lease_token,
            substituted,
            32_003,
        ),
        Err(ModerationOrchestratorError::PanelNotificationReceiptConflict {
            notification_id
        }) if notification_id == target_id
    ));
    assert!(matches!(
        restarted
            .panel_notification_status(target_id)
            .expect("durable status"),
        Some(ModerationPanelNotificationStatusV1::Delivered {
            receipt_digest,
            attempts: 2,
            ..
        }) if receipt_digest == first_receipt.receipt_digest
    ));
}
#[test]
fn panel_notification_backoff_poison_and_retry_exhaustion_are_bounded() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let (snapshot, _) = awaiting_acceptance_snapshot(2, [0x24; 32], governance.clone());
    let orchestrator = ModerationOrchestratorV1::open(
        config(&temp, "panel-retry.norito"),
        deps(
            Arc::new(MockSnapshotReader::new(snapshot.clone())),
            Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
        ),
    )
    .expect("retry orchestrator");
    orchestrator.reconcile().expect("queue retry fixtures");
    let first = orchestrator
        .claim_panel_notifications([0xA1; 32], 1_000, 3)
        .expect("first claims");
    for claim in &first {
        orchestrator
            .release_panel_notification_claim(
                claim.notification.notification_id,
                claim.worker_id,
                claim.lease_token,
                ModerationPanelNotificationFailureV1::NotDelivered,
                1_001,
            )
            .expect("first safe failure");
    }
    assert!(
        orchestrator
            .claim_panel_notifications([0xB2; 32], 2_000, 3)
            .expect("backoff scan")
            .is_empty()
    );
    let second = orchestrator
        .claim_panel_notifications([0xB2; 32], 2_001, 3)
        .expect("second claims");
    for claim in &second {
        orchestrator
            .release_panel_notification_claim(
                claim.notification.notification_id,
                claim.worker_id,
                claim.lease_token,
                ModerationPanelNotificationFailureV1::Ambiguous,
                2_002,
            )
            .expect("ambiguous delivery is safely retryable by identity");
        assert!(matches!(
            orchestrator
                .panel_notification_status(claim.notification.notification_id)
                .expect("ambiguous delivery status"),
            Some(ModerationPanelNotificationStatusV1::Pending { attempts: 2, .. })
        ));
    }
    let third = orchestrator
        .claim_panel_notifications([0xC3; 32], 4_002, 3)
        .expect("third claims");
    for claim in &third {
        orchestrator
            .release_panel_notification_claim(
                claim.notification.notification_id,
                claim.worker_id,
                claim.lease_token,
                ModerationPanelNotificationFailureV1::NotDelivered,
                4_003,
            )
            .expect("exhaust final attempt");
        assert!(matches!(
            orchestrator
                .panel_notification_status(claim.notification.notification_id)
                .expect("retry terminal status"),
            Some(ModerationPanelNotificationStatusV1::DeadLetter {
                reason: ModerationPanelNotificationDeadLetterReasonV1::RetryExhausted,
                attempts: 3,
                ..
            })
        ));
    }
    let poison_cursor = snapshot.anchor();
    let poison = ModerationOrchestratorV1::open(
        config(&temp, "panel-poison.norito"),
        deps(
            Arc::new(MockSnapshotReader::new(snapshot)),
            Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
        ),
    )
    .expect("poison orchestrator");
    poison.reconcile().expect("queue poison fixture");
    let poison_claim = poison
        .claim_panel_notifications([0xD4; 32], 1_000, 1)
        .expect("poison claim")
        .into_iter()
        .next()
        .expect("one poison claim");
    poison
        .release_panel_notification_claim(
            poison_claim.notification.notification_id,
            poison_claim.worker_id,
            poison_claim.lease_token,
            ModerationPanelNotificationFailureV1::Permanent,
            1_001,
        )
        .expect("permanent failure dead letters");
    assert!(matches!(
        poison
            .panel_notification_status(poison_claim.notification.notification_id)
            .expect("poison status"),
        Some(ModerationPanelNotificationStatusV1::DeadLetter {
            reason: ModerationPanelNotificationDeadLetterReasonV1::PermanentRejection,
            attempts: 1,
            ..
        })
    ));
    let health = poison
        .durable_health()
        .expect("payload-free durable health");
    assert_eq!(health.finalized_cursor, Some(poison_cursor));
    assert_eq!(health.panel_notification_dead_letters, 1);
    assert!(health.has_dead_letters());
}
#[test]
fn panel_notification_claim_inputs_tokens_and_clock_are_fail_closed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let (snapshot, _) = awaiting_acceptance_snapshot(2, [0x28; 32], governance);
    let mut bounds = config(&temp, "panel-negative-inputs.norito");
    bounds.max_handoffs = 3;
    let orchestrator = ModerationOrchestratorV1::open(
        bounds,
        deps(
            Arc::new(MockSnapshotReader::new(snapshot)),
            Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
        ),
    )
    .expect("orchestrator");
    orchestrator.reconcile().expect("queue notifications");
    assert_eq!(
        orchestrator.claim_panel_notifications([0; 32], 1_000, 1),
        Err(ModerationOrchestratorError::InvalidPanelNotificationClaim)
    );
    assert!(matches!(
        orchestrator.claim_panel_notifications([0xA1; 32], 1_000, 4),
        Err(ModerationOrchestratorError::ResourceExhausted {
            resource: "panel notification claim batch",
            limit: 3
        })
    ));
    assert_eq!(
        orchestrator.claim_panel_notifications([0xA1; 32], u64::MAX, 1),
        Err(ModerationOrchestratorError::GenerationOverflow)
    );
    let claim = orchestrator
        .claim_panel_notifications([0xA1; 32], 1_000, 1)
        .expect("valid claim")
        .into_iter()
        .next()
        .expect("one valid claim");
    assert_eq!(
        orchestrator.claim_panel_notifications([0xB2; 32], 999, 1),
        Err(
            ModerationOrchestratorError::PanelNotificationClockRollback {
                current: 1_000,
                observed: 999,
            }
        )
    );
    assert!(matches!(
        orchestrator.release_panel_notification_claim(
            claim.notification.notification_id,
            [0xB2; 32],
            claim.lease_token,
            ModerationPanelNotificationFailureV1::NotDelivered,
            1_001,
        ),
        Err(ModerationOrchestratorError::PanelNotificationClaimConflict {
            notification_id
        }) if notification_id == claim.notification.notification_id
    ));
    assert_eq!(
        orchestrator.finalize_panel_notification_delivery(
            claim.worker_id,
            claim.lease_token,
            ModerationPanelNotificationDeliveryReceiptV1 {
                notification_id: claim.notification.notification_id,
                receipt_digest: [0; 32],
                delivered_at_unix_ms: 1_001,
            },
            1_001,
        ),
        Err(ModerationOrchestratorError::InvalidPanelNotificationReceipt)
    );
}
#[test]
fn panel_notification_checkpoint_tampering_and_old_versions_fail_closed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let (snapshot, _) = awaiting_acceptance_snapshot(2, [0x25; 32], governance);
    let reader = Arc::new(MockSnapshotReader::new(snapshot));
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
    let bounds = config(&temp, "panel-tamper.norito");
    let orchestrator = ModerationOrchestratorV1::open(
        bounds.clone(),
        deps(Arc::clone(&reader), Arc::clone(&submitter)),
    )
    .expect("orchestrator");
    orchestrator.reconcile().expect("queue notifications");
    orchestrator
        .claim_panel_notifications([0xA1; 32], 1_000, 3)
        .expect("durable claims");
    drop(orchestrator);
    let original = std::fs::read(&bounds.checkpoint_path).expect("read checkpoint");
    let limits = checkpoint_decode_limits(bounds.checkpoint_max_bytes).expect("decode limits");
    let mut checkpoint =
        decode_from_bytes_with_limits::<ModerationOrchestratorCheckpointV1>(&original, limits)
            .expect("decode checkpoint");
    checkpoint.panel_notifications[0].lease_expires_at_unix_ms = checkpoint.panel_notifications[0]
        .lease_expires_at_unix_ms
        .map(|value| value.saturating_add(1));
    std::fs::write(
        &bounds.checkpoint_path,
        norito::to_bytes(&checkpoint).expect("encode tampered checkpoint"),
    )
    .expect("write tampered checkpoint");
    assert!(matches!(
        ModerationOrchestratorV1::open(
            bounds.clone(),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        ),
        Err(ModerationOrchestratorError::CheckpointCorrupt(_))
    ));
    for version in [2, 3, 4] {
        let mut old_checkpoint =
            decode_from_bytes_with_limits::<ModerationOrchestratorCheckpointV1>(&original, limits)
                .expect("decode original checkpoint");
        old_checkpoint.version = version;
        std::fs::write(
            &bounds.checkpoint_path,
            norito::to_bytes(&old_checkpoint).expect("encode old checkpoint"),
        )
        .expect("write old checkpoint");
        assert!(matches!(
            ModerationOrchestratorV1::open(
                bounds.clone(),
                deps(Arc::clone(&reader), Arc::clone(&submitter)),
            ),
            Err(ModerationOrchestratorError::CheckpointCorrupt(message))
                if message.contains("unsupported checkpoint version")
        ));
    }
}
#[test]
fn panel_notification_source_provenance_mismatch_is_rejected() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let (mut snapshot, _) = awaiting_acceptance_snapshot(2, [0x26; 32], governance);
    snapshot.events[0].event = SorafsModerationLedgerEvent::new(
        SorafsModerationLedgerEventKind::SortitionFinalized,
        Some("case-failover".to_owned()),
        Some("round-1".to_owned()),
        account(98),
        21,
    );
    let orchestrator = ModerationOrchestratorV1::open(
        config(&temp, "panel-provenance.norito"),
        deps(
            Arc::new(MockSnapshotReader::new(snapshot)),
            Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
        ),
    )
    .expect("orchestrator");
    assert!(matches!(
        orchestrator.reconcile(),
        Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(message))
            if message.contains("sortition event provenance")
    ));
}
#[test]
fn panel_notification_scan_rejects_cross_snapshot_event_gaps() {
    let temp = tempfile::tempdir().expect("tempdir");
    let governance = account(99);
    let (awaiting, _) = awaiting_acceptance_snapshot(2, [0x2B; 32], governance.clone());
    let reader = Arc::new(MockSnapshotReader::new(awaiting));
    let orchestrator = ModerationOrchestratorV1::open(
        config(&temp, "panel-event-gap.norito"),
        deps(
            Arc::clone(&reader),
            Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
        ),
    )
    .expect("orchestrator");
    orchestrator.reconcile().expect("scan sortition event");
    let activated = activated_case_snapshot(3, [0x2C; 32], governance.clone());
    reader.replace(finalized_case_snapshot(
        activated, 4, [0x2D; 32], governance,
    ));
    assert!(matches!(
        orchestrator.reconcile(),
        Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(message))
            if message.contains("sequence gap")
    ));
}
#[test]
fn same_tip_with_a_changed_finalized_timestamp_is_rejected_as_equivocation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(2, [2; 32])));
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
        observed_finalized_height: 2,
    }));
    let orchestrator = ModerationOrchestratorV1::open(
        config(&temp, "timestamp-equivocation.norito"),
        deps(Arc::clone(&reader), submitter),
    )
    .expect("orchestrator");
    orchestrator.reconcile().expect("initial finalized tip");
    let mut forged = empty_snapshot(2, [2; 32]);
    forged.finalized_at_unix_ms = forged.finalized_at_unix_ms.saturating_add(1);
    reader.replace(forged);
    assert_eq!(
        orchestrator.reconcile(),
        Err(ModerationOrchestratorError::FinalizedEquivocation { height: 2 })
    );
}
#[test]
fn authenticated_request_binding_is_exact_and_canonical() {
    let authority = account(1);
    let action = policy_action(policy(1));
    let first = moderation_request_binding_digest_v1(
        "POST",
        "/v1/sorafs/moderation/actions?revision=1",
        b"body",
        &authority,
        &action,
    )
    .expect("canonical binding");
    let changed_body = moderation_request_binding_digest_v1(
        "POST",
        "/v1/sorafs/moderation/actions?revision=1",
        b"changed",
        &authority,
        &action,
    )
    .expect("canonical binding");
    let changed_query = moderation_request_binding_digest_v1(
        "POST",
        "/v1/sorafs/moderation/actions?revision=2",
        b"body",
        &authority,
        &action,
    )
    .expect("canonical binding");
    assert_ne!(first, changed_body);
    assert_ne!(first, changed_query);
    assert!(matches!(
        moderation_request_binding_digest_v1(
            "post",
            "/v1/sorafs/moderation/actions",
            b"body",
            &authority,
            &action,
        ),
        Err(ModerationOrchestratorError::InvalidRequestBinding)
    ));
    assert!(matches!(
        moderation_request_binding_digest_v1(
            "POST",
            "/v1/sorafs/../moderation/actions",
            b"body",
            &authority,
            &action,
        ),
        Err(ModerationOrchestratorError::InvalidRequestBinding)
    ));
}
#[cfg(unix)]
#[test]
fn checkpoint_failure_latches_the_process_fail_closed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
        observed_finalized_height: 1,
    }));
    let bounds = config(&temp, "checkpoint.norito");
    let checkpoint_path = bounds.checkpoint_path.clone();
    let orchestrator =
        ModerationOrchestratorV1::open(bounds, deps(reader, submitter)).expect("orchestrator");
    std::fs::remove_file(&checkpoint_path).expect("remove checkpoint cache");
    std::os::unix::fs::symlink(
        checkpoint_path.with_extension("untrusted-target"),
        &checkpoint_path,
    )
    .expect("install checkpoint symlink");
    assert!(matches!(
        orchestrator.reconcile(),
        Err(ModerationOrchestratorError::CheckpointIo(_))
    ));
    std::fs::remove_file(&checkpoint_path).expect("remove checkpoint symlink");
    assert_eq!(
        orchestrator.reconcile(),
        Err(ModerationOrchestratorError::DurabilityFaulted)
    );
    assert!(orchestrator.snapshot().is_none());
}
include!("terminal_handoff_tests.rs");
include!("checkpoint_store_tests.rs");
