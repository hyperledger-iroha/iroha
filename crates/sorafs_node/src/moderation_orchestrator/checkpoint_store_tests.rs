#[derive(Debug)]
struct MockCheckpointStore {
    handle: String,
    qualification: Mutex<
        Result<ModerationRuntimeProviderQualificationV1, ModerationRuntimeProviderReadinessErrorV1>,
    >,
    latest: Mutex<Option<ModerationCheckpointStoreRecordV1>>,
    attestation_signing_key: SigningKey,
    attestation_calls: AtomicUsize,
    next_cas_behavior: AtomicUsize,
}
impl Default for MockCheckpointStore {
    fn default() -> Self {
        Self {
            handle: CHECKPOINT_STORE_HANDLE.to_owned(),
            qualification: Mutex::new(Ok(CHECKPOINT_STORE_QUALIFICATION)),
            latest: Mutex::new(None),
            attestation_signing_key: SigningKey::from_bytes(
                &CHECKPOINT_STORE_ATTESTATION_SIGNING_SEED,
            ),
            attestation_calls: AtomicUsize::new(0),
            next_cas_behavior: AtomicUsize::new(0),
        }
    }
}
impl MockCheckpointStore {
    fn with_handle(handle: impl Into<String>) -> Self {
        Self {
            handle: handle.into(),
            ..Self::default()
        }
    }
    fn set_qualification(
        &self,
        qualification: Result<
            ModerationRuntimeProviderQualificationV1,
            ModerationRuntimeProviderReadinessErrorV1,
        >,
    ) {
        *self.qualification.lock().expect("checkpoint qualification") = qualification;
    }
    fn fail_next_cas(&self, behavior: usize) {
        self.next_cas_behavior
            .store(behavior, AtomicOrdering::SeqCst);
    }
    fn fail_cas_after_one_success(&self) {
        self.next_cas_behavior.store(4, AtomicOrdering::SeqCst);
    }
    fn latest(&self) -> ModerationCheckpointStoreRecordV1 {
        self.latest
            .lock()
            .expect("checkpoint latest")
            .clone()
            .expect("committed checkpoint")
    }
    fn attestation_calls(&self) -> usize {
        self.attestation_calls.load(AtomicOrdering::SeqCst)
    }
    fn replace_latest(&self, record: ModerationCheckpointStoreRecordV1) {
        *self.latest.lock().expect("checkpoint latest") = Some(record);
    }
}
impl ModerationRuntimeProviderV1 for MockCheckpointStore {
    fn handle(&self) -> &str {
        &self.handle
    }
    fn qualification(
        &self,
    ) -> Result<ModerationRuntimeProviderQualificationV1, ModerationRuntimeProviderReadinessErrorV1>
    {
        *self.qualification.lock().expect("checkpoint qualification")
    }
}
impl ModerationCheckpointStoreV1 for MockCheckpointStore {
    fn attestation_public_key(&self) -> [u8; 32] {
        self.attestation_signing_key.verifying_key().to_bytes()
    }
    fn load_latest(
        &self,
    ) -> Result<Option<ModerationCheckpointStoreRecordV1>, ModerationCheckpointStoreExternalErrorV1>
    {
        Ok(self.latest.lock().expect("checkpoint latest").clone())
    }
    fn compare_and_swap_latest(
        &self,
        expected_revision: Option<[u8; 32]>,
        next: &ModerationCheckpointStoreRecordV1,
    ) -> Result<(), ModerationCheckpointStoreExternalErrorV1> {
        let behavior = self.next_cas_behavior.swap(0, AtomicOrdering::SeqCst);
        if behavior == 1 {
            return Err(ModerationCheckpointStoreExternalErrorV1::Unavailable);
        }
        if behavior == 3 {
            return Err(ModerationCheckpointStoreExternalErrorV1::Ambiguous);
        }
        let mut latest = self.latest.lock().expect("checkpoint latest");
        if latest.as_ref().map(|record| record.revision) != expected_revision {
            return Err(ModerationCheckpointStoreExternalErrorV1::Rejected);
        }
        *latest = Some(next.clone());
        if behavior == 4 {
            // Let a caller seal its write-ahead reservation, then model an
            // unapplied ambiguous failure at the following commit boundary.
            self.next_cas_behavior.store(3, AtomicOrdering::SeqCst);
        }
        if behavior == 2 {
            Err(ModerationCheckpointStoreExternalErrorV1::Ambiguous)
        } else {
            Ok(())
        }
    }
    fn attest_terminal_set(
        &self,
        statement: &ModerationPanelNotificationSourceAttestationV1,
    ) -> Result<[u8; 64], ModerationCheckpointStoreExternalErrorV1> {
        self.attestation_calls.fetch_add(1, AtomicOrdering::SeqCst);
        let latest = self.latest.lock().expect("checkpoint latest");
        let Some(latest) = latest.as_ref() else {
            return Err(ModerationCheckpointStoreExternalErrorV1::Rejected);
        };
        if statement.checkpoint_namespace_digest != latest.namespace_digest
            || statement.checkpoint_generation != latest.checkpoint_generation
            || statement.checkpoint_revision != latest.revision
            || statement.checkpoint_digest != latest.checkpoint_digest
            || statement.attestor_handle != latest.checkpoint_store_handle
            || statement.attestor_revision != latest.checkpoint_store_revision
            || statement.attestor_policy_digest != latest.checkpoint_store_policy_digest
            || statement.attestor_public_key != self.attestation_public_key()
            || validate_moderation_panel_notification_source_attestation_for_broker_v1(
                statement,
                &statement.network_id,
                CHECKPOINT_STORE_HANDLE,
                CHECKPOINT_STORE_QUALIFICATION,
                self.attestation_public_key(),
                latest,
            )
            .is_err()
        {
            return Err(ModerationCheckpointStoreExternalErrorV1::Rejected);
        }
        Ok(self
            .attestation_signing_key
            .sign(&panel_notification_source_attestation_message(statement))
            .to_bytes())
    }
}
fn deps_with_checkpoint_store(
    reader: Arc<MockSnapshotReader>,
    submitter: Arc<MockSubmitter>,
    checkpoint_store: Arc<MockCheckpointStore>,
) -> ModerationOrchestratorDepsV1 {
    let mut runtime_deps = deps(reader, submitter);
    runtime_deps.checkpoint_store = checkpoint_store;
    runtime_deps
}
#[test]
fn checkpoint_store_startup_rejects_substituted_stale_and_test_marked_providers() {
    let temp = tempfile::tempdir().expect("tempdir");
    let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
    for store in [
        Arc::new(MockCheckpointStore::with_handle(
            "sealed-cas:moderation-checkpoint-substitute",
        )),
        Arc::new(MockCheckpointStore::with_handle(
            "test:moderation-checkpoint-primary",
        )),
    ] {
        assert!(matches!(
            ModerationOrchestratorV1::open(
                config(&temp, "provider-rejected.norito"),
                deps_with_checkpoint_store(reader.clone(), submitter.clone(), store),
            ),
            Err(ModerationOrchestratorError::InvalidConfiguration(_))
        ));
    }
    let stale = Arc::new(MockCheckpointStore::default());
    stale.set_qualification(Err(ModerationRuntimeProviderReadinessErrorV1::Rejected));
    assert!(matches!(
        ModerationOrchestratorV1::open(
            config(&temp, "provider-stale.norito"),
            deps_with_checkpoint_store(reader, submitter, stale),
        ),
        Err(ModerationOrchestratorError::InvalidConfiguration(_))
    ));
}
#[test]
fn ambiguous_checkpoint_commit_is_resolved_only_by_exact_authoritative_readback() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = Arc::new(MockCheckpointStore::default());
    let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
    let orchestrator = ModerationOrchestratorV1::open(
        config(&temp, "checkpoint-ambiguous.norito"),
        deps_with_checkpoint_store(reader, submitter, store.clone()),
    )
    .expect("open");
    store.fail_next_cas(2);
    orchestrator
        .reconcile()
        .expect("exact authoritative readback resolves ambiguity");
    assert_eq!(store.latest().checkpoint_generation, 1);
}
#[test]
fn ambiguous_checkpoint_commit_without_exact_readback_fences_the_replica() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = Arc::new(MockCheckpointStore::default());
    let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
    let orchestrator = ModerationOrchestratorV1::open(
        config(&temp, "checkpoint-ambiguous-mismatch.norito"),
        deps_with_checkpoint_store(reader, submitter, store.clone()),
    )
    .expect("open");
    store.fail_next_cas(3);
    assert_eq!(
        orchestrator.reconcile(),
        Err(ModerationOrchestratorError::CheckpointStoreFenced)
    );
    assert_eq!(
        orchestrator.reconcile(),
        Err(ModerationOrchestratorError::DurabilityFaulted)
    );
    assert_eq!(store.latest().checkpoint_generation, 0);
}
#[test]
fn competing_replica_is_fenced_before_overwriting_a_committed_successor() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = Arc::new(MockCheckpointStore::default());
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
    let first = ModerationOrchestratorV1::open(
        config(&temp, "checkpoint-first-cache.norito"),
        deps_with_checkpoint_store(
            Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32]))),
            submitter.clone(),
            store.clone(),
        ),
    )
    .expect("first replica");
    let second = ModerationOrchestratorV1::open(
        config(&temp, "checkpoint-second-cache.norito"),
        deps_with_checkpoint_store(
            Arc::new(MockSnapshotReader::new(empty_snapshot(2, [2; 32]))),
            submitter,
            store.clone(),
        ),
    )
    .expect("second replica");
    first.reconcile().expect("first successor");
    assert_eq!(
        second.reconcile(),
        Err(ModerationOrchestratorError::CheckpointStoreFenced)
    );
    assert_eq!(
        second.reconcile(),
        Err(ModerationOrchestratorError::DurabilityFaulted)
    );
    assert_eq!(store.latest().checkpoint_generation, 1);
}
#[test]
fn sealed_record_replay_and_equivocation_fail_startup_closed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = Arc::new(MockCheckpointStore::default());
    let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
    let checkpoint = config(&temp, "checkpoint-equivocation.norito");
    let orchestrator = ModerationOrchestratorV1::open(
        checkpoint.clone(),
        deps_with_checkpoint_store(reader.clone(), submitter.clone(), store.clone()),
    )
    .expect("open");
    orchestrator.reconcile().expect("commit successor");
    drop(orchestrator);
    let mut equivocal = store.latest();
    equivocal.checkpoint_store_revision += 1;
    store.replace_latest(equivocal);
    assert_eq!(
        ModerationOrchestratorV1::open(
            checkpoint,
            deps_with_checkpoint_store(reader, submitter, store),
        )
        .expect_err("equivocal record must fail"),
        ModerationOrchestratorError::CheckpointStoreEquivocation
    );
}
#[test]
fn authoritative_store_recovers_when_the_verified_local_cache_is_absent() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = Arc::new(MockCheckpointStore::default());
    let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
    let checkpoint = config(&temp, "checkpoint-cache-recovery.norito");
    let orchestrator = ModerationOrchestratorV1::open(
        checkpoint.clone(),
        deps_with_checkpoint_store(reader.clone(), submitter.clone(), store.clone()),
    )
    .expect("open");
    orchestrator.reconcile().expect("commit checkpoint");
    let expected = orchestrator.snapshot().expect("snapshot");
    drop(orchestrator);
    std::fs::remove_file(&checkpoint.checkpoint_path).expect("remove cache");
    let recovered = ModerationOrchestratorV1::open(
        checkpoint,
        deps_with_checkpoint_store(reader, submitter, store),
    )
    .expect("recover from authority");
    assert_eq!(recovered.snapshot(), Some(expected));
}
#[test]
fn authoritative_store_rollback_behind_local_cache_fails_startup_closed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = Arc::new(MockCheckpointStore::default());
    let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
    let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
    let checkpoint = config(&temp, "checkpoint-store-rollback.norito");
    let orchestrator = ModerationOrchestratorV1::open(
        checkpoint.clone(),
        deps_with_checkpoint_store(reader.clone(), submitter.clone(), store.clone()),
    )
    .expect("open");
    let genesis = store.latest();
    orchestrator.reconcile().expect("commit successor");
    assert_eq!(store.latest().checkpoint_generation, 1);
    drop(orchestrator);
    store.replace_latest(genesis);
    assert_eq!(
        ModerationOrchestratorV1::open(
            checkpoint,
            deps_with_checkpoint_store(reader, submitter, store),
        )
        .expect_err("authoritative rollback must fail"),
        ModerationOrchestratorError::CheckpointStoreEquivocation
    );
}
