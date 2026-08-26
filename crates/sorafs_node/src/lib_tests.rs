use super::*;
use crate::config::RuntimeRetentionPolicy;
use crate::por::test_support::{
    resign_sample_proof as resign_por_sample_proof,
    resign_sample_verdict as resign_por_sample_verdict,
    sample_auditor_keys as por_sample_auditor_keys, sample_challenge as por_sample_challenge,
    sample_proof as por_sample_proof, sample_provider_key as por_sample_provider_key,
    sample_replay_archive_head as por_sample_replay_archive_head,
    sample_replay_archive_record_and_head as por_sample_replay_archive_record_and_head,
    sample_verdict as por_sample_verdict,
};
use crate::repair_ledger_projection::RepairLedgerTaskProjectionBuilderV1;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature as IrohaSignature, SignatureOf};
use iroha_data_model::{
    block::BlockHeader,
    isi::{InstructionBox, sorafs::CompleteReplicationOrder},
    metadata::Metadata,
    name::Name,
    sorafs::{
        capacity::{CapacityDeclarationRecord, ProviderId},
        moderation::{
            ADVERSARIAL_CORPUS_VERSION_V1, AdversarialCorpusManifestV1,
            AdversarialPerceptualFamilyV1, AdversarialPerceptualVariantV1,
            MODERATION_REPRO_MANIFEST_VERSION_V1, MODERATION_TRUST_POLICY_VERSION_V1,
            ModerationModelFingerprintV1, ModerationReproBodyV1, ModerationReproManifestV1,
            ModerationReproSignatureV1, ModerationSeedMaterialV1, ModerationThresholdsV1,
            ModerationTrustPolicyBodyV1, ModerationTrustPolicySignatureV1, ModerationTrustPolicyV1,
            ModerationTrustedSignerV1,
        },
        moderation_ledger::{
            REPAIR_LEDGER_TASK_VERSION_V1, RepairFinalizedCursorV1, RepairFinalizedStatusV1,
            RepairLedgerStatusV1, RepairLedgerTaskPageV1, RepairLedgerTaskV1,
            RepairLedgerTerminalKindV1, sorafs_repair_task_id_v1,
        },
        pin_registry::{
            ChunkerProfileHandle, ManifestDigest, ManifestRootCid, PinManifestFinalizedCursorV1,
            PinManifestRecord, PinPolicy as RegistryPinPolicy, ProviderIngestCompletionAuthorityV1,
            ProviderIngestFinalizedAnchorV1, ReplicationOrderId, ReplicationOrderRecord,
            ReplicationOrderStatus, StorageClass,
        },
        reserve::{ReserveDuration, ReservePolicyV1, ReserveTier},
    },
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use iroha_telemetry::metrics::global_or_default;
use norito::to_bytes;
use sorafs_car::{CarBuildPlan, CarWriter, compute_chunk_plan_digest_sha3};
use sorafs_manifest::PorReportIsoWeek;
use sorafs_manifest::{
    DagCodecId, ManifestBuilder, PinPolicy, REPUTATION_PROVIDER_INPUT_VERSION_V1,
    REPUTATION_PROVIDER_METRICS_VERSION_V1, REPUTATION_SCORING_EVIDENCE_VERSION_V1,
    REPUTATION_SNAPSHOT_TRUST_POLICY_VERSION_V1, REPUTATION_TRUSTED_SIGNER_VERSION_V1,
    ReputationProviderInputV1, ReputationProviderMetricsV1, ReputationReserveStageV1,
    ReputationScoringEvidenceV1, ReputationSnapshotSignatureV1, ReputationSnapshotTrustPolicyV1,
    ReputationTrustedSignerV1, ReputationWeightsV1, SIGNED_REPUTATION_SNAPSHOT_VERSION_V1,
    SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1, SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1,
    SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1, SORAFS_RECONCILIATION_REPORT_VERSION_V1,
    SignedReputationSnapshotV1, SoraFsAppealFinanceAccountFlowV1, SoraFsAppealFinanceJurorPayoutV1,
    SoraFsAppealFinanceOutcomeV1, SoraFsAppealFinanceReportV1,
    SoraFsAppealFinanceSettlementReceiptV1, SoraFsAppealFinanceWeeklyRollupV1,
    SoraFsModerationBallotGovernanceEventKindV1, SoraFsModerationBallotGovernanceEventV1,
    SoraFsModerationBallotGovernanceTallyV1, SoraFsModerationVoteChoiceV1,
    SoraFsModerationVoteCountsV1, SorafsReconciliationReportV1, build_reputation_snapshot,
    capacity::{
        CAPACITY_DECLARATION_VERSION_V1, CapacityDeclarationV1, CapacityMetadataEntry,
        ChunkerCommitmentV1, LaneCommitmentV1, REPLICATION_ORDER_VERSION_V1,
        ReplicationAssignmentV1, ReplicationOrderSlaV1, ReplicationOrderV1,
    },
    deal::DealSettlementV1,
    repair::{
        GC_AUDIT_EVENT_VERSION_V1, GC_AUDIT_PAYLOAD_VERSION_V1, GcAuditEventV1,
        REPAIR_EVIDENCE_VERSION_V1, REPAIR_REPORT_VERSION_V1, RepairCauseV1, RepairEvidenceV1,
        RepairManualCauseV1, RepairReportV1, RepairTicketId,
    },
};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    str::FromStr,
    sync::{
        Arc, Barrier, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tempfile::TempDir;
include!("lib_early_test_support.rs");
macro_rules! assert_node_init_variant {
    ($variant:ident => $result:expr) => {
        assert!(matches!($result, Err(NodeInitError::$variant { .. })));
    };
}
macro_rules! assert_checkpoint_component {
    ($component:literal => $result:expr) => {
        assert!(matches!(
            $result,
            Err(NodeInitError::Checkpoint {
                component: $component,
                ..
            })
        ));
    };
}
macro_rules! gc_runtime_read {
        ($runtime:expr, $context:literal => $($tail:tt)+) => {
            $runtime.read().expect($context).$($tail)+
        };
    }
fn enabled_storage_builder(data_dir: PathBuf) -> config::StorageConfigBuilder {
    StorageConfig::builder().enabled(true).data_dir(data_dir)
}
fn startup_por_archive_binding(seed: u8) -> PorFinalizedReplayArchiveBindingV1 {
    let key_pair =
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("test Ed25519 key");
    let public_key = key_pair.public_key().to_bytes().1;
    let mut signing_public_key = [0_u8; 32];
    signing_public_key.copy_from_slice(public_key);
    PorFinalizedReplayArchiveBindingV1::try_new(
        [seed.wrapping_add(1); 32],
        7,
        [seed.wrapping_add(2); 32],
        signing_public_key,
    )
    .expect("valid archive binding")
}
fn storage_config_with_por_archive(
    binding: PorFinalizedReplayArchiveBindingV1,
) -> (StorageConfig, TempDir) {
    let temp_dir = tempfile::tempdir().expect("create replay-archive temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let policy = config::PorReplayArchivePolicyV1::try_new(
            "hsm://sorafs/por-replay-archive/primary",
            binding,
            Duration::from_secs(1),
            16,
            iroha_config::parameters::defaults::sorafs::storage::por_replay_archive::MAX_SUCCESSOR_RECEIPTS,
            iroha_config::parameters::defaults::sorafs::storage::por_replay_archive::MAX_SUCCESSOR_PROOF_BYTES,
        )
        .expect("valid replay-archive worker policy");
    let config = enabled_storage_builder(root.join("storage"))
        .por_replay_archive_policy(Some(policy))
        .build();
    (config, temp_dir)
}
#[derive(Debug)]
struct StartupPorReplayArchive {
    handle: &'static str,
    first_binding: PorFinalizedReplayArchiveBindingV1,
    later_binding: Option<PorFinalizedReplayArchiveBindingV1>,
    readiness_error: Option<PorFinalizedReplayArchiveExternalErrorV1>,
    current_head: Option<PorFinalizedReplayArchiveReceiptV1>,
    lookup_result: Option<PorFinalizedReplayArchiveLookupV1>,
    binding_calls: AtomicUsize,
    lookup_calls: AtomicUsize,
}
impl StartupPorReplayArchive {
    fn exact(binding: PorFinalizedReplayArchiveBindingV1) -> Self {
        Self {
            handle: "hsm://sorafs/por-replay-archive/primary",
            first_binding: binding,
            later_binding: None,
            readiness_error: None,
            current_head: None,
            lookup_result: None,
            binding_calls: AtomicUsize::new(0),
            lookup_calls: AtomicUsize::new(0),
        }
    }
}
impl PorFinalizedReplayArchiveV1 for StartupPorReplayArchive {
    fn runtime_handle(&self) -> &str {
        self.handle
    }
    fn binding(
        &self,
    ) -> Result<PorFinalizedReplayArchiveBindingV1, PorFinalizedReplayArchiveExternalErrorV1> {
        let call = self.binding_calls.fetch_add(1, Ordering::Relaxed);
        Ok(if call == 0 {
            self.first_binding
        } else {
            self.later_binding.unwrap_or(self.first_binding)
        })
    }
    fn check_readiness(&self) -> Result<(), PorFinalizedReplayArchiveExternalErrorV1> {
        self.readiness_error.map_or(Ok(()), Err)
    }
    fn current_head(
        &self,
    ) -> Result<Option<PorFinalizedReplayArchiveReceiptV1>, PorFinalizedReplayArchiveExternalErrorV1>
    {
        Ok(self.current_head)
    }
    fn append(
        &self,
        _record: &PorFinalizedReplayArchiveRecordV1,
        _expected_previous_head: Option<[u8; 32]>,
    ) -> Result<PorFinalizedReplayArchiveReceiptV1, PorFinalizedReplayArchiveExternalErrorV1> {
        Err(PorFinalizedReplayArchiveExternalErrorV1::Rejected)
    }
    fn lookup(
        &self,
        challenge_id: [u8; 32],
        expected_checkpoint_head: PorFinalizedReplayArchiveReceiptV1,
        _proof_bounds: PorFinalizedReplayArchiveProofBoundsV1,
    ) -> Result<PorFinalizedReplayArchiveLookupV1, PorFinalizedReplayArchiveExternalErrorV1> {
        self.lookup_calls.fetch_add(1, Ordering::Relaxed);
        if self.current_head != Some(expected_checkpoint_head) {
            return Err(PorFinalizedReplayArchiveExternalErrorV1::Rejected);
        }
        match self.lookup_result.as_ref() {
            Some(PorFinalizedReplayArchiveLookupV1::Found(readback))
                if readback.record.challenge_id() == challenge_id =>
            {
                Ok(PorFinalizedReplayArchiveLookupV1::Found(readback.clone()))
            }
            Some(PorFinalizedReplayArchiveLookupV1::Absent(absence))
                if absence.challenge_id() == challenge_id =>
            {
                Ok(PorFinalizedReplayArchiveLookupV1::Absent(absence.clone()))
            }
            _ => Err(PorFinalizedReplayArchiveExternalErrorV1::Rejected),
        }
    }
}
#[derive(Debug, Clone, Copy)]
enum PostQualificationDrift {
    Handle,
    Readiness,
}
#[derive(Debug)]
struct PostQualificationDriftArchive {
    inner: StartupPorReplayArchive,
    drift: PostQualificationDrift,
    armed: AtomicBool,
    calls_after_arm: AtomicUsize,
}
impl PostQualificationDriftArchive {
    fn new(binding: PorFinalizedReplayArchiveBindingV1, drift: PostQualificationDrift) -> Self {
        Self {
            inner: StartupPorReplayArchive::exact(binding),
            drift,
            armed: AtomicBool::new(false),
            calls_after_arm: AtomicUsize::new(0),
        }
    }
    fn arm(&self) {
        self.calls_after_arm.store(0, Ordering::Relaxed);
        self.armed.store(true, Ordering::Relaxed);
    }
    fn drifted_on_second_call(&self, operation: PostQualificationDrift) -> bool {
        matches!(
            (self.drift, operation),
            (
                PostQualificationDrift::Handle,
                PostQualificationDrift::Handle
            ) | (
                PostQualificationDrift::Readiness,
                PostQualificationDrift::Readiness
            )
        ) && self.armed.load(Ordering::Relaxed)
            && self.calls_after_arm.fetch_add(1, Ordering::Relaxed) != 0
    }
}
impl PorFinalizedReplayArchiveV1 for PostQualificationDriftArchive {
    fn runtime_handle(&self) -> &str {
        if self.drifted_on_second_call(PostQualificationDrift::Handle) {
            "test://substituted/por-replay-archive"
        } else {
            self.inner.runtime_handle()
        }
    }
    fn binding(
        &self,
    ) -> Result<PorFinalizedReplayArchiveBindingV1, PorFinalizedReplayArchiveExternalErrorV1> {
        self.inner.binding()
    }
    fn check_readiness(&self) -> Result<(), PorFinalizedReplayArchiveExternalErrorV1> {
        if self.drifted_on_second_call(PostQualificationDrift::Readiness) {
            Err(PorFinalizedReplayArchiveExternalErrorV1::Unavailable)
        } else {
            self.inner.check_readiness()
        }
    }
    fn current_head(
        &self,
    ) -> Result<Option<PorFinalizedReplayArchiveReceiptV1>, PorFinalizedReplayArchiveExternalErrorV1>
    {
        self.inner.current_head()
    }
    fn append(
        &self,
        record: &PorFinalizedReplayArchiveRecordV1,
        expected_previous_head: Option<[u8; 32]>,
    ) -> Result<PorFinalizedReplayArchiveReceiptV1, PorFinalizedReplayArchiveExternalErrorV1> {
        self.inner.append(record, expected_previous_head)
    }
    fn lookup(
        &self,
        challenge_id: [u8; 32],
        expected_checkpoint_head: PorFinalizedReplayArchiveReceiptV1,
        proof_bounds: PorFinalizedReplayArchiveProofBoundsV1,
    ) -> Result<PorFinalizedReplayArchiveLookupV1, PorFinalizedReplayArchiveExternalErrorV1> {
        self.inner
            .lookup(challenge_id, expected_checkpoint_head, proof_bounds)
    }
}
#[test]
fn node_archive_injection_is_exact_and_standard_challenge_path_uses_it() {
    let binding = startup_por_archive_binding(0xD1);
    let (config, _temp_dir) = storage_config_with_por_archive(binding);
    let archive = Arc::new(StartupPorReplayArchive::exact(binding));
    let handle = NodeHandle::try_new_with_policies_and_runtime_deps(
        config,
        RepairConfig::default(),
        GcConfig::default(),
        NodeRuntimeDeps::default().with_por_finalized_replay_archive(archive.clone()),
    )
    .expect("exact replay archive starts");
    handle
        .record_por_challenge(&por_sample_challenge())
        .expect("standard challenge path records fresh local state");
    assert_eq!(
        archive.lookup_calls.load(Ordering::Relaxed),
        0,
        "without a checkpointed archive head the node must not fabricate an absence lookup"
    );
    assert!(
        archive.binding_calls.load(Ordering::Relaxed) >= 4,
        "startup and route admission must each observe a stable binding twice"
    );
}
#[test]
fn node_archive_operation_rolls_back_on_post_handle_or_readiness_drift() {
    for (seed, drift) in [
        (0xD5, PostQualificationDrift::Handle),
        (0xD6, PostQualificationDrift::Readiness),
    ] {
        let binding = startup_por_archive_binding(seed);
        let (config, _temp_dir) = storage_config_with_por_archive(binding);
        let archive = Arc::new(PostQualificationDriftArchive::new(binding, drift));
        let node = NodeHandle::try_new_with_policies_and_runtime_deps(
            config,
            RepairConfig::default(),
            GcConfig::default(),
            NodeRuntimeDeps::default().with_por_finalized_replay_archive(archive.clone()),
        )
        .expect("exact provider starts before drift is armed");
        let before = node.por.checkpoint();
        archive.arm();
        assert!(
            node.record_por_challenge(&por_sample_challenge()).is_err(),
            "post-operation {drift:?} drift must fail closed"
        );
        assert_eq!(
            node.por.checkpoint(),
            before,
            "post-operation {drift:?} drift must roll local state back"
        );
    }
}
#[test]
fn node_archive_startup_rejects_nonempty_head_without_a_restored_local_head() {
    let (binding, current_head) = por_sample_replay_archive_head(0xD3);
    let (config, _temp_dir) = storage_config_with_por_archive(binding);
    let mut archive = StartupPorReplayArchive::exact(binding);
    archive.current_head = Some(current_head);
    assert_node_init_variant!(PorReplayArchive =>
        NodeHandle::try_new_with_policies_and_runtime_deps(
            config,
            RepairConfig::default(),
            GcConfig::default(),
            NodeRuntimeDeps::default().with_por_finalized_replay_archive(Arc::new(archive)),
        )
    );
}
#[test]
fn node_archive_startup_reconciles_and_persists_an_exact_first_append_prefix() {
    let (binding, record, current_head) = por_sample_replay_archive_record_and_head(0xD4);
    let (config, _temp_dir) = storage_config_with_por_archive(binding);
    let initial_archive = Arc::new(StartupPorReplayArchive::exact(binding));
    let node = NodeHandle::try_new_with_policies_and_runtime_deps(
        config.clone(),
        RepairConfig::default(),
        GcConfig::default(),
        NodeRuntimeDeps::default().with_por_finalized_replay_archive(initial_archive),
    )
    .expect("start against an empty exact archive");
    let challenge = por_sample_challenge();
    let proof = por_sample_proof(&challenge);
    let verdict = por_sample_verdict(&challenge, proof.proof_digest());
    node.record_por_challenge(&challenge)
        .expect("checkpoint challenge");
    node.record_por_proof(&proof, &por_sample_provider_key())
        .expect("checkpoint proof");
    node.record_por_verdict(&verdict, &por_sample_auditor_keys(), 1)
        .expect("checkpoint finalized terminal");
    let work = node
        .next_por_reputation_terminal_work()
        .expect("read retained terminal")
        .expect("one retained terminal");
    node.acknowledge_por_reputation_terminal(work.sequence, work.work_digest)
        .expect("checkpoint acknowledgement before external append");
    assert!(node.por.checkpoint().replay_archive_receipt().is_none());
    drop(node);
    let mut live_ahead = StartupPorReplayArchive::exact(binding);
    live_ahead.current_head = Some(current_head);
    live_ahead.lookup_result = Some(PorFinalizedReplayArchiveLookupV1::Found(Box::new(
        PorFinalizedReplayArchiveReadbackV1 {
            record,
            receipt: current_head,
            successor_receipts: Vec::new(),
        },
    )));
    let reopened = NodeHandle::try_new_with_policies_and_runtime_deps(
        config.clone(),
        RepairConfig::default(),
        GcConfig::default(),
        NodeRuntimeDeps::default().with_por_finalized_replay_archive(Arc::new(live_ahead)),
    )
    .expect("reconcile an exact acknowledged first-append crash window");
    let reconciled = reopened.por.checkpoint();
    assert!(reconciled.has_no_finalized_challenges());
    assert_eq!(reconciled.replay_archive_receipt(), Some(current_head));
    drop(reopened);
    let mut persisted = StartupPorReplayArchive::exact(binding);
    persisted.current_head = Some(current_head);
    let reopened_again = NodeHandle::try_new_with_policies_and_runtime_deps(
        config,
        RepairConfig::default(),
        GcConfig::default(),
        NodeRuntimeDeps::default().with_por_finalized_replay_archive(Arc::new(persisted)),
    )
    .expect("reopen from the reconciled local checkpoint without another lookup");
    assert_eq!(
        reopened_again.por.checkpoint().replay_archive_receipt(),
        Some(current_head)
    );
}
#[test]
fn node_archive_injection_rejects_missing_unrequested_substituted_and_stale_adapters() {
    let binding = startup_por_archive_binding(0xD2);
    let (missing_config, _missing_dir) = storage_config_with_por_archive(binding);
    assert_node_init_variant!(PorReplayArchive =>
        NodeHandle::try_new_with_policies_and_runtime_deps(
            missing_config,
            RepairConfig::default(),
            GcConfig::default(),
            NodeRuntimeDeps::default(),
        )
    );
    let (unrequested_config, _unrequested_dir) = storage_config_with_temp_dir();
    assert_node_init_variant!(PorReplayArchive =>
        NodeHandle::try_new_with_policies_and_runtime_deps(
            unrequested_config,
            RepairConfig::default(),
            GcConfig::default(),
            NodeRuntimeDeps::default().with_por_finalized_replay_archive(Arc::new(
                StartupPorReplayArchive::exact(binding),
            )),
        )
    );
    let (substituted_config, _substituted_dir) = storage_config_with_por_archive(binding);
    let mut substituted = StartupPorReplayArchive::exact(binding);
    substituted.handle = "hsm://sorafs/por-replay-archive/substituted";
    assert_node_init_variant!(PorReplayArchive =>
        NodeHandle::try_new_with_policies_and_runtime_deps(
            substituted_config,
            RepairConfig::default(),
            GcConfig::default(),
            NodeRuntimeDeps::default().with_por_finalized_replay_archive(Arc::new(substituted)),
        )
    );
    let (stale_config, _stale_dir) = storage_config_with_por_archive(binding);
    let mut stale = StartupPorReplayArchive::exact(binding);
    stale.readiness_error = Some(PorFinalizedReplayArchiveExternalErrorV1::Rejected);
    assert_node_init_variant!(PorReplayArchive =>
        NodeHandle::try_new_with_policies_and_runtime_deps(
            stale_config,
            RepairConfig::default(),
            GcConfig::default(),
            NodeRuntimeDeps::default().with_por_finalized_replay_archive(Arc::new(stale)),
        )
    );
}
fn xor(value: &str) -> XorQuantity {
    value.parse().expect("canonical XOR quantity")
}
fn manifest_builder_for_plan(payload: &[u8], plan: &CarBuildPlan) -> ManifestBuilder {
    let car_stats = CarWriter::new(plan, payload)
        .expect("prepare canonical fixture CAR")
        .write_to(std::io::sink())
        .expect("compute canonical fixture CAR");
    ManifestBuilder::new()
        .root_cid(
            car_stats
                .root_cids
                .first()
                .cloned()
                .expect("fixture CAR root"),
        )
        .dag_codec(DagCodecId(car_stats.dag_codec))
        .chunking_from_profile(
            plan.chunk_profile,
            sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
        )
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(
            sorafs_car::compute_por_root(payload, plan).expect("derive canonical fixture PoR root"),
        )
        .content_length(plan.content_length)
        .car_digest(*car_stats.car_archive_digest.as_bytes())
        .car_size(car_stats.car_size)
}
fn storage_config_with_temp_dir() -> (StorageConfig, TempDir) {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let cfg = enabled_storage_builder(root.join("storage")).build();
    (cfg, temp_dir)
}
fn test_quarantine_key_provider_config()
-> iroha_config::parameters::actual::SorafsModerationQuarantineKeyProviderBinding {
    test_quarantine_key_provider_config_for(TEST_QUARANTINE_KEY_PROVIDER_QUALIFICATION)
}
fn test_quarantine_key_provider_config_for(
    qualification: ModerationQuarantineKeyProviderQualificationV1,
) -> iroha_config::parameters::actual::SorafsModerationQuarantineKeyProviderBinding {
    iroha_config::parameters::actual::SorafsModerationQuarantineKeyProviderBinding {
        handle: TEST_QUARANTINE_KEY_PROVIDER_HANDLE.to_owned(),
        revision: qualification.revision(),
        policy_digest: qualification.policy_digest(),
    }
}
fn storage_config_with_temp_dir_and_quarantine_key_provider() -> (StorageConfig, TempDir) {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let cfg = enabled_storage_builder(root.join("storage"))
        .moderation_quarantine_key_provider(Some(test_quarantine_key_provider_config()))
        .build();
    (cfg, temp_dir)
}
#[derive(Debug)]
struct StartupProviderIngestCheckpointRuntime {
    handle: String,
    qualification: ProviderIngestCheckpointProviderQualificationV1,
    latest: Mutex<Option<ProviderIngestSealedCheckpointRecordV1>>,
}
impl StartupProviderIngestCheckpointRuntime {
    fn new(handle: &str, seed: u8) -> Self {
        Self {
            handle: handle.to_owned(),
            qualification: ProviderIngestCheckpointProviderQualificationV1::new(1, [seed; 32]),
            latest: Mutex::new(None),
        }
    }
}
impl ProviderIngestCheckpointRuntimeV1 for StartupProviderIngestCheckpointRuntime {
    fn handle(&self) -> &str {
        &self.handle
    }
    fn qualification(
        &self,
    ) -> Result<
        ProviderIngestCheckpointProviderQualificationV1,
        ProviderIngestCheckpointExternalErrorV1,
    > {
        Ok(self.qualification)
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
fn provider_ingest_checkpoint_binding(
    handle: &str,
    seed: u8,
) -> ProviderIngestCheckpointProviderBindingV1 {
    ProviderIngestCheckpointProviderBindingV1 {
        handle: handle.to_owned(),
        revision: 1,
        policy_digest: [seed; 32],
    }
}
fn storage_config_with_provider_ingest_checkpoint(
    handle: &str,
    seed: u8,
) -> (StorageConfig, TempDir) {
    let temp_dir = tempfile::tempdir().expect("create provider-ingest startup temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let config = StorageConfig::builder()
        .enabled(true)
        .provider_id(Some(ProviderId::new([seed; 32])))
        .provider_ingest_outbox_policy(Some(ProviderIngestOutboxPolicyV1::default()))
        .provider_ingest_checkpoint_provider(Some(provider_ingest_checkpoint_binding(handle, seed)))
        .data_dir(root.join("storage"))
        .build();
    (config, temp_dir)
}
fn node_with_temp_storage() -> (NodeHandle, TempDir) {
    let (config, temp_dir) = storage_config_with_temp_dir();
    (NodeHandle::new(config), temp_dir)
}
fn node_with_temp_storage_and_recording_publisher() -> (NodeHandle, Arc<RecordingPublisher>, TempDir)
{
    let (handle, temp_dir) = node_with_temp_storage();
    let publisher = Arc::new(RecordingPublisher::default());
    let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
    handle.set_governance_publisher(trait_publisher);
    (handle, publisher, temp_dir)
}
struct FinalizedProviderIngestFixture {
    config: StorageConfig,
    provider_id: ProviderId,
    declaration: CapacityDeclarationRecord,
    order: ReplicationOrderRecord,
    finalized_pin: PinManifestFinalizedRecordV1,
    manifest: ManifestV1,
    plan: CarBuildPlan,
    payload: Vec<u8>,
    capacity_cursor: CapacityFinalizedCursorV1,
    ingest_cursor: ProviderIngestFinalizedCursorV1,
}
fn finalized_provider_ingest_fixture(seed: u8) -> (FinalizedProviderIngestFixture, TempDir) {
    let temp_dir = tempfile::tempdir().expect("create provider-ingest temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let provider_id = ProviderId::new([seed; 32]);
    let config = StorageConfig::builder()
        .enabled(true)
        .provider_id(Some(provider_id))
        .provider_ingest_outbox_policy(Some(ProviderIngestOutboxPolicyV1::default()))
        .data_dir(root.join("storage"))
        .build();
    let authority_key = KeyPair::try_from_seed(vec![seed.wrapping_add(1); 32], Algorithm::Ed25519)
        .expect("provider-ingest fixture authority key");
    let authority = AccountId::new(authority_key.public_key().clone());
    let declaration_body = CapacityDeclarationV1 {
        version: CAPACITY_DECLARATION_VERSION_V1,
        provider_id: *provider_id.as_bytes(),
        stake: sorafs_manifest::provider_advert::StakePointer {
            pool_id: [seed.wrapping_add(2); 32],
            stake_amount: xor("1"),
        },
        committed_capacity_gib: 8,
        chunker_commitments: vec![ChunkerCommitmentV1 {
            profile_id: "sorafs.sf1@1.0.0".into(),
            profile_aliases: None,
            committed_gib: 8,
            capability_refs: Vec::new(),
        }],
        lane_commitments: vec![LaneCommitmentV1 {
            lane_id: "default".into(),
            max_gib: 8,
        }],
        pricing: None,
        valid_from: 1,
        valid_until: 100,
        metadata: Vec::new(),
    };
    let declaration = CapacityDeclarationRecord::new(
        provider_id,
        to_bytes(&declaration_body).expect("encode provider-ingest capacity declaration"),
        declaration_body.committed_capacity_gib,
        1,
        declaration_body.valid_from,
        declaration_body.valid_until,
        Metadata::default(),
    );
    let payload = format!("finalized-provider-ingest-{seed}").into_bytes();
    let plan = CarBuildPlan::single_file(&payload).expect("provider-ingest CAR plan");
    let manifest = manifest_builder_for_plan(&payload, &plan)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("provider-ingest manifest");
    let root_cid = manifest.root_cid.clone();
    let manifest_digest: [u8; 32] = manifest.digest().expect("manifest digest").into();
    let manifest_root_cid =
        ManifestRootCid::try_from_slice(&manifest.root_cid).expect("canonical manifest CID");
    let chunker = ChunkerProfileHandle {
        profile_id: manifest.chunking.profile_id.0,
        namespace: manifest.chunking.namespace.clone(),
        name: manifest.chunking.name.clone(),
        semver: manifest.chunking.semver.clone(),
        multihash_code: manifest.chunking.multihash_code,
    };
    let order_id = [seed.wrapping_add(4); 32];
    let order_body = ReplicationOrderV1 {
        version: REPLICATION_ORDER_VERSION_V1,
        order_id,
        manifest_cid: root_cid,
        manifest_digest,
        chunking_profile: chunker.to_handle(),
        target_replicas: 1,
        assignments: vec![ReplicationAssignmentV1 {
            provider_id: *provider_id.as_bytes(),
            slice_gib: 1,
            lane: Some("default".into()),
        }],
        issued_at: 2,
        deadline_at: 50,
        sla: ReplicationOrderSlaV1 {
            ingest_deadline_secs: 30,
            min_availability_percent_milli: 99_000,
            min_por_success_percent_milli: 99_000,
        },
        metadata: Vec::new(),
    };
    let order = ReplicationOrderRecord {
        order_id: ReplicationOrderId::new(order_id),
        manifest_digest: ManifestDigest::new(manifest_digest),
        manifest_root_cid,
        musubi_archive: None,
        issued_by: authority.clone(),
        issued_epoch: order_body.issued_at,
        deadline_epoch: order_body.deadline_at,
        canonical_order: to_bytes(&order_body).expect("encode replication order"),
        assignment_revision: 1,
        provider_completions: Vec::new(),
        status: ReplicationOrderStatus::Pending,
    };
    let mut pin = PinManifestRecord::new(
        ManifestDigest::new(manifest_digest),
        manifest_root_cid,
        chunker,
        manifest.chunk_digest_sha3_256,
        manifest.por_root,
        manifest.content_length,
        RegistryPinPolicy {
            min_replicas: manifest.pin_policy.min_replicas,
            storage_class: StorageClass::Hot,
            retention_epoch: manifest.pin_policy.retention_epoch,
        },
        authority,
        2,
        None,
        None,
        Metadata::default(),
    );
    pin.approve(3, Some([seed.wrapping_add(5); 32]));
    let capacity_cursor = CapacityFinalizedCursorV1 {
        height: 9,
        block_hash: [seed.wrapping_add(6); 32],
    };
    let ingest_cursor = ProviderIngestFinalizedCursorV1 {
        height: capacity_cursor.height,
        block_hash: capacity_cursor.block_hash,
    };
    let finalized_pin = PinManifestFinalizedRecordV1 {
        finalized_cursor: PinManifestFinalizedCursorV1 {
            height: capacity_cursor.height,
            block_hash: capacity_cursor.block_hash,
        },
        manifest: pin,
    };
    (
        FinalizedProviderIngestFixture {
            config,
            provider_id,
            declaration,
            order,
            finalized_pin,
            manifest,
            plan,
            payload,
            capacity_cursor,
            ingest_cursor,
        },
        temp_dir,
    )
}
#[test]
fn provider_ingest_outbox_is_not_opened_when_runtime_policy_is_absent() {
    struct DisabledCaptureLedger;
    impl ProviderIngestCompletedMusubiSignedCaptureLedgerV1 for DisabledCaptureLedger {
        fn capture_verifier_binding(
            &self,
        ) -> Result<
            ProviderIngestCompletedMusubiCaptureVerifierBindingV1,
            ProviderIngestFinalizedLedgerErrorV1,
        > {
            Err(ProviderIngestFinalizedLedgerErrorV1::Unavailable)
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
            Box::pin(async { Err(ProviderIngestFinalizedLedgerErrorV1::Unavailable) })
        }
    }
    let (config, _temp_dir) = storage_config_with_temp_dir();
    assert!(config.provider_ingest_outbox_policy().is_none());
    let handle = NodeHandle::try_new(config).expect("open storage without provider ingest");
    assert!(matches!(
        handle.finalized_provider_ingest_status_page(None, 1),
        Err(FinalizedProviderIngestError::Disabled)
    ));
    assert!(matches!(
        handle.take_provider_ingest_completed_musubi_capture_coordinator(
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
                Hash::prehashed([0x4B; 32]),
            )),
            1,
            Arc::new(DisabledCaptureLedger),
        ),
        Err(FinalizedProviderIngestError::Disabled)
    ));
}
#[test]
fn provider_ingest_startup_requires_exact_sealed_checkpoint_injection() {
    const HANDLE: &str = "sealed.sorafs.provider-ingest.startup";
    let (config, _temp_dir) = storage_config_with_provider_ingest_checkpoint(HANDLE, 0x61);
    assert_node_init_variant!(ProviderIngestOutbox => NodeHandle::try_new(config.clone()));
    let substituted = Arc::new(StartupProviderIngestCheckpointRuntime::new(
        "sealed.sorafs.provider-ingest.substituted",
        0x61,
    ));
    assert_node_init_variant!(ProviderIngestOutbox =>
        NodeHandle::try_new_with_runtime_deps(
            config,
            NodeRuntimeDeps::default().with_provider_ingest_checkpoint_runtime(substituted),
        )
    );
}
#[test]
fn provider_ingest_startup_rejects_unexpected_and_test_marked_checkpoint_providers() {
    const HANDLE: &str = "sealed.sorafs.provider-ingest.unexpected";
    let (config, _temp_dir) = storage_config_with_temp_dir();
    let unexpected = Arc::new(StartupProviderIngestCheckpointRuntime::new(HANDLE, 0x62));
    assert_node_init_variant!(ProviderIngestOutbox =>
        NodeHandle::try_new_with_runtime_deps(
            config,
            NodeRuntimeDeps::default().with_provider_ingest_checkpoint_runtime(unexpected),
        )
    );
    let (test_marked_config, _test_marked_temp_dir) =
        storage_config_with_provider_ingest_checkpoint("sealed.sorafs.provider-ingest.test", 0x63);
    let test_marked = Arc::new(StartupProviderIngestCheckpointRuntime::new(
        "sealed.sorafs.provider-ingest.test",
        0x63,
    ));
    assert_node_init_variant!(ProviderIngestOutbox =>
        NodeHandle::try_new_with_runtime_deps(
            test_marked_config,
            NodeRuntimeDeps::default().with_provider_ingest_checkpoint_runtime(test_marked),
        )
    );
}
fn install_finalized_provider_ingest_projection(
    handle: &NodeHandle,
    fixture: &FinalizedProviderIngestFixture,
) {
    let outcome = handle
        .reconcile_finalized_capacity(
            fixture.capacity_cursor,
            CapacityReconcileModeV1::FullRebuild,
            fixture.provider_id,
            Some(&fixture.declaration),
            std::slice::from_ref(&fixture.order),
        )
        .expect("install finalized provider-ingest projection");
    assert_eq!(outcome.pending_order_count, 1);
}
#[test]
fn finalized_provider_ingest_local_storage_awaits_signing_and_survives_restart() {
    let (fixture, _temp_dir) = finalized_provider_ingest_fixture(0x71);
    let handle = NodeHandle::new(fixture.config.clone());
    install_finalized_provider_ingest_projection(&handle, &fixture);
    let inserted = handle
        .enqueue_finalized_provider_ingest(
            &fixture.finalized_pin,
            *fixture.order.order_id.as_bytes(),
        )
        .expect("enqueue finalized provider ingest");
    assert!(matches!(
        inserted,
        ProviderIngestEnqueueResultV1::Inserted { .. }
    ));
    let replay = handle
        .enqueue_finalized_provider_ingest(
            &fixture.finalized_pin,
            *fixture.order.order_id.as_bytes(),
        )
        .expect("replay finalized provider ingest admission");
    assert!(matches!(
        replay,
        ProviderIngestEnqueueResultV1::ExistingActive { .. }
    ));
    assert_eq!(replay.job_id(), inserted.job_id());
    let claim = handle
        .claim_finalized_provider_ingest_source(
            inserted.job_id(),
            ProviderIngestClaimOwnerV1::new([0x72; 32]).expect("source owner"),
            1_000,
            fixture.ingest_cursor,
        )
        .expect("claim finalized provider ingest");
    let mut reader = fixture.payload.as_slice();
    let manifest_id = handle
        .ingest_finalized_provider_payload(
            &claim,
            &fixture.manifest,
            &fixture.plan,
            &mut reader,
            1_001,
            fixture.ingest_cursor,
        )
        .expect("store finalized provider payload");
    assert_eq!(
        manifest_id,
        hex::encode(
            fixture
                .manifest
                .digest()
                .expect("manifest digest")
                .as_bytes()
        )
    );
    let completion_epoch = 4;
    let completion_signer_policy = ProviderIngestCompletionSignerPolicyV1 {
        policy_id: [0xA1; 32],
        revision: 1,
        predecessor_digest: None,
        policy_digest: [0xA2; 32],
    };
    let completion_authority = ProviderIngestCompletionAuthorityV1::new(
        fixture.order.issued_by.clone(),
        completion_signer_policy,
    );
    let network_id = iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(
        iroha_crypto::Hash::prehashed([0x91; iroha_crypto::Hash::LENGTH]),
    ));
    let mut completion_builder = TransactionBuilder::new(
        network_id,
        fixture.order.issued_by.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([InstructionBox::from(CompleteReplicationOrder {
        order_id: fixture.order.order_id,
        provider_id: fixture.provider_id,
        completion_epoch,
        expected_authority: completion_authority,
        expected_assignment_revision: fixture.order.assignment_revision,
        finalized_anchor: ProviderIngestFinalizedAnchorV1 {
            height: fixture.ingest_cursor.height,
            block_hash: fixture.ingest_cursor.block_hash,
        },
    })]);
    completion_builder.set_creation_time(Duration::from_secs(1));
    completion_builder.set_ttl(Duration::from_secs(30));
    let completion_payload = completion_builder
        .into_payload()
        .expect("exact provider completion payload");
    let outbox = handle
        .provider_ingest_outbox
        .as_ref()
        .expect("provider ingest outbox");
    outbox
        .observe_finalized_snapshot(fixture.ingest_cursor, 1_000)
        .expect("observe finalized provider-ingest snapshot");
    let signing_claim = outbox
        .claim_completion_signing(
            inserted.job_id(),
            ProviderIngestCompletionSigningContextV1 {
                baseline_finalized_cursor: fixture.ingest_cursor,
                network_id,
                provider_owner: completion_payload.authority.clone(),
                signer_policy: completion_signer_policy,
                assignment_revision: fixture.order.assignment_revision,
                completion_epoch,
                expected_payload: completion_payload,
            },
            1_002,
        )
        .expect("claim exact completion signing");
    assert_eq!(signing_claim.job_id(), inserted.job_id());
    assert_eq!(signing_claim.generation(), 1);
    let signing_status = outbox
        .status(inserted.job_id())
        .expect("signing provider ingest status");
    assert!(matches!(
        &signing_status.state,
        ProviderIngestDeliveryStateV1::LocalStored {
            completion: ProviderIngestCompletionStateV1::Signing {
                completion_epoch: 4,
                ..
            },
            ..
        }
    ));
    assert!(!matches!(
        &signing_status.state,
        ProviderIngestDeliveryStateV1::FinalizedCompleted { .. }
    ));
    let signing_retry = outbox
        .release_completion_signing(&signing_claim, 1_003, fixture.ingest_cursor)
        .expect("release exact signing claim");
    let next_signing_attempt_at_ms = match signing_retry {
        ProviderIngestRetryOutcomeV1::RetryScheduled {
            attempts: 1,
            next_attempt_at_ms,
        } => next_attempt_at_ms,
        other => panic!("unexpected signing retry outcome: {other:?}"),
    };
    let page = handle
        .finalized_provider_ingest_status_page(None, 1)
        .expect("provider ingest status page");
    assert_eq!(page.rows.len(), 1);
    assert!(page.next_after_job_id.is_none());
    assert_eq!(page.rows[0].job_id, inserted.job_id());
    assert!(matches!(
        &page.rows[0].state,
        ProviderIngestDeliveryStateV1::LocalStored {
            manifest_id: stored_manifest_id,
            musubi_bundle: None,
            completion: ProviderIngestCompletionStateV1::Ready {
                attempts: 1,
                next_attempt_at_ms,
                last_failure_class: Some(
                    ProviderIngestFailureClassV1::SignerUnavailable
                ),
            },
        } if stored_manifest_id == &manifest_id
            && *next_attempt_at_ms == next_signing_attempt_at_ms
    ));
    assert!(
        !matches!(
            &page.rows[0].state,
            ProviderIngestDeliveryStateV1::FinalizedCompleted { .. }
        ),
        "LocalStored must never imply finalized ledger completion"
    );
    drop(handle);
    let restored = NodeHandle::new(fixture.config);
    let restored_page = restored
        .finalized_provider_ingest_status_page(None, 1)
        .expect("restored provider ingest status page");
    assert_eq!(restored_page, page);
}
#[test]
fn finalized_provider_ingest_source_mismatch_is_retryable_and_payload_free() {
    let (fixture, _temp_dir) = finalized_provider_ingest_fixture(0x81);
    let handle = NodeHandle::new(fixture.config.clone());
    install_finalized_provider_ingest_projection(&handle, &fixture);
    let inserted = handle
        .enqueue_finalized_provider_ingest(
            &fixture.finalized_pin,
            *fixture.order.order_id.as_bytes(),
        )
        .expect("enqueue finalized provider ingest");
    let claim = handle
        .claim_finalized_provider_ingest_source(
            inserted.job_id(),
            ProviderIngestClaimOwnerV1::new([0x82; 32]).expect("source owner"),
            2_000,
            fixture.ingest_cursor,
        )
        .expect("claim finalized provider ingest");
    let mut substituted_manifest = fixture.manifest.clone();
    substituted_manifest.content_length = substituted_manifest.content_length.saturating_add(1);
    let mut reader = fixture.payload.as_slice();
    let error = handle
        .ingest_finalized_provider_payload(
            &claim,
            &substituted_manifest,
            &fixture.plan,
            &mut reader,
            2_001,
            fixture.ingest_cursor,
        )
        .expect_err("substituted manifest must be rejected");
    assert!(matches!(
        error,
        FinalizedProviderIngestError::BindingMismatch(
            "manifest bytes disagree with finalized authorization"
        )
    ));
    assert!(handle.storage().expect("storage").manifests().is_empty());
    let page = handle
        .finalized_provider_ingest_status_page(None, 1)
        .expect("provider ingest status page");
    assert_eq!(page.rows.len(), 1);
    assert!(matches!(
        &page.rows[0].state,
        ProviderIngestDeliveryStateV1::RetryScheduled {
            attempts: 1,
            next_attempt_at_ms,
            failure_class: ProviderIngestFailureClassV1::SourceRejected,
        } if *next_attempt_at_ms > 2_001
    ));
    let checkpoint = std::fs::read(
        fixture
            .config
            .data_dir()
            .join(PROVIDER_INGEST_OUTBOX_FILE_V1),
    )
    .expect("read provider ingest checkpoint");
    assert!(
        !checkpoint
            .windows(fixture.payload.len())
            .any(|window| window == fixture.payload),
        "provider-ingest checkpoint must remain payload-free"
    );
}
#[test]
fn finalized_provider_ingest_uses_typed_cancellation_evidence() {
    let (fixture, _temp_dir) = finalized_provider_ingest_fixture(0x91);
    let handle = NodeHandle::new(fixture.config.clone());
    install_finalized_provider_ingest_projection(&handle, &fixture);
    let inserted = handle
        .enqueue_finalized_provider_ingest(
            &fixture.finalized_pin,
            *fixture.order.order_id.as_bytes(),
        )
        .expect("enqueue finalized provider ingest");
    let cancellation_cursor = ProviderIngestFinalizedCursorV1 {
        height: fixture.ingest_cursor.height + 1,
        block_hash: [0x92; 32],
    };
    let outbox = handle
        .provider_ingest_outbox
        .as_ref()
        .expect("provider ingest outbox");
    outbox
        .observe_finalized_snapshot(cancellation_cursor, 2_000)
        .expect("observe finalized cancellation snapshot");
    outbox
        .cancel(
            inserted.job_id(),
            ProviderIngestFinalizedCancellationV1 {
                finalized_cursor: cancellation_cursor,
                provider_id: *fixture.provider_id.as_bytes(),
                order_id: *fixture.order.order_id.as_bytes(),
                manifest_digest: fixture.manifest.digest().expect("manifest digest").into(),
                reason: ProviderIngestCancellationReasonV1::OrderExpired,
            },
        )
        .expect("apply typed finalized cancellation");
    let status = handle
        .finalized_provider_ingest_status_page(None, 1)
        .expect("cancelled provider ingest status");
    assert!(matches!(
        &status.rows[0].state,
        ProviderIngestDeliveryStateV1::Cancelled {
            reason: ProviderIngestCancellationReasonV1::OrderExpired,
            observed_finalized_cursor,
        } if *observed_finalized_cursor == cancellation_cursor
    ));
}
#[test]
fn orderbook_forwarder_survives_restart_when_worker_and_provider_are_disabled() {
    use iroha_data_model::{
        isi::sorafs::MatchSorafsOrderbook,
        sorafs::orderbook::{
            ORDERBOOK_ADMISSION_POLICY_VERSION_V1, OrderbookAdmissionPolicyRecord,
            OrderbookAdmissionPolicyV1, OrderbookFinalizedCursorV1,
        },
    };
    let temp_dir = tempfile::tempdir().expect("create orderbook forwarder temp dir");
    let data_dir = temp_dir.path().join("validator-state");
    let config = StorageConfig::builder()
        .enabled(false)
        .data_dir(data_dir.clone())
        .build();
    assert!(!config.orderbook_worker_policy().enabled());
    let matcher = KeyPair::try_from_seed(vec![0x61; 32], Algorithm::Ed25519).expect("matcher key");
    let settlement =
        KeyPair::try_from_seed(vec![0x62; 32], Algorithm::Ed25519).expect("settlement key");
    let matcher_authority = AccountId::new(matcher.public_key().clone());
    let policy = OrderbookAdmissionPolicyV1 {
        version: ORDERBOOK_ADMISSION_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        market_id: [0x63; 32],
        matcher_authority: matcher_authority.clone(),
        settlement_authority: AccountId::new(settlement.public_key().clone()),
        paused: false,
        min_order_gib: 1,
        max_order_gib: 1_024,
        price_tick_micro_xor: 1,
        max_maker_fee_bps: 100,
        max_taker_fee_bps: 100,
        max_order_lifetime_secs: 86_400,
        max_receipt_age_secs: 3_600,
        max_clock_skew_secs: 30,
        max_receipt_bytes: 1 << 30,
        max_receipts_per_channel: 128,
    };
    let policy_digest = policy.digest().expect("digest orderbook policy");
    let context = OrderbookTransactionContextV1 {
        network_id: transaction_network_id(0x65),
        policy_record: OrderbookAdmissionPolicyRecord {
            policy,
            policy_digest,
            activated_at_unix: 1,
            activated_by: matcher_authority,
        },
        book_revision: 7,
        finalized_cursor: OrderbookFinalizedCursorV1 {
            height: 11,
            block_hash: [0x64; 32],
        },
    };
    let operation = OrderbookOperationV1::Match(MatchSorafsOrderbook::new(
        policy_digest,
        context.book_revision,
        8,
    ));
    let operation_id = {
        let node = NodeHandle::try_new(config.clone()).expect("start validator-only node");
        assert!(node.storage.is_none());
        let operation_id = node
            .enqueue_orderbook_transaction(operation, &context)
            .expect("persist orderbook operation")
            .operation_id();
        assert_eq!(
            node.pending_orderbook_transactions(1)
                .expect("read pending operation")[0]
                .operation_id,
            operation_id
        );
        let claimed = node
            .claim_orderbook_transaction_for_signing(operation_id)
            .expect("persist signer-only claim before restart");
        assert_eq!(claimed.operation_id, operation_id);
        let signing = node
            .pending_orderbook_transactions(1)
            .expect("read signing handoff");
        assert_eq!(
            signing[0].state,
            crate::orderbook_transaction_forwarder::OrderbookTransactionDeliveryStateV1::Signing
        );
        operation_id
    };
    assert!(
            data_dir
                .join("orderbook-transaction-forwarder")
                .join(
                    crate::orderbook_transaction_forwarder::ORDERBOOK_TRANSACTION_FORWARDER_CHECKPOINT_FILE_NAME_V1,
                )
                .is_file()
        );
    let recovered = NodeHandle::try_new(config).expect("restart validator-only node");
    assert!(recovered.storage.is_none());
    let pending = recovered
        .pending_orderbook_transactions(1)
        .expect("recover pending orderbook operation");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].operation_id, operation_id);
    assert_eq!(pending[0].expected_book_revision, Some(7));
    assert_eq!(
        pending[0].state,
        crate::orderbook_transaction_forwarder::OrderbookTransactionDeliveryStateV1::Ready,
        "restart must recover an interrupted signer-only claim for handoff"
    );
    assert_eq!(
        pending[0].attempts, 1,
        "restart recovery must not refund the consumed signing attempt"
    );
}
#[test]
fn reserve_forwarder_survives_restart_when_worker_and_provider_are_disabled() {
    use iroha_data_model::{
        ChainId,
        asset::AssetDefinitionId,
        domain::DomainId,
        isi::sorafs::RegisterSorafsReserveAccount,
        sorafs::reserve::{
            RESERVE_AUTHORITY_POLICY_VERSION_V1, ReserveAuthorityPolicyRecordV1,
            ReserveAuthorityPolicyV1, ReserveFinalizedCursorV1, ReserveProviderTermsV1,
        },
    };
    let temp_dir = tempfile::tempdir().expect("create reserve forwarder temp dir");
    let data_dir = temp_dir.path().join("validator-state");
    let config = StorageConfig::builder()
        .enabled(false)
        .data_dir(data_dir.clone())
        .build();
    assert!(!config.reserve_worker_policy().enabled());
    let operations =
        KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519).expect("operations key");
    let decision =
        KeyPair::try_from_seed(vec![0x72; 32], Algorithm::Ed25519).expect("decision key");
    let provider =
        KeyPair::try_from_seed(vec![0x73; 32], Algorithm::Ed25519).expect("provider key");
    let operations_authority = AccountId::new(operations.public_key().clone());
    let provider_authority = AccountId::new(provider.public_key().clone());
    let policy = ReserveAuthorityPolicyV1 {
        version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        economics: ReservePolicyV1::default(),
        asset_definition: AssetDefinitionId::derive_from_components(
            DomainId::try_new("reserve", "universal").expect("reserve domain"),
            "xor".parse().expect("reserve asset name"),
        ),
        custody_account: AccountId::new(
            KeyPair::try_from_seed(vec![0x74; 32], Algorithm::Ed25519)
                .expect("custody key")
                .public_key()
                .clone(),
        ),
        treasury_account: AccountId::new(
            KeyPair::try_from_seed(vec![0x75; 32], Algorithm::Ed25519)
                .expect("treasury key")
                .public_key()
                .clone(),
        ),
        operations_authority: operations_authority.clone(),
        decision_authority: AccountId::new(decision.public_key().clone()),
        grace_period_days: 7,
        default_after_days: 30,
        max_provider_debt: XorQuantity::try_from_micro(1_000_000_000)
            .expect("valid reserve debt cap"),
        max_pending_movements_per_provider: 8,
        max_open_appeals_per_provider: 4,
    };
    let policy_digest = policy.digest().expect("digest reserve policy");
    let context = ReserveTransactionContextV1 {
        network_id: transaction_network_id(0x78),
        chain_id: ChainId::from("reserve-forwarder-restart-test"),
        policy_record: ReserveAuthorityPolicyRecordV1 {
            policy,
            policy_digest,
            activated_by: operations_authority,
            activated_at_unix: 1,
        },
        projection:
            crate::reserve_transaction_forwarder::ReserveTransactionProjectionV1::Registration {
                provider_owner: provider_authority.clone(),
            },
        finalized_cursor: ReserveFinalizedCursorV1 {
            height: 11,
            block_hash: [0x76; 32],
        },
    };
    let operation = ReserveOperationV1::RegisterProvider(RegisterSorafsReserveAccount::new(
        ReserveProviderTermsV1 {
            provider_id: ProviderId::new([0x77; 32]),
            provider_account: provider_authority,
            tier: ReserveTier::TierA,
            storage_class: StorageClass::Hot,
            duration: ReserveDuration::Monthly,
            capacity_gib: 64,
        },
        policy_digest,
    ));
    let operation_id = {
        let node = NodeHandle::try_new(config.clone()).expect("start validator-only node");
        assert!(node.storage.is_none());
        let operation_id = node
            .enqueue_reserve_transaction(operation, &context)
            .expect("persist reserve operation")
            .operation_id();
        assert_eq!(
            node.pending_reserve_transactions(1)
                .expect("read pending operation")[0]
                .operation_id,
            operation_id
        );
        let claimed = node
            .claim_reserve_transaction_for_signing(operation_id)
            .expect("persist signer-only claim before restart");
        assert_eq!(claimed.operation_id, operation_id);
        let signing = node
            .pending_reserve_transactions(1)
            .expect("read signing handoff");
        assert_eq!(
            signing[0].state,
            crate::reserve_transaction_forwarder::ReserveTransactionDeliveryStateV1::Signing
        );
        operation_id
    };
    assert!(
            data_dir
                .join("reserve-transaction-forwarder")
                .join(
                    crate::reserve_transaction_forwarder::RESERVE_TRANSACTION_FORWARDER_CHECKPOINT_FILE_NAME_V1,
                )
                .is_file()
        );
    let recovered = NodeHandle::try_new(config).expect("restart validator-only node");
    assert!(recovered.storage.is_none());
    let pending = recovered
        .pending_reserve_transactions(1)
        .expect("recover pending reserve operation");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].operation_id, operation_id);
    assert_eq!(
        pending[0].state,
        crate::reserve_transaction_forwarder::ReserveTransactionDeliveryStateV1::Ready,
        "restart must recover an interrupted signer-only claim for handoff"
    );
    assert_eq!(
        pending[0].attempts, 1,
        "restart recovery must not refund the consumed signing attempt"
    );
}
#[test]
fn validator_only_startup_opens_all_proof_and_repair_checkpoints() {
    fn startup_with_corrupt_checkpoint(
        root: &std::path::Path,
        state_dir_name: &str,
        checkpoint_name: &str,
    ) -> NodeInitError {
        let data_dir = root.join(state_dir_name.replace('-', "_"));
        let checkpoint_dir = data_dir.join(state_dir_name);
        std::fs::create_dir_all(&checkpoint_dir).expect("create checkpoint directory");
        std::fs::write(
            checkpoint_dir.join(checkpoint_name),
            b"not canonical Norito",
        )
        .expect("write corrupt checkpoint");
        let config = StorageConfig::builder()
            .enabled(false)
            .data_dir(data_dir)
            .build();
        NodeHandle::try_new(config)
            .expect_err("validator-only startup must validate the durable checkpoint")
    }
    let temp_dir = tempfile::tempdir().expect("create validator-only state root");
    let potr_error = startup_with_corrupt_checkpoint(
        temp_dir.path(),
        "potr-receipts",
        crate::potr::POTR_TRACKER_CHECKPOINT_FILE_NAME_V1,
    );
    assert!(matches!(potr_error, NodeInitError::Potr { .. }));
    let proof_error = startup_with_corrupt_checkpoint(
        temp_dir.path(),
        "proof-outcome-forwarder",
        crate::proof_outcome_forwarder::PROOF_OUTCOME_OUTBOX_CHECKPOINT_FILE_NAME_V1,
    );
    assert!(matches!(
        proof_error,
        NodeInitError::ProofOutcomeOutbox { .. }
    ));
    let repair_error = startup_with_corrupt_checkpoint(
        temp_dir.path(),
        "repair-transaction-forwarder",
        crate::repair_transaction_forwarder::REPAIR_TRANSACTION_FORWARDER_CHECKPOINT_FILE_NAME_V1,
    );
    assert!(matches!(
        repair_error,
        NodeInitError::RepairTransactionForwarder { .. }
    ));
}
#[test]
fn node_startup_rejects_unsafe_programmatic_reserve_worker_policy() {
    let temp_dir = tempfile::tempdir().expect("create invalid reserve policy temp dir");
    let mut actual = iroha_config::parameters::actual::SorafsStorage::default();
    actual.enabled = false;
    actual.data_dir = temp_dir.path().join("validator-state");
    actual.reserve_worker.scan_batch_limit = 0;
    let error = NodeHandle::try_new(StorageConfig::from(actual))
        .expect_err("unsafe programmatic reserve worker policy must fail closed");
    assert!(matches!(error, NodeInitError::ReserveWorkerConfig { .. }));
}
const TEST_QUARANTINE_KEY_PROVIDER_HANDLE: &str = "kms://moderation/quarantine/primary";
const TEST_QUARANTINE_KEY_PROVIDER_QUALIFICATION: ModerationQuarantineKeyProviderQualificationV1 =
    ModerationQuarantineKeyProviderQualificationV1::new(1, [0x51; 32]);
const TEST_ROTATED_QUARANTINE_KEY_PROVIDER_QUALIFICATION:
    ModerationQuarantineKeyProviderQualificationV1 =
    ModerationQuarantineKeyProviderQualificationV1::new(2, [0x52; 32]);
#[derive(Debug)]
struct TestQuarantineKeyWrapper {
    qualification: ModerationQuarantineKeyProviderQualificationV1,
    active_key_id: String,
    keys: BTreeMap<String, [u8; 32]>,
}
impl TestQuarantineKeyWrapper {
    fn single(key_id: &str, seed: u8) -> Self {
        Self::single_with_qualification(key_id, seed, TEST_QUARANTINE_KEY_PROVIDER_QUALIFICATION)
    }
    fn single_with_qualification(
        key_id: &str,
        seed: u8,
        qualification: ModerationQuarantineKeyProviderQualificationV1,
    ) -> Self {
        Self {
            qualification,
            active_key_id: key_id.to_owned(),
            keys: BTreeMap::from([(key_id.to_owned(), [seed; 32])]),
        }
    }
    fn rotated(old_key_id: &str, old_seed: u8, new_key_id: &str, new_seed: u8) -> Self {
        Self {
            qualification: TEST_ROTATED_QUARANTINE_KEY_PROVIDER_QUALIFICATION,
            active_key_id: new_key_id.to_owned(),
            keys: BTreeMap::from([
                (old_key_id.to_owned(), [old_seed; 32]),
                (new_key_id.to_owned(), [new_seed; 32]),
            ]),
        }
    }
    fn wrapping_key(
        &self,
        key_id: &str,
    ) -> Result<[u8; 32], ModerationQuarantineKeyOperationErrorV1> {
        self.keys
            .get(key_id)
            .copied()
            .ok_or(ModerationQuarantineKeyOperationErrorV1::StaleOrRevoked)
    }
    fn nonce(key_id: &str, context_digest: [u8; 32], key: [u8; 32]) -> [u8; 12] {
        let mut hasher = blake3::Hasher::new_keyed(&key);
        hasher.update(b"sorafs.node.test-quarantine-wrapper.nonce.v1");
        hasher.update(key_id.as_bytes());
        hasher.update(&context_digest);
        let mut nonce = [0_u8; 12];
        nonce.copy_from_slice(&hasher.finalize().as_bytes()[..12]);
        nonce
    }
}
impl ModerationQuarantineKeyWrapper for TestQuarantineKeyWrapper {
    fn provider_handle(&self) -> &str {
        TEST_QUARANTINE_KEY_PROVIDER_HANDLE
    }
    fn qualification(
        &self,
    ) -> Result<
        ModerationQuarantineKeyProviderQualificationV1,
        ModerationQuarantineKeyProviderReadinessErrorV1,
    > {
        Ok(self.qualification)
    }
    fn active_key_id(&self) -> &str {
        &self.active_key_id
    }
    fn wrap_dek(
        &self,
        context_digest: [u8; 32],
        dek: &[u8; 32],
    ) -> Result<Vec<u8>, ModerationQuarantineKeyOperationErrorV1> {
        use iroha_crypto::encryption::{ChaCha20Poly1305, SymmetricEncryptor};
        let key = self.wrapping_key(&self.active_key_id)?;
        let nonce = Self::nonce(&self.active_key_id, context_digest, key);
        SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(key)
            .map_err(|error| {
                ModerationQuarantineKeyOperationErrorV1::Rejected
                    .after_scrubbing_provider_diagnostic(error.to_string())
            })?
            .encrypt(nonce.as_slice(), context_digest.as_slice(), dek.as_slice())
            .map_err(|error| {
                ModerationQuarantineKeyOperationErrorV1::Rejected
                    .after_scrubbing_provider_diagnostic(error.to_string())
            })
    }
    fn unwrap_dek(
        &self,
        key_id: &str,
        context_digest: [u8; 32],
        wrapped_dek: &[u8],
    ) -> Result<[u8; 32], ModerationQuarantineKeyOperationErrorV1> {
        use iroha_crypto::encryption::{ChaCha20Poly1305, SymmetricEncryptor};
        let key = self.wrapping_key(key_id)?;
        let nonce = Self::nonce(key_id, context_digest, key);
        SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(key)
            .map_err(|error| {
                ModerationQuarantineKeyOperationErrorV1::Rejected
                    .after_scrubbing_provider_diagnostic(error.to_string())
            })?
            .decrypt(nonce.as_slice(), context_digest.as_slice(), wrapped_dek)
            .map_err(|error| {
                ModerationQuarantineKeyOperationErrorV1::Rejected
                    .after_scrubbing_provider_diagnostic(error.to_string())
            })?
            .try_into()
            .map_err(|_| ModerationQuarantineKeyOperationErrorV1::Rejected)
    }
}
fn test_quarantine_key_wrapper() -> Arc<dyn ModerationQuarantineKeyWrapper> {
    Arc::new(TestQuarantineKeyWrapper::single(
        "kms:test/quarantine-v1",
        0xA5,
    ))
}
fn node_with_test_quarantine_key_wrapper(config: StorageConfig) -> NodeHandle {
    NodeHandle::try_new_with_quarantine_key_wrapper(config, test_quarantine_key_wrapper())
        .expect("initialise node with test-only quarantine key wrapper")
}
#[test]
fn node_startup_rejects_quarantine_key_wrapper_without_configured_binding() {
    let temp_dir = tempfile::tempdir().expect("create unbound-wrapper temp dir");
    let config = enabled_storage_builder(temp_dir.path().join("storage")).build();
    let error =
        NodeHandle::try_new_with_quarantine_key_wrapper(config, test_quarantine_key_wrapper())
            .expect_err("runtime quarantine-key wrapper must have an exact configured binding");
    assert!(matches!(
        error,
        NodeInitError::ModerationQuarantineKeyWrapperInvalid { .. }
    ));
}
#[derive(Debug, Clone, Copy)]
enum TestPrivacyCyclePrfMode {
    Bound,
    Failure(PrivacyCyclePrfProviderErrorV1),
}
const TEST_PRIVACY_CYCLE_PRF_PROVIDER_HANDLE: &str = "threshold-prf:transparency:primary";
const TEST_PRIVACY_RELEASE_ANCHOR_HANDLE: &str = "governance-dag:transparency:primary";
const TEST_TRANSPARENCY_LEADER_LEASE_HANDLE: &str = "sealed-cas:transparency:leader-primary";
const TEST_FENCED_TRANSPARENCY_PROVIDER_HANDLE: &str =
    "governance-cas:transparency:privacy-primary";
fn test_privacy_cycle_prf_provider_binding() -> TransparencyRuntimeProviderBindingV1 {
    TransparencyRuntimeProviderBindingV1::try_new(
        TEST_PRIVACY_CYCLE_PRF_PROVIDER_HANDLE,
        1,
        [0xC7; 32],
    )
    .expect("valid test threshold-PRF provider binding")
}
fn test_privacy_release_anchor_binding() -> TransparencyRuntimeProviderBindingV1 {
    TransparencyRuntimeProviderBindingV1::try_new(TEST_PRIVACY_RELEASE_ANCHOR_HANDLE, 1, [0xD7; 32])
        .expect("valid test release-anchor provider binding")
}
fn test_transparency_leader_lease_binding() -> TransparencyRuntimeProviderBindingV1 {
    TransparencyRuntimeProviderBindingV1::try_new(
        TEST_TRANSPARENCY_LEADER_LEASE_HANDLE,
        1,
        [0xE7; 32],
    )
    .expect("valid test transparency leader-lease binding")
}
fn test_fenced_transparency_provider_binding() -> TransparencyRuntimeProviderBindingV1 {
    TransparencyRuntimeProviderBindingV1::try_new(
        TEST_FENCED_TRANSPARENCY_PROVIDER_HANDLE,
        1,
        [0xF7; 32],
    )
    .expect("valid test fused transparency provider binding")
}
fn with_test_signed_governance_config(
    builder: config::StorageConfigBuilder,
    root: &Path,
) -> config::StorageConfigBuilder {
    let signer = TestGovernanceDagSigner::new();
    builder
        .governance_dir(Some(root.join("governance")))
        .governance_dag_publisher_peer_id(Some(
            String::from_utf8(signer.publisher_peer_id().to_vec())
                .expect("test Governance peer id is UTF-8"),
        ))
        .governance_dag_signer_handle(Some(signer.handle().to_owned()))
        .governance_dag_signer_qualification(
            Some(TestGovernanceDagSigner::expected_qualification()),
        )
        .governance_dag_checkpoint_store_handle(Some(
            TestGovernanceDagCheckpointStore::HANDLE.to_owned(),
        ))
        .governance_dag_checkpoint_store_qualification(Some(
            TestGovernanceDagCheckpointStore::expected_qualification(),
        ))
        .governance_dag_publisher_public_key_hex(Some(hex::encode(signer.public_key())))
}
fn governance_signer_storage_config(
    root: &Path,
    enabled: bool,
    signer: &TestGovernanceDagSigner,
    configured_handle: &str,
    expected_qualification: GovernanceDagRuntimeProviderQualificationV1,
) -> StorageConfig {
    StorageConfig::builder()
        .enabled(enabled)
        .data_dir(root.join("storage"))
        .governance_dir(Some(root.join("governance")))
        .governance_dag_publisher_peer_id(Some(
            String::from_utf8(signer.publisher_peer_id().to_vec()).expect("test peer id is UTF-8"),
        ))
        .governance_dag_signer_handle(Some(configured_handle.to_owned()))
        .governance_dag_signer_qualification(Some(expected_qualification))
        .governance_dag_checkpoint_store_handle(Some(
            TestGovernanceDagCheckpointStore::HANDLE.to_owned(),
        ))
        .governance_dag_checkpoint_store_qualification(Some(
            TestGovernanceDagCheckpointStore::expected_qualification(),
        ))
        .governance_dag_publisher_public_key_hex(Some(hex::encode(signer.public_key())))
        .build()
}
fn governance_signer_runtime_deps(signer: Arc<TestGovernanceDagSigner>) -> NodeRuntimeDeps {
    NodeRuntimeDeps::default()
        .with_governance_dag_signer(signer)
        .with_governance_dag_checkpoint_store(Arc::new(TestGovernanceDagCheckpointStore::default()))
}
struct TestPrivacyCyclePrfProvider {
    mode: TestPrivacyCyclePrfMode,
    handle: &'static str,
    revision: u64,
    policy_digest: [u8; 32],
    qualification_error: bool,
    requests: Mutex<Vec<PrivacyCyclePrfRequestV1>>,
}
impl std::fmt::Debug for TestPrivacyCyclePrfProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TEST-PRF-VENDOR-DIAGNOSTIC-MUST-NOT-LEAK")
    }
}
impl TestPrivacyCyclePrfProvider {
    fn bound() -> Self {
        Self {
            mode: TestPrivacyCyclePrfMode::Bound,
            handle: TEST_PRIVACY_CYCLE_PRF_PROVIDER_HANDLE,
            revision: 1,
            policy_digest: [0xC7; 32],
            qualification_error: false,
            requests: Mutex::new(Vec::new()),
        }
    }
    fn failing(error: PrivacyCyclePrfProviderErrorV1) -> Self {
        Self {
            mode: TestPrivacyCyclePrfMode::Failure(error),
            handle: TEST_PRIVACY_CYCLE_PRF_PROVIDER_HANDLE,
            revision: 1,
            policy_digest: [0xC7; 32],
            qualification_error: false,
            requests: Mutex::new(Vec::new()),
        }
    }
    fn with_qualification(
        handle: &'static str,
        revision: u64,
        policy_digest: [u8; 32],
        qualification_error: bool,
    ) -> Self {
        Self {
            mode: TestPrivacyCyclePrfMode::Bound,
            handle,
            revision,
            policy_digest,
            qualification_error,
            requests: Mutex::new(Vec::new()),
        }
    }
    fn requests(&self) -> Vec<PrivacyCyclePrfRequestV1> {
        self.requests.lock().expect("test PRF requests").clone()
    }
}
impl PrivacyCyclePrfProviderV1 for TestPrivacyCyclePrfProvider {
    fn derive_cycle_output(
        &self,
        request: &PrivacyCyclePrfRequestV1,
    ) -> Result<PrivacyCyclePrfOutputV1, PrivacyCyclePrfProviderErrorV1> {
        self.requests
            .lock()
            .expect("test PRF requests")
            .push(*request);
        match self.mode {
            TestPrivacyCyclePrfMode::Bound => {
                let mut hasher = blake3::Hasher::new();
                hasher.update(b"sorafs.node.test-privacy-cycle-prf.v1");
                hasher.update(&request.binding_digest());
                let output = *hasher.finalize().as_bytes();
                debug_assert_ne!(output, [0; 32]);
                Ok(PrivacyCyclePrfOutputV1::new(output)
                    .expect("test PRF hash cannot be all zeroes"))
            }
            TestPrivacyCyclePrfMode::Failure(error) => Err(error),
        }
    }
}
impl ProductionTransparencyRuntimeProviderV1 for TestPrivacyCyclePrfProvider {
    fn handle(&self) -> &str {
        self.handle
    }
    fn qualification(&self) -> Result<TransparencyRuntimeProviderQualificationV1, String> {
        if self.qualification_error {
            return Err("provider_secret=must-never-escape".to_owned());
        }
        Ok(TransparencyRuntimeProviderQualificationV1::new(
            self.revision,
            self.policy_digest,
        ))
    }
}
#[derive(Default)]
struct TestPrivacyReleaseAnchor {
    heads: Mutex<BTreeMap<[u8; 32], PrivacyReleaseAnchorHeadV1>>,
}
impl PrivacyReleaseAnchorV1 for TestPrivacyReleaseAnchor {
    fn finalized_head(
        &self,
        query_id: [u8; 32],
    ) -> Result<PrivacyReleaseAnchorHeadV1, PrivacyReleaseAnchorErrorV1> {
        Ok(self
            .heads
            .lock()
            .map_err(|_| PrivacyReleaseAnchorErrorV1::Internal)?
            .get(&query_id)
            .copied()
            .unwrap_or_else(|| PrivacyReleaseAnchorHeadV1::genesis(query_id)))
    }
    fn compare_and_set_finalized_head(
        &self,
        expected: PrivacyReleaseAnchorHeadV1,
        next: PrivacyReleaseAnchorHeadV1,
        _lease: &TransparencyLeaderLeaseGrantV1,
    ) -> Result<(), PrivacyReleaseAnchorErrorV1> {
        if expected.query_id() != next.query_id()
            || next.sequence() != expected.sequence().saturating_add(1)
        {
            return Err(PrivacyReleaseAnchorErrorV1::InvalidState);
        }
        let mut heads = self
            .heads
            .lock()
            .map_err(|_| PrivacyReleaseAnchorErrorV1::Internal)?;
        let current = heads
            .get(&expected.query_id())
            .copied()
            .unwrap_or_else(|| PrivacyReleaseAnchorHeadV1::genesis(expected.query_id()));
        if current != expected {
            return Err(PrivacyReleaseAnchorErrorV1::Conflict);
        }
        heads.insert(next.query_id(), next);
        Ok(())
    }
}
impl ProductionTransparencyRuntimeProviderV1 for TestPrivacyReleaseAnchor {
    fn handle(&self) -> &str {
        TEST_PRIVACY_RELEASE_ANCHOR_HANDLE
    }
    fn qualification(&self) -> Result<TransparencyRuntimeProviderQualificationV1, String> {
        Ok(TransparencyRuntimeProviderQualificationV1::new(
            1, [0xD7; 32],
        ))
    }
}
fn test_privacy_cycle_prf_provider() -> Arc<dyn ProductionPrivacyCyclePrfProviderV1> {
    Arc::new(TestPrivacyCyclePrfProvider::bound())
}
fn test_privacy_release_anchor() -> Arc<dyn ProductionPrivacyReleaseAnchorV1> {
    Arc::new(TestPrivacyReleaseAnchor::default())
}
#[derive(Default)]
struct TestTransparencyLeaderLeaseProvider {
    state: Mutex<Option<TransparencyLeaderLeaseGrantV1>>,
    fencing_token: AtomicU64,
}
impl ProductionTransparencyRuntimeProviderV1 for TestTransparencyLeaderLeaseProvider {
    fn handle(&self) -> &str {
        TEST_TRANSPARENCY_LEADER_LEASE_HANDLE
    }
    fn qualification(&self) -> Result<TransparencyRuntimeProviderQualificationV1, String> {
        Ok(TransparencyRuntimeProviderQualificationV1::new(
            1, [0xE7; 32],
        ))
    }
}
impl TransparencyLeaderLeaseProviderV1 for TestTransparencyLeaderLeaseProvider {
    fn acquire(
        &self,
        request: &TransparencyLeaderLeaseAcquireRequestV1,
    ) -> Result<TransparencyLeaderLeaseGrantV1, TransparencyLeaderLeaseProviderErrorV1> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| TransparencyLeaderLeaseProviderErrorV1::Internal)?;
        if state
            .as_ref()
            .is_some_and(|active| request.acquire_at_unix() < active.expires_at_unix())
        {
            return Err(TransparencyLeaderLeaseProviderErrorV1::Conflict);
        }
        let fencing_token = self
            .fencing_token
            .load(Ordering::SeqCst)
            .max(request.fencing_floor())
            .checked_add(1)
            .ok_or(TransparencyLeaderLeaseProviderErrorV1::Internal)?;
        self.fencing_token.store(fencing_token, Ordering::SeqCst);
        let mut lease_id = [0xA7; 32];
        lease_id[..8].copy_from_slice(&fencing_token.to_le_bytes());
        let grant = TransparencyLeaderLeaseGrantV1::try_new(
            lease_id,
            request.scope(),
            fencing_token,
            request.acquire_at_unix(),
            request.expires_at_unix(),
            test_transparency_leader_lease_binding(),
        )
        .map_err(|_| TransparencyLeaderLeaseProviderErrorV1::Internal)?;
        *state = Some(grant.clone());
        Ok(grant)
    }
    fn renew(
        &self,
        request: &TransparencyLeaderLeaseRenewRequestV1,
    ) -> Result<TransparencyLeaderLeaseGrantV1, TransparencyLeaderLeaseProviderErrorV1> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| TransparencyLeaderLeaseProviderErrorV1::Internal)?;
        if state.as_ref() != Some(request.current_grant()) {
            return Err(TransparencyLeaderLeaseProviderErrorV1::Conflict);
        }
        let fencing_token = self
            .fencing_token
            .load(Ordering::SeqCst)
            .max(request.fencing_floor())
            .checked_add(1)
            .ok_or(TransparencyLeaderLeaseProviderErrorV1::Internal)?;
        self.fencing_token.store(fencing_token, Ordering::SeqCst);
        let grant = TransparencyLeaderLeaseGrantV1::try_new(
            request.current_grant().lease_id(),
            request.current_grant().scope(),
            fencing_token,
            request.renew_at_unix(),
            request.expires_at_unix(),
            test_transparency_leader_lease_binding(),
        )
        .map_err(|_| TransparencyLeaderLeaseProviderErrorV1::Internal)?;
        *state = Some(grant.clone());
        Ok(grant)
    }
    fn release(
        &self,
        request: &TransparencyLeaderLeaseReleaseRequestV1,
    ) -> Result<TransparencyLeaderLeaseReleaseReceiptV1, TransparencyLeaderLeaseProviderErrorV1>
    {
        let mut state = self
            .state
            .lock()
            .map_err(|_| TransparencyLeaderLeaseProviderErrorV1::Internal)?;
        if state.as_ref() != Some(request.current_grant()) {
            return Err(TransparencyLeaderLeaseProviderErrorV1::Conflict);
        }
        let grant = state
            .take()
            .ok_or(TransparencyLeaderLeaseProviderErrorV1::Conflict)?;
        TransparencyLeaderLeaseReleaseReceiptV1::try_new(
            grant.lease_id(),
            grant.scope(),
            grant.fencing_token(),
            request.release_at_unix(),
            test_transparency_leader_lease_binding(),
        )
        .map_err(|_| TransparencyLeaderLeaseProviderErrorV1::Internal)
    }
}
fn test_transparency_leader_lease_provider() -> Arc<dyn ProductionTransparencyLeaderLeaseProviderV1>
{
    Arc::new(TestTransparencyLeaderLeaseProvider::default())
}
type TestFencedTransparencyPublications =
    BTreeMap<([u8; 32], [u8; 16]), ([u8; 32], [u8; 32], FencedTransparencyTargetHeadV1)>;
#[derive(Debug, Default)]
struct TestFencedTransparencyState {
    head: Option<FencedTransparencyTargetHeadV1>,
    publications: TestFencedTransparencyPublications,
    receipts: BTreeMap<
        [u8; 32],
        (
            FencedPrivacyPublicationRequestV1,
            FencedPrivacyPublicationReceiptV1,
        ),
    >,
    history: Vec<FencedTransparencyTargetHeadV1>,
}
struct TestFencedTransparencyProvider {
    handle: &'static str,
    qualification: GovernanceDagRuntimeProviderQualificationV1,
    qualification_error: bool,
    state: Mutex<TestFencedTransparencyState>,
    head_override: Mutex<Option<Option<FencedTransparencyTargetHeadV1>>>,
    governance_checkpoint_store: Arc<TestGovernanceDagCheckpointStore>,
}
impl std::fmt::Debug for TestFencedTransparencyProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TestFencedTransparencyProvider(<runtime-only>)")
    }
}
impl TestFencedTransparencyProvider {
    fn bound() -> Self {
        Self::with_qualification(
            TEST_FENCED_TRANSPARENCY_PROVIDER_HANDLE,
            GovernanceDagRuntimeProviderQualificationV1::new(1, [0xF7; 32]),
            false,
        )
    }
    fn with_qualification(
        handle: &'static str,
        qualification: GovernanceDagRuntimeProviderQualificationV1,
        qualification_error: bool,
    ) -> Self {
        Self {
            handle,
            qualification,
            qualification_error,
            state: Mutex::new(TestFencedTransparencyState::default()),
            head_override: Mutex::new(None),
            governance_checkpoint_store: Arc::new(TestGovernanceDagCheckpointStore::default()),
        }
    }
    fn override_authoritative_head(&self, head: Option<Option<FencedTransparencyTargetHeadV1>>) {
        *self
            .head_override
            .lock()
            .expect("test authoritative-head override") = head;
    }
}
impl FencedTransparencyPublisherV1 for TestFencedTransparencyProvider {
    fn handle(&self) -> &str {
        self.handle
    }
    fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
        if self.qualification_error {
            return Err("target_credential=must-never-escape".to_owned());
        }
        Ok(self.qualification)
    }
    fn compare_and_append_privacy(
        &self,
        request: &FencedPrivacyPublicationRequestV1,
    ) -> Result<FencedPrivacyPublicationReceiptV1, FencedTransparencyPublishErrorV1> {
        request.validate()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| FencedTransparencyPublishErrorV1::Ambiguous)?;
        if let Some((retained_request, receipt)) = state.receipts.get(&request.request_digest()) {
            return if retained_request == request {
                Ok(receipt.clone())
            } else {
                Err(FencedTransparencyPublishErrorV1::InvalidRequest)
            };
        }
        if let Some((idempotency_digest, payload_digest, included_head)) = state
            .publications
            .get(&request.publication_scope())
            .copied()
        {
            if idempotency_digest != request.publication_idempotency_digest()
                || payload_digest != request.payload_digest()
            {
                return Err(FencedTransparencyPublishErrorV1::PublicationConflict);
            }
            let readback_head = state
                .head
                .ok_or(FencedTransparencyPublishErrorV1::InvalidReceipt)?;
            let receipt = FencedPrivacyPublicationReceiptV1::from_verified_existing(
                request,
                self.handle,
                self.qualification,
                included_head,
                readback_head,
            )?;
            state
                .receipts
                .insert(request.request_digest(), (request.clone(), receipt.clone()));
            return Ok(receipt);
        }
        if state.head != request.expected_authoritative_head() {
            return Err(FencedTransparencyPublishErrorV1::CompareConflict);
        }
        if request.fencing_token()
            <= state
                .head
                .map_or(0, FencedTransparencyTargetHeadV1::fencing_floor)
        {
            return Err(FencedTransparencyPublishErrorV1::StaleFencingToken);
        }
        let receipt = FencedPrivacyPublicationReceiptV1::from_verified_append(
            request,
            self.handle,
            self.qualification,
        )?;
        state.head = Some(receipt.included_head());
        state.history.push(receipt.included_head());
        state.publications.insert(
            request.publication_scope(),
            (
                request.publication_idempotency_digest(),
                request.payload_digest(),
                receipt.included_head(),
            ),
        );
        state
            .receipts
            .insert(request.request_digest(), (request.clone(), receipt.clone()));
        Ok(receipt)
    }
}
impl FencedTransparencyAuthoritativeHeadReaderV1 for TestFencedTransparencyProvider {
    fn handle(&self) -> &str {
        self.handle
    }
    fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
        if self.qualification_error {
            return Err("target_credential=must-never-escape".to_owned());
        }
        Ok(self.qualification)
    }
    fn read_authoritative_head_with_ancestry(
        &self,
        required_ancestors: &[FencedTransparencyTargetHeadV1],
        required_publications: &[FencedTransparencyPublicationInclusionV1],
    ) -> Result<FencedTransparencyHeadAncestryProofV1, String> {
        let head_override = *self
            .head_override
            .lock()
            .map_err(|_| "target_credential=must-never-escape".to_owned())?;
        let state = self
            .state
            .lock()
            .map_err(|_| "target_credential=must-never-escape".to_owned())?;
        let authoritative_head = head_override.unwrap_or(state.head);
        if authoritative_head != state.head {
            return Err("target ancestry proof unavailable".to_owned());
        }
        let current_index = authoritative_head
            .map(|head| {
                state
                    .history
                    .iter()
                    .position(|candidate| *candidate == head)
                    .ok_or_else(|| "target ancestry proof unavailable".to_owned())
            })
            .transpose()?;
        for ancestor in required_ancestors {
            let ancestor_index = state
                .history
                .iter()
                .position(|candidate| candidate == ancestor)
                .ok_or_else(|| "target ancestry proof unavailable".to_owned())?;
            if current_index.is_none_or(|current| ancestor_index > current) {
                return Err("target ancestry proof unavailable".to_owned());
            }
        }
        for publication in required_publications {
            if !state.publications.values().any(
                |(publication_idempotency_digest, payload_digest, included_head)| {
                    *publication_idempotency_digest == publication.publication_idempotency_digest()
                        && *payload_digest == publication.payload_digest()
                        && *included_head == publication.included_head()
                },
            ) {
                return Err("target publication inclusion proof unavailable".to_owned());
            }
        }
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"sorafs.test.node.fenced-head-ancestry-proof.v1");
        fenced_privacy_digest_head(&mut hasher, authoritative_head);
        for ancestor in required_ancestors {
            fenced_privacy_digest_head(&mut hasher, Some(*ancestor));
        }
        for publication in required_publications {
            hasher.update(&publication.publication_idempotency_digest());
            hasher.update(&publication.payload_digest());
            fenced_privacy_digest_head(&mut hasher, Some(publication.included_head()));
        }
        FencedTransparencyHeadAncestryProofV1::try_new(
            authoritative_head,
            required_ancestors.to_vec(),
            required_publications.to_vec(),
            *hasher.finalize().as_bytes(),
        )
        .map_err(|_| "target ancestry proof malformed".to_owned())
    }
}
fn with_test_fenced_privacy_runtime(
    deps: NodeRuntimeDeps,
    provider: Arc<TestFencedTransparencyProvider>,
) -> NodeRuntimeDeps {
    let publisher: Arc<dyn FencedTransparencyPublisherV1> = provider.clone();
    let head_reader: Arc<dyn FencedTransparencyAuthoritativeHeadReaderV1> = provider.clone();
    deps.with_governance_dag_signer(Arc::new(TestGovernanceDagSigner::new()))
        .with_governance_dag_checkpoint_store(provider.governance_checkpoint_store.clone())
        .with_fenced_transparency_publisher(publisher)
        .with_fenced_transparency_head_reader(head_reader)
}
fn with_fresh_test_fenced_privacy_runtime(deps: NodeRuntimeDeps) -> NodeRuntimeDeps {
    with_test_fenced_privacy_runtime(deps, Arc::new(TestFencedTransparencyProvider::bound()))
}
fn privacy_runtime_deps(
    provider: Arc<dyn ProductionPrivacyCyclePrfProviderV1>,
    anchor: Arc<dyn ProductionPrivacyReleaseAnchorV1>,
) -> NodeRuntimeDeps {
    NodeRuntimeDeps::default()
        .with_privacy_cycle_prf_provider(provider)
        .with_privacy_release_anchor(anchor)
        .with_transparency_leader_lease_provider(test_transparency_leader_lease_provider())
}
fn test_privacy_runtime_deps(anchor: Arc<dyn ProductionPrivacyReleaseAnchorV1>) -> NodeRuntimeDeps {
    privacy_runtime_deps(test_privacy_cycle_prf_provider(), anchor)
}
fn fresh_test_privacy_runtime_deps(
    anchor: Arc<dyn ProductionPrivacyReleaseAnchorV1>,
) -> NodeRuntimeDeps {
    with_fresh_test_fenced_privacy_runtime(test_privacy_runtime_deps(anchor))
}
fn fenced_test_privacy_runtime_deps(
    anchor: Arc<dyn ProductionPrivacyReleaseAnchorV1>,
    provider: Arc<TestFencedTransparencyProvider>,
) -> NodeRuntimeDeps {
    with_test_fenced_privacy_runtime(test_privacy_runtime_deps(anchor), provider)
}
fn test_privacy_runtime_deps_without_fenced_target() -> NodeRuntimeDeps {
    test_privacy_runtime_deps(test_privacy_release_anchor())
        .with_governance_dag_signer(Arc::new(TestGovernanceDagSigner::new()))
        .with_governance_dag_checkpoint_store(Arc::new(TestGovernanceDagCheckpointStore::default()))
}
fn node_with_test_privacy_cycle_prf_provider(config: StorageConfig) -> NodeHandle {
    NodeHandle::try_new_with_runtime_deps(
        config,
        with_fresh_test_fenced_privacy_runtime(test_privacy_runtime_deps_without_fenced_target()),
    )
    .expect("initialise node with test-only privacy cycle PRF provider")
}
fn reputation_signing_key() -> iroha_crypto::KeyPair {
    iroha_crypto::KeyPair::try_from_seed(vec![0x5A; 32], Algorithm::Ed25519)
        .expect("derive reputation signing key")
}
fn reputation_trust_policy_fixture() -> ReputationSnapshotTrustPolicyV1 {
    ReputationSnapshotTrustPolicyV1 {
        version: REPUTATION_SNAPSHOT_TRUST_POLICY_VERSION_V1,
        policy_id: [0xA5; 32],
        valid_from_unix: 1_700_000_000,
        valid_until_unix: 2_000_000_000,
        max_snapshot_age_secs: 600,
        max_future_skew_secs: 30,
        min_signatures: 1,
        signers: vec![ReputationTrustedSignerV1 {
            version: REPUTATION_TRUSTED_SIGNER_VERSION_V1,
            signer_id: "council-1".to_owned(),
            public_key: reputation_signing_key()
                .public_key()
                .try_to_bytes()
                .expect("export reputation verifying key")
                .1
                .try_into()
                .expect("Ed25519 public key is fixed-width"),
        }],
        revoked_signer_ids: Vec::new(),
    }
}
fn storage_config_with_reputation_policy() -> (StorageConfig, TempDir) {
    storage_config_with_reputation_policy_fixture(&reputation_trust_policy_fixture())
}
fn storage_config_with_reputation_policy_fixture(
    policy: &ReputationSnapshotTrustPolicyV1,
) -> (StorageConfig, TempDir) {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let policy_path = root.join("reputation-trust-policy.to");
    let policy_bytes = policy
        .canonical_bytes()
        .expect("encode reputation trust policy");
    write_local_checkpoint_atomic(&policy_path, &policy_bytes)
        .expect("write reputation trust policy");
    let cfg = enabled_storage_builder(root.join("storage"))
        .reputation_trust_policy_path(Some(policy_path))
        .build();
    (cfg, temp_dir)
}
fn enabled_repair_config(max_attempts: u32) -> RepairConfig {
    RepairConfig::from(&iroha_config::parameters::actual::SorafsRepair {
        enabled: true,
        max_attempts,
        ..Default::default()
    })
}
fn enabled_gc_config(max_deletions_per_run: u32) -> GcConfig {
    GcConfig::from(&iroha_config::parameters::actual::SorafsGc {
        enabled: true,
        retention_grace_secs: 0,
        max_deletions_per_run,
        ..Default::default()
    })
}
fn record_capacity_declaration_fixture(
    handle: &NodeHandle,
    provider_id: [u8; 32],
    pool_id: [u8; 32],
) -> CapacityDeclarationV1 {
    let declaration = CapacityDeclarationV1 {
        version: CAPACITY_DECLARATION_VERSION_V1,
        provider_id,
        stake: sorafs_manifest::provider_advert::StakePointer {
            pool_id,
            stake_amount: xor("1"),
        },
        committed_capacity_gib: 100,
        chunker_commitments: vec![ChunkerCommitmentV1 {
            profile_id: "sorafs.sf1@1.0.0".into(),
            profile_aliases: None,
            committed_gib: 100,
            capability_refs: Vec::new(),
        }],
        lane_commitments: vec![LaneCommitmentV1 {
            lane_id: "default".into(),
            max_gib: 100,
        }],
        pricing: None,
        valid_from: 1,
        valid_until: 2,
        metadata: vec![],
    };
    let payload = to_bytes(&declaration).expect("encode declaration");
    let record = CapacityDeclarationRecord::new(
        ProviderId::new(declaration.provider_id),
        payload,
        declaration.committed_capacity_gib,
        1,
        1,
        2,
        Metadata::default(),
    );
    handle
        .record_capacity_declaration(&record)
        .expect("record declaration");
    declaration
}
fn gc_node_with_temp_storage() -> (StorageConfig, NodeHandle, TempDir) {
    let (config, temp_dir) = storage_config_with_temp_dir();
    let handle = NodeHandle::new_with_policies(
        config.clone(),
        RepairConfig::default(),
        enabled_gc_config(1),
    );
    (config, handle, temp_dir)
}
fn expired_gc_manifest_fixture(
    handle: &NodeHandle,
    seed: u8,
    now_unix: u64,
    payload: &[u8],
) -> ([u8; 32], String) {
    let digest = build_manifest_with_retention(vec![seed; 8], now_unix - 1, payload, handle);
    let manifest_id = hex::encode(digest);
    (digest, manifest_id)
}
fn ensure_test_capacity_provider(handle: &NodeHandle) -> [u8; 32] {
    if let Some(provider_id) = handle.capacity_usage().provider_id {
        return provider_id;
    }
    let provider_id = [0xAB; 32];
    let declaration = CapacityDeclarationV1 {
        version: CAPACITY_DECLARATION_VERSION_V1,
        provider_id,
        stake: sorafs_manifest::provider_advert::StakePointer {
            pool_id: [0xAC; 32],
            stake_amount: xor("1"),
        },
        committed_capacity_gib: 100,
        chunker_commitments: vec![ChunkerCommitmentV1 {
            profile_id: "sorafs.sf1@1.0.0".to_owned(),
            profile_aliases: None,
            committed_gib: 100,
            capability_refs: Vec::new(),
        }],
        lane_commitments: vec![LaneCommitmentV1 {
            lane_id: "default".to_owned(),
            max_gib: 100,
        }],
        pricing: None,
        valid_from: 1,
        valid_until: u64::MAX,
        metadata: Vec::new(),
    };
    let record = CapacityDeclarationRecord::new(
        ProviderId::new(provider_id),
        norito::to_bytes(&declaration).expect("encode capacity declaration"),
        declaration.committed_capacity_gib,
        1,
        declaration.valid_from,
        declaration.valid_until,
        Metadata::default(),
    );
    handle
        .record_capacity_declaration(&record)
        .expect("record test capacity declaration");
    provider_id
}
fn run_test_gc(
    handle: &NodeHandle,
    now_unix: u64,
    repair_projection: &RepairLedgerTaskProjectionV1,
) -> GcSweepReport {
    ensure_test_capacity_provider(handle);
    handle.run_gc_once(now_unix, repair_projection)
}
fn finalized_repair_projection(tasks: Vec<RepairLedgerTaskV1>) -> RepairLedgerTaskProjectionV1 {
    let finalized_cursor = RepairFinalizedCursorV1 {
        height: 42,
        block_hash: [0xA5; 32],
    };
    let mut status = RepairLedgerStatusV1::default();
    status.tasks = u64::try_from(tasks.len()).expect("test task count fits u64");
    if !tasks.is_empty() {
        status.updated_at_unix_ms = 1_700_000_000_000;
    }
    for task in &tasks {
        status.leased_tasks += u64::from(task.lease.is_some());
        status.slash_proposals += u64::from(task.slash.is_some());
        status.appeals += u64::from(task.appeal.is_some());
        let Some(terminal) = task.terminal_outcome.as_ref() else {
            continue;
        };
        status.terminal_outcomes += 1;
        match &terminal.kind {
            RepairLedgerTerminalKindV1::Completed(_) => status.completed += 1,
            RepairLedgerTerminalKindV1::Failed(_) => status.failed += 1,
            RepairLedgerTerminalKindV1::Escalated(_) => status.escalated += 1,
        }
    }
    let mut builder = RepairLedgerTaskProjectionBuilderV1::new(RepairFinalizedStatusV1 {
        finalized_cursor,
        status,
    })
    .expect("initialize finalized repair projection");
    builder
        .push_page(RepairLedgerTaskPageV1 {
            finalized_cursor,
            tasks,
            has_more: false,
            next_after_task_id: None,
        })
        .expect("append finalized repair projection page");
    builder.finish().expect("finish repair projection")
}
fn empty_finalized_repair_projection() -> RepairLedgerTaskProjectionV1 {
    finalized_repair_projection(Vec::new())
}
fn active_native_repair_task(
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
) -> RepairLedgerTaskV1 {
    let source_identity = [0x61; 32];
    let auditor = AccountId::new(
        KeyPair::try_from_seed(vec![0x60; 32], Algorithm::Ed25519)
            .expect("repair auditor key")
            .public_key()
            .clone(),
    );
    let report = RepairReportV1 {
        version: REPAIR_REPORT_VERSION_V1,
        ticket_id: RepairTicketId("REP-GC-NATIVE-001".to_owned()),
        auditor_account: auditor.to_string(),
        submitted_at_unix: 1_700_000_000,
        evidence: RepairEvidenceV1 {
            version: REPAIR_EVIDENCE_VERSION_V1,
            manifest_digest,
            provider_id,
            por_history_id: None,
            cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                reason: "native finalized GC exclusion".to_owned(),
            }),
            evidence_json: None,
            notes: None,
        },
        notes: None,
    };
    RepairLedgerTaskV1 {
        version: REPAIR_LEDGER_TASK_VERSION_V1,
        task_id: sorafs_repair_task_id_v1(source_identity),
        source_identity,
        ticket_id: report.ticket_id.0.clone(),
        canonical_report: norito::to_bytes(&report).expect("encode repair report"),
        manifest_digest,
        provider_id,
        submitted_by: auditor,
        submitted_at_unix_ms: report.submitted_at_unix * 1_000,
        revision: 1,
        lease: None,
        terminal_outcome: None,
        slash: None,
        appeal: None,
        action_receipts: Vec::new(),
        updated_at_unix_ms: report.submitted_at_unix * 1_000,
    }
}
#[test]
fn bounded_event_history_preserves_monotonic_sequences_and_reports_gaps() {
    let mut history = BoundedEventHistory::new(2);
    assert_eq!(history.append(|sequence| sequence).unwrap(), 1);
    assert_eq!(history.append(|sequence| sequence).unwrap(), 2);
    assert_eq!(history.append(|sequence| sequence).unwrap(), 3);
    let replay = history.replay(Some(0), 10, |sequence| *sequence);
    assert_eq!(replay.oldest_available_sequence, Some(2));
    assert_eq!(replay.latest_sequence, Some(3));
    assert!(replay.gap);
    assert_eq!(replay.events, vec![2, 3]);
    assert_eq!(history.append(|sequence| sequence).unwrap(), 4);
    assert_eq!(history.retained(), vec![3, 4]);
}
#[test]
fn bounded_event_history_rejects_non_monotonic_restore() {
    let mut history = BoundedEventHistory::new(4);
    let error = history
        .restore(vec![1_u64, 3, 3], |sequence| *sequence)
        .expect_err("duplicate checkpoint sequence must fail");
    assert!(error.to_string().contains("strictly consecutive"));
    assert!(history.retained().is_empty());
}
#[test]
fn local_checkpoint_roundtrip_is_bounded_and_atomic() {
    let temp = tempfile::tempdir().expect("temp dir");
    let root = temp.path().canonicalize().expect("canonical temp dir");
    let path = root.join("runtime").join("checkpoint.to");
    write_local_checkpoint_atomic_bounded(&path, b"checkpoint", 10)
        .expect("write bounded checkpoint");
    assert_eq!(
        read_local_checkpoint_bounded(&path, 10).expect("read checkpoint"),
        Some(b"checkpoint".to_vec())
    );
    assert!(write_local_checkpoint_atomic_bounded(&path, b"too-large", 4).is_err());
    assert!(read_local_checkpoint_bounded(&path, 4).is_err());
}
#[test]
fn local_checkpoint_decoder_rejects_trailing_bytes_and_sequence_bombs() {
    let value = vec![1_u64, 2];
    let canonical = norito::to_bytes(&value).expect("encode canonical checkpoint fixture");
    assert_eq!(
        decode_local_checkpoint_canonical::<Vec<u64>>(&canonical, 4_096, 2)
            .expect("decode canonical checkpoint fixture"),
        value
    );
    let mut trailing = canonical;
    trailing.push(0);
    assert!(
        decode_local_checkpoint_canonical::<Vec<u64>>(&trailing, 4_096, 2).is_err(),
        "trailing bytes must not be accepted as an equivalent checkpoint"
    );
    let oversized_sequence =
        norito::to_bytes(&vec![1_u64, 2, 3]).expect("encode sequence bomb fixture");
    assert!(
        decode_local_checkpoint_canonical::<Vec<u64>>(&oversized_sequence, 4_096, 2).is_err(),
        "declared sequence length must fail before allocation beyond the configured bound"
    );
}
#[test]
fn local_checkpoint_decode_limits_follow_actual_wire_size() {
    let limits = local_checkpoint_decode_limits(64, 4_096, usize::MAX)
        .expect("derive bounded checkpoint limits");
    assert_eq!(limits.max_sequence_elements(), 64 * 8);
    assert_eq!(limits.max_field_bytes(), 64);
    assert_eq!(limits.max_total_elements(), 64 * 8);
    assert_eq!(limits.max_total_allocated_bytes(), 4_096 * 4);
    assert_eq!(limits.max_nesting_depth(), 64);
    assert!(local_checkpoint_decode_limits(0, 4_096, 1).is_err());
    assert!(local_checkpoint_decode_limits(4_097, 4_096, 1).is_err());
}
#[test]
fn local_checkpoint_distinguishes_precommit_and_visible_uncertain_failures() {
    fn fail_parent_sync(_: &Path) -> io::Result<()> {
        Err(io::Error::other("injected parent sync failure"))
    }
    let temp = tempfile::tempdir().expect("temp dir");
    let root = temp.path().canonicalize().expect("canonical temp dir");
    let path = root.join("checkpoint.to");
    let precommit = write_local_checkpoint_atomic_bounded(&path, b"too-large", 4)
        .expect_err("size limit fails before commit");
    assert!(!precommit.committed);
    assert!(!path.exists());
    let uncertain = write_local_checkpoint_atomic_with_mode_and_parent_sync(
        &path,
        b"visible-state",
        false,
        fail_parent_sync,
    )
    .expect_err("parent sync failure follows rename");
    assert!(uncertain.committed);
    assert_eq!(
        fs::read(&path).expect("visible committed bytes"),
        b"visible-state"
    );
}
#[test]
fn concurrent_local_checkpoint_writers_never_publish_partial_bytes() {
    let temp = tempfile::tempdir().expect("temp dir");
    let root = temp.path().canonicalize().expect("canonical temp dir");
    let path = Arc::new(root.join("checkpoint.to"));
    let barrier = Arc::new(Barrier::new(8));
    let payloads = (0_u8..8)
        .map(|byte| Arc::<[u8]>::from(vec![byte; 4_096]))
        .collect::<Vec<_>>();
    let workers = payloads
        .iter()
        .map(|payload| {
            let path = Arc::clone(&path);
            let barrier = Arc::clone(&barrier);
            let payload = Arc::clone(payload);
            std::thread::spawn(move || {
                barrier.wait();
                write_local_checkpoint_atomic_bounded(&path, payload.as_ref(), 8_192)
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().expect("checkpoint writer joins").unwrap();
    }
    let bytes = fs::read(&*path).expect("read final checkpoint");
    assert!(
        payloads
            .iter()
            .any(|payload| payload.as_ref() == bytes.as_slice())
    );
    let leftovers = fs::read_dir(root)
        .expect("read temp dir")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
        .count();
    assert_eq!(leftovers, 0);
}
#[cfg(unix)]
#[test]
fn local_checkpoint_rejects_symlink_targets_and_parents() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    let temp = tempfile::tempdir().expect("temp dir");
    let root = temp.path().canonicalize().expect("canonical temp dir");
    let victim = root.join("victim");
    fs::write(&victim, b"unchanged").expect("write victim");
    let target = root.join("checkpoint.to");
    symlink(&victim, &target).expect("symlink target");
    assert!(write_local_checkpoint_atomic(&target, b"replacement").is_err());
    assert!(read_local_checkpoint_bounded(&target, 128).is_err());
    assert_eq!(fs::read(&victim).unwrap(), b"unchanged");
    let real_parent = root.join("real-parent");
    fs::create_dir(&real_parent).expect("create real parent");
    let linked_parent = root.join("linked-parent");
    symlink(&real_parent, &linked_parent).expect("symlink parent");
    assert!(write_local_checkpoint_atomic(&linked_parent.join("state.to"), b"state").is_err());
    assert!(!real_parent.join("state.to").exists());
    let nested_target = linked_parent.join("nested").join("state.to");
    assert!(write_local_checkpoint_atomic(&nested_target, b"state").is_err());
    assert!(!real_parent.join("nested").exists());
    let hardlink_target = root.join("hardlink-target.to");
    write_local_checkpoint_atomic(&hardlink_target, b"state").expect("write hardlink target");
    let hardlink_alias = root.join("hardlink-alias.to");
    fs::hard_link(&hardlink_target, &hardlink_alias).expect("create hardlink alias");
    assert!(read_local_checkpoint_bounded(&hardlink_target, 128).is_err());
    assert!(write_local_checkpoint_atomic(&hardlink_target, b"replacement").is_err());
    let permissive_parent = root.join("permissive-parent");
    fs::create_dir(&permissive_parent).expect("create permissive parent");
    fs::set_permissions(&permissive_parent, fs::Permissions::from_mode(0o777))
        .expect("make parent writable");
    assert!(write_local_checkpoint_atomic(&permissive_parent.join("state.to"), b"state").is_err());
    fs::set_permissions(&permissive_parent, fs::Permissions::from_mode(0o700))
        .expect("restore parent permissions");
}
#[test]
fn local_checkpoint_rejects_parent_traversal_and_randomizes_temporary_names() {
    let temp = tempfile::tempdir().expect("temp dir");
    let traversal = temp.path().join("runtime").join("..").join("escaped.to");
    assert!(write_local_checkpoint_atomic(&traversal, b"state").is_err());
    assert!(!temp.path().join("escaped.to").exists());
    let path = temp.path().join("checkpoint.to");
    let first = local_checkpoint_tmp_path(&path).expect("first randomized temp path");
    let second = local_checkpoint_tmp_path(&path).expect("second randomized temp path");
    assert_ne!(first, second);
    assert!(
        first
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with('.')
    );
    assert!(
        second
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with('.')
    );
}
#[cfg(unix)]
#[test]
fn local_checkpoint_identity_rejects_same_length_file_swap() {
    let temp = tempfile::tempdir().expect("temp dir");
    let first = temp.path().join("first.to");
    let second = temp.path().join("second.to");
    fs::write(&first, b"same-size-a").expect("write first");
    fs::write(&second, b"same-size-b").expect("write second");
    let first_meta = fs::metadata(&first).expect("first metadata");
    let second_meta = fs::metadata(&second).expect("second metadata");
    assert!(same_local_file_identity(&first_meta, &first_meta));
    assert!(!same_local_file_identity(&first_meta, &second_meta));
}
#[test]
fn runtime_initialization_requires_every_checkpoint_after_first_start() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let handle = NodeHandle::try_new(cfg.clone()).expect("initialize runtime checkpoints");
    drop(handle);
    let marker = runtime_state_initialization_path(cfg.data_dir());
    assert_eq!(
        fs::read(&marker).expect("read initialization marker"),
        RUNTIME_STATE_INITIALIZATION_BYTES
    );
    for (_, path) in required_runtime_checkpoint_paths(cfg.data_dir()) {
        assert!(
            path.is_file(),
            "missing initialized checkpoint {}",
            path.display()
        );
        let bytes = fs::read(&path).expect("read initialized checkpoint");
        fs::remove_file(&path).expect("remove initialized checkpoint");
        assert_node_init_variant!(Checkpoint => NodeHandle::try_new(cfg.clone()));
        write_local_checkpoint_atomic(&path, &bytes).expect("restore checkpoint for next case");
    }
    fs::remove_file(&marker).expect("remove initialization marker");
    assert_checkpoint_component!("runtime initialization marker" =>
        NodeHandle::try_new(cfg)
    );
}
#[test]
fn runtime_initialization_rejects_retired_markers_and_checkpoints() {
    for (retired_path, component) in [
        (
            retired_runtime_state_initialization_path_v1 as fn(&Path) -> PathBuf,
            "retired runtime initialization marker",
        ),
        (
            retired_auxiliary_runtime_checkpoint_path_v1 as fn(&Path) -> PathBuf,
            "retired auxiliary runtime checkpoint",
        ),
        (
            retired_runtime_state_initialization_path_v2 as fn(&Path) -> PathBuf,
            "retired runtime initialization marker",
        ),
        (
            retired_auxiliary_runtime_checkpoint_path_v2 as fn(&Path) -> PathBuf,
            "retired auxiliary runtime checkpoint",
        ),
        (
            retired_runtime_state_initialization_path_v3 as fn(&Path) -> PathBuf,
            "retired runtime initialization marker",
        ),
        (
            retired_auxiliary_runtime_checkpoint_path_v3 as fn(&Path) -> PathBuf,
            "retired auxiliary runtime checkpoint",
        ),
        (
            retired_runtime_state_initialization_path_v4 as fn(&Path) -> PathBuf,
            "retired runtime initialization marker",
        ),
        (
            retired_auxiliary_runtime_checkpoint_path_v4 as fn(&Path) -> PathBuf,
            "retired auxiliary runtime checkpoint",
        ),
    ] {
        let (cfg, _dir) = storage_config_with_temp_dir();
        drop(NodeHandle::new(cfg.clone()));
        let path = retired_path(cfg.data_dir());
        write_local_checkpoint_atomic(&path, b"retired-runtime-state")
            .expect("write retired runtime-state artifact");
        let error = NodeHandle::try_new(cfg).expect_err("retired runtime-state artifact must fail");
        assert!(
            matches!(
                error,
                NodeInitError::Checkpoint {
                    component: actual_component,
                    ..
                } if actual_component == component
            ),
            "unexpected startup error: {error}"
        );
    }
}
#[test]
fn runtime_initialization_rejects_noncanonical_v5_marker() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    drop(NodeHandle::new(cfg.clone()));
    let marker = runtime_state_initialization_path(cfg.data_dir());
    write_local_private_checkpoint_atomic(&marker, b"sorafs.node.runtime-state.initialized.v4\n")
        .expect("replace runtime-state marker");
    let error = NodeHandle::try_new(cfg)
        .expect_err("noncanonical runtime-state v5 marker must fail startup");
    assert!(
        matches!(
            error,
            NodeInitError::Checkpoint {
                component: "runtime initialization marker",
                ref message,
                ..
            } if message.contains("runtime-state v5")
        ),
        "unexpected startup error: {error}"
    );
}
#[test]
fn auxiliary_runtime_checkpoint_rejects_retired_version() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    drop(NodeHandle::new(cfg.clone()));
    let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let bytes = fs::read(&path).expect("read initialized auxiliary checkpoint");
    for retired_version in [1, 2, 3, 4] {
        let mut checkpoint = norito::decode_from_bytes::<AuxiliaryRuntimeCheckpointV5>(&bytes)
            .expect("decode initialized auxiliary checkpoint");
        checkpoint.version = retired_version;
        let retired = norito::to_bytes(&checkpoint).expect("encode retired checkpoint version");
        write_local_checkpoint_atomic(&path, &retired)
            .expect("replace auxiliary checkpoint with retired version");
        let error = NodeHandle::try_new(cfg.clone())
            .expect_err("retired auxiliary version must fail startup");
        assert!(
            matches!(
                error,
                NodeInitError::Checkpoint {
                    component: "auxiliary runtime",
                    ref message,
                    ..
                } if message.contains(&format!(
                    "unsupported auxiliary runtime checkpoint version {retired_version}"
                ))
            ),
            "unexpected startup error: {error}"
        );
    }
}
#[test]
fn governance_outbox_rejects_retired_entry_version_before_kind_dispatch() {
    let payload_bytes = b"retired-governance-outbox-entry".to_vec();
    let payload_digest = *blake3::hash(&payload_bytes).as_bytes();
    let entry = GovernanceOutboxEntryV1 {
        version: 1,
        sequence: 1,
        kind: GovernanceOutboxKindV1::DealSettlement,
        payload_digest,
        provenance: None,
        binding_digest: governance_outbox_binding_digest(
            1,
            1,
            GovernanceOutboxKindV1::DealSettlement,
            payload_digest,
            None,
        ),
        payload_bytes,
    };
    let error =
        validate_governance_outbox_entry(&entry).expect_err("retired outbox version must fail");
    assert!(
        error
            .to_string()
            .contains("unsupported governance outbox entry version 1"),
        "unexpected validation error: {error}"
    );
}
#[test]
fn governance_outbox_rejects_finance_submission_without_provenance() {
    let cases = [
        (
            GovernanceOutboxKindV1::AppealFinanceReport,
            norito::to_bytes(&appeal_finance_report_fixture()).expect("encode finance report"),
        ),
        (
            GovernanceOutboxKindV1::AppealFinanceWeeklyRollup,
            norito::to_bytes(&appeal_finance_weekly_rollup_fixture())
                .expect("encode finance rollup"),
        ),
    ];
    for (kind, payload_bytes) in cases {
        let payload_digest = *blake3::hash(&payload_bytes).as_bytes();
        let entry = GovernanceOutboxEntryV1 {
            version: GOVERNANCE_OUTBOX_VERSION_V3,
            sequence: 1,
            kind,
            payload_digest,
            provenance: None,
            binding_digest: governance_outbox_binding_digest(
                GOVERNANCE_OUTBOX_VERSION_V3,
                1,
                kind,
                payload_digest,
                None,
            ),
            payload_bytes,
        };
        let error = validate_governance_outbox_entry(&entry)
            .expect_err("unauthenticated finance entry must fail closed");
        assert!(
            error
                .to_string()
                .contains("missing authenticated provenance"),
            "unexpected validation error for {kind:?}: {error}"
        );
    }
}
#[test]
fn governance_outbox_allows_trusted_internal_proof_and_transparency_producers() {
    let cases = [
        (
            GovernanceOutboxKindV1::ProofTokenIssuance,
            norito::to_bytes(&proof_token_issuance_fixture()).expect("encode proof-token issuance"),
        ),
        (
            GovernanceOutboxKindV1::TransparencyLedgerPublication,
            norito::to_bytes(&transparency_ledger_publication_fixture())
                .expect("encode transparency publication"),
        ),
    ];
    for (kind, payload_bytes) in cases {
        let payload_digest = *blake3::hash(&payload_bytes).as_bytes();
        let entry = GovernanceOutboxEntryV1 {
            version: GOVERNANCE_OUTBOX_VERSION_V3,
            sequence: 1,
            kind,
            payload_digest,
            provenance: None,
            binding_digest: governance_outbox_binding_digest(
                GOVERNANCE_OUTBOX_VERSION_V3,
                1,
                kind,
                payload_digest,
                None,
            ),
            payload_bytes,
        };
        validate_governance_outbox_entry(&entry)
            .unwrap_or_else(|error| panic!("trusted internal {kind:?} rejected: {error}"));
    }
}
#[test]
fn auxiliary_runtime_checkpoint_restores_privacy_source_state() {
    let (base, _dir) = storage_config_with_temp_dir();
    let cfg = enabled_storage_builder(base.data_dir().clone())
        .runtime_retention(RuntimeRetentionPolicy::new(4, 8, 1024 * 1024))
        .build();
    let source = NodeHandle::new(cfg.clone());
    source
        .record_privacy_aggregate_source_event(privacy_source_event(
            "restart-event",
            "restart-population",
            0x42,
            1_800_000_001,
        ))
        .expect("persist privacy source event");
    let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    assert!(path.exists());
    drop(source);
    let restored = NodeHandle::new(cfg);
    assert_eq!(restored.privacy_aggregate_source_event_count(), 1);
    assert_eq!(
        restored
            .record_privacy_aggregate_source_event(privacy_source_event(
                "restart-event",
                "restart-population",
                0x42,
                1_800_000_001,
            ))
            .expect("restart retry is idempotent"),
        PrivacySourceEventRecordOutcomeV1::AlreadyRecorded
    );
}
#[test]
fn concurrent_auxiliary_runtime_updates_survive_restart() {
    let (base, _dir) = storage_config_with_temp_dir();
    let cfg = enabled_storage_builder(base.data_dir().clone())
        .runtime_retention(RuntimeRetentionPolicy::new(8, 32, 1024 * 1024))
        .build();
    let source = NodeHandle::new(cfg.clone());
    let barrier = Arc::new(Barrier::new(16));
    let workers = (0_u8..16)
        .map(|index| {
            let handle = source.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                handle.record_privacy_aggregate_source_event(privacy_source_event(
                    &format!("concurrent-{index}"),
                    "concurrent-population",
                    index.saturating_add(1),
                    1_800_000_100 + u64::from(index),
                ))
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().expect("runtime writer joins").unwrap();
    }
    assert_eq!(source.privacy_aggregate_source_event_count(), 16);
    drop(source);
    let restored = NodeHandle::new(cfg);
    assert_eq!(restored.privacy_aggregate_source_event_count(), 16);
}
#[test]
fn auxiliary_runtime_checkpoint_corruption_and_oversize_fail_startup() {
    let (base, _dir) = storage_config_with_temp_dir();
    let checkpoint_max_bytes = 64 * 1024;
    let cfg = enabled_storage_builder(base.data_dir().clone())
        .runtime_retention(RuntimeRetentionPolicy::new(4, 8, checkpoint_max_bytes))
        .build();
    drop(NodeHandle::new(cfg.clone()));
    let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"not-norito").unwrap();
    let corrupt_error = NodeHandle::try_new(cfg.clone())
        .expect_err("corrupt auxiliary checkpoint must fail startup");
    assert!(
        matches!(
            corrupt_error,
            NodeInitError::Checkpoint {
                component: "auxiliary runtime",
                ..
            }
        ),
        "unexpected startup error: {corrupt_error}"
    );
    let oversized_len = usize::try_from(checkpoint_max_bytes)
        .expect("test checkpoint limit fits usize")
        .checked_add(1)
        .expect("test checkpoint oversize length does not overflow");
    fs::write(&path, vec![0xAA; oversized_len]).unwrap();
    assert!(
        matches!(
            NodeHandle::try_new(cfg),
            Err(NodeInitError::Checkpoint {
                component: "auxiliary runtime",
                ..
            })
        ),
        "oversized auxiliary checkpoint must fail its own startup boundary"
    );
}
#[cfg(unix)]
#[test]
fn auxiliary_runtime_checkpoint_symlink_fails_startup() {
    use std::os::unix::fs::symlink;
    let (cfg, _dir) = storage_config_with_temp_dir();
    drop(NodeHandle::new(cfg.clone()));
    let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let victim = cfg.data_dir().join("victim.to");
    fs::write(&victim, b"not-a-checkpoint").unwrap();
    fs::remove_file(&path).expect("remove initialized auxiliary checkpoint");
    symlink(&victim, &path).unwrap();
    assert!(
        matches!(
            NodeHandle::try_new(cfg),
            Err(NodeInitError::Checkpoint {
                component: "auxiliary runtime",
                ..
            })
        ),
        "symlinked auxiliary checkpoint must fail its own startup boundary"
    );
}
fn moderation_repro_manifest_fixture(
    manifest_id_byte: u8,
    runner_hash_byte: u8,
) -> ModerationReproManifestV1 {
    let mut body = ModerationReproBodyV1 {
            schema_version: MODERATION_REPRO_MANIFEST_VERSION_V1,
            manifest_id: [manifest_id_byte; 16],
            // The digest is derived from the canonical body below; it is not
            // an operator-selected fixture label.
            manifest_digest: [0; 32],
            runner_hash: [runner_hash_byte; 32],
            runtime_version: format!("sorafs-ai-runner {runner_hash_byte}.0.0"),
            issued_at_unix: 1_800_000_000 + u64::from(runner_hash_byte),
            seed_material: ModerationSeedMaterialV1 {
                domain_tag: "sfm4a:calibration".to_string(),
                seed_version: 1,
                run_nonce: [0x44; 32],
            },
            thresholds: ModerationThresholdsV1 {
                quarantine: 6_000,
                escalate: 8_500,
            },
            models: vec![ModerationModelFingerprintV1 {
                model_id: [0x55; 16],
                artifact_path: "models/model-55.norito".to_string(),
                artifact_bytes: 1,
                artifact_digest: [0x66; 32],
                weights_digest: [0x77; 32],
                engine: iroha_data_model::sorafs::moderation::ModerationModelEngineV1::DeterministicLinearV1,
                feature_profile: iroha_data_model::sorafs::moderation::ModerationFeatureProfileV1::ByteHistogramAndBigramV1,
                calibration_knot_count: 2,
                max_input_bytes: 1024,
                max_operations: 3073,
                working_memory_bytes: 4096,
                weight: Some(10_000),
            }],
            notes: Some("registry fixture".to_string()),
        };
    body.refresh_manifest_digest()
        .expect("refresh moderation fixture digest");
    let keypair = iroha_crypto::KeyPair::try_from_seed(vec![0x9A; 32], Algorithm::Ed25519)
        .expect("moderation fixture seed must derive keypair");
    let signature =
        SignatureOf::try_new(keypair.private_key(), &body).expect("moderation fixture signature");
    ModerationReproManifestV1 {
        body,
        signatures: vec![ModerationReproSignatureV1 {
            role: "council".to_string(),
            public_key: keypair.public_key().clone(),
            signature,
        }],
    }
}
fn moderation_screening_authority_bundle_fixture(
    now_unix: u64,
    policy_issued_at_unix: u64,
    policy_id_byte: u8,
) -> ModerationScreeningAuthorityBundleV1 {
    let manifest_key =
        KeyPair::try_from_seed(vec![0xD1; 32], Algorithm::Ed25519).expect("manifest key");
    let governance_key =
        KeyPair::try_from_seed(vec![0xD2; 32], Algorithm::Ed25519).expect("governance key");
    let runner_key =
        KeyPair::try_from_seed(vec![0xD3; 32], Algorithm::Ed25519).expect("runner key");
    let mut manifest_body = ModerationReproBodyV1 {
            schema_version: MODERATION_REPRO_MANIFEST_VERSION_V1,
            manifest_id: [0xD4; 16],
            manifest_digest: [0; 32],
            runner_hash: [0xD5; 32],
            runtime_version: "sorafs-ai-runner config-authority-v1".to_owned(),
            issued_at_unix: now_unix.saturating_sub(200),
            seed_material: ModerationSeedMaterialV1 {
                domain_tag: "sfm4a:config-authority".to_owned(),
                seed_version: 1,
                run_nonce: [0xD6; 32],
            },
            thresholds: ModerationThresholdsV1 {
                quarantine: 6_000,
                escalate: 8_500,
            },
            models: vec![ModerationModelFingerprintV1 {
                model_id: [0xD7; 16],
                artifact_path: "models/config-authority-v1.norito".to_owned(),
                artifact_bytes: 1,
                artifact_digest: [0xD8; 32],
                weights_digest: [0xD9; 32],
                engine: iroha_data_model::sorafs::moderation::ModerationModelEngineV1::DeterministicLinearV1,
                feature_profile: iroha_data_model::sorafs::moderation::ModerationFeatureProfileV1::ByteHistogramAndBigramV1,
                calibration_knot_count: 2,
                max_input_bytes: 1024,
                max_operations: 3073,
                working_memory_bytes: 4096,
                weight: Some(10_000),
            }],
            notes: None,
        };
    manifest_body
        .refresh_manifest_digest()
        .expect("manifest digest");
    let manifest = ModerationReproManifestV1 {
        signatures: vec![ModerationReproSignatureV1 {
            role: "model-governance".to_owned(),
            public_key: manifest_key.public_key().clone(),
            signature: SignatureOf::try_new(manifest_key.private_key(), &manifest_body)
                .expect("manifest signature"),
        }],
        body: manifest_body,
    };
    let mut policy_body = ModerationTrustPolicyBodyV1 {
        schema_version: MODERATION_TRUST_POLICY_VERSION_V1,
        policy_id: [policy_id_byte; 16],
        policy_digest: [0; 32],
        manifest_id: manifest.body.manifest_id,
        manifest_digest: manifest.body.manifest_digest,
        runner_hash: manifest.body.runner_hash,
        issued_at_unix: policy_issued_at_unix,
        valid_from_unix: policy_issued_at_unix,
        valid_until_unix: now_unix.saturating_add(3_600),
        result_quorum: 1,
        governance_quorum: 1,
        max_result_age_secs: 600,
        max_result_ttl_secs: 300,
        max_clock_skew_secs: 30,
        trusted_signers: vec![ModerationTrustedSignerV1 {
            role: "runner".to_owned(),
            public_key: runner_key.public_key().clone(),
            valid_from_unix: policy_issued_at_unix,
            valid_until_unix: now_unix.saturating_add(3_600),
            revoked_at_unix: None,
        }],
        notes: None,
    };
    policy_body.refresh_policy_digest().expect("policy digest");
    let policy = ModerationTrustPolicyV1 {
        signatures: vec![ModerationTrustPolicySignatureV1 {
            role: "governance".to_owned(),
            public_key: governance_key.public_key().clone(),
            signature: SignatureOf::try_new(governance_key.private_key(), &policy_body)
                .expect("policy signature"),
        }],
        body: policy_body,
    };
    ModerationScreeningAuthorityBundleV1 {
        version: MODERATION_SCREENING_AUTHORITY_BUNDLE_VERSION_V1,
        manifest,
        policy,
        governance_trust_anchors: vec![governance_key.public_key().clone()],
        minimum_governance_quorum: 1,
    }
}
fn write_moderation_screening_authority_bundle(
    root: &Path,
    bundle: &ModerationScreeningAuthorityBundleV1,
) -> (PathBuf, [u8; 32]) {
    let bytes = norito::to_bytes(bundle).expect("encode authority bundle");
    let root = fs::canonicalize(root).expect("canonicalize authority fixture root");
    let path = root.join("moderation-screening-authority.to");
    fs::write(&path, &bytes).expect("write authority bundle");
    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .expect("secure authority bundle permissions");
    (path, *blake3::hash(&bytes).as_bytes())
}
#[test]
fn moderation_screening_authority_loads_from_digest_pinned_config_and_rejects_rotation_attacks() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let now_unix = unix_now_secs();
    let policy_issued_at_unix = now_unix.saturating_sub(50);
    let bundle =
        moderation_screening_authority_bundle_fixture(now_unix, policy_issued_at_unix, 0xE1);
    let (path, digest) = write_moderation_screening_authority_bundle(temp_dir.path(), &bundle);
    let storage_dir = path
        .parent()
        .expect("authority fixture must have a parent")
        .join("storage");
    let cfg = enabled_storage_builder(storage_dir)
        .moderation_screening_enabled(true)
        .moderation_screening_authority_bundle_path(Some(path))
        .moderation_screening_authority_bundle_digest(Some(digest))
        .moderation_quarantine_key_provider(Some(test_quarantine_key_provider_config()))
        .build();
    assert!(matches!(
        NodeHandle::try_new(cfg.clone()),
        Err(NodeInitError::ModerationQuarantineKeyWrapperUnavailable)
    ));
    let key_wrapper = test_quarantine_key_wrapper();
    let node = NodeHandle::try_new_with_quarantine_key_wrapper(cfg, Arc::clone(&key_wrapper))
        .expect("load config-authoritative screening bundle with runtime key wrapper");
    assert!(node.has_moderation_screening_authority());
    assert!(node.moderation_screening_enabled());
    assert!(node.uses_moderation_quarantine_key_wrapper(&key_wrapper));
    assert_eq!(
        node.moderation_quarantine_key_id(),
        Some("kms:test/quarantine-v1")
    );
    let older = moderation_screening_authority_bundle_fixture(
        now_unix,
        policy_issued_at_unix.saturating_sub(1),
        0xE0,
    )
    .into_authority(now_unix)
    .expect("older authority is otherwise valid");
    assert!(matches!(
        node.install_moderation_screening_authority(older),
        Err(ModerationScreeningAuthenticationError::PolicyRollback { .. })
    ));
    let equivocation =
        moderation_screening_authority_bundle_fixture(now_unix, policy_issued_at_unix, 0xE2)
            .into_authority(now_unix)
            .expect("equivocating authority is otherwise valid");
    assert!(matches!(
        node.install_moderation_screening_authority(equivocation),
        Err(ModerationScreeningAuthenticationError::PolicyEquivocation { .. })
    ));
}
#[test]
fn moderation_screening_authority_startup_rejects_missing_mismatched_and_noncanonical_bundles() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let missing = enabled_storage_builder(temp_dir.path().join("missing-storage"))
        .moderation_screening_enabled(true)
        .build();
    assert_node_init_variant!(ModerationScreeningAuthorityBundle =>
        NodeHandle::try_new(missing)
    );
    let relative_path = enabled_storage_builder(temp_dir.path().join("relative-path-storage"))
        .moderation_screening_enabled(true)
        .moderation_screening_authority_bundle_path(Some(PathBuf::from("relative-authority.to")))
        .moderation_screening_authority_bundle_digest(Some([0xA5; 32]))
        .build();
    assert_node_init_variant!(ModerationScreeningAuthorityBundle =>
        NodeHandle::try_new(relative_path)
    );
    let now_unix = unix_now_secs();
    let bundle =
        moderation_screening_authority_bundle_fixture(now_unix, now_unix.saturating_sub(50), 0xE3);
    let (path, digest) = write_moderation_screening_authority_bundle(temp_dir.path(), &bundle);
    let missing_digest = enabled_storage_builder(temp_dir.path().join("missing-digest-storage"))
        .moderation_screening_enabled(true)
        .moderation_screening_authority_bundle_path(Some(path.clone()))
        .build();
    assert_node_init_variant!(ModerationScreeningAuthorityBundle =>
        NodeHandle::try_new(missing_digest)
    );
    let mismatched_digest = enabled_storage_builder(temp_dir.path().join("mismatch-storage"))
        .moderation_screening_enabled(true)
        .moderation_screening_authority_bundle_path(Some(path.clone()))
        .moderation_screening_authority_bundle_digest(Some([0xFF; 32]))
        .build();
    assert_node_init_variant!(ModerationScreeningAuthorityBundle =>
        NodeHandle::try_new(mismatched_digest)
    );
    let mut noncanonical = fs::read(&path).expect("read canonical bundle");
    noncanonical.push(0);
    fs::write(&path, &noncanonical).expect("write noncanonical bundle");
    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .expect("secure noncanonical bundle permissions");
    let noncanonical_config = enabled_storage_builder(temp_dir.path().join("noncanonical-storage"))
        .moderation_screening_enabled(true)
        .moderation_screening_authority_bundle_path(Some(path))
        .moderation_screening_authority_bundle_digest(Some(*blake3::hash(&noncanonical).as_bytes()))
        .build();
    assert_node_init_variant!(ModerationScreeningAuthorityBundle =>
        NodeHandle::try_new(noncanonical_config)
    );
    assert_ne!(digest, [0; 32]);
}
#[test]
fn moderation_screening_authority_startup_bounds_oversized_and_sequence_bomb_inputs() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let temp_root = fs::canonicalize(temp_dir.path()).expect("canonicalize temp root");
    let oversized =
        vec![0_u8; MODERATION_SCREENING_AUTHORITY_BUNDLE_MAX_BYTES_V1.saturating_add(1)];
    let oversized_path = temp_root.join("oversized-authority.to");
    fs::write(&oversized_path, &oversized).expect("write oversized authority");
    #[cfg(unix)]
    fs::set_permissions(&oversized_path, fs::Permissions::from_mode(0o600))
        .expect("secure oversized bundle permissions");
    let oversized_config = enabled_storage_builder(temp_root.join("oversized-storage"))
        .moderation_screening_enabled(true)
        .moderation_screening_authority_bundle_path(Some(oversized_path))
        .moderation_screening_authority_bundle_digest(Some(*blake3::hash(&oversized).as_bytes()))
        .build();
    assert_node_init_variant!(ModerationScreeningAuthorityBundle =>
        NodeHandle::try_new(oversized_config)
    );
    let now_unix = unix_now_secs();
    let mut sequence_bomb =
        moderation_screening_authority_bundle_fixture(now_unix, now_unix.saturating_sub(50), 0xE4);
    sequence_bomb.governance_trust_anchors =
        vec![sequence_bomb.governance_trust_anchors[0].clone(); 4_097];
    let sequence_bomb_bytes = norito::to_bytes(&sequence_bomb).expect("encode sequence bomb");
    assert!(
        sequence_bomb_bytes.len() < MODERATION_SCREENING_AUTHORITY_BUNDLE_MAX_BYTES_V1,
        "sequence bomb must exercise decode bounds rather than the byte cap"
    );
    let sequence_bomb_path = temp_root.join("sequence-bomb-authority.to");
    fs::write(&sequence_bomb_path, &sequence_bomb_bytes).expect("write sequence bomb");
    #[cfg(unix)]
    fs::set_permissions(&sequence_bomb_path, fs::Permissions::from_mode(0o600))
        .expect("secure sequence bomb permissions");
    let sequence_bomb_config = enabled_storage_builder(temp_root.join("sequence-bomb-storage"))
        .moderation_screening_enabled(true)
        .moderation_screening_authority_bundle_path(Some(sequence_bomb_path))
        .moderation_screening_authority_bundle_digest(Some(
            *blake3::hash(&sequence_bomb_bytes).as_bytes(),
        ))
        .build();
    assert_node_init_variant!(ModerationScreeningAuthorityBundle =>
        NodeHandle::try_new(sequence_bomb_config)
    );
    let mut duplicate_anchors =
        moderation_screening_authority_bundle_fixture(now_unix, now_unix.saturating_sub(50), 0xE6);
    duplicate_anchors
        .governance_trust_anchors
        .push(duplicate_anchors.governance_trust_anchors[0].clone());
    let duplicate_anchor_bytes =
        norito::to_bytes(&duplicate_anchors).expect("encode duplicate anchors");
    let duplicate_anchor_path = temp_root.join("duplicate-anchor-authority.to");
    fs::write(&duplicate_anchor_path, &duplicate_anchor_bytes)
        .expect("write duplicate-anchor authority");
    #[cfg(unix)]
    fs::set_permissions(&duplicate_anchor_path, fs::Permissions::from_mode(0o600))
        .expect("secure duplicate-anchor permissions");
    let duplicate_anchor_config =
        enabled_storage_builder(temp_root.join("duplicate-anchor-storage"))
            .moderation_screening_enabled(true)
            .moderation_screening_authority_bundle_path(Some(duplicate_anchor_path))
            .moderation_screening_authority_bundle_digest(Some(
                *blake3::hash(&duplicate_anchor_bytes).as_bytes(),
            ))
            .build();
    assert_node_init_variant!(ModerationScreeningAuthorityBundle =>
        NodeHandle::try_new(duplicate_anchor_config)
    );
}
#[cfg(unix)]
#[test]
fn moderation_screening_authority_startup_rejects_symlink_and_hardlink_replacement_paths() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let now_unix = unix_now_secs();
    let bundle =
        moderation_screening_authority_bundle_fixture(now_unix, now_unix.saturating_sub(50), 0xE5);
    let (source_path, digest) =
        write_moderation_screening_authority_bundle(temp_dir.path(), &bundle);
    let temp_root = source_path
        .parent()
        .expect("authority fixture must have a parent");
    let symlink_path = temp_root.join("symlink-authority.to");
    std::os::unix::fs::symlink(&source_path, &symlink_path).expect("create authority symlink");
    let symlink_config = enabled_storage_builder(temp_root.join("symlink-storage"))
        .moderation_screening_enabled(true)
        .moderation_screening_authority_bundle_path(Some(symlink_path))
        .moderation_screening_authority_bundle_digest(Some(digest))
        .build();
    assert_node_init_variant!(ModerationScreeningAuthorityBundle =>
        NodeHandle::try_new(symlink_config)
    );
    let hardlink_path = temp_root.join("hardlink-authority.to");
    fs::hard_link(&source_path, &hardlink_path).expect("create authority hard link");
    let hardlink_config = enabled_storage_builder(temp_root.join("hardlink-storage"))
        .moderation_screening_enabled(true)
        .moderation_screening_authority_bundle_path(Some(hardlink_path))
        .moderation_screening_authority_bundle_digest(Some(digest))
        .build();
    assert_node_init_variant!(ModerationScreeningAuthorityBundle =>
        NodeHandle::try_new(hardlink_config)
    );
    let directory_path = temp_root.join("directory-authority");
    fs::create_dir(&directory_path).expect("create non-regular authority path");
    let directory_config = enabled_storage_builder(temp_root.join("directory-storage"))
        .moderation_screening_enabled(true)
        .moderation_screening_authority_bundle_path(Some(directory_path))
        .moderation_screening_authority_bundle_digest(Some(digest))
        .build();
    assert_node_init_variant!(ModerationScreeningAuthorityBundle =>
        NodeHandle::try_new(directory_config)
    );
}
fn adversarial_corpus_manifest_fixture() -> AdversarialCorpusManifestV1 {
    AdversarialCorpusManifestV1 {
        schema_version: ADVERSARIAL_CORPUS_VERSION_V1,
        issued_at_unix: 1_800_000_010,
        cohort_label: Some("sfm4a-2026-q1".to_string()),
        families: vec![AdversarialPerceptualFamilyV1 {
            family_id: [0x21; 16],
            description: "jpeg jitter corpus".to_string(),
            variants: vec![
                AdversarialPerceptualVariantV1 {
                    variant_id: [0x31; 16],
                    attack_vector: "jpeg_jitter".to_string(),
                    reference_cid_b64: None,
                    perceptual_hash: Some([0x41; 32]),
                    hamming_radius: 8,
                    embedding_digest: None,
                    notes: Some("hash match".to_string()),
                },
                AdversarialPerceptualVariantV1 {
                    variant_id: [0x32; 16],
                    attack_vector: "mosaic".to_string(),
                    reference_cid_b64: None,
                    perceptual_hash: None,
                    hamming_radius: 0,
                    embedding_digest: Some([0x42; 32]),
                    notes: Some("embedding match".to_string()),
                },
            ],
        }],
    }
}
fn moderation_screening_input_fixture(
    subject: &str,
    verdict: ModerationScreeningVerdict,
) -> ModerationScreeningInput {
    ModerationScreeningInput {
        subject: subject.to_string(),
        subject_digest: [0xA1; 32],
        manifest_id: [0x12; 16],
        runner_hash: [0x34; 32],
        combined_score_bps: match verdict {
            ModerationScreeningVerdict::Pass => 1_000,
            ModerationScreeningVerdict::Warn => 3_000,
            ModerationScreeningVerdict::Quarantine => 6_500,
            ModerationScreeningVerdict::Escalate => 8_700,
            ModerationScreeningVerdict::Block => 9_900,
        },
        verdict,
        screened_at_unix: 1_800_000_050,
        evidence_digest: Some([0xE1; 32]),
        policy_digest: Some([0xC1; 32]),
        notes: Some("local screening fixture".to_string()),
    }
}
fn record_moderation_quarantine_fixture(
    handle: &NodeHandle,
    subject: &str,
    payload: &[u8],
) -> [u8; 16] {
    let mut screening =
        moderation_screening_input_fixture(subject, ModerationScreeningVerdict::Quarantine);
    screening.subject_digest = *blake3::hash(payload).as_bytes();
    handle
        .record_moderation_screening_result(screening)
        .expect("record quarantine result")
        .quarantine
        .expect("quarantine record")
        .quarantine_id
}
fn store_moderation_quarantine_fixture(
    handle: &NodeHandle,
    subject: &str,
    payload: &[u8],
    captured_at_unix: u64,
    content_type: Option<&str>,
) -> ([u8; 16], ModerationQuarantineObjectRecord) {
    let quarantine_id = record_moderation_quarantine_fixture(handle, subject, payload);
    let record = handle
        .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
            quarantine_id,
            payload: payload.to_vec(),
            captured_at_unix,
            content_type: content_type.map(str::to_owned),
            notes: None,
        })
        .expect("store quarantine object");
    (quarantine_id, record)
}
fn moderation_quarantine_review_input(quarantine_id: [u8; 16]) -> ModerationQuarantineReviewInput {
    ModerationQuarantineReviewInput {
        quarantine_id,
        reviewed_by: "operator@moderation".to_string(),
        reviewed_at_unix: 1_800_000_060,
        notes: Some("reviewed locally".to_string()),
    }
}
fn moderation_quarantine_release_input(
    quarantine_id: [u8; 16],
) -> ModerationQuarantineReleaseInput {
    ModerationQuarantineReleaseInput {
        quarantine_id,
        release_authority: "release-authority@moderation".to_string(),
        released_at_unix: 1_800_000_070,
        notes: Some("released locally".to_string()),
    }
}
fn moderation_evidence_viewer_session_input(
    quarantine_id: [u8; 16],
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
) -> ModerationEvidenceViewerSessionInput {
    ModerationEvidenceViewerSessionInput {
        quarantine_id,
        requested_by: "operator@moderation".to_string(),
        viewer_account: "juror-1@moderation".to_string(),
        viewer_role: "juror".to_string(),
        purpose: "appeal evidence review".to_string(),
        attestation_digest: [0xA7; 32],
        watermark_metadata_digest: [0xB7; 32],
        session_nonce_digest: [0xC7; 32],
        issued_at_unix_ms,
        expires_at_unix_ms,
        legal_hold_id: Some("legal-hold-2026-07".to_string()),
        notes: Some("payload-free viewer session".to_string()),
        raw_evidence_included: false,
        signed_url_included: false,
        session_token_included: false,
        watermark_secret_included: false,
    }
}
fn moderation_evidence_viewer_access_input(
    session_id: [u8; 16],
    kind: ModerationEvidenceViewerAccessKind,
    event_at_unix_ms: u64,
) -> ModerationEvidenceViewerAccessInput {
    ModerationEvidenceViewerAccessInput {
        session_id,
        kind,
        actor_account: "juror-1@moderation".to_string(),
        event_at_unix_ms,
        request_digest: [0xD7; 32],
        event_metadata_digest: Some([0xE7; 32]),
        notes: Some("payload-free viewer access".to_string()),
        raw_evidence_included: false,
        signed_url_included: false,
        session_token_included: false,
        response_body_included: false,
    }
}
fn moderation_evidence_viewer_audit_report_input(
    window_start_unix: u64,
    window_end_unix: u64,
    generated_at_unix: u64,
) -> ModerationEvidenceViewerAuditReportInput {
    ModerationEvidenceViewerAuditReportInput {
        report_scope: "local-daily".to_string(),
        window_start_unix,
        window_end_unix,
        generated_at_unix,
        policy_digest: Some([0xF7; 32]),
        raw_evidence_included: false,
        raw_access_logs_included: false,
        viewer_accounts_included: false,
        signed_urls_included: false,
        session_tokens_included: false,
        response_bodies_included: false,
    }
}
fn seed_moderation_evidence_viewer_activity(
    handle: &NodeHandle,
    subject: &str,
    payload: &[u8],
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    access_events: &[(ModerationEvidenceViewerAccessKind, u64)],
) -> ModerationEvidenceViewerSessionRecord {
    let (quarantine_id, _) = store_moderation_quarantine_fixture(
        handle,
        subject,
        payload,
        issued_at_unix_ms / 1_000,
        None,
    );
    let session = handle
        .create_moderation_evidence_viewer_session(moderation_evidence_viewer_session_input(
            quarantine_id,
            issued_at_unix_ms,
            expires_at_unix_ms,
        ))
        .expect("create viewer session");
    for (idx, (kind, event_at_unix_ms)) in access_events.iter().copied().enumerate() {
        let mut input =
            moderation_evidence_viewer_access_input(session.session_id, kind, event_at_unix_ms);
        input.request_digest = [0xD7_u8.wrapping_add(idx as u8); 32];
        input.event_metadata_digest = Some([0xE7_u8.wrapping_add(idx as u8); 32]);
        handle
            .record_moderation_evidence_viewer_access(input)
            .expect("record viewer access");
    }
    session
}
fn reputation_provider_input_fixture() -> ReputationProviderInputV1 {
    let metrics = ReputationProviderMetricsV1 {
        version: REPUTATION_PROVIDER_METRICS_VERSION_V1,
        por_success_bps: 9_800,
        pdp_success_bps: 9_700,
        potr_success_bps: 9_600,
        latency_health_bps: 9_000,
        dispute_rate_bps: 100,
        token_violation_rate_bps: 50,
        repair_breach_rate_bps: 0,
    };
    ReputationProviderInputV1 {
        version: REPUTATION_PROVIDER_INPUT_VERSION_V1,
        provider_id: "provider-a".to_string(),
        metrics,
        reserve_stage: ReputationReserveStageV1::Active,
        previous_score_bps: None,
        active_dispute: false,
        slashing_event: false,
    }
}
fn signed_reputation_snapshot_fixture_with(
    snapshot_id: [u8; 16],
    generated_at_unix: u64,
    previous_snapshot_id: Option<[u8; 16]>,
) -> SignedReputationSnapshotV1 {
    signed_reputation_snapshot_fixture_for_policy(
        &reputation_trust_policy_fixture(),
        snapshot_id,
        generated_at_unix,
        previous_snapshot_id,
    )
}
fn signed_reputation_snapshot_fixture_for_policy(
    policy: &ReputationSnapshotTrustPolicyV1,
    snapshot_id: [u8; 16],
    generated_at_unix: u64,
    previous_snapshot_id: Option<[u8; 16]>,
) -> SignedReputationSnapshotV1 {
    let input = reputation_provider_input_fixture();
    let scoring_evidence = ReputationScoringEvidenceV1 {
        version: REPUTATION_SCORING_EVIDENCE_VERSION_V1,
        provider_inputs: vec![input.clone()],
        trust_edges: Vec::new(),
    };
    let snapshot = build_reputation_snapshot(
        snapshot_id,
        generated_at_unix,
        ReputationWeightsV1::default(),
        &[input],
        previous_snapshot_id,
    )
    .expect("reputation snapshot fixture");
    let mut envelope = SignedReputationSnapshotV1 {
        version: SIGNED_REPUTATION_SNAPSHOT_VERSION_V1,
        policy_digest: policy.canonical_digest().expect("reputation policy digest"),
        snapshot,
        scoring_evidence_digest: scoring_evidence
            .canonical_digest()
            .expect("reputation evidence digest"),
        scoring_evidence,
        signatures: Vec::new(),
    };
    let signing_key = reputation_signing_key();
    let signature = IrohaSignature::try_new(
        signing_key.private_key(),
        &envelope
            .signing_digest()
            .expect("reputation signing digest"),
    )
    .expect("sign reputation snapshot");
    envelope.signatures.push(ReputationSnapshotSignatureV1 {
        signer_id: "council-1".to_owned(),
        signature: signature
            .payload()
            .try_into()
            .expect("Ed25519 signature is fixed-width"),
    });
    envelope
}
fn signed_reputation_snapshot_fixture() -> SignedReputationSnapshotV1 {
    signed_reputation_snapshot_fixture_with([0x42; 16], unix_now_secs(), None)
}
fn transparency_ledger_publication_fixture() -> ModerationLedgerCyclePublicationV1 {
    use iroha_data_model::sorafs::transparency::{
        MODERATION_LEDGER_ENTRY_VERSION_V1, ModerationLedgerEntryKindV1, ModerationLedgerEntryV1,
        ModerationLedgerMetadataV1,
    };
    let cycle_id = *b"cycle-2026-wk-02";
    let entries = [
        ModerationLedgerEntryV1 {
            version: MODERATION_LEDGER_ENTRY_VERSION_V1,
            cycle_id,
            entry_id: [0x22; 16],
            sequence: 2,
            occurred_at_unix: 1_800_000_020,
            kind: ModerationLedgerEntryKindV1::GarEnforcementReceipt,
            subject: "gar-receipt-22".to_string(),
            subject_digest: [0x22; 32],
            payload_digest: [0x23; 32],
            summary_digest: [0x24; 32],
            policy_digest: Some([0x25; 32]),
            evidence_uris: vec!["sora://transparency/22".to_string()],
            metadata: vec![ModerationLedgerMetadataV1 {
                key: "source".to_string(),
                value: "gar".to_string(),
            }],
        },
        ModerationLedgerEntryV1 {
            version: MODERATION_LEDGER_ENTRY_VERSION_V1,
            cycle_id,
            entry_id: [0x11; 16],
            sequence: 1,
            occurred_at_unix: 1_800_000_010,
            kind: ModerationLedgerEntryKindV1::AppealOutcome,
            subject: "appeal-case-11".to_string(),
            subject_digest: [0x11; 32],
            payload_digest: [0x12; 32],
            summary_digest: [0x13; 32],
            policy_digest: Some([0x14; 32]),
            evidence_uris: vec!["sora://transparency/11".to_string()],
            metadata: vec![ModerationLedgerMetadataV1 {
                key: "source".to_string(),
                value: "appeal".to_string(),
            }],
        },
    ];
    ModerationLedgerCyclePublicationV1::from_entries(
        cycle_id,
        1_800_000_000,
        1_800_604_800,
        1_800_604_801,
        None,
        &entries,
    )
    .expect("transparency ledger publication fixture")
}
fn privacy_aggregate_fixture(aggregate_id: &str, seed: u8) -> ModerationPrivacyAggregateV1 {
    use iroha_data_model::sorafs::transparency::{
        MODERATION_PRIVACY_AGGREGATE_VERSION_V1, MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
        ModerationLedgerMetadataV1, ModerationPrivacyAggregateMetricV1, ModerationPrivacyModeV1,
        ModerationPrivacyNoiseSourceV1, ModerationPrivacyParametersV1,
        ModerationPrivacyThresholdPrfCommitmentV1,
    };
    ModerationPrivacyAggregateV1 {
        version: MODERATION_PRIVACY_AGGREGATE_VERSION_V1,
        aggregate_id: aggregate_id.to_string(),
        window_start_unix: 1_800_000_000,
        window_end_unix: 1_800_604_800,
        generated_at_unix: 1_800_604_800,
        population_label: format!("{aggregate_id}-population"),
        population_digest: [seed; 32],
        source_commitment: [seed.wrapping_add(1); 32],
        privacy: ModerationPrivacyParametersV1 {
            version: MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
            mode: ModerationPrivacyModeV1::DifferentialPrivacyWithSuppression,
            epsilon_numerator: Some(4),
            epsilon_denominator: Some(5),
            delta_ppb: Some(0),
            per_subject_metric_cap: Some(1),
            suppression_threshold: Some(25),
        },
        noise_source: ModerationPrivacyNoiseSourceV1::ThresholdPrf(
            ModerationPrivacyThresholdPrfCommitmentV1 {
                commitment: [0xCC; 32],
            },
        ),
        metrics: vec![
            ModerationPrivacyAggregateMetricV1 {
                key: "appeals_upheld".to_string(),
                value: u64::from(seed),
                unit: "count".to_string(),
            },
            ModerationPrivacyAggregateMetricV1 {
                key: "moderation_actions".to_string(),
                value: u64::from(seed) + 10,
                unit: "count".to_string(),
            },
        ],
        policy_digest: [0xC0; 32],
        metadata: vec![ModerationLedgerMetadataV1 {
            key: "publisher".to_string(),
            value: "sfm4c".to_string(),
        }],
    }
}
fn privacy_source_event(
    event_id: &str,
    population_label: &str,
    seed: u8,
    occurred_at_unix: u64,
) -> PrivacyAggregateSourceEvent {
    PrivacyAggregateSourceEvent {
        event_id: event_id.to_string(),
        occurred_at_unix,
        population_label: population_label.to_string(),
        population_digest: [seed; 32],
        subject_digest: *blake3::hash(event_id.as_bytes()).as_bytes(),
        metrics: vec![
            PrivacyAggregateSourceMetric {
                key: "appeals_upheld".to_string(),
                value: 1,
                unit: "count".to_string(),
            },
            PrivacyAggregateSourceMetric {
                key: "moderation_actions".to_string(),
                value: 3,
                unit: "count".to_string(),
            },
        ],
        policy_digest: [0xC0; 32],
        provenance: None,
    }
}
fn transparency_ledger_source_entry(
    event_id: &str,
    occurred_at_unix: u64,
    kind: iroha_data_model::sorafs::transparency::ModerationLedgerEntryKindV1,
    subject: &str,
    seed: u8,
) -> TransparencyLedgerSourceEntry {
    TransparencyLedgerSourceEntry {
        event_id: event_id.to_string(),
        occurred_at_unix,
        kind,
        subject: subject.to_string(),
        subject_digest: [seed; 32],
        payload_digest: [seed.wrapping_add(1); 32],
        summary_digest: [seed.wrapping_add(2); 32],
        policy_digest: Some([seed.wrapping_add(3); 32]),
        evidence_uris: vec![format!("sora://transparency/{event_id}")],
        metadata: vec![
            ModerationLedgerMetadataV1 {
                key: "pipeline".to_string(),
                value: "sfm4c".to_string(),
            },
            ModerationLedgerMetadataV1 {
                key: "source".to_string(),
                value: "unit-test".to_string(),
            },
        ],
    }
}
fn gar_enforcement_receipt_fixture(
    action: iroha_data_model::sorafs::gar::GarEnforcementActionV1,
) -> GarEnforcementReceiptV1 {
    GarEnforcementReceiptV1 {
        receipt_id: *b"gar-receipt-0001",
        gar_name: "docs.sora".to_string(),
        canonical_host: "docs.gateway.sora.net".to_string(),
        action,
        triggered_at_unix: 1_800_000_010,
        expires_at_unix: Some(1_800_086_410),
        policy_version: Some("2026-q2".to_string()),
        policy_digest: Some([0xAB; 32]),
        operator: iroha_data_model::account::AccountId::parse_encoded(
            "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
        )
        .expect("account id"),
        reason: "Guardian freeze window".to_string(),
        notes: Some("Escalated during SFM-4c drill".to_string()),
        evidence_uris: vec!["sora://gar/receipts/docs/0001".to_string()],
        labels: vec!["guardian-freeze".to_string(), "sfm4c".to_string()],
    }
}
fn privacy_aggregate_cycle_config() -> PrivacyAggregateCycleConfig {
    use iroha_data_model::sorafs::transparency::{
        MODERATION_PRIVACY_PARAMETERS_VERSION_V1, ModerationLedgerMetadataV1,
        ModerationPrivacyModeV1, ModerationPrivacyParametersV1,
    };
    PrivacyAggregateCycleConfig {
        query_id: [0xB0; 32],
        first_cycle_start_unix: 100,
        cycle_seconds: 100,
        aggregate_id_prefix: "sfm4c-weekly".to_string(),
        populations: vec![
            PrivacyAggregatePopulationV1 {
                label: "jurisdiction-a".to_string(),
                digest: [0xA0; 32],
            },
            PrivacyAggregatePopulationV1 {
                label: "jurisdiction-b".to_string(),
                digest: [0xB0; 32],
            },
        ],
        metrics: vec![
            PrivacyAggregateMetricSchemaV1 {
                key: "appeals_upheld".to_string(),
                unit: "count".to_string(),
            },
            PrivacyAggregateMetricSchemaV1 {
                key: "moderation_actions".to_string(),
                unit: "count".to_string(),
            },
        ],
        privacy: ModerationPrivacyParametersV1 {
            version: MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
            mode: ModerationPrivacyModeV1::DifferentialPrivacyWithSuppression,
            epsilon_numerator: Some(4),
            epsilon_denominator: Some(5),
            delta_ppb: Some(0),
            per_subject_metric_cap: Some(1),
            suppression_threshold: Some(2),
        },
        policy_digest: [0xC0; 32],
        metadata: vec![ModerationLedgerMetadataV1 {
            key: "publisher".to_string(),
            value: "sfm4c-worker".to_string(),
        }],
    }
}
fn privacy_aggregate_schedule_config() -> PrivacyAggregateScheduleConfig {
    PrivacyAggregateScheduleConfig {
        first_cycle_start_unix: 100,
        cycle_seconds: 100,
        publish_delay_seconds: 10,
    }
}
fn privacy_cycle_prf_input(
    config: &PrivacyAggregateCycleConfig,
    cycle_start_unix: u64,
    cycle_end_unix: u64,
    due_at_unix: u64,
    output: [u8; 32],
) -> PrivacyCyclePrfInputV1 {
    let request = PrivacyCyclePrfRequestV1::new(
        config.query_id,
        config.policy_digest,
        privacy_population_inventory_digest(&config.populations),
        privacy_metric_schema_digest(&config.metrics),
        PrivacyAggregateCycleWindow {
            cycle_start_unix,
            cycle_end_unix,
            due_at_unix,
        },
    )
    .expect("test privacy PRF request");
    PrivacyCyclePrfInputV1::new(
        request,
        PrivacyCyclePrfOutputV1::new(output).expect("test privacy PRF input"),
    )
}
fn privacy_composition_budget_policy() -> PrivacyCompositionBudgetPolicyV1 {
    PrivacyCompositionBudgetPolicyV1 {
        budget_id: [0xB0; 32],
        epsilon_limit_numerator: 80,
        epsilon_limit_denominator: 1,
        max_publications: 100,
    }
}
fn privacy_aggregate_policy_config() -> config::PrivacyAggregatePolicyConfig {
    privacy_aggregate_policy_config_for_cycle(privacy_aggregate_cycle_config())
}
fn privacy_aggregate_policy_config_for_cycle(
    mut cycle: PrivacyAggregateCycleConfig,
) -> config::PrivacyAggregatePolicyConfig {
    // Runtime publication metadata is valid on generated records, but it
    // is intentionally excluded from the governed policy projection.
    cycle.metadata.clear();
    config::PrivacyAggregatePolicyConfig::new(cycle, privacy_composition_budget_policy())
        .expect("test privacy aggregate policy")
}
fn privacy_aggregate_storage_builder_without_fenced_target(
    root: &Path,
) -> config::StorageConfigBuilder {
    StorageConfig::builder()
        .enabled(true)
        .provider_id(Some(ProviderId::new([0x91; 32])))
        .data_dir(root.join("storage"))
        .privacy_aggregate_schedule(Some(privacy_aggregate_schedule_config()))
        .privacy_aggregate_policy(Some(privacy_aggregate_policy_config()))
        .privacy_cycle_prf_provider_binding(Some(test_privacy_cycle_prf_provider_binding()))
        .privacy_release_anchor_provider_binding(Some(test_privacy_release_anchor_binding()))
        .privacy_leader_lease_provider_binding(Some(test_transparency_leader_lease_binding()))
}
fn privacy_aggregate_storage_builder(root: &Path) -> config::StorageConfigBuilder {
    privacy_aggregate_storage_builder_without_fenced_target(root)
        .privacy_fenced_publisher_binding(Some(test_fenced_transparency_provider_binding()))
}
fn privacy_aggregate_storage_config(root: &Path) -> StorageConfig {
    with_test_signed_governance_config(privacy_aggregate_storage_builder(root), root).build()
}
fn privacy_aggregate_storage_config_with_temp_dir() -> (StorageConfig, TempDir) {
    let temp_dir = tempfile::tempdir().expect("create privacy aggregate temp dir");
    let root = temp_dir
        .path()
        .canonicalize()
        .expect("canonical privacy aggregate temp dir");
    (privacy_aggregate_storage_config(&root), temp_dir)
}
fn publish_due_test_privacy_cycle(
    handle: &NodeHandle,
    now_unix: u64,
    cycle_start_unix: u64,
    cycle_end_unix: u64,
    idempotency_key: &str,
) -> Result<PrivacyAggregateScheduleOutcome, GovernancePublishError> {
    handle.publish_due_privacy_aggregate_cycle_from_source_events(
        now_unix,
        privacy_aggregate_cycle_id([0xB0; 32], cycle_start_unix, cycle_end_unix),
        idempotency_key.to_owned(),
        privacy_aggregate_schedule_config(),
        privacy_aggregate_cycle_config(),
        Some(privacy_composition_budget_policy()),
    )
}
fn governance_submission_account(seed: u8) -> AccountId {
    let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("derive authenticated governance publisher key");
    AccountId::new(key.public_key().clone())
}
fn appeal_finance_report_fixture() -> SoraFsAppealFinanceReportV1 {
    SoraFsAppealFinanceReportV1 {
        version: SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
        report_id: [0x42; 16],
        case_id: "case-42".to_string(),
        round_id: Some("round-1".to_string()),
        generated_at_unix_ms: 1_800_000_031_000,
        appeal_finance_config_version: "baseline-v1".to_string(),
        evidence_bundle_digest: Some([0xA7; 32]),
        outcome: SoraFsAppealFinanceOutcomeV1::Overturn,
        deposit_xor: xor("420"),
        refund: SoraFsAppealFinanceAccountFlowV1 {
            account_id: "refund-account".to_string(),
            amount_xor: xor("420"),
        },
        treasury: SoraFsAppealFinanceAccountFlowV1 {
            account_id: "treasury-account".to_string(),
            amount_xor: xor("50"),
        },
        held: SoraFsAppealFinanceAccountFlowV1 {
            account_id: "escrow-account".to_string(),
            amount_xor: xor("0"),
        },
        panel_size: 3,
        panel_reward_total_xor: xor("85"),
        rewards_paid_total_xor: xor("60"),
        rewards_forfeited_treasury_xor: xor("25"),
        juror_payouts: vec![
            SoraFsAppealFinanceJurorPayoutV1 {
                juror_id: "juror-a".to_string(),
                stipend_xor: xor("25"),
                bonus_xor: xor("5"),
                total_xor: xor("30"),
            },
            SoraFsAppealFinanceJurorPayoutV1 {
                juror_id: "juror-b".to_string(),
                stipend_xor: xor("25"),
                bonus_xor: xor("5"),
                total_xor: xor("30"),
            },
        ],
        no_show_juror_ids: vec!["juror-c".to_string()],
    }
}
fn proof_token_issuance_fixture() -> ProofTokenIssuanceV1 {
    ProofTokenIssuanceV1 {
        version: iroha_data_model::sorafs::transparency::PROOF_TOKEN_ISSUANCE_VERSION_V1,
        token_id: [0x61; 16],
        issued_at_unix: 1_800_000_030,
        expires_at_unix: Some(1_800_086_430),
        moderation_action_code: 2,
        signer_key: [0x62; 32],
        token_blake3: [0x63; 32],
        blinded_digest: [0x64; 32],
        entry_ids: vec!["denylist/global".to_string(), "gar/policy/42".to_string()],
        evidence_digest: Some([0x65; 32]),
        policy_digest: Some([0x66; 32]),
        metadata: vec![
            iroha_data_model::sorafs::transparency::ModerationLedgerMetadataV1 {
                key: "issuer".to_string(),
                value: "gateway-a".to_string(),
            },
        ],
    }
}
const VALID_PROOF_TOKEN_SIGNER_HEX: &str =
    "f4bfda67d38a409557e4a910dbdf0a862ee5aa6cf6c2284aa38b0b82c4f16532";
const VALID_PROOF_TOKEN_B64: &str = "U0ZHVAEBAgAAAABrSdIeAAAAAGtLI55hYWFhYWFhYWFhYWFhYWFhAAIAD2RlbnlsaXN0L2dsb2JhbAANZ2FyL3BvbGljeS80MmRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkAEDHmshANx2cvkpmh1mCkrE94PJ6hL0A0qX4vQ-T3rWyTUKZG6uGoYM2sXbL36cYTahpsgcQ35z4R9bb1owinokB";
fn proof_token_signer_key_fixture() -> [u8; 32] {
    hex::decode(VALID_PROOF_TOKEN_SIGNER_HEX)
        .expect("valid proof-token signer hex")
        .try_into()
        .expect("proof-token signer key length")
}
fn appeal_finance_weekly_rollup_fixture() -> SoraFsAppealFinanceWeeklyRollupV1 {
    let report = appeal_finance_report_fixture();
    SoraFsAppealFinanceWeeklyRollupV1::from_reports(
        PorReportIsoWeek {
            year: 2026,
            week: 26,
        },
        1_800_000_100_000,
        &[report],
    )
    .expect("appeal finance weekly rollup fixture")
}
fn por_challenge_publication_fixture() -> PorChallengePublicationV1 {
    PorChallengePublicationV1::try_new(por_sample_challenge(), 0)
        .expect("PoR challenge publication fixture")
}
fn por_weekly_report_fixture() -> PorWeeklyReportV1 {
    let report = PorWeeklyReportV1 {
        version: sorafs_manifest::por::POR_WEEKLY_REPORT_VERSION_V1,
        cycle: PorReportIsoWeek {
            year: 2026,
            week: 30,
        },
        generated_at: 1_800_604_800,
        challenges_total: 2,
        challenges_verified: 1,
        challenges_failed: 1,
        forced_challenges: 0,
        repairs_enqueued: 1,
        repairs_completed: 1,
        mean_latency_ms: Some(80),
        p95_latency_ms: Some(130),
        slashing_events: Vec::new(),
        providers_missing_vrf: Vec::new(),
        top_offenders: Vec::new(),
        notes: None,
    };
    report.validate().expect("PoR weekly report fixture");
    report
}
fn appeal_finance_settlement_receipt_fixture() -> SoraFsAppealFinanceSettlementReceiptV1 {
    SoraFsAppealFinanceSettlementReceiptV1 {
        version: SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1,
        receipt_id: [0x52; 16],
        case_id: "case-42".to_string(),
        round_id: Some("round-1".to_string()),
        generated_at_unix_ms: 1_800_000_032_000,
        finalized_block_height: 42,
        finalized_block_hash: [0x43; 32],
        appeal_finance_config_version: "baseline-v1".to_string(),
        appeal_finance_policy_digest: [0x44; 32],
        outcome: SoraFsAppealFinanceOutcomeV1::Frivolous,
        escrow_id_hex: "11".repeat(32),
        payer_account: "payer-account".to_string(),
        destination_account: "escrow-account".to_string(),
        release_authority_account: Some("release-authority".to_string()),
        submitted_step: "drawdown_non_refund".to_string(),
        required_authority: "release-authority".to_string(),
        amount_xor: xor("420"),
        tx_hash_hex: "22".repeat(32),
        reconciliation_digest_hex: "33".repeat(32),
        reconciliation_status: "settled".to_string(),
        observed_lifecycle_status: "drawn_down".to_string(),
        observed_remaining_xor: xor("0"),
        deposit_xor: xor("420"),
        refund_xor: xor("0"),
        treasury_xor: xor("210"),
        held_xor: xor("210"),
        panel_size: 7,
        configured_signer_count: 1,
    }
}
#[test]
fn moderation_model_registry_admits_repro_manifest_and_rejects_conflict() {
    let (handle, _dir) = node_with_temp_storage();
    let manifest = moderation_repro_manifest_fixture(0x10, 0x30);
    let expected_manifest_digest = manifest.body.manifest_digest;
    let record = handle
        .admit_moderation_repro_manifest(manifest.clone())
        .expect("admit repro manifest");
    assert_eq!(record.manifest_id, [0x10; 16]);
    assert_eq!(record.manifest_digest, expected_manifest_digest);
    assert_eq!(record.runner_hash, [0x30; 32]);
    assert_eq!(record.model_count, 1);
    assert_eq!(record.signer_count, 1);
    let repeated = handle
        .admit_moderation_repro_manifest(manifest)
        .expect("re-admit matching repro manifest");
    assert_eq!(repeated, record);
    let err = handle
        .admit_moderation_repro_manifest(moderation_repro_manifest_fixture(0x10, 0x31))
        .expect_err("conflicting manifest id rejected");
    assert!(matches!(
        err,
        ModerationModelRegistryError::ConflictingReproManifest { .. }
    ));
    let snapshot = handle
        .export_moderation_model_registry_snapshot()
        .expect("export moderation model registry snapshot");
    assert_eq!(snapshot.reproducibility_manifests, vec![record]);
    assert!(snapshot.adversarial_corpora.is_empty());
}
#[test]
fn moderation_model_registry_admits_corpus_manifest_snapshot() {
    let (handle, _dir) = node_with_temp_storage();
    let manifest = adversarial_corpus_manifest_fixture();
    let expected_digest =
        *blake3::hash(&to_bytes(&manifest).expect("encode corpus fixture")).as_bytes();
    let record = handle
        .admit_moderation_corpus_manifest(manifest.clone())
        .expect("admit corpus manifest");
    assert_eq!(record.corpus_digest, expected_digest);
    assert_eq!(record.cohort_label.as_deref(), Some("sfm4a-2026-q1"));
    assert_eq!(record.family_count, 1);
    assert_eq!(record.variant_count, 2);
    let repeated = handle
        .admit_moderation_corpus_manifest(manifest)
        .expect("re-admit matching corpus manifest");
    assert_eq!(repeated, record);
    let snapshot = handle
        .export_moderation_model_registry_snapshot()
        .expect("export moderation model registry snapshot");
    assert!(snapshot.reproducibility_manifests.is_empty());
    assert_eq!(snapshot.adversarial_corpora, vec![record]);
}
#[test]
fn moderation_model_registry_checkpoint_persists_and_reloads_snapshot() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let source = NodeHandle::new(cfg.clone());
    let repro_record = source
        .admit_moderation_repro_manifest(moderation_repro_manifest_fixture(0x12, 0x32))
        .expect("admit repro manifest");
    let corpus_record = source
        .admit_moderation_corpus_manifest(adversarial_corpus_manifest_fixture())
        .expect("admit corpus manifest");
    let checkpoint_path = moderation_model_registry_checkpoint_path(cfg.data_dir());
    let checkpoint_bytes = fs::read(&checkpoint_path).expect("read registry checkpoint");
    let checkpoint: ModerationModelRegistrySnapshot =
        norito::decode_from_bytes(&checkpoint_bytes).expect("decode registry checkpoint");
    assert_eq!(
        checkpoint.reproducibility_manifests,
        vec![repro_record.clone()]
    );
    assert_eq!(checkpoint.adversarial_corpora, vec![corpus_record.clone()]);
    drop(source);
    let restored = NodeHandle::new(cfg);
    let restored_snapshot = restored
        .export_moderation_model_registry_snapshot()
        .expect("export restored registry snapshot");
    assert_eq!(
        restored_snapshot,
        ModerationModelRegistrySnapshot {
            reproducibility_manifests: vec![repro_record],
            adversarial_corpora: vec![corpus_record],
        }
    );
}
#[test]
fn moderation_model_registry_restore_rejects_duplicate_records() {
    let cfg = StorageConfig::builder().enabled(false).build();
    let handle = NodeHandle::new(cfg);
    let record = ModerationReproRegistryRecord {
        manifest_id: [0x14; 16],
        manifest_digest: [0x24; 32],
        runner_hash: [0x34; 32],
        runtime_version: "sorafs-ai-runner 1.0.0".to_string(),
        issued_at_unix: 1_800_000_040,
        model_count: 1,
        signer_count: 1,
    };
    let err = handle
        .restore_moderation_model_registry_snapshot(ModerationModelRegistrySnapshot {
            reproducibility_manifests: vec![record.clone(), record],
            adversarial_corpora: Vec::new(),
        })
        .expect_err("duplicate manifest ids rejected");
    assert!(matches!(
        err,
        ModerationModelRegistryError::InvalidRegistrySnapshot { .. }
    ));
    assert_eq!(
        handle
            .export_moderation_model_registry_snapshot()
            .expect("export unchanged model registry snapshot"),
        ModerationModelRegistrySnapshot::default()
    );
}
#[test]
fn moderation_screening_records_deterministic_quarantine_queue() {
    let cfg = StorageConfig::builder().enabled(false).build();
    let handle = NodeHandle::new(cfg);
    let input = moderation_screening_input_fixture(
        "cid:bafy-screening",
        ModerationScreeningVerdict::Quarantine,
    );
    let outcome = handle
        .record_moderation_screening_result(input.clone())
        .expect("record screening result");
    assert_eq!(outcome.record.subject, "cid:bafy-screening");
    assert_eq!(
        outcome.record.verdict,
        ModerationScreeningVerdict::Quarantine
    );
    assert_eq!(outcome.record.combined_score_bps, 6_500);
    assert_eq!(
        &outcome.record.record_digest[..16],
        outcome.record.record_id
    );
    let quarantine = outcome.quarantine.expect("quarantine record");
    assert_eq!(quarantine.screening_record_id, outcome.record.record_id);
    assert_eq!(quarantine.subject_digest, outcome.record.subject_digest);
    assert_eq!(quarantine.verdict, ModerationScreeningVerdict::Quarantine);
    assert_eq!(quarantine.state, ModerationQuarantineState::PendingReview);
    assert!(quarantine.reviewed_at_unix.is_none());
    assert!(quarantine.released_at_unix.is_none());
    let repeated = handle
        .record_moderation_screening_result(input)
        .expect("idempotent screening result");
    assert_eq!(repeated.record, outcome.record);
    assert_eq!(repeated.quarantine, Some(quarantine.clone()));
    let snapshot = handle
        .export_moderation_screening_snapshot()
        .expect("export moderation screening snapshot");
    assert_eq!(snapshot.screening_records, vec![outcome.record]);
    assert_eq!(snapshot.quarantine_records, vec![quarantine]);
}
#[test]
fn moderation_screening_pass_does_not_create_quarantine_record() {
    let cfg = StorageConfig::builder().enabled(false).build();
    let handle = NodeHandle::new(cfg);
    let outcome = handle
        .record_moderation_screening_result(moderation_screening_input_fixture(
            "cid:bafy-pass",
            ModerationScreeningVerdict::Pass,
        ))
        .expect("record pass result");
    assert!(outcome.quarantine.is_none());
    let snapshot = handle
        .export_moderation_screening_snapshot()
        .expect("export moderation screening snapshot");
    assert_eq!(snapshot.screening_records, vec![outcome.record]);
    assert!(snapshot.quarantine_records.is_empty());
}
#[test]
fn moderation_screening_checkpoint_persists_and_reloads_snapshot() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let source = NodeHandle::new(cfg.clone());
    let quarantine_outcome = source
        .record_moderation_screening_result(moderation_screening_input_fixture(
            "cid:bafy-quarantine",
            ModerationScreeningVerdict::Quarantine,
        ))
        .expect("record quarantine result");
    let pass_outcome = source
        .record_moderation_screening_result(moderation_screening_input_fixture(
            "cid:bafy-pass",
            ModerationScreeningVerdict::Pass,
        ))
        .expect("record pass result");
    let checkpoint_path = moderation_screening_checkpoint_path(cfg.data_dir());
    let checkpoint_bytes = fs::read(&checkpoint_path).expect("read screening checkpoint");
    let checkpoint: ModerationScreeningSnapshot =
        norito::decode_from_bytes(&checkpoint_bytes).expect("decode screening checkpoint");
    assert_eq!(checkpoint.screening_records.len(), 2);
    assert_eq!(checkpoint.quarantine_records.len(), 1);
    drop(source);
    let restored = NodeHandle::new(cfg);
    let restored_snapshot = restored
        .export_moderation_screening_snapshot()
        .expect("export restored screening snapshot");
    let mut expected_records = vec![quarantine_outcome.record, pass_outcome.record];
    expected_records.sort_by_key(|record| record.record_id);
    assert_eq!(
        restored_snapshot,
        ModerationScreeningSnapshot {
            screening_records: expected_records,
            quarantine_records: vec![quarantine_outcome.quarantine.expect("quarantine")],
            authenticated_admissions: Vec::new(),
        }
    );
}
#[test]
fn moderation_quarantine_review_and_release_updates_checkpoint() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let source = NodeHandle::new(cfg.clone());
    let outcome = source
        .record_moderation_screening_result(moderation_screening_input_fixture(
            "cid:bafy-review-release",
            ModerationScreeningVerdict::Quarantine,
        ))
        .expect("record quarantine result");
    let quarantine_id = outcome
        .quarantine
        .as_ref()
        .expect("quarantine record")
        .quarantine_id;
    let reviewed = source
        .review_moderation_quarantine_record(moderation_quarantine_review_input(quarantine_id))
        .expect("review quarantine record");
    assert_eq!(reviewed.state, ModerationQuarantineState::Reviewed);
    assert_eq!(reviewed.reviewed_at_unix, Some(1_800_000_060));
    assert_eq!(reviewed.reviewed_by.as_deref(), Some("operator@moderation"));
    assert!(reviewed.released_at_unix.is_none());
    let released = source
        .release_moderation_quarantine_record(moderation_quarantine_release_input(quarantine_id))
        .expect("release quarantine record");
    assert_eq!(released.state, ModerationQuarantineState::Released);
    assert_eq!(released.reviewed_at_unix, Some(1_800_000_060));
    assert_eq!(released.released_at_unix, Some(1_800_000_070));
    assert_eq!(
        released.release_authority.as_deref(),
        Some("release-authority@moderation")
    );
    drop(source);
    let restored = NodeHandle::new(cfg);
    let snapshot = restored
        .export_moderation_screening_snapshot()
        .expect("export restored screening snapshot");
    assert_eq!(snapshot.screening_records, vec![outcome.record]);
    assert_eq!(snapshot.quarantine_records, vec![released]);
}
#[test]
fn moderation_quarantine_object_store_persists_encrypted_payload_and_reloads() {
    let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
    let payload = b"quarantine payload bytes retained for operator review".to_vec();
    let source = node_with_test_quarantine_key_wrapper(cfg.clone());
    let (quarantine_id, record) = store_moderation_quarantine_fixture(
        &source,
        "cid:bafy-object-store",
        &payload,
        1_800_000_080,
        Some("application/octet-stream"),
    );
    assert_eq!(record.payload_digest, *blake3::hash(&payload).as_bytes());
    assert_eq!(record.payload_len, payload.len() as u64);
    assert_eq!(record.notes, None);
    let envelope_path =
        moderation_quarantine_object_store_root(cfg.data_dir()).join(&record.envelope_path);
    let envelope_bytes = fs::read(&envelope_path).expect("read encrypted envelope");
    let envelope: crate::moderation::ModerationQuarantineObjectEnvelopeV1 =
        norito::decode_from_bytes(&envelope_bytes).expect("decode encrypted envelope");
    assert_eq!(envelope.wrapping_key_id, "kms:test/quarantine-v1");
    assert!(!envelope.wrapped_dek.is_empty());
    assert!(!envelope.chunks.is_empty());
    assert!(
        !moderation_quarantine_object_store_root(cfg.data_dir())
            .join("local-seal.key")
            .exists(),
        "runtime wrapping keys must never be persisted in the object store"
    );
    assert!(
        !envelope_bytes
            .windows(payload.len())
            .any(|window| window == payload.as_slice()),
        "encrypted object envelope must not contain plaintext payload bytes"
    );
    let decrypted = source
        .read_moderation_quarantine_object(quarantine_id)
        .expect("read quarantine object");
    assert_eq!(decrypted.record, record);
    assert_eq!(decrypted.payload, payload);
    let replay = source
        .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
            quarantine_id,
            payload: decrypted.payload.clone(),
            captured_at_unix: 1_800_000_080,
            content_type: Some("application/octet-stream".to_owned()),
            notes: None,
        })
        .expect("idempotently replay quarantine object store");
    assert_eq!(replay, record);
    let index_path = moderation_quarantine_object_index_path(cfg.data_dir());
    let index_bytes = fs::read(&index_path).expect("read object index");
    let index: ModerationQuarantineObjectSnapshot =
        norito::decode_from_bytes(&index_bytes).expect("decode object index");
    assert_eq!(index.objects, vec![record.clone()]);
    drop(source);
    let restored = node_with_test_quarantine_key_wrapper(cfg);
    assert_eq!(
        restored
            .export_moderation_quarantine_object_snapshot()
            .expect("export restored object index")
            .objects,
        vec![record.clone()]
    );
    let restored_payload = restored
        .read_moderation_quarantine_object(quarantine_id)
        .expect("read restored object");
    assert_eq!(restored_payload.record, record);
    assert_eq!(restored_payload.payload, payload);
}
#[test]
fn moderation_quarantine_range_and_dek_rewrap_survive_restart() {
    let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
    let payload = (0..(crate::moderation::MODERATION_QUARANTINE_OBJECT_CHUNK_BYTES_V1 as usize
        + 8_192))
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let old_wrapper: Arc<dyn ModerationQuarantineKeyWrapper> = Arc::new(
        TestQuarantineKeyWrapper::single("kms:test/quarantine-old", 0x31),
    );
    let source =
        NodeHandle::try_new_with_quarantine_key_wrapper(cfg.clone(), Arc::clone(&old_wrapper))
            .expect("initialise with old wrapping key");
    let quarantine_id =
        record_moderation_quarantine_fixture(&source, "cid:bafy-object-range-rewrap", &payload);
    let record = source
        .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
            quarantine_id,
            payload: payload.clone(),
            captured_at_unix: 1_800_000_085,
            content_type: Some("application/octet-stream".to_owned()),
            notes: None,
        })
        .expect("store multi-chunk object");
    let range_start =
        u64::from(crate::moderation::MODERATION_QUARANTINE_OBJECT_CHUNK_BYTES_V1 - 1_024);
    let range_end = range_start + 4_096;
    let range = source
        .read_moderation_quarantine_object_range(quarantine_id, range_start, range_end)
        .expect("read authenticated cross-chunk range");
    assert_eq!(range.record, record);
    assert_eq!(range.start, range_start);
    assert_eq!(range.end, range_end);
    assert_eq!(
        range.payload,
        payload[range_start as usize..range_end as usize]
    );
    assert!(matches!(
        source
            .read_moderation_quarantine_object_range(
                quarantine_id,
                range_end,
                record.payload_len + 1,
            )
            .expect_err("out-of-bounds range rejected"),
        ModerationQuarantineObjectError::InvalidRange { .. }
    ));
    drop(source);
    let rotated_cfg = enabled_storage_builder(cfg.data_dir().clone())
        .moderation_quarantine_key_provider(Some(test_quarantine_key_provider_config_for(
            TEST_ROTATED_QUARANTINE_KEY_PROVIDER_QUALIFICATION,
        )))
        .build();
    let rotated_wrapper: Arc<dyn ModerationQuarantineKeyWrapper> =
        Arc::new(TestQuarantineKeyWrapper::rotated(
            "kms:test/quarantine-old",
            0x31,
            "kms:test/quarantine-new",
            0x52,
        ));
    assert!(matches!(
        NodeHandle::try_new_with_quarantine_key_wrapper(cfg.clone(), Arc::clone(&rotated_wrapper),),
        Err(NodeInitError::ModerationQuarantineKeyWrapperInvalid { .. })
    ));
    let rotated = NodeHandle::try_new_with_quarantine_key_wrapper(
        rotated_cfg.clone(),
        Arc::clone(&rotated_wrapper),
    )
    .expect("restart with rotation-capable wrapper");
    let envelope_path =
        moderation_quarantine_object_store_root(cfg.data_dir()).join(&record.envelope_path);
    let before_bytes = fs::read(&envelope_path).expect("read pre-rotation envelope");
    let before: crate::moderation::ModerationQuarantineObjectEnvelopeV1 =
        norito::decode_from_bytes(&before_bytes).expect("decode pre-rotation envelope");
    assert_eq!(before.wrapping_key_id, "kms:test/quarantine-old");
    assert_eq!(
        rotated
            .rewrap_moderation_quarantine_object_dek(quarantine_id)
            .expect("rewrap object DEK"),
        record
    );
    let after_bytes = fs::read(&envelope_path).expect("read rewrapped envelope");
    let after: crate::moderation::ModerationQuarantineObjectEnvelopeV1 =
        norito::decode_from_bytes(&after_bytes).expect("decode rewrapped envelope");
    assert_ne!(after_bytes, before_bytes);
    assert_eq!(after.wrapping_key_id, "kms:test/quarantine-new");
    assert_eq!(after.object_id, before.object_id);
    assert_eq!(after.ciphertext_digest, before.ciphertext_digest);
    assert_eq!(after.chunks, before.chunks);
    assert_eq!(
        rotated
            .read_moderation_quarantine_object(quarantine_id)
            .expect("read rewrapped object")
            .payload,
        payload
    );
    drop(rotated);
    let new_only_wrapper: Arc<dyn ModerationQuarantineKeyWrapper> =
        Arc::new(TestQuarantineKeyWrapper::single_with_qualification(
            "kms:test/quarantine-new",
            0x52,
            TEST_ROTATED_QUARANTINE_KEY_PROVIDER_QUALIFICATION,
        ));
    let restored = NodeHandle::try_new_with_quarantine_key_wrapper(rotated_cfg, new_only_wrapper)
        .expect("restart using only the replacement key");
    assert_eq!(
        restored
            .read_moderation_quarantine_object(quarantine_id)
            .expect("read after rewrap restart")
            .payload,
        payload
    );
}
#[test]
fn moderation_quarantine_startup_recovers_canonical_unindexed_envelope() {
    let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
    let payload = b"crash between envelope rename and index commit".to_vec();
    let source = node_with_test_quarantine_key_wrapper(cfg.clone());
    let (_, record) = store_moderation_quarantine_fixture(
        &source,
        "cid:bafy-object-crash-orphan",
        &payload,
        1_800_000_086,
        None,
    );
    let envelope_path =
        moderation_quarantine_object_store_root(cfg.data_dir()).join(&record.envelope_path);
    source
        .persist_moderation_quarantine_object_index_snapshot(
            &ModerationQuarantineObjectSnapshot::default(),
        )
        .expect("simulate index state before interrupted insertion");
    drop(source);
    let restored = node_with_test_quarantine_key_wrapper(cfg);
    assert!(
        restored
            .export_moderation_quarantine_object_snapshot()
            .expect("export moderation quarantine object snapshot")
            .objects
            .is_empty()
    );
    assert!(
        !envelope_path.exists(),
        "startup recovery must durably remove a canonical unindexed envelope"
    );
}
#[test]
fn moderation_quarantine_object_store_rejects_digest_mismatch() {
    let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
    let expected_payload = b"expected quarantined bytes".to_vec();
    let handle = node_with_test_quarantine_key_wrapper(cfg);
    let quarantine_id =
        record_moderation_quarantine_fixture(&handle, "cid:bafy-object-digest", &expected_payload);
    let err = handle
        .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
            quarantine_id,
            payload: b"different bytes".to_vec(),
            captured_at_unix: 1_800_000_081,
            content_type: None,
            notes: None,
        })
        .expect_err("digest mismatch rejected");
    assert!(matches!(
        err,
        ModerationQuarantineObjectError::DigestMismatch { .. }
    ));
    assert!(
        handle
            .export_moderation_quarantine_object_snapshot()
            .expect("export moderation quarantine object snapshot")
            .objects
            .is_empty()
    );
}
#[test]
fn moderation_quarantine_object_read_rejects_tampered_envelope() {
    let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
    let payload = b"tamper-detected quarantine payload".to_vec();
    let handle = node_with_test_quarantine_key_wrapper(cfg.clone());
    let (quarantine_id, record) = store_moderation_quarantine_fixture(
        &handle,
        "cid:bafy-object-tamper",
        &payload,
        1_800_000_082,
        None,
    );
    let envelope_path =
        moderation_quarantine_object_store_root(cfg.data_dir()).join(&record.envelope_path);
    let envelope_bytes = fs::read(&envelope_path).expect("read envelope");
    let mut envelope: crate::moderation::ModerationQuarantineObjectEnvelopeV1 =
        norito::decode_from_bytes(&envelope_bytes).expect("decode envelope");
    envelope.chunks[0].ciphertext[0] ^= 0x01;
    let tampered_bytes = norito::to_bytes(&envelope).expect("encode tampered envelope");
    fs::write(&envelope_path, tampered_bytes).expect("write tampered envelope");
    let err = handle
        .read_moderation_quarantine_object(quarantine_id)
        .expect_err("tampered envelope rejected");
    assert!(matches!(
        err,
        ModerationQuarantineObjectError::AuthenticationFailed { .. }
    ));
}
#[test]
fn moderation_quarantine_store_rejects_authenticated_envelope_tampering_on_restart() {
    let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
    let payload = b"restart audit must authenticate every quarantine envelope".to_vec();
    let handle = node_with_test_quarantine_key_wrapper(cfg.clone());
    let (_, record) = store_moderation_quarantine_fixture(
        &handle,
        "cid:bafy-object-restart-tamper",
        &payload,
        1_800_000_083,
        None,
    );
    drop(handle);
    let envelope_path =
        moderation_quarantine_object_store_root(cfg.data_dir()).join(&record.envelope_path);
    let bytes = fs::read(&envelope_path).expect("read envelope");
    let mut envelope: crate::moderation::ModerationQuarantineObjectEnvelopeV1 =
        norito::decode_from_bytes(&bytes).expect("decode envelope");
    envelope.chunks[0].ciphertext[0] ^= 0x80;
    fs::write(
        &envelope_path,
        norito::to_bytes(&envelope).expect("encode canonical tampered envelope"),
    )
    .expect("write tampered envelope");
    assert_checkpoint_component!("moderation quarantine object envelope" =>
        NodeHandle::try_new_with_quarantine_key_wrapper(cfg, test_quarantine_key_wrapper())
    );
}
#[test]
fn moderation_quarantine_store_requires_runtime_wrapper_and_rejects_unknown_orphans() {
    let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
    let payload = b"indexed quarantine object requires its runtime key wrapper".to_vec();
    let handle = node_with_test_quarantine_key_wrapper(cfg.clone());
    store_moderation_quarantine_fixture(
        &handle,
        "cid:bafy-object-missing-wrapper",
        &payload,
        1_800_000_084,
        None,
    );
    drop(handle);
    assert!(matches!(
        NodeHandle::try_new(cfg.clone()),
        Err(NodeInitError::ModerationQuarantineKeyWrapperUnavailable)
    ));
    let orphan = moderation_quarantine_object_store_root(cfg.data_dir()).join("orphan.to");
    fs::write(&orphan, b"not indexed").expect("write orphan object");
    assert_checkpoint_component!("moderation quarantine object store" =>
        NodeHandle::try_new_with_quarantine_key_wrapper(cfg, test_quarantine_key_wrapper())
    );
}
#[cfg(unix)]
#[test]
fn moderation_quarantine_store_rejects_symlink_entries_on_restart() {
    use std::os::unix::fs::symlink;
    let (cfg, _dir) = storage_config_with_temp_dir();
    drop(NodeHandle::new(cfg.clone()));
    let victim = cfg.data_dir().join("quarantine-symlink-victim");
    fs::write(&victim, b"victim").expect("write symlink victim");
    let link = moderation_quarantine_object_store_root(cfg.data_dir()).join("orphan-link.to");
    symlink(&victim, &link).expect("create object-store symlink");
    assert_checkpoint_component!("moderation quarantine object store" =>
        NodeHandle::try_new(cfg)
    );
    assert_eq!(fs::read(victim).expect("victim remains intact"), b"victim");
}
#[test]
fn moderation_snapshot_restore_rejects_dangling_cross_checkpoint_references() {
    let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
    let handle = node_with_test_quarantine_key_wrapper(cfg);
    seed_moderation_evidence_viewer_activity(
        &handle,
        "cid:bafy-cross-checkpoint-refs",
        b"cross-checkpoint reference fixture",
        1_800_000_100_000,
        1_800_000_200_000,
        &[],
    );
    let screening_before = handle
        .export_moderation_screening_snapshot()
        .expect("export moderation screening snapshot");
    let objects_before = handle
        .export_moderation_quarantine_object_snapshot()
        .expect("export moderation quarantine object snapshot");
    let screening_error = handle
        .restore_moderation_screening_snapshot(ModerationScreeningSnapshot::default())
        .expect_err("referenced quarantine cannot be removed");
    assert!(matches!(
        screening_error,
        ModerationScreeningError::InvalidSnapshot { .. }
    ));
    assert_eq!(
        handle
            .export_moderation_screening_snapshot()
            .expect("export moderation screening snapshot"),
        screening_before
    );
    let object_error =
        handle
            .restore_moderation_quarantine_object_snapshot(
                ModerationQuarantineObjectSnapshot::default(),
            )
            .expect_err("viewer-referenced object cannot be removed");
    assert!(matches!(
        object_error,
        ModerationQuarantineObjectError::InvalidSnapshot { .. }
    ));
    assert_eq!(
        handle
            .export_moderation_quarantine_object_snapshot()
            .expect("export moderation quarantine object snapshot"),
        objects_before
    );
}
#[test]
fn moderation_evidence_viewer_session_access_persists_and_reloads() {
    let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
    let payload = b"payload-free evidence viewer audit fixture".to_vec();
    let source = node_with_test_quarantine_key_wrapper(cfg.clone());
    let (quarantine_id, object) = store_moderation_quarantine_fixture(
        &source,
        "cid:bafy-evidence-viewer",
        &payload,
        1_800_000_090,
        Some("application/octet-stream"),
    );
    let session = source
        .create_moderation_evidence_viewer_session(moderation_evidence_viewer_session_input(
            quarantine_id,
            1_800_000_000_000,
            1_800_000_300_000,
        ))
        .expect("create viewer session");
    assert_eq!(session.quarantine_id, quarantine_id);
    assert_eq!(session.object_id, object.object_id);
    assert_eq!(session.evidence_digest, *blake3::hash(&payload).as_bytes());
    assert_eq!(session.viewer_role, "juror");
    assert_eq!(
        session.session_id.as_slice(),
        &session.session_manifest_digest[..16]
    );
    let access = source
        .record_moderation_evidence_viewer_access(moderation_evidence_viewer_access_input(
            session.session_id,
            ModerationEvidenceViewerAccessKind::Viewed,
            1_800_000_010_000,
        ))
        .expect("record viewer access");
    assert_eq!(access.sequence, 1);
    assert_eq!(access.session_id, session.session_id);
    assert_eq!(access.quarantine_id, quarantine_id);
    assert_eq!(access.kind, ModerationEvidenceViewerAccessKind::Viewed);
    let checkpoint_path = moderation_evidence_viewer_checkpoint_path(cfg.data_dir());
    assert!(
        checkpoint_path.exists(),
        "evidence viewer checkpoint must persist when storage is enabled"
    );
    drop(source);
    let restored = node_with_test_quarantine_key_wrapper(cfg);
    let snapshot = restored
        .export_moderation_evidence_viewer_snapshot()
        .expect("export restored evidence viewer snapshot");
    assert_eq!(snapshot.sessions, vec![session]);
    assert_eq!(snapshot.access_events, vec![access]);
}
#[test]
fn moderation_evidence_viewer_session_rejects_missing_object_and_payload_material() {
    let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
    let payload = b"evidence viewer missing object fixture".to_vec();
    let handle = node_with_test_quarantine_key_wrapper(cfg);
    let quarantine_id =
        record_moderation_quarantine_fixture(&handle, "cid:bafy-evidence-viewer-missing", &payload);
    let err = handle
        .create_moderation_evidence_viewer_session(moderation_evidence_viewer_session_input(
            quarantine_id,
            1_800_000_000_000,
            1_800_000_300_000,
        ))
        .expect_err("missing object rejected");
    assert!(matches!(
        err,
        ModerationEvidenceViewerError::MissingObject { .. }
    ));
    handle
        .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
            quarantine_id,
            payload,
            captured_at_unix: 1_800_000_091,
            content_type: None,
            notes: None,
        })
        .expect("store quarantine object");
    let mut unsafe_input = moderation_evidence_viewer_session_input(
        quarantine_id,
        1_800_000_000_000,
        1_800_000_300_000,
    );
    unsafe_input.raw_evidence_included = true;
    let err = handle
        .create_moderation_evidence_viewer_session(unsafe_input)
        .expect_err("raw evidence marker rejected");
    assert!(matches!(
        err,
        ModerationEvidenceViewerError::PayloadSafetyViolation { .. }
    ));
}
#[test]
fn moderation_evidence_viewer_access_rejects_expiry_and_tampered_snapshot() {
    let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
    let payload = b"evidence viewer expired access fixture".to_vec();
    let handle = node_with_test_quarantine_key_wrapper(cfg);
    let (quarantine_id, _) = store_moderation_quarantine_fixture(
        &handle,
        "cid:bafy-evidence-viewer-expired",
        &payload,
        1_800_000_092,
        None,
    );
    let session = handle
        .create_moderation_evidence_viewer_session(moderation_evidence_viewer_session_input(
            quarantine_id,
            1_800_000_000_000,
            1_800_000_300_000,
        ))
        .expect("create viewer session");
    let err = handle
        .record_moderation_evidence_viewer_access(moderation_evidence_viewer_access_input(
            session.session_id,
            ModerationEvidenceViewerAccessKind::Viewed,
            1_800_000_300_000,
        ))
        .expect_err("expired normal access rejected");
    assert!(matches!(
        err,
        ModerationEvidenceViewerError::ExpiredSession { .. }
    ));
    let expiry_event = handle
        .record_moderation_evidence_viewer_access(moderation_evidence_viewer_access_input(
            session.session_id,
            ModerationEvidenceViewerAccessKind::SessionExpired,
            1_800_000_300_000,
        ))
        .expect("record session expiry anomaly");
    assert_eq!(
        expiry_event.kind,
        ModerationEvidenceViewerAccessKind::SessionExpired
    );
    let mut tampered = handle
        .export_moderation_evidence_viewer_snapshot()
        .expect("export moderation evidence viewer snapshot");
    tampered.sessions[0].evidence_digest = [0x44; 32];
    let err = handle
        .restore_moderation_evidence_viewer_snapshot(tampered)
        .expect_err("tampered evidence digest rejected");
    assert!(matches!(
        err,
        ModerationEvidenceViewerError::InvalidSnapshot { .. }
    ));
}
#[test]
fn moderation_evidence_viewer_audit_report_records_transparency_source_entry() {
    use iroha_data_model::sorafs::transparency::ModerationLedgerEntryKindV1;
    let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
    let payload = b"evidence viewer audit report fixture".to_vec();
    let handle = node_with_test_quarantine_key_wrapper(cfg);
    let publisher = Arc::new(RecordingPublisher::default());
    let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
    handle.set_governance_publisher(trait_publisher);
    let (quarantine_id, _) = store_moderation_quarantine_fixture(
        &handle,
        "cid:bafy-evidence-viewer-report",
        &payload,
        1_800_000_093,
        None,
    );
    let session = handle
        .create_moderation_evidence_viewer_session(moderation_evidence_viewer_session_input(
            quarantine_id,
            1_800_000_000_000,
            1_800_000_300_000,
        ))
        .expect("create viewer session");
    let mut seeked = moderation_evidence_viewer_access_input(
        session.session_id,
        ModerationEvidenceViewerAccessKind::Seeked,
        1_800_000_020_000,
    );
    seeked.request_digest = [0xD8; 32];
    handle
        .record_moderation_evidence_viewer_access(moderation_evidence_viewer_access_input(
            session.session_id,
            ModerationEvidenceViewerAccessKind::Viewed,
            1_800_000_010_000,
        ))
        .expect("record viewer access");
    handle
        .record_moderation_evidence_viewer_access(seeked)
        .expect("record seek access");
    let result = handle
        .record_moderation_evidence_viewer_audit_report(
            moderation_evidence_viewer_audit_report_input(
                1_800_000_000,
                1_800_086_400,
                1_800_086_401,
            ),
        )
        .expect("record audit report");
    assert_eq!(result.report.session_count, 1);
    assert_eq!(result.report.logged_session_count, 1);
    assert_eq!(result.report.access_event_count, 2);
    assert_eq!(
        result
            .report
            .access_kind_counts
            .iter()
            .map(|count| (count.kind.as_str(), count.count))
            .collect::<Vec<_>>(),
        vec![("seeked", 1), ("viewed", 1)]
    );
    assert_eq!(
        result.source_entry.kind,
        ModerationLedgerEntryKindV1::EvidenceAccess
    );
    assert_eq!(result.source_entry.occurred_at_unix, 1_800_086_399);
    assert_eq!(handle.transparency_ledger_source_entry_count(), 1);
    assert!(
        result
            .source_entry
            .metadata
            .iter()
            .any(|item| item.key == "viewer_accounts_included" && item.value == "false")
    );
    assert!(
        result
            .source_entry
            .metadata
            .iter()
            .all(|item| !item.value.contains("juror-1@moderation"))
    );
    handle
        .record_moderation_evidence_viewer_audit_report(
            moderation_evidence_viewer_audit_report_input(
                1_800_000_000,
                1_800_086_400,
                1_800_086_401,
            ),
        )
        .expect("duplicate report export is idempotent");
    assert_eq!(handle.transparency_ledger_source_entry_count(), 1);
    let publication = handle
        .publish_transparency_ledger_cycle_from_source_entries(
            *b"cycle-evrpt00001",
            1_800_000_000,
            1_800_086_402,
            1_800_086_403,
            None,
        )
        .expect("publish evidence viewer report source cycle");
    assert_eq!(publication.block.entry_count, 1);
    assert_eq!(
        publication.proofs[0].entry.kind,
        ModerationLedgerEntryKindV1::EvidenceAccess
    );
    let published = publisher.take();
    assert_eq!(published.len(), 1);
}
#[test]
fn moderation_evidence_viewer_audit_report_publish_due_publishes_and_is_idempotent() {
    use iroha_data_model::sorafs::transparency::ModerationLedgerEntryKindV1;
    let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
    let handle = node_with_test_quarantine_key_wrapper(cfg);
    let publisher = Arc::new(RecordingPublisher::default());
    let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
    handle.set_governance_publisher(trait_publisher);
    seed_moderation_evidence_viewer_activity(
        &handle,
        "cid:bafy-evidence-viewer-due-report",
        b"evidence viewer due report fixture",
        1_800_000_000_000,
        1_800_000_060_000,
        &[
            (
                ModerationEvidenceViewerAccessKind::Viewed,
                1_800_000_010_000,
            ),
            (
                ModerationEvidenceViewerAccessKind::DownloadAttempted,
                1_800_000_020_000,
            ),
        ],
    );
    let schedule = PrivacyAggregateScheduleConfig {
        first_cycle_start_unix: 1_800_000_000,
        cycle_seconds: 100,
        publish_delay_seconds: 10,
    };
    let outcome = handle
        .publish_due_moderation_evidence_viewer_audit_report(
            1_800_000_110,
            schedule,
            "local-daily".to_string(),
            Some([0xF7; 32]),
            None,
        )
        .expect("publish due evidence-viewer report");
    let ModerationEvidenceViewerAuditScheduleOutcome::Published {
        window,
        report,
        source_entry,
        publication,
    } = outcome
    else {
        panic!("expected published evidence-viewer audit report");
    };
    assert_eq!(window.cycle_start_unix, 1_800_000_000);
    assert_eq!(window.cycle_end_unix, 1_800_000_100);
    assert_eq!(window.due_at_unix, 1_800_000_110);
    assert_eq!(report.session_count, 1);
    assert_eq!(report.logged_session_count, 1);
    assert_eq!(report.access_event_count, 2);
    assert_eq!(
        source_entry.kind,
        ModerationLedgerEntryKindV1::EvidenceAccess
    );
    assert_eq!(source_entry.occurred_at_unix, 1_800_000_099);
    assert_eq!(publication.block.entry_count, 1);
    assert_eq!(publication.proofs.len(), 1);
    assert_eq!(
        publication.proofs[0].entry.kind,
        ModerationLedgerEntryKindV1::EvidenceAccess
    );
    assert_eq!(handle.transparency_ledger_source_entry_count(), 0);
    assert_eq!(publisher.take().len(), 1);
    let repeat = handle
        .publish_due_moderation_evidence_viewer_audit_report(
            1_800_000_110,
            schedule,
            "local-daily".to_string(),
            Some([0xF7; 32]),
            None,
        )
        .expect("repeat due evidence-viewer report");
    assert!(matches!(
        repeat,
        ModerationEvidenceViewerAuditScheduleOutcome::AlreadyPublished { .. }
    ));
    assert_eq!(handle.transparency_ledger_source_entry_count(), 0);
    assert_eq!(publisher.take().len(), 0);
}
#[test]
fn moderation_evidence_viewer_audit_report_publish_due_configured_uses_storage_config() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let schedule = PrivacyAggregateScheduleConfig {
        first_cycle_start_unix: 1_800_000_000,
        cycle_seconds: 100,
        publish_delay_seconds: 10,
    };
    let cfg = enabled_storage_builder(root.join("storage"))
        .moderation_quarantine_key_provider(Some(test_quarantine_key_provider_config()))
        .evidence_viewer_audit_schedule(Some(schedule))
        .build();
    let handle = node_with_test_quarantine_key_wrapper(cfg);
    assert_eq!(
        handle.configured_evidence_viewer_audit_schedule(),
        Some(schedule)
    );
    let publisher = Arc::new(RecordingPublisher::default());
    let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
    handle.set_governance_publisher(trait_publisher);
    seed_moderation_evidence_viewer_activity(
        &handle,
        "cid:bafy-evidence-viewer-configured-due-report",
        b"configured evidence viewer due report fixture",
        1_800_000_000_000,
        1_800_000_060_000,
        &[(
            ModerationEvidenceViewerAccessKind::Viewed,
            1_800_000_010_000,
        )],
    );
    let outcome = handle
        .publish_due_configured_moderation_evidence_viewer_audit_report(
            1_800_000_110,
            "local-daily".to_string(),
            Some([0xF7; 32]),
            None,
        )
        .expect("publish configured due evidence-viewer report");
    let ModerationEvidenceViewerAuditScheduleOutcome::Published {
        window,
        report,
        publication,
        ..
    } = outcome
    else {
        panic!("expected configured published evidence-viewer audit report");
    };
    assert_eq!(window.cycle_start_unix, 1_800_000_000);
    assert_eq!(window.cycle_end_unix, 1_800_000_100);
    assert_eq!(report.session_count, 1);
    assert_eq!(report.access_event_count, 1);
    assert_eq!(publication.block.entry_count, 1);
    assert_eq!(publisher.take().len(), 1);
}
#[test]
fn moderation_evidence_viewer_audit_report_publish_due_configured_skips_when_disabled() {
    let cfg = StorageConfig::builder()
        .enabled(false)
        .evidence_viewer_audit_schedule(None)
        .build();
    let handle = NodeHandle::new(cfg);
    assert_eq!(handle.configured_evidence_viewer_audit_schedule(), None);
    let outcome = handle
        .publish_due_configured_moderation_evidence_viewer_audit_report(
            1_800_000_110,
            "local-daily".to_string(),
            None,
            None,
        )
        .expect("disabled configured evidence-viewer report");
    assert_eq!(
        outcome,
        ModerationEvidenceViewerAuditScheduleOutcome::Disabled
    );
}
#[test]
fn moderation_evidence_viewer_audit_report_publish_due_reports_empty_and_bad_schedules() {
    let cfg = StorageConfig::builder().enabled(false).build();
    let empty = NodeHandle::new(cfg);
    let schedule = PrivacyAggregateScheduleConfig {
        first_cycle_start_unix: 1_800_000_000,
        cycle_seconds: 100,
        publish_delay_seconds: 10,
    };
    let outcome = empty
        .publish_due_moderation_evidence_viewer_audit_report(
            1_800_000_110,
            schedule,
            "local-daily".to_string(),
            None,
            None,
        )
        .expect("empty due check");
    assert!(matches!(
        outcome,
        ModerationEvidenceViewerAuditScheduleOutcome::NoSourceEvents { .. }
    ));
    let err = empty
        .publish_due_moderation_evidence_viewer_audit_report(
            1_800_000_110,
            PrivacyAggregateScheduleConfig {
                first_cycle_start_unix: 1_800_000_000,
                cycle_seconds: 0,
                publish_delay_seconds: 10,
            },
            "local-daily".to_string(),
            None,
            None,
        )
        .expect_err("zero cycle rejected");
    assert!(err.to_string().contains("evidence viewer audit schedule"));
    let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
    let handle = node_with_test_quarantine_key_wrapper(cfg);
    seed_moderation_evidence_viewer_activity(
        &handle,
        "cid:bafy-evidence-viewer-oversized-due-report",
        b"evidence viewer oversized due report fixture",
        1_800_000_000_000,
        1_800_000_060_000,
        &[(
            ModerationEvidenceViewerAccessKind::Viewed,
            1_800_000_010_000,
        )],
    );
    let oversized = PrivacyAggregateScheduleConfig {
        first_cycle_start_unix: 1_799_992_033,
        cycle_seconds: 86_401,
        publish_delay_seconds: 1,
    };
    let due_at_unix = oversized
        .event_window(1_800_000_010)
        .expect("oversized event window")
        .due_at_unix;
    let err = handle
        .publish_due_moderation_evidence_viewer_audit_report(
            due_at_unix,
            oversized,
            "local-daily".to_string(),
            None,
            None,
        )
        .expect_err("oversized report window rejected");
    assert!(
        err.to_string()
            .contains("record evidence viewer audit report")
    );
}
#[test]
fn moderation_evidence_viewer_audit_report_rejects_unsafe_and_tampered_inputs() {
    let cfg = StorageConfig::builder().enabled(false).build();
    let handle = NodeHandle::new(cfg);
    let mut unsafe_input =
        moderation_evidence_viewer_audit_report_input(1_800_000_000, 1_800_086_400, 1_800_086_401);
    unsafe_input.viewer_accounts_included = true;
    let err = handle
        .build_moderation_evidence_viewer_audit_report(unsafe_input)
        .expect_err("viewer account export rejected");
    assert!(matches!(
        err,
        ModerationEvidenceViewerError::PayloadSafetyViolation { .. }
    ));
    let err = handle
        .build_moderation_evidence_viewer_audit_report(
            moderation_evidence_viewer_audit_report_input(
                1_800_000_000,
                1_800_172_801,
                1_800_172_802,
            ),
        )
        .expect_err("oversized report window rejected");
    assert!(matches!(
        err,
        ModerationEvidenceViewerError::InvalidInput { .. }
    ));
    let mut report = handle
        .build_moderation_evidence_viewer_audit_report(
            moderation_evidence_viewer_audit_report_input(
                1_800_000_000,
                1_800_086_400,
                1_800_086_401,
            ),
        )
        .expect("empty report is valid");
    report.access_event_count = 1;
    assert!(
        moderation_evidence_viewer_audit_report_source_entry(&report)
            .expect_err("tampered report rejected")
            .to_string()
            .contains("access-kind counts do not sum")
    );
}
#[test]
fn moderation_quarantine_release_requires_review() {
    let cfg = StorageConfig::builder().enabled(false).build();
    let handle = NodeHandle::new(cfg);
    let outcome = handle
        .record_moderation_screening_result(moderation_screening_input_fixture(
            "cid:bafy-release-before-review",
            ModerationScreeningVerdict::Quarantine,
        ))
        .expect("record quarantine result");
    let quarantine_id = outcome.quarantine.expect("quarantine record").quarantine_id;
    let err = handle
        .release_moderation_quarantine_record(moderation_quarantine_release_input(quarantine_id))
        .expect_err("release before review rejected");
    assert!(matches!(
        err,
        ModerationScreeningError::InvalidTransition { .. }
    ));
    assert_eq!(
        handle
            .export_moderation_screening_snapshot()
            .expect("export moderation screening snapshot")
            .quarantine_records
            .first()
            .map(|record| record.state),
        Some(ModerationQuarantineState::PendingReview)
    );
}
#[test]
fn moderation_screening_restore_rejects_tampered_digest() {
    let cfg = StorageConfig::builder().enabled(false).build();
    let handle = NodeHandle::new(cfg);
    let outcome = handle
        .record_moderation_screening_result(moderation_screening_input_fixture(
            "cid:bafy-tamper",
            ModerationScreeningVerdict::Quarantine,
        ))
        .expect("record screening result");
    let mut tampered = outcome.record;
    tampered.record_digest[0] ^= 0xFF;
    let err = handle
        .restore_moderation_screening_snapshot(ModerationScreeningSnapshot {
            screening_records: vec![tampered],
            quarantine_records: Vec::new(),
            authenticated_admissions: Vec::new(),
        })
        .expect_err("tampered digest rejected");
    assert!(matches!(
        err,
        ModerationScreeningError::InvalidSnapshot { .. }
    ));
}
#[test]
fn manifest_metadata_resolves_by_digest() {
    let (handle, _dir) = node_with_temp_storage();
    let payload = b"digest-lookup-fixture";
    let plan = CarBuildPlan::single_file(payload).expect("plan");
    let manifest = manifest_builder_for_plan(payload, &plan)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("manifest");
    let mut reader = payload.as_slice();
    let manifest_id = handle
        .ingest_manifest(&manifest, &plan, &mut reader)
        .expect("ingest manifest");
    let manifest_digest: [u8; 32] = manifest.digest().expect("manifest digest").into();
    let by_id = handle
        .manifest_metadata(&manifest_id)
        .expect("lookup by id");
    let by_digest = handle
        .manifest_metadata_by_digest(&manifest_digest)
        .expect("lookup by digest");
    assert_eq!(by_digest.manifest_id(), manifest_id);
    assert_eq!(by_digest.manifest_digest(), &manifest_digest);
    assert_eq!(by_id.manifest_digest(), by_digest.manifest_digest());
}
#[test]
fn moderation_state_limit_allows_boundary_replays_and_existing_updates() {
    let cfg = StorageConfig::builder()
        .enabled(false)
        .runtime_retention(RuntimeRetentionPolicy::new(1, 1, 1024 * 1024))
        .build();
    let handle = NodeHandle::new(cfg);
    let repro = moderation_repro_manifest_fixture(0x11, 0x31);
    let admitted = handle
        .admit_moderation_repro_manifest(repro.clone())
        .expect("admit repro at boundary");
    assert_eq!(
        handle
            .admit_moderation_repro_manifest(repro)
            .expect("replay repro at capacity"),
        admitted
    );
    assert!(matches!(
        handle
            .admit_moderation_repro_manifest(moderation_repro_manifest_fixture(0x12, 0x32,))
            .expect_err("new repro above capacity must fail"),
        ModerationModelRegistryError::ResourceExhausted {
            resource: "reproducibility_manifests",
            limit: 1
        }
    ));
    let corpus = adversarial_corpus_manifest_fixture();
    let admitted = handle
        .admit_moderation_corpus_manifest(corpus.clone())
        .expect("admit corpus at boundary");
    assert_eq!(
        handle
            .admit_moderation_corpus_manifest(corpus.clone())
            .expect("replay corpus at capacity"),
        admitted
    );
    let mut second_corpus = corpus;
    second_corpus.issued_at_unix += 1;
    assert!(matches!(
        handle
            .admit_moderation_corpus_manifest(second_corpus)
            .expect_err("new corpus above capacity must fail"),
        ModerationModelRegistryError::ResourceExhausted {
            resource: "adversarial_corpora",
            limit: 1
        }
    ));
    let screening =
        moderation_screening_input_fixture("limit-subject", ModerationScreeningVerdict::Quarantine);
    let first = handle
        .record_moderation_screening_result(screening.clone())
        .expect("record screening at boundary");
    assert_eq!(
        handle
            .record_moderation_screening_result(screening)
            .expect("replay screening at capacity")
            .record,
        first.record
    );
    let quarantine_id = first
        .quarantine
        .expect("quarantine at boundary")
        .quarantine_id;
    handle
        .review_moderation_quarantine_record(moderation_quarantine_review_input(quarantine_id))
        .expect("review existing quarantine at capacity");
    handle
        .release_moderation_quarantine_record(moderation_quarantine_release_input(quarantine_id))
        .expect("release existing quarantine at capacity");
    assert!(matches!(
        handle
            .record_moderation_screening_result(moderation_screening_input_fixture(
                "second-subject",
                ModerationScreeningVerdict::Pass,
            ))
            .expect_err("new screening above capacity must fail"),
        ModerationScreeningError::ResourceExhausted {
            resource: "screening_records",
            limit: 1
        }
    ));
}
#[test]
fn moderation_object_viewer_limits_and_checkpoints_survive_restart() {
    let (base, _dir) = storage_config_with_temp_dir();
    let cfg = enabled_storage_builder(base.data_dir().clone())
        .moderation_quarantine_key_provider(Some(test_quarantine_key_provider_config()))
        .runtime_retention(RuntimeRetentionPolicy::new(2, 2, 2 * 1024 * 1024))
        .build();
    let handle = node_with_test_quarantine_key_wrapper(cfg.clone());
    let mut sessions = Vec::new();
    let mut session_inputs = Vec::new();
    for index in 0_u8..2 {
        let payload = vec![index.saturating_add(1); 32];
        let mut screening = moderation_screening_input_fixture(
            &format!("restart-viewer-{index}"),
            ModerationScreeningVerdict::Quarantine,
        );
        screening.subject_digest = *blake3::hash(&payload).as_bytes();
        screening.evidence_digest = Some([0xE1_u8.saturating_add(index); 32]);
        let quarantine_id = handle
            .record_moderation_screening_result(screening)
            .expect("record screening at boundary")
            .quarantine
            .expect("quarantine record")
            .quarantine_id;
        handle
            .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
                quarantine_id,
                payload,
                captured_at_unix: 1_800_000_100 + u64::from(index),
                content_type: None,
                notes: None,
            })
            .expect("store object at boundary");
        let input = moderation_evidence_viewer_session_input(
            quarantine_id,
            1_800_000_100_000 + u64::from(index) * 1_000,
            1_800_000_200_000 + u64::from(index) * 1_000,
        );
        let session = handle
            .create_moderation_evidence_viewer_session(input.clone())
            .expect("create session at boundary");
        let mut access = moderation_evidence_viewer_access_input(
            session.session_id,
            ModerationEvidenceViewerAccessKind::Viewed,
            input.issued_at_unix_ms + 1,
        );
        access.request_digest = [0xD7_u8.saturating_add(index); 32];
        handle
            .record_moderation_evidence_viewer_access(access)
            .expect("record access at boundary");
        sessions.push(session);
        session_inputs.push(input);
    }
    assert_eq!(
        handle
            .create_moderation_evidence_viewer_session(session_inputs[0].clone())
            .expect("replay session at capacity"),
        sessions[0]
    );
    let mut third_session = session_inputs[0].clone();
    third_session.session_nonce_digest = [0xF1; 32];
    assert!(matches!(
        handle
            .create_moderation_evidence_viewer_session(third_session)
            .expect_err("new session above capacity must fail"),
        ModerationEvidenceViewerError::ResourceExhausted {
            resource: "evidence_viewer_sessions",
            limit: 2
        }
    ));
    assert!(matches!(
        handle
            .record_moderation_evidence_viewer_access(moderation_evidence_viewer_access_input(
                sessions[0].session_id,
                ModerationEvidenceViewerAccessKind::Seeked,
                session_inputs[0].issued_at_unix_ms + 2,
            ),)
            .expect_err("new access event above capacity must fail"),
        ModerationEvidenceViewerError::ResourceExhausted {
            resource: "evidence_viewer_access_events",
            limit: 2
        }
    ));
    let objects = handle
        .export_moderation_quarantine_object_snapshot()
        .expect("export moderation quarantine object snapshot");
    let viewer = handle
        .export_moderation_evidence_viewer_snapshot()
        .expect("export moderation evidence viewer snapshot");
    assert_eq!(objects.objects.len(), 2);
    assert_eq!(viewer.sessions.len(), 2);
    assert_eq!(viewer.access_events.len(), 2);
    drop(handle);
    let restored = node_with_test_quarantine_key_wrapper(cfg);
    let screening = restored
        .export_moderation_screening_snapshot()
        .expect("export restored moderation screening snapshot");
    let objects = restored
        .export_moderation_quarantine_object_snapshot()
        .expect("export restored moderation quarantine object snapshot");
    let viewer = restored
        .export_moderation_evidence_viewer_snapshot()
        .expect("export restored moderation evidence viewer snapshot");
    assert_eq!(screening.screening_records.len(), 2);
    assert_eq!(objects.objects.len(), 2);
    assert_eq!(viewer.sessions.len(), 2);
    assert_eq!(viewer.access_events.len(), 2);
    assert_eq!(
        restored
            .create_moderation_evidence_viewer_session(session_inputs[0].clone())
            .expect("replay restored session at capacity"),
        sessions[0]
    );
}
#[test]
fn publish_reputation_snapshot_updates_cache_and_governance_publisher() {
    let (cfg, _dir) = storage_config_with_reputation_policy();
    let handle = NodeHandle::new(cfg);
    let publisher = Arc::new(RecordingPublisher::default());
    let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
    handle.set_governance_publisher(trait_publisher);
    let envelope = signed_reputation_snapshot_fixture();
    let snapshot = envelope.snapshot.clone();
    let expected = envelope
        .canonical_bytes()
        .expect("encode signed reputation snapshot");
    let mut event_receiver = handle.subscribe_reputation_events();
    handle
        .publish_signed_reputation_snapshot(envelope.clone())
        .expect("publish signed reputation snapshot");
    let published = publisher.take();
    assert_eq!(published, vec![expected]);
    let cached = handle
        .latest_reputation_snapshot()
        .expect("latest reputation snapshot");
    assert_eq!(cached.snapshot_id, snapshot.snapshot_id);
    assert_eq!(cached.merkle_root, snapshot.merkle_root);
    let historical = handle
        .reputation_snapshot(snapshot.snapshot_id)
        .expect("historical reputation snapshot");
    assert_eq!(historical.snapshot_id, snapshot.snapshot_id);
    assert_eq!(historical.merkle_root, snapshot.merkle_root);
    assert_eq!(
        handle.latest_signed_reputation_snapshot(),
        Some(envelope.clone())
    );
    assert_eq!(
        handle.signed_reputation_snapshot(snapshot.snapshot_id),
        Some(envelope)
    );
    let events = handle.reputation_events_since(None, 10);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].sequence, 1);
    assert_eq!(events[0].snapshot_id, snapshot.snapshot_id);
    assert_eq!(events[0].merkle_root, snapshot.merkle_root);
    assert_eq!(handle.latest_reputation_event_sequence(), Some(1));
    let live_event = event_receiver
        .try_recv()
        .expect("live reputation event broadcast");
    assert_eq!(live_event.sequence, 1);
    assert_eq!(live_event.snapshot_id, snapshot.snapshot_id);
    assert!(handle.reputation_events_since(Some(1), 10).is_empty());
}
#[test]
fn signed_reputation_admission_fails_closed_without_policy_and_on_adversarial_envelopes() {
    let (unconfigured, _dir) = storage_config_with_temp_dir();
    let unconfigured_handle = NodeHandle::new(unconfigured);
    let error = unconfigured_handle
        .publish_signed_reputation_snapshot(signed_reputation_snapshot_fixture())
        .expect_err("missing external policy must fail closed");
    assert!(error.to_string().contains("no external trust policy"));
    assert!(
        unconfigured_handle
            .latest_signed_reputation_snapshot()
            .is_none()
    );
    let (configured, _dir) = storage_config_with_reputation_policy();
    let handle = NodeHandle::new(configured);
    let now = unix_now_secs();
    let mut adversarial = Vec::new();
    let mut bad_signature = signed_reputation_snapshot_fixture_with([0x51; 16], now, None);
    bad_signature.signatures[0].signature[0] ^= 0x80;
    adversarial.push(bad_signature);
    let mut wrong_policy = signed_reputation_snapshot_fixture_with([0x52; 16], now, None);
    wrong_policy.policy_digest[0] ^= 0x40;
    adversarial.push(wrong_policy);
    adversarial.push(signed_reputation_snapshot_fixture_with(
        [0x53; 16],
        now.saturating_sub(601),
        None,
    ));
    adversarial.push(signed_reputation_snapshot_fixture_with(
        [0x54; 16],
        now.saturating_add(60),
        None,
    ));
    let mut tampered_evidence = signed_reputation_snapshot_fixture_with([0x55; 16], now, None);
    tampered_evidence.scoring_evidence.provider_inputs[0]
        .metrics
        .por_success_bps -= 1;
    adversarial.push(tampered_evidence);
    let mut untrusted_signer = signed_reputation_snapshot_fixture_with([0x56; 16], now, None);
    untrusted_signer.signatures[0].signer_id = "attacker".to_owned();
    adversarial.push(untrusted_signer);
    let mut no_quorum = signed_reputation_snapshot_fixture_with([0x57; 16], now, None);
    no_quorum.signatures.clear();
    adversarial.push(no_quorum);
    for envelope in adversarial {
        handle
            .publish_signed_reputation_snapshot(envelope)
            .expect_err("adversarial signed envelope must be rejected");
        assert!(handle.latest_reputation_snapshot().is_none());
        assert_eq!(handle.pending_governance_publication_count(), 0);
    }
}
#[test]
fn reputation_trust_policy_loading_rejects_missing_noncanonical_and_unsafe_files() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let missing = root.join("missing-policy.to");
    let missing_config = StorageConfig::builder()
        .enabled(false)
        .reputation_trust_policy_path(Some(missing))
        .build();
    assert_node_init_variant!(ReputationTrustPolicy => NodeHandle::try_new(missing_config));
    let malformed = root.join("malformed-policy.to");
    write_local_checkpoint_atomic(&malformed, b"not canonical Norito")
        .expect("write malformed policy");
    let malformed_config = StorageConfig::builder()
        .enabled(false)
        .reputation_trust_policy_path(Some(malformed))
        .build();
    assert_node_init_variant!(ReputationTrustPolicy => NodeHandle::try_new(malformed_config));
    let oversized = root.join("oversized-policy.to");
    let oversized_bytes = vec![0_u8; MAX_REPUTATION_TRUST_POLICY_ENCODED_BYTES + 1];
    write_local_checkpoint_atomic(&oversized, &oversized_bytes).expect("write oversized policy");
    let oversized_config = StorageConfig::builder()
        .enabled(false)
        .reputation_trust_policy_path(Some(oversized))
        .build();
    assert_node_init_variant!(ReputationTrustPolicy => NodeHandle::try_new(oversized_config));
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let target = root.join("valid-policy.to");
        write_local_checkpoint_atomic(
            &target,
            &reputation_trust_policy_fixture()
                .canonical_bytes()
                .expect("encode valid policy"),
        )
        .expect("write valid policy");
        let symlink_path = root.join("policy-symlink.to");
        symlink(&target, &symlink_path).expect("create policy symlink");
        let symlink_config = StorageConfig::builder()
            .enabled(false)
            .reputation_trust_policy_path(Some(symlink_path))
            .build();
        assert_node_init_variant!(ReputationTrustPolicy =>
            NodeHandle::try_new(symlink_config)
        );
        let writable_path = root.join("writable-policy.to");
        write_local_checkpoint_atomic(
            &writable_path,
            &reputation_trust_policy_fixture()
                .canonical_bytes()
                .expect("encode writable policy"),
        )
        .expect("write policy before permission tamper");
        fs::set_permissions(&writable_path, fs::Permissions::from_mode(0o666))
            .expect("make policy writable by other users");
        let writable_config = StorageConfig::builder()
            .enabled(false)
            .reputation_trust_policy_path(Some(writable_path))
            .build();
        assert_node_init_variant!(ReputationTrustPolicy =>
            NodeHandle::try_new(writable_config)
        );
        let hardlink_path = root.join("policy-hardlink.to");
        fs::hard_link(&target, &hardlink_path).expect("create policy hard link");
        let hardlink_config = StorageConfig::builder()
            .enabled(false)
            .reputation_trust_policy_path(Some(hardlink_path))
            .build();
        assert_node_init_variant!(ReputationTrustPolicy =>
            NodeHandle::try_new(hardlink_config)
        );
    }
}
#[test]
fn reputation_snapshot_rejects_conflicting_ids_and_evicts_only_unreferenced_history() {
    let (base, _dir) = storage_config_with_reputation_policy();
    let cfg = enabled_storage_builder(base.data_dir().clone())
        .reputation_trust_policy_path(base.reputation_trust_policy_path().cloned())
        .runtime_retention(RuntimeRetentionPolicy::new(1, 1, 1024 * 1024))
        .build();
    let handle = NodeHandle::new(cfg);
    handle.set_governance_publisher(Arc::new(RecordingPublisher::default()));
    let first = signed_reputation_snapshot_fixture();
    handle
        .publish_signed_reputation_snapshot(first.clone())
        .expect("publish first snapshot");
    let conflicting = signed_reputation_snapshot_fixture_with(
        first.snapshot.snapshot_id,
        first.snapshot.generated_at_unix + 1,
        None,
    );
    let conflict_error = handle
        .publish_signed_reputation_snapshot(conflicting)
        .expect_err("conflicting canonical bytes under one id must fail");
    assert!(conflict_error.to_string().contains("conflicts"));
    let broken_head = signed_reputation_snapshot_fixture_with(
        [0x43; 16],
        first.snapshot.generated_at_unix + 1,
        None,
    );
    let head_error = handle
        .publish_signed_reputation_snapshot(broken_head)
        .expect_err("snapshot must extend current head");
    assert!(head_error.to_string().contains("exact retained head"));
    let next = signed_reputation_snapshot_fixture_with(
        [0x44; 16],
        first.snapshot.generated_at_unix + 1,
        Some(first.snapshot.snapshot_id),
    );
    handle
        .publish_signed_reputation_snapshot(next)
        .expect("unreferenced predecessor can be safely evicted");
    assert_eq!(
        handle
            .latest_reputation_snapshot()
            .map(|snapshot| snapshot.snapshot_id),
        Some([0x44; 16])
    );
    assert!(
        handle
            .reputation_snapshot(first.snapshot.snapshot_id)
            .is_none()
    );
    assert_eq!(handle.reputation_events_since(None, 10).len(), 1);
    assert_eq!(
        handle.reputation_events_since(None, 10)[0].snapshot_id,
        [0x44; 16]
    );
}
#[test]
fn reputation_snapshot_publish_failure_keeps_durable_state_for_exact_retry() {
    let (cfg, _dir) = storage_config_with_reputation_policy();
    let handle = NodeHandle::new(cfg.clone());
    let failing = Arc::new(FailingPublisher::default());
    handle.set_governance_publisher(failing.clone());
    let envelope = signed_reputation_snapshot_fixture();
    let snapshot = envelope.snapshot.clone();
    handle
        .publish_signed_reputation_snapshot(envelope.clone())
        .expect_err("external publisher failure is surfaced");
    assert_eq!(failing.attempts(), 1);
    assert_eq!(handle.pending_governance_publication_count(), 1);
    assert_eq!(
        handle.latest_reputation_snapshot(),
        Some(snapshot.clone()),
        "local commit must survive publication failure"
    );
    assert_eq!(handle.reputation_events_since(None, 10).len(), 1);
    drop(handle);
    let restored = NodeHandle::new(cfg);
    assert_eq!(
        restored.latest_reputation_snapshot(),
        Some(snapshot.clone())
    );
    assert_eq!(restored.reputation_events_since(None, 10).len(), 1);
    assert_eq!(
        restored.latest_signed_reputation_snapshot(),
        Some(envelope.clone())
    );
    assert_eq!(restored.pending_governance_publication_count(), 1);
    let recording = Arc::new(RecordingPublisher::default());
    restored
        .try_set_governance_publisher(recording.clone())
        .expect("publisher registration replays durable pending snapshot");
    assert_eq!(
        recording.take(),
        vec![envelope.canonical_bytes().expect("encode signed snapshot")]
    );
    assert_eq!(restored.pending_governance_publication_count(), 0);
    assert_eq!(restored.reputation_events_since(None, 10).len(), 1);
}
#[test]
fn reputation_restart_rejects_missing_or_changed_external_policy() {
    let (cfg, _dir) = storage_config_with_reputation_policy();
    let envelope = signed_reputation_snapshot_fixture();
    let handle = NodeHandle::new(cfg.clone());
    handle
        .publish_signed_reputation_snapshot(envelope)
        .expect("persist signed reputation envelope");
    drop(handle);
    let no_policy_config = enabled_storage_builder(cfg.data_dir().clone()).build();
    assert_checkpoint_component!("auxiliary runtime" =>
        NodeHandle::try_new(no_policy_config)
    );
    let policy_path = cfg
        .reputation_trust_policy_path()
        .expect("configured reputation policy");
    let mut changed_policy = reputation_trust_policy_fixture();
    changed_policy.policy_id[0] ^= 0x01;
    write_local_checkpoint_atomic(
        policy_path,
        &changed_policy
            .canonical_bytes()
            .expect("encode changed policy"),
    )
    .expect("replace reputation policy");
    assert_checkpoint_component!("auxiliary runtime" => NodeHandle::try_new(cfg));
}
#[test]
fn reputation_restart_reuses_original_admission_time_for_freshness() {
    let mut short_policy = reputation_trust_policy_fixture();
    short_policy.max_snapshot_age_secs = 1;
    let (cfg, _dir) = storage_config_with_reputation_policy_fixture(&short_policy);
    let handle = NodeHandle::new(cfg.clone());
    let envelope = signed_reputation_snapshot_fixture_for_policy(
        &short_policy,
        [0x61; 16],
        unix_now_secs(),
        None,
    );
    handle
        .publish_signed_reputation_snapshot(envelope.clone())
        .expect("admit fresh signed envelope");
    drop(handle);
    std::thread::sleep(Duration::from_secs(2));
    let restored =
        NodeHandle::try_new(cfg).expect("restart replays the persisted original admission time");
    assert_eq!(restored.latest_signed_reputation_snapshot(), Some(envelope));
}
#[test]
fn governance_outbox_survives_restart_without_a_publisher_and_replays_on_registration() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let issuance = proof_token_issuance_fixture();
    let expected = to_bytes(&issuance).expect("encode proof-token issuance");
    let handle = NodeHandle::new(cfg.clone());
    handle
        .publish_proof_token_issuance(issuance)
        .expect("durably queue without a publisher");
    assert_eq!(handle.pending_governance_publication_count(), 1);
    drop(handle);
    let restored = NodeHandle::new(cfg.clone());
    assert_eq!(restored.pending_governance_publication_count(), 1);
    let recording = Arc::new(RecordingPublisher::default());
    restored
        .try_set_governance_publisher(recording.clone())
        .expect("replay pending issuance");
    assert_eq!(recording.take(), vec![expected]);
    assert_eq!(restored.pending_governance_publication_count(), 0);
    drop(restored);
    let acknowledged = NodeHandle::new(cfg);
    assert_eq!(acknowledged.pending_governance_publication_count(), 0);
}
#[test]
fn governance_outbox_deduplicates_pending_payloads_and_fails_closed_at_retention_limit() {
    let (base, _dir) = storage_config_with_temp_dir();
    let cfg = enabled_storage_builder(base.data_dir().clone())
        .runtime_retention(RuntimeRetentionPolicy::new(1, 1, 1024 * 1024))
        .build();
    let handle = NodeHandle::new(cfg);
    let first = proof_token_issuance_fixture();
    handle
        .publish_proof_token_issuance(first.clone())
        .expect("queue first issuance");
    handle
        .publish_proof_token_issuance(first)
        .expect("exact pending retry is idempotent");
    assert_eq!(handle.pending_governance_publication_count(), 1);
    let mut second = proof_token_issuance_fixture();
    second.token_id = [0x71; 16];
    second.token_blake3 = [0x72; 32];
    let err = handle
        .publish_proof_token_issuance(second)
        .expect_err("outbox retention exhaustion must fail closed");
    assert!(err.to_string().contains("retention exhausted"));
    assert_eq!(handle.pending_governance_publication_count(), 1);
}
#[test]
fn governance_outbox_replays_at_least_once_after_publish_before_ack_crash() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let issuance = proof_token_issuance_fixture();
    let expected = to_bytes(&issuance).expect("encode proof-token issuance");
    let handle = NodeHandle::new(cfg.clone());
    handle
        .publish_proof_token_issuance(issuance)
        .expect("queue issuance");
    let checkpoint_path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let before_ack = fs::read(&checkpoint_path).expect("read pending checkpoint");
    let recording = Arc::new(RecordingPublisher::default());
    handle
        .try_set_governance_publisher(recording.clone())
        .expect("publish and acknowledge issuance");
    assert_eq!(handle.pending_governance_publication_count(), 0);
    write_local_checkpoint_atomic(&checkpoint_path, &before_ack)
        .expect("simulate crash before acknowledgement became durable");
    drop(handle);
    let restored = NodeHandle::new(cfg);
    restored
        .try_set_governance_publisher(recording.clone())
        .expect("at-least-once replay succeeds");
    assert_eq!(recording.take(), vec![expected.clone(), expected]);
    assert_eq!(restored.pending_governance_publication_count(), 0);
}
#[test]
fn governance_outbox_checkpoint_rejects_digest_kind_and_sequence_tampering() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let handle = NodeHandle::new(cfg.clone());
    handle
        .publish_proof_token_issuance(proof_token_issuance_fixture())
        .expect("queue issuance");
    drop(handle);
    let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let original = fs::read(&path).expect("read auxiliary checkpoint");
    for tamper in 0..3 {
        let mut checkpoint: AuxiliaryRuntimeCheckpointV5 =
            norito::decode_from_bytes(&original).expect("decode auxiliary checkpoint");
        let entry = checkpoint
            .governance_outbox_entries
            .first_mut()
            .expect("pending outbox entry");
        match tamper {
            0 => entry.payload_digest[0] ^= 0x80,
            1 => entry.kind = GovernanceOutboxKindV1::DealSettlement,
            2 => entry.sequence = checkpoint.governance_outbox_next_sequence,
            _ => unreachable!(),
        }
        write_local_checkpoint_atomic(
            &path,
            &norito::to_bytes(&checkpoint).expect("encode tampered checkpoint"),
        )
        .expect("write tampered checkpoint");
        assert_checkpoint_component!("auxiliary runtime" =>
            NodeHandle::try_new(cfg.clone())
        );
    }
    write_local_checkpoint_atomic(&path, &original).expect("restore original checkpoint");
    assert!(NodeHandle::try_new(cfg).is_ok());
}
#[test]
fn governance_outbox_checkpoint_rejects_semantically_tampered_audit_header() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let handle = NodeHandle::new(cfg.clone());
    handle
        .publish_gc_audit_event(GcAuditPayloadV1 {
            version: GC_AUDIT_PAYLOAD_VERSION_V1,
            manifest_digest: [0x81; 32],
            provider_id: [0; 32],
            evicted_at_unix: 1_800_000_000,
            freed_bytes: 0,
            reason: GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1.to_owned(),
            blocked_reason: Some(GC_AUDIT_BLOCKED_REPAIR_ACTIVE_V1.to_owned()),
        })
        .expect("queue blocked GC audit");
    assert_eq!(handle.pending_governance_publication_count(), 1);
    drop(handle);
    let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let bytes = fs::read(&path).expect("read auxiliary checkpoint");
    let mut checkpoint: AuxiliaryRuntimeCheckpointV5 =
        norito::decode_from_bytes(&bytes).expect("decode auxiliary checkpoint");
    let entry = checkpoint
        .governance_outbox_entries
        .first_mut()
        .expect("pending audit entry");
    let mut audit: GcAuditEventV1 =
        norito::decode_from_bytes(&entry.payload_bytes).expect("decode GC audit event");
    audit.header.signer = "attacker".to_owned();
    entry.payload_bytes = norito::to_bytes(&audit).expect("encode tampered audit event");
    entry.payload_digest = *blake3::hash(&entry.payload_bytes).as_bytes();
    entry.binding_digest = governance_outbox_binding_digest(
        entry.version,
        entry.sequence,
        entry.kind,
        entry.payload_digest,
        entry.provenance.as_ref(),
    );
    write_local_checkpoint_atomic(
        &path,
        &norito::to_bytes(&checkpoint).expect("encode tampered checkpoint"),
    )
    .expect("write tampered checkpoint");
    assert_checkpoint_component!("auxiliary runtime" => NodeHandle::try_new(cfg));
}
#[test]
fn reputation_checkpoint_rejects_event_snapshot_metadata_tampering() {
    let (cfg, _dir) = storage_config_with_reputation_policy();
    let handle = NodeHandle::new(cfg.clone());
    handle
        .publish_signed_reputation_snapshot(signed_reputation_snapshot_fixture())
        .expect("publish signed snapshot");
    drop(handle);
    let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let bytes = fs::read(&path).expect("read auxiliary checkpoint");
    let mut checkpoint: AuxiliaryRuntimeCheckpointV5 =
        norito::decode_from_bytes(&bytes).expect("decode auxiliary checkpoint");
    checkpoint.reputation_events[0].snapshot_id = [0x99; 16];
    write_local_checkpoint_atomic(
        &path,
        &norito::to_bytes(&checkpoint).expect("encode tampered checkpoint"),
    )
    .expect("write tampered checkpoint");
    assert_checkpoint_component!("auxiliary runtime" => NodeHandle::try_new(cfg));
}
#[test]
fn reputation_checkpoint_rejects_envelope_admission_and_outbox_tampering() {
    let (cfg, _dir) = storage_config_with_reputation_policy();
    let handle = NodeHandle::new(cfg.clone());
    handle
        .publish_signed_reputation_snapshot(signed_reputation_snapshot_fixture())
        .expect("persist signed reputation envelope");
    drop(handle);
    let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let original = fs::read(&path).expect("read auxiliary checkpoint");
    for case in 0..7_u8 {
        let mut checkpoint: AuxiliaryRuntimeCheckpointV5 =
            norito::decode_from_bytes(&original).expect("decode auxiliary checkpoint");
        match case {
            0 => {
                checkpoint.reputation_snapshots[0].version ^= 1;
            }
            1 => checkpoint.reputation_snapshots[0].admitted_at_unix = 0,
            2 => {
                checkpoint.reputation_snapshots[0].envelope.signatures[0].signature[0] ^= 0x80;
            }
            3 => checkpoint.reputation_snapshots[0].envelope.policy_digest[0] ^= 0x40,
            4 => {
                checkpoint.reputation_snapshots[0]
                    .envelope
                    .scoring_evidence_digest[0] ^= 0x20;
            }
            5 => checkpoint.reputation_snapshots[0].encoded_len ^= 1,
            6 => {
                let replacement =
                    signed_reputation_snapshot_fixture_with([0x7A; 16], unix_now_secs(), None);
                let entry = checkpoint
                    .governance_outbox_entries
                    .first_mut()
                    .expect("pending reputation outbox entry");
                entry.payload_bytes = replacement
                    .canonical_bytes()
                    .expect("encode replacement signed envelope");
                entry.payload_digest = *blake3::hash(&entry.payload_bytes).as_bytes();
                entry.binding_digest = governance_outbox_binding_digest(
                    entry.version,
                    entry.sequence,
                    entry.kind,
                    entry.payload_digest,
                    entry.provenance.as_ref(),
                );
            }
            _ => unreachable!("bounded checkpoint tamper case"),
        }
        write_local_checkpoint_atomic(
            &path,
            &norito::to_bytes(&checkpoint).expect("encode tampered checkpoint"),
        )
        .expect("write tampered checkpoint");
        assert_checkpoint_component!("auxiliary runtime" =>
            NodeHandle::try_new(cfg.clone())
        );
    }
}
#[test]
fn publish_appeal_finance_report_writes_governance_publisher() {
    let (handle, publisher, _dir) = node_with_temp_storage_and_recording_publisher();
    let report = appeal_finance_report_fixture();
    let expected = to_bytes(&report).expect("encode appeal finance report");
    handle
        .publish_authenticated_appeal_finance_report(
            report.clone(),
            governance_submission_account(0xB3),
        )
        .expect("publish appeal finance report");
    let published = publisher.take();
    assert_eq!(published, vec![expected]);
    assert_eq!(handle.transparency_ledger_source_entry_count(), 0);
}
#[test]
fn authenticated_governance_outbox_binds_publisher_and_rejects_reattribution() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let handle = NodeHandle::new(cfg.clone());
    let publisher_key = KeyPair::try_from_seed(vec![0xB4; 32], Algorithm::Ed25519)
        .expect("derive finance publisher key");
    let publisher = AccountId::new(publisher_key.public_key().clone());
    let other_key = KeyPair::try_from_seed(vec![0xB5; 32], Algorithm::Ed25519)
        .expect("derive alternate finance publisher key");
    let other = AccountId::new(other_key.public_key().clone());
    let report = appeal_finance_report_fixture();
    handle
        .publish_authenticated_appeal_finance_report(report.clone(), publisher.clone())
        .expect("enqueue authenticated finance report");
    handle
        .publish_authenticated_appeal_finance_report(report.clone(), publisher.clone())
        .expect("same publisher retry is idempotent");
    assert_eq!(handle.pending_governance_publication_count(), 1);
    let error = handle
        .publish_authenticated_appeal_finance_report(report, other.clone())
        .expect_err("another publisher cannot claim the retained canonical payload");
    assert!(
        error
            .to_string()
            .contains("conflicts with retained authenticated provenance")
    );
    assert_eq!(handle.pending_governance_publication_count(), 1);
    assert_eq!(handle.transparency_ledger_source_entry_count(), 0);
    let checkpoint_path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let checkpoint_bytes = fs::read(&checkpoint_path).expect("read governance checkpoint");
    let mut checkpoint: AuxiliaryRuntimeCheckpointV5 =
        norito::decode_from_bytes(&checkpoint_bytes).expect("decode governance checkpoint");
    let entry = checkpoint
        .governance_outbox_entries
        .first_mut()
        .expect("authenticated governance outbox entry");
    let provenance = entry
        .provenance
        .as_mut()
        .expect("governance entry retains authenticated provenance");
    assert_eq!(provenance.publisher_account(), &publisher);
    assert_eq!(
        provenance.origin(),
        GovernanceSubmissionOriginV1::AppealFinanceReport
    );
    provenance.publisher_account = other;
    drop(handle);
    write_local_checkpoint_atomic(
        &checkpoint_path,
        &norito::to_bytes(&checkpoint).expect("encode tampered governance checkpoint"),
    )
    .expect("write tampered governance checkpoint");
    assert_checkpoint_component!("auxiliary runtime" => NodeHandle::try_new(cfg));
}
#[test]
fn publish_transparency_ledger_publication_writes_governance_publisher() {
    let (handle, publisher, _dir) = node_with_temp_storage_and_recording_publisher();
    let publication = transparency_ledger_publication_fixture();
    let expected = to_bytes(&publication).expect("encode transparency ledger publication");
    handle
        .publish_transparency_ledger_publication(publication.clone())
        .expect("publish transparency ledger publication");
    let published = publisher.take();
    assert_eq!(published, vec![expected]);
    let decoded: ModerationLedgerCyclePublicationV1 =
        norito::decode_from_bytes(&published[0]).expect("decode transparency ledger publication");
    assert_eq!(decoded.block.entry_count, 2);
    decoded.validate().expect("publication validates");
}
#[test]
fn direct_privacy_publication_rejects_unfenced_outbox_mutation() {
    let (handle, publisher, _dir) = node_with_temp_storage_and_recording_publisher();
    let publication = NodeHandle::build_privacy_aggregate_publication(
        *b"cycle-2026-wk-03",
        1_800_000_000,
        1_800_604_800,
        1_800_604_800,
        None,
        vec![privacy_aggregate_fixture("sfm4c-jurisdiction-a", 0xA0)],
    )
    .expect("build privacy aggregate publication");
    let error = handle
        .publish_transparency_ledger_publication(publication)
        .expect_err("direct privacy publication must not bypass leader fencing");
    assert!(
        error
            .to_string()
            .contains("must use the finalized release scheduler")
    );
    assert_eq!(handle.pending_governance_publication_count(), 0);
    assert!(publisher.take().is_empty());
}
#[test]
fn publish_proof_token_issuance_writes_governance_publisher() {
    let (handle, publisher, _dir) = node_with_temp_storage_and_recording_publisher();
    let issuance = proof_token_issuance_fixture();
    let expected = to_bytes(&issuance).expect("encode proof-token issuance");
    handle
        .publish_proof_token_issuance(issuance.clone())
        .expect("publish proof-token issuance");
    let published = publisher.take();
    assert_eq!(published, vec![expected]);
    let decoded: ProofTokenIssuanceV1 =
        norito::decode_from_bytes(&published[0]).expect("decode proof-token issuance");
    assert_eq!(decoded, issuance);
    decoded.validate().expect("issuance validates");
}
#[test]
fn publish_proof_token_base64_issuance_derives_and_writes_governance_publisher() {
    let (handle, publisher, _dir) = node_with_temp_storage_and_recording_publisher();
    let issuance = handle
        .publish_proof_token_base64_issuance(
            VALID_PROOF_TOKEN_B64,
            proof_token_signer_key_fixture(),
            Some([0x65; 32]),
            Some([0x66; 32]),
            vec![
                iroha_data_model::sorafs::transparency::ModerationLedgerMetadataV1 {
                    key: "issuer".to_string(),
                    value: "gateway-a".to_string(),
                },
            ],
        )
        .expect("publish proof-token issuance from base64");
    assert_eq!(issuance.token_id, [0x61; 16]);
    assert_eq!(issuance.issued_at_unix, 1_800_000_030);
    assert_eq!(issuance.expires_at_unix, Some(1_800_086_430));
    assert_eq!(issuance.moderation_action_code, 2);
    assert_eq!(issuance.signer_key, proof_token_signer_key_fixture());
    assert_eq!(issuance.blinded_digest, [0x64; 32]);
    assert_eq!(
        issuance.entry_ids,
        vec!["denylist/global".to_string(), "gar/policy/42".to_string()]
    );
    let published = publisher.take();
    assert_eq!(published.len(), 1);
    let decoded: ProofTokenIssuanceV1 =
        norito::decode_from_bytes(&published[0]).expect("decode proof-token issuance");
    assert_eq!(decoded, issuance);
}
#[test]
fn record_transparency_ledger_source_entry_is_idempotent_and_rejects_conflicts() {
    use iroha_data_model::sorafs::transparency::ModerationLedgerEntryKindV1;
    let (handle, _dir) = node_with_temp_storage();
    let entry = transparency_ledger_source_entry(
        "gar-1",
        1_800_000_010,
        ModerationLedgerEntryKindV1::GarEnforcementReceipt,
        "gar-receipt-1",
        0x50,
    );
    handle
        .record_transparency_ledger_source_entry(entry.clone())
        .expect("record source entry");
    handle
        .record_transparency_ledger_source_entry(entry.clone())
        .expect("exact duplicate is idempotent");
    let mut conflicting = entry;
    conflicting.payload_digest = [0xA5; 32];
    let err = handle
        .record_transparency_ledger_source_entry(conflicting)
        .expect_err("conflicting source entry rejected");
    assert!(
        err.to_string()
            .contains("conflicts with retained canonical data")
    );
    assert_eq!(handle.transparency_ledger_source_entry_count(), 1);
}
#[test]
fn publish_transparency_ledger_source_entries_builds_and_publishes_publication() {
    use iroha_data_model::sorafs::transparency::ModerationLedgerEntryKindV1;
    let (handle, publisher, _dir) = node_with_temp_storage_and_recording_publisher();
    for entry in [
        transparency_ledger_source_entry(
            "redaction-1",
            1_800_000_030,
            ModerationLedgerEntryKindV1::Redaction,
            "redaction-case-1",
            0x70,
        ),
        transparency_ledger_source_entry(
            "gar-1",
            1_800_000_010,
            ModerationLedgerEntryKindV1::GarEnforcementReceipt,
            "gar-receipt-1",
            0x50,
        ),
        transparency_ledger_source_entry(
            "hold-1",
            1_800_000_030,
            ModerationLedgerEntryKindV1::LegalHold,
            "hold-case-1",
            0x60,
        ),
        transparency_ledger_source_entry(
            "appeal-1",
            1_800_000_005,
            ModerationLedgerEntryKindV1::AppealOutcome,
            "appeal-case-1",
            0x40,
        ),
        transparency_ledger_source_entry(
            "future-1",
            1_800_604_900,
            ModerationLedgerEntryKindV1::EvidenceAccess,
            "evidence-view-1",
            0x80,
        ),
    ] {
        handle
            .record_transparency_ledger_source_entry(entry)
            .expect("record source entry");
    }
    let publication = handle
        .publish_transparency_ledger_cycle_from_source_entries(
            *b"cycle-src-pub001",
            1_800_000_000,
            1_800_604_800,
            1_800_604_801,
            Some([0x44; 32]),
        )
        .expect("publish transparency source cycle");
    publication.validate().expect("publication validates");
    assert_eq!(publication.block.entry_count, 4);
    assert_eq!(publication.block.previous_block_hash, Some([0x44; 32]));
    let subjects = publication
        .proofs
        .iter()
        .map(|proof| proof.entry.subject.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        subjects,
        vec![
            "appeal-case-1",
            "gar-receipt-1",
            "hold-case-1",
            "redaction-case-1"
        ]
    );
    for (index, proof) in publication.proofs.iter().enumerate() {
        assert_eq!(proof.entry.sequence, u64::try_from(index).unwrap() + 1);
        assert_eq!(proof.entry.cycle_id, publication.block.cycle_id);
        assert_ne!(proof.entry.entry_id, [0; 16]);
    }
    let published = publisher.take();
    assert_eq!(published.len(), 1);
    let decoded: ModerationLedgerCyclePublicationV1 =
        norito::decode_from_bytes(&published[0]).expect("decode transparency source publication");
    assert_eq!(decoded, publication);
}
#[test]
fn publish_transparency_ledger_source_entries_rejects_empty_window() {
    let (handle, publisher, _dir) = node_with_temp_storage_and_recording_publisher();
    let err = handle
        .publish_transparency_ledger_cycle_from_source_entries(
            *b"cycle-src-pub001",
            1_800_000_000,
            1_800_604_800,
            1_800_604_801,
            None,
        )
        .expect_err("empty source window rejected");
    assert!(err.to_string().contains("no source entries"));
    assert!(publisher.take().is_empty());
}
#[test]
fn record_concrete_transparency_source_entries_builds_publication() {
    use iroha_data_model::sorafs::{
        gar::GarEnforcementActionV1, transparency::ModerationLedgerEntryKindV1,
    };
    let (handle, _dir) = node_with_temp_storage();
    handle
        .record_gar_enforcement_receipt_transparency_entry(&gar_enforcement_receipt_fixture(
            GarEnforcementActionV1::LegalHold,
        ))
        .expect("record GAR receipt source entry");
    let moderation_event = SoraFsModerationBallotGovernanceEventV1 {
        version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
        sequence: 7,
        kind: SoraFsModerationBallotGovernanceEventKindV1::BallotTallied,
        generated_at_unix_ms: 1_800_000_020_000,
        case_id: "case-42".to_string(),
        round_id: "round-1".to_string(),
        juror_id: None,
        committed_count: 3,
        revealed_count: 3,
        challenge_count: 0,
        tally: Some(SoraFsModerationBallotGovernanceTallyV1 {
            case_id: "case-42".to_string(),
            round_id: "round-1".to_string(),
            counts: SoraFsModerationVoteCountsV1 {
                uphold: 1,
                overturn: 2,
                modify: 0,
                escalate: 0,
            },
            votes_total: 3,
            quorum: 2,
            winning_choice: Some(SoraFsModerationVoteChoiceV1::Overturn),
            contested: false,
            tallied_at_unix_ms: 1_800_000_020_000,
        }),
        challenge: None,
    };
    handle
        .record_moderation_ballot_governance_transparency_entry(&moderation_event)
        .expect("record moderation governance source entry");
    let report = appeal_finance_report_fixture();
    handle
        .record_appeal_finance_report_transparency_entry(&report)
        .expect("record appeal report source entry");
    let receipt = appeal_finance_settlement_receipt_fixture();
    handle
        .record_appeal_finance_settlement_receipt_transparency_entry(&receipt)
        .expect("record appeal settlement source entry");
    assert_eq!(handle.transparency_ledger_source_entry_count(), 4);
    let publication = handle
        .publish_transparency_ledger_cycle_from_source_entries(
            *b"cycle-src-pub002",
            1_800_000_000,
            1_800_604_800,
            1_800_604_801,
            None,
        )
        .expect("publish concrete source cycle");
    publication.validate().expect("publication validates");
    assert_eq!(publication.block.entry_count, 4);
    let kinds = publication
        .proofs
        .iter()
        .map(|proof| proof.entry.kind.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            ModerationLedgerEntryKindV1::LegalHold,
            ModerationLedgerEntryKindV1::ModerationAction,
            ModerationLedgerEntryKindV1::AppealOutcome,
            ModerationLedgerEntryKindV1::AppealOutcome,
        ]
    );
    assert!(
        publication
            .proofs
            .iter()
            .any(|proof| proof.entry.subject == "docs.sora@docs.gateway.sora.net")
    );
    assert!(
        publication
            .proofs
            .iter()
            .any(|proof| proof.entry.subject == "case-42:drawdown_non_refund")
    );
}
#[test]
fn publish_privacy_aggregate_cycle_builds_and_publishes_publication() {
    use iroha_data_model::sorafs::transparency::ModerationLedgerEntryKindV1;
    let (handle, publisher, _dir) = node_with_temp_storage_and_recording_publisher();
    let cycle_id = *b"cycle-2026-wk-03";
    let aggregate_b = privacy_aggregate_fixture("sfm4c-jurisdiction-b", 0xB0);
    let aggregate_a = privacy_aggregate_fixture("sfm4c-jurisdiction-a", 0xA0);
    let publication = handle
        .publish_privacy_aggregate_cycle(
            cycle_id,
            1_800_000_000,
            1_800_604_800,
            1_800_604_801,
            None,
            vec![aggregate_b, aggregate_a],
        )
        .expect("publish privacy aggregate cycle");
    publication.validate().expect("publication validates");
    assert_eq!(publication.block.entry_count, 2);
    let subjects = publication
        .proofs
        .iter()
        .map(|proof| proof.entry.subject.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        subjects,
        vec!["sfm4c-jurisdiction-a", "sfm4c-jurisdiction-b"]
    );
    assert!(
        publication
            .proofs
            .iter()
            .all(|proof| proof.entry.kind == ModerationLedgerEntryKindV1::PrivacyAggregate)
    );
    let published = publisher.take();
    assert_eq!(published.len(), 1);
    let decoded: ModerationLedgerCyclePublicationV1 =
        norito::decode_from_bytes(&published[0]).expect("decode privacy aggregate publication");
    assert_eq!(decoded, publication);
}
#[test]
fn privacy_aggregate_publication_rejects_mixed_cycle_policy_or_randomness() {
    use iroha_data_model::sorafs::transparency::{
        ModerationPrivacyNoiseSourceV1, ModerationPrivacyThresholdPrfCommitmentV1,
    };
    let aggregate_a = privacy_aggregate_fixture("sfm4c-jurisdiction-a", 0xA0);
    let mut aggregate_b = privacy_aggregate_fixture("sfm4c-jurisdiction-b", 0xB0);
    aggregate_b.policy_digest = [0xD0; 32];
    let err = NodeHandle::build_privacy_aggregate_publication(
        *b"cycle-2026-wk-03",
        1_800_000_000,
        1_800_604_800,
        1_800_604_800,
        None,
        vec![aggregate_a.clone(), aggregate_b.clone()],
    )
    .expect_err("mixed policy digests are rejected");
    assert!(err.to_string().contains("mixed policy digests"));
    aggregate_b.policy_digest = aggregate_a.policy_digest;
    aggregate_b.noise_source =
        ModerationPrivacyNoiseSourceV1::ThresholdPrf(ModerationPrivacyThresholdPrfCommitmentV1 {
            commitment: [0xDD; 32],
        });
    let err = NodeHandle::build_privacy_aggregate_publication(
        *b"cycle-2026-wk-03",
        1_800_000_000,
        1_800_604_800,
        1_800_604_800,
        None,
        vec![aggregate_a, aggregate_b],
    )
    .expect_err("mixed privacy randomness commitments are rejected");
    assert!(err.to_string().contains("mixed privacy noise sources"));
}
#[test]
fn publish_privacy_aggregate_cycle_rejects_out_of_window_without_publishing() {
    let (handle, publisher, _dir) = node_with_temp_storage_and_recording_publisher();
    let mut aggregate = privacy_aggregate_fixture("sfm4c-jurisdiction-a", 0xA0);
    aggregate.window_start_unix = 1_799_999_999;
    let err = handle
        .publish_privacy_aggregate_cycle(
            *b"cycle-2026-wk-03",
            1_800_000_000,
            1_800_604_800,
            1_800_604_801,
            None,
            vec![aggregate],
        )
        .expect_err("out-of-window aggregate is rejected");
    assert!(
        err.to_string()
            .contains("window must equal the publication cycle")
    );
    assert!(publisher.take().is_empty());
}
#[test]
fn record_privacy_aggregate_source_event_is_idempotent_and_rejects_equivocation() {
    let (handle, _dir) = node_with_temp_storage();
    let event = privacy_source_event("event-a", "jurisdiction-a", 0xA0, 1_800_000_010);
    assert_eq!(
        handle
            .record_privacy_aggregate_source_event(event.clone())
            .expect("record source event"),
        PrivacySourceEventRecordOutcomeV1::Recorded
    );
    assert_eq!(
        handle
            .record_privacy_aggregate_source_event(event.clone())
            .expect("exact retry is idempotent"),
        PrivacySourceEventRecordOutcomeV1::AlreadyRecorded
    );
    let mut equivocation = event;
    equivocation.occurred_at_unix += 1;
    let err = handle
        .record_privacy_aggregate_source_event(equivocation)
        .expect_err("changed bytes under one event id are rejected");
    assert!(err.to_string().contains("idempotency key equivocation"));
    assert_eq!(handle.privacy_aggregate_source_event_count(), 1);
}
#[test]
fn authenticated_privacy_source_event_binds_durable_publisher_provenance() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let handle = NodeHandle::new(cfg.clone());
    let publisher_key = KeyPair::try_from_seed(vec![0xA4; 32], Algorithm::Ed25519)
        .expect("derive source publisher key");
    let publisher = AccountId::new(publisher_key.public_key().clone());
    let other_key = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519)
        .expect("derive alternate source publisher key");
    let other = AccountId::new(other_key.public_key().clone());
    let event = privacy_source_event(
        "authenticated-event-a",
        "jurisdiction-a",
        0xA0,
        1_800_000_010,
    );
    assert_eq!(
        handle
            .record_authenticated_privacy_aggregate_source_event(event.clone(), publisher.clone(),)
            .expect("record authenticated source event"),
        PrivacySourceEventRecordOutcomeV1::Recorded
    );
    let checkpoint_bytes = fs::read(auxiliary_runtime_checkpoint_path(cfg.data_dir()))
        .expect("read authenticated source checkpoint");
    let checkpoint: AuxiliaryRuntimeCheckpointV5 =
        norito::decode_from_bytes(&checkpoint_bytes).expect("decode source checkpoint");
    let provenance = checkpoint.privacy_source_events[0]
        .provenance
        .as_ref()
        .expect("source checkpoint retains authenticated provenance");
    assert_eq!(provenance.publisher_account(), &publisher);
    assert_eq!(
        provenance.origin(),
        GovernanceSubmissionOriginV1::PrivacyAggregateSourceEvent
    );
    drop(handle);
    let restored = NodeHandle::new(cfg);
    assert_eq!(
        restored
            .record_authenticated_privacy_aggregate_source_event(event.clone(), publisher,)
            .expect("same publisher retry remains idempotent after restart"),
        PrivacySourceEventRecordOutcomeV1::AlreadyRecorded
    );
    let error = restored
        .record_authenticated_privacy_aggregate_source_event(event, other)
        .expect_err("same event id from another publisher must conflict");
    assert!(error.to_string().contains("idempotency key equivocation"));
}
#[test]
fn publish_privacy_aggregate_cycle_from_source_events_suppresses_and_publishes() {
    use iroha_data_model::sorafs::transparency::{
        MODERATION_PRIVACY_RANDOMNESS_COMMITMENT_METADATA_KEY_V1, ModerationLedgerEntryKindV1,
    };
    let (handle, publisher, _dir) = node_with_temp_storage_and_recording_publisher();
    for event in [
        privacy_source_event("alpha-1", "jurisdiction-a", 0xA0, 1_800_000_010),
        privacy_source_event("alpha-2", "jurisdiction-a", 0xA0, 1_800_000_020),
        privacy_source_event("beta-1", "jurisdiction-b", 0xB0, 1_800_000_030),
        privacy_source_event("future-1", "jurisdiction-c", 0xC0, 1_800_604_900),
    ] {
        handle
            .record_privacy_aggregate_source_event(event)
            .expect("record source event");
    }
    let config = privacy_aggregate_cycle_config();
    let cycle_prf_input = privacy_cycle_prf_input(
        &config,
        1_800_000_000,
        1_800_604_800,
        1_800_604_801,
        [0x5A; 32],
    );
    let publication = handle
        .publish_privacy_aggregate_cycle_from_source_events(PrivacyAggregateSourceCycleInput {
            cycle_id: *b"cycle-2026-wk-04",
            cycle_start_unix: 1_800_000_000,
            cycle_end_unix: 1_800_604_800,
            previous_block_hash: None,
            config,
            cycle_prf_input: Some(cycle_prf_input),
        })
        .expect("publish aggregate cycle from source events");
    publication.validate().expect("publication validates");
    assert_eq!(publication.block.entry_count, 2);
    let entry = &publication.proofs[0].entry;
    assert_eq!(entry.kind, ModerationLedgerEntryKindV1::PrivacyAggregate);
    assert!(entry.subject.contains("jurisdiction-a"));
    assert_eq!(entry.evidence_uris.len(), 0);
    assert!(
        entry
            .metadata
            .iter()
            .any(|item| { item.key == MODERATION_PRIVACY_RANDOMNESS_COMMITMENT_METADATA_KEY_V1 })
    );
    assert!(
        entry.metadata.iter().all(|item| !matches!(
            item.key.as_str(),
            "source_event_count" | "source_subject_count" | "suppressed_count"
        )),
        "public ledger metadata must not disclose exact private counts"
    );
    let published = publisher.take();
    assert_eq!(published.len(), 1);
    let decoded: ModerationLedgerCyclePublicationV1 =
        norito::decode_from_bytes(&published[0]).expect("decode aggregate publication");
    assert_eq!(decoded, publication);
}
#[test]
fn publish_privacy_aggregate_cycle_from_source_events_requires_cycle_prf_output() {
    let (handle, publisher, _dir) = node_with_temp_storage_and_recording_publisher();
    handle
        .record_privacy_aggregate_source_event(privacy_source_event(
            "alpha-1",
            "jurisdiction-a",
            0xA0,
            1_800_000_010,
        ))
        .expect("record source event");
    handle
        .record_privacy_aggregate_source_event(privacy_source_event(
            "alpha-2",
            "jurisdiction-a",
            0xA0,
            1_800_000_020,
        ))
        .expect("record source event");
    let err = handle
        .publish_privacy_aggregate_cycle_from_source_events(PrivacyAggregateSourceCycleInput {
            cycle_id: *b"cycle-2026-wk-04",
            cycle_start_unix: 1_800_000_000,
            cycle_end_unix: 1_800_604_800,
            previous_block_hash: None,
            config: privacy_aggregate_cycle_config(),
            cycle_prf_input: None,
        })
        .expect_err("missing cycle PRF output rejected");
    assert!(err.to_string().contains("hidden cycle PRF output"));
    assert!(publisher.take().is_empty());
}
#[test]
fn publish_due_privacy_aggregate_cycle_from_source_events_publishes_once() {
    let (cfg, _dir) = privacy_aggregate_storage_config_with_temp_dir();
    let handle = node_with_test_privacy_cycle_prf_provider(cfg);
    for event in [
        privacy_source_event("alpha-1", "jurisdiction-a", 0xA0, 110),
        privacy_source_event("alpha-2", "jurisdiction-a", 0xA0, 120),
        privacy_source_event("future-1", "jurisdiction-a", 0xA0, 220),
    ] {
        handle
            .record_privacy_aggregate_source_event(event)
            .expect("record source event");
    }
    let outcome = publish_due_test_privacy_cycle(&handle, 211, 100, 200, "publish-once")
        .expect("publish due aggregate cycle");
    let publication = match outcome {
        PrivacyAggregateScheduleOutcome::Published {
            window,
            publication,
        } => {
            assert_eq!(window.cycle_start_unix, 100);
            assert_eq!(window.cycle_end_unix, 200);
            publication
        }
        other => panic!("expected published outcome, got {other:?}"),
    };
    assert_eq!(publication.block.cycle_start_unix, 100);
    assert_eq!(publication.block.cycle_end_unix, 200);
    assert_eq!(publication.block.generated_at_unix, 200);
    assert_eq!(publication.block.entry_count, 2);
    let repeated = publish_due_test_privacy_cycle(&handle, 211, 100, 200, "publish-once")
        .expect("repeat due aggregate cycle");
    assert!(matches!(
        repeated,
        PrivacyAggregateScheduleOutcome::Published { .. }
    ));
    assert_eq!(handle.pending_governance_publication_count(), 0);
    assert_eq!(
        handle
            .privacy_composition_budget_snapshot()
            .expect("privacy budget after exact replay")
            .chains[0]
            .charges
            .len(),
        1
    );
}
#[test]
fn publish_due_privacy_aggregate_cycle_from_source_events_catches_up_stale_windows() {
    let (cfg, _dir) = privacy_aggregate_storage_config_with_temp_dir();
    let handle = node_with_test_privacy_cycle_prf_provider(cfg);
    for event in [
        privacy_source_event("alpha-1", "jurisdiction-a", 0xA0, 110),
        privacy_source_event("alpha-2", "jurisdiction-a", 0xA0, 120),
        privacy_source_event("beta-1", "jurisdiction-b", 0xB0, 210),
        privacy_source_event("beta-2", "jurisdiction-b", 0xB0, 220),
    ] {
        handle
            .record_privacy_aggregate_source_event(event)
            .expect("record source event");
    }
    let first = publish_due_test_privacy_cycle(&handle, 311, 100, 200, "catchup-cycle-1")
        .expect("publish first stale aggregate cycle");
    let first_publication = match first {
        PrivacyAggregateScheduleOutcome::Published {
            window,
            publication,
        } => {
            assert_eq!(window.cycle_start_unix, 100);
            assert_eq!(window.cycle_end_unix, 200);
            assert_eq!(publication.block.cycle_start_unix, 100);
            assert_eq!(publication.block.cycle_end_unix, 200);
            assert_eq!(publication.block.generated_at_unix, 200);
            publication
        }
        other => panic!("expected stale published outcome, got {other:?}"),
    };
    let second = publish_due_test_privacy_cycle(&handle, 311, 200, 300, "catchup-cycle-2")
        .expect("publish latest aggregate cycle after catch-up");
    match second {
        PrivacyAggregateScheduleOutcome::Published {
            window,
            publication,
        } => {
            assert_eq!(window.cycle_start_unix, 200);
            assert_eq!(window.cycle_end_unix, 300);
            assert_eq!(publication.block.cycle_start_unix, 200);
            assert_eq!(publication.block.cycle_end_unix, 300);
            assert_eq!(publication.block.generated_at_unix, 300);
        }
        other => panic!("expected latest published outcome, got {other:?}"),
    }
    let replayed = publish_due_test_privacy_cycle(&handle, 311, 100, 200, "catchup-cycle-1")
        .expect("old exact request replays after the head advances");
    let replayed_publication = match replayed {
        PrivacyAggregateScheduleOutcome::Published { publication, .. } => publication,
        other => panic!("expected replayed publication, got {other:?}"),
    };
    assert_eq!(
        norito::to_bytes(&replayed_publication).expect("encode replayed publication"),
        norito::to_bytes(&first_publication).expect("encode original publication")
    );
    let mut rotated_delay = privacy_aggregate_schedule_config();
    rotated_delay.publish_delay_seconds = rotated_delay.publish_delay_seconds.saturating_add(1);
    let rotated_replay = handle
        .publish_due_privacy_aggregate_cycle_from_source_events(
            311,
            privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
            "catchup-cycle-1".to_string(),
            rotated_delay,
            privacy_aggregate_cycle_config(),
            Some(privacy_composition_budget_policy()),
        )
        .expect_err("old exact request cannot replay under a rotated release cadence");
    assert!(
        rotated_replay
            .to_string()
            .contains("cadence does not match the configured query lineage")
    );
    let stale_fresh = publish_due_test_privacy_cycle(&handle, 411, 100, 200, "catchup-stale-fresh")
        .expect_err("a fresh key cannot target an old terminal release");
    assert!(
        stale_fresh
            .to_string()
            .contains("does not match the direct successor")
    );
    let mismatched_old_key =
        publish_due_test_privacy_cycle(&handle, 411, 200, 300, "catchup-cycle-1")
            .expect_err("an old key cannot be rebound to another cycle");
    assert!(
        mismatched_old_key
            .to_string()
            .contains("idempotency key equivocation")
    );
    assert_eq!(handle.pending_governance_publication_count(), 0);
    assert_eq!(
        handle
            .privacy_composition_budget_snapshot()
            .expect("privacy budget after catch-up")
            .chains[0]
            .charges
            .len(),
        2
    );
}
#[test]
fn privacy_cycle_prf_derives_distinct_requests_for_catch_up_windows() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let cfg = privacy_aggregate_storage_config(&root);
    let provider = Arc::new(TestPrivacyCyclePrfProvider::bound());
    let trait_provider: Arc<dyn ProductionPrivacyCyclePrfProviderV1> = provider.clone();
    let handle = NodeHandle::try_new_with_runtime_deps(
        cfg,
        with_fresh_test_fenced_privacy_runtime(privacy_runtime_deps(
            trait_provider,
            test_privacy_release_anchor(),
        )),
    )
    .expect("initialise node with recording threshold PRF provider");
    for event in [
        privacy_source_event("alpha-1", "jurisdiction-a", 0xA0, 110),
        privacy_source_event("beta-1", "jurisdiction-b", 0xB0, 210),
        privacy_source_event("beta-2", "jurisdiction-b", 0xB0, 220),
    ] {
        handle
            .record_privacy_aggregate_source_event(event)
            .expect("record source event");
    }
    let first = publish_due_test_privacy_cycle(&handle, 311, 100, 200, "prf-cycle-1")
        .expect("publish first due aggregate cycle");
    match first {
        PrivacyAggregateScheduleOutcome::Published {
            window,
            publication,
        } => {
            assert_eq!(window.cycle_start_unix, 100);
            assert_eq!(window.cycle_end_unix, 200);
            assert_eq!(publication.block.entry_count, 2);
        }
        other => panic!("expected first published outcome, got {other:?}"),
    }
    let second = publish_due_test_privacy_cycle(&handle, 311, 200, 300, "prf-cycle-2")
        .expect("publish second due aggregate cycle");
    match second {
        PrivacyAggregateScheduleOutcome::Published {
            window,
            publication,
        } => {
            assert_eq!(window.cycle_start_unix, 200);
            assert_eq!(window.cycle_end_unix, 300);
            assert_eq!(publication.block.entry_count, 2);
        }
        other => panic!("expected second published outcome, got {other:?}"),
    }
    assert_eq!(handle.pending_governance_publication_count(), 0);
    let requests = provider.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].policy_digest(), [0xC0; 32]);
    assert_eq!(requests[1].policy_digest(), [0xC0; 32]);
    assert_eq!(
        (requests[0].cycle_start_unix(), requests[0].cycle_end_unix()),
        (100, 200)
    );
    assert_eq!(
        (requests[1].cycle_start_unix(), requests[1].cycle_end_unix()),
        (200, 300)
    );
    assert_ne!(requests[0].cycle_id(), requests[1].cycle_id());
    assert_ne!(requests[0].binding_digest(), requests[1].binding_digest());
}
#[test]
fn privacy_cycle_prf_startup_requires_runtime_provider() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let cfg = privacy_aggregate_storage_config(&root);
    assert!(matches!(
        NodeHandle::try_new(cfg),
        Err(NodeInitError::PrivacyCyclePrfProviderQualification {
            error: TransparencyRuntimeProviderQualificationErrorV1::MissingProvider,
        })
    ));
}
#[test]
fn privacy_cycle_prf_qualification_fails_before_persistence() {
    let cases: [(
        Arc<dyn ProductionPrivacyCyclePrfProviderV1>,
        TransparencyRuntimeProviderQualificationErrorV1,
    ); 5] = [
        (
            Arc::new(TestPrivacyCyclePrfProvider::with_qualification(
                "threshold-prf:transparency:secondary",
                1,
                [0xC7; 32],
                false,
            )),
            TransparencyRuntimeProviderQualificationErrorV1::SubstitutedProvider,
        ),
        (
            Arc::new(TestPrivacyCyclePrfProvider::with_qualification(
                TEST_PRIVACY_CYCLE_PRF_PROVIDER_HANDLE,
                2,
                [0xC7; 32],
                false,
            )),
            TransparencyRuntimeProviderQualificationErrorV1::ConfiguredQualificationMismatch,
        ),
        (
            Arc::new(TestPrivacyCyclePrfProvider::with_qualification(
                TEST_PRIVACY_CYCLE_PRF_PROVIDER_HANDLE,
                1,
                [0xC8; 32],
                false,
            )),
            TransparencyRuntimeProviderQualificationErrorV1::ConfiguredQualificationMismatch,
        ),
        (
            Arc::new(TestPrivacyCyclePrfProvider::with_qualification(
                "threshold-prf:test:primary",
                1,
                [0xC7; 32],
                false,
            )),
            TransparencyRuntimeProviderQualificationErrorV1::TestMarkedProviderHandle,
        ),
        (
            Arc::new(TestPrivacyCyclePrfProvider::with_qualification(
                TEST_PRIVACY_CYCLE_PRF_PROVIDER_HANDLE,
                1,
                [0xC7; 32],
                true,
            )),
            TransparencyRuntimeProviderQualificationErrorV1::UnavailableOrStale,
        ),
    ];
    for (provider, expected) in cases {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let cfg = privacy_aggregate_storage_config(&root);
        let data_dir = cfg.data_dir().clone();
        assert!(!data_dir.exists());
        let error = NodeHandle::try_new_with_runtime_deps(
            cfg,
            privacy_runtime_deps(provider, test_privacy_release_anchor()),
        )
        .expect_err("invalid production threshold-PRF qualification must fail startup");
        assert!(matches!(
            &error,
            NodeInitError::PrivacyCyclePrfProviderQualification { error }
                if *error == expected
        ));
        assert!(!error.to_string().contains("must-never-escape"));
        assert!(!format!("{error:?}").contains("must-never-escape"));
        assert!(
            !data_dir.exists(),
            "provider qualification must complete before persistence opens"
        );
    }
}
#[test]
fn differential_privacy_startup_requires_finalized_release_anchor() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let cfg = privacy_aggregate_storage_config(&root);
    assert!(matches!(
        NodeHandle::try_new_with_runtime_deps(
            cfg,
            NodeRuntimeDeps::default()
                .with_privacy_cycle_prf_provider(test_privacy_cycle_prf_provider()),
        ),
        Err(NodeInitError::PrivacyReleaseAnchorQualification {
            error: TransparencyRuntimeProviderQualificationErrorV1::MissingProvider,
        })
    ));
}
#[test]
fn differential_privacy_startup_requires_transparency_leader_lease_provider() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let cfg = privacy_aggregate_storage_config(&root);
    assert!(matches!(
        NodeHandle::try_new_with_runtime_deps(
            cfg,
            NodeRuntimeDeps::default()
                .with_privacy_cycle_prf_provider(test_privacy_cycle_prf_provider())
                .with_privacy_release_anchor(test_privacy_release_anchor()),
        ),
        Err(
            NodeInitError::TransparencyLeaderLeaseProviderQualification {
                error: TransparencyLeaderLeaseErrorV1::ProviderQualification(
                    TransparencyRuntimeProviderQualificationErrorV1::MissingProvider
                ),
            }
        )
    ));
}
#[test]
fn differential_privacy_startup_requires_fused_target_binding_and_both_runtime_roles() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let data_dir = root.join("storage");
    let without_binding = with_test_signed_governance_config(
        StorageConfig::builder()
            .enabled(true)
            .provider_id(Some(ProviderId::new([0x91; 32])))
            .data_dir(data_dir.clone())
            .privacy_aggregate_schedule(Some(privacy_aggregate_schedule_config()))
            .privacy_aggregate_policy(Some(privacy_aggregate_policy_config()))
            .privacy_cycle_prf_provider_binding(Some(test_privacy_cycle_prf_provider_binding()))
            .privacy_release_anchor_provider_binding(Some(test_privacy_release_anchor_binding()))
            .privacy_leader_lease_provider_binding(Some(test_transparency_leader_lease_binding())),
        &root,
    )
    .build();
    let error = NodeHandle::try_new_with_runtime_deps(
        without_binding,
        test_privacy_runtime_deps_without_fenced_target(),
    )
    .expect_err("privacy publication must require its configured fused target binding");
    assert!(
        error
            .to_string()
            .contains("requires an exact configured fused target binding")
    );
    assert!(!data_dir.exists());
    for (label, inject_writer, inject_reader, expected) in [
        (
            "missing writer",
            false,
            true,
            "requires an injected fused target writer",
        ),
        (
            "missing reader",
            true,
            false,
            "requires an injected authenticated authoritative-head reader",
        ),
    ] {
        let temp_dir = tempfile::tempdir().expect("create missing-role temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let config = privacy_aggregate_storage_config(&root);
        let data_dir = config.data_dir().clone();
        let provider = Arc::new(TestFencedTransparencyProvider::bound());
        let mut deps = test_privacy_runtime_deps_without_fenced_target();
        if inject_writer {
            let writer: Arc<dyn FencedTransparencyPublisherV1> = provider.clone();
            deps = deps.with_fenced_transparency_publisher(writer);
        }
        if inject_reader {
            let reader: Arc<dyn FencedTransparencyAuthoritativeHeadReaderV1> = provider;
            deps = deps.with_fenced_transparency_head_reader(reader);
        }
        let error = NodeHandle::try_new_with_runtime_deps(config, deps).expect_err(label);
        assert!(
            error.to_string().contains(expected),
            "{label} produced unexpected error: {error}"
        );
        assert!(!data_dir.exists(), "{label} opened durable state");
    }
}
#[test]
fn fused_privacy_target_qualification_fails_before_persistence() {
    for (label, provider, expected) in [
        (
            "substituted handle",
            TestFencedTransparencyProvider::with_qualification(
                "governance-cas:transparency:privacy-secondary",
                GovernanceDagRuntimeProviderQualificationV1::new(1, [0xF7; 32]),
                false,
            ),
            "identity or policy does not match configuration",
        ),
        (
            "substituted revision",
            TestFencedTransparencyProvider::with_qualification(
                TEST_FENCED_TRANSPARENCY_PROVIDER_HANDLE,
                GovernanceDagRuntimeProviderQualificationV1::new(2, [0xF7; 32]),
                false,
            ),
            "identity or policy does not match configuration",
        ),
        (
            "substituted policy",
            TestFencedTransparencyProvider::with_qualification(
                TEST_FENCED_TRANSPARENCY_PROVIDER_HANDLE,
                GovernanceDagRuntimeProviderQualificationV1::new(1, [0xF8; 32]),
                false,
            ),
            "identity or policy does not match configuration",
        ),
        (
            "test-marked provider",
            TestFencedTransparencyProvider::with_qualification(
                "governance-cas:transparency:test",
                GovernanceDagRuntimeProviderQualificationV1::new(1, [0xF7; 32]),
                false,
            ),
            "identity or policy does not match configuration",
        ),
        (
            "stale provider",
            TestFencedTransparencyProvider::with_qualification(
                TEST_FENCED_TRANSPARENCY_PROVIDER_HANDLE,
                GovernanceDagRuntimeProviderQualificationV1::new(1, [0xF7; 32]),
                true,
            ),
            "unavailable, stale, or unqualified",
        ),
    ] {
        let temp_dir = tempfile::tempdir().expect("create qualification temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let config = privacy_aggregate_storage_config(&root);
        let data_dir = config.data_dir().clone();
        let provider = Arc::new(provider);
        let error = NodeHandle::try_new_with_runtime_deps(
            config,
            with_test_fenced_privacy_runtime(
                test_privacy_runtime_deps_without_fenced_target(),
                provider,
            ),
        )
        .expect_err(label);
        assert!(
            error.to_string().contains(expected),
            "{label} produced unexpected error: {error}"
        );
        assert!(!error.to_string().contains("must-never-escape"));
        assert!(!format!("{error:?}").contains("must-never-escape"));
        assert!(!data_dir.exists(), "{label} opened durable state");
    }
}
#[test]
fn fused_privacy_head_reader_must_match_the_writer_binding() {
    let temp_dir = tempfile::tempdir().expect("create reader mismatch temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let config = privacy_aggregate_storage_config(&root);
    let data_dir = config.data_dir().clone();
    let writer: Arc<dyn FencedTransparencyPublisherV1> =
        Arc::new(TestFencedTransparencyProvider::bound());
    let reader: Arc<dyn FencedTransparencyAuthoritativeHeadReaderV1> =
        Arc::new(TestFencedTransparencyProvider::with_qualification(
            "governance-cas:transparency:privacy-secondary",
            GovernanceDagRuntimeProviderQualificationV1::new(1, [0xF7; 32]),
            false,
        ));
    let error = NodeHandle::try_new_with_runtime_deps(
        config,
        test_privacy_runtime_deps_without_fenced_target()
            .with_fenced_transparency_publisher(writer)
            .with_fenced_transparency_head_reader(reader),
    )
    .expect_err("substituted authoritative-head reader must fail startup");
    assert!(
        error
            .to_string()
            .contains("head reader identity or policy does not match configuration")
    );
    assert!(!data_dir.exists());
}
#[test]
fn disabled_privacy_publication_rejects_unrequested_fused_target_inputs() {
    for (label, config_binding, writer, reader, expected) in [
        (
            "configured binding",
            true,
            false,
            false,
            "binding is unexpected",
        ),
        ("runtime writer", false, true, false, "writer is unexpected"),
        ("runtime reader", false, false, true, "reader is unexpected"),
    ] {
        let temp_dir = tempfile::tempdir().expect("create unexpected-input temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let mut builder = StorageConfig::builder().data_dir(root.join("storage"));
        if config_binding {
            builder = builder.privacy_fenced_publisher_binding(Some(
                test_fenced_transparency_provider_binding(),
            ));
        }
        let provider = Arc::new(TestFencedTransparencyProvider::bound());
        let mut deps = NodeRuntimeDeps::default();
        if writer {
            let provider: Arc<dyn FencedTransparencyPublisherV1> = provider.clone();
            deps = deps.with_fenced_transparency_publisher(provider);
        }
        if reader {
            let provider: Arc<dyn FencedTransparencyAuthoritativeHeadReaderV1> = provider;
            deps = deps.with_fenced_transparency_head_reader(provider);
        }
        let error = NodeHandle::try_new_with_runtime_deps(builder.build(), deps).expect_err(label);
        assert!(
            error.to_string().contains(expected),
            "{label} produced unexpected error: {error}"
        );
    }
}
#[test]
fn privacy_startup_without_explicit_signed_governance_root_fails_before_state() {
    let temp_dir = tempfile::tempdir().expect("create privacy publisher temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let data_dir = root.join("storage");
    let governance_dir = root.join("governance");
    let config = StorageConfig::builder()
        .enabled(true)
        .provider_id(Some(ProviderId::new([0x91; 32])))
        .data_dir(data_dir.clone())
        .privacy_aggregate_schedule(Some(privacy_aggregate_schedule_config()))
        .privacy_aggregate_policy(Some(privacy_aggregate_policy_config()))
        .privacy_cycle_prf_provider_binding(Some(test_privacy_cycle_prf_provider_binding()))
        .privacy_release_anchor_provider_binding(Some(test_privacy_release_anchor_binding()))
        .privacy_leader_lease_provider_binding(Some(test_transparency_leader_lease_binding()))
        .privacy_fenced_publisher_binding(Some(test_fenced_transparency_provider_binding()))
        .build();
    assert!(config.governance_dir().is_none());
    let provider = Arc::new(TestFencedTransparencyProvider::bound());
    let error = NodeHandle::try_new_with_runtime_deps(
        config,
        with_test_fenced_privacy_runtime(
            test_privacy_runtime_deps_without_fenced_target(),
            Arc::clone(&provider),
        ),
    )
    .expect_err("privacy publication must not derive an unsigned local publisher root");
    assert!(
        error
            .to_string()
            .contains("requires an explicit signed Governance DAG directory"),
        "unexpected startup error: {error}"
    );
    assert!(!data_dir.exists());
    assert!(!governance_dir.exists());
    let state = provider.state.lock().expect("fused target state");
    assert!(state.head.is_none());
    assert!(state.publications.is_empty());
    assert!(state.receipts.is_empty());
}
#[test]
fn disabled_privacy_publication_rejects_unrequested_leader_lease_provider() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let cfg = StorageConfig::builder()
        .data_dir(root.join("storage"))
        .build();
    assert!(matches!(
            NodeHandle::try_new_with_runtime_deps(
                cfg,
                NodeRuntimeDeps::default().with_transparency_leader_lease_provider(
                    test_transparency_leader_lease_provider(),
                ),
            ),
            Err(
                NodeInitError::TransparencyLeaderLeaseProviderQualification {
                    error: TransparencyLeaderLeaseErrorV1::ProviderQualification(
                        TransparencyRuntimeProviderQualificationErrorV1::UnexpectedProvider
                    ),
                }
            )
        ));
}
#[test]
fn privacy_cycle_prf_rejects_zero_output_before_provider_boundary() {
    assert_eq!(
        PrivacyCyclePrfOutputV1::new([0; 32]).expect_err("zero output must fail"),
        PrivacyCyclePrfInputErrorV1::ZeroOutput
    );
}
#[test]
fn privacy_cycle_prf_redacts_failed_provider_diagnostics() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let cfg = privacy_aggregate_storage_config(&root);
    let provider: Arc<dyn ProductionPrivacyCyclePrfProviderV1> = Arc::new(
        TestPrivacyCyclePrfProvider::failing(PrivacyCyclePrfProviderErrorV1::Internal),
    );
    let handle = NodeHandle::try_new_with_runtime_deps(
        cfg,
        with_fresh_test_fenced_privacy_runtime(privacy_runtime_deps(
            provider,
            test_privacy_release_anchor(),
        )),
    )
    .expect("initialise node with error-injecting threshold PRF provider");
    for event in [
        privacy_source_event("alpha-1", "jurisdiction-a", 0xA0, 110),
        privacy_source_event("alpha-2", "jurisdiction-a", 0xA0, 120),
    ] {
        handle
            .record_privacy_aggregate_source_event(event)
            .expect("record source event");
    }
    let error = handle
        .publish_due_configured_privacy_aggregate_cycle_from_source_events(
            privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
            "failed-provider".to_string(),
        )
        .expect_err("failed provider output must fail closed");
    assert_eq!(
        error.to_string(),
        "runtime threshold PRF provider internal failure"
    );
    assert!(
        !error
            .to_string()
            .contains("TEST-PRF-VENDOR-DIAGNOSTIC-MUST-NOT-LEAK")
    );
}
#[test]
fn privacy_cycle_prf_debug_redacts_runtime_crypto_provider_implementations() {
    let temp_dir = tempfile::tempdir().expect("create debug-provider temp dir");
    let root = temp_dir
        .path()
        .canonicalize()
        .expect("canonical debug-provider temp dir");
    let config = with_test_signed_governance_config(
        privacy_aggregate_storage_builder(&root)
            .moderation_quarantine_key_provider(Some(test_quarantine_key_provider_config())),
        &root,
    )
    .build();
    let privacy_provider: Arc<dyn ProductionPrivacyCyclePrfProviderV1> =
        Arc::new(TestPrivacyCyclePrfProvider::bound());
    let quarantine_wrapper = test_quarantine_key_wrapper();
    let runtime_deps = with_fresh_test_fenced_privacy_runtime(
        NodeRuntimeDeps::default()
            .with_moderation_quarantine_key_wrapper(quarantine_wrapper)
            .with_privacy_cycle_prf_provider(privacy_provider)
            .with_privacy_release_anchor(test_privacy_release_anchor())
            .with_transparency_leader_lease_provider(test_transparency_leader_lease_provider()),
    );
    let runtime_debug = format!("{runtime_deps:?}");
    assert!(runtime_debug.contains("fenced_transparency_publisher: true"));
    assert!(runtime_debug.contains("fenced_transparency_head_reader: true"));
    assert!(!runtime_debug.contains("TestFencedTransparencyProvider"));
    let node = NodeHandle::try_new_with_runtime_deps(config, runtime_deps)
        .expect("initialise node with runtime crypto providers");
    let debug = format!("{node:?}");
    assert!(debug.contains("ModerationQuarantineKeyWrapper(<runtime-only>)"));
    assert!(debug.contains("PrivacyCyclePrfProviderV1(<runtime-only>)"));
    assert!(debug.contains("PrivacyReleaseAnchorV1(<runtime-only>)"));
    assert!(debug.contains("TransparencyLeaderLeaseProviderV1(<runtime-only>)"));
    assert!(!debug.contains("kms:test/quarantine-v1"));
    assert!(!debug.contains("TEST-PRF-VENDOR-DIAGNOSTIC-MUST-NOT-LEAK"));
}
#[test]
fn publish_due_configured_privacy_aggregate_cycle_uses_storage_config() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let schedule = privacy_aggregate_schedule_config();
    let cfg = privacy_aggregate_storage_config(&root);
    let handle = node_with_test_privacy_cycle_prf_provider(cfg);
    assert_eq!(
        handle.configured_privacy_aggregate_schedule(),
        Some(schedule)
    );
    assert!(handle.has_governance_publisher());
    for event in [
        privacy_source_event("alpha-1", "jurisdiction-a", 0xA0, 110),
        privacy_source_event("alpha-2", "jurisdiction-a", 0xA0, 120),
    ] {
        handle
            .record_privacy_aggregate_source_event(event)
            .expect("record source event");
    }
    let outcome = handle
        .publish_due_configured_privacy_aggregate_cycle_from_source_events(
            privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
            "configured-cycle-1".to_string(),
        )
        .expect("publish configured aggregate cycle");
    let publication = match outcome {
        PrivacyAggregateScheduleOutcome::Published {
            window,
            publication,
        } => {
            assert_eq!(window.cycle_start_unix, 100);
            assert_eq!(window.cycle_end_unix, 200);
            publication
        }
        other => panic!("expected published outcome, got {other:?}"),
    };
    assert_eq!(publication.block.generated_at_unix, 200);
    assert_eq!(handle.pending_governance_publication_count(), 0);
    assert_eq!(handle.privacy_aggregate_source_event_count(), 0);
    let budget = handle
        .privacy_composition_budget_snapshot()
        .expect("privacy composition budget snapshot");
    assert_eq!(budget.chains.len(), 1);
    assert_eq!(budget.chains[0].charges.len(), 1);
    assert_eq!(
        budget.chains[0].charges[0].cycle_id,
        publication.block.cycle_id
    );
    assert_eq!(
        handle
            .record_privacy_aggregate_source_event(privacy_source_event(
                "alpha-1",
                "jurisdiction-a",
                0xA0,
                110,
            ))
            .expect("processed event retry remains idempotent"),
        PrivacySourceEventRecordOutcomeV1::AlreadyRecorded
    );
    let replay_error = handle
        .record_privacy_aggregate_source_event(privacy_source_event(
            "late-replay",
            "jurisdiction-a",
            0xA0,
            110,
        ))
        .expect_err("a finalized release window must reject later source events");
    assert!(
        replay_error
            .to_string()
            .contains("targets a finalized release window")
    );
    assert_eq!(handle.privacy_aggregate_source_event_count(), 0);
}
#[test]
fn privacy_publication_budget_state_and_fused_head_restore_atomically() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let cfg = privacy_aggregate_storage_config(&root);
    let anchor = Arc::new(TestPrivacyReleaseAnchor::default());
    let fused_provider = Arc::new(TestFencedTransparencyProvider::bound());
    let source = NodeHandle::try_new_with_runtime_deps(
        cfg.clone(),
        fenced_test_privacy_runtime_deps(anchor.clone(), fused_provider.clone()),
    )
    .expect("initialise source node with shared release anchor");
    for event in [
        privacy_source_event("alpha-1", "jurisdiction-a", 0xA0, 110),
        privacy_source_event("alpha-2", "jurisdiction-a", 0xA0, 120),
    ] {
        source
            .record_privacy_aggregate_source_event(event)
            .expect("record source event");
    }
    let published = source
        .publish_due_configured_privacy_aggregate_cycle_from_source_events(
            privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
            "restore-cycle-1".to_string(),
        )
        .expect("commit configured privacy cycle through the fused publisher");
    let cycle_id = match published {
        PrivacyAggregateScheduleOutcome::Published { publication, .. } => {
            publication.block.cycle_id
        }
        other => panic!("expected published outcome, got {other:?}"),
    };
    assert_eq!(source.pending_governance_publication_count(), 0);
    assert_eq!(source.privacy_aggregate_source_event_count(), 0);
    let persisted_fencing_floor = source
        .transparency_leader_lease_fencing_floor()
        .expect("read source leader-lease fencing floor")
        .expect("privacy publication has a leader-lease provider");
    assert!(persisted_fencing_floor > 0);
    source
        .revalidate_fenced_privacy_runtime()
        .expect("current fused head and inclusion revalidate");
    fused_provider.override_authoritative_head(Some(None));
    let error = source
        .revalidate_fenced_privacy_runtime()
        .expect_err("post-construction target rollback must fail preflight");
    assert!(
        error.to_string().contains("authoritative")
            || error.to_string().contains("ancestry")
            || error.to_string().contains("unavailable"),
        "unexpected target rollback error: {error}"
    );
    fused_provider.override_authoritative_head(None);
    source
        .revalidate_fenced_privacy_runtime()
        .expect("restored authoritative target revalidates");
    drop(source);
    let restored = NodeHandle::try_new_with_runtime_deps(
        cfg,
        fenced_test_privacy_runtime_deps(anchor, fused_provider),
    )
    .expect("restore node with shared release anchor");
    assert_eq!(restored.pending_governance_publication_count(), 0);
    assert_eq!(restored.privacy_aggregate_source_event_count(), 0);
    assert_eq!(
        restored
            .transparency_leader_lease_fencing_floor()
            .expect("read restored leader-lease fencing floor"),
        Some(persisted_fencing_floor)
    );
    let budget = restored
        .privacy_composition_budget_snapshot()
        .expect("restored privacy budget");
    assert_eq!(budget.chains.len(), 1);
    assert_eq!(budget.chains[0].charges.len(), 1);
    assert_eq!(budget.chains[0].charges[0].cycle_id, cycle_id);
    let repeated = restored
        .publish_due_configured_privacy_aggregate_cycle_from_source_events(
            privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
            "restore-cycle-1".to_string(),
        )
        .expect("repeat restored privacy cycle");
    assert!(matches!(
        repeated,
        PrivacyAggregateScheduleOutcome::Published { .. }
    ));
    assert_eq!(
        restored
            .privacy_composition_budget_snapshot()
            .expect("budget after replay")
            .chains[0]
            .charges
            .len(),
        1
    );
    assert_eq!(restored.pending_governance_publication_count(), 0);
}
#[test]
fn privacy_publish_receipt_checkpoint_rejects_pre_due_observation_and_delay_tampering() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let cfg = privacy_aggregate_storage_config(&root);
    let checkpoint_path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let anchor = Arc::new(TestPrivacyReleaseAnchor::default());
    let fused_provider = Arc::new(TestFencedTransparencyProvider::bound());
    let source = NodeHandle::try_new_with_runtime_deps(
        cfg.clone(),
        fenced_test_privacy_runtime_deps(anchor.clone(), fused_provider.clone()),
    )
    .expect("initialise privacy receipt source node");
    assert!(matches!(
        source
            .publish_due_configured_privacy_aggregate_cycle_from_source_events(
                privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                "receipt-tamper-cycle-1".to_string(),
            )
            .expect("commit privacy release"),
        PrivacyAggregateScheduleOutcome::Published { .. }
    ));
    drop(source);
    let original = fs::read(&checkpoint_path).expect("read privacy receipt checkpoint");
    for tamper in 0..2 {
        let mut checkpoint: AuxiliaryRuntimeCheckpointV5 =
            norito::decode_from_bytes(&original).expect("decode privacy receipt checkpoint");
        let receipt = checkpoint
            .privacy_publish_request_receipts
            .first_mut()
            .expect("privacy publish receipt");
        match tamper {
            0 => receipt.requested_now_unix = receipt.due_at_unix.saturating_sub(1),
            1 => {
                receipt.publish_delay_seconds = receipt.publish_delay_seconds.saturating_add(1);
            }
            _ => unreachable!("bounded privacy receipt tamper case"),
        }
        write_local_checkpoint_atomic(
            &checkpoint_path,
            &norito::to_bytes(&checkpoint).expect("encode tampered privacy receipt checkpoint"),
        )
        .expect("write tampered privacy receipt checkpoint");
        let error = NodeHandle::try_new_with_runtime_deps(
            cfg.clone(),
            fenced_test_privacy_runtime_deps(anchor.clone(), fused_provider.clone()),
        )
        .expect_err("tampered privacy publish receipt must fail restart");
        assert!(matches!(
            error,
            NodeInitError::Checkpoint {
                component: "auxiliary runtime",
                ..
            }
        ));
    }
    write_local_checkpoint_atomic(&checkpoint_path, &original)
        .expect("restore canonical privacy receipt checkpoint");
    NodeHandle::try_new_with_runtime_deps(
        cfg,
        fenced_test_privacy_runtime_deps(anchor, fused_provider),
    )
    .expect("canonical privacy receipt checkpoint restores");
}
#[test]
fn privacy_source_receipt_only_checkpoint_rejects_oversized_event_id() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let cfg = privacy_aggregate_storage_config(&root);
    let checkpoint_path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let anchor = Arc::new(TestPrivacyReleaseAnchor::default());
    let source = NodeHandle::try_new_with_runtime_deps(
        cfg.clone(),
        fresh_test_privacy_runtime_deps(anchor.clone()),
    )
    .expect("initialise privacy source-receipt node");
    source
        .record_privacy_aggregate_source_event(privacy_source_event(
            "receipt-only-event",
            "jurisdiction-a",
            0xA0,
            110,
        ))
        .expect("record privacy source event");
    assert!(matches!(
        source
            .publish_due_configured_privacy_aggregate_cycle_from_source_events(
                privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                "receipt-only-cycle-1".to_string(),
            )
            .expect("commit privacy release"),
        PrivacyAggregateScheduleOutcome::Published { .. }
    ));
    assert_eq!(source.privacy_aggregate_source_event_count(), 0);
    drop(source);
    let bytes = fs::read(&checkpoint_path).expect("read receipt-only checkpoint");
    let mut checkpoint: AuxiliaryRuntimeCheckpointV5 =
        norito::decode_from_bytes(&bytes).expect("decode receipt-only checkpoint");
    assert!(checkpoint.privacy_source_events.is_empty());
    checkpoint
        .privacy_source_event_receipts
        .first_mut()
        .expect("retained source-event receipt")
        .event_id = "x".repeat(MODERATION_LEDGER_MAX_PUBLIC_TEXT_BYTES_V1 + 1);
    write_local_checkpoint_atomic(
        &checkpoint_path,
        &norito::to_bytes(&checkpoint).expect("encode oversized receipt-only checkpoint"),
    )
    .expect("write oversized receipt-only checkpoint");
    let error = NodeHandle::try_new_with_runtime_deps(cfg, fresh_test_privacy_runtime_deps(anchor))
        .expect_err("oversized retained source-event receipt must fail restart");
    assert!(matches!(
        error,
        NodeInitError::Checkpoint {
            component: "auxiliary runtime",
            ..
        }
    ));
}
#[test]
fn privacy_release_restart_rejects_cadence_and_delay_rotation() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let cfg = privacy_aggregate_storage_config(&root);
    let anchor = Arc::new(TestPrivacyReleaseAnchor::default());
    let source =
        NodeHandle::try_new_with_runtime_deps(cfg, fresh_test_privacy_runtime_deps(anchor.clone()))
            .expect("initialise privacy delay-lineage node");
    assert!(matches!(
        source
            .publish_due_configured_privacy_aggregate_cycle_from_source_events(
                privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                "delay-lineage-cycle-1".to_string(),
            )
            .expect("commit privacy release"),
        PrivacyAggregateScheduleOutcome::Published { .. }
    ));
    drop(source);
    let mut delay_schedule = privacy_aggregate_schedule_config();
    delay_schedule.publish_delay_seconds = delay_schedule.publish_delay_seconds.saturating_add(1);
    let mut first_schedule = privacy_aggregate_schedule_config();
    first_schedule.first_cycle_start_unix = first_schedule
        .first_cycle_start_unix
        .saturating_add(first_schedule.cycle_seconds);
    let mut first_cycle = privacy_aggregate_cycle_config();
    first_cycle.first_cycle_start_unix = first_schedule.first_cycle_start_unix;
    let mut width_schedule = privacy_aggregate_schedule_config();
    width_schedule.cycle_seconds /= 2;
    let mut width_cycle = privacy_aggregate_cycle_config();
    width_cycle.cycle_seconds = width_schedule.cycle_seconds;
    for (case, schedule, policy) in [
        (
            "publish delay",
            delay_schedule,
            privacy_aggregate_policy_config(),
        ),
        (
            "first-cycle activation",
            first_schedule,
            privacy_aggregate_policy_config_for_cycle(first_cycle),
        ),
        (
            "cycle width",
            width_schedule,
            privacy_aggregate_policy_config_for_cycle(width_cycle),
        ),
    ] {
        let rotated_cfg = with_test_signed_governance_config(
            StorageConfig::builder()
                .enabled(true)
                .provider_id(Some(ProviderId::new([0x91; 32])))
                .data_dir(root.join("storage"))
                .privacy_aggregate_schedule(Some(schedule))
                .privacy_aggregate_policy(Some(policy))
                .privacy_cycle_prf_provider_binding(Some(test_privacy_cycle_prf_provider_binding()))
                .privacy_release_anchor_provider_binding(
                    Some(test_privacy_release_anchor_binding()),
                )
                .privacy_leader_lease_provider_binding(Some(
                    test_transparency_leader_lease_binding(),
                ))
                .privacy_fenced_publisher_binding(
                    Some(test_fenced_transparency_provider_binding()),
                ),
            &root,
        )
        .build();
        let error = NodeHandle::try_new_with_runtime_deps(
            rotated_cfg,
            fresh_test_privacy_runtime_deps(anchor.clone()),
        )
        .expect_err("cadence rotation must fail the durable query lineage");
        assert!(
            matches!(
                &error,
                NodeInitError::Checkpoint {
                    component: "auxiliary runtime",
                    ..
                }
            ),
            "{case} rotation returned the wrong startup error: {error}"
        );
        assert!(
            error
                .to_string()
                .contains("cadence does not match the configured query lineage"),
            "{case} rotation was not rejected as a lineage conflict: {error}"
        );
    }
}
#[test]
fn privacy_checkpoint_rollback_behind_finalized_release_anchor_fails_closed() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let cfg = privacy_aggregate_storage_config(&root);
    let checkpoint_path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let anchor = Arc::new(TestPrivacyReleaseAnchor::default());
    let source = NodeHandle::try_new_with_runtime_deps(
        cfg.clone(),
        fresh_test_privacy_runtime_deps(anchor.clone()),
    )
    .expect("initialise source node");
    for event in [
        privacy_source_event("alpha-1", "jurisdiction-a", 0xA0, 110),
        privacy_source_event("alpha-2", "jurisdiction-a", 0xA0, 120),
    ] {
        source
            .record_privacy_aggregate_source_event(event)
            .expect("record source event");
    }
    let rolled_back_checkpoint =
        fs::read(&checkpoint_path).expect("capture pre-release checkpoint");
    assert!(matches!(
        source
            .publish_due_configured_privacy_aggregate_cycle_from_source_events(
                privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                "rollback-cycle-1".to_string(),
            )
            .expect("commit privacy release"),
        PrivacyAggregateScheduleOutcome::Published { .. }
    ));
    assert_eq!(
        anchor
            .finalized_head([0xB0; 32])
            .expect("read finalized release head")
            .sequence(),
        1
    );
    drop(source);
    fs::write(&checkpoint_path, rolled_back_checkpoint)
        .expect("simulate checkpoint rollback behind finalized head");
    let error = NodeHandle::try_new_with_runtime_deps(cfg, fresh_test_privacy_runtime_deps(anchor))
        .expect_err("rollback behind the finalized release anchor must fail");
    assert!(
        error
            .to_string()
            .contains("behind or equivocates with the finalized anchor")
    );
}
#[test]
fn publish_due_configured_privacy_aggregate_cycle_skips_when_disabled() {
    let cfg = StorageConfig::builder()
        .enabled(false)
        .privacy_aggregate_schedule(None)
        .build();
    let handle = NodeHandle::new(cfg);
    assert_eq!(handle.configured_privacy_aggregate_schedule(), None);
    let outcome = handle
        .publish_due_configured_privacy_aggregate_cycle_from_source_events(
            privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
            "disabled-cycle".to_string(),
        )
        .expect("disabled configured aggregate cycle");
    assert_eq!(outcome, PrivacyAggregateScheduleOutcome::Disabled);
}
#[test]
fn publish_due_privacy_aggregate_cycle_from_source_events_emits_fixed_empty_population_set() {
    let (cfg, _dir) = privacy_aggregate_storage_config_with_temp_dir();
    let handle = node_with_test_privacy_cycle_prf_provider(cfg);
    let empty = publish_due_test_privacy_cycle(&handle, 211, 100, 200, "empty-cycle-1")
        .expect("empty due aggregate cycle");
    let publication = match empty {
        PrivacyAggregateScheduleOutcome::Published {
            window,
            publication,
        } => {
            assert_eq!(window.cycle_start_unix, 100);
            assert_eq!(window.cycle_end_unix, 200);
            publication
        }
        other => panic!("expected empty fixed-schema publication, got {other:?}"),
    };
    assert_eq!(publication.block.generated_at_unix, 200);
    assert_eq!(publication.block.entry_count, 2);
    assert_eq!(handle.pending_governance_publication_count(), 0);
}
#[test]
fn governance_publisher_presence_tracks_set_and_clear() {
    let (handle, _dir) = node_with_temp_storage();
    assert!(!handle.has_governance_publisher());
    let publisher = Arc::new(RecordingPublisher::default());
    let trait_publisher: Arc<dyn GovernancePublisher> = publisher;
    handle.set_governance_publisher(trait_publisher);
    assert!(handle.has_governance_publisher());
    handle.clear_governance_publisher();
    assert!(!handle.has_governance_publisher());
}
#[test]
fn node_handle_exposes_only_typed_governance_authority_snapshots() {
    let temp = tempfile::tempdir().expect("typed authority temp dir");
    let root = temp.path().canonicalize().expect("canonical temp root");
    let config =
        with_test_signed_governance_config(enabled_storage_builder(root.join("storage")), &root)
            .build();
    let node = NodeHandle::try_new_with_runtime_deps(
        config,
        NodeRuntimeDeps::default()
            .with_governance_dag_signer(Arc::new(TestGovernanceDagSigner::new()))
            .with_governance_dag_checkpoint_store(Arc::new(
                TestGovernanceDagCheckpointStore::default(),
            )),
    )
    .expect("start typed Governance DAG producer");
    let clone_before_mirror_install = node.clone();
    assert!(
        Arc::ptr_eq(
            &node.governance_dag_mirror_reader,
            &clone_before_mirror_install.governance_dag_mirror_reader,
        ),
        "clones created before service preparation must share the exact installation slot"
    );
    let empty_publication = node
        .governance_dag_publication_snapshot()
        .expect("read empty typed publication authority")
        .expect("initialized empty publication authority is authenticated");
    assert!(empty_publication.store_identity().0 > 0);
    assert_ne!(empty_publication.store_identity().1, [0; 32]);
    let empty_value: norito::json::Value =
        norito::json::from_slice(empty_publication.canonical_bytes())
            .expect("decode empty publication authority");
    assert_eq!(
        empty_value
            .get("publish_index")
            .and_then(|index| index.get("entry_count"))
            .and_then(norito::json::Value::as_u64),
        Some(0)
    );
    assert!(
        node.governance_dag_runtime_snapshot()
            .expect("read authenticated genesis runtime authority")
            .is_none()
    );
    assert!(
        node.governance_dag_mirror_snapshot()
            .expect("an absent mirror capability is not an error")
            .is_none()
    );
    assert!(
        clone_before_mirror_install
            .governance_dag_mirror_snapshot()
            .expect("a clone observes the same absent capability")
            .is_none()
    );
    node.publish_authenticated_appeal_finance_weekly_rollup(
        appeal_finance_weekly_rollup_fixture(),
        governance_submission_account(0xB6),
    )
    .expect("publish one typed Governance DAG entry");
    let publication = node
        .governance_dag_publication_snapshot()
        .expect("read typed publication authority")
        .expect("published authority is present");
    assert!(!publication.canonical_bytes().is_empty());
    assert!(publication.store_identity().0 > 0);
    assert_ne!(publication.store_identity().1, [0; 32]);
    let publication_value: norito::json::Value =
        norito::json::from_slice(publication.canonical_bytes())
            .expect("decode canonical publication authority");
    assert_eq!(
        publication_value
            .get("root")
            .and_then(norito::json::Value::as_str),
        Some(".")
    );
    let runtime = node
        .governance_dag_runtime_snapshot()
        .expect("read authenticated runtime authority")
        .expect("published runtime authority is present");
    assert!(!runtime.head_bytes().is_empty());
    assert!(!runtime.index_bytes().is_empty());
    assert!(runtime.store_identity().0 > 0);
    assert_ne!(runtime.store_identity().1, [0; 32]);
    assert!(runtime.checkpoint_identity().0 > 0);
    assert_ne!(runtime.checkpoint_identity().1, [0; 32]);
}
#[test]
fn configured_governance_dir_requires_complete_runtime_signing_identity() {
    let missing_identity_dir = tempfile::tempdir().expect("missing identity temp dir");
    let missing_identity_root = missing_identity_dir
        .path()
        .canonicalize()
        .expect("canonical missing identity root");
    let missing_identity = enabled_storage_builder(missing_identity_root.join("storage"))
        .governance_dir(Some(missing_identity_root.join("governance")))
        .build();
    let error = NodeHandle::try_new(missing_identity)
        .expect_err("governance directory without a signed identity must fail");
    assert!(matches!(
        error,
        NodeInitError::GovernancePublisher(message)
            if message.contains(
                "requires peer id, signer handle, signer revision, signer policy digest, public key"
            )
    ));
    assert!(!missing_identity_root.join("storage").exists());
    assert!(!missing_identity_root.join("governance").exists());
    let signer = Arc::new(TestGovernanceDagSigner::new());
    let signed_dir = tempfile::tempdir().expect("signed publisher temp dir");
    let signed_root = signed_dir
        .path()
        .canonicalize()
        .expect("canonical signed publisher root");
    let signed_config = governance_signer_storage_config(
        &signed_root,
        true,
        &signer,
        signer.handle(),
        TestGovernanceDagSigner::expected_qualification(),
    );
    let error = NodeHandle::try_new(signed_config.clone())
        .expect_err("configured identity without injected signer must fail");
    assert!(matches!(
        error,
        NodeInitError::GovernancePublisher(message)
            if message.contains("injected runtime signer")
    ));
    let handle = NodeHandle::try_new_with_runtime_deps(
        signed_config,
        governance_signer_runtime_deps(signer.clone()),
    )
    .expect("complete runtime-signed Governance DAG publisher starts");
    assert!(handle.has_governance_publisher());
    handle
        .revalidate_fenced_privacy_runtime()
        .expect("startup-pinned signed publisher revalidates");
    let replacement: Arc<dyn GovernancePublisher> = Arc::new(RecordingPublisher::default());
    let error = handle
        .try_set_governance_publisher(replacement)
        .expect_err("startup-pinned signed publisher cannot be replaced");
    assert!(error.to_string().contains("cannot be replaced"));
    handle.clear_governance_publisher();
    assert!(
        handle.has_governance_publisher(),
        "startup-pinned signed publisher cannot be cleared"
    );
    handle
        .revalidate_fenced_privacy_runtime()
        .expect("rejected publisher mutation leaves the exact instance active");
    signer.qualification_refuse.store(true, Ordering::SeqCst);
    let error = handle
        .revalidate_fenced_privacy_runtime()
        .expect_err("post-construction signer revocation must fail preflight");
    assert!(error.to_string().contains("stale"));
    assert!(!error.to_string().contains("must-never-escape"));
    signer.qualification_refuse.store(false, Ordering::SeqCst);
    handle
        .revalidate_fenced_privacy_runtime()
        .expect("restored signer qualification revalidates");
}
#[test]
fn node_restart_recovers_checkpoint_cas_applied_response_error() {
    let temp = tempfile::tempdir().expect("producer recovery temp dir");
    let root = temp.path().canonicalize().expect("canonical temp root");
    let config =
        with_test_signed_governance_config(enabled_storage_builder(root.join("storage")), &root)
            .build();
    let checkpoint_store = Arc::new(TestGovernanceDagCheckpointStore::default());
    let node = NodeHandle::try_new_with_runtime_deps(
        config.clone(),
        NodeRuntimeDeps::default()
            .with_governance_dag_signer(Arc::new(TestGovernanceDagSigner::new()))
            .with_governance_dag_checkpoint_store(checkpoint_store.clone()),
    )
    .expect("start node with signed producer providers");
    checkpoint_store
        .fail_after_next_checkpoint_cas
        .store(true, Ordering::SeqCst);
    let error = node
        .publish_authenticated_appeal_finance_weekly_rollup(
            appeal_finance_weekly_rollup_fixture(),
            governance_submission_account(0xB6),
        )
        .expect_err("ambiguous checkpoint CAS response must surface");
    assert!(error.to_string().contains("compare-and-swap failed"));
    assert!(
        checkpoint_store
            .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
            .expect("load retained producer intent")
            .is_some()
    );
    drop(node);
    let restored = NodeHandle::try_new_with_runtime_deps(
        config,
        NodeRuntimeDeps::default()
            .with_governance_dag_signer(Arc::new(TestGovernanceDagSigner::new()))
            .with_governance_dag_checkpoint_store(checkpoint_store.clone()),
    )
    .expect("restart authenticates already-committed producer target");
    assert!(
        checkpoint_store
            .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
            .expect("reload producer intent")
            .is_none()
    );
    restored
        .revalidate_fenced_privacy_runtime()
        .expect("restored signed producer root revalidates");
}
#[test]
fn governance_signer_qualification_precedes_durable_node_state() {
    let temp = tempfile::tempdir().expect("signer qualification temp dir");
    let root = temp.path().canonicalize().expect("canonical temp root");
    let data_dir = root.join("storage");
    let governance_dir = root.join("governance");
    let signer = Arc::new(TestGovernanceDagSigner::new());
    let config = governance_signer_storage_config(
        &root,
        true,
        &signer,
        signer.handle(),
        TestGovernanceDagSigner::expected_qualification(),
    );
    signer.qualification_refuse.store(true, Ordering::SeqCst);
    let error =
        NodeHandle::try_new_with_runtime_deps(config, governance_signer_runtime_deps(signer))
            .expect_err("stale signer must fail before node durability opens");
    let rendered = error.to_string();
    assert!(rendered.contains("stale"));
    assert!(!rendered.contains("must-never-escape"));
    assert!(
        !data_dir.exists(),
        "storage must not open before Governance DAG signer qualification"
    );
    assert!(
        !governance_dir.exists(),
        "publisher root must not open before Governance DAG signer qualification"
    );
}
#[test]
fn governance_signer_rejects_test_marked_binding_before_durable_state() {
    let temp = tempfile::tempdir().expect("test-marked signer temp dir");
    let root = temp.path().canonicalize().expect("canonical temp root");
    let data_dir = root.join("storage");
    let governance_dir = root.join("governance");
    let signer = Arc::new(TestGovernanceDagSigner::new());
    let config = governance_signer_storage_config(
        &root,
        true,
        &signer,
        "pkcs11:governance-dag:test",
        TestGovernanceDagSigner::expected_qualification(),
    );
    let error =
        NodeHandle::try_new_with_runtime_deps(config, governance_signer_runtime_deps(signer))
            .expect_err("test-marked signer handle must fail before node durability opens");
    assert!(error.to_string().contains("test-marked"));
    assert!(!data_dir.exists());
    assert!(!governance_dir.exists());
}
#[test]
fn governance_signer_rejects_substituted_qualification_and_startup_drift_before_state() {
    for (label, configured_handle, expected_qualification, drift_on_second_read, expected_error) in [
        (
            "substituted signer handle",
            "pkcs11:governance-dag:other",
            TestGovernanceDagSigner::expected_qualification(),
            false,
            "handle does not match",
        ),
        (
            "substituted revision",
            "pkcs11:governance-dag:node-primary",
            GovernanceDagRuntimeProviderQualificationV1::new(2, [0x84; 32]),
            false,
            "policy qualification does not match",
        ),
        (
            "substituted policy digest",
            "pkcs11:governance-dag:node-primary",
            GovernanceDagRuntimeProviderQualificationV1::new(1, [0x85; 32]),
            false,
            "policy qualification does not match",
        ),
        (
            "qualification drift",
            "pkcs11:governance-dag:node-primary",
            TestGovernanceDagSigner::expected_qualification(),
            true,
            "policy changed during startup qualification",
        ),
    ] {
        let temp = tempfile::tempdir().expect("signer binding temp dir");
        let root = temp.path().canonicalize().expect("canonical temp root");
        let data_dir = root.join("storage");
        let governance_dir = root.join("governance");
        let signer = Arc::new(TestGovernanceDagSigner::new());
        signer
            .drift_on_second_qualification_read
            .store(drift_on_second_read, Ordering::SeqCst);
        let config = governance_signer_storage_config(
            &root,
            true,
            &signer,
            configured_handle,
            expected_qualification,
        );
        let error =
            NodeHandle::try_new_with_runtime_deps(config, governance_signer_runtime_deps(signer))
                .expect_err(label);
        assert!(
            error.to_string().contains(expected_error),
            "{label} produced unexpected error: {error}"
        );
        assert!(!data_dir.exists(), "{label} opened storage state");
        assert!(
            !governance_dir.exists(),
            "{label} opened Governance DAG state"
        );
    }
}
#[test]
fn governance_signer_binding_without_publisher_directory_precedes_durable_state() {
    for complete_binding in [false, true] {
        let label = if complete_binding {
            "complete dormant signer binding"
        } else {
            "partial dormant signer binding"
        };
        let temp = tempfile::tempdir().expect("dormant signer temp dir");
        let root = temp.path().canonicalize().expect("canonical temp root");
        let data_dir = root.join("storage");
        let governance_dir = root.join("governance");
        let signer = Arc::new(TestGovernanceDagSigner::new());
        let mut builder = enabled_storage_builder(data_dir.clone())
            .governance_dag_signer_handle(Some(signer.handle().to_owned()));
        if complete_binding {
            builder = builder
                .governance_dag_publisher_peer_id(Some(
                    String::from_utf8(signer.publisher_peer_id().to_vec())
                        .expect("test peer id is UTF-8"),
                ))
                .governance_dag_signer_qualification(Some(
                    TestGovernanceDagSigner::expected_qualification(),
                ))
                .governance_dag_checkpoint_store_handle(Some(
                    TestGovernanceDagCheckpointStore::HANDLE.to_owned(),
                ))
                .governance_dag_checkpoint_store_qualification(Some(
                    TestGovernanceDagCheckpointStore::expected_qualification(),
                ))
                .governance_dag_publisher_public_key_hex(Some(hex::encode(signer.public_key())));
        }
        let error = NodeHandle::try_new_with_runtime_deps(
            builder.build(),
            NodeRuntimeDeps::default().with_governance_dag_signer(signer),
        )
        .expect_err(label);
        assert!(
            error
                .to_string()
                .contains("forbidden without a configured Governance DAG directory"),
            "{label} produced unexpected error: {error}"
        );
        assert!(!data_dir.exists(), "{label} opened storage state");
        assert!(!governance_dir.exists(), "{label} opened publisher state");
    }
}
#[test]
fn governance_publisher_rejects_disabled_storage_before_durable_state() {
    let temp = tempfile::tempdir().expect("disabled publisher temp dir");
    let root = temp.path().canonicalize().expect("canonical temp root");
    let data_dir = root.join("storage");
    let governance_dir = root.join("governance");
    let signer = Arc::new(TestGovernanceDagSigner::new());
    let config = governance_signer_storage_config(
        &root,
        false,
        &signer,
        signer.handle(),
        TestGovernanceDagSigner::expected_qualification(),
    );
    let error = NodeHandle::try_new_with_runtime_deps(
        config,
        NodeRuntimeDeps::default().with_governance_dag_signer(signer),
    )
    .expect_err("disabled storage must not open a Governance DAG publisher");
    assert!(
        error.to_string().contains("requires storage.enabled"),
        "unexpected disabled-publisher error: {error}"
    );
    assert!(!data_dir.exists());
    assert!(!governance_dir.exists());
}
#[test]
fn publish_appeal_finance_weekly_rollup_writes_governance_publisher() {
    let (handle, publisher, _dir) = node_with_temp_storage_and_recording_publisher();
    let rollup = appeal_finance_weekly_rollup_fixture();
    let expected = to_bytes(&rollup).expect("encode appeal finance weekly rollup");
    handle
        .publish_authenticated_appeal_finance_weekly_rollup(
            rollup,
            governance_submission_account(0xB7),
        )
        .expect("publish appeal finance weekly rollup");
    let published = publisher.take();
    assert_eq!(published, vec![expected]);
    assert_eq!(handle.transparency_ledger_source_entry_count(), 0);
}
#[test]
fn publish_appeal_finance_settlement_receipt_writes_governance_publisher() {
    let (handle, publisher, _dir) = node_with_temp_storage_and_recording_publisher();
    let receipt = appeal_finance_settlement_receipt_fixture();
    let expected = to_bytes(&receipt).expect("encode appeal finance settlement receipt");
    handle
        .publish_appeal_finance_settlement_receipt(receipt)
        .expect("publish appeal finance settlement receipt");
    let published = publisher.take();
    assert_eq!(published, vec![expected]);
}
#[test]
fn publish_por_governance_payloads_use_canonical_outbox_dispatch() {
    let (handle, _dir) = node_with_temp_storage();
    let publisher = Arc::new(RecordingPublisher::default());
    handle
        .try_set_governance_publisher(publisher.clone())
        .expect("register recording publisher");
    let publication = por_challenge_publication_fixture();
    let report = por_weekly_report_fixture();
    let expected_publication = to_bytes(&publication).expect("encode PoR challenge publication");
    let expected_report = to_bytes(&report).expect("encode PoR weekly report");
    handle
        .publish_por_challenge_publication(publication)
        .expect("publish PoR challenge publication");
    handle
        .publish_por_weekly_report(report)
        .expect("publish PoR weekly report");
    assert_eq!(
        publisher.take(),
        vec![expected_publication, expected_report]
    );
    assert_eq!(handle.pending_governance_publication_count(), 0);
}
#[test]
fn por_governance_payloads_remain_ordered_and_retryable_after_publish_failure() {
    let (handle, _dir) = node_with_temp_storage();
    let failing = Arc::new(FailingPublisher::default());
    handle
        .try_set_governance_publisher(failing.clone())
        .expect("register failing publisher before enqueue");
    let publication = por_challenge_publication_fixture();
    let report = por_weekly_report_fixture();
    let expected_publication = to_bytes(&publication).expect("encode PoR challenge publication");
    let expected_report = to_bytes(&report).expect("encode PoR weekly report");
    handle
        .publish_por_challenge_publication(publication)
        .expect_err("challenge publish failure remains durable");
    handle
        .publish_por_weekly_report(report)
        .expect_err("report publish failure remains durable");
    assert_eq!(failing.attempts(), 2);
    assert_eq!(handle.pending_governance_publication_count(), 2);
    let recording = Arc::new(RecordingPublisher::default());
    handle
        .try_set_governance_publisher(recording.clone())
        .expect("retry queued PoR publications");
    assert_eq!(
        recording.take(),
        vec![expected_publication, expected_report]
    );
    assert_eq!(handle.pending_governance_publication_count(), 0);
}
#[test]
fn por_ingestion_status_tracks_backlog_and_history() {
    let (handle, _dir) = node_with_temp_storage();
    let challenge = por_sample_challenge();
    handle
        .record_por_challenge(&challenge)
        .expect("record challenge");
    let initial = handle
        .por_ingestion_status(&challenge.manifest_digest)
        .expect("status before verdict");
    assert_eq!(initial.providers.len(), 1);
    assert_eq!(initial.providers[0].pending_challenges, 1);
    assert_eq!(initial.providers[0].last_success_unix, None);
    let proof = por_sample_proof(&challenge);
    handle
        .record_por_proof(&proof, &por_sample_provider_key())
        .expect("record proof succeeds");
    let verdict = por_sample_verdict(&challenge, proof.proof_digest());
    handle
        .record_por_verdict(&verdict, &por_sample_auditor_keys(), 1)
        .expect("record verdict succeeds");
    let after = handle
        .por_ingestion_status(&challenge.manifest_digest)
        .expect("status after verdict");
    assert_eq!(after.providers.len(), 1);
    let provider = &after.providers[0];
    assert_eq!(provider.pending_challenges, 0);
    assert_eq!(provider.last_success_unix, Some(verdict.decided_at));
    assert_eq!(provider.failures_total, 0);
    assert_eq!(provider.consecutive_failures, 0);
}
#[test]
fn por_authority_updates_advance_one_durable_record_and_survive_restart() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let handle = NodeHandle::new(cfg.clone());
    let challenge = por_sample_challenge();
    let initial = handle
        .por_status_authority_snapshot()
        .expect("initial authoritative projection");
    assert_eq!(initial.generation, 1);
    assert!(initial.statuses.is_empty());
    let challenge_update = handle
        .record_por_challenge_with_authority_update(&challenge)
        .expect("durably record challenge with bounded authority update");
    assert_eq!(challenge_update.generation, 2);
    assert_eq!(challenge_update.status.challenge_id, challenge.challenge_id);
    assert_eq!(
        challenge_update.status.status,
        sorafs_manifest::por::PorChallengeOutcome::AwaitingProof
    );
    assert_eq!(
        handle
            .record_por_challenge_with_authority_update(&challenge)
            .expect("exact challenge replay returns the same authority record"),
        challenge_update
    );
    let proof = por_sample_proof(&challenge);
    let proof_update = handle
        .record_por_proof_with_authority_update(&proof, &por_sample_provider_key())
        .expect("durably record proof with bounded authority update");
    assert_eq!(proof_update.generation, 3);
    assert_eq!(proof_update.status.proof_digest, Some(proof.proof_digest()));
    assert_eq!(proof_update.status.responded_at, Some(proof.submitted_at));
    let verdict = por_sample_verdict(&challenge, proof.proof_digest());
    let (_outcome, verdict_update) = handle
        .record_por_verdict_with_authority_update(&verdict, &por_sample_auditor_keys(), 1)
        .expect("durably record verdict with bounded authority update");
    assert_eq!(verdict_update.generation, 4);
    assert_eq!(
        verdict_update.status.status,
        sorafs_manifest::por::PorChallengeOutcome::Verified
    );
    let visible = handle
        .por_status_authority_snapshot()
        .expect("read complete authority after incremental mutations");
    assert_eq!(visible.generation, verdict_update.generation);
    assert_eq!(visible.statuses, vec![verdict_update.status.clone()]);
    drop(handle);
    assert_eq!(
        NodeHandle::new(cfg)
            .por_status_authority_snapshot()
            .expect("restart restores the same authoritative checkpoint"),
        visible
    );
}
#[test]
fn por_validation_failures_report_no_mutation_for_projection_preservation() {
    let (handle, _dir) = node_with_temp_storage();
    let challenge = por_sample_challenge();
    handle
        .record_por_challenge_with_authority_update(&challenge)
        .expect("record authority challenge");
    let before = handle.por_status_authority_snapshot().unwrap();
    let mut unknown = por_sample_proof(&challenge);
    unknown.challenge_id = [0xE1; 32];
    resign_por_sample_proof(&mut unknown);
    let error = handle
        .record_por_proof_with_authority_update(&unknown, &por_sample_provider_key())
        .expect_err("valid signed unknown proof is rejected before mutation");
    assert_eq!(error.disposition(), PorMutationDispositionV1::NoMutation);
    assert!(!error.disposition().invalidates_projection());
    assert_eq!(handle.por_status_authority_snapshot().unwrap(), before);
    let mut mismatched = por_sample_proof(&challenge);
    mismatched.manifest_digest = [0xE2; 32];
    resign_por_sample_proof(&mut mismatched);
    let error = handle
        .record_por_proof_with_authority_update(&mismatched, &por_sample_provider_key())
        .expect_err("valid signed mismatched proof is rejected before mutation");
    assert_eq!(error.disposition(), PorMutationDispositionV1::NoMutation);
    assert!(!error.disposition().invalidates_projection());
    assert_eq!(handle.por_status_authority_snapshot().unwrap(), before);
}
#[test]
fn por_reputation_admission_replays_without_duplicate_after_ack_checkpoint_failure() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let handle = NodeHandle::new(cfg.clone());
    let challenge = por_sample_challenge();
    let proof = por_sample_proof(&challenge);
    let verdict = por_sample_verdict(&challenge, proof.proof_digest());
    handle
        .record_por_challenge(&challenge)
        .expect("record challenge");
    handle
        .record_por_proof(&proof, &por_sample_provider_key())
        .expect("record proof");
    handle
        .record_por_verdict(&verdict, &por_sample_auditor_keys(), 1)
        .expect("finalize PoR terminal and durable work");
    assert_eq!(handle.pending_por_reputation_terminal_count(), 1);
    let checkpoint_path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let before_ack = fs::read(&checkpoint_path).expect("read pre-ack checkpoint");
    fs::remove_file(&checkpoint_path).expect("remove checkpoint to inject target failure");
    fs::create_dir(&checkpoint_path).expect("inject non-file checkpoint target");
    let admission = RecordingReputationAdmission::default();
    assert!(matches!(
        handle.reconcile_next_por_reputation_terminal(&admission),
        Err(PorReputationReconcileErrorV1::Tracker(
            PorTrackerError::RuntimeCheckpoint(_)
        ))
    ));
    assert_eq!(admission.calls.load(Ordering::Relaxed), 1);
    assert!(
        admission.retained.lock().expect("admission lock").is_some(),
        "native admission succeeded before node acknowledgement failed"
    );
    assert_eq!(
        handle.pending_por_reputation_terminal_count(),
        1,
        "failed acknowledgement checkpoint must roll the cursor back"
    );
    fs::remove_dir(&checkpoint_path).expect("remove injected checkpoint directory");
    write_local_checkpoint_atomic(&checkpoint_path, &before_ack)
        .expect("restore exact pre-ack checkpoint");
    drop(handle);
    let restored = NodeHandle::new(cfg.clone());
    let outcome = restored
        .reconcile_next_por_reputation_terminal(&admission)
        .expect("retry exact retained terminal after restart");
    assert!(matches!(
        outcome,
        PorReputationReconcileOutcomeV1::Reconciled {
            admission: reputation::runtime::ReputationJournalEnqueueOutcomeV1::ExactReplay { .. },
            acknowledgement: PorReputationTerminalAckOutcomeV1::Advanced,
            ..
        }
    ));
    assert_eq!(admission.calls.load(Ordering::Relaxed), 2);
    assert_eq!(restored.pending_por_reputation_terminal_count(), 0);
    drop(restored);
    let replay = NodeHandle::new(cfg);
    assert_eq!(replay.pending_por_reputation_terminal_count(), 0);
    assert_eq!(
        replay
            .reconcile_next_por_reputation_terminal(&admission)
            .expect("no work after durable acknowledgement"),
        PorReputationReconcileOutcomeV1::Idle
    );
    assert_eq!(
        admission.calls.load(Ordering::Relaxed),
        2,
        "durable acknowledgement prevents a duplicate native admission"
    );
}
#[test]
fn por_authority_checkpoint_prepublication_and_commit_uncertain_matrix() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let checkpoint_path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let challenge = por_sample_challenge();
    let handle = NodeHandle::new(cfg.clone());
    let initial_checkpoint = fs::read(&checkpoint_path).expect("read initial checkpoint");
    fs::remove_file(&checkpoint_path).expect("remove checkpoint for prepublication fault");
    fs::create_dir(&checkpoint_path).expect("install invalid checkpoint target");
    assert!(matches!(
        handle.record_por_challenge(&challenge),
        Err(PorTrackerError::RuntimeCheckpoint(_))
    ));
    let rolled_back = handle
        .por_status_authority_snapshot()
        .expect("read rolled-back authority");
    assert_eq!(rolled_back.generation, 1);
    assert!(rolled_back.statuses.is_empty());
    fs::remove_dir(&checkpoint_path).expect("remove invalid checkpoint target");
    write_local_checkpoint_atomic(&checkpoint_path, &initial_checkpoint)
        .expect("restore initial checkpoint");
    drop(handle);
    let uncertain = NodeHandle::new(cfg.clone());
    uncertain
        .fail_after_next_auxiliary_checkpoint_publication
        .store(true, Ordering::SeqCst);
    assert!(matches!(
        uncertain.record_por_challenge(&challenge),
        Err(PorTrackerError::RuntimeCheckpoint(_))
    ));
    let visible = uncertain
        .por_status_authority_snapshot()
        .expect("commit-uncertain state remains readable");
    assert_eq!(visible.generation, 2);
    assert_eq!(visible.statuses.len(), 1);
    assert_eq!(visible.statuses[0].challenge_id, challenge.challenge_id);
    drop(uncertain);
    let restored = NodeHandle::new(cfg);
    assert_eq!(
        restored
            .por_status_authority_snapshot()
            .expect("restore visible checkpoint"),
        visible
    );
    restored
        .record_por_challenge(&challenge)
        .expect("exact replay after commit uncertainty is side-effect free");
}
#[test]
fn por_repair_outbox_replays_enqueue_and_acknowledgement_faults() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let checkpoint_path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let handle = NodeHandle::new(cfg.clone());
    let challenge = por_sample_challenge();
    handle
        .record_por_challenge(&challenge)
        .expect("record challenge");
    let mut verdict = por_sample_verdict(&challenge, [0; 32]);
    verdict.outcome = AuditOutcomeV1::Failed;
    verdict.failure_reason = Some("deadline elapsed".to_owned());
    verdict.proof_digest = None;
    verdict.decided_at = challenge.deadline_at;
    resign_por_sample_verdict(&mut verdict);
    handle
        .record_por_verdict(&verdict, &por_sample_auditor_keys(), 1)
        .expect("commit failed verdict and repair outbox entry");
    assert!(matches!(
        handle.reconcile_next_por_repair_handoff(&FailingPorRepairHandoff),
        Err(PorRepairReconcileErrorV1::Handoff(_))
    ));
    assert!(handle.next_pending_por_repair_work().unwrap().is_some());
    let before_ack = fs::read(&checkpoint_path).expect("read pending-repair checkpoint");
    fs::remove_file(&checkpoint_path).expect("remove checkpoint for ack failure");
    fs::create_dir(&checkpoint_path).expect("install invalid checkpoint target");
    assert!(matches!(
        handle.reconcile_next_por_repair_handoff(&SuccessfulPorRepairHandoff),
        Err(PorRepairReconcileErrorV1::Tracker(
            PorTrackerError::RuntimeCheckpoint(_)
        ))
    ));
    assert!(handle.next_pending_por_repair_work().unwrap().is_some());
    fs::remove_dir(&checkpoint_path).expect("remove invalid checkpoint target");
    write_local_checkpoint_atomic(&checkpoint_path, &before_ack)
        .expect("restore pending-repair checkpoint");
    drop(handle);
    let restored = NodeHandle::new(cfg.clone());
    assert!(matches!(
        restored
            .reconcile_next_por_repair_handoff(&SuccessfulPorRepairHandoff)
            .expect("replay exact repair admission and acknowledge"),
        PorRepairReconcileOutcomeV1::Reconciled {
            acknowledgement: PorRepairHandoffAckOutcomeV1::Advanced,
            ..
        }
    ));
    assert!(restored.next_pending_por_repair_work().unwrap().is_none());
    drop(restored);
    let replay = NodeHandle::new(cfg);
    assert_eq!(
        replay
            .reconcile_next_por_repair_handoff(&SuccessfulPorRepairHandoff)
            .expect("durable repair acknowledgement survives restart"),
        PorRepairReconcileOutcomeV1::Idle
    );
}
#[test]
fn por_ingestion_status_tracks_failures() {
    let (handle, _dir) = node_with_temp_storage();
    let challenge = por_sample_challenge();
    handle
        .record_por_challenge(&challenge)
        .expect("record challenge");
    let mut verdict = por_sample_verdict(&challenge, [0; 32]);
    verdict.outcome = AuditOutcomeV1::Failed;
    verdict.failure_reason = Some("timeout".to_string());
    verdict.proof_digest = None;
    verdict.decided_at = challenge.deadline_at;
    resign_por_sample_verdict(&mut verdict);
    handle
        .record_por_verdict(&verdict, &por_sample_auditor_keys(), 1)
        .expect("record failure verdict");
    let status = handle
        .por_ingestion_status(&challenge.manifest_digest)
        .expect("status after failure");
    assert_eq!(status.providers.len(), 1);
    let provider = &status.providers[0];
    assert_eq!(provider.pending_challenges, 0);
    assert_eq!(provider.failures_total, 1);
    assert_eq!(provider.consecutive_failures, 1);
    assert_eq!(provider.last_failure_unix, Some(verdict.decided_at));
    assert!(provider.last_success_unix.is_none());
}
#[test]
fn por_history_rolls_oldest_unprotected_entry_and_preserves_live_lifecycle_keys() {
    let (base, _dir) = storage_config_with_temp_dir();
    let cfg = enabled_storage_builder(base.data_dir().clone())
        .runtime_retention(RuntimeRetentionPolicy::new(1, 1, 1024 * 1024))
        .build();
    let handle = NodeHandle::new(cfg);
    let challenge = por_sample_challenge();
    let first = por_sample_verdict(&challenge, [0x44; 32]);
    handle
        .update_por_history_entry(&first)
        .expect("first history key fits");
    let mut replacement = first.clone();
    replacement.manifest_digest = [0x91; 32];
    replacement.provider_id = [0x92; 32];
    replacement.decided_at = replacement.decided_at.saturating_add(1);
    handle
        .update_por_history_entry(&replacement)
        .expect("oldest unprotected history key rolls out");
    let history = handle.por_history.read().expect("history lock");
    assert_eq!(history.len(), 1);
    assert!(history.contains_key(&(replacement.manifest_digest, replacement.provider_id)));
    drop(history);
    let (base, _dir) = storage_config_with_temp_dir();
    let cfg = enabled_storage_builder(base.data_dir().clone())
        .runtime_retention(RuntimeRetentionPolicy::new(1, 1, 1024 * 1024))
        .build();
    let protected = NodeHandle::new(cfg);
    protected
        .record_por_challenge(&challenge)
        .expect("retain live lifecycle key");
    protected
        .update_por_history_entry(&first)
        .expect("record protected history key");
    assert!(matches!(
        protected.update_por_history_entry(&replacement),
        Err(PorTrackerError::HistoryRetentionExhausted { limit: 1 })
    ));
    let history = protected.por_history.read().expect("history lock");
    assert_eq!(history.len(), 1);
    assert!(history.contains_key(&(challenge.manifest_digest, challenge.provider_id)));
}
#[test]
fn por_ingestion_overview_reports_pending_and_failures() {
    let (handle, _dir) = node_with_temp_storage();
    let challenge = por_sample_challenge();
    handle
        .record_por_challenge(&challenge)
        .expect("record challenge");
    let overview = handle.por_ingestion_overview();
    assert_eq!(overview.len(), 1);
    assert_eq!(overview[0].pending_challenges, 1);
    assert_eq!(overview[0].failures_total, 0);
    let mut verdict = por_sample_verdict(&challenge, [0; 32]);
    verdict.outcome = AuditOutcomeV1::Failed;
    verdict.failure_reason = Some("missed".to_string());
    verdict.proof_digest = None;
    verdict.decided_at = challenge.deadline_at;
    resign_por_sample_verdict(&mut verdict);
    handle
        .record_por_verdict(&verdict, &por_sample_auditor_keys(), 1)
        .expect("record failure verdict");
    let overview_after = handle.por_ingestion_overview();
    assert_eq!(overview_after.len(), 1);
    assert_eq!(overview_after[0].pending_challenges, 0);
    assert_eq!(overview_after[0].failures_total, 1);
    assert_eq!(
        overview_after[0].last_failure_unix,
        Some(verdict.decided_at)
    );
}
#[test]
fn node_handle_gc_evicts_expired_manifest_and_publishes_audit() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let gc_actual = iroha_config::parameters::actual::SorafsGc {
        enabled: true,
        retention_grace_secs: 0,
        max_deletions_per_run: 10,
        ..Default::default()
    };
    let handle =
        NodeHandle::new_with_policies(cfg, RepairConfig::default(), GcConfig::from(&gc_actual));
    let publisher = Arc::new(RecordingPublisher::default());
    let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
    handle.set_governance_publisher(trait_publisher);
    let declaration = record_capacity_declaration_fixture(&handle, [0xAB; 32], [0xAA; 32]);
    let payload = b"gc-expired-payload";
    let plan = CarBuildPlan::single_file(payload).expect("plan");
    let retention_epoch = 1_700_000_000;
    let now_unix = retention_epoch + 10;
    let mut policy = PinPolicy::default();
    policy.retention_epoch = retention_epoch;
    let manifest = manifest_builder_for_plan(payload, &plan)
        .pin_policy(policy)
        .build()
        .expect("manifest");
    let mut reader = payload.as_slice();
    let manifest_id = handle
        .ingest_manifest(&manifest, &plan, &mut reader)
        .expect("ingest manifest");
    let manifest_digest: [u8; 32] = manifest.digest().expect("digest").into();
    let report = run_test_gc(&handle, now_unix, &empty_finalized_repair_projection());
    assert_eq!(report.evictions.len(), 1);
    assert_eq!(report.freed_bytes, plan.content_length);
    assert!(handle.manifest_metadata(&manifest_id).is_err());
    let payloads = publisher.take();
    let mut gc_events = Vec::new();
    for payload in payloads {
        if let Ok(event) = norito::decode_from_bytes::<GcAuditEventV1>(&payload) {
            gc_events.push(event);
        }
    }
    assert_eq!(gc_events.len(), 1);
    let event = &gc_events[0];
    assert_eq!(event.version, GC_AUDIT_EVENT_VERSION_V1);
    assert_eq!(event.payload.version, GC_AUDIT_PAYLOAD_VERSION_V1);
    assert_eq!(event.payload.manifest_digest, manifest_digest);
    assert_eq!(event.payload.provider_id, declaration.provider_id);
    assert_eq!(event.payload.freed_bytes, plan.content_length);
    assert!(event.payload.blocked_reason.is_none());
}
#[test]
fn gc_eviction_transaction_prepare_checkpoint_failure_prevents_domain_commit() {
    let (cfg, handle, _dir) = gc_node_with_temp_storage();
    let now_unix = 1_710_000_050;
    let (_, manifest_id) =
        expired_gc_manifest_fixture(&handle, 0x70, now_unix, b"gc-prepare-checkpoint-failure");
    ensure_test_capacity_provider(&handle);
    let checkpoint_path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let committed = fs::read(&checkpoint_path).expect("read committed auxiliary checkpoint");
    fs::remove_file(&checkpoint_path).expect("remove auxiliary checkpoint");
    fs::create_dir(&checkpoint_path).expect("inject auxiliary checkpoint directory");
    let report = run_test_gc(&handle, now_unix, &empty_finalized_repair_projection());
    assert!(report.evictions.is_empty());
    assert_eq!(report.errors, 1);
    assert!(handle.manifest_metadata(&manifest_id).is_ok());
    assert_eq!(handle.storage.as_ref().unwrap().gc_counters(), (0, 0));
    assert_eq!(handle.pending_governance_publication_count(), 0);
    assert!(gc_runtime_read!(handle.gc_eviction_intents, "intent lock" => entries.is_empty()));
    assert!(handle.durability_failure_reason().is_none());
    fs::remove_dir(&checkpoint_path).expect("remove injected checkpoint directory");
    write_local_checkpoint_atomic(&checkpoint_path, &committed)
        .expect("restore committed checkpoint");
    drop(handle);
    let restored =
        NodeHandle::new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1));
    assert!(restored.manifest_metadata(&manifest_id).is_ok());
    assert_eq!(restored.pending_governance_publication_count(), 0);
}
#[test]
fn gc_eviction_transaction_discards_pre_domain_crash_intent() {
    let (cfg, handle, _dir) = gc_node_with_temp_storage();
    let now_unix = 1_710_000_100;
    let (_, manifest_id) =
        expired_gc_manifest_fixture(&handle, 0x71, now_unix, b"gc-pre-domain-crash");
    let storage = handle.storage.as_ref().expect("storage backend");
    let target = storage.manifest(&manifest_id).expect("GC target");
    {
        let gc_guard = handle.gc_mutation_lock.lock().expect("GC mutation lock");
        let drain_guard = handle
            .governance_outbox_drain_lock
            .lock()
            .expect("outbox drain lock");
        let intent = handle
            .prepare_gc_eviction_intent(
                (&gc_guard, &drain_guard),
                storage,
                &target,
                [0; 32],
                now_unix,
                GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1,
            )
            .expect("persist GC eviction intent");
        assert_eq!(intent.reserved_outbox_slots, 1);
    }
    assert_eq!(
        gc_runtime_read!(handle.gc_eviction_intents, "intent lock" => entries.len()),
        1
    );
    assert_eq!(handle.pending_governance_publication_count(), 0);
    assert!(storage.manifest(&manifest_id).is_some());
    drop(target);
    drop(handle);
    let restored =
        NodeHandle::new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1));
    assert!(restored.manifest_metadata(&manifest_id).is_ok());
    assert_eq!(restored.storage.as_ref().unwrap().gc_counters(), (0, 0));
    assert_eq!(restored.pending_governance_publication_count(), 0);
    assert!(
        gc_runtime_read!(restored.gc_eviction_intents, "restored intent lock" => entries.is_empty())
    );
    assert!(gc_runtime_read!(restored.gc_eviction_audit_links, "restored link lock" => is_empty()));
}
#[test]
fn gc_eviction_transaction_fail_closes_storage_generation_drift() {
    let (cfg, handle, _dir) = gc_node_with_temp_storage();
    let now_unix = 1_710_000_150;
    let (_, manifest_id) =
        expired_gc_manifest_fixture(&handle, 0x7B, now_unix, b"gc-generation-drift-target");
    let storage = handle.storage.as_ref().expect("storage backend");
    let target = storage.manifest(&manifest_id).expect("GC target");
    let gc_guard = handle.gc_mutation_lock.lock().expect("GC mutation lock");
    let drain_guard = handle
        .governance_outbox_drain_lock
        .lock()
        .expect("outbox drain lock");
    let intent = handle
        .prepare_gc_eviction_intent(
            (&gc_guard, &drain_guard),
            storage,
            &target,
            [0; 32],
            now_unix,
            GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1,
        )
        .expect("persist GC eviction intent");
    build_manifest_with_retention(
        vec![0x7C; 8],
        now_unix + 100,
        b"gc-generation-drift-interloper",
        &handle,
    );
    let error = handle
        .settle_gc_eviction_intent_against_storage(&gc_guard, &drain_guard, storage, &intent, true)
        .expect_err("unexpected storage generation must fail closed");
    assert!(error.to_string().contains("storage generation drift"));
    assert!(handle.durability_failure_reason().is_some());
    assert!(storage.manifest(&manifest_id).is_some());
    assert_eq!(storage.gc_counters(), (0, 0));
    assert_eq!(handle.pending_governance_publication_count(), 0);
    assert_eq!(
        gc_runtime_read!(handle.gc_eviction_intents, "intent lock" => entries.len()),
        1
    );
    drop(drain_guard);
    drop(gc_guard);
    drop(target);
    drop(handle);
    let error =
        NodeHandle::try_new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1))
            .expect_err("ambiguous GC generation must also fail startup");
    assert!(error.to_string().contains("storage generation drift"));
}
#[test]
fn gc_eviction_transaction_recovers_post_domain_crash_exactly_once() {
    let (cfg, handle, _dir) = gc_node_with_temp_storage();
    let now_unix = 1_710_000_200;
    let payload = b"gc-post-domain-crash";
    let (digest, manifest_id) = expired_gc_manifest_fixture(&handle, 0x72, now_unix, payload);
    let storage = handle.storage.as_ref().expect("storage backend");
    let target = storage.manifest(&manifest_id).expect("GC target");
    {
        let gc_guard = handle.gc_mutation_lock.lock().expect("GC mutation lock");
        let drain_guard = handle
            .governance_outbox_drain_lock
            .lock()
            .expect("outbox drain lock");
        handle
            .prepare_gc_eviction_intent(
                (&gc_guard, &drain_guard),
                storage,
                &target,
                [0; 32],
                now_unix,
                GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1,
            )
            .expect("persist GC eviction intent");
        assert_eq!(
            storage
                .evict_manifest(&manifest_id)
                .expect("commit storage eviction"),
            u64::try_from(payload.len()).unwrap()
        );
    }
    assert_eq!(storage.gc_counters(), (payload.len() as u64, 1));
    assert_eq!(handle.pending_governance_publication_count(), 0);
    drop(target);
    drop(handle);
    let restored =
        NodeHandle::new_with_policies(cfg.clone(), RepairConfig::default(), enabled_gc_config(1));
    assert!(restored.manifest_metadata(&manifest_id).is_err());
    assert_eq!(restored.pending_governance_publication_count(), 1);
    assert_eq!(
        gc_runtime_read!(restored.gc_eviction_audit_links, "link lock" => len()),
        1
    );
    assert!(gc_runtime_read!(restored.gc_eviction_intents, "intent lock" => entries.is_empty()));
    let publisher = Arc::new(RecordingPublisher::default());
    restored
        .try_set_governance_publisher(publisher.clone())
        .expect("publish recovered GC audit");
    let published = publisher.take();
    assert_eq!(published.len(), 1);
    let audit: GcAuditEventV1 =
        norito::decode_from_bytes(&published[0]).expect("decode recovered GC audit");
    audit.validate().expect("recovered audit validates");
    assert_eq!(audit.payload.manifest_digest, digest);
    assert_eq!(audit.payload.freed_bytes, payload.len() as u64);
    drop(restored);
    let acknowledged =
        NodeHandle::new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1));
    assert_eq!(acknowledged.pending_governance_publication_count(), 0);
    assert_eq!(
        acknowledged.storage.as_ref().unwrap().gc_counters(),
        (payload.len() as u64, 1)
    );
    assert_eq!(
        gc_runtime_read!(acknowledged.gc_eviction_audit_links, "acknowledged link lock" => len()),
        1
    );
}
#[test]
fn gc_eviction_transaction_finalization_checkpoint_failure_fail_stops_and_recovers() {
    let (cfg, handle, _dir) = gc_node_with_temp_storage();
    let now_unix = 1_710_000_250;
    let payload = b"gc-finalization-checkpoint-failure";
    let (_, manifest_id) = expired_gc_manifest_fixture(&handle, 0x7A, now_unix, payload);
    let storage = handle.storage.as_ref().expect("storage backend");
    let target = storage.manifest(&manifest_id).expect("GC target");
    let gc_guard = handle.gc_mutation_lock.lock().expect("GC mutation lock");
    let drain_guard = handle
        .governance_outbox_drain_lock
        .lock()
        .expect("outbox drain lock");
    let intent = handle
        .prepare_gc_eviction_intent(
            (&gc_guard, &drain_guard),
            storage,
            &target,
            [0; 32],
            now_unix,
            GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1,
        )
        .expect("persist GC eviction intent");
    let checkpoint_path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let prepared = fs::read(&checkpoint_path).expect("read prepared GC checkpoint");
    assert_eq!(
        storage
            .evict_manifest(&manifest_id)
            .expect("commit storage eviction"),
        payload.len() as u64
    );
    fs::remove_file(&checkpoint_path).expect("remove prepared auxiliary checkpoint");
    fs::create_dir(&checkpoint_path).expect("inject auxiliary checkpoint directory");
    let error = handle
        .settle_gc_eviction_intent_against_storage(&gc_guard, &drain_guard, storage, &intent, true)
        .expect_err("GC publication checkpoint failure must surface");
    assert!(
        error
            .to_string()
            .contains("storage state may have committed without its audit publication")
    );
    assert!(handle.durability_failure_reason().is_some());
    assert_eq!(handle.pending_governance_publication_count(), 0);
    assert_eq!(
        gc_runtime_read!(handle.gc_eviction_intents, "rolled-back intent lock" => entries.len()),
        1
    );
    assert!(
        gc_runtime_read!(handle.gc_eviction_audit_links, "rolled-back link lock" => is_empty())
    );
    drop(drain_guard);
    drop(gc_guard);
    let publisher = Arc::new(RecordingPublisher::default());
    assert!(
        handle
            .try_set_governance_publisher(publisher.clone())
            .is_err()
    );
    assert!(publisher.take().is_empty());
    drop(target);
    fs::remove_dir(&checkpoint_path).expect("remove injected checkpoint directory");
    write_local_checkpoint_atomic(&checkpoint_path, &prepared)
        .expect("restore prepared GC checkpoint");
    drop(handle);
    let restored =
        NodeHandle::new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1));
    assert!(restored.manifest_metadata(&manifest_id).is_err());
    assert_eq!(restored.storage.as_ref().unwrap().gc_counters().1, 1);
    assert_eq!(restored.pending_governance_publication_count(), 1);
    assert!(
        gc_runtime_read!(restored.gc_eviction_intents, "restored intent lock" => entries.is_empty())
    );
    assert_eq!(
        gc_runtime_read!(restored.gc_eviction_audit_links, "restored link lock" => len()),
        1
    );
}
#[test]
fn gc_eviction_transaction_reservation_survives_full_outbox_restart() {
    let (base, _dir) = storage_config_with_temp_dir();
    let cfg = enabled_storage_builder(base.data_dir().clone())
        .runtime_retention(RuntimeRetentionPolicy::new(1, 2, 2 * 1024 * 1024))
        .build();
    let handle =
        NodeHandle::new_with_policies(cfg.clone(), RepairConfig::default(), enabled_gc_config(1));
    let now_unix = 1_710_000_300;
    let (_, manifest_id) =
        expired_gc_manifest_fixture(&handle, 0x73, now_unix, b"gc-full-outbox-restart");
    let issuance = proof_token_issuance_fixture();
    handle
        .publish_proof_token_issuance(issuance)
        .expect("occupy first outbox slot");
    let storage = handle.storage.as_ref().expect("storage backend");
    let target = storage.manifest(&manifest_id).expect("GC target");
    {
        let gc_guard = handle.gc_mutation_lock.lock().expect("GC mutation lock");
        let drain_guard = handle
            .governance_outbox_drain_lock
            .lock()
            .expect("outbox drain lock");
        handle
            .prepare_gc_eviction_intent(
                (&gc_guard, &drain_guard),
                storage,
                &target,
                [0; 32],
                now_unix,
                GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1,
            )
            .expect("reserve final outbox slot");
        let mut competing = proof_token_issuance_fixture();
        competing.token_id = [0xD1; 16];
        competing.token_blake3 = [0xD2; 32];
        let error = handle
            .enqueue_governance_outbox(
                GovernanceOutboxKindV1::ProofTokenIssuance,
                norito::to_bytes(&competing).expect("encode competing publication"),
            )
            .expect_err("competing publication cannot consume GC reservation");
        assert!(error.to_string().contains("slots are reserved"));
        storage
            .evict_manifest(&manifest_id)
            .expect("commit storage eviction");
    }
    assert_eq!(handle.pending_governance_publication_count(), 1);
    drop(target);
    drop(handle);
    let restored =
        NodeHandle::new_with_policies(cfg.clone(), RepairConfig::default(), enabled_gc_config(1));
    assert_eq!(restored.pending_governance_publication_count(), 2);
    assert!(
        gc_runtime_read!(restored.gc_eviction_intents, "restored intent lock" => entries.is_empty())
    );
    let publisher = Arc::new(RecordingPublisher::default());
    restored
        .try_set_governance_publisher(publisher.clone())
        .expect("drain full recovered outbox");
    assert_eq!(publisher.take().len(), 2);
    drop(restored);
    let acknowledged =
        NodeHandle::new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1));
    assert_eq!(acknowledged.pending_governance_publication_count(), 0);
    assert_eq!(acknowledged.storage.as_ref().unwrap().gc_counters().1, 1);
}
#[test]
fn gc_eviction_transaction_rejects_intent_binding_and_counter_tampering() {
    for tamper_counter in [false, true] {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new_with_policies(
            cfg.clone(),
            RepairConfig::default(),
            enabled_gc_config(1),
        );
        let now_unix = if tamper_counter {
            1_710_000_401
        } else {
            1_710_000_400
        };
        let digest = build_manifest_with_retention(
            vec![if tamper_counter { 0x75 } else { 0x74 }; 8],
            now_unix - 1,
            if tamper_counter {
                b"gc-counter-tamper".as_slice()
            } else {
                b"gc-binding-tamper".as_slice()
            },
            &handle,
        );
        let manifest_id = hex::encode(digest);
        let storage = handle.storage.as_ref().expect("storage backend");
        let target = storage.manifest(&manifest_id).expect("GC target");
        {
            let gc_guard = handle.gc_mutation_lock.lock().expect("GC mutation lock");
            let drain_guard = handle
                .governance_outbox_drain_lock
                .lock()
                .expect("outbox drain lock");
            handle
                .prepare_gc_eviction_intent(
                    (&gc_guard, &drain_guard),
                    storage,
                    &target,
                    [0; 32],
                    now_unix,
                    GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1,
                )
                .expect("persist GC intent");
        }
        let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
        let bytes = fs::read(&path).expect("read GC intent checkpoint");
        let mut checkpoint: AuxiliaryRuntimeCheckpointV5 =
            norito::decode_from_bytes(&bytes).expect("decode GC intent checkpoint");
        let intent = checkpoint
            .gc_eviction_intents
            .first_mut()
            .expect("GC intent");
        if tamper_counter {
            intent.storage_after.gc_evictions_total =
                intent.storage_after.gc_evictions_total.saturating_add(1);
            intent.binding_digest =
                gc_eviction_intent_binding_digest(intent).expect("rebind forged intent");
        } else {
            intent.binding_digest[0] ^= 0x80;
        }
        write_local_checkpoint_atomic(
            &path,
            &norito::to_bytes(&checkpoint).expect("encode tampered GC checkpoint"),
        )
        .expect("write tampered GC checkpoint");
        drop(target);
        drop(handle);
        let error =
            NodeHandle::try_new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1))
                .expect_err("tampered GC intent must fail startup");
        let message = error.to_string();
        assert!(
            message.contains("binding digest mismatch") || message.contains("one exact eviction"),
            "unexpected error: {message}"
        );
    }
}
#[test]
fn gc_eviction_transaction_rejects_acknowledged_link_counter_tampering() {
    let (cfg, handle, _dir) = gc_node_with_temp_storage();
    let now_unix = 1_710_000_500;
    build_manifest_with_retention(
        vec![0x76; 8],
        now_unix - 1,
        b"gc-link-counter-tamper",
        &handle,
    );
    let publisher = Arc::new(RecordingPublisher::default());
    handle
        .try_set_governance_publisher(publisher)
        .expect("install publisher");
    let report = run_test_gc(&handle, now_unix, &empty_finalized_repair_projection());
    assert_eq!(report.evictions.len(), 1);
    assert_eq!(handle.pending_governance_publication_count(), 0);
    let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let bytes = fs::read(&path).expect("read linked GC checkpoint");
    let mut checkpoint: AuxiliaryRuntimeCheckpointV5 =
        norito::decode_from_bytes(&bytes).expect("decode linked GC checkpoint");
    let link = checkpoint
        .gc_eviction_audit_links
        .first_mut()
        .expect("GC audit link");
    link.storage_gc_evictions_total = link.storage_gc_evictions_total.saturating_add(1);
    link.binding_digest = gc_eviction_audit_link_binding_digest(link);
    write_local_checkpoint_atomic(
        &path,
        &norito::to_bytes(&checkpoint).expect("encode forged GC link checkpoint"),
    )
    .expect("write forged GC link checkpoint");
    drop(handle);
    let error =
        NodeHandle::try_new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1))
            .expect_err("forged GC storage counter linkage must fail startup");
    assert!(error.to_string().contains("storage counter generation"));
}
#[test]
fn gc_blocks_shared_chunks_with_zero_byte_audit() {
    let (_cfg, handle, _dir) = gc_node_with_temp_storage();
    let now_unix = 1_710_000_600;
    let payload = b"gc-shared-chunk-zero-byte-audit";
    build_manifest_with_retention(vec![0x76; 8], now_unix + 60, payload, &handle);
    build_manifest_with_retention(vec![0x77; 8], now_unix - 1, payload, &handle);
    let report = run_test_gc(&handle, now_unix, &empty_finalized_repair_projection());
    assert_eq!(report.errors, 0);
    assert!(report.evictions.is_empty());
    assert_eq!(report.freed_bytes, 0);
    assert!(
        report
            .skipped
            .iter()
            .any(|skip| skip.reason == GC_AUDIT_BLOCKED_SHARED_CHUNKS_V1)
    );
    assert_eq!(handle.storage.as_ref().unwrap().gc_counters(), (0, 0));
    let outbox = handle.governance_outbox.read().expect("outbox lock");
    let entry = outbox.entries.values().next().expect("GC audit entry");
    let audit: GcAuditEventV1 =
        norito::decode_from_bytes(&entry.payload_bytes).expect("decode zero-byte audit");
    audit.validate().expect("zero-byte GC audit validates");
    assert_eq!(audit.payload.freed_bytes, 0);
    assert_eq!(
        audit.payload.blocked_reason.as_deref(),
        Some(GC_AUDIT_BLOCKED_SHARED_CHUNKS_V1)
    );
}
#[test]
fn gc_eviction_transaction_publisher_failure_retries_durable_audit() {
    let (cfg, handle, _dir) = gc_node_with_temp_storage();
    let now_unix = 1_710_000_700;
    build_manifest_with_retention(vec![0x78; 8], now_unix - 1, b"gc-publisher-retry", &handle);
    let failing = Arc::new(FailingPublisher::default());
    handle
        .try_set_governance_publisher(failing.clone())
        .expect("install initially idle failing publisher");
    let report = run_test_gc(&handle, now_unix, &empty_finalized_repair_projection());
    assert_eq!(report.evictions.len(), 1);
    assert_eq!(report.errors, 1);
    assert!(failing.attempts() >= 1);
    assert_eq!(handle.pending_governance_publication_count(), 1);
    handle.clear_governance_publisher();
    let recording = Arc::new(RecordingPublisher::default());
    handle
        .try_set_governance_publisher(recording.clone())
        .expect("retry durable GC audit");
    assert_eq!(recording.take().len(), 1);
    assert_eq!(handle.pending_governance_publication_count(), 0);
    drop(handle);
    let restored =
        NodeHandle::new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1));
    assert_eq!(restored.storage.as_ref().unwrap().gc_counters().1, 1);
    assert_eq!(
        gc_runtime_read!(restored.gc_eviction_audit_links, "restored link lock" => len()),
        1
    );
}
#[test]
fn gc_eviction_transaction_serializes_concurrent_sweeps() {
    let (_cfg, handle, _dir) = gc_node_with_temp_storage();
    let now_unix = 1_710_000_800;
    for index in 0_u8..4 {
        let payload = vec![0x80 + index; 32];
        build_manifest_with_retention(vec![0x80 + index; 8], now_unix - 1, &payload, &handle);
    }
    let barrier = Arc::new(Barrier::new(8));
    let workers = (0..8)
        .map(|_| {
            let handle = handle.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                run_test_gc(&handle, now_unix, &empty_finalized_repair_projection())
            })
        })
        .collect::<Vec<_>>();
    let reports = workers
        .into_iter()
        .map(|worker| worker.join().expect("GC worker joins"))
        .collect::<Vec<_>>();
    assert_eq!(
        reports
            .iter()
            .map(|report| report.evictions.len())
            .sum::<usize>(),
        4
    );
    assert_eq!(reports.iter().map(|report| report.errors).sum::<u32>(), 0);
    let storage = handle.storage.as_ref().expect("storage backend");
    assert_eq!(storage.manifest_count(), 0);
    assert_eq!(storage.gc_counters(), (128, 4));
    assert_eq!(handle.pending_governance_publication_count(), 4);
    assert_eq!(
        gc_runtime_read!(handle.gc_eviction_audit_links, "link lock" => len()),
        4
    );
}
#[test]
fn gc_blocked_audit_full_outbox_is_reported_without_eviction() {
    let (base, _dir) = storage_config_with_temp_dir();
    let cfg = enabled_storage_builder(base.data_dir().clone())
        .runtime_retention(RuntimeRetentionPolicy::new(1, 1, 2 * 1024 * 1024))
        .build();
    let handle = NodeHandle::new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1));
    handle
        .publish_proof_token_issuance(proof_token_issuance_fixture())
        .expect("fill governance outbox");
    let now_unix = 1_710_000_900;
    let payload = b"gc-blocked-full-outbox";
    let first = build_manifest_with_retention(vec![0x91; 8], now_unix - 1, payload, &handle);
    let second = build_manifest_with_retention(vec![0x92; 8], now_unix - 1, payload, &handle);
    let report = run_test_gc(&handle, now_unix, &empty_finalized_repair_projection());
    assert!(report.evictions.is_empty());
    assert_eq!(report.errors, 1);
    assert!(
        report
            .skipped
            .iter()
            .any(|skip| skip.reason == GC_AUDIT_BLOCKED_SHARED_CHUNKS_V1)
    );
    assert!(handle.manifest_metadata(&hex::encode(first)).is_ok());
    assert!(handle.manifest_metadata(&hex::encode(second)).is_ok());
    assert_eq!(handle.storage.as_ref().unwrap().gc_counters(), (0, 0));
    assert_eq!(handle.pending_governance_publication_count(), 1);
}
#[test]
fn node_handle_reconciliation_emits_report() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let handle = NodeHandle::new_with_policies(cfg, enabled_repair_config(1), GcConfig::default());
    let publisher = Arc::new(RecordingPublisher::default());
    let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
    handle.set_governance_publisher(trait_publisher);
    let declaration = record_capacity_declaration_fixture(&handle, [0x11; 32], [0x22; 32]);
    let payload = b"reconciliation-payload";
    let plan = CarBuildPlan::single_file(payload).expect("plan");
    let mut policy = PinPolicy::default();
    policy.retention_epoch = 1_700_000_000;
    let manifest = manifest_builder_for_plan(payload, &plan)
        .pin_policy(policy)
        .build()
        .expect("manifest");
    let manifest_digest: [u8; 32] = manifest.digest().expect("digest").into();
    let mut reader = payload.as_slice();
    handle
        .ingest_manifest(&manifest, &plan, &mut reader)
        .expect("ingest manifest");
    let repair_projection = finalized_repair_projection(vec![active_native_repair_task(
        manifest_digest,
        declaration.provider_id,
    )]);
    publisher.take();
    let now_unix = 1_700_000_200;
    let reconciliation = handle
        .run_reconciliation_once(now_unix, &repair_projection)
        .expect("reconciliation report");
    assert_eq!(
        reconciliation.version,
        SORAFS_RECONCILIATION_REPORT_VERSION_V1
    );
    assert_eq!(reconciliation.provider_id, declaration.provider_id);
    assert_eq!(reconciliation.generated_at_unix, now_unix);
    assert_eq!(reconciliation.repair_task_count, 1);
    assert_eq!(reconciliation.retention_manifest_count, 1);
    assert_eq!(reconciliation.gc_evictions_total, 0);
    assert_eq!(reconciliation.gc_freed_bytes_total, 0);
    assert_eq!(reconciliation.divergence_count, 0);
    assert!(reconciliation.appeal_finance.is_none());
    let payloads = publisher.take();
    let decoded = payloads
        .iter()
        .find_map(|payload| norito::decode_from_bytes::<SorafsReconciliationReportV1>(payload).ok())
        .expect("reconciliation payload");
    assert_eq!(decoded, reconciliation);
    let reconciliation_again = handle
        .run_reconciliation_once(now_unix, &repair_projection)
        .expect("reconciliation report");
    assert_eq!(
        reconciliation_again.repair_snapshot_hash,
        reconciliation.repair_snapshot_hash
    );
    assert_eq!(
        reconciliation_again.retention_snapshot_hash,
        reconciliation.retention_snapshot_hash
    );
    assert_eq!(
        reconciliation_again.gc_snapshot_hash,
        reconciliation.gc_snapshot_hash
    );
}
include!("lib/reconciliation_and_repair_tests.rs");
include!("lib/storage_disabled_test.rs");
include!("lib/governance_dag_test_support.rs");
