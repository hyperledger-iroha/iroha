struct TestStorage {
    existing: AtomicBool,
}
impl ProviderIngestLocalStorageV1<Vec<u8>> for TestStorage {
    fn verify_existing<'a>(
        &'a self,
        authorization: FinalizedProviderIngestAuthorizationV1,
        musubi_archive: Option<ProviderIngestFinalizedMusubiArchiveClaimV1>,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<Option<ProviderIngestLocalStoredV1>, ProviderIngestLocalStorageErrorV1>,
    > {
        let existing = self.existing.load(Ordering::SeqCst);
        Box::pin(async move {
            if musubi_archive.is_some() {
                return Err(ProviderIngestLocalStorageErrorV1::Permanent);
            }
            Ok(existing.then(|| {
                ProviderIngestLocalStoredV1::generic(hex::encode(authorization.manifest_digest()))
            }))
        })
    }
    fn store<'a>(
        &'a self,
        authorization: FinalizedProviderIngestAuthorizationV1,
        musubi_archive: Option<ProviderIngestFinalizedMusubiArchiveClaimV1>,
        fetched: Vec<u8>,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<ProviderIngestLocalStoredV1, ProviderIngestLocalStorageErrorV1>,
    > {
        Box::pin(async move {
            if fetched != vec![0xA5] || musubi_archive.is_some() {
                return Err(ProviderIngestLocalStorageErrorV1::Retryable);
            }
            if authorization.order_id() == [0x3E; 32] {
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
            if authorization.order_id() == [0x45; 32] {
                return Err(ProviderIngestLocalStorageErrorV1::Quarantined);
            }
            Ok(ProviderIngestLocalStoredV1::generic(hex::encode(
                authorization.manifest_digest(),
            )))
        })
    }
}
struct TestPayloadBuilder {
    network_id: NetworkId,
}
impl ProviderIngestCompletionPayloadBuilderV1 for TestPayloadBuilder {
    fn build_payload<'a>(
        &'a self,
        request: ProviderIngestCompletionPayloadRequestV1,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<TransactionPayload, ProviderIngestCompletionPayloadErrorV1>,
    > {
        let network_id = self.network_id;
        Box::pin(async move {
            if request.network_id != network_id || request.authorization.order_id() == [0x3B; 32] {
                return Err(ProviderIngestCompletionPayloadErrorV1::Rejected);
            }
            let mut builder = TransactionBuilder::new(
                network_id,
                request.provider_owner,
                FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([InstructionBox::from(CompleteReplicationOrder {
                order_id: ReplicationOrderId::new(request.authorization.order_id()),
                provider_id: ProviderId::new(request.authorization.provider_id()),
                completion_epoch: request.completion_epoch,
                expected_authority: request.expected_authority,
                expected_assignment_revision: request.expected_assignment_revision,
                finalized_anchor: ProviderIngestFinalizedAnchorV1 {
                    height: request.finalized_cursor.height,
                    block_hash: request.finalized_cursor.block_hash,
                },
            })]);
            builder.set_creation_time(Duration::from_millis(1_000));
            builder.set_ttl(Duration::from_secs(30));
            builder
                .into_payload()
                .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)
        })
    }
}
struct TestSigner {
    key: KeyPair,
    authority: AccountId,
    signer_policy_revision: Arc<AtomicU64>,
    eligibility_flip_on_call: usize,
    eligibility_flip_to_revision: u64,
    eligibility_calls: AtomicUsize,
}
impl ProviderIngestCompletionSignerV1 for TestSigner {
    fn runtime_handle(&self) -> &str {
        "pkcs11:sorafs-provider-ingest-unit"
    }
    fn authority(&self) -> &AccountId {
        &self.authority
    }
    fn qualification(
        &self,
    ) -> Result<ProviderIngestCompletionSignerQualificationV1, ProviderIngestCompletionSignerErrorV1>
    {
        Ok(ProviderIngestCompletionSignerQualificationV1::new(
            1,
            self.signer_policy(),
            self.key.public_key().algorithm(),
            self.key.public_key().clone(),
        ))
    }
    fn signer_policy(&self) -> ProviderIngestCompletionSignerPolicyV1 {
        completion_signer_policy(self.signer_policy_revision.load(Ordering::SeqCst))
    }
    fn current_eligibility(
        &self,
    ) -> Result<ProviderIngestCompletionSignerPolicyV1, ProviderIngestCompletionSignerErrorV1> {
        let call = self
            .eligibility_calls
            .fetch_add(1, Ordering::SeqCst)
            .saturating_add(1);
        if self.eligibility_flip_on_call != 0 && call == self.eligibility_flip_on_call {
            self.signer_policy_revision
                .store(self.eligibility_flip_to_revision, Ordering::SeqCst);
        }
        let signer_policy = self.signer_policy();
        if signer_policy.is_valid() {
            Ok(signer_policy)
        } else {
            Err(ProviderIngestCompletionSignerErrorV1::Rejected)
        }
    }
    fn sign<'a>(
        &'a self,
        payload: TransactionPayload,
    ) -> ProviderIngestFutureV1<'a, Result<SignedTransaction, ProviderIngestCompletionSignerErrorV1>>
    {
        Box::pin(async move {
            TransactionBuilder::from_payload(payload)
                .and_then(|builder| builder.try_sign(self.key.private_key()))
                .map_err(|_| ProviderIngestCompletionSignerErrorV1::Rejected)
        })
    }
}
struct TestResolver {
    wrong_authority: AtomicBool,
    signer_policy_revision: Arc<AtomicU64>,
    eligibility_flip_on_call: AtomicUsize,
    eligibility_flip_to_revision: AtomicU64,
}
impl ProviderIngestCompletionSignerResolverV1 for TestResolver {
    type Signer = TestSigner;
    fn resolve<'a>(
        &'a self,
        _context: ProviderIngestCompletionSignerResolutionContextV1,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<Option<Self::Signer>, ProviderIngestCompletionSignerResolverErrorV1>,
    > {
        let seed = if self.wrong_authority.load(Ordering::SeqCst) {
            9
        } else {
            8
        };
        let signer_policy_revision = Arc::clone(&self.signer_policy_revision);
        let eligibility_flip_on_call = self.eligibility_flip_on_call.load(Ordering::SeqCst);
        let eligibility_flip_to_revision = self.eligibility_flip_to_revision.load(Ordering::SeqCst);
        Box::pin(async move {
            let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("key");
            let authority = AccountId::new(key.public_key().clone());
            Ok(Some(TestSigner {
                key,
                authority,
                signer_policy_revision,
                eligibility_flip_on_call,
                eligibility_flip_to_revision,
                eligibility_calls: AtomicUsize::new(0),
            }))
        })
    }
}
struct TestIngress {
    outbox: ProviderIngestOutbox,
    job_id: [u8; 32],
    prepare_error: Mutex<Option<ProviderIngestIngressPrepareErrorV1>>,
    disposition: Mutex<ProviderIngestIngressDispositionV1>,
    observation: Mutex<ProviderIngestTransactionObservationV1>,
    observe_calls: AtomicUsize,
    events: Mutex<Vec<&'static str>>,
}
impl ProviderIngestTransactionIngressV1 for TestIngress {
    type Prepared = SignedTransaction;
    fn prepare<'a>(
        &'a self,
        transaction: SignedTransaction,
    ) -> ProviderIngestFutureV1<'a, Result<Self::Prepared, ProviderIngestIngressPrepareErrorV1>>
    {
        let state = self.outbox.status(self.job_id).unwrap().state;
        assert!(matches!(
            state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Signed { .. },
                ..
            }
        ));
        self.events.lock().unwrap().push("prepare_signed");
        let error = *self.prepare_error.lock().unwrap();
        Box::pin(async move {
            if let Some(error) = error {
                Err(error)
            } else {
                Ok(transaction)
            }
        })
    }
    fn expose<'a>(
        &'a self,
        prepared: Self::Prepared,
        transaction: SignedTransaction,
    ) -> ProviderIngestFutureV1<'a, ProviderIngestIngressDispositionV1> {
        assert_eq!(prepared, transaction);
        let state = self.outbox.status(self.job_id).unwrap().state;
        assert!(matches!(
            state,
            ProviderIngestDeliveryStateV1::LocalStored {
                completion: ProviderIngestCompletionStateV1::Ambiguous { .. },
                ..
            }
        ));
        self.events.lock().unwrap().push("expose_ambiguous");
        let disposition = *self.disposition.lock().unwrap();
        Box::pin(async move { disposition })
    }
    fn observe<'a>(
        &'a self,
        _transaction_hash: [u8; 32],
    ) -> ProviderIngestFutureV1<'a, ProviderIngestTransactionObservationV1> {
        self.observe_calls.fetch_add(1, Ordering::SeqCst);
        let observation = *self.observation.lock().unwrap();
        Box::pin(async move { observation })
    }
}
struct TestClock {
    start: Instant,
    base_ms: AtomicU64,
}
impl ProviderIngestClockV1 for TestClock {
    fn now_ms(&self) -> u64 {
        self.base_ms
            .load(Ordering::SeqCst)
            .saturating_add(u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX))
    }
}
type TestRuntime = ProviderIngestRuntimeV1<
    TestLedger,
    TestFetch,
    TestStorage,
    TestPayloadBuilder,
    TestResolver,
    TestIngress,
    TestClock,
>;
type TestRuntimeParts = (
    TestRuntime,
    Arc<TestLedger>,
    Arc<TestFetch>,
    Arc<TestIngress>,
);
fn test_runtime_with_network_id(
    row: ProviderIngestFinalizedAssignmentV1,
    existing: bool,
    fetch_result: Result<Vec<u8>, ProviderIngestSourceFetchErrorV1>,
    fetch_delay_ms: u64,
    disposition: ProviderIngestIngressDispositionV1,
    wrong_signer: bool,
    network_id: NetworkId,
) -> Result<TestRuntimeParts, ProviderIngestRuntimeErrorV1> {
    let page = fixture_page(row.clone());
    let finalized_cursor = page.finalized_cursor;
    let ledger = Arc::new(TestLedger {
        page: Mutex::new(page),
    });
    let fetch = Arc::new(TestFetch {
        result: Mutex::new(fetch_result),
        delay_ms: fetch_delay_ms,
        calls: AtomicUsize::new(0),
    });
    let storage = Arc::new(TestStorage {
        existing: AtomicBool::new(existing),
    });
    let outbox = ProviderIngestOutbox::in_memory(outbox_policy()).expect("outbox");
    let validated =
        validate_assignment(&row, finalized_cursor, LOCAL_PROVIDER, runtime_policy()).unwrap();
    let ingress = Arc::new(TestIngress {
        outbox: outbox.clone(),
        job_id: validated.authorization.job_id(),
        prepare_error: Mutex::new(None),
        disposition: Mutex::new(disposition),
        observation: Mutex::new(ProviderIngestTransactionObservationV1::Unavailable),
        observe_calls: AtomicUsize::new(0),
        events: Mutex::new(Vec::new()),
    });
    let payload_builder = Arc::new(TestPayloadBuilder { network_id });
    let runtime = ProviderIngestRuntimeV1::new(
        LOCAL_PROVIDER,
        network_id,
        ProviderIngestClaimOwnerV1::new([0xCC; 32]).unwrap(),
        runtime_policy(),
        outbox,
        ledger.clone(),
        fetch.clone(),
        storage,
        payload_builder,
        Arc::new(TestResolver {
            wrong_authority: AtomicBool::new(wrong_signer),
            signer_policy_revision: Arc::new(AtomicU64::new(1)),
            eligibility_flip_on_call: AtomicUsize::new(0),
            eligibility_flip_to_revision: AtomicU64::new(0),
        }),
        ingress.clone(),
        Arc::new(TestClock {
            start: Instant::now(),
            base_ms: AtomicU64::new(1_000),
        }),
    )?;
    Ok((runtime, ledger, fetch, ingress))
}
fn test_runtime(
    row: ProviderIngestFinalizedAssignmentV1,
    existing: bool,
    fetch_result: Result<Vec<u8>, ProviderIngestSourceFetchErrorV1>,
    fetch_delay_ms: u64,
    disposition: ProviderIngestIngressDispositionV1,
    wrong_signer: bool,
) -> (
    TestRuntime,
    Arc<TestLedger>,
    Arc<TestFetch>,
    Arc<TestIngress>,
) {
    test_runtime_with_network_id(
        row,
        existing,
        fetch_result,
        fetch_delay_ms,
        disposition,
        wrong_signer,
        test_network_id(),
    )
    .expect("runtime")
}
#[test]
fn runtime_requires_marked_network_identity_and_preserves_it_exactly() {
    let row = fixture_row(0x30);
    assert!(matches!(
        test_runtime_with_network_id(
            row.clone(),
            true,
            Ok(vec![0xA5]),
            0,
            ProviderIngestIngressDispositionV1::Submitted,
            false,
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
                Hash::prehashed([0; 32]),
            )),
        ),
        Err(ProviderIngestRuntimeErrorV1::InvalidNetworkId)
    ));
    let (runtime, _, _, _) = test_runtime(
        row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    assert_eq!(runtime.network_id, test_network_id());
}
#[test]
fn finalized_page_rejects_cursor_order_and_pagination_substitution() {
    let row = fixture_row(0x31);
    let page = fixture_page(row.clone());
    validate_page(&page, None, cursor(8), 16).expect("valid page");
    let mut wrong_cursor = page.clone();
    wrong_cursor.finalized_cursor = cursor(9);
    assert!(matches!(
        validate_page(&wrong_cursor, None, cursor(8), 16),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
    ));
    let mut duplicate = page.clone();
    duplicate.rows.push(row);
    assert!(matches!(
        validate_page(&duplicate, None, cursor(8), 16),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
    ));
    let mut forged_next = page;
    forged_next.next_after_order_id = Some([0xFF; 32]);
    assert!(matches!(
        validate_page(&forged_next, None, cursor(8), 16),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
    ));
}
#[test]
fn finalized_cursor_and_order_lifecycle_fail_closed_on_substitution() {
    assert!(validate_monotonic_finalized_cursor(None, cursor(8)).is_ok());
    assert!(validate_monotonic_finalized_cursor(Some(cursor(8)), cursor(8)).is_ok());
    assert!(matches!(
        validate_monotonic_finalized_cursor(Some(cursor(8)), cursor(7)),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
    ));
    let fork = ProviderIngestFinalizedCursorV1 {
        height: 8,
        block_hash: [0xFE; 32],
    };
    assert!(matches!(
        validate_monotonic_finalized_cursor(Some(cursor(8)), fork),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
    ));
    let mut unassigned_completion = fixture_row(0x30);
    unassigned_completion
        .order
        .provider_completions
        .push(completion_record(
            ProviderId::new([0x99; 32]),
            account(9),
            8,
        ));
    assert!(matches!(
        validate_assignment(
            &unassigned_completion,
            cursor(8),
            LOCAL_PROVIDER,
            runtime_policy(),
        ),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
    ));
    let mut inconsistent_status = fixture_row(0x31);
    inconsistent_status.order.status = ReplicationOrderStatus::Completed(8);
    assert!(matches!(
        validate_assignment(
            &inconsistent_status,
            cursor(8),
            LOCAL_PROVIDER,
            runtime_policy(),
        ),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
    ));
}
#[test]
fn finalized_claim_factory_and_runtime_reject_musubi_binding_substitution() {
    let factory = ProviderIngestFinalizedClaimFactoryV1::new(test_network_id(), LOCAL_PROVIDER);
    let mut row = fixture_row(0x32);
    let binding = musubi_binding_for_row(&row, 0x81);
    let mut substituted_pin = row.pin.manifest.clone();
    substituted_pin.content_length = substituted_pin.content_length.saturating_add(1);
    assert_eq!(
        factory.seal_musubi_archive(
            &test_network_id(),
            cursor(8),
            *row.order.order_id.as_bytes(),
            &substituted_pin,
            binding.clone(),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected),
        "a reader cannot seal a publisher-substituted pin commitment"
    );
    let claim = factory
        .seal_musubi_archive(
            &test_network_id(),
            cursor(8),
            *row.order.order_id.as_bytes(),
            &row.pin.manifest,
            binding.clone(),
        )
        .expect("seal finalized Musubi binding");
    assert_eq!(claim.replication_order(), *row.order.order_id.as_bytes());
    assert_eq!(claim.archive_id(), binding.archive_id);
    row.musubi_archive = Some(claim);
    assert!(matches!(
        validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy()),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
    ));
    row.order.musubi_archive = Some(binding.archive_id);
    assert!(
        validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy()).is_ok(),
        "an exact finalized claim must remain usable"
    );
    let validated = validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy()).unwrap();
    let receipt = test_verified_musubi_receipt(
        row.musubi_archive.as_ref().unwrap(),
        &validated.authorization,
    );
    assert!(receipt.validate_stored(&validated.authorization));
    assert!(
        norito::to_bytes(&receipt.to_stored()).unwrap().len()
            <= PROVIDER_INGEST_VERIFIED_MUSUBI_RECEIPT_MAX_CANONICAL_BYTES_V1
    );
    let mut missing_claim = row.clone();
    missing_claim.musubi_archive = None;
    assert!(matches!(
        validate_assignment(&missing_claim, cursor(8), LOCAL_PROVIDER, runtime_policy(),),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
    ));
    let other_order = fixture_row(0x33);
    assert_eq!(
        factory.seal_musubi_archive(
            &test_network_id(),
            cursor(8),
            *other_order.order.order_id.as_bytes(),
            &other_order.pin.manifest,
            binding,
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected),
        "a reader cannot seal another order's publisher-shaped binding"
    );
    row.pin.manifest.content_length = row.pin.manifest.content_length.saturating_add(1);
    assert!(matches!(
        validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy()),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
    ));
}
#[test]
fn completed_musubi_claim_exists_only_for_the_local_finalized_completion() {
    let factory = ProviderIngestFinalizedClaimFactoryV1::new(test_network_id(), LOCAL_PROVIDER);
    let mut pending = fixture_musubi_row(0x34, 0x82);
    let binding = musubi_binding_for_row(&pending, 0x82);
    assert_eq!(
        factory.seal_completed_musubi_archive(
            &test_network_id(),
            cursor(8),
            ProviderId::new(LOCAL_PROVIDER),
            &pending.order,
            &pending.pin.manifest,
            binding.clone(),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected),
        "a pending local provider cannot receive a completed-row capability"
    );
    assert!(validate_assignment(&pending, cursor(8), LOCAL_PROVIDER, runtime_policy()).is_ok());
    let mut completed_other = pending.clone();
    completed_other
        .order
        .provider_completions
        .push(completion_record(
            ProviderId::new(SOURCE_PROVIDER),
            account(9),
            8,
        ));
    assert!(
        validate_assignment(
            &completed_other,
            cursor(8),
            LOCAL_PROVIDER,
            runtime_policy(),
        )
        .is_ok(),
        "another provider's completion must not require a local completed claim"
    );
    pending.order.provider_completions.push(completion_record(
        ProviderId::new(LOCAL_PROVIDER),
        account(8),
        8,
    ));
    let completed = factory
        .seal_completed_musubi_archive(
            &test_network_id(),
            cursor(8),
            ProviderId::new(LOCAL_PROVIDER),
            &pending.order,
            &pending.pin.manifest,
            binding,
        )
        .expect("seal exact local finalized completion");
    assert_eq!(completed.provider_id(), LOCAL_PROVIDER);
    assert!(
        completed.completed_musubi_store_instance.is_none(),
        "generic finalized-ledger claims must not carry attestation store authority"
    );
    assert_eq!(
        completed.completion(),
        &pending.order.provider_completions[0]
    );
    pending.completed_musubi_archive = Some(completed);
    assert!(
        validate_assignment(&pending, cursor(8), LOCAL_PROVIDER, runtime_policy()).is_ok(),
        "a completed Musubi row must carry its exact post-completion capability"
    );
    let mut substituted_claim = pending.clone();
    substituted_claim
        .completed_musubi_archive
        .as_mut()
        .expect("completed claim")
        .observed_finalized_cursor = cursor(7);
    assert!(matches!(
        validate_assignment(
            &substituted_claim,
            cursor(8),
            LOCAL_PROVIDER,
            runtime_policy(),
        ),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
    ));
    let mut missing = pending.clone();
    missing.completed_musubi_archive = None;
    assert!(matches!(
        validate_assignment(&missing, cursor(8), LOCAL_PROVIDER, runtime_policy()),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
    ));
    let mut generic = fixture_row(0x35);
    generic.order.provider_completions.push(completion_record(
        ProviderId::new(LOCAL_PROVIDER),
        account(8),
        8,
    ));
    assert!(
        validate_assignment(&generic, cursor(8), LOCAL_PROVIDER, runtime_policy()).is_ok(),
        "a generic finalized completion must not carry a Musubi capability"
    );
    generic.completed_musubi_archive = pending.completed_musubi_archive;
    assert!(matches!(
        validate_assignment(&generic, cursor(8), LOCAL_PROVIDER, runtime_policy()),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
    ));
}
#[test]
fn musubi_claims_and_receipts_remain_valid_across_later_finalized_scans() {
    let row = fixture_musubi_row(0x38, 0x85);
    let admission = validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy())
        .expect("validate admission")
        .authorization;
    let admitted_claim = row.musubi_archive.as_ref().expect("Musubi admission claim");
    let admitted_receipt = test_verified_musubi_receipt(admitted_claim, &admission);
    assert!(admitted_receipt.validate_stored(&admission));
    let mut later_claim = admitted_claim.clone();
    later_claim.observed_finalized_cursor = cursor(9);
    assert!(later_claim.matches_authorization(&admission));
    assert!(admitted_receipt.matches(&later_claim, &admission));
    assert!(
        ProviderIngestMusubiArchiveFetchBindingV1::from_finalized_claim(&later_claim)
            .matches_authorization(&admission)
    );
    assert_eq!(
        ProviderIngestSourceRequestV1::new(admission.clone(), vec![SOURCE_PROVIDER], None,),
        Err(ProviderIngestSourceFetchErrorV1::Rejected),
        "a durable Musubi authorization cannot be downgraded to a generic fetch"
    );
    let generic_authorization = FinalizedProviderIngestAuthorizationV1::from_finalized_state(
        admission.finalized_height(),
        admission.finalized_block_hash(),
        admission.provider_id(),
        admission.order_id(),
        admission.manifest_digest(),
        admission.manifest_cid().to_vec(),
        admission.chunker_handle().to_owned(),
        admission.chunk_digest_sha3_256(),
        admission.por_root(),
        admission.content_length(),
    )
    .expect("generic authorization with the same storage binding");
    assert_eq!(
        ProviderIngestSourceRequestV1::new(
            generic_authorization,
            vec![SOURCE_PROVIDER],
            Some(ProviderIngestMusubiArchiveFetchBindingV1::from_finalized_claim(&later_claim,)),
        ),
        Err(ProviderIngestSourceFetchErrorV1::Rejected),
        "a generic authorization cannot be upgraded by an informational fetch binding"
    );
    let later_receipt = test_verified_musubi_receipt(&later_claim, &admission);
    assert!(later_receipt.validate_stored(&admission));
    let mut latest_claim = later_claim.clone();
    latest_claim.observed_finalized_cursor = cursor(10);
    assert!(later_receipt.matches(&latest_claim, &admission));
    let replay_authorization = FinalizedProviderIngestAuthorizationV1::from_finalized_musubi_state(
        cursor(9).height,
        cursor(9).block_hash,
        admission.provider_id(),
        admission.order_id(),
        admission.manifest_digest(),
        admission.manifest_cid().to_vec(),
        admission.chunker_handle().to_owned(),
        admission.chunk_digest_sha3_256(),
        admission.por_root(),
        admission.content_length(),
        admission
            .musubi_context()
            .expect("Musubi authorization context")
            .clone(),
    )
    .expect("later finalized replay authorization");
    let outbox = ProviderIngestOutbox::in_memory(outbox_policy()).expect("Musubi outbox");
    outbox
        .enqueue(admission.clone())
        .expect("enqueue admission authorization");
    assert!(matches!(
        outbox
            .enqueue(replay_authorization)
            .expect("replay at later finalized head"),
        crate::provider_ingest_outbox::ProviderIngestEnqueueResultV1::ExistingActive { .. }
    ));
    let retained = outbox
        .authorization(admission.job_id())
        .expect("retained admission authorization");
    assert_eq!(retained, admission);
    outbox
        .observe_finalized_snapshot(cursor(9), 9_000)
        .expect("advance durable finalized high-water for later receipt");
    let source_claim = outbox
        .claim_source(
            retained.job_id(),
            ProviderIngestClaimOwnerV1::new([0xD4; 32]).expect("claim owner"),
            100,
            cursor(9),
        )
        .expect("claim replayed Musubi source work");
    outbox
        .mark_local_stored_verified(
            &source_claim,
            101,
            hex::encode(retained.manifest_digest()),
            Some(later_receipt.clone()),
        )
        .expect("persist verifier receipt from later finalized scan");
    let status = outbox.status(retained.job_id()).expect("stored status");
    let ProviderIngestDeliveryStateV1::LocalStored {
        musubi_bundle: Some(stored_receipt),
        ..
    } = status.state
    else {
        panic!("expected persisted Musubi verifier receipt");
    };
    assert!(persisted_receipt_matches(
        &retained,
        Some(&latest_claim),
        Some(stored_receipt.as_ref()),
    ));
    let mut stale_claim = admitted_claim.clone();
    stale_claim.observed_finalized_cursor = cursor(7);
    assert!(!stale_claim.matches_authorization(&admission));
    let mut forked_claim = admitted_claim.clone();
    forked_claim.observed_finalized_cursor.block_hash = [0xF8; 32];
    assert!(!forked_claim.matches_authorization(&admission));
    let mut receipt_from_future = later_receipt;
    receipt_from_future.observed_finalized_cursor = cursor(11);
    assert!(!receipt_from_future.matches(&latest_claim, &admission));
}
#[test]
fn musubi_attestation_approval_request_binds_exact_completed_verified_evidence() {
    let VerifiedAttestationBundleFixtureV1 {
        verified,
        commitment,
        ..
    } = verified_attestation_bundle_fixture(0xD1);
    let claim = completed_attestation_claim(commitment.clone());
    let first = ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
        &claim, &verified,
    )
    .expect("derive approval request");
    let second = ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
        &claim, &verified,
    )
    .expect("derive deterministic approval request");
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let ambient = {
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
            &claim, &verified,
        )
        .expect("derive ambient-independent approval request")
    };
    assert_eq!(first, second);
    assert_eq!(first, ambient);
    assert_ne!(first.completion_claim_digest(), [0; 32]);
    assert_eq!(first.observed_finalized_cursor(), cursor(8));
    assert_eq!(
        first.signer_policy(),
        claim.completion().completion_authority.signer_policy
    );
    assert_eq!(first.payload().version, MUSUBI_REGISTRY_VERSION_V1);
    assert_eq!(
        first.payload().binding.network_id,
        *claim.network_id(),
        "payload must bind the runtime-selected network"
    );
    assert_eq!(
        first.payload().binding.provider_id,
        ProviderId::new(LOCAL_PROVIDER)
    );
    assert_eq!(
        first.payload().binding.replication_order.as_bytes(),
        &claim.replication_order()
    );
    assert_eq!(first.payload().binding.archive_id, commitment.archive_id());
    assert_eq!(
        first.payload().binding.bundle_digest,
        commitment.bundle_digest
    );
    assert_eq!(
        first.payload().binding.descriptor_digest,
        commitment.descriptor_digest
    );
    assert_eq!(
        first.payload().binding.semantic_release_manifest_digest,
        verified.semantic_release().semantic_digest()
    );
    assert_eq!(
        first.payload().binding.verification_lock_digest,
        verified.verification_lock().digest()
    );
    assert_eq!(
        first.payload().signing_hash(),
        second.payload().signing_hash()
    );
    let mut later_claim = claim.clone();
    later_claim.observed_finalized_cursor = cursor(9);
    let later = ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
        &later_claim,
        &verified,
    )
    .expect("the identical completed row can be reverified at a later finalized head");
    assert_eq!(later.payload(), first.payload());
    assert_eq!(
        later.completion_claim_digest(),
        first.completion_claim_digest()
    );
    assert_eq!(later.observed_finalized_cursor(), cursor(9));
}
#[test]
fn musubi_attestation_approval_request_rejects_substituted_evidence() {
    let VerifiedAttestationBundleFixtureV1 {
        verified,
        commitment,
        ..
    } = verified_attestation_bundle_fixture(0xD2);
    let claim = completed_attestation_claim(commitment);
    let other_verified = verified_attestation_bundle_fixture(0xD3).verified;
    assert_eq!(
        ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
            &claim,
            &other_verified,
        ),
        Err(ProviderIngestMusubiAttestationApprovalRequestErrorV1::Rejected),
        "verified bundle evidence from another commitment must fail"
    );
    let mut substituted_bundle_commitment = claim.clone();
    substituted_bundle_commitment
        .binding
        .commitment
        .bundle_digest = MusubiContentDigestV1::new([0xD6; 32]);
    substituted_bundle_commitment.binding.archive_id = substituted_bundle_commitment
        .binding
        .commitment
        .archive_id();
    assert_eq!(
        ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
            &substituted_bundle_commitment,
            &verified,
        ),
        Err(ProviderIngestMusubiAttestationApprovalRequestErrorV1::Rejected),
        "verified evidence must retain the exact archive identity, not only projected fields"
    );
    let mut substituted_descriptor_commitment = claim.clone();
    substituted_descriptor_commitment
        .binding
        .commitment
        .descriptor_digest = MusubiContentDigestV1::new([0xD4; 32]);
    substituted_descriptor_commitment.binding.archive_id = substituted_descriptor_commitment
        .binding
        .commitment
        .archive_id();
    assert_eq!(
        ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
            &substituted_descriptor_commitment,
            &verified,
        ),
        Err(ProviderIngestMusubiAttestationApprovalRequestErrorV1::Rejected),
        "a substituted descriptor commitment must fail even with a self-consistent archive ID"
    );
    let mut substituted_completion = claim.clone();
    substituted_completion.completion.provider_id = ProviderId::new(SOURCE_PROVIDER);
    assert_eq!(
        ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
            &substituted_completion,
            &verified,
        ),
        Err(ProviderIngestMusubiAttestationApprovalRequestErrorV1::Rejected),
        "another provider's completion must fail"
    );
    let mut substituted_cursor = claim;
    substituted_cursor.observed_finalized_cursor.block_hash = [0xD5; 32];
    assert_eq!(
        ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
            &substituted_cursor,
            &verified,
        ),
        Err(ProviderIngestMusubiAttestationApprovalRequestErrorV1::Rejected),
        "a same-height cursor that does not cover the completion anchor must fail"
    );
    let mut lower_cursor = substituted_cursor;
    lower_cursor.observed_finalized_cursor = cursor(7);
    assert_eq!(
        ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
            &lower_cursor,
            &verified,
        ),
        Err(ProviderIngestMusubiAttestationApprovalRequestErrorV1::Rejected),
        "a cursor below the completed-row anchor must fail"
    );
}
#[test]
fn completed_musubi_claim_matches_only_exact_authorization_and_finalized_prefix() {
    let fixture = verified_attestation_bundle_fixture(0xD8);
    let mut claim = completed_attestation_claim(fixture.commitment.clone());
    let authorization = completed_attestation_authorization(&claim, [0x93; 32]);
    assert!(claim.matches_authorization(&authorization));
    claim.observed_finalized_cursor = cursor(10);
    claim.completion.finalized_anchor = ProviderIngestFinalizedAnchorV1 {
        height: 9,
        block_hash: cursor(9).block_hash,
    };
    let late_admission = completed_attestation_authorization(&claim, [0x93; 32]);
    assert!(
        claim.matches_authorization(&late_admission),
        "a completed row may first be observed after its own finalized anchor"
    );
    let mut stale = claim.clone();
    stale.observed_finalized_cursor = cursor(8);
    assert!(!stale.matches_authorization(&late_admission));
    let mut substituted_network = claim.clone();
    substituted_network.network_id = foreign_test_network_id();
    assert!(!substituted_network.matches_authorization(&late_admission));
    let mut substituted_commitment = claim.clone();
    substituted_commitment.binding.commitment.por_root = MusubiContentDigestV1::new([0xDA; 32]);
    substituted_commitment.binding.archive_id =
        substituted_commitment.binding.commitment.archive_id();
    assert!(!substituted_commitment.matches_authorization(&late_admission));
    let mut inert_cursor = claim.clone();
    inert_cursor.observed_finalized_cursor.block_hash = [0; 32];
    assert!(!inert_cursor.matches_authorization(&late_admission));
    let conflicting_admission =
        FinalizedProviderIngestAuthorizationV1::from_finalized_musubi_state(
            claim.completion.finalized_anchor.height,
            [0xDB; 32],
            claim.provider_id(),
            claim.replication_order(),
            [0x93; 32],
            claim.commitment().root_cid.as_bytes().to_vec(),
            claim.commitment().chunker.to_handle(),
            *claim.commitment().chunk_plan_digest.as_bytes(),
            *claim.commitment().por_root.as_bytes(),
            claim.commitment().content_length,
            FinalizedProviderIngestMusubiContextV1::new(*claim.network_id(), claim.archive_id())
                .expect("conflicting-admission Musubi context"),
        )
        .expect("conflicting retained authorization");
    assert!(
        !claim.matches_authorization(&conflicting_admission),
        "admission and completion cannot name different blocks at one height"
    );
}
#[test]
fn store_bound_admitted_payload_lease_mints_completed_musubi_approval_request() {
    let fixture = verified_attestation_bundle_fixture(0xD9);
    let manifest = completed_attestation_manifest(&fixture);
    let temp_dir = tempfile::tempdir().expect("completed-attestation storage tempdir");
    let data_dir = temp_dir
        .path()
        .canonicalize()
        .expect("canonical completed-attestation tempdir")
        .join("storage");
    let backend = StorageBackend::new(
        StorageConfig::builder()
            .enabled(true)
            .data_dir(data_dir)
            .build(),
    )
    .expect("completed-attestation storage backend");
    let mut payload_reader = fixture.payload.as_slice();
    let manifest_id = backend
        .ingest_manifest(&manifest, &fixture.plan, &mut payload_reader)
        .expect("ingest completed-attestation payload");
    let stored = backend
        .manifest(&manifest_id)
        .expect("completed-attestation stored manifest");
    let manifest_digest = *stored.manifest_digest();
    let claim = completed_attestation_claim(fixture.commitment.clone());
    let store_instance = claim
        .completed_musubi_store_instance
        .clone()
        .expect("completed-attestation fixture is store-bound");
    let authorization = completed_attestation_authorization(&claim, manifest_digest);
    let schedulers = StorageSchedulersRuntime::new(StorageSchedulerConfig::default());
    let first_request = ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
        &claim,
        &fixture.verified,
    )
    .expect("derive first store-bound approval request");
    let mut restarted_claim = claim.clone();
    restarted_claim.completed_musubi_store_instance = Some(CompletedMusubiStoreInstanceV1::new());
    let restarted_request =
        ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
            &restarted_claim,
            &fixture.verified,
        )
        .expect("rederive approval request under a restarted store instance");
    assert!(
        !first_request
            .completed_musubi_store_instance
            .matches(&restarted_request.completed_musubi_store_instance,),
        "a restarted process must receive a fresh in-memory authority"
    );
    assert_eq!(
        claim, restarted_claim,
        "public claim equality must ignore the process-local authority"
    );
    assert_eq!(
        first_request, restarted_request,
        "public request equality must ignore the process-local authority"
    );
    let first_candidate = ProviderIngestCompletedMusubiCaptureCandidateV1 {
        authorization: authorization.clone(),
        completed_claim: claim.clone(),
        completed_musubi_store_instance: store_instance.clone(),
    };
    let restarted_candidate = ProviderIngestCompletedMusubiCaptureCandidateV1 {
        authorization: authorization.clone(),
        completed_claim: restarted_claim.clone(),
        completed_musubi_store_instance: restarted_request.completed_musubi_store_instance.clone(),
    };
    assert_eq!(
        first_candidate, restarted_candidate,
        "public candidate equality must ignore the process-local authority"
    );
    assert_eq!(
        first_request.completion_claim_digest(),
        restarted_request.completion_claim_digest(),
        "the ephemeral authority must not enter the stable completion digest"
    );
    assert_eq!(
        musubi_provider_attestation_approval_id_v1(&first_request),
        musubi_provider_attestation_approval_id_v1(&restarted_request),
        "the ephemeral authority must not enter the durable approval ID"
    );
    let mut unbound_claim = claim.clone();
    unbound_claim.completed_musubi_store_instance = None;
    assert_eq!(
        ProviderIngestMusubiAttestationApprovalRequestV1::from_verified_completion(
            &unbound_claim,
            &fixture.verified,
        ),
        Err(ProviderIngestMusubiAttestationApprovalRequestErrorV1::Rejected),
    );
    let foreign_store_instance = CompletedMusubiStoreInstanceV1::new();
    let (foreign_result, first_reader_still_available) = backend
        .with_admitted_payload_read_lease_by_digest(&manifest_digest, &schedulers, |lease| {
            let result = lease.verify_completed_musubi_bundle(
                &foreign_store_instance,
                &fixture.plan,
                &authorization,
                &claim,
            );
            (result, lease.open_reader().is_ok())
        })
        .expect("acquire foreign-instance lifecycle lease")
        .expect("completed-attestation payload remains admitted");
    assert_eq!(
        foreign_result,
        Err(ProviderIngestLocalStorageErrorV1::Permanent)
    );
    assert!(
        first_reader_still_available,
        "foreign instance must fail before any admitted payload reader opens"
    );
    assert_eq!(fixture.verified.archive_id(), claim.archive_id());
    let (request, fourth_reader_rejected) = backend
        .with_admitted_payload_read_lease_by_digest(&manifest_digest, &schedulers, |lease| {
            let request = lease.verify_completed_musubi_bundle(
                &store_instance,
                &fixture.plan,
                &authorization,
                &claim,
            );
            let fourth_reader_rejected = matches!(
                lease.open_reader(),
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied
            );
            (request, fourth_reader_rejected)
        })
        .expect("acquire completed-attestation lifecycle lease")
        .expect("completed-attestation payload remains admitted");
    let request = request.expect("fresh lease verification mints approval request");
    assert!(
        fourth_reader_rejected,
        "verification must consume exactly three fresh readers"
    );
    assert_eq!(request.payload().binding.archive_id, claim.archive_id());
    let mut substituted_plan = fixture.plan.clone();
    substituted_plan.payload_digest = blake3::hash(b"substituted admitted payload");
    assert_eq!(
        backend
            .with_admitted_payload_read_lease_by_digest(&manifest_digest, &schedulers, |lease| {
                lease.verify_completed_musubi_bundle(
                    &store_instance,
                    &substituted_plan,
                    &authorization,
                    &claim,
                )
            },)
            .expect("acquire substituted-plan lifecycle lease")
            .expect("completed-attestation payload remains admitted"),
        Err(ProviderIngestLocalStorageErrorV1::Permanent),
    );
    let substituted_authorization = completed_attestation_authorization(&claim, [0xDC; 32]);
    assert_eq!(
        backend
            .with_admitted_payload_read_lease_by_digest(&manifest_digest, &schedulers, |lease| {
                lease.verify_completed_musubi_bundle(
                    &store_instance,
                    &fixture.plan,
                    &substituted_authorization,
                    &claim,
                )
            },)
            .expect("acquire substituted-authorization lifecycle lease")
            .expect("completed-attestation payload remains admitted"),
        Err(ProviderIngestLocalStorageErrorV1::Permanent),
    );
    let mut stale_claim = claim.clone();
    stale_claim.observed_finalized_cursor = cursor(7);
    assert_eq!(
        backend
            .with_admitted_payload_read_lease_by_digest(&manifest_digest, &schedulers, |lease| {
                lease.verify_completed_musubi_bundle(
                    &store_instance,
                    &fixture.plan,
                    &authorization,
                    &stale_claim,
                )
            },)
            .expect("acquire stale-claim lifecycle lease")
            .expect("completed-attestation payload remains admitted"),
        Err(ProviderIngestLocalStorageErrorV1::Permanent),
    );
    let mut substituted_claim = claim;
    substituted_claim.binding.commitment.bundle_digest = MusubiContentDigestV1::new([0xDD; 32]);
    substituted_claim.binding.archive_id = substituted_claim.binding.commitment.archive_id();
    assert_eq!(
        backend
            .with_admitted_payload_read_lease_by_digest(&manifest_digest, &schedulers, |lease| {
                lease.verify_completed_musubi_bundle(
                    &store_instance,
                    &fixture.plan,
                    &authorization,
                    &substituted_claim,
                )
            },)
            .expect("acquire substituted-claim lifecycle lease")
            .expect("completed-attestation payload remains admitted"),
        Err(ProviderIngestLocalStorageErrorV1::Permanent),
    );
}
#[test]
fn completed_musubi_lease_preserves_transient_reader_classification() {
    for kind in [
        io::ErrorKind::Interrupted,
        io::ErrorKind::WouldBlock,
        io::ErrorKind::TimedOut,
        io::ErrorKind::NotFound,
        io::ErrorKind::Other,
    ] {
        assert!(provider_ingest_admitted_payload_read_error_is_retryable(
            kind
        ));
    }
    for kind in [
        io::ErrorKind::InvalidData,
        io::ErrorKind::UnexpectedEof,
        io::ErrorKind::PermissionDenied,
    ] {
        assert!(!provider_ingest_admitted_payload_read_error_is_retryable(
            kind
        ));
    }
}
#[test]
fn verified_musubi_receipt_rejects_archive_identity_substitution() {
    let VerifiedAttestationBundleFixtureV1 {
        verified,
        commitment,
        ..
    } = verified_attestation_bundle_fixture(0xD7);
    let binding = MusubiReplicationOrderArchiveBindingV1::new(
        ReplicationOrderId::new([0xAD; 32]),
        commitment.archive_id(),
        commitment,
    );
    let claim = ProviderIngestFinalizedMusubiArchiveClaimV1 {
        network_id: test_network_id(),
        provider_id: LOCAL_PROVIDER,
        observed_finalized_cursor: cursor(8),
        binding,
    };
    let authorization_for = |claim: &ProviderIngestFinalizedMusubiArchiveClaimV1| {
        FinalizedProviderIngestAuthorizationV1::from_finalized_musubi_state(
            8,
            cursor(8).block_hash,
            LOCAL_PROVIDER,
            claim.replication_order(),
            [0xAE; 32],
            claim.commitment().root_cid.as_bytes().to_vec(),
            claim.commitment().chunker.to_handle(),
            *claim.commitment().chunk_plan_digest.as_bytes(),
            *claim.commitment().por_root.as_bytes(),
            claim.commitment().content_length,
            FinalizedProviderIngestMusubiContextV1::new(test_network_id(), claim.archive_id())
                .expect("Musubi context"),
        )
        .expect("Musubi authorization")
    };
    let authorization = authorization_for(&claim);
    ProviderIngestVerifiedMusubiBundleReceiptV1::from_verified_bundle(
        &claim,
        &authorization,
        &verified,
    )
    .expect("exact verifier evidence");
    let mut substituted = claim;
    substituted.binding.commitment.bundle_digest = MusubiContentDigestV1::new([0xAF; 32]);
    substituted.binding.archive_id = substituted.binding.commitment.archive_id();
    substituted
        .binding
        .validate()
        .expect("structurally valid substituted binding");
    let substituted_authorization = authorization_for(&substituted);
    assert_eq!(
        ProviderIngestVerifiedMusubiBundleReceiptV1::from_verified_bundle(
            &substituted,
            &substituted_authorization,
            &verified,
        ),
        Err(ProviderIngestLocalStorageErrorV1::Permanent),
        "a verifier result cannot be replayed under a different archive identity"
    );
}
#[test]
fn completed_musubi_claim_factory_rejects_completion_substitutions() {
    let factory = ProviderIngestFinalizedClaimFactoryV1::new(test_network_id(), LOCAL_PROVIDER);
    let mut row = fixture_musubi_row(0x36, 0x83);
    row.order.provider_completions.push(completion_record(
        ProviderId::new(LOCAL_PROVIDER),
        account(8),
        8,
    ));
    let binding = musubi_binding_for_row(&row, 0x83);
    assert_eq!(
        factory.seal_completed_musubi_archive(
            &foreign_test_network_id(),
            cursor(8),
            ProviderId::new(LOCAL_PROVIDER),
            &row.order,
            &row.pin.manifest,
            binding.clone(),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
    );
    assert_eq!(
        factory.seal_completed_musubi_archive(
            &test_network_id(),
            cursor(8),
            ProviderId::new(SOURCE_PROVIDER),
            &row.order,
            &row.pin.manifest,
            binding.clone(),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
    );
    let mut earlier_cursor = cursor(7);
    earlier_cursor.block_hash = [0xE7; 32];
    assert_eq!(
        factory.seal_completed_musubi_archive(
            &test_network_id(),
            earlier_cursor,
            ProviderId::new(LOCAL_PROVIDER),
            &row.order,
            &row.pin.manifest,
            binding.clone(),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
    );
    let mut substituted_pin = row.pin.manifest.clone();
    substituted_pin.por_root = [0xE8; 32];
    assert_eq!(
        factory.seal_completed_musubi_archive(
            &test_network_id(),
            cursor(8),
            ProviderId::new(LOCAL_PROVIDER),
            &row.order,
            &substituted_pin,
            binding.clone(),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
    );
    let mut substituted_completion = row.order.clone();
    substituted_completion.provider_completions[0].assignment_revision = 2;
    assert_eq!(
        factory.seal_completed_musubi_archive(
            &test_network_id(),
            cursor(8),
            ProviderId::new(LOCAL_PROVIDER),
            &substituted_completion,
            &row.pin.manifest,
            binding.clone(),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
    );
    let mut duplicate_completion = row.order.clone();
    duplicate_completion
        .provider_completions
        .push(duplicate_completion.provider_completions[0].clone());
    assert_eq!(
        factory.seal_completed_musubi_archive(
            &test_network_id(),
            cursor(8),
            ProviderId::new(LOCAL_PROVIDER),
            &duplicate_completion,
            &row.pin.manifest,
            binding.clone(),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
    );
    let mut unassigned_order = row.order.clone();
    let mut canonical_order = decode_from_bytes_with_limits::<ReplicationOrderV1>(
        &unassigned_order.canonical_order,
        REPLICATION_ORDER_DECODE_LIMITS_V1,
    )
    .expect("decode fixture order");
    canonical_order
        .assignments
        .retain(|assignment| assignment.provider_id != LOCAL_PROVIDER);
    unassigned_order.canonical_order = norito::to_bytes(&canonical_order).expect("order bytes");
    assert_eq!(
        factory.seal_completed_musubi_archive(
            &test_network_id(),
            cursor(8),
            ProviderId::new(LOCAL_PROVIDER),
            &unassigned_order,
            &row.pin.manifest,
            binding.clone(),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
    );
    let other = fixture_musubi_row(0x37, 0x84);
    assert_eq!(
        factory.seal_completed_musubi_archive(
            &test_network_id(),
            cursor(8),
            ProviderId::new(LOCAL_PROVIDER),
            &row.order,
            &row.pin.manifest,
            musubi_binding_for_row(&other, 0x84),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
    );
}
#[tokio::test]
async fn runtime_recovers_durable_finalized_high_water_before_scanning() {
    let row = fixture_row(0x2F);
    let (runtime, _, _, _) = test_runtime(
        row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    runtime
        .outbox
        .observe_finalized_snapshot(cursor(9), 9_000)
        .expect("persist later cursor");
    let mut restarted = ProviderIngestRuntimeV1::new(
        runtime.provider_id,
        runtime.network_id,
        runtime.claim_owner,
        runtime.policy,
        runtime.outbox.clone(),
        runtime.ledger.clone(),
        runtime.fetch.clone(),
        runtime.storage.clone(),
        runtime.payload_builder.clone(),
        runtime.signer_resolver.clone(),
        runtime.ingress.clone(),
        runtime.clock.clone(),
    )
    .expect("restart runtime");
    assert_eq!(restarted.last_finalized_cursor, Some(cursor(9)));
    assert!(matches!(
        restarted.tick().await,
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
    ));
    assert_eq!(
        restarted.outbox.finalized_cursor_high_water().unwrap(),
        Some(cursor(9))
    );
}
#[tokio::test]
async fn finalized_block_time_equivocation_is_rejected_after_restart() {
    let row = fixture_row(0x44);
    let (runtime, ledger, _, _) = test_runtime(
        row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    runtime
        .outbox
        .observe_finalized_snapshot(cursor(8), 8_000)
        .expect("persist finalized snapshot");
    ledger.page.lock().unwrap().finalized_block_time_ms = 8_001;
    let mut restarted = ProviderIngestRuntimeV1::new(
        runtime.provider_id,
        runtime.network_id,
        runtime.claim_owner,
        runtime.policy,
        runtime.outbox.clone(),
        runtime.ledger.clone(),
        runtime.fetch.clone(),
        runtime.storage.clone(),
        runtime.payload_builder.clone(),
        runtime.signer_resolver.clone(),
        runtime.ingress.clone(),
        runtime.clock.clone(),
    )
    .expect("restart runtime");
    assert!(matches!(
        restarted.tick().await,
        Err(ProviderIngestRuntimeErrorV1::Outbox(
            ProviderIngestOutboxError::FinalizedSnapshotConflict
        ))
    ));
    assert_eq!(
        restarted.outbox.finalized_snapshot_high_water().unwrap(),
        Some((cursor(8), 8_000))
    );
}
#[tokio::test]
async fn local_existing_path_skips_network_and_preflights_before_ambiguity() {
    let row = fixture_row(0x32);
    let (mut runtime, _, fetch, ingress) = test_runtime(
        row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    let outcome = runtime.tick().await.expect("tick");
    assert_eq!(outcome.manifests_stored, 1);
    assert_eq!(outcome.completions_signed, 1);
    assert_eq!(outcome.completion_submissions, 1);
    assert_eq!(fetch.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        *ingress.events.lock().unwrap(),
        vec!["prepare_signed", "expose_ambiguous"]
    );
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::LocalStored {
            completion: ProviderIngestCompletionStateV1::Submitted { .. },
            ..
        }
    ));
}
#[test]
fn musubi_local_stored_without_verifier_receipt_is_never_checkpointed() {
    let row = fixture_musubi_row(0x6A, 0xB1);
    let authorization = validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy())
        .expect("valid Musubi assignment")
        .authorization;
    let (runtime, _, _, ingress) = test_runtime(
        row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    runtime
        .outbox
        .enqueue(authorization.clone())
        .expect("enqueue Musubi job");
    let source_claim = runtime
        .outbox
        .claim_source(
            authorization.job_id(),
            runtime.claim_owner,
            1_000,
            cursor(8),
        )
        .expect("claim Musubi source job");
    assert_eq!(
        runtime.outbox.mark_local_stored(
            &source_claim,
            1_001,
            hex::encode(authorization.manifest_digest()),
        ),
        Err(ProviderIngestOutboxError::InvalidAuthorization)
    );
    assert!(matches!(
        runtime.outbox.status(authorization.job_id()).unwrap().state,
        ProviderIngestDeliveryStateV1::SourceClaimed { .. }
    ));
    assert!(ingress.events.lock().unwrap().is_empty());
}
#[tokio::test]
async fn single_replica_without_remote_sources_releases_source_claim_for_retry() {
    let mut row = fixture_row(0x35);
    let mut order = decode_from_bytes_with_limits::<ReplicationOrderV1>(
        &row.order.canonical_order,
        REPLICATION_ORDER_DECODE_LIMITS_V1,
    )
    .expect("decode fixture order");
    order.target_replicas = 1;
    order
        .assignments
        .retain(|assignment| assignment.provider_id == LOCAL_PROVIDER);
    order.validate().expect("valid single-replica order");
    row.order.canonical_order = norito::to_bytes(&order).expect("order bytes");
    let (mut runtime, _, fetch, ingress) = test_runtime(
        row,
        false,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    let outcome = runtime.tick().await.expect("single-replica source tick");
    assert_eq!(outcome.source_jobs_claimed, 1);
    assert_eq!(fetch.calls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::RetryScheduled {
            failure_class: ProviderIngestFailureClassV1::SourceUnavailable,
            ..
        }
    ));
}
#[tokio::test]
async fn corrupt_authenticated_source_is_retryable_not_a_permanent_dead_letter() {
    let row = fixture_row(0x33);
    let (mut runtime, _, fetch, ingress) = test_runtime(
        row,
        false,
        Err(ProviderIngestSourceFetchErrorV1::ContentRejected),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    runtime.tick().await.expect("tick");
    assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::RetryScheduled {
            failure_class: ProviderIngestFailureClassV1::SourceRejected,
            ..
        }
    ));
}
#[tokio::test]
async fn authenticated_source_binding_rejection_is_terminal_for_the_tick() {
    let row = fixture_row(0x34);
    let (mut runtime, _, fetch, ingress) = test_runtime(
        row,
        false,
        Err(ProviderIngestSourceFetchErrorV1::Rejected),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    assert!(matches!(
        runtime.tick().await,
        Err(ProviderIngestRuntimeErrorV1::SourceProtocolViolation)
    ));
    assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::RetryScheduled {
            failure_class: ProviderIngestFailureClassV1::SourceRejected,
            ..
        }
    ));
}
#[tokio::test]
async fn ineligible_early_source_does_not_consume_fair_work_budget() {
    let first = fixture_row(0x10);
    let second = fixture_row(0x20);
    let (mut runtime, ledger, fetch, _) = test_runtime(
        first.clone(),
        false,
        Err(ProviderIngestSourceFetchErrorV1::ContentRejected),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    ledger.page.lock().unwrap().rows = vec![first.clone(), second.clone()];
    let first_authorization =
        validate_assignment(&first, cursor(8), LOCAL_PROVIDER, runtime_policy())
            .unwrap()
            .authorization;
    runtime.outbox.enqueue(first_authorization.clone()).unwrap();
    let claim = runtime
        .outbox
        .claim_source(
            first_authorization.job_id(),
            ProviderIngestClaimOwnerV1::new([0xDD; 32]).unwrap(),
            10_000,
            cursor(8),
        )
        .unwrap();
    runtime
        .outbox
        .schedule_source_retry(
            &claim,
            10_001,
            cursor(8),
            ProviderIngestFailureClassV1::SourceUnavailable,
        )
        .unwrap();
    runtime.policy.max_source_jobs_per_tick = 1;
    let outcome = runtime.tick().await.expect("fair source tick");
    assert_eq!(outcome.source_jobs_claimed, 1);
    assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
    let second_authorization =
        validate_assignment(&second, cursor(8), LOCAL_PROVIDER, runtime_policy())
            .unwrap()
            .authorization;
    assert!(matches!(
        runtime
            .outbox
            .status(second_authorization.job_id())
            .unwrap()
            .state,
        ProviderIngestDeliveryStateV1::RetryScheduled {
            failure_class: ProviderIngestFailureClassV1::SourceRejected,
            ..
        }
    ));
}
#[tokio::test]
async fn slow_fetch_renews_the_source_lease_until_atomic_storage_finishes() {
    let row = fixture_row(0x34);
    let (mut runtime, _, fetch, ingress) = test_runtime(
        row,
        false,
        Ok(vec![0xA5]),
        45,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    runtime.tick().await.expect("renewed tick");
    assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::LocalStored { .. }
    ));
}
#[tokio::test]
async fn semantic_completion_from_another_replica_wins_over_ambiguous_local_hash() {
    let row = fixture_row(0x35);
    let (mut runtime, ledger, _, ingress) = test_runtime(
        row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Ambiguous,
        false,
    );
    runtime.tick().await.expect("first tick");
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::LocalStored {
            completion: ProviderIngestCompletionStateV1::Ambiguous { .. },
            ..
        }
    ));
    {
        let mut page = ledger.page.lock().unwrap();
        page.finalized_cursor = cursor(9);
        page.finalized_block_time_ms = 9_000;
        page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
            height: 9,
            block_hash: cursor(9).block_hash,
        };
        page.rows[0]
            .order
            .provider_completions
            .push(completion_record(
                ProviderId::new(SOURCE_PROVIDER),
                account(7),
                8,
            ));
        page.rows[0]
            .order
            .provider_completions
            .push(completion_record(
                ProviderId::new(LOCAL_PROVIDER),
                account(9),
                9,
            ));
        page.rows[0].order.status = ReplicationOrderStatus::Completed(9);
        page.rows[0].completion_epoch = Some(9);
    }
    runtime.tick().await.expect("semantic reconciliation");
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::FinalizedCompleted {
            completion_epoch: 9,
            completed_by,
            committed_transaction_hash: None,
            ..
        } if completed_by == account(9)
    ));
}
#[tokio::test]
async fn finalized_completion_first_row_bypasses_full_active_capacity() {
    let mut completed = fixture_row(0x2E);
    completed.order.provider_completions = vec![
        completion_record(ProviderId::new(SOURCE_PROVIDER), account(7), 8),
        completion_record(ProviderId::new(LOCAL_PROVIDER), account(9), 9),
    ];
    completed.order.status = ReplicationOrderStatus::Completed(9);
    completed.pin.finalized_cursor = PinManifestFinalizedCursorV1 {
        height: 9,
        block_hash: cursor(9).block_hash,
    };
    completed.completion_epoch = Some(9);
    let (mut runtime, _, fetch, ingress) = test_runtime(
        completed,
        false,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    for seed in 0x40_u8..=0x5F {
        let pending = fixture_row(seed);
        let authorization =
            validate_assignment(&pending, cursor(8), LOCAL_PROVIDER, runtime_policy())
                .unwrap()
                .authorization;
        runtime.outbox.enqueue(authorization).expect("fill active");
    }
    assert_eq!(
        runtime
            .outbox
            .aggregate_counts()
            .expect("full active inventory")
            .active,
        runtime.outbox.policy().max_active_entries
    );
    let outcome = runtime.tick().await.expect("finalized reconciliation");
    assert_eq!(outcome.rows_scanned, 1);
    assert_eq!(outcome.jobs_inserted, 0);
    assert_eq!(outcome.jobs_finalized, 1);
    assert_eq!(fetch.calls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::FinalizedCompleted {
            manifest_id: None,
            completion_epoch: 9,
            ..
        }
    ));
}
#[tokio::test]
async fn preflight_rejection_resigns_without_entering_ambiguous_state() {
    let row = fixture_row(0x39);
    let (mut runtime, _, _, ingress) = test_runtime(
        row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    *ingress.prepare_error.lock().unwrap() = Some(ProviderIngestIngressPrepareErrorV1::Rejected);
    runtime.tick().await.expect("preflight rejection");
    assert_eq!(*ingress.events.lock().unwrap(), vec!["prepare_signed"]);
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::LocalStored {
            completion: ProviderIngestCompletionStateV1::Ready {
                last_failure_class: Some(ProviderIngestFailureClassV1::TransactionRejected),
                ..
            },
            ..
        }
    ));
}
#[tokio::test]
async fn payload_and_preflight_failures_are_durably_backed_off() {
    let payload_row = fixture_row(0x3B);
    let (mut payload_runtime, _, _, payload_ingress) = test_runtime(
        payload_row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    payload_runtime.tick().await.expect("payload failure tick");
    assert!(matches!(
        payload_runtime
            .outbox
            .status(payload_ingress.job_id)
            .unwrap()
            .state,
        ProviderIngestDeliveryStateV1::LocalStored {
            completion: ProviderIngestCompletionStateV1::Ready {
                attempts: 1,
                last_failure_class: Some(ProviderIngestFailureClassV1::PayloadPreparationFailed),
                ..
            },
            ..
        }
    ));
    payload_runtime.tick().await.expect("payload backoff tick");
    assert!(matches!(
        payload_runtime
            .outbox
            .status(payload_ingress.job_id)
            .unwrap()
            .state,
        ProviderIngestDeliveryStateV1::LocalStored {
            completion: ProviderIngestCompletionStateV1::Ready { attempts: 1, .. },
            ..
        }
    ));
    let ingress_row = fixture_row(0x3C);
    let (mut ingress_runtime, _, _, ingress) = test_runtime(
        ingress_row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    *ingress.prepare_error.lock().unwrap() = Some(ProviderIngestIngressPrepareErrorV1::Unavailable);
    ingress_runtime.tick().await.expect("ingress unavailable");
    assert_eq!(*ingress.events.lock().unwrap(), vec!["prepare_signed"]);
    assert!(matches!(
        ingress_runtime
            .outbox
            .status(ingress.job_id)
            .unwrap()
            .state,
        ProviderIngestDeliveryStateV1::LocalStored {
            completion: ProviderIngestCompletionStateV1::Signed {
                attempts: 2,
                next_attempt_at_ms,
                ..
            },
            ..
        } if next_attempt_at_ms > ingress_runtime.clock.now_ms()
    ));
    ingress_runtime.tick().await.expect("signed retry not due");
    assert_eq!(*ingress.events.lock().unwrap(), vec!["prepare_signed"]);
}
#[tokio::test]
async fn ambiguous_unknown_retries_only_after_a_later_finalized_cursor() {
    let row = fixture_row(0x3A);
    let (mut runtime, ledger, _, ingress) = test_runtime(
        row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Ambiguous,
        false,
    );
    runtime.tick().await.expect("ambiguous submit");
    *ingress.observation.lock().unwrap() = ProviderIngestTransactionObservationV1::Unknown;
    {
        let mut page = ledger.page.lock().unwrap();
        page.finalized_cursor = cursor(9);
        page.finalized_block_time_ms = 9_000;
        page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
            height: 9,
            block_hash: cursor(9).block_hash,
        };
        page.rows[0].completion_epoch = Some(9);
    }
    runtime.tick().await.expect("finalized absence");
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::LocalStored {
            completion: ProviderIngestCompletionStateV1::Signed {
                baseline_finalized_cursor,
                ..
            },
            ..
        } if baseline_finalized_cursor == cursor(9)
    ));
}
#[tokio::test]
async fn committed_hash_outcome_never_substitutes_for_semantic_completion() {
    let row = fixture_row(0x3D);
    let (mut runtime, ledger, _, ingress) = test_runtime(
        row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    runtime.tick().await.expect("submitted transaction");
    *ingress.observation.lock().unwrap() = ProviderIngestTransactionObservationV1::CommittedSuccess;
    {
        let mut page = ledger.page.lock().unwrap();
        page.finalized_cursor = cursor(9);
        page.finalized_block_time_ms = 9_000;
        page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
            height: 9,
            block_hash: cursor(9).block_hash,
        };
        page.rows[0].completion_epoch = Some(9);
    }
    let outcome = runtime.tick().await.expect("committed-success observation");
    assert_eq!(outcome.jobs_finalized, 0);
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::LocalStored {
            completion: ProviderIngestCompletionStateV1::Submitted { .. },
            ..
        }
    ));
    *ingress.observation.lock().unwrap() =
        ProviderIngestTransactionObservationV1::CommittedRejected;
    runtime
        .tick()
        .await
        .expect("committed rejection is retryable");
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::LocalStored {
            completion: ProviderIngestCompletionStateV1::Ready {
                last_failure_class: Some(ProviderIngestFailureClassV1::TransactionRejected),
                ..
            },
            ..
        }
    ));
}
#[tokio::test]
async fn owner_rotation_reconciles_exposed_transaction_before_authority_change() {
    let row = fixture_row(0x3F);
    let (mut runtime, ledger, _, ingress) = test_runtime(
        row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    runtime.tick().await.expect("submitted transaction");
    *ingress.observation.lock().unwrap() = ProviderIngestTransactionObservationV1::CommittedSuccess;
    {
        let mut page = ledger.page.lock().unwrap();
        page.finalized_cursor = cursor(9);
        page.finalized_block_time_ms = 9_000;
        page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
            height: 9,
            block_hash: cursor(9).block_hash,
        };
        page.rows[0].provider_owner = Some(account(9));
        page.rows[0].completion_authority = Some(ProviderIngestCompletionAuthorityV1::new(
            account(9),
            completion_signer_policy(1),
        ));
        page.rows[0].completion_epoch = Some(9);
    }
    runtime.tick().await.expect("owner rotation");
    assert_eq!(ingress.observe_calls.load(Ordering::SeqCst), 1);
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::LocalStored {
            completion: ProviderIngestCompletionStateV1::Submitted { .. },
            ..
        }
    ));
}
#[tokio::test]
async fn signer_policy_rotation_reconciles_exposed_transaction_before_authority_change() {
    let row = fixture_row(0x41);
    let (mut runtime, ledger, _, ingress) = test_runtime(
        row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    runtime.tick().await.expect("submitted transaction");
    *ingress.observation.lock().unwrap() = ProviderIngestTransactionObservationV1::CommittedSuccess;
    runtime
        .signer_resolver
        .signer_policy_revision
        .store(2, Ordering::SeqCst);
    {
        let mut page = ledger.page.lock().unwrap();
        page.finalized_cursor = cursor(9);
        page.finalized_block_time_ms = 9_000;
        page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
            height: 9,
            block_hash: cursor(9).block_hash,
        };
        page.rows[0].completion_epoch = Some(9);
    }
    runtime.tick().await.expect("signer policy rotation");
    assert_eq!(ingress.observe_calls.load(Ordering::SeqCst), 1);
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::LocalStored {
            completion: ProviderIngestCompletionStateV1::Submitted { .. },
            ..
        }
    ));
}
#[tokio::test]
async fn owner_removal_reconciles_exposed_transaction_before_authority_change() {
    let row = fixture_row(0x40);
    let (mut runtime, ledger, _, ingress) = test_runtime(
        row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    runtime.tick().await.expect("submitted transaction");
    *ingress.observation.lock().unwrap() = ProviderIngestTransactionObservationV1::CommittedSuccess;
    {
        let mut page = ledger.page.lock().unwrap();
        page.finalized_cursor = cursor(9);
        page.finalized_block_time_ms = 9_000;
        page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
            height: 9,
            block_hash: cursor(9).block_hash,
        };
        page.rows[0].provider_owner = None;
        page.rows[0].completion_authority = None;
        page.rows[0].completion_epoch = None;
    }
    runtime.tick().await.expect("owner removal");
    assert_eq!(ingress.observe_calls.load(Ordering::SeqCst), 1);
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::LocalStored {
            completion: ProviderIngestCompletionStateV1::Submitted { .. },
            ..
        }
    ));
}
#[tokio::test]
async fn owner_rotation_invalidates_never_exposed_signed_bytes_without_preflight() {
    let row = fixture_row(0x42);
    let (mut runtime, ledger, _, ingress) = test_runtime(
        row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    *ingress.prepare_error.lock().unwrap() = Some(ProviderIngestIngressPrepareErrorV1::Unavailable);
    let first = runtime
        .tick()
        .await
        .expect("sign before unavailable preflight");
    assert_eq!(first.completions_signed, 1);
    assert_eq!(first.completion_submissions, 0);
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::LocalStored {
            completion: ProviderIngestCompletionStateV1::Signed {
                ever_exposed: false,
                ..
            },
            ..
        }
    ));
    {
        let mut page = ledger.page.lock().unwrap();
        page.finalized_cursor = cursor(9);
        page.finalized_block_time_ms = 9_000;
        page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
            height: 9,
            block_hash: cursor(9).block_hash,
        };
        page.rows[0].provider_owner = Some(account(9));
        page.rows[0].completion_authority = Some(ProviderIngestCompletionAuthorityV1::new(
            account(9),
            completion_signer_policy(1),
        ));
        page.rows[0].completion_epoch = Some(9);
    }
    runtime.tick().await.expect("invalidate stale owner");
    assert_eq!(ingress.observe_calls.load(Ordering::SeqCst), 0);
    assert_eq!(*ingress.events.lock().unwrap(), vec!["prepare_signed"]);
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::LocalStored {
            completion: ProviderIngestCompletionStateV1::Ready {
                last_failure_class: Some(ProviderIngestFailureClassV1::ProviderOwnerChanged),
                ..
            },
            ..
        }
    ));
}
#[tokio::test]
async fn signer_policy_rotation_invalidates_never_exposed_signed_bytes_without_preflight() {
    let row = fixture_row(0x43);
    let (mut runtime, ledger, _, ingress) = test_runtime(
        row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    *ingress.prepare_error.lock().unwrap() = Some(ProviderIngestIngressPrepareErrorV1::Unavailable);
    runtime
        .tick()
        .await
        .expect("sign before unavailable preflight");
    runtime
        .signer_resolver
        .signer_policy_revision
        .store(2, Ordering::SeqCst);
    {
        let mut page = ledger.page.lock().unwrap();
        page.finalized_cursor = cursor(9);
        page.finalized_block_time_ms = 9_000;
        page.rows[0].pin.finalized_cursor = PinManifestFinalizedCursorV1 {
            height: 9,
            block_hash: cursor(9).block_hash,
        };
        page.rows[0].completion_epoch = Some(9);
    }
    runtime
        .tick()
        .await
        .expect("invalidate stale signer policy");
    assert_eq!(ingress.observe_calls.load(Ordering::SeqCst), 0);
    assert_eq!(*ingress.events.lock().unwrap(), vec!["prepare_signed"]);
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::LocalStored {
            completion: ProviderIngestCompletionStateV1::Ready {
                last_failure_class: Some(ProviderIngestFailureClassV1::SignerPolicyChanged),
                ..
            },
            ..
        }
    ));
}
#[tokio::test]
async fn policy_rotation_after_durable_begin_never_reaches_ingress_exposure() {
    let row = fixture_row(0x44);
    let (mut runtime, _, _, ingress) = test_runtime(
        row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    *ingress.prepare_error.lock().unwrap() = Some(ProviderIngestIngressPrepareErrorV1::Unavailable);
    let first = runtime
        .tick()
        .await
        .expect("retain signed bytes after unavailable preflight");
    assert_eq!(first.completions_signed, 1);
    assert_eq!(first.completion_submissions, 0);
    let next_attempt_at_ms = match runtime.outbox.status(ingress.job_id).unwrap().state {
        ProviderIngestDeliveryStateV1::LocalStored {
            completion:
                ProviderIngestCompletionStateV1::Signed {
                    next_attempt_at_ms,
                    ever_exposed: false,
                    ..
                },
            ..
        } => next_attempt_at_ms,
        other => panic!("expected a signed retry, got {other:?}"),
    };
    *ingress.prepare_error.lock().unwrap() = None;
    runtime
        .signer_resolver
        .eligibility_flip_on_call
        .store(3, Ordering::SeqCst);
    runtime
        .signer_resolver
        .eligibility_flip_to_revision
        .store(2, Ordering::SeqCst);
    runtime
        .clock
        .base_ms
        .store(next_attempt_at_ms, Ordering::SeqCst);
    let second = runtime
        .tick()
        .await
        .expect("policy loss after durable begin is retryable");
    assert_eq!(second.completions_signed, 0);
    assert_eq!(second.completion_submissions, 0);
    assert_eq!(
        *ingress.events.lock().unwrap(),
        vec!["prepare_signed", "prepare_signed"]
    );
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::LocalStored {
            completion: ProviderIngestCompletionStateV1::Signed {
                ever_exposed: true,
                ..
            },
            ..
        }
    ));
}
#[tokio::test]
async fn mutating_storage_soft_timeout_awaits_late_success_without_retry() {
    let row = fixture_row(0x3E);
    let (mut runtime, _, fetch, ingress) = test_runtime(
        row,
        false,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    let outcome = runtime.tick().await.expect("late atomic store");
    assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
    assert_eq!(outcome.manifests_stored, 1);
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::LocalStored { .. }
    ));
}
#[tokio::test]
async fn newly_admitted_quarantine_is_a_receipt_bound_terminal() {
    let row = fixture_row(0x45);
    let expected_manifest_digest = *row.pin.manifest.digest.as_bytes();
    let (mut runtime, _, fetch, ingress) = test_runtime(
        row,
        false,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    let outcome = runtime.tick().await.expect("quarantine transition");
    assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
    assert_eq!(outcome.manifests_stored, 0);
    assert_eq!(outcome.completions_signed, 0);
    assert_eq!(outcome.completion_submissions, 0);
    assert!(ingress.events.lock().unwrap().is_empty());
    let terminal = runtime
        .outbox
        .status(ingress.job_id)
        .expect("retained quarantine receipt");
    assert_eq!(terminal.job_id, ingress.job_id);
    assert_eq!(terminal.provider_id, LOCAL_PROVIDER);
    assert_eq!(terminal.order_id, [0x45; 32]);
    assert_eq!(terminal.manifest_digest, expected_manifest_digest);
    assert!(matches!(
        terminal.state,
        ProviderIngestDeliveryStateV1::DeadLetter {
            attempts: 1,
            reason: ProviderIngestDeadLetterReasonV1::StorageRejected,
            last_failure_class: ProviderIngestFailureClassV1::StorageRejected,
            observed_finalized_cursor,
        } if observed_finalized_cursor == cursor(8)
    ));
}
#[tokio::test]
async fn wrong_owner_signer_is_released_and_fails_the_supervised_runtime() {
    let row = fixture_row(0x36);
    let (mut runtime, _, _, ingress) = test_runtime(
        row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        true,
    );
    assert!(matches!(
        runtime.tick().await,
        Err(ProviderIngestRuntimeErrorV1::SignerProtocolViolation)
    ));
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::LocalStored {
            completion: ProviderIngestCompletionStateV1::Ready {
                last_failure_class: Some(ProviderIngestFailureClassV1::SignerUnavailable),
                ..
            },
            ..
        }
    ));
}
#[tokio::test]
async fn finalized_expiry_cancels_retained_work_without_fetching() {
    let mut row = fixture_row(0x37);
    let authorization = validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy())
        .unwrap()
        .authorization;
    row.order.status = ReplicationOrderStatus::Expired(8);
    let (mut runtime, _, fetch, ingress) = test_runtime(
        row,
        false,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    assert_eq!(authorization.job_id(), ingress.job_id);
    runtime.tick().await.expect("expiry tick");
    assert_eq!(fetch.calls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        runtime.outbox.status(ingress.job_id).unwrap().state,
        ProviderIngestDeliveryStateV1::Cancelled {
            reason: ProviderIngestCancellationReasonV1::OrderExpired,
            ..
        }
    ));
}
#[tokio::test]
async fn cooperative_shutdown_drains_active_store_before_skipping_next_row() {
    let first = fixture_row(0x3E);
    let second = fixture_row(0x40);
    let second_authorization =
        validate_assignment(&second, cursor(8), LOCAL_PROVIDER, runtime_policy())
            .expect("second assignment")
            .authorization;
    let (mut runtime, ledger, fetch, _) = test_runtime(
        first.clone(),
        false,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    ledger.page.lock().unwrap().rows = vec![first, second];
    let shutdown_requested = AtomicBool::new(false);
    let request_shutdown = async {
        tokio::time::sleep(Duration::from_millis(270)).await;
        shutdown_requested.store(true, Ordering::Release);
    };
    let (result, ()) = tokio::join!(
        runtime.tick_with_shutdown(&shutdown_requested),
        request_shutdown
    );
    let outcome = result.expect("drained cooperative shutdown");
    assert_eq!(outcome.rows_scanned, 1);
    assert_eq!(fetch.calls.load(Ordering::SeqCst), 1);
    assert!(matches!(
        runtime.outbox.status(second_authorization.job_id()),
        Err(ProviderIngestOutboxError::UnknownJob)
    ));
}
#[tokio::test]
async fn pre_signalled_shutdown_returns_without_detaching_work() {
    let row = fixture_row(0x38);
    let (runtime, _, _, _) = test_runtime(
        row,
        true,
        Ok(vec![0xA5]),
        0,
        ProviderIngestIngressDispositionV1::Submitted,
        false,
    );
    let (sender, receiver) = watch::channel(true);
    runtime.run(receiver).await.expect("clean shutdown");
    drop(sender);
}
