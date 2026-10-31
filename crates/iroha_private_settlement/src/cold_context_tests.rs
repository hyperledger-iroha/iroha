//! Real filesystem/journal reopen over genuine native QCs and synthetic public World values.
//! Deliberate wallet refusal stops before any plaintext witness or proof exists.
use super::*;
use crate::test_support::{client_with_base_url, compatible_capabilities_response};
use iroha::{
    client::Client,
    http::{HttpTransport, TransportFuture, TransportRequest},
    workflow::FileWorkflowStorage,
};
use iroha_data_model::{
    sumeragi_finality::{
        SumeragiFinalityProof, WorldStateElementKindV1, WorldStateSnapshotEntryV1,
        test_fixtures::NativeFinalityFixture, world_state_value_hash_v1,
    },
    sumeragi_lanes::{
        SumeragiLaneFrontier, SumeragiLaneMember, SumeragiLaneRecord, SumeragiLaneState,
    },
};
use iroha_test_samples::private_settlement::{availability_material_fixture_v1, context_fixture};
use std::sync::atomic::{AtomicUsize, Ordering};
#[derive(Debug)]
pub struct FinalityTransport(pub SumeragiFinalityProof);
impl HttpTransport for FinalityTransport {
    fn send_blocking(&self, _: TransportRequest) -> eyre::Result<http::Response<Vec<u8>>> {
        panic!("async original finality required")
    }
    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move {
            if request.url.path() == "/v1/node/capabilities" {
                assert_eq!(request.method, http::Method::GET);
                return compatible_capabilities_response();
            }
            assert_eq!(request.method, http::Method::GET);
            assert_eq!(request.url.path(), "/v1/bridge/finality/2");
            Ok(http::Response::builder()
                .status(200)
                .header("content-type", "application/x-norito")
                .body(norito::encode_canonical(&self.0)?)
                .unwrap())
        })
    }
}
struct Source {
    proof: Option<PrivateSettlementContextsProofV1>,
    reads: AtomicUsize,
}
impl PrivateSettlementContextSource for Source {
    fn fetch(
        &self,
        height: u64,
        selectors: PrivateSettlementContextRequestV1,
    ) -> WorkflowFuture<
        '_,
        std::result::Result<PrivateSettlementContextsProofV1, OneCallPrivateSettlementError>,
    > {
        Box::pin(async move {
            self.reads.fetch_add(1, Ordering::SeqCst);
            assert_eq!(height, 2);
            let proof = self.proof.as_ref().ok_or_else(|| {
                OneCallPrivateSettlementError::Provider(eyre::eyre!("old cut no longer served"))
            })?;
            assert_eq!(
                selectors.pools,
                proof
                    .contexts
                    .iter()
                    .map(|p| PrivateSettlementPoolKeyV1 {
                        route: p.pool.pool.route,
                        pool_id: p.pool.pool.pool_id
                    })
                    .collect::<Vec<_>>()
            );
            Ok(proof.clone())
        })
    }
}
#[derive(Default)]
struct RefusingWallet {
    open: AtomicUsize,
    reserve: AtomicUsize,
}
impl PrivateSettlementWallet for RefusingWallet {
    fn open_policy(
        &self,
        _: &PrivateSettlementProofIntentV1,
        _: u8,
        _: &PrivateSettlementBusinessTerms,
    ) -> std::result::Result<PrivateSettlementRestrictedOpening, OneCallPrivateSettlementError>
    {
        self.open.fetch_add(1, Ordering::SeqCst);
        Err(OneCallPrivateSettlementError::Provider(eyre::eyre!(
            "caller opening temporarily unavailable"
        )))
    }
    fn reserve_or_reopen(
        &self,
        _: Digest32,
        _: &PrivateSettlementProofIntentV1,
        _: u8,
        _: &PrivateSettlementBusinessTerms,
    ) -> std::result::Result<
        AtomicPrivateSettlementAuthorizedWalletLegV1,
        OneCallPrivateSettlementError,
    > {
        self.reserve.fetch_add(1, Ordering::SeqCst);
        panic!("no wallet opening means no witness reservation")
    }
    fn reconcile_verified(
        &self,
        _: Digest32,
        _: &PrivateSettlementRequest,
        _: &VerifiedPrivateSettlementOutcome,
    ) -> Result<(), OneCallPrivateSettlementError> {
        panic!("a refused wallet opening cannot yield verified settlement")
    }
}
fn row<K: norito::codec::Encode, V: norito::codec::Encode>(
    field: &str,
    key: &K,
    value: &V,
) -> WorldStateSnapshotEntryV1 {
    WorldStateSnapshotEntryV1 {
        field_id: field.into(),
        kind: WorldStateElementKindV1::Table,
        key_hash: Some(world_state_value_hash_v1(key).unwrap()),
        value_hash: world_state_value_hash_v1(value).unwrap(),
    }
}
pub fn context(
    client: &Client,
) -> (
    PrivateSettlementContextsProofV1,
    PrivateSettlementProofIntentV1,
) {
    let template = context_fixture(client.network_id(), client.account());
    let (material, _) = availability_material_fixture_v1(client.network_id(), client.account());
    let mut contexts = Vec::new();
    let mut lanes = SumeragiLaneState::default();
    let mut entries = std::collections::BTreeMap::new();
    let mut add = |row: WorldStateSnapshotEntryV1| {
        entries.insert((row.field_id.clone(), row.kind, row.key_hash), row);
    };
    for leg in &material.manifest.legs {
        let mut proof = template.clone();
        let route = leg.route;
        proof.pool.pool.route = route;
        proof.pool.pool.pool_id = leg.pool_id;
        proof.pool.governance.route = route;
        proof.pool.governance.pool_id = leg.pool_id;
        proof.pool.governance.asset_binding_commitment = leg.asset_binding_commitment;
        proof.pool.governance.audit_policy_digest = leg.audit_policy_digest;
        let members = proof
            .committee_keys
            .iter()
            .map(|key| SumeragiLaneMember {
                peer: key.record.public_key.clone().into(),
                pop: key.record.pop.clone().unwrap(),
            })
            .collect();
        lanes.upsert(SumeragiLaneRecord {
            da_layout: iroha_sumeragi::availability::recommended_data_availability_layout(),
            lane: route.lane_id,
            dataspace: route.dataspace_id,
            incarnation: *route.lane_incarnation.as_ref(),
            params: iroha_data_model::parameter::system::SumeragiParameters::default(),
            committee: members,
            created_at: 0,
            active_from: 2,
            closing: None,
            anchor_freshness: 16,
            merged: SumeragiLaneFrontier::default(),
            merged_at: 2,
            rescued: 0,
        });
        let key = PrivateSettlementPoolKeyV1 {
            route,
            pool_id: leg.pool_id,
        };
        add(row(
            "world.private_settlement_governance",
            &key,
            &proof.pool.governance,
        ));
        add(row(
            "world.private_settlement_pools",
            &key,
            &proof.pool.pool,
        ));
        for original in &proof.committee_keys {
            add(row(
                "world.consensus_keys_by_pk",
                &original.record.public_key.to_string(),
                &original.index,
            ));
            add(row(
                "world.consensus_keys",
                &original.record.id,
                &original.record,
            ));
        }
        contexts.push(proof);
    }
    add(WorldStateSnapshotEntryV1 {
        field_id: "world.parameters".into(),
        kind: WorldStateElementKindV1::Cell,
        key_hash: None,
        value_hash: world_state_value_hash_v1(&template.parameters).unwrap(),
    });
    add(WorldStateSnapshotEntryV1 {
        field_id: "world.sumeragi_lanes".into(),
        kind: WorldStateElementKindV1::Cell,
        key_hash: None,
        value_hash: world_state_value_hash_v1(&lanes).unwrap(),
    });
    let entries = entries.into_values().collect::<Vec<_>>();
    for proof in &mut contexts {
        proof.lanes = lanes.clone();
        proof.pool.world.entries = entries.clone();
    }
    (
        PrivateSettlementContextsProofV1 { contexts },
        material.manifest.proof_intent(),
    )
}
pub fn request(
    client: &Client,
    proof: &PrivateSettlementContextsProofV1,
    original: &PrivateSettlementProofIntentV1,
) -> OneCallPrivateSettlementRequest {
    request_at_height(client, proof, original, 2)
}
fn request_at_height(
    client: &Client,
    proof: &PrivateSettlementContextsProofV1,
    original: &PrivateSettlementProofIntentV1,
    height: u64,
) -> OneCallPrivateSettlementRequest {
    let intent = PrivateSettlementProofIntentV1::new(
        original.network_id,
        height,
        original.expiry_height,
        original.sponsor.clone(),
        original.public_fee_intent.clone(),
        original.reimbursement_terms_commitment,
        original.reimbursement_leg_ordinal,
        original.legs.clone(),
    )
    .unwrap();
    let terms = (0..2)
        .map(|index| PrivateSettlementBusinessTerms {
            asset: "6TEAJqbb8oEPmLncoNiMRbLEK6tw".parse().unwrap(),
            payer: client.account().clone(),
            recipient: client.account().clone(),
            amount: 1,
            reimbursement: u128::from(index == usize::from(intent.reimbursement_leg_ordinal)),
            memo: b"cold context fixture".to_vec(),
        })
        .collect();
    OneCallPrivateSettlementRequest::new(
        intent,
        terms,
        proof.contexts[0].pool.world.schema_hash,
        0,
        vec![
            (0..4)
                .map(|index| format!("http://availability-{index}.mock/")
                    .parse()
                    .unwrap())
                .collect();
            2
        ],
        PrivateSettlementCapsulePaddingV1::KiB16,
        original
            .expiry_height
            .checked_add(
                iroha_data_model::privacy::PrivateSettlementPolicyV1::from_parameters(
                    &proof.contexts[0].parameters,
                )
                .unwrap()
                .current_limits
                .sidecar_retention_blocks,
            )
            .unwrap(),
        PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1 as u64,
        32 * 1024 * 1024,
        64 * 1024 * 1024,
    )
    .unwrap()
}
#[tokio::test(flavor = "current_thread")]
async fn private_onecall_cold_reopen_preserves_context_before_wallet_and_never_fetches_replacement()
{
    cold_reopen_preparation(false).await;
}

#[tokio::test(flavor = "current_thread")]
async fn private_prepare_cold_reopen_preserves_original_round_without_monetary_effects() {
    cold_reopen_preparation(true).await;
}

async fn prepare_action(
    operation: &OneCallPrivateSettlementOperation,
    options: &PrivateSettlementRunOptions,
    prepare_only: bool,
) -> Result<(), OneCallPrivateSettlementError> {
    if prepare_only {
        operation.prepare(options).await.map(|_| ())
    } else {
        operation.run(options).await.map(|_| ())
    }
}

async fn cold_reopen_preparation(prepare_only: bool) {
    let mut native = NativeFinalityFixture::start("private-onecall-cold-context");
    let mut builder =
        client_with_base_url(&"http://context.invalid/".parse().unwrap()).to_builder();
    builder.network_id = native.network_id();
    builder.chain = native.chain_id().parse().unwrap();
    let client = builder.build().unwrap();
    let (proof, original_intent) = context(&client);
    let genesis_verifier = native.verifier();
    let block = native.block_with_submitted_work(native.next_header());
    let finality =
        native.certify_with_world_root(block, proof.contexts[0].pool.world.root().unwrap());
    let client = client
        .to_builder()
        .http_transport(Arc::new(FinalityTransport(finality)))
        .build()
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    let journals = root.path().join("journals");
    let artifacts = root.path().join("artifacts");
    let source = Arc::new(Source {
        proof: Some(proof.clone()),
        reads: AtomicUsize::new(0),
    });
    let wallet = Arc::new(RefusingWallet::default());
    let operation = OneCallPrivateSettlementOperation::with_source(
        client.account_client().unwrap(),
        request(&client, &proof, &original_intent),
        Arc::new(FileWorkflowStorage::open_or_create(&journals).unwrap()),
        Arc::new(FilePrivateSettlementArtifacts::open(&artifacts, &[29; 32]).unwrap()),
        source.clone(),
        wallet.clone(),
        genesis_verifier,
    )
    .unwrap();
    let id = operation.id();
    let error = prepare_action(
        &operation,
        &PrivateSettlementRunOptions::default(),
        prepare_only,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(error, OneCallPrivateSettlementError::Provider(_)),
        "original context must be durable before the deliberate wallet refusal: {error:?}"
    );
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
    assert_eq!(wallet.open.load(Ordering::SeqCst), 1);
    let journal = operation.handle.journal_snapshot().unwrap().unwrap();
    assert_eq!(journal.steps.len(), 1);
    assert!(journal.steps[0].transaction.is_none());
    let round = journal.steps[0].round.as_ref().unwrap().round_id.unwrap();
    let original_path = artifacts.join(format!("{}.contexts", hex::encode(round)));
    let original_bytes = std::fs::read(&original_path).unwrap();
    let reopened = FilePrivateSettlementArtifacts::open(&artifacts, &[29; 32]).unwrap();
    let saved = reopened
        .read_context(
            ArtifactKey {
                request: request(&client, &proof, &original_intent).0.digest,
                round,
                ordinal: 0,
            },
            32 * 1024 * 1024,
        )
        .unwrap()
        .unwrap()
        .0;
    assert_eq!(
        saved.contexts.len(),
        2,
        "every selected original must precede the first wallet call"
    );
    assert_eq!(saved, proof);
    drop(reopened);
    drop(operation);
    drop(journal);
    drop(source);
    let absent = Arc::new(Source {
        proof: None,
        reads: AtomicUsize::new(0),
    });
    let rebuilt = OneCallPrivateSettlementOperation::with_source(
        client.account_client().unwrap(),
        request(&client, &proof, &original_intent),
        Arc::new(FileWorkflowStorage::open_or_create(&journals).unwrap()),
        Arc::new(FilePrivateSettlementArtifacts::open(&artifacts, &[29; 32]).unwrap()),
        absent.clone(),
        wallet.clone(),
        NativeFinalityFixture::start("private-onecall-cold-context").verifier(),
    )
    .unwrap();
    assert_eq!(rebuilt.id(), id);
    assert!(
        rebuilt
            .prepared_request(&PrivateSettlementRunOptions::default())
            .await
            .unwrap()
            .is_none(),
        "advanced read-only promotion cannot create missing artifacts"
    );
    assert_eq!(wallet.open.load(Ordering::SeqCst), 1);
    assert_eq!(wallet.reserve.load(Ordering::SeqCst), 0);
    assert_eq!(
        absent.reads.load(Ordering::SeqCst),
        0,
        "advanced promotion cannot replace the persisted original context"
    );
    let recovered = rebuilt
        .recover(&PrivateSettlementRunOptions::default())
        .await
        .unwrap();
    assert!(recovered.outcome.is_none());
    assert!(recovered.progress.is_some());
    assert_eq!(
        wallet.open.load(Ordering::SeqCst),
        1,
        "read-only recovery cannot invoke wallet"
    );
    assert_eq!(
        absent.reads.load(Ordering::SeqCst),
        0,
        "retained exact cut replaces no online source"
    );
    assert!(matches!(
        prepare_action(
            &rebuilt,
            &PrivateSettlementRunOptions::default(),
            prepare_only
        )
        .await,
        Err(OneCallPrivateSettlementError::Provider(_))
    ));
    assert_eq!(wallet.open.load(Ordering::SeqCst), 2);
    assert_eq!(wallet.reserve.load(Ordering::SeqCst), 0);
    assert_eq!(absent.reads.load(Ordering::SeqCst), 0);
    assert_eq!(std::fs::read(original_path).unwrap(), original_bytes);
    assert!(
        rebuilt.handle.journal_snapshot().unwrap().unwrap().steps[0]
            .transaction
            .is_none()
    );
    assert_eq!(
        rebuilt.handle.journal_snapshot().unwrap().unwrap().steps[0]
            .round
            .as_ref()
            .unwrap()
            .round_id,
        Some(round)
    );
}

struct HeldWallet {
    started: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    released: Mutex<std::sync::mpsc::Receiver<()>>,
    finished: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}
impl PrivateSettlementWallet for HeldWallet {
    fn open_policy(
        &self,
        _: &PrivateSettlementProofIntentV1,
        _: u8,
        _: &PrivateSettlementBusinessTerms,
    ) -> std::result::Result<PrivateSettlementRestrictedOpening, OneCallPrivateSettlementError>
    {
        self.started
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .send(())
            .unwrap();
        self.released.lock().unwrap().recv().unwrap();
        let _ = self.finished.lock().unwrap().take().unwrap().send(());
        Err(OneCallPrivateSettlementError::Provider(eyre::eyre!(
            "deliberately held wallet opening"
        )))
    }
    fn reserve_or_reopen(
        &self,
        _: Digest32,
        _: &PrivateSettlementProofIntentV1,
        _: u8,
        _: &PrivateSettlementBusinessTerms,
    ) -> std::result::Result<
        AtomicPrivateSettlementAuthorizedWalletLegV1,
        OneCallPrivateSettlementError,
    > {
        panic!("held opening never grants witness reservation")
    }
    fn reconcile_verified(
        &self,
        _: Digest32,
        _: &PrivateSettlementRequest,
        _: &VerifiedPrivateSettlementOutcome,
    ) -> Result<(), OneCallPrivateSettlementError> {
        panic!("a held wallet opening cannot yield verified settlement")
    }
}

#[tokio::test(flavor = "current_thread")]
async fn private_onecall_cancelled_wallet_worker_keeps_original_round_lease_until_exit() {
    let mut native = NativeFinalityFixture::start("private-onecall-cancel-custody");
    let mut builder =
        client_with_base_url(&"http://context.invalid/".parse().unwrap()).to_builder();
    builder.network_id = native.network_id();
    builder.chain = native.chain_id().parse().unwrap();
    let client = builder.build().unwrap();
    let (proof, original_intent) = context(&client);
    let verifier = native.verifier();
    let block = native.block_with_submitted_work(native.next_header());
    let finality =
        native.certify_with_world_root(block, proof.contexts[0].pool.world.root().unwrap());
    let client = client
        .to_builder()
        .http_transport(Arc::new(FinalityTransport(finality)))
        .build()
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    let journals = root.path().join("journals");
    let artifacts = root.path().join("artifacts");
    let (started, started_rx) = tokio::sync::oneshot::channel();
    let (release, release_rx) = std::sync::mpsc::channel();
    let (finished, finished_rx) = tokio::sync::oneshot::channel();
    let operation = Arc::new(
        OneCallPrivateSettlementOperation::with_source(
            client.account_client().unwrap(),
            request(&client, &proof, &original_intent),
            Arc::new(FileWorkflowStorage::open_or_create(&journals).unwrap()),
            Arc::new(FilePrivateSettlementArtifacts::open(&artifacts, &[31; 32]).unwrap()),
            Arc::new(Source {
                proof: Some(proof.clone()),
                reads: AtomicUsize::new(0),
            }),
            Arc::new(HeldWallet {
                started: Mutex::new(Some(started)),
                released: Mutex::new(release_rx),
                finished: Mutex::new(Some(finished)),
            }),
            verifier.clone(),
        )
        .unwrap(),
    );
    let running = operation.clone();
    let task =
        tokio::spawn(async move { running.run(&PrivateSettlementRunOptions::default()).await });
    tokio::time::timeout(std::time::Duration::from_secs(5), started_rx)
        .await
        .unwrap()
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    let wallet = Arc::new(RefusingWallet::default());
    let source = Arc::new(Source {
        proof: None,
        reads: AtomicUsize::new(0),
    });
    let reopened = OneCallPrivateSettlementOperation::with_source(
        client.account_client().unwrap(),
        request(&client, &proof, &original_intent),
        Arc::new(FileWorkflowStorage::open_or_create(&journals).unwrap()),
        Arc::new(FilePrivateSettlementArtifacts::open(&artifacts, &[31; 32]).unwrap()),
        source.clone(),
        wallet.clone(),
        verifier,
    )
    .unwrap();
    let error = reopened
        .run(&PrivateSettlementRunOptions::default())
        .await
        .unwrap_err();
    match error {
        OneCallPrivateSettlementError::Provider(cause) => {
            let storage = cause
                .downcast_ref::<iroha::workflow::WorkflowStorageError>()
                .expect("original durable runner refusal");
            assert_eq!(
                storage.kind,
                iroha::workflow::WorkflowStorageErrorKind::Busy
            );
        }
        other => {
            panic!("cancelled awaiter must leave the exact original proof lease held: {other:?}")
        }
    }
    assert_eq!(wallet.open.load(Ordering::SeqCst), 0);
    assert_eq!(source.reads.load(Ordering::SeqCst), 0);
    release.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), finished_rx)
        .await
        .unwrap()
        .unwrap();
    // The admitted join owner drops the lease only after native thread exit and
    // disposal of the cancelled result; retry may observe Busy until that completes.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match reopened
            .recover(&PrivateSettlementRunOptions::default())
            .await
        {
            Ok(value) => {
                assert!(value.outcome.is_none());
                break;
            }
            Err(OneCallPrivateSettlementError::Provider(cause))
                if cause
                    .downcast_ref::<iroha::workflow::WorkflowStorageError>()
                    .is_some_and(|e| e.kind == iroha::workflow::WorkflowStorageErrorKind::Busy) =>
            {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "original proof lease never retired"
                );
                tokio::task::yield_now().await;
            }
            other => panic!("unexpected original lease recovery: {other:?}"),
        }
    }
    assert_eq!(wallet.open.load(Ordering::SeqCst), 0);
    assert_eq!(source.reads.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn private_onecall_all_public_intent_bindings_precede_every_wallet_effect() {
    // A mismatch on the second leg must not reserve the first leg. Both mutations remain
    // internally valid public intents, so this tests the authenticated context join.
    for changed_policy in [false, true] {
        let mut native = NativeFinalityFixture::start("private-onecall-public-binding");
        let mut builder =
            client_with_base_url(&"http://context.invalid/".parse().unwrap()).to_builder();
        builder.network_id = native.network_id();
        builder.chain = native.chain_id().parse().unwrap();
        let client = builder.build().unwrap();
        let (proof, original_intent) = context(&client);
        let verifier = native.verifier();
        let block = native.block_with_submitted_work(native.next_header());
        let finality =
            native.certify_with_world_root(block, proof.contexts[0].pool.world.root().unwrap());
        let verified_block = verifier.clone().verify(&finality).unwrap();
        let client = client
            .to_builder()
            .http_transport(Arc::new(FinalityTransport(finality)))
            .build()
            .unwrap();
        let original = request(&client, &proof, &original_intent);
        let mut legs = original.intent().legs.clone();
        if changed_policy {
            legs[1].audit_policy_digest = Hash::new(b"different independently selected policy");
        } else {
            legs[1].asset_binding_commitment = Hash::new(b"different independently selected asset");
        }
        let intent = PrivateSettlementProofIntentV1::new(
            original.intent().network_id,
            original.intent().authority_context_height,
            original.intent().expiry_height,
            original.intent().sponsor.clone(),
            original.intent().public_fee_intent.clone(),
            original.intent().reimbursement_terms_commitment,
            original.intent().reimbursement_leg_ordinal,
            legs,
        )
        .unwrap();
        let changed = OneCallPrivateSettlementRequest::new(
            intent,
            original.0.terms.clone(),
            original.0.schema,
            original.0.minimum_context_time_ms,
            original.0.endpoints.clone(),
            original.0.padding,
            original.0.retention_height,
            original.0.max_carrier_bytes,
            original.0.max_artifact_bytes,
            original.0.max_total_artifact_bytes,
        )
        .unwrap();
        // Prove that this exact authenticated original accepts both initial legs and that
        // only the independently mutated second-leg binding rejects the changed intent.
        for (index, member) in proof.contexts.iter().enumerate() {
            let leg = &original.intent().legs[index];
            let selected = member
                .verify(
                    original.intent().network_id,
                    original.0.schema,
                    PrivateSettlementPoolKeyV1 {
                        route: leg.route,
                        pool_id: leg.pool_id,
                    },
                    &verified_block,
                )
                .unwrap();
            selected
                .verify_intent(original.intent(), leg.ordinal)
                .unwrap();
            assert_eq!(
                selected
                    .verify_intent(changed.intent(), leg.ordinal)
                    .is_err(),
                index == 1,
                "only the selected second-leg binding may differ, changed_policy={changed_policy}"
            );
        }
        let root = tempfile::tempdir().unwrap();
        let source = Arc::new(Source {
            proof: Some(proof),
            reads: AtomicUsize::new(0),
        });
        let wallet = Arc::new(RefusingWallet::default());
        let artifact_path = root.path().join("artifacts");
        let operation = OneCallPrivateSettlementOperation::with_source(
            client.account_client().unwrap(),
            changed,
            Arc::new(FileWorkflowStorage::open_or_create(root.path().join("journals")).unwrap()),
            Arc::new(FilePrivateSettlementArtifacts::open(&artifact_path, &[29; 32]).unwrap()),
            source.clone(),
            wallet.clone(),
            verifier,
        )
        .unwrap();
        let error = operation
            .run(&PrivateSettlementRunOptions::default())
            .await
            .unwrap_err();
        assert!(
            matches!(error, OneCallPrivateSettlementError::Finality(_)),
            "valid public intent must fail at its exact authenticated context join: {error:?}"
        );
        assert_eq!(source.reads.load(Ordering::SeqCst), 1);
        assert_eq!(
            wallet.open.load(Ordering::SeqCst),
            0,
            "every public leg binding must precede the first wallet opening"
        );
        assert_eq!(
            wallet.reserve.load(Ordering::SeqCst),
            0,
            "a known public commitment mismatch must never reserve wallet notes"
        );
        assert!(
            !std::fs::read_dir(&artifact_path).unwrap().any(|entry| entry
                .unwrap()
                .path()
                .extension()
                .is_some_and(|extension| extension == "contexts")),
            "a foreign intent cannot persist a usable original context checkpoint"
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn private_onecall_fresh_readonly_recovery_has_no_journal_or_wallet_effects() {
    let native = NativeFinalityFixture::start("private-onecall-cold-context");
    let mut builder =
        client_with_base_url(&"http://context.invalid/".parse().unwrap()).to_builder();
    builder.network_id = native.network_id();
    builder.chain = native.chain_id().parse().unwrap();
    let client = builder.build().unwrap();
    let (proof, original_intent) = context(&client);
    let root = tempfile::tempdir().unwrap();
    let journals = root.path().join("journals");
    let artifacts = root.path().join("artifacts");
    let source = Arc::new(Source {
        proof: None,
        reads: AtomicUsize::new(0),
    });
    let wallet = Arc::new(RefusingWallet::default());
    let operation = OneCallPrivateSettlementOperation::with_source(
        client.account_client().unwrap(),
        request(&client, &proof, &original_intent),
        Arc::new(FileWorkflowStorage::open_or_create(&journals).unwrap()),
        Arc::new(FilePrivateSettlementArtifacts::open(&artifacts, &[29; 32]).unwrap()),
        source.clone(),
        wallet.clone(),
        native.verifier(),
    )
    .unwrap();
    let options = PrivateSettlementRunOptions::default();
    assert!(
        operation
            .prepared_request(&options)
            .await
            .unwrap()
            .is_none()
    );
    let recovered = operation.recover(&options).await.unwrap();
    assert!(recovered.progress.is_none() && recovered.outcome.is_none());
    assert_eq!(wallet.open.load(Ordering::SeqCst), 0);
    assert_eq!(wallet.reserve.load(Ordering::SeqCst), 0);
    assert_eq!(source.reads.load(Ordering::SeqCst), 0);
    assert_eq!(std::fs::read_dir(journals).unwrap().count(), 0);
    assert_eq!(std::fs::read_dir(artifacts).unwrap().count(), 0);
}

#[derive(Debug, Default)]
struct ZeroReadTransport {
    requests: AtomicUsize,
}
impl HttpTransport for ZeroReadTransport {
    fn send_blocking(&self, _: TransportRequest) -> eyre::Result<http::Response<Vec<u8>>> {
        self.requests.fetch_add(1, Ordering::SeqCst);
        Err(eyre::eyre!("zero-read fixture forbids HTTP"))
    }

    fn send(&self, _: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move {
            self.requests.fetch_add(1, Ordering::SeqCst);
            Err(eyre::eyre!("zero-read fixture forbids HTTP"))
        })
    }
}

fn zero_read_operation() -> (
    tempfile::TempDir,
    OneCallPrivateSettlementOperation,
    Arc<ZeroReadTransport>,
    Arc<Source>,
    Arc<RefusingWallet>,
) {
    let native = NativeFinalityFixture::start("private-zero-finality-read-allowance");
    let transport = Arc::new(ZeroReadTransport::default());
    let mut builder =
        client_with_base_url(&"http://zero-read.invalid/".parse().unwrap()).to_builder();
    builder.network_id = native.network_id();
    builder.chain = native.chain_id().parse().unwrap();
    let client = builder.http_transport(transport.clone()).build().unwrap();
    let (proof, intent) = context(&client);
    let root = tempfile::tempdir().unwrap();
    let source = Arc::new(Source {
        proof: None,
        reads: AtomicUsize::new(0),
    });
    let wallet = Arc::new(RefusingWallet::default());
    let operation = OneCallPrivateSettlementOperation::with_source(
        client.account_client().unwrap(),
        request(&client, &proof, &intent),
        Arc::new(FileWorkflowStorage::open_or_create(root.path().join("journals")).unwrap()),
        Arc::new(
            FilePrivateSettlementArtifacts::open(root.path().join("artifacts"), &[63; 32]).unwrap(),
        ),
        source.clone(),
        wallet.clone(),
        native.verifier(),
    )
    .unwrap();
    (root, operation, transport, source, wallet)
}

#[test]
fn exhausted_finality_allowance_can_handoff_original_cached_settlement() {
    let (_root, operation, transport, source, wallet) = zero_read_operation();
    operation.driver.begin_call(0).unwrap();
    let (_, remaining) = operation
        .driver
        .settlement_handoff(&PrivateSettlementRunOptions::default())
        .expect("zero remaining HTTP reads must preserve the original cached settlement path");
    assert_eq!(remaining.max_finality_proof_reads, 0);
    assert_eq!(transport.requests.load(Ordering::SeqCst), 0);
    assert_eq!(source.reads.load(Ordering::SeqCst), 0);
    assert_eq!(wallet.open.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn zero_finality_allowance_refuses_new_proof_before_http_or_wallet_work() {
    let (_root, operation, transport, source, wallet) = zero_read_operation();
    for _ in 0..2 {
        operation.driver.begin_call(0).unwrap();
        let failure = operation
            .handle
            .run(
                operation.driver.as_ref(),
                &PrivateSettlementRunOptions::default().workflow,
            )
            .await
            .unwrap_err();
        assert_eq!(
            failure.code,
            iroha::workflow::WorkflowErrorCode::LocalResourceUnavailable
        );
        assert!(matches!(
            operation.driver.take_error().unwrap(),
            Some(OneCallPrivateSettlementError::Capacity)
        ));
    }
    assert_eq!(transport.requests.load(Ordering::SeqCst), 0);
    assert_eq!(source.reads.load(Ordering::SeqCst), 0);
    assert_eq!(wallet.open.load(Ordering::SeqCst), 0);
    assert_eq!(wallet.reserve.load(Ordering::SeqCst), 0);
    let journal = operation.handle.journal_snapshot().unwrap().unwrap();
    assert!(
        journal
            .steps
            .iter()
            .all(|step| step.transaction.is_none() && !step.is_done())
    );
}

#[derive(Clone, Copy, Debug)]
enum OperationalRefusal {
    Proof,
    Encoding,
    Binding,
    Signing,
    Permission,
    Unknown,
    RawCodecLimit,
    OriginalDecoderLimit,
    FixedProofLimit,
    Finality,
    Capacity,
}
impl OperationalRefusal {
    fn expected(self) -> WorkflowErrorCode {
        match self {
            Self::Proof | Self::Finality => WorkflowErrorCode::ProofInvalid,
            Self::Encoding => WorkflowErrorCode::NonCanonicalEncoding,
            Self::Binding => WorkflowErrorCode::BindingMismatch,
            Self::Signing | Self::Permission => WorkflowErrorCode::AuthorizationDenied,
            Self::Unknown | Self::RawCodecLimit => WorkflowErrorCode::EvidenceUnavailable,
            Self::OriginalDecoderLimit | Self::Capacity => {
                WorkflowErrorCode::LocalResourceUnavailable
            }
            Self::FixedProofLimit => WorkflowErrorCode::ResourceLimitExceeded,
        }
    }
    fn original(self) -> OneCallPrivateSettlementError {
        use iroha_core_privacy::privacy_engines::atomic_private_settlement::AtomicPrivateSettlementProofErrorV1;
        match self {
            Self::Proof => AtomicPrivateSettlementPreparationErrorV1::Proof(
                AtomicPrivateSettlementProofErrorV1::InvalidProof,
            )
            .into(),
            Self::Encoding => norito::Error::InvalidMagic.into(),
            Self::Binding => OneCallPrivateSettlementError::Binding,
            Self::Signing => OneCallPrivateSettlementError::Provider(eyre::Report::new(
                iroha::Error::RequestSigning {
                    operation: "private-preparation-classification-test",
                    details: "private-test-sentinel must never enter durable diagnostics".into(),
                },
            )),
            Self::Permission => {
                OneCallPrivateSettlementError::Provider(eyre::Report::new(iroha::Error::Http {
                    operation: "private-preparation-classification-test",
                    status: 403,
                    retry_after: None,
                    body: b"private-test-sentinel".to_vec(),
                }))
            }
            Self::Unknown => OneCallPrivateSettlementError::Provider(eyre::eyre!(
                "private-test-sentinel capacity exhausted forbidden proof invalid"
            )),
            Self::RawCodecLimit => norito::Error::TotalAllocationExceeded {
                attempted: 2,
                limit: 1,
            }
            .into(),
            Self::OriginalDecoderLimit => {
                let bytes = norito::encode_canonical(&vec![7_u8; 128]).unwrap();
                let source = norito::core::with_decode_limits_scope(
                    norito::DecodeLimits::new(1024, 1024, 1024, 1, 64),
                    || {
                        norito::core::classify_decode_attempt(|| {
                            norito::decode_canonical::<Vec<u8>>(&bytes)
                        })
                    },
                )
                .unwrap_err();
                assert_eq!(
                    source.kind(),
                    norito::core::DecodeAttemptErrorKind::EnclosingLimit
                );
                OneCallPrivateSettlementError::Provider(eyre::Report::new(
                    iroha::Error::CanonicalDecode(Box::new(iroha::CanonicalDecodeFailure {
                        operation: "private-preparation-classification-test",
                        source,
                    })),
                ))
            }
            Self::FixedProofLimit => AtomicPrivateSettlementPreparationErrorV1::Proof(
                AtomicPrivateSettlementProofErrorV1::ResourceLimit,
            )
            .into(),
            Self::Finality => FinalityReadError::Invalid(FinalityError(
                "private-test-sentinel completed certified root mismatch".into(),
            ))
            .into(),
            Self::Capacity => OneCallPrivateSettlementError::Capacity,
        }
    }
}
struct ClassifiedRefusingWallet {
    cause: OperationalRefusal,
    openings: AtomicUsize,
    reservations: AtomicUsize,
}
impl PrivateSettlementWallet for ClassifiedRefusingWallet {
    fn open_policy(
        &self,
        _: &PrivateSettlementProofIntentV1,
        _: u8,
        _: &PrivateSettlementBusinessTerms,
    ) -> std::result::Result<PrivateSettlementRestrictedOpening, OneCallPrivateSettlementError>
    {
        self.openings.fetch_add(1, Ordering::SeqCst);
        Err(self.cause.original())
    }
    fn reserve_or_reopen(
        &self,
        _: Digest32,
        _: &PrivateSettlementProofIntentV1,
        _: u8,
        _: &PrivateSettlementBusinessTerms,
    ) -> std::result::Result<
        AtomicPrivateSettlementAuthorizedWalletLegV1,
        OneCallPrivateSettlementError,
    > {
        self.reservations.fetch_add(1, Ordering::SeqCst);
        panic!("operational refusal cannot grant original secret reservation")
    }
    fn reconcile_verified(
        &self,
        _: Digest32,
        _: &PrivateSettlementRequest,
        _: &VerifiedPrivateSettlementOutcome,
    ) -> Result<(), OneCallPrivateSettlementError> {
        panic!("an original operational refusal cannot yield verified settlement")
    }
}

#[tokio::test(flavor = "current_thread")]
async fn private_onecall_original_failure_category_survives_store_reopen_without_terminal_verdict()
{
    for cause in [
        OperationalRefusal::Proof,
        OperationalRefusal::Encoding,
        OperationalRefusal::Binding,
        OperationalRefusal::Signing,
        OperationalRefusal::Permission,
        OperationalRefusal::Unknown,
        OperationalRefusal::RawCodecLimit,
        OperationalRefusal::OriginalDecoderLimit,
        OperationalRefusal::FixedProofLimit,
        OperationalRefusal::Finality,
        OperationalRefusal::Capacity,
    ] {
        let mut native = NativeFinalityFixture::start("private-preparation-classification");
        let mut builder =
            client_with_base_url(&"http://context.invalid/".parse().unwrap()).to_builder();
        builder.network_id = native.network_id();
        builder.chain = native.chain_id().parse().unwrap();
        let client = builder.build().unwrap();
        let (proof, original_intent) = context(&client);
        let original_verifier = native.verifier();
        let block = native.block_with_submitted_work(native.next_header());
        let finality =
            native.certify_with_world_root(block, proof.contexts[0].pool.world.root().unwrap());
        // The public compatibility handshake precedes finality reads; any monetary dispatch fails.
        let client = client
            .to_builder()
            .http_transport(Arc::new(FinalityTransport(finality)))
            .build()
            .unwrap();
        let root = tempfile::tempdir().unwrap();
        let journals = root.path().join("journals");
        let artifacts = root.path().join("artifacts");
        let mut original = None;
        for reopening in [false, true] {
            // Every pass creates fresh storage, wallet, request, artifact, verifier and driver owners.
            let storage = Arc::new(FileWorkflowStorage::open_or_create(&journals).unwrap());
            let source = Arc::new(Source {
                proof: (!reopening).then(|| proof.clone()),
                reads: AtomicUsize::new(0),
            });
            let wallet = Arc::new(ClassifiedRefusingWallet {
                cause,
                openings: AtomicUsize::new(0),
                reservations: AtomicUsize::new(0),
            });
            let operation = OneCallPrivateSettlementOperation::with_source(
                client.account_client().unwrap(),
                request(&client, &proof, &original_intent),
                storage.clone(),
                Arc::new(FilePrivateSettlementArtifacts::open(&artifacts, &[73; 32]).unwrap()),
                source.clone(),
                wallet.clone(),
                original_verifier.clone(),
            )
            .unwrap();
            let options = PrivateSettlementRunOptions::default();
            if reopening {
                assert!(operation.recover(&options).await.unwrap().outcome.is_none());
                assert_eq!(wallet.openings.load(Ordering::SeqCst), 0);
                assert_eq!(wallet.reservations.load(Ordering::SeqCst), 0);
                assert_eq!(source.reads.load(Ordering::SeqCst), 0);
            }
            operation
                .driver
                .begin_call(usize::from(options.max_finality_proof_reads))
                .unwrap();
            // Exercise the actual canonical workflow owner before the public wrapper returns
            // the separately preserved original error. No replacement classifier is used.
            let failure = operation
                .handle
                .run(operation.driver.as_ref(), &options.workflow)
                .await
                .unwrap_err();
            assert_eq!(
                failure.code,
                cause.expected(),
                "{cause:?}, reopened={reopening}"
            );
            assert!(!failure.detail.contains("private-test-sentinel"));
            assert!(
                operation.driver.take_error().unwrap().is_some(),
                "public original remains available"
            );
            let journal = operation.handle.journal_snapshot().unwrap().unwrap();
            assert_eq!(
                journal.status,
                iroha::workflow::WorkflowStatusV1::InProgress
            );
            assert_eq!(
                journal.failure_code, None,
                "operational errors are not stored terminal outcomes"
            );
            assert_eq!(journal.failure_step, None);
            assert_eq!(journal.steps.len(), 1);
            assert!(journal.steps.iter().all(|step| step.transaction.is_none()));
            let round = journal.steps[0].round.as_ref().unwrap();
            let round_id = round.round_id.unwrap();
            let stored = storage.load(&operation.id()).unwrap().unwrap();
            assert!(
                !stored
                    .windows(b"private-test-sentinel".len())
                    .any(|window| window == b"private-test-sentinel")
            );
            let context =
                std::fs::read(artifacts.join(format!("{}.contexts", hex::encode(round_id))))
                    .unwrap();
            let identity = (operation.id(), round_id, round.start_ordinal, context);
            if let Some(original) = &original {
                assert_eq!(
                    &identity, original,
                    "restart must retain the original round and encrypted cut"
                );
            } else {
                original = Some(identity);
            }
            assert_eq!(wallet.openings.load(Ordering::SeqCst), 1);
            assert_eq!(wallet.reservations.load(Ordering::SeqCst), 0);
            assert_eq!(source.reads.load(Ordering::SeqCst), usize::from(!reopening));
            // End scope drops every operation/storage owner before the next pass reopens files.
        }
    }
}

#[derive(Debug)]
struct PrefixTransport {
    proofs: Vec<SumeragiFinalityProof>,
    reads: Mutex<Vec<u64>>,
}
impl HttpTransport for PrefixTransport {
    fn send_blocking(&self, _: TransportRequest) -> eyre::Result<http::Response<Vec<u8>>> {
        panic!("pending preparation uses async finality reads")
    }
    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move {
            if request.url.path() == "/v1/node/capabilities" {
                return compatible_capabilities_response();
            }
            assert_eq!(request.method, http::Method::GET);
            let height: u64 = request
                .url
                .path()
                .strip_prefix("/v1/bridge/finality/")
                .expect("only finality acquisition is permitted")
                .parse()
                .unwrap();
            self.reads.lock().unwrap().push(height);
            let proof = self
                .proofs
                .iter()
                .find(|proof| proof.height() == height)
                .expect("selected original proof exists");
            Ok(http::Response::builder()
                .status(200)
                .header("content-type", "application/x-norito")
                .body(norito::encode_canonical(proof)?)
                .unwrap())
        })
    }
}
struct SelectedCutSource {
    proof: PrivateSettlementContextsProofV1,
    height: u64,
    reads: AtomicUsize,
}
impl PrivateSettlementContextSource for SelectedCutSource {
    fn fetch(
        &self,
        height: u64,
        _: PrivateSettlementContextRequestV1,
    ) -> WorkflowFuture<
        '_,
        std::result::Result<PrivateSettlementContextsProofV1, OneCallPrivateSettlementError>,
    > {
        Box::pin(async move {
            assert_eq!(height, self.height);
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(self.proof.clone())
        })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn private_onecall_pending_prefix_advances_across_cold_genesis_restarts_past_read_budget() {
    cold_prefix_preparation(false).await;
}

#[tokio::test(flavor = "current_thread")]
async fn private_prepare_pending_prefix_reopens_beyond_read_budget_without_a_carrier() {
    cold_prefix_preparation(true).await;
}

async fn cold_prefix_preparation(prepare_only: bool) {
    const SELECTED: u64 = 70;
    let mut native = NativeFinalityFixture::start("private-pending-prefix");
    let genesis = native.verifier();
    let mut builder = client_with_base_url(&"http://prefix.invalid/".parse().unwrap()).to_builder();
    builder.network_id = native.network_id();
    builder.chain = native.chain_id().parse().unwrap();
    let client = builder.build().unwrap();
    let (context, intent) = context(&client);
    let mut proofs = Vec::new();
    for _ in 2..=SELECTED {
        let block = native.block_with_submitted_work(native.next_header());
        proofs.push(
            native.certify_with_world_root(block, context.contexts[0].pool.world.root().unwrap()),
        );
    }
    let transport = Arc::new(PrefixTransport {
        proofs,
        reads: Mutex::new(Vec::new()),
    });
    let client = client
        .to_builder()
        .http_transport(transport.clone())
        .build()
        .unwrap();
    let request = request_at_height(&client, &context, &intent, SELECTED);
    let root = tempfile::tempdir().unwrap();
    let journal_path = root.path().join("journals");
    let artifact_path = root.path().join("artifacts");
    let source = Arc::new(SelectedCutSource {
        proof: context,
        height: SELECTED,
        reads: AtomicUsize::new(0),
    });
    let wallet = Arc::new(RefusingWallet::default());
    let reopen = || {
        OneCallPrivateSettlementOperation::with_source(
            client.account_client().unwrap(),
            request.clone(),
            Arc::new(FileWorkflowStorage::open_or_create(&journal_path).unwrap()),
            Arc::new(FilePrivateSettlementArtifacts::open(&artifact_path, &[41; 32]).unwrap()),
            source.clone(),
            wallet.clone(),
            genesis.clone(),
        )
        .unwrap()
    };
    let mut original_round = None;
    for (budget, end) in [(64, 65), (4, 69)] {
        let operation = reopen();
        let mut options = PrivateSettlementRunOptions::default();
        options.max_finality_proof_reads = budget;
        assert!(matches!(
            prepare_action(&operation, &options, prepare_only).await,
            Err(OneCallPrivateSettlementError::Capacity)
        ));
        let journal = operation.handle.journal_snapshot().unwrap().unwrap();
        assert!(journal.steps.iter().all(|step| step.transaction.is_none()));
        let round = journal.steps[0].round.as_ref().unwrap().round_id.unwrap();
        assert_eq!(*original_round.get_or_insert(round), round);
        let saved = FilePrivateSettlementArtifacts::open(&artifact_path, &[41; 32])
            .unwrap()
            .read_finality(
                ArtifactKey {
                    request: request.0.digest,
                    round,
                    ordinal: 0,
                },
                request.0.max_artifact_bytes,
            )
            .unwrap()
            .unwrap()
            .0;
        assert_eq!(saved.height(), end);
        assert_eq!(
            *transport.reads.lock().unwrap(),
            (2..=end).collect::<Vec<_>>()
        );
        assert_eq!(source.reads.load(Ordering::SeqCst), 0);
        assert_eq!(wallet.open.load(Ordering::SeqCst), 0);
        assert_eq!(wallet.reserve.load(Ordering::SeqCst), 0);
        drop(operation);
    }
    let operation = reopen();
    assert!(matches!(
        prepare_action(
            &operation,
            &PrivateSettlementRunOptions::default(),
            prepare_only
        )
        .await,
        Err(OneCallPrivateSettlementError::Provider(_))
    ));
    assert_eq!(
        *transport.reads.lock().unwrap(),
        (2..=SELECTED).collect::<Vec<_>>()
    );
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
    assert_eq!(wallet.open.load(Ordering::SeqCst), 1);
    assert_eq!(wallet.reserve.load(Ordering::SeqCst), 0);
    drop(operation);
    let operation = reopen();
    assert!(
        operation
            .prepared_request(&PrivateSettlementRunOptions::default())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        *transport.reads.lock().unwrap(),
        (2..=SELECTED).collect::<Vec<_>>(),
        "the selected locally authenticated tip needs no new ledger traffic"
    );
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
    assert_eq!(wallet.open.load(Ordering::SeqCst), 1);
    drop(operation);

    let block = native.block_with_submitted_work(native.next_header());
    native.certify(block);
    let newer = OneCallPrivateSettlementOperation::with_source(
        client.account_client().unwrap(),
        request.clone(),
        Arc::new(FileWorkflowStorage::open_or_create(&journal_path).unwrap()),
        Arc::new(FilePrivateSettlementArtifacts::open(&artifact_path, &[41; 32]).unwrap()),
        source.clone(),
        wallet.clone(),
        native.verifier(),
    )
    .unwrap();
    assert!(
        newer
            .prepared_request(&PrivateSettlementRunOptions::default())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        *transport.reads.lock().unwrap(),
        (2..=SELECTED).collect::<Vec<_>>(),
        "a newer caller verifier cannot replace the pending operation's selected cut"
    );
    drop(newer);

    let key = ArtifactKey {
        request: request.0.digest,
        round: original_round.unwrap(),
        ordinal: 0,
    };
    let store = FilePrivateSettlementArtifacts::open(&artifact_path, &[41; 32]).unwrap();
    let foreign = NativeFinalityFixture::new();
    store
        .publish_finality(key, &foreign.checkpoint(), request.0.max_artifact_bytes)
        .unwrap();
    assert!(matches!(
        prepare_action(
            &reopen(),
            &PrivateSettlementRunOptions::default(),
            prepare_only
        )
        .await,
        Err(OneCallPrivateSettlementError::Binding)
    ));
    store
        .publish_finality(key, &native.checkpoint(), request.0.max_artifact_bytes)
        .unwrap();
    assert!(
        matches!(
            prepare_action(
                &reopen(),
                &PrivateSettlementRunOptions::default(),
                prepare_only
            )
            .await,
            Err(OneCallPrivateSettlementError::Binding)
        ),
        "a valid later checkpoint cannot change the selected cut"
    );
    assert_eq!(
        *transport.reads.lock().unwrap(),
        (2..=SELECTED).collect::<Vec<_>>()
    );
    assert_eq!(wallet.open.load(Ordering::SeqCst), 1);
    assert_eq!(wallet.reserve.load(Ordering::SeqCst), 0);
}

#[derive(Debug)]
struct RefusePublicationTransport {
    proof: SumeragiFinalityProof,
    artifact_path: std::path::PathBuf,
    handle: Mutex<Option<WorkflowHandle>>,
    reads: AtomicUsize,
}
impl HttpTransport for RefusePublicationTransport {
    fn send_blocking(&self, _: TransportRequest) -> eyre::Result<http::Response<Vec<u8>>> {
        panic!("preparation reads are async")
    }
    fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
        Box::pin(async move {
            if request.url.path() == "/v1/node/capabilities" {
                return compatible_capabilities_response();
            }
            assert_eq!(request.url.path(), "/v1/bridge/finality/2");
            if self.reads.fetch_add(1, Ordering::SeqCst) == 0 {
                let journal = self
                    .handle
                    .lock()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .journal_snapshot()
                    .unwrap()
                    .unwrap();
                let round = journal.steps[0].round.as_ref().unwrap().round_id.unwrap();
                // The read already observed no checkpoint. Refuse its later atomic publication
                // through actual filesystem custody, without replacing verification or I/O.
                std::fs::create_dir(
                    self.artifact_path
                        .join(format!("{}.preparation-finality", hex::encode(round))),
                )
                .unwrap();
            }
            Ok(http::Response::builder()
                .status(200)
                .header("content-type", "application/x-norito")
                .body(norito::encode_canonical(&self.proof)?)
                .unwrap())
        })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn private_pending_finality_publication_refusal_keeps_original_verifier_until_retry() {
    let mut native = NativeFinalityFixture::start("private-pending-publication-refusal");
    let genesis = native.verifier();
    let mut builder =
        client_with_base_url(&"http://publication.invalid/".parse().unwrap()).to_builder();
    builder.network_id = native.network_id();
    builder.chain = native.chain_id().parse().unwrap();
    let client = builder.build().unwrap();
    let (context, intent) = context(&client);
    let block = native.block_with_submitted_work(native.next_header());
    let proof =
        native.certify_with_world_root(block, context.contexts[0].pool.world.root().unwrap());
    let root = tempfile::tempdir().unwrap();
    let artifact_path = root.path().join("artifacts");
    let transport = Arc::new(RefusePublicationTransport {
        proof,
        artifact_path: artifact_path.clone(),
        handle: Mutex::new(None),
        reads: AtomicUsize::new(0),
    });
    let client = client
        .to_builder()
        .http_transport(transport.clone())
        .build()
        .unwrap();
    let source = Arc::new(Source {
        proof: Some(context.clone()),
        reads: AtomicUsize::new(0),
    });
    let wallet = Arc::new(RefusingWallet::default());
    let request = request(&client, &context, &intent);
    let operation = OneCallPrivateSettlementOperation::with_source(
        client.account_client().unwrap(),
        request.clone(),
        Arc::new(FileWorkflowStorage::open_or_create(root.path().join("journal")).unwrap()),
        Arc::new(FilePrivateSettlementArtifacts::open(&artifact_path, &[45; 32]).unwrap()),
        source.clone(),
        wallet.clone(),
        genesis,
    )
    .unwrap();
    *transport.handle.lock().unwrap() = Some(operation.handle.clone());
    let options = PrivateSettlementRunOptions::default();
    assert!(matches!(
        operation.run(&options).await,
        Err(OneCallPrivateSettlementError::Storage(_))
    ));
    assert_eq!(
        operation
            .driver
            .settlement_handoff(&options)
            .unwrap()
            .0
            .authenticated_height(),
        Some(1),
        "unpublished progress must not advance the original verifier"
    );
    assert_eq!(source.reads.load(Ordering::SeqCst), 0);
    assert_eq!(wallet.open.load(Ordering::SeqCst), 0);
    let journal = operation.handle.journal_snapshot().unwrap().unwrap();
    let round = journal.steps[0].round.as_ref().unwrap().round_id.unwrap();
    std::fs::remove_dir(artifact_path.join(format!("{}.preparation-finality", hex::encode(round))))
        .unwrap();
    assert!(matches!(
        operation.run(&options).await,
        Err(OneCallPrivateSettlementError::Provider(_))
    ));
    assert_eq!(
        transport.reads.load(Ordering::SeqCst),
        2,
        "retry reauthenticates and republishes the refused cut"
    );
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
    assert_eq!(wallet.open.load(Ordering::SeqCst), 1);
    let store = FilePrivateSettlementArtifacts::open(&artifact_path, &[45; 32]).unwrap();
    assert_eq!(
        store
            .read_finality(
                ArtifactKey {
                    request: request.0.digest,
                    round,
                    ordinal: 0
                },
                request.0.max_artifact_bytes
            )
            .unwrap()
            .unwrap()
            .0
            .height(),
        2
    );
    assert_eq!(wallet.reserve.load(Ordering::SeqCst), 0);
    *transport.handle.lock().unwrap() = None;
}

struct HeldPanicTls {
    started: Option<tokio::sync::oneshot::Sender<()>>,
    release: std::sync::mpsc::Receiver<()>,
    finished: Option<tokio::sync::oneshot::Sender<()>>,
}
impl Drop for HeldPanicTls {
    fn drop(&mut self) {
        if let Some(started) = self.started.take() {
            let _ = started.send(());
        }
        // Dropping the test's sender also releases teardown on an assertion failure.
        // No nested panic is permitted while the native wallet panic is unwinding.
        let _ = self.release.recv();
        if let Some(finished) = self.finished.take() {
            let _ = finished.send(());
        }
    }
}
std::thread_local! {
    static WALLET_PANIC_TLS: std::cell::RefCell<Option<HeldPanicTls>> = const { std::cell::RefCell::new(None) };
}
struct PanicTlsWallet(Mutex<Option<HeldPanicTls>>);
impl PrivateSettlementWallet for PanicTlsWallet {
    fn open_policy(
        &self,
        _: &PrivateSettlementProofIntentV1,
        _: u8,
        _: &PrivateSettlementBusinessTerms,
    ) -> std::result::Result<PrivateSettlementRestrictedOpening, OneCallPrivateSettlementError>
    {
        let original = self.0.lock().unwrap().take().unwrap();
        WALLET_PANIC_TLS.with(|slot| {
            *slot.borrow_mut() = Some(original);
        });
        panic!("original wallet panic with retained native TLS");
    }
    fn reserve_or_reopen(
        &self,
        _: Digest32,
        _: &PrivateSettlementProofIntentV1,
        _: u8,
        _: &PrivateSettlementBusinessTerms,
    ) -> std::result::Result<
        AtomicPrivateSettlementAuthorizedWalletLegV1,
        OneCallPrivateSettlementError,
    > {
        panic!("panicking opening never authorizes a reservation")
    }
    fn reconcile_verified(
        &self,
        _: Digest32,
        _: &PrivateSettlementRequest,
        _: &VerifiedPrivateSettlementOutcome,
    ) -> Result<(), OneCallPrivateSettlementError> {
        panic!("a panicking wallet opening cannot yield verified settlement")
    }
}

#[tokio::test(flavor = "current_thread")]
async fn private_onecall_wallet_panic_retains_round_lease_through_original_tls_exit() {
    let mut native = NativeFinalityFixture::start("private-wallet-panic-tls");
    let verifier = native.verifier();
    let mut builder =
        client_with_base_url(&"http://panic-tls.invalid/".parse().unwrap()).to_builder();
    builder.network_id = native.network_id();
    builder.chain = native.chain_id().parse().unwrap();
    let client = builder.build().unwrap();
    let (proof, original_intent) = context(&client);
    let block = native.block_with_submitted_work(native.next_header());
    let finality =
        native.certify_with_world_root(block, proof.contexts[0].pool.world.root().unwrap());
    let client = client
        .to_builder()
        .http_transport(Arc::new(FinalityTransport(finality)))
        .build()
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    let journals = root.path().join("journals");
    let artifacts = root.path().join("artifacts");
    let (started, started_rx) = tokio::sync::oneshot::channel();
    let (release, release_rx) = std::sync::mpsc::channel();
    let (finished, finished_rx) = tokio::sync::oneshot::channel();
    let original = Arc::new(
        OneCallPrivateSettlementOperation::with_source(
            client.account_client().unwrap(),
            request(&client, &proof, &original_intent),
            Arc::new(FileWorkflowStorage::open_or_create(&journals).unwrap()),
            Arc::new(FilePrivateSettlementArtifacts::open(&artifacts, &[47; 32]).unwrap()),
            Arc::new(Source {
                proof: Some(proof.clone()),
                reads: AtomicUsize::new(0),
            }),
            Arc::new(PanicTlsWallet(Mutex::new(Some(HeldPanicTls {
                started: Some(started),
                release: release_rx,
                finished: Some(finished),
            })))),
            verifier.clone(),
        )
        .unwrap(),
    );
    let running = original.clone();
    let task =
        tokio::spawn(async move { running.run(&PrivateSettlementRunOptions::default()).await });
    tokio::time::timeout(std::time::Duration::from_secs(5), started_rx)
        .await
        .unwrap()
        .unwrap();
    // The wallet has already unwound; its original native thread remains inside TLS teardown.
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    drop(original);
    let wallet = Arc::new(RefusingWallet::default());
    let source = Arc::new(Source {
        proof: None,
        reads: AtomicUsize::new(0),
    });
    let reopened = OneCallPrivateSettlementOperation::with_source(
        client.account_client().unwrap(),
        request(&client, &proof, &original_intent),
        Arc::new(FileWorkflowStorage::open_or_create(&journals).unwrap()),
        Arc::new(FilePrivateSettlementArtifacts::open(&artifacts, &[47; 32]).unwrap()),
        source.clone(),
        wallet.clone(),
        verifier,
    )
    .unwrap();
    let error = reopened
        .run(&PrivateSettlementRunOptions::default())
        .await
        .unwrap_err();
    assert!(
        matches!(&error, OneCallPrivateSettlementError::Provider(cause)
        if cause.downcast_ref::<iroha::workflow::WorkflowStorageError>()
            .is_some_and(|error| error.kind == iroha::workflow::WorkflowStorageErrorKind::Busy)),
        "a reconstructed operation cannot replace panicked wallet custody before TLS exit: {error:?}"
    );
    assert_eq!(wallet.open.load(Ordering::SeqCst), 0);
    assert_eq!(wallet.reserve.load(Ordering::SeqCst), 0);
    assert_eq!(source.reads.load(Ordering::SeqCst), 0);
    release.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), finished_rx)
        .await
        .unwrap()
        .unwrap();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match reopened
            .recover(&PrivateSettlementRunOptions::default())
            .await
        {
            Ok(result) => {
                assert!(result.outcome.is_none());
                break;
            }
            Err(OneCallPrivateSettlementError::Provider(cause))
                if cause
                    .downcast_ref::<iroha::workflow::WorkflowStorageError>()
                    .is_some_and(|error| {
                        error.kind == iroha::workflow::WorkflowStorageErrorKind::Busy
                    }) =>
            {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "native custody never retired"
                );
                tokio::task::yield_now().await;
            }
            other => panic!("unexpected recovery after native TLS exit: {other:?}"),
        }
    }
    assert_eq!(wallet.open.load(Ordering::SeqCst), 0);
    assert_eq!(wallet.reserve.load(Ordering::SeqCst), 0);
    assert_eq!(source.reads.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn private_onecall_fresh_custody_uses_original_older_cut_from_newer_caller_verifier() {
    let mut native = NativeFinalityFixture::start("private-fresh-older-cut");
    let mut builder =
        client_with_base_url(&"http://older-cut.invalid/".parse().unwrap()).to_builder();
    builder.network_id = native.network_id();
    builder.chain = native.chain_id().parse().unwrap();
    let client = builder.build().unwrap();
    let (proof, original_intent) = context(&client);
    let block = native.block_with_submitted_work(native.next_header());
    let selected =
        native.certify_with_world_root(block, proof.contexts[0].pool.world.root().unwrap());
    let block = native.block_with_submitted_work(native.next_header());
    native.certify(block);
    assert_eq!(native.verifier().authenticated_height(), Some(3));
    let transport = Arc::new(PrefixTransport {
        proofs: vec![selected],
        reads: Mutex::new(Vec::new()),
    });
    let client = client
        .to_builder()
        .http_transport(transport.clone())
        .build()
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    let source = Arc::new(Source {
        proof: Some(proof.clone()),
        reads: AtomicUsize::new(0),
    });
    let wallet = Arc::new(RefusingWallet::default());
    let operation = OneCallPrivateSettlementOperation::with_source(
        client.account_client().unwrap(),
        request(&client, &proof, &original_intent),
        Arc::new(FileWorkflowStorage::open_or_create(root.path().join("journals")).unwrap()),
        Arc::new(
            FilePrivateSettlementArtifacts::open(root.path().join("artifacts"), &[49; 32]).unwrap(),
        ),
        source.clone(),
        wallet.clone(),
        native.verifier(),
    )
    .unwrap();
    assert!(
        matches!(
            operation.run(&PrivateSettlementRunOptions::default()).await,
            Err(OneCallPrivateSettlementError::Provider(_))
        ),
        "the exact authenticated older cut must reach the deliberately refusing wallet"
    );
    assert_eq!(
        *transport.reads.lock().unwrap(),
        vec![2],
        "no latest-cut substitution or new ledger traffic"
    );
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
    assert_eq!(wallet.open.load(Ordering::SeqCst), 1);
    assert_eq!(wallet.reserve.load(Ordering::SeqCst), 0);
    assert_eq!(
        operation
            .driver
            .settlement_handoff(&PrivateSettlementRunOptions::default())
            .unwrap()
            .0
            .authenticated_height(),
        Some(3),
        "borrowing an older decision must not mutate the original supplied prefix"
    );
}

fn request_with_local_limits(
    original: &OneCallPrivateSettlementRequest,
    carrier: u64,
    artifact: usize,
    total: usize,
) -> OneCallPrivateSettlementRequest {
    let original = &original.0;
    OneCallPrivateSettlementRequest::new(
        original.intent.clone(),
        original.terms.clone(),
        original.schema,
        original.minimum_context_time_ms,
        original.endpoints.clone(),
        original.padding,
        original.retention_height,
        carrier,
        artifact,
        total,
    )
    .unwrap()
}

#[test]
fn private_onecall_local_limits_preserve_original_identity_but_semantic_terms_do_not() {
    let client = client_with_base_url(&"http://context.invalid/".parse().unwrap());
    let (proof, intent) = context(&client);
    let original = request(&client, &proof, &intent);
    let restricted = request_with_local_limits(&original, 1, 1, 1);
    assert_eq!(original.0.digest, restricted.0.digest);
    assert_eq!(
        norito::encode_canonical(&original.plan()).unwrap(),
        norito::encode_canonical(&restricted.plan()).unwrap()
    );
    let binding = AccountWorkflowContext::new(client.account_client().unwrap())
        .unwrap()
        .binding();
    let id = |request: &OneCallPrivateSettlementRequest| {
        iroha::workflow::WorkflowIdentityV1::new(&request.plan(), &binding)
            .unwrap()
            .workflow_id()
            .unwrap()
    };
    assert_eq!(id(&original), id(&restricted));
    for changed in 0..3 {
        let source = &original.0;
        let mut terms = source.terms.clone();
        if changed == 0 {
            terms[0].amount += 1;
        }
        let modified = OneCallPrivateSettlementRequest::new(
            source.intent.clone(),
            terms,
            if changed == 1 {
                Hash::new(b"another independent schema")
            } else {
                source.schema
            },
            source.minimum_context_time_ms,
            source.endpoints.clone(),
            if changed == 2 {
                PrivateSettlementCapsulePaddingV1::KiB64
            } else {
                source.padding
            },
            source.retention_height,
            source.max_carrier_bytes,
            source.max_artifact_bytes,
            source.max_total_artifact_bytes,
        )
        .unwrap();
        assert_ne!(id(&original), id(&modified));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn private_onecall_larger_local_budget_reopens_original_round_after_capacity_refusal() {
    let mut native = NativeFinalityFixture::start("private-onecall-local-budget-resume");
    let mut builder =
        client_with_base_url(&"http://context.invalid/".parse().unwrap()).to_builder();
    builder.network_id = native.network_id();
    builder.chain = native.chain_id().parse().unwrap();
    let client = builder.build().unwrap();
    let (proof, intent) = context(&client);
    let original = request(&client, &proof, &intent);
    let restricted = request_with_local_limits(&original, 1, 1, 1);
    let selected = native.verifier();
    let block = native.block_with_submitted_work(native.next_header());
    let finality =
        native.certify_with_world_root(block, proof.contexts[0].pool.world.root().unwrap());
    let client = client
        .to_builder()
        .http_transport(Arc::new(FinalityTransport(finality)))
        .build()
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    let source = Arc::new(Source {
        proof: Some(proof),
        reads: AtomicUsize::new(0),
    });
    let wallet = Arc::new(RefusingWallet::default());
    let open = |request| {
        OneCallPrivateSettlementOperation::with_source(
            client.account_client().unwrap(),
            request,
            Arc::new(FileWorkflowStorage::open_or_create(root.path().join("journals")).unwrap()),
            Arc::new(
                FilePrivateSettlementArtifacts::open(root.path().join("artifacts"), &[62; 32])
                    .unwrap(),
            ),
            source.clone(),
            wallet.clone(),
            selected.clone(),
        )
        .unwrap()
    };
    let operation = open(restricted);
    let id = operation.id();
    assert!(matches!(
        operation.run(&PrivateSettlementRunOptions::default()).await,
        Err(OneCallPrivateSettlementError::Capacity)
    ));
    let before = operation.handle.journal_snapshot().unwrap().unwrap();
    let round = before.steps[0].round.as_ref().unwrap().round_id;
    assert_eq!(wallet.open.load(Ordering::SeqCst), 0);
    drop(operation);
    let operation = open(original);
    assert_eq!(operation.id(), id);
    assert!(matches!(
        operation.run(&PrivateSettlementRunOptions::default()).await,
        Err(OneCallPrivateSettlementError::Provider(_))
    ));
    let after = operation.handle.journal_snapshot().unwrap().unwrap();
    assert_eq!(after.steps[0].round.as_ref().unwrap().round_id, round);
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
    assert_eq!(wallet.open.load(Ordering::SeqCst), 1);
    assert_eq!(wallet.reserve.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn committed_padding_and_retention_refuse_before_original_wallet_effects() {
    for insufficient_retention in [false, true] {
        let mut native = NativeFinalityFixture::start("private-selected-policy-before-wallet");
        let genesis = native.verifier();
        let mut builder =
            client_with_base_url(&"http://selected.invalid/".parse().unwrap()).to_builder();
        builder.network_id = native.network_id();
        builder.chain = native.chain_id().parse().unwrap();
        let client = builder.build().unwrap();
        let (mut proof, intent) = context(&client);
        if !insufficient_retention {
            for member in &mut proof.contexts {
                let mut policy =
                    iroha_data_model::privacy::PrivateSettlementPolicyV1::from_parameters(
                        &member.parameters,
                    )
                    .unwrap();
                policy.current_limits.capsule_padding_classes = 1;
                member
                    .parameters
                    .set_parameter(iroha_data_model::parameter::Parameter::Custom(
                        policy.to_custom_parameter().unwrap(),
                    ));
                let hash = world_state_value_hash_v1(&member.parameters).unwrap();
                member
                    .pool
                    .world
                    .entries
                    .iter_mut()
                    .find(|entry| entry.field_id == "world.parameters")
                    .unwrap()
                    .value_hash = hash;
            }
        }
        let mut original = request(&client, &proof, &intent);
        if insufficient_retention {
            let selected = &original.0;
            original = OneCallPrivateSettlementRequest::new(
                selected.intent.clone(),
                selected.terms.clone(),
                selected.schema,
                selected.minimum_context_time_ms,
                selected.endpoints.clone(),
                selected.padding,
                selected.retention_height - 1,
                selected.max_carrier_bytes,
                selected.max_artifact_bytes,
                selected.max_total_artifact_bytes,
            )
            .unwrap();
        }
        let block = native.block_with_submitted_work(native.next_header());
        let finality =
            native.certify_with_world_root(block, proof.contexts[0].pool.world.root().unwrap());
        let client = client
            .to_builder()
            .http_transport(Arc::new(FinalityTransport(finality)))
            .build()
            .unwrap();
        let root = tempfile::tempdir().unwrap();
        let source = Arc::new(Source {
            proof: Some(proof),
            reads: AtomicUsize::new(0),
        });
        let wallet = Arc::new(RefusingWallet::default());
        let operation = OneCallPrivateSettlementOperation::with_source(
            client.account_client().unwrap(),
            original,
            Arc::new(FileWorkflowStorage::open_or_create(root.path().join("journals")).unwrap()),
            Arc::new(
                FilePrivateSettlementArtifacts::open(root.path().join("artifacts"), &[83; 32])
                    .unwrap(),
            ),
            source.clone(),
            wallet.clone(),
            genesis,
        )
        .unwrap();
        assert!(matches!(
            operation
                .prepare(&PrivateSettlementRunOptions::default())
                .await,
            Err(OneCallPrivateSettlementError::Binding)
        ));
        assert_eq!(source.reads.load(Ordering::SeqCst), 1);
        assert_eq!(wallet.open.load(Ordering::SeqCst), 0);
        assert_eq!(wallet.reserve.load(Ordering::SeqCst), 0);
        let journal = operation.handle.journal_snapshot().unwrap().unwrap();
        assert!(journal.steps.iter().all(|step| step.transaction.is_none()));
        assert!(
            std::fs::read_dir(root.path().join("artifacts"))
                .unwrap()
                .all(|entry| entry
                    .unwrap()
                    .path()
                    .extension()
                    .is_none_or(|extension| extension != "prepared"))
        );
    }
}
