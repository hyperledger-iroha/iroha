#[tokio::test]
async fn completed_musubi_capture_scanner_pages_and_restarts_at_a_later_head() {
    let ledger = Arc::new(CaptureScannerLedgerV1::new(
        vec![
            fixture_completed_musubi_capture_row(0x39, 0x86),
            fixture_completed_musubi_capture_row(0x3A, 0x87),
        ],
        8,
        CaptureScannerLedgerFaultV1::None,
    ));
    let mut scanner = ProviderIngestCompletedMusubiCaptureScannerV1::new(
        CompletedMusubiStoreInstanceV1::new(),
        LOCAL_PROVIDER,
        test_network_id(),
        1,
        Arc::clone(&ledger),
    )
    .expect("construct bounded capture scanner");

    let first = scanner.next_page().await.expect("first capture page");
    assert_eq!(first.finalized_cursor(), cursor(8));
    assert_eq!(first.candidates().len(), 1);
    assert!(!first.scan_complete());
    assert!(
        first.candidates()[0]
            .completed_claim()
            .matches_authorization(first.candidates()[0].authorization())
    );

    let second = scanner.next_page().await.expect("second capture page");
    assert_eq!(second.finalized_cursor(), cursor(8));
    assert_eq!(second.candidates().len(), 1);
    assert!(second.scan_complete());
    assert_ne!(
        first.candidates()[0].completed_claim().replication_order(),
        second.candidates()[0].completed_claim().replication_order()
    );

    let unchanged = scanner
        .next_page()
        .await
        .expect("unchanged finalized head probe");
    assert_eq!(unchanged.finalized_cursor(), cursor(8));
    assert!(unchanged.candidates().is_empty());
    assert!(unchanged.scan_complete());

    ledger.set_fault(CaptureScannerLedgerFaultV1::SubstitutedArchiveBinding);
    assert!(matches!(
        scanner.next_page().await,
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
    ));
    ledger.set_fault(CaptureScannerLedgerFaultV1::None);

    ledger.set_finalized_height(9);
    let later = scanner
        .next_page()
        .await
        .expect("fresh capture scan at later head");
    assert_eq!(later.finalized_cursor(), cursor(9));
    assert_eq!(later.candidates().len(), 1);
    assert!(!later.scan_complete());
    assert_eq!(
        later.candidates()[0]
            .completed_claim()
            .observed_finalized_cursor(),
        cursor(9)
    );
    let later_terminal = scanner
        .next_page()
        .await
        .expect("terminal page at later head");
    assert_eq!(later_terminal.finalized_cursor(), cursor(9));
    assert_eq!(later_terminal.candidates().len(), 1);
    assert!(later_terminal.scan_complete());
    let later_unchanged = scanner
        .next_page()
        .await
        .expect("unchanged later-head probe");
    assert_eq!(later_unchanged.finalized_cursor(), cursor(9));
    assert!(later_unchanged.candidates().is_empty());
    assert!(later_unchanged.scan_complete());
    assert_eq!(ledger.requested_limits(), vec![1; 7]);
    assert_eq!(ledger.requested_generations(), vec![1, 2, 3, 4, 4, 5, 6]);
}

#[tokio::test]
async fn completed_musubi_capture_scanner_rejects_malformed_and_substituted_raw_pages() {
    for fault in [
        CaptureScannerLedgerFaultV1::MalformedRow,
        CaptureScannerLedgerFaultV1::SubstitutedArchiveBinding,
        CaptureScannerLedgerFaultV1::MutatedAfterSigning,
        CaptureScannerLedgerFaultV1::WrongSigningKey,
        CaptureScannerLedgerFaultV1::RequestMismatch,
    ] {
        let ledger = Arc::new(CaptureScannerLedgerV1::new(
            vec![fixture_completed_musubi_capture_row(0x3B, 0x88)],
            8,
            fault,
        ));
        let mut scanner = ProviderIngestCompletedMusubiCaptureScannerV1::new(
            CompletedMusubiStoreInstanceV1::new(),
            LOCAL_PROVIDER,
            test_network_id(),
            1,
            ledger,
        )
        .expect("construct capture scanner");
        assert!(scanner.next_page().await.is_err());
    }
}

#[tokio::test]
async fn completed_musubi_capture_scanner_retries_an_unavailable_page_without_generation_drift() {
    let ledger = Arc::new(CaptureScannerLedgerV1::new(
        vec![fixture_completed_musubi_capture_row(0x3C, 0x89)],
        8,
        CaptureScannerLedgerFaultV1::Unavailable,
    ));
    let mut scanner = ProviderIngestCompletedMusubiCaptureScannerV1::new(
        CompletedMusubiStoreInstanceV1::new(),
        LOCAL_PROVIDER,
        test_network_id(),
        1,
        Arc::clone(&ledger),
    )
    .expect("construct capture scanner");

    assert!(matches!(
        scanner.next_page().await,
        Err(ProviderIngestRuntimeErrorV1::FinalizedLedgerUnavailable)
    ));
    ledger.set_fault(CaptureScannerLedgerFaultV1::None);
    let repaired = scanner
        .next_page()
        .await
        .expect("retry exact repaired page");
    assert_eq!(repaired.finalized_cursor(), cursor(8));
    assert_eq!(repaired.candidates().len(), 1);
    assert!(repaired.scan_complete());
    assert_eq!(ledger.requested_limits(), vec![1, 1]);
    assert_eq!(ledger.requested_generations(), vec![1, 1]);
}

#[tokio::test]
async fn completed_musubi_capture_scanner_rejects_a_previous_generation_replay() {
    let ledger = Arc::new(CaptureScannerLedgerV1::new(
        vec![fixture_completed_musubi_capture_row(0x3D, 0x8A)],
        8,
        CaptureScannerLedgerFaultV1::None,
    ));
    let mut scanner = ProviderIngestCompletedMusubiCaptureScannerV1::new(
        CompletedMusubiStoreInstanceV1::new(),
        LOCAL_PROVIDER,
        test_network_id(),
        1,
        Arc::clone(&ledger),
    )
    .expect("construct replay-check scanner");

    let first = scanner.next_page().await.expect("generation-one page");
    assert!(first.scan_complete());
    let after_first = scanner.progress();
    ledger.set_fault(CaptureScannerLedgerFaultV1::ReplayPrevious);
    assert!(matches!(
        scanner.next_page().await,
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
    ));
    assert_eq!(scanner.progress(), after_first);
    ledger.set_fault(CaptureScannerLedgerFaultV1::None);
    let exact_retry = scanner.next_page().await.expect("generation-two retry");
    assert!(exact_retry.scan_complete());
    assert!(exact_retry.candidates().is_empty());
    assert_eq!(ledger.requested_generations(), vec![1, 2, 2]);
}

#[test]
fn completed_musubi_capture_transcript_ignores_ambient_norito_flags() {
    let ledger = CaptureScannerLedgerV1::new(Vec::new(), 8, CaptureScannerLedgerFaultV1::None);
    let request = ProviderIngestCompletedMusubiCaptureRequestV1::new(
        ledger.binding.clone(),
        None,
        None,
        1,
        1,
    )
    .expect("canonical transcript request");
    let mut row = fixture_completed_musubi_capture_row(0x3E, 0x8B);
    row.pin.finalized_cursor = PinManifestFinalizedCursorV1 {
        height: 8,
        block_hash: cursor(8).block_hash,
    };
    let page = ProviderIngestCompletedMusubiCaptureSourcePageV1 {
        network_id: test_network_id(),
        provider_id: LOCAL_PROVIDER,
        finalized_cursor: cursor(8),
        finalized_block_time_ms: 8_000,
        rows: vec![row],
        next_after_order_id: None,
    };
    let expected = provider_ingest_completed_musubi_capture_transcript_digest_v1(&request, &page)
        .expect("baseline transcript digest");
    validate_completed_musubi_capture_source_page(
        &page,
        None,
        None,
        1,
        test_network_id(),
        LOCAL_PROVIDER,
    )
    .expect("baseline canonical source page");
    let original_flags = norito::core::get_decode_flags();
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    {
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_eq!(norito::core::get_decode_flags(), alternate_flags);
        assert_eq!(
            provider_ingest_completed_musubi_capture_transcript_digest_v1(&request, &page)
                .expect("ambient-independent transcript digest"),
            expected
        );
        validate_completed_musubi_capture_source_page(
            &page,
            None,
            None,
            1,
            test_network_id(),
            LOCAL_PROVIDER,
        )
        .expect("ambient-independent canonical order validation");
        assert_eq!(norito::core::get_decode_flags(), alternate_flags);
    }
    assert_eq!(norito::core::get_decode_flags(), original_flags);
}

struct CaptureCoordinatorProbeLedgerV1 {
    binding: ProviderIngestCompletedMusubiCaptureVerifierBindingV1,
    binding_available: AtomicBool,
    binding_calls: AtomicUsize,
    page_reads: AtomicUsize,
}

impl CaptureCoordinatorProbeLedgerV1 {
    fn new(binding_available: bool, key_seed: u8) -> Self {
        let key_pair = KeyPair::from_seed(vec![key_seed; 32], Algorithm::Ed25519);
        let public_key: [u8; 32] = key_pair
            .public_key()
            .to_bytes()
            .1
            .try_into()
            .expect("Ed25519 coordinator probe key");
        Self {
            binding:
                ProviderIngestCompletedMusubiCaptureVerifierBindingV1::try_from_untrusted_reader_parts(
                    test_network_id(),
                    LOCAL_PROVIDER,
                    u64::from(key_seed).max(1),
                    public_key,
                )
                .expect("valid coordinator probe binding"),
            binding_available: AtomicBool::new(binding_available),
            binding_calls: AtomicUsize::new(0),
            page_reads: AtomicUsize::new(0),
        }
    }
}

impl ProviderIngestCompletedMusubiSignedCaptureLedgerV1 for CaptureCoordinatorProbeLedgerV1 {
    fn capture_verifier_binding(
        &self,
    ) -> Result<
        ProviderIngestCompletedMusubiCaptureVerifierBindingV1,
        ProviderIngestFinalizedLedgerErrorV1,
    > {
        self.binding_calls.fetch_add(1, Ordering::SeqCst);
        if self.binding_available.load(Ordering::SeqCst) {
            Ok(self.binding.clone())
        } else {
            Err(ProviderIngestFinalizedLedgerErrorV1::Unavailable)
        }
    }

    fn read_signed_completed_musubi_capture_page(
        &self,
        _request: ProviderIngestCompletedMusubiCaptureRequestV1,
    ) -> ProviderIngestFutureV1<
        '_,
        Result<
            ProviderIngestCompletedMusubiSignedCapturePageV1,
            ProviderIngestFinalizedLedgerErrorV1,
        >,
    > {
        self.page_reads.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(ProviderIngestFinalizedLedgerErrorV1::Unavailable) })
    }
}

fn capture_coordinator_test_handle(root: &std::path::Path) -> NodeHandle {
    NodeHandle::try_new(
        StorageConfig::builder()
            .enabled(true)
            .provider_id(Some(ProviderId::new(LOCAL_PROVIDER)))
            .data_dir(root.join("storage"))
            .provider_ingest_outbox_policy(Some(ProviderIngestOutboxPolicyV1::default()))
            .build(),
    )
    .expect("open capture coordinator test node")
}

#[test]
fn completed_musubi_capture_coordinator_tenure_is_take_once_and_reader_stable() {
    let first_root = tempfile::tempdir().expect("first coordinator root");
    let second_root = tempfile::tempdir().expect("second coordinator root");
    let failed_root = tempfile::tempdir().expect("failed coordinator root");
    let handle = capture_coordinator_test_handle(first_root.path());
    let cloned_handle = handle.clone();
    let retained_reader = Arc::new(CaptureCoordinatorProbeLedgerV1::new(false, 0xC1));
    let substituted_reader = Arc::new(CaptureCoordinatorProbeLedgerV1::new(true, 0xC2));

    let mut coordinator = handle
        .take_provider_ingest_completed_musubi_capture_coordinator(
            test_network_id(),
            1,
            retained_reader.clone(),
        )
        .expect("reserve first coordinator tenure");
    assert_eq!(retained_reader.binding_calls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        cloned_handle.take_provider_ingest_completed_musubi_capture_coordinator(
            test_network_id(),
            1,
            substituted_reader.clone(),
        ),
        Err(FinalizedProviderIngestError::CompletedMusubiCaptureCoordinatorTaken)
    ));
    assert_eq!(substituted_reader.binding_calls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        coordinator.try_activate(),
        Err(ProviderIngestRuntimeErrorV1::FinalizedLedgerUnavailable)
    ));
    assert_eq!(retained_reader.binding_calls.load(Ordering::SeqCst), 1);
    assert_eq!(retained_reader.page_reads.load(Ordering::SeqCst), 0);

    retained_reader
        .binding_available
        .store(true, Ordering::SeqCst);
    coordinator
        .try_activate()
        .expect("retry exact retained reader after genesis becomes available");
    assert!(coordinator.active_scanner_mut().is_some());
    assert_eq!(retained_reader.binding_calls.load(Ordering::SeqCst), 2);
    drop(coordinator);
    assert!(matches!(
        cloned_handle.take_provider_ingest_completed_musubi_capture_coordinator(
            test_network_id(),
            1,
            substituted_reader.clone(),
        ),
        Err(FinalizedProviderIngestError::CompletedMusubiCaptureCoordinatorTaken)
    ));
    assert_eq!(substituted_reader.binding_calls.load(Ordering::SeqCst), 0);

    let restarted_handle = capture_coordinator_test_handle(second_root.path());
    let mut restarted = restarted_handle
        .take_provider_ingest_completed_musubi_capture_coordinator(
            test_network_id(),
            1,
            substituted_reader.clone(),
        )
        .expect("separately constructed handle owns an independent tenure");
    assert_eq!(substituted_reader.binding_calls.load(Ordering::SeqCst), 0);
    restarted
        .try_activate()
        .expect("bind reader under independent restarted handle");
    assert_eq!(substituted_reader.binding_calls.load(Ordering::SeqCst), 1);

    let failed_handle = capture_coordinator_test_handle(failed_root.path());
    let never_read = Arc::new(CaptureCoordinatorProbeLedgerV1::new(true, 0xC3));
    assert!(matches!(
        failed_handle.take_provider_ingest_completed_musubi_capture_coordinator(
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
                Hash::prehashed([0; 32]),
            )),
            1,
            never_read.clone(),
        ),
        Err(FinalizedProviderIngestError::Runtime(
            ProviderIngestRuntimeErrorV1::InvalidNetworkId
        ))
    ));
    assert!(matches!(
        failed_handle.take_provider_ingest_completed_musubi_capture_coordinator(
            test_network_id(),
            1,
            never_read.clone(),
        ),
        Err(FinalizedProviderIngestError::CompletedMusubiCaptureCoordinatorTaken)
    ));
    assert_eq!(never_read.binding_calls.load(Ordering::SeqCst), 0);
    assert_eq!(never_read.page_reads.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn completed_musubi_capture_reconciliation_retries_without_skipping_and_enqueues_once() {
    let fixture = verified_attestation_bundle_fixture(0xEC);
    let manifest = completed_attestation_manifest(&fixture);
    let source_row = completed_attestation_capture_source_row(&fixture, &manifest);
    let ledger = Arc::new(CaptureScannerLedgerV1::new(
        vec![source_row],
        8,
        CaptureScannerLedgerFaultV1::None,
    ));
    let temp_dir = tempfile::tempdir().expect("capture reconciliation tempdir");
    let temp_root = temp_dir
        .path()
        .canonicalize()
        .expect("canonical capture reconciliation tempdir");
    let handle = NodeHandle::try_new(
        StorageConfig::builder()
            .enabled(true)
            .provider_id(Some(ProviderId::new(LOCAL_PROVIDER)))
            .data_dir(temp_root.join("storage"))
            .provider_ingest_outbox_policy(Some(ProviderIngestOutboxPolicyV1::default()))
            .build(),
    )
    .expect("open capture reconciliation storage");
    let journal = MusubiProviderAttestationJournalV1::new(
        Arc::new(CaptureJournalMemoryStore::default()),
        MusubiProviderAttestationJournalPolicyV1::default(),
    )
    .expect("open capture reconciliation journal");
    let inventory = CaptureInventory::new(None);
    let mut foreign_scanner = ProviderIngestCompletedMusubiCaptureScannerV1::new(
        CompletedMusubiStoreInstanceV1::new(),
        LOCAL_PROVIDER,
        test_network_id(),
        1,
        Arc::clone(&ledger),
    )
    .expect("construct foreign-instance scanner");
    assert_eq!(
        handle
            .reconcile_provider_ingest_completed_musubi_capture_page(
                &mut foreign_scanner,
                &journal,
                &inventory,
            )
            .await,
        Err(ProviderIngestCompletedMusubiReconcileErrorV1::VerificationFailed)
    );
    assert!(
        ledger.requested_generations().is_empty(),
        "foreign scanner must fail before a signed page read is requested"
    );
    let mut coordinator = handle
        .take_provider_ingest_completed_musubi_capture_coordinator(
            test_network_id(),
            1,
            ledger.clone(),
        )
        .expect("reserve reconciliation coordinator");
    coordinator
        .try_activate()
        .expect("bind reconciliation scanner");
    let scanner = coordinator
        .active_scanner_mut()
        .expect("active reconciliation scanner");
    let cloned_handle = handle.clone();
    let initial_progress = scanner.progress();

    assert_eq!(
        cloned_handle
            .reconcile_provider_ingest_completed_musubi_capture_page(
                &mut *scanner,
                &journal,
                &inventory,
            )
            .await,
        Err(ProviderIngestCompletedMusubiReconcileErrorV1::AdmittedPlanUnavailable)
    );
    assert_eq!(
        scanner.progress(),
        initial_progress,
        "a failed page must restore its exact scanner continuation"
    );
    assert_eq!(inventory.readiness_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 0);

    let mut payload = fixture.payload.as_slice();
    handle
        .ingest_manifest(&manifest, &fixture.plan, &mut payload)
        .expect("admit completed Musubi payload");

    inventory.block_get();
    {
        let reconciliation = cloned_handle.reconcile_provider_ingest_completed_musubi_capture_page(
            &mut *scanner,
            &journal,
            &inventory,
        );
        tokio::pin!(reconciliation);
        tokio::select! {
            () = inventory.wait_until_get_entered() => {}
            result = &mut reconciliation => {
                panic!("blocked inventory read unexpectedly completed: {result:?}");
            }
            () = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                panic!("reconciliation did not reach the blocked inventory read");
            }
        }
    }
    inventory.unblock_get();
    assert_eq!(
        scanner.progress(),
        initial_progress,
        "dropping a reconciliation future must restore scanner progress"
    );

    let inserted = cloned_handle
        .reconcile_provider_ingest_completed_musubi_capture_page(
            &mut *scanner,
            &journal,
            &inventory,
        )
        .await
        .expect("verify and enqueue completed Musubi page");
    assert_eq!(inserted.finalized_cursor, cursor(8));
    assert_eq!(inserted.candidates, 1);
    assert_eq!(inserted.inserted, 1);
    assert_eq!(inserted.existing, 0);
    assert_eq!(inserted.inventory_suppressed, 0);
    assert!(inserted.scan_complete);
    assert_eq!(inventory.readiness_calls.load(Ordering::SeqCst), 3);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 2);
    assert_eq!(inventory.put_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.inventory_calls.load(Ordering::SeqCst), 0);

    assert_ne!(
        scanner.progress(),
        initial_progress,
        "successful reconciliation must retain the scanner's committed page progress"
    );
    scanner.restore_progress(initial_progress);
    let replayed = cloned_handle
        .reconcile_provider_ingest_completed_musubi_capture_page(
            &mut *scanner,
            &journal,
            &inventory,
        )
        .await
        .expect("idempotently replay exact completed Musubi page");
    assert_eq!(replayed.candidates, 1);
    assert_eq!(replayed.inserted, 0);
    assert_eq!(replayed.existing, 1);
    assert_eq!(replayed.inventory_suppressed, 0);
    assert!(replayed.scan_complete);
    assert_eq!(inventory.readiness_calls.load(Ordering::SeqCst), 3);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 2);
    assert_eq!(ledger.requested_limits(), vec![1, 1, 1, 1]);
    assert_eq!(ledger.requested_generations(), vec![1, 1, 1, 1]);
}

#[tokio::test]
async fn completed_musubi_capture_reconciliation_replays_a_durable_prefix_after_cancellation() {
    let first_fixture = verified_attestation_bundle_fixture(0xF1);
    let second_fixture = verified_attestation_bundle_fixture(0xF2);
    let first_manifest = completed_attestation_manifest(&first_fixture);
    let second_manifest = completed_attestation_manifest(&second_fixture);
    let first_order_id = [0xA1; 32];
    let second_order_id = [0xA2; 32];
    let first_row = completed_attestation_capture_source_row_with_order_id(
        &first_fixture,
        &first_manifest,
        first_order_id,
    );
    let second_row = completed_attestation_capture_source_row_with_order_id(
        &second_fixture,
        &second_manifest,
        second_order_id,
    );
    let ledger = Arc::new(CaptureScannerLedgerV1::new(
        vec![first_row, second_row],
        8,
        CaptureScannerLedgerFaultV1::None,
    ));
    let temp_dir = tempfile::tempdir().expect("capture prefix replay tempdir");
    let temp_root = temp_dir
        .path()
        .canonicalize()
        .expect("canonical capture prefix replay tempdir");
    let handle = NodeHandle::try_new(
        StorageConfig::builder()
            .enabled(true)
            .provider_id(Some(ProviderId::new(LOCAL_PROVIDER)))
            .data_dir(temp_root.join("storage"))
            .provider_ingest_outbox_policy(Some(ProviderIngestOutboxPolicyV1::default()))
            .build(),
    )
    .expect("open capture prefix replay storage");
    for (manifest, fixture) in [
        (&first_manifest, &first_fixture),
        (&second_manifest, &second_fixture),
    ] {
        let mut payload = fixture.payload.as_slice();
        handle
            .ingest_manifest(manifest, &fixture.plan, &mut payload)
            .expect("admit capture prefix replay payload");
    }
    let store = Arc::new(CaptureJournalMemoryStore::default());
    let journal = MusubiProviderAttestationJournalV1::new(
        store,
        MusubiProviderAttestationJournalPolicyV1::default(),
    )
    .expect("open capture prefix replay journal");
    let inventory = CaptureInventory::new(None);
    inventory.block_get_on_call(2);
    let mut coordinator = handle
        .take_provider_ingest_completed_musubi_capture_coordinator(
            test_network_id(),
            2,
            ledger.clone(),
        )
        .expect("reserve capture prefix replay coordinator");
    coordinator
        .try_activate()
        .expect("bind capture prefix replay scanner");
    let scanner = coordinator
        .active_scanner_mut()
        .expect("active capture prefix replay scanner");
    let initial_progress = scanner.progress();

    let first_request = ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
        &completed_attestation_claim_with_order_id(
            first_fixture.commitment.clone(),
            first_order_id,
        ),
        &first_fixture.verified,
    )
    .expect("derive first capture prefix request");
    let first_approval_id = musubi_provider_attestation_approval_id_v1(&first_request)
        .expect("derive first capture prefix approval ID");
    let second_request =
        ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
            &completed_attestation_claim_with_order_id(
                second_fixture.commitment.clone(),
                second_order_id,
            ),
            &second_fixture.verified,
        )
        .expect("derive second capture prefix request");
    let second_approval_id = musubi_provider_attestation_approval_id_v1(&second_request)
        .expect("derive second capture prefix approval ID");

    {
        let reconciliation = handle.reconcile_provider_ingest_completed_musubi_capture_page(
            &mut *scanner,
            &journal,
            &inventory,
        );
        tokio::pin!(reconciliation);
        tokio::select! {
            () = inventory.wait_until_get_entered() => {}
            result = &mut reconciliation => {
                panic!("second blocked inventory read unexpectedly completed: {result:?}");
            }
            () = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                panic!("reconciliation did not reach the second blocked inventory read");
            }
        }
    }
    assert_eq!(
        scanner.progress(),
        initial_progress,
        "cancellation after a durable prefix must restore exact page progress"
    );
    assert!(
        journal
            .status(first_approval_id)
            .await
            .expect("read first durable prefix status")
            .is_some(),
        "candidate one must be durable before candidate two blocks"
    );
    assert!(
        journal
            .status(second_approval_id)
            .await
            .expect("read second durable prefix status")
            .is_none(),
        "candidate two must remain absent while its inventory read is blocked"
    );
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 2);

    inventory.unblock_get();
    let retried = handle
        .reconcile_provider_ingest_completed_musubi_capture_page(
            &mut *scanner,
            &journal,
            &inventory,
        )
        .await
        .expect("retry capture page after durable prefix cancellation");
    assert_eq!(retried.finalized_cursor, cursor(8));
    assert_eq!(retried.candidates, 2);
    assert_eq!(retried.inserted, 1);
    assert_eq!(retried.existing, 1);
    assert_eq!(retried.inventory_suppressed, 0);
    assert!(retried.scan_complete);
    assert_ne!(scanner.progress(), initial_progress);
    assert!(
        journal
            .status(first_approval_id)
            .await
            .expect("reread first durable prefix status")
            .is_some()
    );
    assert!(
        journal
            .status(second_approval_id)
            .await
            .expect("read inserted suffix status")
            .is_some()
    );
    assert_eq!(inventory.readiness_calls.load(Ordering::SeqCst), 5);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 3);
    assert_eq!(inventory.put_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.inventory_calls.load(Ordering::SeqCst), 0);
    assert_eq!(ledger.requested_generations(), vec![1, 1]);

    scanner.restore_progress(initial_progress);
    let replayed = handle
        .reconcile_provider_ingest_completed_musubi_capture_page(
            &mut *scanner,
            &journal,
            &inventory,
        )
        .await
        .expect("replay fully retained capture page");
    assert_eq!(replayed.candidates, 2);
    assert_eq!(replayed.inserted, 0);
    assert_eq!(replayed.existing, 2);
    assert_eq!(replayed.inventory_suppressed, 0);
    assert!(replayed.scan_complete);
    assert_eq!(inventory.readiness_calls.load(Ordering::SeqCst), 5);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 3);
    assert_eq!(ledger.requested_generations(), vec![1, 1, 1]);
}

#[tokio::test]
async fn completed_musubi_pre_enqueue_probe_rejects_invalid_request_before_inventory() {
    let fixture = verified_attestation_bundle_fixture(0xEE);
    let claim = completed_attestation_claim(fixture.commitment.clone());
    let mut request = ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
        &claim,
        &fixture.verified,
    )
    .expect("derive probe request fixture");
    request.payload.version = MUSUBI_REGISTRY_VERSION_V1 + 1;
    let journal = MusubiProviderAttestationJournalV1::new(
        Arc::new(CaptureJournalMemoryStore::default()),
        MusubiProviderAttestationJournalPolicyV1::default(),
    )
    .expect("open invalid-request probe journal");
    let inventory = CaptureInventory::new(None);

    assert_eq!(
        journal
            .probe_pre_enqueue_with_inventory(&request, &inventory)
            .await,
        Err(
            crate::provider_attestation_journal::MusubiProviderAttestationJournalErrorV1::InvalidIntent
        )
    );
    assert_eq!(inventory.readiness_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.put_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.inventory_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn completed_musubi_capture_reconciliation_fails_closed_then_inventory_suppresses() {
    let fixture = verified_attestation_bundle_fixture(0xED);
    let manifest = completed_attestation_manifest(&fixture);
    let source_row = completed_attestation_capture_source_row(&fixture, &manifest);
    let ledger = Arc::new(CaptureScannerLedgerV1::new(
        vec![source_row],
        8,
        CaptureScannerLedgerFaultV1::None,
    ));
    let temp_dir = tempfile::tempdir().expect("inventory reconciliation tempdir");
    let temp_root = temp_dir
        .path()
        .canonicalize()
        .expect("canonical inventory reconciliation tempdir");
    let handle = NodeHandle::try_new(
        StorageConfig::builder()
            .enabled(true)
            .provider_id(Some(ProviderId::new(LOCAL_PROVIDER)))
            .data_dir(temp_root.join("storage"))
            .provider_ingest_outbox_policy(Some(ProviderIngestOutboxPolicyV1::default()))
            .build(),
    )
    .expect("open inventory reconciliation storage");
    let mut payload = fixture.payload.as_slice();
    handle
        .ingest_manifest(&manifest, &fixture.plan, &mut payload)
        .expect("admit inventory reconciliation payload");
    let store = Arc::new(CaptureJournalMemoryStore::default());
    let journal = MusubiProviderAttestationJournalV1::new(
        store.clone(),
        MusubiProviderAttestationJournalPolicyV1::default(),
    )
    .expect("open inventory reconciliation journal");
    let exact_item = completed_attestation_inventory_item(&fixture, false);
    let conflicting_item = completed_attestation_inventory_item(&fixture, true);
    assert_eq!(exact_item.scope(), conflicting_item.scope());
    assert_eq!(exact_item.key(), conflicting_item.key());
    assert_ne!(
        exact_item.attestation().payload,
        conflicting_item.attestation().payload
    );
    let inventory = CaptureInventory::new(Some(conflicting_item));
    let mut coordinator = handle
        .take_provider_ingest_completed_musubi_capture_coordinator(
            test_network_id(),
            1,
            ledger.clone(),
        )
        .expect("reserve inventory reconciliation coordinator");
    coordinator
        .try_activate()
        .expect("bind inventory reconciliation scanner");
    let scanner = coordinator
        .active_scanner_mut()
        .expect("active inventory reconciliation scanner");
    let initial_progress = scanner.progress();

    assert_eq!(
        handle
            .reconcile_provider_ingest_completed_musubi_capture_page(
                &mut *scanner,
                &journal,
                &inventory,
            )
            .await,
        Err(ProviderIngestCompletedMusubiReconcileErrorV1::InventoryRejected)
    );
    assert_eq!(scanner.progress(), initial_progress);
    assert!(
        store
            .checkpoint
            .lock()
            .expect("capture journal checkpoint lock")
            .is_none()
    );

    inventory.set_item(exact_item);
    inventory.set_get_error(Some(MusubiProviderAttestationInventoryErrorV1::Unavailable));
    assert_eq!(
        handle
            .reconcile_provider_ingest_completed_musubi_capture_page(
                &mut *scanner,
                &journal,
                &inventory,
            )
            .await,
        Err(ProviderIngestCompletedMusubiReconcileErrorV1::InventoryUnavailable)
    );
    assert_eq!(scanner.progress(), initial_progress);
    assert!(
        store
            .checkpoint
            .lock()
            .expect("capture journal checkpoint lock")
            .is_none()
    );

    inventory.set_get_error(None);
    let suppressed = handle
        .reconcile_provider_ingest_completed_musubi_capture_page(
            &mut *scanner,
            &journal,
            &inventory,
        )
        .await
        .expect("suppress exact authenticated inventory payload");
    assert_eq!(suppressed.finalized_cursor, cursor(8));
    assert_eq!(suppressed.candidates, 1);
    assert_eq!(suppressed.inserted, 0);
    assert_eq!(suppressed.existing, 0);
    assert_eq!(suppressed.inventory_suppressed, 1);
    assert!(suppressed.scan_complete);
    assert!(
        store
            .checkpoint
            .lock()
            .expect("capture journal checkpoint lock")
            .is_none(),
        "inventory suppression must not manufacture local delivery state"
    );
    assert_eq!(inventory.readiness_calls.load(Ordering::SeqCst), 6);
    assert_eq!(inventory.get_calls.load(Ordering::SeqCst), 3);
    assert_eq!(inventory.put_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inventory.inventory_calls.load(Ordering::SeqCst), 0);
    assert_eq!(ledger.requested_generations(), vec![1, 1, 1]);
}

#[test]
fn completed_musubi_capture_ledger_never_receives_claim_minting_capabilities() {
    let source = include_str!("../completed_musubi_capture.rs");
    let trait_start = source
        .find("pub trait ProviderIngestCompletedMusubiSignedCaptureLedgerV1")
        .expect("capture ledger trait");
    let trait_tail = &source[trait_start..];
    let trait_end = trait_tail
        .find("\n}\n")
        .expect("capture ledger trait terminator");
    let trait_source = &trait_tail[..trait_end];
    assert!(trait_source.contains("ProviderIngestCompletedMusubiCaptureRequestV1"));
    assert!(trait_source.contains("ProviderIngestCompletedMusubiSignedCapturePageV1"));
    assert!(!trait_source.contains("ProviderIngestFinalizedClaimFactoryV1"));
    assert!(!trait_source.contains("ProviderIngestFinalizedMusubiCompletionClaimV1"));
    assert!(!trait_source.contains("ProviderIngestFinalizedAssignmentPageV1"));
}

#[test]
fn completed_musubi_capture_scanner_enforces_identity_and_page_bounds() {
    let ledger = Arc::new(CaptureScannerLedgerV1::new(
        Vec::new(),
        8,
        CaptureScannerLedgerFaultV1::None,
    ));
    let unmarked_network_id = NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0; 32])),
    );
    for (provider_id, network_id, max_page_rows, expected) in [
        ([0; 32], test_network_id(), 1, "provider"),
        (LOCAL_PROVIDER, foreign_test_network_id(), 1, "binding"),
        (LOCAL_PROVIDER, unmarked_network_id, 1, "network"),
        (LOCAL_PROVIDER, test_network_id(), 0, "policy"),
        (
            LOCAL_PROVIDER,
            test_network_id(),
            PROVIDER_INGEST_STATUS_PAGE_MAX_V1 + 1,
            "policy",
        ),
    ] {
        let result = ProviderIngestCompletedMusubiCaptureScannerV1::new(
            CompletedMusubiStoreInstanceV1::new(),
            provider_id,
            network_id,
            max_page_rows,
            Arc::clone(&ledger),
        );
        match expected {
            "provider" => assert!(matches!(
                result,
                Err(ProviderIngestRuntimeErrorV1::InvalidProviderId)
            )),
            "binding" => assert!(matches!(
                result,
                Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
            )),
            "network" => assert!(matches!(
                result,
                Err(ProviderIngestRuntimeErrorV1::InvalidNetworkId)
            )),
            "policy" => assert!(matches!(
                result,
                Err(ProviderIngestRuntimeErrorV1::InvalidPolicy)
            )),
            _ => unreachable!("known expected capture-scanner error"),
        }
    }
}

struct TestFetch {
    result: Mutex<Result<Vec<u8>, ProviderIngestSourceFetchErrorV1>>,
    delay_ms: u64,
    calls: AtomicUsize,
}

impl ProviderIngestAuthenticatedSourceFetchV1 for TestFetch {
    type Fetched = Vec<u8>;

    fn fetch<'a>(
        &'a self,
        request: ProviderIngestSourceRequestV1,
    ) -> ProviderIngestFutureV1<'a, Result<Self::Fetched, ProviderIngestSourceFetchErrorV1>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(request.source_provider_ids(), [SOURCE_PROVIDER]);
        let result = self.result.lock().unwrap().clone();
        let delay_ms = self.delay_ms;
        Box::pin(async move {
            if delay_ms != 0 {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
            result
        })
    }
}

struct TestProviderSource {
    provider_id: [u8; 32],
    runtime_handle: &'static str,
    drifted_runtime_handle: &'static str,
    drift_after_fetch: bool,
    drifted: AtomicBool,
    qualification: Mutex<ProviderIngestSourceQualificationV1>,
    qualification_after_fetch: Mutex<Option<ProviderIngestSourceQualificationV1>>,
    readiness: Mutex<Result<(), ProviderIngestSourceFetchErrorV1>>,
    result: Mutex<Result<Vec<u8>, ProviderIngestSourceFetchErrorV1>>,
    calls: Arc<Mutex<Vec<[u8; 32]>>>,
    musubi_calls: Mutex<Vec<Option<ProviderIngestMusubiArchiveFetchBindingV1>>>,
}

impl ProviderIngestAuthenticatedProviderSourceV1 for TestProviderSource {
    type Fetched = Vec<u8>;

    fn provider_id(&self) -> [u8; 32] {
        self.provider_id
    }

    fn runtime_handle(&self) -> &str {
        if self.drifted.load(Ordering::SeqCst) {
            self.drifted_runtime_handle
        } else {
            self.runtime_handle
        }
    }

    fn qualification(
        &self,
    ) -> Result<ProviderIngestSourceQualificationV1, ProviderIngestSourceFetchErrorV1> {
        Ok(*self.qualification.lock().unwrap())
    }

    fn check_readiness(&self) -> Result<(), ProviderIngestSourceFetchErrorV1> {
        *self.readiness.lock().unwrap()
    }

    fn fetch_provider<'a>(
        &'a self,
        authorization: FinalizedProviderIngestAuthorizationV1,
        musubi_archive: Option<ProviderIngestMusubiArchiveFetchBindingV1>,
    ) -> ProviderIngestFutureV1<'a, Result<Self::Fetched, ProviderIngestSourceFetchErrorV1>> {
        assert_eq!(authorization.provider_id(), LOCAL_PROVIDER);
        self.calls.lock().unwrap().push(self.provider_id);
        self.musubi_calls.lock().unwrap().push(musubi_archive);
        let result = self.result.lock().unwrap().clone();
        let qualification_after_fetch = self.qualification_after_fetch.lock().unwrap().take();
        if let Some(qualification) = qualification_after_fetch {
            *self.qualification.lock().unwrap() = qualification;
        }
        if self.drift_after_fetch {
            self.drifted.store(true, Ordering::SeqCst);
        }
        Box::pin(async move { result })
    }
}

fn test_provider_source(
    provider_id: [u8; 32],
    runtime_handle: &'static str,
    readiness: Result<(), ProviderIngestSourceFetchErrorV1>,
    result: Result<Vec<u8>, ProviderIngestSourceFetchErrorV1>,
    drift_after_fetch: bool,
    calls: Arc<Mutex<Vec<[u8; 32]>>>,
) -> Arc<TestProviderSource> {
    Arc::new(TestProviderSource {
        provider_id,
        runtime_handle,
        drifted_runtime_handle: "https-pinned:provider-substituted",
        drift_after_fetch,
        drifted: AtomicBool::new(false),
        qualification: Mutex::new(ProviderIngestSourceQualificationV1::new(1, provider_id)),
        qualification_after_fetch: Mutex::new(None),
        readiness: Mutex::new(readiness),
        result: Mutex::new(result),
        calls,
        musubi_calls: Mutex::new(Vec::new()),
    })
}

fn test_source_binding(
    provider_id: [u8; 32],
    runtime_handle: impl Into<String>,
) -> ProviderIngestAuthenticatedSourceBindingV1 {
    ProviderIngestAuthenticatedSourceBindingV1 {
        provider_id,
        runtime_handle: runtime_handle.into(),
        revision: 1,
        policy_digest: provider_id,
    }
}

fn test_source_registration(
    source: Arc<TestProviderSource>,
    binding: ProviderIngestAuthenticatedSourceBindingV1,
) -> ProviderIngestAuthenticatedSourceRegistrationV1<Vec<u8>> {
    let source: Arc<dyn ProviderIngestAuthenticatedProviderSourceV1<Fetched = Vec<u8>>> = source;
    ProviderIngestAuthenticatedSourceRegistrationV1::new(binding, source)
}

fn test_source_pool(
    sources: Vec<Arc<TestProviderSource>>,
) -> Result<
    ProviderIngestAuthenticatedSourcePoolV1<Vec<u8>>,
    ProviderIngestAuthenticatedSourcePoolErrorV1,
> {
    let sources = sources
        .into_iter()
        .map(|source| {
            let binding = test_source_binding(source.provider_id, source.runtime_handle);
            test_source_registration(source, binding)
        })
        .collect();
    ProviderIngestAuthenticatedSourcePoolV1::new(
        "https-pinned-source-pool:region-a",
        ProviderIngestRuntimeProviderQualificationV1::new(9, [0xA9; 32]),
        4,
        sources,
    )
}

fn test_source_request_result(
    source_provider_ids: Vec<[u8; 32]>,
) -> Result<ProviderIngestSourceRequestV1, ProviderIngestSourceFetchErrorV1> {
    let row = fixture_row(0x31);
    let validated = validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy()).unwrap();
    ProviderIngestSourceRequestV1::new(validated.authorization, source_provider_ids, None)
}

fn test_source_request(source_provider_ids: Vec<[u8; 32]>) -> ProviderIngestSourceRequestV1 {
    test_source_request_result(source_provider_ids).expect("valid test source request")
}

#[test]
fn authenticated_source_qualification_rejects_unsupported_or_zero_pins() {
    let valid = ProviderIngestSourceQualificationV1::new(1, [0x22; 32]);
    assert_eq!(valid.validate(), Ok(()));

    let mut unsupported = valid;
    unsupported.version = 2;
    for invalid in [
        unsupported,
        ProviderIngestSourceQualificationV1::new(0, [0x22; 32]),
        ProviderIngestSourceQualificationV1::new(1, [0; 32]),
    ] {
        assert_eq!(
            invalid.validate(),
            Err(ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceQualification)
        );
    }
}

#[test]
fn runtime_provider_qualification_requires_both_public_pins() {
    assert!(ProviderIngestRuntimeProviderQualificationV1::new(9, [0xA9; 32]).is_valid());
    assert!(!ProviderIngestRuntimeProviderQualificationV1::new(0, [0xA9; 32]).is_valid());
    assert!(!ProviderIngestRuntimeProviderQualificationV1::new(9, [0; 32]).is_valid());
}

#[test]
fn authenticated_source_pool_rejects_incomplete_duplicate_and_test_marked_inventory() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source_a = test_provider_source(
        [0x22; 32],
        "https-pinned:provider-a",
        Ok(()),
        Err(ProviderIngestSourceFetchErrorV1::Unavailable),
        false,
        Arc::clone(&calls),
    );
    assert_eq!(
        test_source_pool(vec![Arc::clone(&source_a)]).unwrap_err(),
        ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceCount
    );

    let duplicate_provider = test_provider_source(
        [0x22; 32],
        "https-pinned:provider-b",
        Ok(()),
        Err(ProviderIngestSourceFetchErrorV1::Unavailable),
        false,
        Arc::clone(&calls),
    );
    assert_eq!(
        test_source_pool(vec![Arc::clone(&source_a), duplicate_provider]).unwrap_err(),
        ProviderIngestAuthenticatedSourcePoolErrorV1::DuplicateProvider
    );

    let duplicate_handle = test_provider_source(
        [0x33; 32],
        "https-pinned:provider-a",
        Ok(()),
        Err(ProviderIngestSourceFetchErrorV1::Unavailable),
        false,
        Arc::clone(&calls),
    );
    assert_eq!(
        test_source_pool(vec![Arc::clone(&source_a), duplicate_handle]).unwrap_err(),
        ProviderIngestAuthenticatedSourcePoolErrorV1::DuplicateSourceHandle
    );

    let credential_handle = test_provider_source(
        [0x33; 32],
        "https://operator:secret@provider.example",
        Ok(()),
        Err(ProviderIngestSourceFetchErrorV1::Unavailable),
        false,
        Arc::clone(&calls),
    );
    assert_eq!(
        test_source_pool(vec![Arc::clone(&source_a), credential_handle]).unwrap_err(),
        ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceHandle
    );

    let test_marked = test_provider_source(
        [0x33; 32],
        "https-pinned:provider-test",
        Ok(()),
        Err(ProviderIngestSourceFetchErrorV1::Unavailable),
        false,
        calls,
    );
    assert_eq!(
        test_source_pool(vec![source_a, test_marked]).unwrap_err(),
        ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceHandle
    );
}

#[test]
fn authenticated_source_pool_requires_independent_valid_qualification_pins() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source_a = test_provider_source(
        [0x22; 32],
        "https-pinned:provider-a",
        Ok(()),
        Err(ProviderIngestSourceFetchErrorV1::Unavailable),
        false,
        Arc::clone(&calls),
    );
    let source_b = test_provider_source(
        [0x33; 32],
        "https-pinned:provider-b",
        Ok(()),
        Err(ProviderIngestSourceFetchErrorV1::Unavailable),
        false,
        calls,
    );

    for invalid_pool_qualification in [
        ProviderIngestRuntimeProviderQualificationV1::new(0, [0xA9; 32]),
        ProviderIngestRuntimeProviderQualificationV1::new(9, [0; 32]),
    ] {
        assert_eq!(
            ProviderIngestAuthenticatedSourcePoolV1::new(
                "https-pinned-source-pool:region-a",
                invalid_pool_qualification,
                4,
                vec![
                    test_source_registration(
                        Arc::clone(&source_a),
                        test_source_binding(source_a.provider_id, source_a.runtime_handle),
                    ),
                    test_source_registration(
                        Arc::clone(&source_b),
                        test_source_binding(source_b.provider_id, source_b.runtime_handle),
                    ),
                ],
            )
            .unwrap_err(),
            ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidPoolQualification
        );
    }

    let mut invalid_binding = test_source_binding(source_b.provider_id, source_b.runtime_handle);
    invalid_binding.revision = 0;
    assert_eq!(
        ProviderIngestAuthenticatedSourcePoolV1::new(
            "https-pinned-source-pool:region-a",
            ProviderIngestRuntimeProviderQualificationV1::new(9, [0xA9; 32]),
            4,
            vec![
                test_source_registration(
                    Arc::clone(&source_a),
                    test_source_binding(source_a.provider_id, source_a.runtime_handle),
                ),
                test_source_registration(Arc::clone(&source_b), invalid_binding),
            ],
        )
        .unwrap_err(),
        ProviderIngestAuthenticatedSourcePoolErrorV1::InvalidSourceQualification
    );

    let mut substituted_binding =
        test_source_binding(source_b.provider_id, source_b.runtime_handle);
    substituted_binding.revision = 2;
    assert_eq!(
        ProviderIngestAuthenticatedSourcePoolV1::new(
            "https-pinned-source-pool:region-a",
            ProviderIngestRuntimeProviderQualificationV1::new(9, [0xA9; 32]),
            4,
            vec![
                test_source_registration(
                    Arc::clone(&source_a),
                    test_source_binding(source_a.provider_id, source_a.runtime_handle),
                ),
                test_source_registration(source_b, substituted_binding),
            ],
        )
        .unwrap_err(),
        ProviderIngestAuthenticatedSourcePoolErrorV1::SourceBindingMismatch
    );
}

#[test]
fn authenticated_source_pool_rejects_qualification_drift_at_readiness() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source_a = test_provider_source(
        [0x22; 32],
        "https-pinned:provider-a",
        Ok(()),
        Err(ProviderIngestSourceFetchErrorV1::Unavailable),
        false,
        Arc::clone(&calls),
    );
    let source_b = test_provider_source(
        [0x33; 32],
        "https-pinned:provider-b",
        Ok(()),
        Err(ProviderIngestSourceFetchErrorV1::Unavailable),
        false,
        Arc::clone(&calls),
    );
    let pool = test_source_pool(vec![Arc::clone(&source_a), source_b]).unwrap();
    *source_a.qualification.lock().unwrap() =
        ProviderIngestSourceQualificationV1::new(2, [0x22; 32]);

    assert_eq!(
        pool.check_readiness(),
        Err(ProviderIngestSourceFetchErrorV1::Rejected)
    );
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn authenticated_source_pool_is_ready_when_one_qualified_source_is_available() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source_a = test_provider_source(
        [0x22; 32],
        "https-pinned:provider-a",
        Err(ProviderIngestSourceFetchErrorV1::Unavailable),
        Err(ProviderIngestSourceFetchErrorV1::Unavailable),
        false,
        Arc::clone(&calls),
    );
    let source_b = test_provider_source(
        [0x33; 32],
        "https-pinned:provider-b",
        Ok(()),
        Err(ProviderIngestSourceFetchErrorV1::Unavailable),
        false,
        calls,
    );
    let pool = test_source_pool(vec![source_a, source_b]).unwrap();

    assert_eq!(pool.check_readiness(), Ok(()));
}

#[tokio::test]
async fn authenticated_source_pool_fails_over_in_canonical_provider_order() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source_a = test_provider_source(
        [0x22; 32],
        "https-pinned:provider-a",
        Ok(()),
        Err(ProviderIngestSourceFetchErrorV1::Unavailable),
        false,
        Arc::clone(&calls),
    );
    let source_b = test_provider_source(
        [0x33; 32],
        "https-pinned:provider-b",
        Ok(()),
        Ok(vec![0xA5]),
        false,
        Arc::clone(&calls),
    );
    let pool = test_source_pool(vec![source_a, source_b]).unwrap();

    assert_eq!(pool.runtime_handle(), "https-pinned-source-pool:region-a");
    assert_eq!(pool.source_provider_ids(), &[[0x22; 32], [0x33; 32]]);
    assert_eq!(pool.max_sources_per_fetch(), 4);
    assert!(pool.check_readiness().is_ok());
    assert_eq!(
        pool.fetch(test_source_request(vec![[0x22; 32], [0x33; 32]]))
            .await,
        Ok(vec![0xA5])
    );
    assert_eq!(*calls.lock().unwrap(), vec![[0x22; 32], [0x33; 32]]);
}

#[tokio::test]
async fn authenticated_source_pool_preserves_exact_musubi_fetch_binding() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source = test_provider_source(
        SOURCE_PROVIDER,
        "https-pinned:provider-musubi",
        Ok(()),
        Ok(vec![0xA5]),
        false,
        Arc::clone(&calls),
    );
    let fallback = test_provider_source(
        [0x33; 32],
        "https-pinned:provider-musubi-fallback",
        Ok(()),
        Err(ProviderIngestSourceFetchErrorV1::Unavailable),
        false,
        Arc::clone(&calls),
    );
    let pool = test_source_pool(vec![Arc::clone(&source), fallback]).unwrap();
    let row = fixture_musubi_row(0x68, 0xB4);
    let validated = validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy()).unwrap();
    let musubi_archive = ProviderIngestMusubiArchiveFetchBindingV1::from_finalized_claim(
        row.musubi_archive.as_ref().unwrap(),
    );
    let request = ProviderIngestSourceRequestV1::new(
        validated.authorization,
        vec![SOURCE_PROVIDER],
        Some(musubi_archive.clone()),
    )
    .unwrap();

    assert_eq!(pool.fetch(request).await, Ok(vec![0xA5]));
    assert_eq!(*calls.lock().unwrap(), vec![SOURCE_PROVIDER]);
    assert_eq!(
        *source.musubi_calls.lock().unwrap(),
        vec![Some(musubi_archive)]
    );
}

#[tokio::test]
async fn authenticated_source_pool_fails_over_after_content_rejection() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source_a = test_provider_source(
        [0x22; 32],
        "https-pinned:provider-a",
        Ok(()),
        Err(ProviderIngestSourceFetchErrorV1::ContentRejected),
        false,
        Arc::clone(&calls),
    );
    let source_b = test_provider_source(
        [0x33; 32],
        "https-pinned:provider-b",
        Ok(()),
        Ok(vec![0xA5]),
        false,
        Arc::clone(&calls),
    );
    let pool = test_source_pool(vec![source_a, source_b]).unwrap();

    assert_eq!(
        pool.fetch(test_source_request(vec![[0x22; 32], [0x33; 32]]))
            .await,
        Ok(vec![0xA5])
    );
    assert_eq!(*calls.lock().unwrap(), vec![[0x22; 32], [0x33; 32]]);
}

#[tokio::test]
async fn authenticated_source_pool_rejects_noncanonical_or_unpinned_requests_before_io() {
    for source_provider_ids in [
        vec![[0x33; 32], [0x22; 32]],
        vec![[0x22; 32], [0x44; 32]],
        vec![LOCAL_PROVIDER, [0x22; 32]],
    ] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let source_a = test_provider_source(
            [0x22; 32],
            "https-pinned:provider-a",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        let source_b = test_provider_source(
            [0x33; 32],
            "https-pinned:provider-b",
            Ok(()),
            Err(ProviderIngestSourceFetchErrorV1::Unavailable),
            false,
            Arc::clone(&calls),
        );
        let pool = test_source_pool(vec![source_a, source_b]).unwrap();

        match test_source_request_result(source_provider_ids) {
            Ok(request) => assert_eq!(
                pool.fetch(request).await,
                Err(ProviderIngestSourceFetchErrorV1::Rejected)
            ),
            Err(error) => assert_eq!(error, ProviderIngestSourceFetchErrorV1::Rejected),
        }
        assert!(calls.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn authenticated_source_pool_does_not_mask_identity_drift_with_later_success() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source_a = test_provider_source(
        [0x22; 32],
        "https-pinned:provider-a",
        Ok(()),
        Ok(vec![0xA5]),
        true,
        Arc::clone(&calls),
    );
    let source_b = test_provider_source(
        [0x33; 32],
        "https-pinned:provider-b",
        Ok(()),
        Ok(vec![0xB6]),
        false,
        Arc::clone(&calls),
    );
    let pool = test_source_pool(vec![source_a, source_b]).unwrap();

    assert_eq!(
        pool.fetch(test_source_request(vec![[0x22; 32], [0x33; 32]]))
            .await,
        Err(ProviderIngestSourceFetchErrorV1::Rejected)
    );
    assert_eq!(*calls.lock().unwrap(), vec![[0x22; 32]]);
}

#[tokio::test]
async fn authenticated_source_pool_does_not_mask_qualification_drift_with_later_success() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source_a = test_provider_source(
        [0x22; 32],
        "https-pinned:provider-a",
        Ok(()),
        Ok(vec![0xA5]),
        false,
        Arc::clone(&calls),
    );
    *source_a.qualification_after_fetch.lock().unwrap() =
        Some(ProviderIngestSourceQualificationV1::new(2, [0x22; 32]));
    let source_b = test_provider_source(
        [0x33; 32],
        "https-pinned:provider-b",
        Ok(()),
        Ok(vec![0xB6]),
        false,
        Arc::clone(&calls),
    );
    let pool = test_source_pool(vec![source_a, source_b]).unwrap();

    assert_eq!(
        pool.fetch(test_source_request(vec![[0x22; 32], [0x33; 32]]))
            .await,
        Err(ProviderIngestSourceFetchErrorV1::Rejected)
    );
    assert_eq!(*calls.lock().unwrap(), vec![[0x22; 32]]);
}

#[test]
fn completed_musubi_effect_pump_orders_handoff_prepare_approval_and_commit() {
    let source = include_str!("../completed_musubi_capture.rs");
    let driver = source
        .find("pub async fn drive_one_bounded_page")
        .expect("bounded effect-pump entrypoint");
    let source = &source[driver..];
    let handoff = source
        .find("ready_handoff_page(None")
        .expect("handoff page drains first");
    let prepare = source
        .find("prepare_provider_ingest_completed_musubi_capture_page")
        .expect("capture prepare transaction");
    let approve = source
        .find("approve_claim_with_signer")
        .expect("durable approval effect");
    let commit = source.find("prepared.commit()").expect("progress commit");
    assert!(handoff < prepare && prepare < approve && approve < commit);
    assert!(source.contains("ProviderIngestCompletedMusubiAttestationDriveErrorV1::ApprovalBlocked"));
}

#[test]
fn completed_musubi_effect_error_classes_are_fail_closed() {
    assert!(
        ProviderIngestCompletedMusubiAttestationDriveErrorV1::CaptureUnavailable.is_retryable()
    );
    assert!(ProviderIngestCompletedMusubiAttestationDriveErrorV1::EffectUnavailable.is_retryable());
    assert!(!ProviderIngestCompletedMusubiAttestationDriveErrorV1::CapacityBlocked.is_retryable());
    assert!(!ProviderIngestCompletedMusubiAttestationDriveErrorV1::ApprovalBlocked.is_retryable());
    assert_eq!(
        map_completed_musubi_driver_journal_error(
            MusubiProviderAttestationJournalErrorV1::ClockRollback,
        ),
        ProviderIngestCompletedMusubiAttestationDriveErrorV1::IntegrityRejected,
    );
    assert_eq!(
        map_completed_musubi_driver_journal_error(
            MusubiProviderAttestationJournalErrorV1::StoreUnavailable,
        ),
        ProviderIngestCompletedMusubiAttestationDriveErrorV1::EffectUnavailable,
    );
}
