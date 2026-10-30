//! Native QC tests over synthetic execution; no World or monetary qualification is inferred.

use super::super::super::tests::{bundle, checkpoint, prepared};
use super::*;
use crate::{
    client::evidence_http_tests::{base_url, client_with_base_url, with_mock_http},
    http::Response,
    workflow::FileWorkflowStorage,
};
use iroha_data_model::{
    block::{
        BlockSignatures,
        builder::BlockBuilder,
        execution_output::{ExecutionOutputV1, NetworkExecutionOutputV1},
        output_budget::ExecutionOutputLimits,
    },
    nexus::{AxtPolicySnapshot, PrivateSettlementAbortReceiptV1},
    sumeragi_finality::{SumeragiFinalityProof, test_fixtures::NativeFinalityFixture},
    transaction::{
        TransactionBuilder, TransactionResult,
        error::{TransactionLimitError, TransactionRejectionReason},
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

struct Fixture {
    directory: tempfile::TempDir,
    client: Client,
    request: PrivateSettlementRequest,
    barrier: PrivateSettlementPrepareBarrierV1,
    selected: SumeragiFinalityVerifier,
    register: TransactionIdentityV1,
    success: TransactionIdentityV1,
    same_block_rejection: TransactionIdentityV1,
    later_rejection: TransactionIdentityV1,
    reads: Arc<Mutex<Vec<u64>>>,
}

fn carrier(
    native: &mut NativeFinalityFixture,
    client: &Client,
    instructions: Vec<(InstructionBox, bool)>,
    fee: &iroha_data_model::transaction::FeePaymentIntent,
) -> (SumeragiFinalityProof, Vec<TransactionIdentityV1>) {
    let header = native.next_header();
    let mut builder = BlockBuilder::new(header);
    let mut identities = Vec::new();
    let mut outputs = Vec::new();
    for (index, (instruction, rejected)) in instructions.into_iter().enumerate() {
        let mut transaction =
            TransactionBuilder::new(client.network_id, client.account.clone(), fee.clone())
                .with_instructions([instruction]);
        transaction.set_creation_time(Duration::from_millis(
            header.creation_time_ms - 1 - index as u64,
        ));
        let transaction = transaction.sign(client.key_pair.private_key());
        identities.push(super::super::super::identity(&transaction));
        builder.push_transaction(transaction);
        outputs.push(ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
            input_index: index as u32,
            result: TransactionResult::new(if rejected {
                Err(TransactionRejectionReason::LimitCheck(
                    TransactionLimitError {
                        reason: "synthetic rejected carrier".into(),
                    },
                ))
            } else {
                Ok(Vec::new())
            }),
            completions: Vec::new(),
        }));
    }
    let mut block = builder.build(BlockSignatures::default());
    block
        .set_execution_outputs(
            outputs,
            0,
            BTreeMap::new(),
            Vec::new(),
            AxtPolicySnapshot::default(),
            BTreeSet::new(),
            &ExecutionOutputLimits {
                max_outputs: 16,
                max_output_bytes: 16 * 1024 * 1024,
                max_total_output_bytes: 32 * 1024 * 1024,
                max_executed_wire_bytes: 32 * 1024 * 1024,
            },
        )
        .unwrap();
    (native.certify(block), identities)
}

impl Fixture {
    fn new(success_height: u64) -> Self {
        Self::with_prepare_quorum(success_height, false)
    }
    fn with_prepare_quorum(success_height: u64, alternate: bool) -> Self {
        let mut native = NativeFinalityFixture::start("private-settlement-cold-phase-custody");
        let selected = native.verifier().into_genesis_verifier();
        let mut client = client_with_base_url(base_url());
        client.network_id = native.network_id();
        client.chain = native.chain_id().parse().unwrap();
        let (request, certificates) = prepared(&client);
        let journal = checkpoint(&request, &certificates);
        let (barrier, mut bundle) = bundle(&request, &journal);
        if alternate {
            for (index, leg) in bundle.legs.iter_mut().enumerate() {
                let (authority, keys) =
                    iroha_test_samples::private_settlement::finalization_authority_v1(
                        request.manifest().legs[index].route,
                        index as u8,
                    );
                let votes = super::super::super::super::tests::phase_votes_v1(
                    &authority,
                    &keys,
                    &leg.prepare.body,
                    &[1, 2, 3],
                );
                leg.prepare =
                    aggregate_phase_votes_v1(&leg.prepare.body, index as u8, &authority, &votes)
                        .unwrap();
                assert_ne!(leg.prepare, barrier.prepare_certificates[index]);
            }
        }
        let mut proofs = BTreeMap::from([(1, native.genesis_proof().clone())]);
        let (proof, identities) = carrier(
            &mut native,
            &client,
            vec![(
                RegisterAtomicPrivateSettlementPrepareV1::new(barrier.clone()).into(),
                false,
            )],
            &barrier.manifest.public_fee_intent,
        );
        proofs.insert(proof.height(), proof);
        let register = identities[0];
        while native.latest().height() + 1 < success_height {
            let block = native.block_with_submitted_work(native.next_header());
            let proof = native.certify(block);
            proofs.insert(proof.height(), proof);
        }
        let (proof, identities) = carrier(
            &mut native,
            &client,
            vec![
                (
                    FinalizeAtomicPrivateSettlementV1::new(bundle.clone()).into(),
                    false,
                ),
                (
                    FinalizeAtomicPrivateSettlementV1::new(bundle.clone()).into(),
                    true,
                ),
            ],
            &barrier.manifest.public_fee_intent,
        );
        proofs.insert(proof.height(), proof);
        let success = identities[0];
        let same_block_rejection = identities[1];
        let (proof, identities) = carrier(
            &mut native,
            &client,
            vec![(FinalizeAtomicPrivateSettlementV1::new(bundle).into(), true)],
            &barrier.manifest.public_fee_intent,
        );
        proofs.insert(proof.height(), proof);
        let later_rejection = identities[0];
        let reads = Arc::new(Mutex::new(Vec::new()));
        let observed = reads.clone();
        let transport = with_mock_http(
            move |http| {
                let height = http.url.path().rsplit('/').next().unwrap().parse::<u64>()?;
                observed.lock().unwrap().push(height);
                Ok(Response::builder()
                    .status(200)
                    .header("content-type", "application/x-norito")
                    .body(norito::encode_canonical(&proofs[&height])?)?)
            },
            |transport| transport,
        );
        let client = client.with_test_http_transport(transport);
        crate::client::evidence_http_tests::mark_data_model_compatible(&client);
        Self {
            directory: tempfile::tempdir().unwrap(),
            client,
            request,
            barrier,
            selected,
            register,
            success,
            same_block_rejection,
            later_rejection,
            reads,
        }
    }
    fn ledger(&self, maximum: usize) -> SettlementLedger {
        let account = self.client.account_client().unwrap();
        let storage = Arc::new(
            FileWorkflowStorage::open_or_create(self.directory.path().join("journal")).unwrap(),
        );
        let owner = Arc::new(
            FilePrivateSettlementFinality::open(
                self.directory.path().join("finality"),
                &[57; 32],
                maximum,
            )
            .unwrap(),
        );
        let binding = AccountWorkflowContext::new(account.clone())
            .unwrap()
            .binding();
        let workflow = WorkflowIdentityV1::new(&self.request.plan(), &binding)
            .unwrap()
            .workflow_id()
            .unwrap();
        let finality = owner
            .bind(workflow, self.request.0.digest, &self.selected)
            .unwrap();
        SettlementLedger::new(
            account,
            self.request.clone(),
            Arc::new(Mutex::new(Some(PublicContext::prepared(
                self.barrier.clone(),
                FINALIZE,
            )))),
            self.selected.clone(),
            Arc::new(tokio::sync::Semaphore::new(1)),
            storage,
            finality,
        )
    }
    fn reopen(&self) -> SettlementLedger {
        self.ledger(MAX_PRIVATE_SETTLEMENT_FINALITY_BYTES)
    }
}

async fn found(
    ledger: &SettlementLedger,
    phase: FinalityPhase,
    identity: TransactionIdentityV1,
    height: u64,
) -> (VerifiedSumeragiBlock, CommittedTransaction) {
    ledger
        .finality_located(phase, Some(identity), Some(height))
        .await
        .unwrap()
        .found()
        .unwrap()
}

#[tokio::test]
async fn cold_prefix_exceeds_one_call_budget_and_keeps_register_beyond_retention() {
    let fixture = Fixture::new(70);
    let ledger = fixture.reopen();
    ledger.begin_call(64).unwrap();
    assert!(
        found(&ledger, FinalityPhase::Register, fixture.register, 2)
            .await
            .1
            .result()
            .is_ok()
    );
    drop(ledger);
    let ledger = fixture.reopen();
    ledger.begin_call(64).unwrap();
    assert!(matches!(
        ledger
            .finality_located(FinalityPhase::Finalize, Some(fixture.success), Some(70))
            .await,
        Err(PrivateSettlementError::ReadBudget)
    ));
    assert_eq!(
        ledger
            .finality
            .load()
            .unwrap()
            .record
            .prefix
            .unwrap()
            .checkpoint
            .height(),
        66
    );
    drop(ledger);
    let ledger = fixture.reopen();
    ledger.begin_call(4).unwrap();
    let (block, execution) = found(&ledger, FinalityPhase::Finalize, fixture.success, 70).await;
    assert_eq!(block.height(), 70);
    assert!(execution.result().is_ok());
    drop(ledger);
    let ledger = fixture.reopen();
    ledger.begin_call(0).unwrap();
    assert_eq!(
        found(&ledger, FinalityPhase::Register, fixture.register, 2)
            .await
            .0
            .height(),
        2
    );
    let saved = ledger
        .finality_located(FinalityPhase::Finalize, None, None)
        .await
        .unwrap()
        .found()
        .unwrap();
    assert_eq!(saved.0.height(), 70);
    assert_eq!(*fixture.reads.lock().unwrap(), (1..=70).collect::<Vec<_>>());
}

#[tokio::test]
async fn corrected_earlier_locator_resets_pending_prefix_under_original_genesis() {
    let fixture = Fixture::new(7);
    let ledger = fixture.reopen();
    ledger.begin_call(8).unwrap();
    assert!(matches!(
        ledger
            .finality_located(FinalityPhase::Finalize, Some(fixture.success), Some(8))
            .await,
        Err(PrivateSettlementError::Binding)
    ));
    assert!(ledger.finality.load().unwrap().record.terminal.is_none());
    drop(ledger);
    for expected in [3, 6] {
        let ledger = fixture.reopen();
        ledger.begin_call(3).unwrap();
        assert!(matches!(
            ledger
                .finality_located(FinalityPhase::Finalize, Some(fixture.success), Some(7))
                .await,
            Err(PrivateSettlementError::ReadBudget)
        ));
        assert_eq!(
            ledger
                .finality
                .load()
                .unwrap()
                .record
                .prefix
                .unwrap()
                .checkpoint
                .height(),
            expected
        );
    }
    let ledger = fixture.reopen();
    ledger.begin_call(1).unwrap();
    assert!(
        found(&ledger, FinalityPhase::Finalize, fixture.success, 7)
            .await
            .1
            .result()
            .is_ok()
    );
}

#[tokio::test]
async fn rejected_later_or_same_block_carrier_preserves_original_success() {
    for rejected_first in [false, true] {
        let fixture = Fixture::new(3);
        let ledger = fixture.reopen();
        ledger.begin_call(12).unwrap();
        if rejected_first {
            assert!(
                found(&ledger, FinalityPhase::Finalize, fixture.later_rejection, 4)
                    .await
                    .1
                    .result()
                    .is_err()
            );
            drop(ledger);
            let ledger = fixture.reopen();
            ledger.begin_call(3).unwrap();
            assert!(
                found(&ledger, FinalityPhase::Finalize, fixture.success, 3)
                    .await
                    .1
                    .result()
                    .is_ok()
            );
        } else {
            assert!(
                found(&ledger, FinalityPhase::Finalize, fixture.success, 3)
                    .await
                    .1
                    .result()
                    .is_ok()
            );
        }
        for (identity, height) in [
            (fixture.same_block_rejection, 3),
            (fixture.later_rejection, 4),
        ] {
            let ledger = fixture.reopen();
            ledger.begin_call(4).unwrap();
            assert!(
                found(&ledger, FinalityPhase::Finalize, identity, height)
                    .await
                    .1
                    .result()
                    .is_err()
            );
            assert_eq!(
                ledger
                    .finality
                    .load()
                    .unwrap()
                    .record
                    .terminal
                    .unwrap()
                    .pin
                    .identity,
                fixture.success
            );
        }
        let ledger = fixture.reopen();
        ledger.begin_call(0).unwrap();
        let (block, evidence) = ledger
            .finality_located(FinalityPhase::Finalize, None, None)
            .await
            .unwrap()
            .found()
            .unwrap();
        assert_eq!(block.height(), 3);
        assert!(evidence.result().is_ok());
    }
}

#[tokio::test]
async fn substituted_saved_phase_pin_refuses_even_when_other_slot_answers() {
    let fixture = Fixture::new(3);
    let ledger = fixture.reopen();
    ledger.begin_call(3).unwrap();
    found(&ledger, FinalityPhase::Register, fixture.register, 2).await;
    found(&ledger, FinalityPhase::Finalize, fixture.success, 3).await;
    let mut saved = ledger.finality.load().unwrap();
    saved.record.register.as_mut().unwrap().identity = fixture.success;
    ledger
        .finality
        .compare_and_publish(saved.digest.as_ref(), &saved.record)
        .unwrap();
    drop(ledger);
    let ledger = fixture.reopen();
    ledger.begin_call(0).unwrap();
    assert!(matches!(
        ledger
            .finality_located(FinalityPhase::Finalize, None, None)
            .await,
        Err(PrivateSettlementError::Binding)
    ));
}

#[tokio::test]
async fn publication_capacity_refusal_does_not_install_unpublished_progress() {
    let fixture = Fixture::new(3);
    let ledger = fixture.ledger(1);
    ledger.begin_call(3).unwrap();
    assert!(matches!(
        ledger
            .finality_located(FinalityPhase::Register, Some(fixture.register), Some(2))
            .await,
        Err(PrivateSettlementError::FinalityStorage(
            PrivateSettlementFinalityStorageError::Capacity
        ))
    ));
    assert_eq!(lock(&ledger.verifier).unwrap().authenticated_height(), None);
    assert!(ledger.finality.load().unwrap().record.prefix.is_none());
    drop(ledger);
    let ledger = fixture.reopen();
    ledger.begin_call(3).unwrap();
    assert!(
        found(&ledger, FinalityPhase::Register, fixture.register, 2)
            .await
            .1
            .result()
            .is_ok()
    );
}

#[tokio::test]
async fn recovered_success_retains_valid_alternate_prepare_quorum_across_restart() {
    let fixture = Fixture::with_prepare_quorum(3, true);
    let ledger = fixture.reopen();
    ledger.begin_call(3).unwrap();
    let (block, evidence) = ledger
        .finality_located(FinalityPhase::Finalize, None, Some(3))
        .await
        .unwrap()
        .found()
        .unwrap();
    let receipt =
        VerifiedPrivateSettlementReceipt::verify_recovered(&block, &fixture.barrier, evidence)
            .unwrap();
    assert_ne!(
        receipt.receipt().legs[0].prepare,
        fixture.barrier.prepare_certificates[0]
    );
    drop(ledger);
    let ledger = fixture.reopen();
    ledger.begin_call(0).unwrap();
    assert!(matches!(
        ledger.outcome(&fixture.success).await.unwrap(),
        CommittedOutcome::Applied {
            block_height: Some(3)
        }
    ));
    assert_eq!(fixture.reads.lock().unwrap().len(), 3);
}

#[tokio::test]
async fn prepared_local_carrier_limit_preserves_original_custody_and_phase_lease() {
    let fixture = Fixture::new(3);
    let restricted = PrivateSettlementRequest::new(
        fixture.request.0.materials.clone(),
        fixture.request.0.endpoints.clone(),
        1,
    )
    .unwrap();
    assert_eq!(restricted.0.digest, fixture.request.0.digest);
    assert_eq!(
        norito::encode_canonical(&restricted.plan()).unwrap(),
        norito::encode_canonical(&fixture.request.plan()).unwrap()
    );
    let ledger = fixture.reopen();
    ledger.begin_call(2).unwrap();
    found(&ledger, FinalityPhase::Register, fixture.register, 2).await;
    let owner = Arc::new(
        FilePrivateSettlementFinality::open(
            fixture.directory.path().join("finality"),
            &[57; 32],
            MAX_PRIVATE_SETTLEMENT_FINALITY_BYTES,
        )
        .unwrap(),
    );
    let account = fixture.client.account_client().unwrap();
    let binding = AccountWorkflowContext::new(account.clone())
        .unwrap()
        .binding();
    let id = |request: &PrivateSettlementRequest| {
        WorkflowIdentityV1::new(&request.plan(), &binding)
            .unwrap()
            .workflow_id()
            .unwrap()
    };
    assert_eq!(id(&restricted), id(&fixture.request));
    let restored = SettlementLedger::new(
        account,
        restricted.clone(),
        ledger.state.clone(),
        fixture.selected.clone(),
        Arc::new(tokio::sync::Semaphore::new(1)),
        ledger.storage.clone(),
        owner
            .bind(id(&restricted), restricted.0.digest, &fixture.selected)
            .unwrap(),
    );
    assert_eq!(restored.finality_lease, ledger.finality_lease);
    restored.begin_call(0).unwrap();
    // Carrier limits govern new effects. A smaller local limit cannot erase independently
    // authenticated original receipt/custody; restore with sufficient capacity before reading.
    let lease = ledger
        .storage
        .acquire_runner(&ledger.finality_lease)
        .unwrap();
    assert!(matches!(
        restored
            .finality_located(FinalityPhase::Register, Some(fixture.register), Some(2))
            .await,
        Err(PrivateSettlementError::Custody(_))
    ));
    drop(lease);
    drop(restored);
    let restored = fixture.reopen();
    restored.begin_call(0).unwrap();
    assert!(
        found(&restored, FinalityPhase::Register, fixture.register, 2)
            .await
            .1
            .result()
            .is_ok()
    );
}

// Native CommitQCs and signed carriers are genuine; execution below remains an explicit
// synthetic fixture. No live network, STARK proving or monetary execution is qualified here.
struct AbortFixture {
    directory: tempfile::TempDir,
    client: Client,
    request: PrivateSettlementRequest,
    manifest: AtomicPrivateSettlementV1,
    barrier: PrivateSettlementPrepareBarrierV1,
    journal: WorkflowJournalV1,
    selected: SumeragiFinalityVerifier,
    abort: TransactionIdentityV1,
    later: Option<TransactionIdentityV1>,
    locator_height: Arc<Mutex<u64>>,
    reads: Arc<Mutex<Vec<String>>>,
}
impl AbortFixture {
    fn new(rejected_abort: bool, later_finalize: Option<bool>) -> Self {
        let mut native = NativeFinalityFixture::start("private-original-abort-final-tip");
        let selected = native.verifier().into_genesis_verifier();
        let mut client = client_with_base_url(base_url());
        client.network_id = native.network_id();
        client.chain = native.chain_id().parse().unwrap();
        let (request, certificates) = prepared(&client);
        let manifest = request.manifest_with(&certificates).unwrap();
        let full = checkpoint(&request, &certificates);
        let (barrier, commit) = bundle(&request, &full);
        let binding = AccountWorkflowContext::new(client.account_client().unwrap())
            .unwrap()
            .binding();
        let mut journal = WorkflowJournalV1::new(&request.plan(), &binding, 1).unwrap();
        super::super::super::tests::complete_round(
            &mut journal,
            AVAILABILITY,
            norito::encode_canonical(&certificates).unwrap(),
        );
        assert!(!journal.steps[PREPARE].is_done());
        let mut proofs = BTreeMap::from([(1, native.genesis_proof().clone())]);
        let (proof, abort_ids) = carrier(
            &mut native,
            &client,
            vec![(
                AbortAtomicPrivateSettlementV1::new(
                    manifest.clone(),
                    PrivateSettlementAbortReasonV1::ParticipantRejected,
                )
                .into(),
                rejected_abort,
            )],
            &manifest.public_fee_intent,
        );
        assert_eq!(proof.height(), 2);
        proofs.insert(2, proof);
        let later = later_finalize.map(|rejected| {
            let (proof, identities) = carrier(
                &mut native,
                &client,
                vec![(
                    FinalizeAtomicPrivateSettlementV1::new(commit).into(),
                    rejected,
                )],
                &manifest.public_fee_intent,
            );
            proofs.insert(proof.height(), proof);
            identities[0]
        });
        let locator_height = Arc::new(Mutex::new(2));
        let locator = locator_height.clone();
        let reads = Arc::new(Mutex::new(Vec::new()));
        let observed = reads.clone();
        let public_manifest = manifest.clone();
        let transport = with_mock_http(
            move |http| {
                assert_eq!(
                    http.method,
                    http::Method::GET,
                    "terminal recovery must never sign or dispatch"
                );
                let path = http.url.path().to_owned();
                observed.lock().unwrap().push(path.clone());
                let (content_type, bytes) = if path.ends_with("/receipt") {
                    let response = PrivateSettlementBundleReceiptResponseV1::Aborted(
                        PrivateSettlementAbortReceiptV1 {
                            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
                            network_id: public_manifest.network_id,
                            bundle_id: public_manifest.bundle_id,
                            manifest_digest: public_manifest.manifest_digest().unwrap(),
                            finalized_height: *locator.lock().unwrap(),
                            reason: PrivateSettlementAbortReasonV1::ParticipantRejected,
                        },
                    );
                    ("application/json", norito::json::to_vec(&response)?)
                } else {
                    let height = path.rsplit('/').next().unwrap().parse::<u64>()?;
                    (
                        "application/x-norito",
                        norito::encode_canonical(
                            proofs
                                .get(&height)
                                .expect("no successor or unrelated traffic exists"),
                        )?,
                    )
                };
                Ok(Response::builder()
                    .status(200)
                    .header("content-type", content_type)
                    .body(bytes)?)
            },
            |transport| transport,
        );
        let client = client.with_test_http_transport(transport);
        crate::client::evidence_http_tests::mark_data_model_compatible(&client);
        Self {
            directory: tempfile::tempdir().unwrap(),
            client,
            request,
            manifest,
            barrier,
            journal,
            selected,
            abort: abort_ids[0],
            later,
            locator_height,
            reads,
        }
    }
    fn custody(&self) -> Arc<FilePrivateSettlementFinality> {
        Arc::new(
            FilePrivateSettlementFinality::open(
                self.directory.path().join("finality"),
                &[92; 32],
                MAX_PRIVATE_SETTLEMENT_FINALITY_BYTES,
            )
            .unwrap(),
        )
    }
    fn storage(&self) -> Arc<FileWorkflowStorage> {
        Arc::new(
            FileWorkflowStorage::open_or_create(self.directory.path().join("journal")).unwrap(),
        )
    }
    fn ledger(&self, prepared: bool) -> SettlementLedger {
        let account = self.client.account_client().unwrap();
        let finality = self
            .custody()
            .bind(self.journal.id(), self.request.0.digest, &self.selected)
            .unwrap();
        let state = if prepared {
            PublicContext::prepared(self.barrier.clone(), FINALIZE)
        } else {
            PublicContext::Available {
                manifest: self.manifest.clone(),
            }
        };
        SettlementLedger::new(
            account,
            self.request.clone(),
            Arc::new(Mutex::new(Some(state))),
            self.selected.clone(),
            Arc::new(tokio::sync::Semaphore::new(1)),
            self.storage(),
            finality,
        )
    }
    fn operation(&self) -> PrivateSettlementOperation {
        self.client
            .account_client()
            .unwrap()
            .private_settlement(
                self.request.clone(),
                self.storage(),
                self.custody(),
                self.selected.clone(),
            )
            .unwrap()
    }
}

#[tokio::test]
async fn aborted_final_tip_before_prepare_recovers_after_cold_reopen_without_phase_or_new_traffic()
{
    let mut fixture = AbortFixture::new(false, None);
    fixture.journal.sequence = 1;
    let storage = fixture.storage();
    let bytes = fixture.journal.encode_record().unwrap();
    storage
        .compare_and_store(&fixture.journal.id(), None, &bytes)
        .unwrap();
    drop(storage);
    let operation = fixture.operation();
    let recovered = operation
        .recover(&PrivateSettlementRunOptions::default())
        .await
        .unwrap();
    let VerifiedPrivateSettlementOutcome::Aborted(abort) = recovered.outcome.unwrap() else {
        panic!("Abort became settled money")
    };
    assert_eq!(
        abort.transaction_hash().as_ref(),
        &fixture.abort.signed_transaction_hash
    );
    assert_eq!(abort.receipt().finalized_height, 2);
    assert!(
        recovered.progress.unwrap().steps[PREPARE]
            .transcript_digest
            .is_none()
    );
    drop(operation);
    let first_reads = fixture.reads.lock().unwrap().clone();
    assert_eq!(
        first_reads.len(),
        3,
        "one untrusted locator and H1/H2 native proofs only"
    );
    let operation = fixture.operation();
    let mut options = PrivateSettlementRunOptions::default();
    options.max_finality_proof_reads = 1;
    let recovered = operation.recover(&options).await.unwrap();
    assert!(matches!(
        recovered.outcome,
        Some(VerifiedPrivateSettlementOutcome::Aborted(_))
    ));
    assert_eq!(
        *fixture.reads.lock().unwrap(),
        first_reads,
        "authenticated terminal pin needs no traffic"
    );
    assert_eq!(
        fixture
            .storage()
            .load(&fixture.journal.id())
            .unwrap()
            .unwrap(),
        bytes
    );
    let stored = fixture.ledger(false).finality.load().unwrap();
    assert_eq!(
        stored.record.terminal.unwrap().kind,
        FinalityTerminalKind::Aborted
    );
    assert_eq!(
        MAX_PRIVATE_SETTLEMENT_FINALITY_BYTES,
        3 * iroha_data_model::sumeragi_finality::MAX_FINALITY_CHECKPOINT_BYTES + 4096
    );
}

#[tokio::test]
async fn forged_abort_height_and_included_rejection_never_create_abort_authority() {
    let rejected = AbortFixture::new(true, None);
    let ledger = rejected.ledger(false);
    ledger.begin_call(4).unwrap();
    assert!(ledger.recover_business(&rejected.journal).await.is_err());
    assert!(ledger.finality.load().unwrap().record.terminal.is_none());

    let fixture = AbortFixture::new(false, Some(true));
    *fixture.locator_height.lock().unwrap() = 3;
    let ledger = fixture.ledger(false);
    ledger.begin_call(4).unwrap();
    assert!(ledger.recover_business(&fixture.journal).await.is_err());
    assert!(ledger.finality.load().unwrap().record.terminal.is_none());
    drop(ledger);
    *fixture.locator_height.lock().unwrap() = 2;
    let ledger = fixture.ledger(false);
    ledger.begin_call(2).unwrap();
    assert!(matches!(
        ledger.recover_business(&fixture.journal).await.unwrap(),
        Some(VerifiedPrivateSettlementOutcome::Aborted(_))
    ));
}

#[tokio::test]
async fn abort_terminal_is_immutable_while_exact_later_rejected_finalize_survives_restart() {
    let fixture = AbortFixture::new(false, Some(true));
    let ledger = fixture.ledger(true);
    ledger.begin_call(3).unwrap();
    assert!(matches!(
        ledger.recover_business(&fixture.journal).await.unwrap(),
        Some(VerifiedPrivateSettlementOutcome::Aborted(_))
    ));
    let rejected = fixture.later.unwrap();
    assert!(
        found(&ledger, FinalityPhase::Finalize, rejected, 3)
            .await
            .1
            .result()
            .is_err()
    );
    let original_terminal = ledger.finality.load().unwrap().record.terminal.unwrap();
    assert_eq!(original_terminal.kind, FinalityTerminalKind::Aborted);
    assert_eq!(original_terminal.pin.identity, fixture.abort);
    drop(ledger);
    let ledger = fixture.ledger(true);
    ledger.begin_call(0).unwrap();
    assert!(matches!(
        ledger.outcome(&rejected).await.unwrap(),
        CommittedOutcome::Rejected { .. }
    ));
    assert!(matches!(
        ledger.recover_business(&fixture.journal).await.unwrap(),
        Some(VerifiedPrivateSettlementOutcome::Aborted(_))
    ));
    assert_eq!(
        ledger
            .finality
            .load()
            .unwrap()
            .record
            .terminal
            .unwrap()
            .pin
            .identity,
        fixture.abort
    );
}

#[tokio::test]
async fn conflicting_applied_terminal_or_substituted_terminal_kind_is_refused() {
    let fixture = AbortFixture::new(false, Some(false));
    let ledger = fixture.ledger(true);
    ledger.begin_call(3).unwrap();
    assert!(matches!(
        ledger.recover_business(&fixture.journal).await.unwrap(),
        Some(VerifiedPrivateSettlementOutcome::Aborted(_))
    ));
    assert!(matches!(
        ledger
            .finality_located(FinalityPhase::Finalize, fixture.later, Some(3))
            .await,
        Err(PrivateSettlementError::Binding)
    ));
    let mut saved = ledger.finality.load().unwrap();
    assert_eq!(
        saved.record.terminal.as_ref().unwrap().kind,
        FinalityTerminalKind::Aborted
    );
    saved.record.terminal.as_mut().unwrap().kind = FinalityTerminalKind::Settled;
    ledger
        .finality
        .compare_and_publish(saved.digest.as_ref(), &saved.record)
        .unwrap();
    drop(ledger);
    let ledger = fixture.ledger(true);
    ledger.begin_call(0).unwrap();
    assert!(
        ledger
            .finality_located(FinalityPhase::Abort, None, None)
            .await
            .is_err(),
        "corrupt other-kind pin must not fallback to prefix"
    );
}
