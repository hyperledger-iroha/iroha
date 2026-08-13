use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::BlockHeader,
    metadata::Metadata,
    musubi::{
        MusubiArchiveCommitmentV1, MusubiContentDigestV1, MusubiReplicationOrderArchiveBindingV1,
    },
    sorafs::{
        capacity::ProviderId,
        pin_registry::{
            ChunkerProfileHandle, ManifestDigest, ManifestRootCid, PinManifestFinalizedCursorV1,
            PinManifestFinalizedRecordV1, PinManifestRecord, PinPolicy as RegistryPinPolicy,
            ReplicationOrderId, ReplicationOrderRecord, ReplicationOrderStatus,
            StorageClass as RegistryStorageClass,
        },
    },
    transaction::{SignedTransaction, TransactionPayload},
};
use sorafs_car::{CarBuildPlan, CarWriter, compute_chunk_plan_digest_sha3, compute_por_root};
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, DagCodecId, ManifestBuilder, PinPolicy as ManifestPinPolicy,
    capacity::{
        REPLICATION_ORDER_VERSION_V1, ReplicationAssignmentV1, ReplicationOrderSlaV1,
        ReplicationOrderV1,
    },
};
use sorafs_node::{
    FinalizedProviderIngestAuthorizationV1, FinalizedProviderIngestMusubiContextV1, NodeHandle,
    NodeRuntimeDeps, NodeStorageError, PROVIDER_INGEST_OUTBOX_FILE_V1,
    ProviderIngestAuthenticatedSourceFetchV1, ProviderIngestCheckpointExternalErrorV1,
    ProviderIngestCheckpointProviderBindingV1, ProviderIngestCheckpointProviderQualificationV1,
    ProviderIngestCheckpointRuntimeV1, ProviderIngestClaimOwnerV1, ProviderIngestClockV1,
    ProviderIngestCompletionPayloadBuilderV1, ProviderIngestCompletionPayloadErrorV1,
    ProviderIngestCompletionPayloadRequestV1, ProviderIngestCompletionSignerResolverErrorV1,
    ProviderIngestCompletionSignerResolverV1, ProviderIngestDeadLetterReasonV1,
    ProviderIngestDeliveryStateV1, ProviderIngestFailureClassV1,
    ProviderIngestFinalizedAssignmentPageV1, ProviderIngestFinalizedAssignmentV1,
    ProviderIngestFinalizedClaimFactoryV1, ProviderIngestFinalizedCursorV1,
    ProviderIngestFinalizedLedgerErrorV1, ProviderIngestFinalizedLedgerV1, ProviderIngestFutureV1,
    ProviderIngestIngressDispositionV1, ProviderIngestIngressPrepareErrorV1, ProviderIngestOutbox,
    ProviderIngestOutboxPolicyV1, ProviderIngestRuntimePolicyV1,
    ProviderIngestSealedCheckpointRecordV1, ProviderIngestSourceFetchErrorV1,
    ProviderIngestSourceRequestV1, ProviderIngestTransactionIngressV1,
    ProviderIngestTransactionObservationV1, config::StorageConfig, store::StorageError,
};

use super::*;

const LOCAL_PROVIDER: [u8; 32] = [0x11; 32];
const SOURCE_PROVIDER: [u8; 32] = [0x22; 32];
const ORDER_ID: [u8; 32] = [0x31; 32];
const CHECKPOINT_HANDLE: &str = "sealed:sorafs-provider-ingest-primary";
const CHECKPOINT_POLICY_DIGEST: [u8; 32] = [0xC7; 32];

#[derive(Debug)]
struct CrashRestartCheckpointRuntimeV1 {
    latest: Mutex<Option<ProviderIngestSealedCheckpointRecordV1>>,
}

impl CrashRestartCheckpointRuntimeV1 {
    fn binding() -> ProviderIngestCheckpointProviderBindingV1 {
        ProviderIngestCheckpointProviderBindingV1 {
            handle: CHECKPOINT_HANDLE.to_owned(),
            revision: 1,
            policy_digest: CHECKPOINT_POLICY_DIGEST,
        }
    }
}

impl ProviderIngestCheckpointRuntimeV1 for CrashRestartCheckpointRuntimeV1 {
    fn handle(&self) -> &str {
        CHECKPOINT_HANDLE
    }

    fn qualification(
        &self,
    ) -> Result<
        ProviderIngestCheckpointProviderQualificationV1,
        ProviderIngestCheckpointExternalErrorV1,
    > {
        Ok(ProviderIngestCheckpointProviderQualificationV1::new(
            1,
            CHECKPOINT_POLICY_DIGEST,
        ))
    }

    fn load_latest(
        &self,
    ) -> Result<
        Option<ProviderIngestSealedCheckpointRecordV1>,
        ProviderIngestCheckpointExternalErrorV1,
    > {
        self.latest
            .lock()
            .map(|latest| latest.clone())
            .map_err(|_| ProviderIngestCheckpointExternalErrorV1::Unavailable)
    }

    fn compare_and_swap_latest(
        &self,
        expected_revision: Option<[u8; 32]>,
        next: &ProviderIngestSealedCheckpointRecordV1,
    ) -> Result<(), ProviderIngestCheckpointExternalErrorV1> {
        let mut latest = self
            .latest
            .lock()
            .map_err(|_| ProviderIngestCheckpointExternalErrorV1::Unavailable)?;
        if latest.as_ref().map(|record| record.revision) != expected_revision {
            return Err(ProviderIngestCheckpointExternalErrorV1::Rejected);
        }
        *latest = Some(next.clone());
        Ok(())
    }
}

struct CrashRestartLedgerV1 {
    network_id: NetworkId,
    cursor: ProviderIngestFinalizedCursorV1,
    pin: PinManifestRecord,
    order: ReplicationOrderRecord,
    archive: MusubiReplicationOrderArchiveBindingV1,
}

impl ProviderIngestFinalizedLedgerV1 for CrashRestartLedgerV1 {
    fn read_assignment_page<'a>(
        &'a self,
        claim_factory: ProviderIngestFinalizedClaimFactoryV1,
        at_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
        after_order_id: Option<[u8; 32]>,
        limit: usize,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<ProviderIngestFinalizedAssignmentPageV1, ProviderIngestFinalizedLedgerErrorV1>,
    > {
        let result = (|| {
            if limit == 0
                || after_order_id.is_some()
                || at_finalized_cursor.is_some_and(|cursor| cursor != self.cursor)
            {
                return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
            }
            let claim = claim_factory.seal_musubi_archive(
                &self.network_id,
                self.cursor,
                *self.order.order_id.as_bytes(),
                &self.pin,
                self.archive.clone(),
            )?;
            Ok(ProviderIngestFinalizedAssignmentPageV1 {
                finalized_cursor: self.cursor,
                finalized_block_time_ms: 8_000,
                rows: vec![ProviderIngestFinalizedAssignmentV1 {
                    pin: PinManifestFinalizedRecordV1 {
                        finalized_cursor: PinManifestFinalizedCursorV1 {
                            height: self.cursor.height,
                            block_hash: self.cursor.block_hash,
                        },
                        manifest: self.pin.clone(),
                    },
                    order: self.order.clone(),
                    musubi_archive: Some(claim),
                    completed_musubi_archive: None,
                    provider_owner: None,
                    completion_authority: None,
                    completion_epoch: None,
                    committed_transaction_hash: None,
                }],
                next_after_order_id: None,
            })
        })();
        Box::pin(async move { result })
    }
}

struct CrashRestartFetchV1 {
    calls: AtomicU64,
}

impl ProviderIngestAuthenticatedSourceFetchV1 for CrashRestartFetchV1 {
    type Fetched = VerifiedProviderIngestPayloadV1;

    fn fetch<'a>(
        &'a self,
        _request: ProviderIngestSourceRequestV1,
    ) -> ProviderIngestFutureV1<'a, Result<Self::Fetched, ProviderIngestSourceFetchErrorV1>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(ProviderIngestSourceFetchErrorV1::Unavailable) })
    }
}

struct NeverBuildCompletionV1;

impl ProviderIngestCompletionPayloadBuilderV1 for NeverBuildCompletionV1 {
    fn build_payload<'a>(
        &'a self,
        _request: ProviderIngestCompletionPayloadRequestV1,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<TransactionPayload, ProviderIngestCompletionPayloadErrorV1>,
    > {
        Box::pin(async { Err(ProviderIngestCompletionPayloadErrorV1::Rejected) })
    }
}

struct NeverResolveSignerV1;

impl ProviderIngestCompletionSignerResolverV1 for NeverResolveSignerV1 {
    type Signer = TestGovernedCompletionSignerV1;

    fn resolve<'a>(
        &'a self,
        _context: ProviderIngestCompletionSignerResolutionContextV1,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<Option<Self::Signer>, ProviderIngestCompletionSignerResolverErrorV1>,
    > {
        Box::pin(async { Err(ProviderIngestCompletionSignerResolverErrorV1::Rejected) })
    }
}

struct NeverIngressV1;

impl ProviderIngestTransactionIngressV1 for NeverIngressV1 {
    type Prepared = ();

    fn prepare<'a>(
        &'a self,
        _transaction: SignedTransaction,
    ) -> ProviderIngestFutureV1<'a, Result<Self::Prepared, ProviderIngestIngressPrepareErrorV1>>
    {
        Box::pin(async { Err(ProviderIngestIngressPrepareErrorV1::Rejected) })
    }

    fn expose<'a>(
        &'a self,
        _prepared: Self::Prepared,
        _transaction: SignedTransaction,
    ) -> ProviderIngestFutureV1<'a, ProviderIngestIngressDispositionV1> {
        Box::pin(async { ProviderIngestIngressDispositionV1::Rejected })
    }

    fn observe<'a>(
        &'a self,
        _transaction_hash: [u8; 32],
    ) -> ProviderIngestFutureV1<'a, ProviderIngestTransactionObservationV1> {
        Box::pin(async { ProviderIngestTransactionObservationV1::Unknown })
    }
}

struct FixedClockV1(u64);

impl ProviderIngestClockV1 for FixedClockV1 {
    fn now_ms(&self) -> u64 {
        self.0
    }
}

fn account(seed: u8) -> AccountId {
    let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("account key");
    AccountId::new(key.public_key().clone())
}

fn outbox_policy() -> ProviderIngestOutboxPolicyV1 {
    ProviderIngestOutboxPolicyV1 {
        max_active_entries: 32,
        max_terminal_entries: 32,
        max_attempts: 4,
        checkpoint_max_bytes: 16 * 1024 * 1024,
        checkpoint_operation_timeout_ms: 250,
        source_lease_ttl_ms: 20,
        retry_base_delay_ms: 10_000,
        retry_max_delay_ms: 100_000,
        terminal_retention_blocks: 100,
        max_signed_transaction_bytes: 128 * 1024,
        max_status_page_size: 32,
    }
}

fn runtime_policy() -> ProviderIngestRuntimePolicyV1 {
    ProviderIngestRuntimePolicyV1 {
        max_page_rows: 16,
        max_pages_per_tick: 2,
        max_source_jobs_per_tick: 4,
        max_source_providers: 4,
        scan_interval_ms: 10,
        source_operation_timeout_ms: 250,
        source_lease_renew_interval_ms: 5,
        signer_timeout_ms: 100,
        ingress_timeout_ms: 100,
    }
}

#[tokio::test]
async fn post_admission_quarantine_survives_restart_with_shared_chunks() {
    let temp = tempfile::tempdir().expect("provider-ingest crash tempdir");
    let root = temp.path().canonicalize().expect("canonical crash tempdir");
    let storage_dir = root.join("storage");
    std::fs::create_dir(&storage_dir).expect("create storage directory");

    let network_id = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([0xA7; 32]),
    ));
    let cursor = ProviderIngestFinalizedCursorV1 {
        height: 8,
        block_hash: [8; 32],
    };
    let payload = b"ordinary shared SoraFS payload, not a Musubi bundle".to_vec();
    let plan = CarBuildPlan::single_file(&payload).expect("shared CAR plan");
    let stats = CarWriter::new(&plan, &payload)
        .expect("prepare shared CAR")
        .write_to(io::sink())
        .expect("measure shared CAR");
    let build_manifest = |pin_policy| {
        ManifestBuilder::new()
            .root_cid(stats.root_cids[0].clone())
            .dag_codec(DagCodecId(stats.dag_codec))
            .chunking_from_profile(plan.chunk_profile, BLAKE3_256_MULTIHASH_CODE)
            .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
            .por_root(compute_por_root(&payload, &plan).expect("shared PoR root"))
            .content_length(plan.content_length)
            .car_digest(*stats.car_archive_digest.as_bytes())
            .car_size(stats.car_size)
            .pin_policy(pin_policy)
            .build()
            .expect("shared manifest")
    };
    let manifest = build_manifest(ManifestPinPolicy::default());
    let shared_manifest = build_manifest(ManifestPinPolicy {
        retention_epoch: 1,
        ..ManifestPinPolicy::default()
    });
    assert_ne!(
        manifest.digest().expect("primary manifest digest"),
        shared_manifest.digest().expect("shared manifest digest")
    );

    let digest = ManifestDigest::new(
        *manifest
            .digest()
            .expect("primary manifest digest")
            .as_bytes(),
    );
    let root_cid =
        ManifestRootCid::try_from_slice(&manifest.root_cid).expect("canonical manifest CID");
    let chunker = ChunkerProfileHandle {
        profile_id: manifest.chunking.profile_id.0,
        namespace: manifest.chunking.namespace.clone(),
        name: manifest.chunking.name.clone(),
        semver: manifest.chunking.semver.clone(),
        multihash_code: manifest.chunking.multihash_code,
    };
    let commitment = MusubiArchiveCommitmentV1 {
        root_cid: root_cid.clone(),
        chunker: chunker.clone(),
        chunk_plan_digest: MusubiContentDigestV1::new(manifest.chunk_digest_sha3_256),
        por_root: MusubiContentDigestV1::new(manifest.por_root),
        content_length: manifest.content_length,
        car_digest: MusubiContentDigestV1::new(*stats.car_archive_digest.as_bytes()),
        car_size: stats.car_size,
        bundle_digest: MusubiContentDigestV1::new([0xD1; 32]),
        source_tree_digest: MusubiContentDigestV1::new([0xD2; 32]),
        descriptor_digest: MusubiContentDigestV1::new([0xD3; 32]),
        file_count: 1,
        chunk_count: u32::try_from(plan.chunks.len()).expect("chunk count fits u32"),
    };
    commitment.validate().expect("structural Musubi commitment");
    let archive = MusubiReplicationOrderArchiveBindingV1::new(
        ReplicationOrderId::new(ORDER_ID),
        commitment.archive_id(),
        commitment,
    );
    archive.validate().expect("Musubi archive binding");

    let mut pin = PinManifestRecord::new(
        digest,
        root_cid.clone(),
        chunker.clone(),
        manifest.chunk_digest_sha3_256,
        manifest.por_root,
        manifest.content_length,
        RegistryPinPolicy {
            min_replicas: 1,
            storage_class: RegistryStorageClass::Hot,
            retention_epoch: 0,
        },
        account(1),
        7,
        None,
        None,
        Metadata::default(),
    );
    pin.approve(7, Some([0xA8; 32]));
    let order_body = ReplicationOrderV1 {
        version: REPLICATION_ORDER_VERSION_V1,
        order_id: ORDER_ID,
        manifest_cid: manifest.root_cid.clone(),
        manifest_digest: *digest.as_bytes(),
        chunking_profile: chunker.to_handle(),
        target_replicas: 2,
        assignments: vec![
            ReplicationAssignmentV1 {
                provider_id: LOCAL_PROVIDER,
                slice_gib: 1,
                lane: None,
            },
            ReplicationAssignmentV1 {
                provider_id: SOURCE_PROVIDER,
                slice_gib: 1,
                lane: None,
            },
        ],
        issued_at: 100,
        deadline_at: 200,
        sla: ReplicationOrderSlaV1 {
            ingest_deadline_secs: 10,
            min_availability_percent_milli: 99_000,
            min_por_success_percent_milli: 99_000,
        },
        metadata: Vec::new(),
    };
    order_body.validate().expect("canonical replication order");
    let order = ReplicationOrderRecord {
        order_id: ReplicationOrderId::new(ORDER_ID),
        manifest_digest: digest,
        manifest_root_cid: root_cid,
        musubi_archive: Some(archive.archive_id),
        issued_by: account(1),
        issued_epoch: 7,
        deadline_epoch: 20,
        canonical_order: norito::to_bytes(&order_body).expect("encode replication order"),
        assignment_revision: 1,
        provider_completions: Vec::new(),
        status: ReplicationOrderStatus::Pending,
    };
    let authorization = FinalizedProviderIngestAuthorizationV1::from_finalized_musubi_state(
        cursor.height,
        cursor.block_hash,
        LOCAL_PROVIDER,
        ORDER_ID,
        *digest.as_bytes(),
        manifest.root_cid.clone(),
        chunker.to_handle(),
        manifest.chunk_digest_sha3_256,
        manifest.por_root,
        manifest.content_length,
        FinalizedProviderIngestMusubiContextV1::new(network_id, archive.archive_id)
            .expect("Musubi authorization context"),
    )
    .expect("finalized Musubi authorization");
    let ledger = CrashRestartLedgerV1 {
        network_id,
        cursor,
        pin,
        order,
        archive,
    };

    let checkpoint = Arc::new(CrashRestartCheckpointRuntimeV1 {
        latest: Mutex::new(None),
    });
    let outbox_policy = outbox_policy();
    {
        let outbox = ProviderIngestOutbox::open_with_checkpoint_authority(
            storage_dir.join(PROVIDER_INGEST_OUTBOX_FILE_V1),
            outbox_policy,
            CrashRestartCheckpointRuntimeV1::binding(),
            checkpoint.clone(),
        )
        .expect("open sealed crash outbox");
        outbox
            .enqueue(authorization.clone())
            .expect("enqueue exact authorization");
        outbox
            .claim_source(
                authorization.job_id(),
                ProviderIngestClaimOwnerV1::new([0xC1; 32]).expect("first claim owner"),
                100,
                cursor,
            )
            .expect("persist pre-crash source claim");
        assert!(matches!(
            outbox
                .status(authorization.job_id())
                .expect("pre-crash status")
                .state,
            ProviderIngestDeliveryStateV1::SourceClaimed { attempts: 0, .. }
        ));
    }

    let plain_config = StorageConfig::builder()
        .enabled(true)
        .provider_id(Some(ProviderId::new(LOCAL_PROVIDER)))
        .data_dir(storage_dir.clone())
        .build();
    let (manifest_id, shared_manifest_id) = {
        let node = NodeHandle::try_new(plain_config).expect("open pre-crash storage");
        let mut shared_reader = payload.as_slice();
        let shared_manifest_id = node
            .ingest_manifest(&shared_manifest, &plan, &mut shared_reader)
            .expect("admit pre-existing shared object");
        let mut primary_reader = payload.as_slice();
        let manifest_id = node
            .ingest_manifest(&manifest, &plan, &mut primary_reader)
            .expect("admit quarantined object");
        assert_ne!(manifest_id, shared_manifest_id);
        assert_eq!(node.stored_manifests().expect("stored manifests").len(), 2);
        (manifest_id, shared_manifest_id)
    };

    let configured = StorageConfig::builder()
        .enabled(true)
        .provider_id(Some(ProviderId::new(LOCAL_PROVIDER)))
        .provider_ingest_outbox_policy(Some(outbox_policy))
        .provider_ingest_checkpoint_provider(Some(CrashRestartCheckpointRuntimeV1::binding()))
        .data_dir(storage_dir)
        .build();
    let runtime_deps =
        || NodeRuntimeDeps::default().with_provider_ingest_checkpoint_runtime(checkpoint.clone());
    let node = NodeHandle::try_new_with_runtime_deps(configured.clone(), runtime_deps())
        .expect("restart node with sealed outbox");
    let fetch = Arc::new(CrashRestartFetchV1 {
        calls: AtomicU64::new(0),
    });
    let storage = Arc::new(NativeProviderIngestLocalStorageV1::new(
        node.clone(),
        Duration::from_secs(1),
    ));
    let mut runtime = node
        .build_provider_ingest_runtime(
            network_id,
            ProviderIngestClaimOwnerV1::new([0xC2; 32]).expect("restart claim owner"),
            runtime_policy(),
            Arc::new(ledger),
            Arc::clone(&fetch),
            storage,
            Arc::new(NeverBuildCompletionV1),
            Arc::new(NeverResolveSignerV1),
            Arc::new(NeverIngressV1),
            Arc::new(FixedClockV1(
                100_u64
                    .checked_add(outbox_policy.source_lease_ttl_ms)
                    .and_then(|now| now.checked_add(1))
                    .expect("post-lease time"),
            )),
        )
        .expect("build restarted provider-ingest runtime");
    let outcome = runtime.tick().await.expect("recover interrupted ingest");
    assert_eq!(fetch.calls.load(Ordering::SeqCst), 0);
    assert_eq!(outcome.source_jobs_claimed, 1);
    assert_eq!(outcome.manifests_stored, 0);
    let terminal = node
        .finalized_provider_ingest_status_page(None, 1)
        .expect("post-recovery status page")
        .rows
        .pop()
        .expect("post-recovery terminal");
    assert_eq!(
        terminal.state,
        ProviderIngestDeliveryStateV1::DeadLetter {
            attempts: 2,
            reason: ProviderIngestDeadLetterReasonV1::StorageRejected,
            last_failure_class: ProviderIngestFailureClassV1::StorageRejected,
            observed_finalized_cursor: cursor,
        }
    );
    drop(runtime);
    drop(node);

    let reopened = NodeHandle::try_new_with_runtime_deps(configured, runtime_deps())
        .expect("reopen terminal node");
    assert_eq!(
        reopened
            .finalized_provider_ingest_status_page(None, 1)
            .expect("reopened status page")
            .rows[0]
            .state,
        terminal.state
    );
    assert_eq!(
        reopened
            .read_payload_range(&manifest_id, 0, payload.len())
            .expect("read quarantined manifest"),
        payload
    );
    assert_eq!(
        reopened
            .read_payload_range(&shared_manifest_id, 0, payload.len())
            .expect("read shared manifest"),
        payload
    );
    let mut replay_reader = payload.as_slice();
    assert!(matches!(
        reopened.ingest_manifest(&manifest, &plan, &mut replay_reader),
        Err(NodeStorageError::Storage(StorageError::ManifestExists {
            manifest_id: existing,
        })) if existing == manifest_id
    ));
}
