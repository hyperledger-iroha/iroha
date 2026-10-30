//! Shared-prefix ownership and local retirement using genuine native certificate fixtures.
//! Execution outputs are synthetic; live monetary qualification remains in integration tests.

use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    block::{
        BlockSignatures, builder::BlockBuilder, execution_output::ExecutionOutputV1,
        output_budget::ExecutionOutputLimits,
    },
    isi::SettlementId,
};
use std::{future::Future, num::NonZeroUsize, task::Poll};

fn cold(native: &NativeFinalityFixture) -> SumeragiFinalityVerifier {
    SumeragiFinalityVerifier::new(
        native.genesis(),
        native.chain_id(),
        native.genesis_proof().committee.clone(),
    )
    .unwrap()
}

fn session(native: &NativeFinalityFixture, capacity: usize) -> NativePaymentSession {
    NativePaymentSession::new(cold(native), NonZeroUsize::new(capacity).unwrap()).unwrap()
}

fn with_session(
    request: NativePaymentRequest,
    transaction: SignedTransaction,
    source: Arc<Source>,
    selected_source: Arc<dyn PaymentSource>,
    session: NativePaymentSession,
    storage: Arc<dyn WorkflowStorage>,
) -> (NativePaymentOperation, Arc<Account>) {
    let driver = Arc::new(Driver {
        transaction: transaction.clone(),
        digest: request.digest,
    });
    let account = Arc::new(Account {
        transaction,
        signs: AtomicUsize::new(0),
        dispatches: AtomicUsize::new(0),
        source,
    });
    let finality = test_finality(&account.source);
    let operation = NativePaymentOperation::create(
        request,
        storage,
        finality,
        session,
        account.clone(),
        account.clone(),
        selected_source,
        driver,
    )
    .unwrap();
    (operation, account)
}

fn storage(root: &std::path::Path, name: &str) -> Arc<FileWorkflowStorage> {
    Arc::new(FileWorkflowStorage::open_or_create(root.join(name)).unwrap())
}

fn allowance(reads: u16) -> NativePaymentRunOptions {
    NativePaymentRunOptions {
        max_finality_proof_reads: NonZeroU16::new(reads).unwrap(),
        ..options()
    }
}

// Extend the original genuine proof chain with a distinct exact payment/fee receipt.
fn next_payment(
    native: &mut NativeFinalityFixture,
    prior: &NativePaymentRequest,
    previous: &Source,
) -> (NativePaymentRequest, SignedTransaction, Arc<Source>) {
    let selected = native
        .verifier()
        .verify_retained_decision(native.latest())
        .unwrap();
    let mut output = selected.block().execution_outputs()[0].clone();
    let header = native.next_header();
    let mut intent = prior.intent.clone();
    intent.settlement_id = SettlementId::new(
        intent.settlement_id.owner().clone(),
        format!("session-payment-{}", header.height())
            .parse()
            .unwrap(),
    );
    let request = NativePaymentRequest::new(intent.clone(), prior.fee_payment.clone()).unwrap();
    let signer = KeyPair::from_seed(vec![41; 32], Algorithm::Ed25519);
    let mut builder = TransactionBuilder::new(
        native.network_id(),
        intent.settlement_id.owner().clone(),
        request.fee_payment.clone(),
    )
    .with_instructions([SettlementInstructionBox::Atomic(intent)]);
    builder.set_creation_time(Duration::from_millis(header.creation_time_ms - 1));
    let transaction = builder.sign(signer.private_key());
    let ExecutionOutputV1::Network(network) = &mut output else {
        panic!("original network output")
    };
    let mut fee = network.result.nexus_fee_receipt().unwrap().clone();
    fee.block_height = header.height().get();
    fee.source_id = *Hash::from(transaction.hash_as_entrypoint()).as_ref();
    fee.schedule.tx_bytes_len = norito::canonical_frame_len(transaction.payload()).unwrap() as u64;
    network.result.set_nexus_fee_receipt(Some(fee));
    let mut builder = BlockBuilder::new(header);
    builder.push_transaction(transaction.clone());
    let mut block = builder.build(BlockSignatures::default());
    block
        .set_execution_outputs(
            vec![output],
            0,
            BTreeMap::default(),
            Vec::new(),
            iroha_data_model::nexus::AxtPolicySnapshot::default(),
            std::collections::BTreeSet::default(),
            &ExecutionOutputLimits {
                max_outputs: 16,
                max_output_bytes: 16 * 1024 * 1024,
                max_total_output_bytes: 32 * 1024 * 1024,
                max_executed_wire_bytes: 32 * 1024 * 1024,
            },
        )
        .unwrap();
    let proof = native.certify(block);
    let mut proofs = previous.proofs.clone();
    proofs.insert(header.height().get(), proof);
    let source = Arc::new(Source {
        finality_root: Arc::new(tempfile::tempdir().unwrap()),
        proofs,
        locator: PaymentLocator {
            height: header.height().get(),
            block_hash: native.latest().block_header.hash(),
        },
        visible: AtomicBool::new(false),
        carrier_height: Some(header.height().get()),
        exact_absence: AtomicBool::new(false),
        fail_read: AtomicBool::new(false),
        proof_reads: AtomicUsize::new(0),
        submitted: AtomicBool::new(false),
    });
    (request, transaction, source)
}

#[tokio::test]
async fn cold_pending_payment_beyond_one_read_allowance_resumes_without_a_new_carrier() {
    let (mut native, mut request, mut transaction, mut source) = fixture(false);
    for _ in 3..=70 {
        (request, transaction, source) = next_payment(&mut native, &request, &source);
    }
    assert_eq!(source.proofs.keys().next_back(), Some(&70));
    let root = tempfile::tempdir().unwrap();
    let original_storage = storage(root.path(), "original");
    let (first, account) = with_session(
        request.clone(),
        transaction.clone(),
        source.clone(),
        source.clone(),
        session(&native, 3),
        original_storage.clone(),
    );
    store_carrier_journal(
        original_storage.as_ref(),
        &first,
        &account,
        &request,
        norito::encode_canonical(transaction.payload()).unwrap(),
        Some(&transaction),
    );
    let workflow = first.id();
    // The final payment is discoverable, but its locator is not settlement evidence.
    // Recovery must authenticate all original certificates under the cold genesis owner.
    source.visible.store(true, Ordering::Relaxed);
    let error = first.recover(&allowance(64)).await.unwrap_err();
    assert!(matches!(
        error,
        NativePaymentError::Observation { cause, .. }
            if matches!(cause.as_ref(), NativePaymentObservationError::ReadBudgetExhausted)
    ));
    assert_eq!(source.proof_reads.load(Ordering::Relaxed), 64);
    assert_eq!(account.signs.load(Ordering::Relaxed), 0);
    assert_eq!(account.dispatches.load(Ordering::Relaxed), 0);
    drop(first);
    drop(account);
    drop(original_storage);

    // All SDK session and workflow owners are cold. Only the exact original durable
    // payment and independently selected genesis may resume; there is no H71 traffic.
    let reopened_storage = storage(root.path(), "original");
    let (reopened, account) = with_session(
        request,
        transaction.clone(),
        source.clone(),
        source.clone(),
        session(&native, 3),
        reopened_storage,
    );
    assert_eq!(reopened.id(), workflow);
    let result = reopened.recover(&allowance(64)).await;
    assert!(
        result.is_ok(),
        "cold pending payment must resume its authenticated prefix beyond one read allowance: {result:?}"
    );
    assert_eq!(
        result.unwrap().receipt.unwrap().transaction_hash(),
        transaction.hash()
    );
    assert!(source.proof_reads.load(Ordering::Relaxed) <= 72);
    assert_eq!(account.signs.load(Ordering::Relaxed), 0);
    assert_eq!(account.dispatches.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn sequential_payments_share_original_prefix_and_retire_only_local_history() {
    let (mut native, first_request, first_tx, first_source) = fixture(false);
    let session = session(&native, 3);
    assert!(
        session
            .verifier()
            .unwrap()
            .oldest_authenticated_height()
            .is_none()
    );
    let owner = Arc::as_ptr(&session.owner);
    let instance = session.verifier().unwrap().instance();
    let root = tempfile::tempdir().unwrap();
    let first_storage = storage(root.path(), "first");
    let (first, first_account) = with_session(
        first_request.clone(),
        first_tx.clone(),
        first_source.clone(),
        first_source.clone(),
        session.clone(),
        first_storage.clone(),
    );
    let result = first.submit(&allowance(2)).await.unwrap();
    assert_eq!(result.receipt.transaction_hash(), first_tx.hash());
    assert_eq!(first_source.proof_reads.load(Ordering::Relaxed), 2);
    let first_journal = first_storage.load(&first.id()).unwrap().unwrap();
    let mut previous = first_source.clone();
    let mut request = first_request.clone();
    for height in 3..=6 {
        let (next_request, transaction, source) = next_payment(&mut native, &request, &previous);
        let (operation, account) = with_session(
            next_request.clone(),
            transaction.clone(),
            source.clone(),
            source.clone(),
            session.clone(),
            storage(root.path(), &format!("payment-{height}")),
        );
        let result = operation.submit(&allowance(1)).await.unwrap();
        assert_eq!(result.receipt.transaction_hash(), transaction.hash());
        assert_eq!(result.receipt.settlement().block_height, height);
        assert_eq!(
            source.proof_reads.load(Ordering::Relaxed),
            1,
            "no old prefix is fetched again"
        );
        assert_eq!(account.signs.load(Ordering::Relaxed), 1);
        assert_eq!(account.dispatches.load(Ordering::Relaxed), 1);
        assert_eq!(Arc::as_ptr(&session.owner), owner);
        {
            let verifier = session.verifier().unwrap();
            assert_eq!(verifier.instance(), instance);
            assert_eq!(verifier.authenticated_height(), Some(height));
            assert_eq!(
                verifier.oldest_authenticated_height(),
                Some(height.saturating_sub(2).max(1))
            );
        }
        if height == 3 {
            let before = first_source.proof_reads.load(Ordering::Relaxed);
            let retained = first.recover(&allowance(1)).await.unwrap();
            assert_eq!(
                retained.receipt.unwrap().transaction_hash(),
                first_tx.hash()
            );
            assert_eq!(first_source.proof_reads.load(Ordering::Relaxed), before);
            assert_eq!(
                first_storage.load(&first.id()).unwrap().unwrap(),
                first_journal
            );
        }
        previous = source;
        request = next_request;
    }
    // Reimport the original authenticated payment pin even after its shared warm parents retire.
    let before = first_source.proof_reads.load(Ordering::Relaxed);
    let recovered = first.recover(&allowance(1)).await.unwrap();
    assert_eq!(
        recovered.receipt.unwrap().transaction_hash(),
        first_tx.hash()
    );
    assert_eq!(first_source.proof_reads.load(Ordering::Relaxed), before);
    assert_eq!(
        first_storage.load(&first.id()).unwrap().unwrap(),
        first_journal
    );
    assert_eq!(first_account.signs.load(Ordering::Relaxed), 1);
    assert_eq!(first_account.dispatches.load(Ordering::Relaxed), 1);
    // Cold recovery uses the SAME original request, journal and carrier; it signs nothing.
    let (recovered, account) = with_session(
        first_request,
        first_tx.clone(),
        first_source.clone(),
        first_source.clone(),
        self::session(&native, 3),
        first_storage,
    );
    let result = recovered.recover(&allowance(2)).await.unwrap();
    assert_eq!(result.receipt.unwrap().transaction_hash(), first_tx.hash());
    assert_eq!(account.signs.load(Ordering::Relaxed), 0);
    assert_eq!(account.dispatches.load(Ordering::Relaxed), 0);
    assert_eq!(first_source.proof_reads.load(Ordering::Relaxed), before);
}

struct GatedSource {
    original: Arc<Source>,
    height: u64,
    blocked: AtomicBool,
    entered: tokio::sync::Notify,
    released: tokio::sync::Notify,
    reads: Mutex<Vec<u64>>,
}
impl GatedSource {
    fn new(original: Arc<Source>, height: u64) -> Self {
        Self {
            original,
            height,
            blocked: AtomicBool::new(true),
            entered: tokio::sync::Notify::new(),
            released: tokio::sync::Notify::new(),
            reads: Mutex::new(Vec::new()),
        }
    }
    fn release(&self) {
        self.blocked.store(false, Ordering::Relaxed);
        self.released.notify_one();
    }
}
impl PaymentSource for GatedSource {
    fn settlement<'a>(
        &'a self,
        id: &'a SettlementId,
    ) -> WorkflowFuture<'a, Result<Option<PaymentLocator>, NativePaymentObservationError>> {
        self.original.settlement(id)
    }
    fn carrier(
        &self,
        hash: HashOf<SignedTransaction>,
    ) -> WorkflowFuture<'_, Result<CarrierLocation, NativePaymentObservationError>> {
        self.original.carrier(hash)
    }
    fn proof(
        &self,
        height: NonZeroU64,
    ) -> WorkflowFuture<'_, Result<PaymentProof, NativePaymentObservationError>> {
        Box::pin(async move {
            self.reads.lock().unwrap().push(height.get());
            if height.get() == self.height && self.blocked.load(Ordering::Relaxed) {
                self.entered.notify_one();
                self.released.notified().await;
            }
            self.original.proof(height).await
        })
    }
}

#[tokio::test]
async fn concurrent_operations_serialize_original_catch_up_and_keep_separate_read_budgets() {
    let (mut native, request, transaction, source) = fixture(false);
    let (next_request, next_tx, next_source) = next_payment(&mut native, &request, &source);
    let session = session(&native, 3);
    let cloned = session.clone();
    assert!(Arc::ptr_eq(&session.owner, &cloned.owner));
    let root = tempfile::tempdir().unwrap();
    let gated = Arc::new(GatedSource::new(source.clone(), 2));
    let (first, _) = with_session(
        request,
        transaction,
        source.clone(),
        gated.clone(),
        session.clone(),
        storage(root.path(), "first"),
    );
    let (second, _) = with_session(
        next_request,
        next_tx,
        next_source.clone(),
        next_source.clone(),
        cloned,
        storage(root.path(), "second"),
    );
    let first_options = allowance(2);
    let second_options = allowance(1);
    let mut first_call = Box::pin(first.submit(&first_options));
    tokio::select! {
        result = &mut first_call => panic!("first must await original H2 proof: {result:?}"),
        () = gated.entered.notified() => {},
    }
    assert_eq!(session.verifier().unwrap().authenticated_height(), Some(1));
    let mut second_call = Box::pin(second.submit(&second_options));
    assert!(
        std::future::poll_fn(|cx| Poll::Ready(second_call.as_mut().poll(cx).is_pending())).await
    );
    assert_eq!(
        next_source.proof_reads.load(Ordering::Relaxed),
        0,
        "second operation cannot select a stale next height"
    );
    gated.release();
    let (first_result, second_result) = tokio::join!(first_call, second_call);
    assert_eq!(first_result.unwrap().receipt.settlement().block_height, 2);
    assert_eq!(second_result.unwrap().receipt.settlement().block_height, 3);
    assert_eq!(*gated.reads.lock().unwrap(), [1, 2]);
    assert_eq!(source.proof_reads.load(Ordering::Relaxed), 2);
    assert_eq!(next_source.proof_reads.load(Ordering::Relaxed), 1);
    assert_eq!(session.verifier().unwrap().authenticated_height(), Some(3));
}

#[tokio::test]
async fn cancelled_observation_keeps_authenticated_prefix_and_resumes_original_carrier() {
    let (native, request, transaction, source) = fixture(false);
    let session = session(&native, 3);
    let root = tempfile::tempdir().unwrap();
    let store = storage(root.path(), "original");
    let gated = Arc::new(GatedSource::new(source.clone(), 2));
    let (operation, account) = with_session(
        request,
        transaction.clone(),
        source.clone(),
        gated.clone(),
        session.clone(),
        store.clone(),
    );
    let original_options = allowance(2);
    let mut pending = Box::pin(operation.submit(&original_options));
    tokio::select! {
        result = &mut pending => panic!("must await original H2 proof: {result:?}"),
        () = gated.entered.notified() => {},
    }
    assert_eq!(session.verifier().unwrap().authenticated_height(), Some(1));
    let journal = operation.handle.journal_snapshot().unwrap().unwrap();
    assert!(journal.native_payment_completion.is_none());
    let original_identity = journal.steps[0]
        .transaction
        .as_ref()
        .unwrap()
        .attempts
        .last()
        .unwrap()
        .identity;
    assert_eq!(original_identity, Some(identity(&transaction)));
    drop(pending);
    assert!(session.owner.catch_up.try_lock().is_ok());
    assert_eq!(session.verifier().unwrap().authenticated_height(), Some(1));
    gated.release();
    let result = operation.resume(&allowance(1)).await.unwrap();
    assert_eq!(result.receipt.transaction_hash(), transaction.hash());
    assert_eq!(account.signs.load(Ordering::Relaxed), 1);
    assert_eq!(account.dispatches.load(Ordering::Relaxed), 1);
    assert_eq!(*gated.reads.lock().unwrap(), [1, 2, 2]);
    assert_eq!(session.verifier().unwrap().authenticated_height(), Some(2));
}

struct UnusedStorage;
impl WorkflowStorage for UnusedStorage {
    fn durability(&self) -> WorkflowStorageDurability {
        panic!("binding before storage")
    }
    fn load(&self, _: &WorkflowId) -> Result<Option<Vec<u8>>, WorkflowStorageError> {
        panic!("binding before journal read")
    }
    fn compare_and_store(
        &self,
        _: &WorkflowId,
        _: Option<&Digest32>,
        _: &[u8],
    ) -> Result<(), WorkflowStorageError> {
        panic!("binding before journal mutation")
    }
    fn acquire_runner(
        &self,
        _: &WorkflowId,
    ) -> Result<Box<dyn WorkflowRunnerLease>, WorkflowStorageError> {
        panic!("binding before runner acquisition")
    }
}

#[test]
fn session_rejects_invalid_capacity_and_foreign_root_network_chain_or_payer_before_storage() {
    let (native, request, transaction, _) = fixture(false);
    for capacity in 1..3 {
        assert!(matches!(
            NativePaymentSession::new(cold(&native), NonZeroUsize::new(capacity).unwrap()),
            Err(NativePaymentError::InvalidRequest(_))
        ));
    }
    let private = NativeFinalityFixture::start_with_scope(
        "private-session",
        SumeragiRootScope::Dataspace {
            parent_network_id: native.network_id(),
            dataspace_id: iroha_model_base::topology::DataSpaceId::new(7),
        },
    );
    assert!(matches!(
        NativePaymentSession::new(cold(&private), NonZeroUsize::new(3).unwrap()),
        Err(NativePaymentError::InvalidRequest(_))
    ));
    for mutation in ["network", "chain", "payer", "same-label-foreign-genesis"] {
        let mut client = crate::client::evidence_http_tests::client_with_base_url(
            "http://mock.local/".parse().unwrap(),
        );
        client.network_id = native.network_id();
        client.chain = native.chain_id().parse().unwrap();
        client.account = transaction.authority().clone();
        client.key_pair = KeyPair::from_seed(vec![41; 32], Algorithm::Ed25519);
        let selected = match mutation {
            "same-label-foreign-genesis" => {
                let foreign = NativeFinalityFixture::start_with_genesis_extension(
                    native.chain_id(),
                    [9; 32],
                    Vec::new(),
                );
                assert_ne!(foreign.network_id(), native.network_id());
                session(&foreign, 3)
            }
            _ => session(&native, 3),
        };
        match mutation {
            "network" => client.network_id = crate::client::test_network_id(),
            "chain" => client.chain = "foreign-chain".into(),
            "payer" => {
                client.key_pair = KeyPair::from_seed(vec![42; 32], Algorithm::Ed25519);
                client.account = request.intent.movements.as_slice()[0].recipient.clone();
            }
            _ => {}
        }
        let account = client.account_client().unwrap();
        let refused = account.native_payment_with_session(
            request.clone(),
            Arc::new(UnusedStorage),
            test_finality(&fixture(false).3),
            selected.clone(),
        );
        assert!(
            matches!(refused, Err(NativePaymentError::InvalidRequest(_))),
            "{mutation}"
        );
        assert_eq!(selected.verifier().unwrap().authenticated_height(), None);
    }
}

#[tokio::test]
async fn unresolved_payment_keeps_original_durable_progress_after_shared_cache_pruning() {
    let (mut native, request, transaction, source) = fixture(false);
    let session = session(&native, 3);
    let root = tempfile::tempdir().unwrap();
    let original_storage = storage(root.path(), "unresolved");
    let (original, account) = with_session(
        request.clone(),
        transaction.clone(),
        source.clone(),
        source.clone(),
        session.clone(),
        original_storage.clone(),
    );
    let error = original.submit(&allowance(1)).await.unwrap_err();
    let NativePaymentError::Observation { cause, .. } = error else {
        panic!("original read allowance")
    };
    assert!(matches!(
        cause.as_ref(),
        NativePaymentObservationError::ReadBudgetExhausted
    ));
    assert_eq!(source.proof_reads.load(Ordering::Relaxed), 1);
    assert_eq!(session.verifier().unwrap().authenticated_height(), Some(1));
    let journal = original.handle.journal_snapshot().unwrap().unwrap();
    assert!(journal.native_payment_completion.is_none());
    let attempt = journal.steps[0]
        .transaction
        .as_ref()
        .unwrap()
        .attempts
        .last()
        .unwrap();
    assert_eq!(attempt.identity.as_ref(), Some(&identity(&transaction)));
    assert_eq!(account.signs.load(Ordering::Relaxed), 1);
    assert_eq!(account.dispatches.load(Ordering::Relaxed), 1);
    let mut previous = source.clone();
    let mut previous_request = request.clone();
    for height in 3..=5 {
        let (next_request, next_tx, next_source) =
            next_payment(&mut native, &previous_request, &previous);
        let (next, _) = with_session(
            next_request.clone(),
            next_tx,
            next_source.clone(),
            next_source.clone(),
            session.clone(),
            storage(root.path(), &format!("next-{height}")),
        );
        next.submit(&allowance(if height == 3 { 2 } else { 1 }))
            .await
            .unwrap();
        previous = next_source;
        previous_request = next_request;
    }
    source.visible.store(true, Ordering::Relaxed); // Only an unauthenticated original-receipt locator.
    let reads = source.proof_reads.load(Ordering::Relaxed);
    let recovered = original.resume(&allowance(1)).await.unwrap();
    assert_eq!(recovered.receipt.transaction_hash(), transaction.hash());
    assert_eq!(source.proof_reads.load(Ordering::Relaxed), reads + 1);
    let completed = original_storage.load(&original.id()).unwrap().unwrap();
    assert_eq!(account.signs.load(Ordering::Relaxed), 1);
    assert_eq!(account.dispatches.load(Ordering::Relaxed), 1);
    let (cold_original, cold_account) = with_session(
        request,
        transaction.clone(),
        source.clone(),
        source.clone(),
        self::session(&native, 3),
        original_storage.clone(),
    );
    let recovered = cold_original.resume(&allowance(2)).await.unwrap();
    assert_eq!(recovered.receipt.transaction_hash(), transaction.hash());
    assert_eq!(source.proof_reads.load(Ordering::Relaxed), reads + 1);
    assert_eq!(
        original_storage.load(&cold_original.id()).unwrap().unwrap(),
        completed
    );
    assert_eq!(cold_account.signs.load(Ordering::Relaxed), 0);
    assert_eq!(cold_account.dispatches.load(Ordering::Relaxed), 0);
}

struct ChangedLocatorSource {
    original: Arc<Source>,
    locator: Mutex<PaymentLocator>,
    refuse_publication: AtomicBool,
}
impl PaymentSource for ChangedLocatorSource {
    fn settlement<'a>(
        &'a self,
        _: &'a SettlementId,
    ) -> WorkflowFuture<'a, Result<Option<PaymentLocator>, NativePaymentObservationError>> {
        Box::pin(async { Ok(Some(*self.locator.lock().unwrap())) })
    }
    fn carrier(
        &self,
        hash: HashOf<SignedTransaction>,
    ) -> WorkflowFuture<'_, Result<CarrierLocation, NativePaymentObservationError>> {
        self.original.carrier(hash)
    }
    fn proof(
        &self,
        height: NonZeroU64,
    ) -> WorkflowFuture<'_, Result<PaymentProof, NativePaymentObservationError>> {
        Box::pin(async move {
            let proof = self.original.proof(height).await?;
            if self.refuse_publication.swap(false, Ordering::Relaxed) {
                std::fs::create_dir(
                    self.original
                        .finality_root
                        .path()
                        .join("pending/native-payment.finality"),
                )
                .unwrap();
            }
            Ok(proof)
        })
    }
}

#[tokio::test]
async fn corrected_lower_payment_locator_replays_original_genesis_after_cold_restart() {
    let (mut native, request, transaction, original) = fixture(false);
    let original_locator = original.locator;
    let mut latest_request = request.clone();
    let mut source = original;
    for _ in 3..=70 {
        let (next, _, advanced) = next_payment(&mut native, &latest_request, &source);
        latest_request = next;
        source = advanced;
    }
    let locator = Arc::new(ChangedLocatorSource {
        locator: Mutex::new(source.locator),
        original: source.clone(),
        refuse_publication: AtomicBool::new(false),
    });
    let root = tempfile::tempdir().unwrap();
    let original_storage = storage(root.path(), "original");
    let (first, account) = with_session(
        request.clone(),
        transaction.clone(),
        source.clone(),
        locator.clone(),
        session(&native, 3),
        original_storage.clone(),
    );
    assert!(
        matches!(first.recover(&allowance(64)).await, Err(NativePaymentError::Observation { cause, .. }) if matches!(cause.as_ref(), NativePaymentObservationError::ReadBudgetExhausted))
    );
    assert_eq!(source.proof_reads.load(Ordering::Relaxed), 64);
    assert_eq!(account.signs.load(Ordering::Relaxed), 0);
    assert_eq!(account.dispatches.load(Ordering::Relaxed), 0);
    drop(first);
    drop(account);
    drop(original_storage);
    *locator.locator.lock().unwrap() = original_locator;
    let (reopened, account) = with_session(
        request,
        transaction.clone(),
        source.clone(),
        locator,
        session(&native, 3),
        storage(root.path(), "original"),
    );
    let recovered = reopened.recover(&allowance(2)).await.unwrap();
    assert_eq!(
        recovered.receipt.unwrap().transaction_hash(),
        transaction.hash()
    );
    assert_eq!(source.proof_reads.load(Ordering::Relaxed), 66);
    assert_eq!(account.signs.load(Ordering::Relaxed), 0);
    assert_eq!(account.dispatches.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn refused_pending_publication_does_not_advance_shared_cache_and_same_original_retries() {
    let (native, request, transaction, source) = fixture(false);
    let selected = session(&native, 3);
    let refusing = Arc::new(ChangedLocatorSource {
        locator: Mutex::new(source.locator),
        original: source.clone(),
        refuse_publication: AtomicBool::new(true),
    });
    let root = tempfile::tempdir().unwrap();
    let (operation, account) = with_session(
        request,
        transaction.clone(),
        source.clone(),
        refusing,
        selected.clone(),
        storage(root.path(), "original"),
    );
    assert!(
        matches!(operation.recover(&allowance(2)).await, Err(NativePaymentError::Observation { cause, .. }) if matches!(cause.as_ref(), NativePaymentObservationError::FinalityCustody(_)))
    );
    assert_eq!(selected.verifier().unwrap().authenticated_height(), None);
    std::fs::remove_dir(
        source
            .finality_root
            .path()
            .join("pending/native-payment.finality"),
    )
    .unwrap();
    let recovered = operation.recover(&allowance(2)).await.unwrap();
    assert_eq!(
        recovered.receipt.unwrap().transaction_hash(),
        transaction.hash()
    );
    assert_eq!(source.proof_reads.load(Ordering::Relaxed), 3);
    assert_eq!(account.signs.load(Ordering::Relaxed), 0);
    assert_eq!(account.dispatches.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn unrelated_authenticated_carrier_pin_refuses_even_with_original_success_pin() {
    use super::super::finality_custody::{
        PendingBusinessPin, PendingCarrierPin, PendingFinalityRecord,
    };
    let (mut native, request, transaction, source) = fixture(false);
    let original_checkpoint = native.checkpoint();
    let (_, foreign, _) = next_payment(&mut native, &request, &source);
    let root = tempfile::tempdir().unwrap();
    let (operation, account) = with_session(
        request.clone(),
        transaction.clone(),
        source.clone(),
        source.clone(),
        session(&native, 3),
        storage(root.path(), "original"),
    );
    let owner = test_finality(&source);
    let bound = owner
        .bind(operation.id(), request.digest, &native.verifier())
        .unwrap();
    bound
        .compare_and_publish(
            None,
            &PendingFinalityRecord {
                prefix: None,
                carrier: Some(PendingCarrierPin {
                    identity: identity(&foreign),
                    checkpoint: native.checkpoint(),
                    replay: None,
                }),
                business: Some(PendingBusinessPin {
                    identity: identity(&transaction),
                    checkpoint: original_checkpoint,
                }),
            },
        )
        .unwrap();
    assert!(
        matches!(operation.recover(&allowance(2)).await, Err(NativePaymentError::Observation { cause, .. }) if matches!(cause.as_ref(), NativePaymentObservationError::CandidateMismatch))
    );
    assert_eq!(source.proof_reads.load(Ordering::Relaxed), 0);
    assert_eq!(account.signs.load(Ordering::Relaxed), 0);
    assert_eq!(account.dispatches.load(Ordering::Relaxed), 0);
}
