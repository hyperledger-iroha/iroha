use std::{
    fmt,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use ed25519_dalek::VerifyingKey as DalekVerifyingKey;
use hex::ToHex;
use iroha_crypto::{Algorithm, PublicKey, Signature as IrohaSignature};
use norito::json::{self, Map as JsonMap, Value as JsonValue};
use reqwest::RequestBuilder;
use sorafs_car::{CarBuildPlan, CarWriter, FileEntry};
use sorafs_manifest::{
    GOVERNANCE_DAG_BLOCK_VERSION_V1, GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1,
    GOVERNANCE_DAG_HEAD_VERSION_V1, GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1,
    GOVERNANCE_LOG_VERSION_V1, GovernanceDagBlockV1, GovernanceDagHeadV1,
    GovernanceExternalPayloadV1, GovernanceLogNodeV1, GovernanceLogPayloadV1,
    GovernanceLogSignatureV1, GovernanceSignatureAlgorithm, ModerationLedgerCyclePublicationV1,
    PROOF_TOKEN_ISSUANCE_VERSION_V1, ProofTokenIssuanceV1, SignedReputationSnapshotV1,
    SoraFsAppealFinanceReportV1, SoraFsAppealFinanceSettlementReceiptV1,
    SoraFsAppealFinanceWeeklyRollupV1, SoraFsModerationBallotGovernanceEventV1,
    SorafsReconciliationReportV1,
    deal::{DealSettlementStatusV1, DealSettlementV1},
    governance_dag_block_cid_v1,
    por::{PorChallengePublicationV1, PorWeeklyReportV1},
    repair::GcAuditEventV1,
};

use crate::{
    GovernancePublishError, GovernancePublisher, PdpGovernanceArchiveV1, PdpRejectionReasonV1,
    PdpTerminalDecisionV1,
};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const GOVERNANCE_DAG_SINK_FILESYSTEM: &str = "filesystem";
const GOVERNANCE_PUBLISH_INDEX_FILE: &str = "publish-index.json";
const GOVERNANCE_PUBLISH_INDEX_SCHEMA: &str = "sorafs.governance_dag.local_publish_index.v1";
const GOVERNANCE_CAR_QUEUE_FILE: &str = "car-queue.json";
const GOVERNANCE_CAR_QUEUE_SCHEMA: &str = "sorafs.governance_dag.local_car_queue.v1";
const GOVERNANCE_CAR_SEGMENT_SCHEMA: &str = "sorafs.governance_dag.local_car_segment.v1";
const GOVERNANCE_CAR_PLAN_SCHEMA: &str = "sorafs.governance_dag.local_car_plan.v1";
const GOVERNANCE_CAR_SEGMENTS_DIR: &str = "car-segments";
const GOVERNANCE_RUNTIME_DAG_INDEX_FILE: &str = "runtime-dag-index.json";
const GOVERNANCE_RUNTIME_DAG_INDEX_SCHEMA: &str = "sorafs.governance_dag.runtime_signed_index.v1";
const GOVERNANCE_RUNTIME_DAG_DIR: &str = "runtime-dag";
const GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR: &str = "blocks";
const GOVERNANCE_RUNTIME_DAG_HEAD_FILE: &str = "head.to";
const GOVERNANCE_PUBLISHER_LOCK_FILE: &str = ".governance-publisher.lock";
const GOVERNANCE_MUTABLE_INDEX_MAX_BYTES: usize = 64 * 1024 * 1024;
const GOVERNANCE_RUNTIME_HANDLE_MAX_BYTES: usize = 256;

/// Runtime-only signing boundary for the local Governance DAG publisher.
///
/// Production implementations are expected to delegate to PKCS#11, an HSM, or
/// a managed signing service. Private key bytes must never be returned to the
/// caller, persisted below the publisher root, or sourced from
/// [`iroha_config`](iroha_config).
pub trait GovernanceDagRuntimeSigner: Send + Sync + fmt::Debug {
    /// Opaque, non-secret deployment handle for this signer.
    fn handle(&self) -> &str;

    /// Governed publisher peer identity bound to this signer.
    fn publisher_peer_id(&self) -> &[u8];

    /// Raw Ed25519 public key bound to the opaque handle.
    fn public_key(&self) -> [u8; 32];

    /// Sign one exact canonical Governance DAG payload.
    ///
    /// Implementations must not include credentials or provider diagnostics in
    /// the returned error. This crate nevertheless redacts every provider error
    /// at the trust boundary.
    fn sign(&self, payload: &[u8]) -> Result<[u8; 64], String>;
}

/// Authenticated Governance DAG endpoint class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernanceDagAuthenticationScope {
    /// Kubo/IPFS/IPNS control-plane request.
    Ipfs,
    /// Signed-head compare-and-swap request.
    SignedHead,
}

/// Rotation-aware runtime authenticator for Governance DAG publication.
///
/// Implementations own credentials and apply the currently active credential
/// on every request. This lets a deployment rotate bearer tokens, mTLS
/// identities, or HSM-backed request signatures behind one stable opaque
/// handle without copying secret material into resolved configuration.
pub trait GovernanceDagRequestAuthenticator: Send + Sync + fmt::Debug {
    /// Opaque, non-secret deployment handle for this authenticator.
    fn handle(&self) -> &str;

    /// Authenticate one exact outbound request.
    fn authenticate(
        &self,
        scope: GovernanceDagAuthenticationScope,
        request: RequestBuilder,
    ) -> Result<RequestBuilder, String>;
}

/// Durable object class owned by the sealed Governance DAG checkpoint store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernanceDagSealedStateSlot {
    /// Last fully published and verified checkpoint.
    Checkpoint,
    /// Write-ahead publication intent.
    PublishIntent,
}

impl GovernanceDagSealedStateSlot {
    fn domain(self) -> &'static [u8] {
        match self {
            Self::Checkpoint => b"sorafs.governance_dag.sealed.checkpoint.v1",
            Self::PublishIntent => b"sorafs.governance_dag.sealed.publish_intent.v1",
        }
    }
}

/// Unsealed canonical record returned by the runtime checkpoint provider.
///
/// The provider must keep this payload authenticated and confidential at rest.
/// `revision` is a public content/CAS token checked again by the service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceDagSealedStateRecord {
    /// Monotonic publication generation bound to the record.
    pub generation: u64,
    /// Deterministic content revision.
    pub revision: [u8; 32],
    /// Canonical Norito payload recovered by the provider.
    pub payload: Vec<u8>,
}

impl GovernanceDagSealedStateRecord {
    /// Construct a record and bind its public CAS revision.
    #[must_use]
    pub fn new(slot: GovernanceDagSealedStateSlot, generation: u64, payload: Vec<u8>) -> Self {
        let revision = governance_dag_sealed_state_revision(slot, generation, &payload);
        Self {
            generation,
            revision,
            payload,
        }
    }

    /// Verify the record's deterministic public CAS revision.
    #[must_use]
    pub fn has_valid_revision(&self, slot: GovernanceDagSealedStateSlot) -> bool {
        self.revision == governance_dag_sealed_state_revision(slot, self.generation, &self.payload)
    }
}

/// Derive the deterministic public CAS token for sealed Governance DAG state.
#[must_use]
pub fn governance_dag_sealed_state_revision(
    slot: GovernanceDagSealedStateSlot,
    generation: u64,
    payload: &[u8],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(slot.domain());
    hasher.update(&generation.to_le_bytes());
    hasher.update(
        &u64::try_from(payload.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(payload);
    *hasher.finalize().as_bytes()
}

/// Runtime-only sealed, monotonic Governance DAG checkpoint storage.
///
/// Implementations must seal payloads at rest and enforce linearizable
/// compare-and-swap. A generation may stay equal while an in-flight publish
/// intent advances, but it must never decrease. Checkpoint generation must
/// strictly advance, and deletes must compare-and-swap the exact last revision.
pub trait GovernanceDagSealedCheckpointStore: Send + Sync + fmt::Debug {
    /// Opaque, non-secret deployment handle for this store.
    fn handle(&self) -> &str;

    /// Load and unseal the latest record for `slot`.
    fn load(
        &self,
        slot: GovernanceDagSealedStateSlot,
    ) -> Result<Option<GovernanceDagSealedStateRecord>, String>;

    /// Atomically store `next` if `expected_revision` is still current.
    fn compare_and_swap(
        &self,
        slot: GovernanceDagSealedStateSlot,
        expected_revision: Option<[u8; 32]>,
        next: GovernanceDagSealedStateRecord,
    ) -> Result<(), String>;

    /// Atomically remove a transient record if its exact revision is current.
    fn delete(
        &self,
        slot: GovernanceDagSealedStateSlot,
        expected_revision: [u8; 32],
    ) -> Result<(), String>;
}

#[derive(Debug, Clone)]
struct PublishIndexEntryForCar {
    position: usize,
    payload_kind: String,
    encoded_path: String,
    json_path: String,
    encoded_blake3: String,
    encoded_len: usize,
}

/// Persists governance artefacts on the filesystem for downstream ingestion.
#[derive(Debug)]
pub(crate) struct FilesystemGovernancePublisher {
    root: PathBuf,
    runtime_dag_signer: Option<GovernanceRuntimeDagSigner>,
    publication_lock: Mutex<()>,
    _root_lock: File,
}

#[derive(Clone)]
struct GovernanceRuntimeDagSigner {
    handle: String,
    publisher_peer_id: Vec<u8>,
    public_key: [u8; 32],
    verification_key: PublicKey,
    provider: Arc<dyn GovernanceDagRuntimeSigner>,
}

impl fmt::Debug for GovernanceRuntimeDagSigner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceRuntimeDagSigner")
            .field("handle", &self.handle)
            .field("publisher_peer_id", &self.publisher_peer_id)
            .field("public_key", &hex::encode(&self.public_key))
            .finish_non_exhaustive()
    }
}

impl FilesystemGovernancePublisher {
    /// Construct an unsigned base publisher for crate-internal signed startup
    /// assembly and isolated tests.
    pub(crate) fn try_new(root: PathBuf) -> io::Result<Self> {
        validate_atomic_output_path(&root.join(".governance-root-probe"))?;
        fs::create_dir_all(&root)?;
        validate_atomic_output_path(&root.join(".governance-root-probe"))?;
        let root_lock = acquire_governance_publisher_lock(&root)?;
        Ok(Self {
            root,
            runtime_dag_signer: None,
            publication_lock: Mutex::new(()),
            _root_lock: root_lock,
        })
    }

    /// Enable signed runtime Governance DAG block/head assembly through an
    /// injected runtime-only signer.
    pub(crate) fn with_runtime_dag_signer_provider(
        mut self,
        expected_handle: impl Into<String>,
        expected_publisher_peer_id: impl Into<Vec<u8>>,
        expected_public_key: [u8; 32],
        signer: Arc<dyn GovernanceDagRuntimeSigner>,
    ) -> Result<Self, GovernancePublishError> {
        self.runtime_dag_signer = Some(GovernanceRuntimeDagSigner::try_new(
            expected_handle.into(),
            expected_publisher_peer_id.into(),
            expected_public_key,
            signer,
        )?);
        Ok(self)
    }

    fn settlements_root(&self) -> PathBuf {
        self.root.join("settlements")
    }

    fn pdp_archive_root(&self) -> PathBuf {
        self.root.join("pdp").join("archives")
    }

    fn por_challenge_root(&self) -> PathBuf {
        self.root.join("por").join("challenges")
    }

    fn por_report_root(&self) -> PathBuf {
        self.root.join("por").join("reports")
    }

    fn gc_audit_root(&self) -> PathBuf {
        self.root.join("gc").join("audit")
    }

    fn reconciliation_root(&self) -> PathBuf {
        self.root.join("reconciliation")
    }

    fn reputation_root(&self) -> PathBuf {
        self.root.join("reputation")
    }

    fn reputation_snapshot_root(&self) -> PathBuf {
        self.reputation_root().join("snapshots")
    }

    fn moderation_ballot_root(&self) -> PathBuf {
        self.root.join("moderation").join("ballots")
    }

    fn transparency_ledger_root(&self) -> PathBuf {
        self.root.join("transparency").join("ledger")
    }

    fn proof_token_issuance_root(&self) -> PathBuf {
        self.root.join("transparency").join("proof-tokens")
    }

    fn appeal_finance_root(&self) -> PathBuf {
        self.root.join("appeals").join("finance")
    }

    fn record_publish_index(
        &self,
        payload_kind: &str,
        encoded_path: &Path,
        json_path: &Path,
        digest_hex: &str,
        encoded_len: usize,
        labels: JsonMap,
    ) -> Result<(), GovernancePublishError> {
        let entry = update_publish_index(
            &self.root,
            payload_kind,
            encoded_path,
            json_path,
            digest_hex,
            encoded_len,
            labels,
        )?;
        ensure_governance_car_segment(&self.root, &entry)
    }

    fn record_runtime_signed_payload(
        &self,
        payload_kind: &str,
        payload: GovernanceLogPayloadV1,
        encoded_path: &Path,
        json_path: &Path,
        digest_hex: &str,
        encoded_len: usize,
    ) -> Result<(), GovernancePublishError> {
        let Some(signer) = &self.runtime_dag_signer else {
            return Ok(());
        };
        append_runtime_signed_dag_payload(
            &self.root,
            signer,
            payload_kind,
            payload,
            encoded_path,
            json_path,
            digest_hex,
            encoded_len,
        )
    }

    fn lock_publication(&self) -> Result<MutexGuard<'_, ()>, GovernancePublishError> {
        self.publication_lock.lock().map_err(|_| {
            GovernancePublishError::other(
                "filesystem governance publisher transaction lock is poisoned",
            )
        })
    }

    fn base_path(&self, settlement: &DealSettlementV1, digest_hex: &str) -> PathBuf {
        let deal_hex = settlement.deal_id.encode_hex::<String>();
        let status = status_label(settlement.status);
        let digest_prefix = &digest_hex[..16];
        let base = format!("{:020}_{}_{}", settlement.settled_at, status, digest_prefix);
        self.settlements_root().join(deal_hex).join(base)
    }

    fn gc_audit_path(&self, event: &GcAuditEventV1, digest_hex: &str) -> PathBuf {
        let sequence = format!("{:020}", event.header.sequence);
        let reason = sanitize_label(event.payload.reason.as_str());
        let manifest_hex = hex::encode(event.payload.manifest_digest);
        let digest_prefix = &digest_hex[..16];
        let base = format!("{sequence}_{reason}_{manifest_hex}_{digest_prefix}");
        self.gc_audit_root().join(base)
    }

    fn reconciliation_path(
        &self,
        report: &SorafsReconciliationReportV1,
        digest_hex: &str,
    ) -> PathBuf {
        let provider_hex = hex::encode(report.provider_id);
        let provider_prefix = &provider_hex[..16];
        let digest_prefix = &digest_hex[..16];
        let base = format!(
            "{:020}_{}_{}",
            report.generated_at_unix, provider_prefix, digest_prefix
        );
        self.reconciliation_root().join(base)
    }

    fn reputation_snapshot_path(
        &self,
        envelope: &SignedReputationSnapshotV1,
        digest_hex: &str,
    ) -> PathBuf {
        let snapshot = &envelope.snapshot;
        let snapshot_hex = hex::encode(snapshot.snapshot_id);
        let digest_prefix = &digest_hex[..16];
        let base = format!(
            "{:020}_{}_{}",
            snapshot.generated_at_unix, snapshot_hex, digest_prefix
        );
        self.reputation_snapshot_root()
            .join(snapshot_hex)
            .join(base)
    }

    fn moderation_ballot_event_path(
        &self,
        event: &SoraFsModerationBallotGovernanceEventV1,
        digest_hex: &str,
    ) -> PathBuf {
        let case_id = sanitize_label(&event.case_id);
        let round_id = sanitize_label(&event.round_id);
        let digest_prefix = &digest_hex[..16];
        let base = format!(
            "{:020}_{}_{}",
            event.sequence,
            event.kind.as_str(),
            digest_prefix
        );
        self.moderation_ballot_root()
            .join(case_id)
            .join(round_id)
            .join(base)
    }

    fn appeal_finance_report_path(
        &self,
        report: &SoraFsAppealFinanceReportV1,
        digest_hex: &str,
    ) -> PathBuf {
        let case_id = sanitize_label(&report.case_id);
        let round_id = report
            .round_id
            .as_deref()
            .map(sanitize_label)
            .unwrap_or_else(|| "no_round".to_string());
        let digest_prefix = &digest_hex[..16];
        let base = format!(
            "{:020}_{}_{}_{}",
            report.generated_at_unix_ms,
            round_id,
            report.outcome.as_str(),
            digest_prefix
        );
        self.appeal_finance_root().join(case_id).join(base)
    }

    fn appeal_finance_weekly_rollup_path(
        &self,
        rollup: &SoraFsAppealFinanceWeeklyRollupV1,
        digest_hex: &str,
    ) -> PathBuf {
        let cycle = sanitize_label(&rollup.cycle.to_string());
        let digest_prefix = &digest_hex[..16];
        let base = format!(
            "{:020}_reports-{}_{}",
            rollup.generated_at_unix_ms, rollup.report_count, digest_prefix
        );
        self.appeal_finance_root()
            .join("weekly")
            .join(cycle)
            .join(base)
    }

    fn appeal_finance_settlement_receipt_path(
        &self,
        receipt: &SoraFsAppealFinanceSettlementReceiptV1,
        digest_hex: &str,
    ) -> PathBuf {
        let case_id = sanitize_label(&receipt.case_id);
        let round_id = receipt
            .round_id
            .as_deref()
            .map(sanitize_label)
            .unwrap_or_else(|| "no_round".to_string());
        let digest_prefix = &digest_hex[..16];
        let receipt_id = hex::encode(receipt.receipt_id);
        let receipt_prefix = &receipt_id[..16];
        let base = format!(
            "{:020}_{}_{}_{}_{}",
            receipt.generated_at_unix_ms,
            round_id,
            sanitize_label(&receipt.submitted_step),
            receipt_prefix,
            digest_prefix
        );
        self.appeal_finance_root()
            .join("settlement-receipts")
            .join(case_id)
            .join(base)
    }

    fn transparency_ledger_publication_path(
        &self,
        publication: &ModerationLedgerCyclePublicationV1,
        digest_hex: &str,
    ) -> PathBuf {
        let cycle_id = hex::encode(publication.block.cycle_id);
        let digest_prefix = &digest_hex[..16];
        let base = format!(
            "{:020}_entries-{:010}_{}",
            publication.block.generated_at_unix, publication.block.entry_count, digest_prefix
        );
        self.transparency_ledger_root().join(cycle_id).join(base)
    }

    fn proof_token_issuance_path(
        &self,
        issuance: &ProofTokenIssuanceV1,
        digest_hex: &str,
    ) -> PathBuf {
        let token_id = hex::encode(issuance.token_id);
        let token_prefix = &token_id[..16];
        let digest_prefix = &digest_hex[..16];
        let base = format!(
            "{:020}_{}_{}",
            issuance.issued_at_unix, token_prefix, digest_prefix
        );
        self.proof_token_issuance_root().join(token_id).join(base)
    }
}

fn acquire_governance_publisher_lock(root: &Path) -> io::Result<File> {
    let lock_path = root.join(GOVERNANCE_PUBLISHER_LOCK_FILE);
    validate_atomic_output_path(&lock_path)?;
    let before_open = match fs::symlink_metadata(&lock_path) {
        Ok(metadata) => {
            validate_governance_lock_metadata(&lock_path, &metadata)?;
            Some(metadata)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => None,
        Err(err) => return Err(err),
    };
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true);
    set_no_follow_flag(&mut options);
    let file = options.open(&lock_path)?;
    let opened_metadata = file.metadata()?;
    validate_governance_lock_metadata(&lock_path, &opened_metadata)?;
    if before_open
        .as_ref()
        .is_some_and(|metadata| !metadata_identifies_same_file(metadata, &opened_metadata))
    {
        return Err(io::Error::other(format!(
            "governance publisher lock `{}` changed between inspection and open",
            lock_path.display()
        )));
    }
    let after_open = fs::symlink_metadata(&lock_path)?;
    validate_governance_lock_metadata(&lock_path, &after_open)?;
    if !metadata_identifies_same_file(&opened_metadata, &after_open) {
        return Err(io::Error::other(format!(
            "governance publisher lock path `{}` changed while opening",
            lock_path.display()
        )));
    }
    validate_atomic_output_path(&lock_path)?;
    match file.try_lock() {
        Ok(()) => {
            let locked_path_metadata = fs::symlink_metadata(&lock_path)?;
            validate_governance_lock_metadata(&lock_path, &locked_path_metadata)?;
            if !metadata_identifies_same_file(&opened_metadata, &locked_path_metadata) {
                return Err(io::Error::other(format!(
                    "governance publisher lock path `{}` changed while locking",
                    lock_path.display()
                )));
            }
            validate_atomic_output_path(&lock_path)?;
            Ok(file)
        }
        Err(fs::TryLockError::WouldBlock) => Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!(
                "governance publisher directory is already in use: {}",
                root.display()
            ),
        )),
        Err(fs::TryLockError::Error(err)) => Err(io::Error::new(
            err.kind(),
            format!(
                "failed to lock governance publisher directory via `{}`: {err}",
                lock_path.display()
            ),
        )),
    }
}

fn validate_governance_lock_metadata(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::other(format!(
            "governance publisher lock `{}` must be a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    if metadata.nlink() != 1 {
        return Err(io::Error::other(format!(
            "governance publisher lock `{}` must have exactly one hard link",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len()
}

fn status_label(status: DealSettlementStatusV1) -> &'static str {
    match status {
        DealSettlementStatusV1::WindowSettled => "window_settled",
        DealSettlementStatusV1::Completed => "completed",
        DealSettlementStatusV1::Cancelled => "cancelled",
        DealSettlementStatusV1::Defaulted => "defaulted",
    }
}

fn pdp_decision_label(decision: PdpTerminalDecisionV1) -> &'static str {
    match decision {
        PdpTerminalDecisionV1::Accepted => "accepted",
        PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::DeadlineExpired) => {
            "rejected_deadline_expired"
        }
        PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::SubmissionLate) => {
            "rejected_submission_late"
        }
        PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::FutureTimestamp) => {
            "rejected_future_timestamp"
        }
        PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::InvalidProof) => {
            "rejected_invalid_proof"
        }
        PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::AdmissionRevoked) => {
            "rejected_admission_revoked"
        }
        PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::AdmissionInactive) => {
            "rejected_admission_inactive"
        }
        PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::StorageUnavailable) => {
            "rejected_storage_unavailable"
        }
    }
}

fn sanitize_label(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out
}

fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    write_atomic_with_directory_sync(path, data, sync_directory)
}

fn write_atomic_with_directory_sync<F>(path: &Path, data: &[u8], sync_parent: F) -> io::Result<()>
where
    F: FnOnce(&Path) -> io::Result<()>,
{
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing parent directory"))?;
    validate_atomic_output_path(path)?;
    fs::create_dir_all(parent).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "failed to create output parent `{}`: {err}",
                parent.display()
            ),
        )
    })?;
    validate_atomic_output_path(path)?;
    let counter = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_path = temp_path_for_atomic(path, std::process::id(), counter);

    let write_result = (|| -> io::Result<()> {
        let mut file = open_atomic_temp_file(&tmp_path)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        validate_atomic_output_path(path)?;
        fs::rename(&tmp_path, path)?;
        sync_parent(parent)?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    write_result
}

fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        File::open(path)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

fn write_digest_sidecar(path: &Path, data: &[u8]) -> io::Result<()> {
    let digest = blake3::hash(data);
    let hex = digest.to_hex().to_string();
    let digest_path = digest_sidecar_path_for(path);
    let mut body = hex;
    body.push('\n');
    write_atomic(&digest_path, body.as_bytes())
}

fn digest_sidecar_path_for(path: &Path) -> PathBuf {
    let suffix = match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if !ext.is_empty() => format!("{ext}.blake3"),
        _ => "blake3".to_string(),
    };
    path.with_extension(suffix)
}

fn temp_path_for_atomic(path: &Path, pid: u32, counter: u64) -> PathBuf {
    let suffix = format!("tmp-{pid}-{counter}");
    let candidate = path.with_added_extension(&suffix);
    match candidate.file_name().and_then(|name| name.to_str()) {
        Some(name) => candidate.with_file_name(format!(".{name}")),
        None => candidate,
    }
}

fn open_atomic_temp_file(path: &Path) -> io::Result<File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    set_no_follow_flag(&mut options);
    let file = options.open(path).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("failed to create atomic temp `{}`: {err}", path.display()),
        )
    })?;
    let metadata = file.metadata().map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "failed to inspect atomic temp `{}` after open: {err}",
                path.display()
            ),
        )
    })?;
    if !metadata.is_file() {
        return Err(io::Error::other(format!(
            "atomic temp `{}` must be a regular file",
            path.display()
        )));
    }
    Ok(file)
}

fn validate_atomic_output_path(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(io::Error::other(format!(
                    "output `{}` must not be a symlink",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                return Err(io::Error::other(format!(
                    "output `{}` must not be a directory",
                    path.display()
                )));
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(io::Error::new(
                err.kind(),
                format!("failed to inspect output `{}`: {err}", path.display()),
            ));
        }
    }

    if let Some(parent) = path.parent() {
        for ancestor in std::iter::once(parent).chain(parent.ancestors().skip(1)) {
            if ancestor.as_os_str().is_empty() {
                continue;
            }
            match fs::symlink_metadata(ancestor) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() {
                        return Err(io::Error::other(format!(
                            "output parent `{}` must not be a symlink",
                            ancestor.display()
                        )));
                    }
                    if !metadata.is_dir() {
                        return Err(io::Error::other(format!(
                            "output parent `{}` must be a directory",
                            ancestor.display()
                        )));
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(io::Error::new(
                        err.kind(),
                        format!(
                            "failed to inspect output parent `{}`: {err}",
                            ancestor.display()
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn set_no_follow_flag(options: &mut fs::OpenOptions) {
    options.custom_flags(platform_no_follow_flag());
}

#[cfg(not(unix))]
fn set_no_follow_flag(_options: &mut fs::OpenOptions) {}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn platform_no_follow_flag() -> i32 {
    0o400000
}

#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
fn platform_no_follow_flag() -> i32 {
    0x100
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
fn platform_no_follow_flag() -> i32 {
    0
}

fn current_unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

impl GovernanceRuntimeDagSigner {
    fn try_new(
        expected_handle: String,
        publisher_peer_id: Vec<u8>,
        expected_public_key: [u8; 32],
        provider: Arc<dyn GovernanceDagRuntimeSigner>,
    ) -> Result<Self, GovernancePublishError> {
        validate_runtime_handle(&expected_handle, "governance runtime DAG signer")?;
        if publisher_peer_id.is_empty() {
            return Err(GovernancePublishError::other(
                "governance runtime DAG publisher peer id must not be empty",
            ));
        }
        if publisher_peer_id.len() > GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1 {
            return Err(GovernancePublishError::other(format!(
                "governance runtime DAG publisher peer id exceeds {GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1} bytes"
            )));
        }
        if expected_public_key.iter().all(|byte| *byte == 0) {
            return Err(GovernancePublishError::other(
                "governance runtime DAG signer public key must not be all zero",
            ));
        }
        let dalek_public_key =
            DalekVerifyingKey::from_bytes(&expected_public_key).map_err(|_| {
                GovernancePublishError::other(
                    "governance runtime DAG signer public key is not a canonical Ed25519 point",
                )
            })?;
        if dalek_public_key.to_bytes() != expected_public_key || dalek_public_key.is_weak() {
            return Err(GovernancePublishError::other(
                "governance runtime DAG signer public key is non-canonical or weak",
            ));
        }
        let verification_key = PublicKey::from_bytes(Algorithm::Ed25519, &expected_public_key)
            .map_err(|_| {
                GovernancePublishError::other(
                    "governance runtime DAG signer public key is not canonical Ed25519",
                )
            })?;
        if provider.handle() != expected_handle {
            return Err(GovernancePublishError::other(
                "governance runtime DAG signer handle does not match configured handle",
            ));
        }
        if provider.publisher_peer_id() != publisher_peer_id {
            return Err(GovernancePublishError::other(
                "governance runtime DAG signer publisher identity does not match configured identity",
            ));
        }
        if provider.public_key() != expected_public_key {
            return Err(GovernancePublishError::other(
                "governance runtime DAG signer public key does not match configured public key",
            ));
        }
        Ok(Self {
            handle: expected_handle,
            publisher_peer_id,
            public_key: expected_public_key,
            verification_key,
            provider,
        })
    }

    fn sign(&self, payload: &[u8]) -> Result<GovernanceLogSignatureV1, GovernancePublishError> {
        if self.provider.handle() != self.handle
            || self.provider.publisher_peer_id() != self.publisher_peer_id
            || self.provider.public_key() != self.public_key
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG signer identity changed after injection",
            ));
        }
        let signature_bytes = self.provider.sign(payload).map_err(|_| {
            GovernancePublishError::other(
                "governance runtime DAG signer refused the canonical payload",
            )
        })?;
        let signature = IrohaSignature::try_from_bytes(&signature_bytes).map_err(|_| {
            GovernancePublishError::other(
                "governance runtime DAG signer returned a malformed Ed25519 signature",
            )
        })?;
        signature
            .verify(&self.verification_key, payload)
            .map_err(|_| {
                GovernancePublishError::other(
                    "governance runtime DAG signer returned a signature for another key or payload",
                )
            })?;
        Ok(GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: self.public_key.to_vec(),
            signature: signature_bytes.to_vec(),
        })
    }

    fn publisher_peer_id_hex(&self) -> String {
        hex::encode(&self.publisher_peer_id)
    }

    fn publisher_public_key_hex(&self) -> String {
        hex::encode(&self.public_key)
    }
}

fn validate_runtime_handle(
    handle: &str,
    label: &'static str,
) -> Result<(), GovernancePublishError> {
    if handle.is_empty()
        || handle.len() > GOVERNANCE_RUNTIME_HANDLE_MAX_BYTES
        || !handle.is_ascii()
        || handle
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err(GovernancePublishError::other(format!(
            "{label} handle must be 1..={GOVERNANCE_RUNTIME_HANDLE_MAX_BYTES} visible ASCII bytes without whitespace"
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn metadata_stable_during_read(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    metadata_identifies_same_file(before, after)
        && before.len() == after.len()
        && before.mtime() == after.mtime()
        && before.mtime_nsec() == after.mtime_nsec()
        && before.ctime() == after.ctime()
        && before.ctime_nsec() == after.ctime_nsec()
}

#[cfg(not(unix))]
fn metadata_stable_during_read(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    metadata_identifies_same_file(before, after)
        && before.len() == after.len()
        && before.modified().ok() == after.modified().ok()
}

fn read_bounded_governance_state_file(path: &Path, max_bytes: usize) -> io::Result<Vec<u8>> {
    let max_bytes_u64 = u64::try_from(max_bytes)
        .map_err(|_| io::Error::other("governance state byte limit exceeds u64"))?;
    validate_atomic_output_path(path)?;
    let before_open = fs::symlink_metadata(path)?;
    validate_governance_state_metadata(path, &before_open)?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    set_no_follow_flag(&mut options);
    let mut file = options.open(path)?;
    let opened_metadata = file.metadata()?;
    validate_governance_state_metadata(path, &opened_metadata)?;
    if !metadata_identifies_same_file(&before_open, &opened_metadata) {
        return Err(io::Error::other(format!(
            "governance state `{}` changed while opening",
            path.display()
        )));
    }
    if opened_metadata.len() > max_bytes_u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance state `{}` exceeds {max_bytes} bytes",
                path.display()
            ),
        ));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(opened_metadata.len()).unwrap_or(max_bytes));
    (&mut file)
        .take(max_bytes_u64.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance state `{}` exceeds {max_bytes} bytes",
                path.display()
            ),
        ));
    }
    let after_read_file = file.metadata()?;
    if !metadata_stable_during_read(&opened_metadata, &after_read_file) {
        return Err(io::Error::other(format!(
            "governance state `{}` changed while reading",
            path.display()
        )));
    }
    let after_read = fs::symlink_metadata(path)?;
    validate_governance_state_metadata(path, &after_read)?;
    if !metadata_identifies_same_file(&opened_metadata, &after_read) {
        return Err(io::Error::other(format!(
            "governance state `{}` changed while reading",
            path.display()
        )));
    }
    validate_atomic_output_path(path)?;
    Ok(bytes)
}

fn validate_governance_state_metadata(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::other(format!(
            "governance state `{}` must be a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    if metadata.nlink() != 1 {
        return Err(io::Error::other(format!(
            "governance state `{}` must have exactly one hard link",
            path.display()
        )));
    }
    Ok(())
}

fn update_publish_index(
    root: &Path,
    payload_kind: &str,
    encoded_path: &Path,
    json_path: &Path,
    digest_hex: &str,
    encoded_len: usize,
    labels: JsonMap,
) -> Result<PublishIndexEntryForCar, GovernancePublishError> {
    let index_path = root.join(GOVERNANCE_PUBLISH_INDEX_FILE);
    let mut index = read_publish_index(root, &index_path)?;
    let mut entries = match index.remove("entries") {
        Some(JsonValue::Array(entries)) => entries,
        Some(_) => {
            return Err(GovernancePublishError::other(
                "governance publish index has non-array `entries`",
            ));
        }
        None => Vec::new(),
    };
    let encoded_path = index_path_string(root, encoded_path);
    let json_path = index_path_string(root, json_path);
    let duplicate_position = entries.iter().position(|entry| {
        entry.get("payload_kind").and_then(JsonValue::as_str) == Some(payload_kind)
            && entry.get("encoded_blake3").and_then(JsonValue::as_str) == Some(digest_hex)
            && entry.get("encoded_path").and_then(JsonValue::as_str) == Some(encoded_path.as_str())
    });
    let position = duplicate_position.unwrap_or(entries.len());
    if duplicate_position.is_none() {
        let mut entry = JsonMap::new();
        entry.insert("position".into(), JsonValue::from(position as u64));
        entry.insert("payload_kind".into(), JsonValue::from(payload_kind));
        entry.insert("encoded_path".into(), JsonValue::from(encoded_path.clone()));
        entry.insert("json_path".into(), JsonValue::from(json_path.clone()));
        entry.insert(
            "encoded_blake3".into(),
            JsonValue::from(digest_hex.to_string()),
        );
        entry.insert(
            "encoded_len".into(),
            JsonValue::from(u64::try_from(encoded_len).unwrap_or(u64::MAX)),
        );
        entry.insert(
            "published_at_unix".into(),
            JsonValue::from(current_unix_timestamp_seconds()),
        );
        entry.insert("labels".into(), JsonValue::Object(labels));
        entries.push(JsonValue::Object(entry));
    }
    rebuild_publish_index(root, index, entries, &index_path)?;
    Ok(PublishIndexEntryForCar {
        position,
        payload_kind: payload_kind.to_owned(),
        encoded_path,
        json_path,
        encoded_blake3: digest_hex.to_owned(),
        encoded_len,
    })
}

fn read_publish_index(root: &Path, index_path: &Path) -> Result<JsonMap, GovernancePublishError> {
    match read_bounded_governance_state_file(index_path, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES) {
        Ok(bytes) => {
            let value: JsonValue = json::from_slice(&bytes).map_err(|err| {
                GovernancePublishError::other(format!(
                    "failed to parse governance publish index `{}`: {err}",
                    index_path.display()
                ))
            })?;
            let JsonValue::Object(map) = value else {
                return Err(GovernancePublishError::other(
                    "governance publish index root is not an object",
                ));
            };
            if map.get("schema").and_then(JsonValue::as_str)
                != Some(GOVERNANCE_PUBLISH_INDEX_SCHEMA)
            {
                return Err(GovernancePublishError::other(
                    "governance publish index uses an unsupported schema",
                ));
            }
            Ok(map)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            let mut map = JsonMap::new();
            map.insert(
                "schema".into(),
                JsonValue::from(GOVERNANCE_PUBLISH_INDEX_SCHEMA),
            );
            map.insert(
                "source".into(),
                JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
            );
            map.insert("root".into(), JsonValue::from(root.display().to_string()));
            map.insert("entries".into(), JsonValue::Array(Vec::new()));
            Ok(map)
        }
        Err(err) => Err(err.into()),
    }
}

fn rebuild_publish_index(
    root: &Path,
    mut index: JsonMap,
    mut entries: Vec<JsonValue>,
    index_path: &Path,
) -> Result<(), GovernancePublishError> {
    let mut payload_kind_counts = JsonMap::new();
    let mut by_encoded_blake3 = JsonMap::new();
    let mut by_payload_kind = JsonMap::new();

    for (position, entry) in entries.iter_mut().enumerate() {
        let Some(entry_map) = entry.as_object_mut() else {
            return Err(GovernancePublishError::other(
                "governance publish index entry is not an object",
            ));
        };
        entry_map.insert("position".into(), JsonValue::from(position as u64));
        let Some(payload_kind) = entry_map
            .get("payload_kind")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
        else {
            return Err(GovernancePublishError::other(
                "governance publish index entry is missing `payload_kind`",
            ));
        };
        let count = payload_kind_counts
            .get(&payload_kind)
            .and_then(JsonValue::as_u64)
            .unwrap_or(0)
            .saturating_add(1);
        payload_kind_counts.insert(payload_kind.clone(), JsonValue::from(count));
        append_index_position(&mut by_payload_kind, &payload_kind, position);

        let Some(digest_hex) = entry_map
            .get("encoded_blake3")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
        else {
            return Err(GovernancePublishError::other(
                "governance publish index entry is missing `encoded_blake3`",
            ));
        };
        append_index_position(&mut by_encoded_blake3, &digest_hex, position);
    }

    index.insert(
        "schema".into(),
        JsonValue::from(GOVERNANCE_PUBLISH_INDEX_SCHEMA),
    );
    index.insert(
        "source".into(),
        JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
    );
    index.insert("root".into(), JsonValue::from(root.display().to_string()));
    index.insert(
        "generated_at".into(),
        JsonValue::from(current_unix_timestamp_seconds()),
    );
    index.insert("entry_count".into(), JsonValue::from(entries.len() as u64));
    index.insert(
        "payload_kind_counts".into(),
        JsonValue::Object(payload_kind_counts),
    );
    index.insert(
        "by_encoded_blake3".into(),
        JsonValue::Object(by_encoded_blake3),
    );
    index.insert("by_payload_kind".into(), JsonValue::Object(by_payload_kind));
    index.insert("entries".into(), JsonValue::Array(entries));

    let body = json::to_json_pretty(&JsonValue::Object(index)).map_err(|err| {
        GovernancePublishError::other(format!("serialize governance publish index: {err}"))
    })?;
    write_atomic(index_path, body.as_bytes())?;
    write_digest_sidecar(index_path, body.as_bytes())?;
    Ok(())
}

fn append_index_position(index: &mut JsonMap, key: &str, position: usize) {
    let position = JsonValue::from(position as u64);
    match index.get_mut(key).and_then(JsonValue::as_array_mut) {
        Some(positions) => positions.push(position),
        None => {
            index.insert(key.to_string(), JsonValue::Array(vec![position]));
        }
    }
}

fn index_path_string(root: &Path, path: &Path) -> String {
    let path = path.strip_prefix(root).unwrap_or(path);
    let parts = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

fn ensure_governance_car_segment(
    root: &Path,
    entry: &PublishIndexEntryForCar,
) -> Result<(), GovernancePublishError> {
    let queue_path = root.join(GOVERNANCE_CAR_QUEUE_FILE);
    let mut queue = read_car_queue(root, &queue_path)?;
    let mut segments = match queue.remove("segments") {
        Some(JsonValue::Array(segments)) => segments,
        Some(_) => {
            return Err(GovernancePublishError::other(
                "governance CAR queue has non-array `segments`",
            ));
        }
        None => Vec::new(),
    };
    let existing_position = segments.iter().position(|segment| {
        segment
            .get("source_publish_index_position")
            .and_then(JsonValue::as_u64)
            == Some(entry.position as u64)
            && segment.get("encoded_blake3").and_then(JsonValue::as_str)
                == Some(entry.encoded_blake3.as_str())
    });
    if let Some(position) = existing_position
        && governance_car_segment_files_exist(root, &segments[position])
    {
        record_governance_dag_backlog(governance_car_queue_pending_count(&segments));
        return Ok(());
    }

    let segment = assemble_governance_car_segment(root, entry)?;
    match existing_position {
        Some(position) => segments[position] = JsonValue::Object(segment),
        None => segments.push(JsonValue::Object(segment)),
    }
    rebuild_car_queue(root, queue, segments, &queue_path)
}

fn read_car_queue(root: &Path, queue_path: &Path) -> Result<JsonMap, GovernancePublishError> {
    match read_bounded_governance_state_file(queue_path, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES) {
        Ok(bytes) => {
            let value: JsonValue = json::from_slice(&bytes).map_err(|err| {
                GovernancePublishError::other(format!(
                    "failed to parse governance CAR queue `{}`: {err}",
                    queue_path.display()
                ))
            })?;
            let JsonValue::Object(map) = value else {
                return Err(GovernancePublishError::other(
                    "governance CAR queue root is not an object",
                ));
            };
            if map.get("schema").and_then(JsonValue::as_str) != Some(GOVERNANCE_CAR_QUEUE_SCHEMA) {
                return Err(GovernancePublishError::other(
                    "governance CAR queue uses an unsupported schema",
                ));
            }
            Ok(map)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            let mut map = JsonMap::new();
            map.insert(
                "schema".into(),
                JsonValue::from(GOVERNANCE_CAR_QUEUE_SCHEMA),
            );
            map.insert(
                "source".into(),
                JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
            );
            map.insert("root".into(), JsonValue::from(root.display().to_string()));
            map.insert("segments".into(), JsonValue::Array(Vec::new()));
            Ok(map)
        }
        Err(err) => Err(err.into()),
    }
}

fn rebuild_car_queue(
    root: &Path,
    mut queue: JsonMap,
    mut segments: Vec<JsonValue>,
    queue_path: &Path,
) -> Result<(), GovernancePublishError> {
    let mut by_encoded_blake3 = JsonMap::new();
    let mut by_payload_kind = JsonMap::new();
    let mut assembled_count = 0u64;

    for (position, segment) in segments.iter_mut().enumerate() {
        let Some(segment_map) = segment.as_object_mut() else {
            return Err(GovernancePublishError::other(
                "governance CAR queue segment is not an object",
            ));
        };
        segment_map.insert("queue_position".into(), JsonValue::from(position as u64));
        if segment_map.get("schema").and_then(JsonValue::as_str)
            != Some(GOVERNANCE_CAR_SEGMENT_SCHEMA)
        {
            return Err(GovernancePublishError::other(
                "governance CAR queue segment uses an unsupported schema",
            ));
        }
        let Some(payload_kind) = segment_map
            .get("payload_kind")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
        else {
            return Err(GovernancePublishError::other(
                "governance CAR queue segment is missing `payload_kind`",
            ));
        };
        append_index_position(&mut by_payload_kind, &payload_kind, position);
        let Some(digest_hex) = segment_map
            .get("encoded_blake3")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
        else {
            return Err(GovernancePublishError::other(
                "governance CAR queue segment is missing `encoded_blake3`",
            ));
        };
        append_index_position(&mut by_encoded_blake3, &digest_hex, position);
        if segment_map
            .get("status")
            .and_then(JsonValue::as_str)
            .is_some_and(|status| status == "assembled")
        {
            assembled_count = assembled_count.saturating_add(1);
        }
    }

    queue.insert(
        "schema".into(),
        JsonValue::from(GOVERNANCE_CAR_QUEUE_SCHEMA),
    );
    queue.insert(
        "source".into(),
        JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
    );
    queue.insert("root".into(), JsonValue::from(root.display().to_string()));
    queue.insert(
        "generated_at".into(),
        JsonValue::from(current_unix_timestamp_seconds()),
    );
    queue.insert(
        "segment_count".into(),
        JsonValue::from(segments.len() as u64),
    );
    queue.insert("assembled_count".into(), JsonValue::from(assembled_count));
    let pending_count = (segments.len() as u64).saturating_sub(assembled_count);
    queue.insert("pending_count".into(), JsonValue::from(pending_count));
    queue.insert(
        "by_encoded_blake3".into(),
        JsonValue::Object(by_encoded_blake3),
    );
    queue.insert("by_payload_kind".into(), JsonValue::Object(by_payload_kind));
    queue.insert("segments".into(), JsonValue::Array(segments));

    let body = json::to_json_pretty(&JsonValue::Object(queue)).map_err(|err| {
        GovernancePublishError::other(format!("serialize governance CAR queue: {err}"))
    })?;
    write_atomic(queue_path, body.as_bytes())?;
    write_digest_sidecar(queue_path, body.as_bytes())?;
    record_governance_dag_backlog(pending_count);
    Ok(())
}

fn governance_car_queue_pending_count(segments: &[JsonValue]) -> u64 {
    let assembled_count = segments
        .iter()
        .filter(|segment| {
            segment
                .get("status")
                .and_then(JsonValue::as_str)
                .is_some_and(|status| status == "assembled")
        })
        .count() as u64;
    (segments.len() as u64).saturating_sub(assembled_count)
}

fn governance_car_segment_files_exist(root: &Path, segment: &JsonValue) -> bool {
    let Some(segment) = segment.as_object() else {
        return false;
    };
    ["car_path", "plan_path", "manifest_path"]
        .iter()
        .all(|field| {
            segment
                .get(*field)
                .and_then(JsonValue::as_str)
                .and_then(|path| resolve_index_path(root, path).ok())
                .is_some_and(|path| path.is_file())
        })
}

fn assemble_governance_car_segment(
    root: &Path,
    entry: &PublishIndexEntryForCar,
) -> Result<JsonMap, GovernancePublishError> {
    let (files, file_records) = governance_car_segment_files(root, entry)?;
    let (plan, payload) = CarBuildPlan::from_files(files).map_err(|err| {
        GovernancePublishError::other(format!("build governance CAR segment plan: {err}"))
    })?;
    let mut car_bytes = Vec::new();
    let stats = CarWriter::new(&plan, &payload)
        .map_err(|err| GovernancePublishError::other(format!("initialise CAR writer: {err}")))?
        .write_to(&mut car_bytes)
        .map_err(|err| GovernancePublishError::other(format!("write CAR segment: {err}")))?;

    let base_path = governance_car_segment_base_path(root, entry);
    let car_path = base_path.with_extension("car");
    let plan_path = base_path.with_extension("plan.json");
    let manifest_path = base_path.with_extension("json");

    write_atomic(&car_path, &car_bytes)?;
    write_digest_sidecar(&car_path, &car_bytes)?;

    let plan_json = governance_car_plan_json(entry, &plan, &stats, &file_records);
    let plan_body = json::to_json_pretty(&JsonValue::Object(plan_json)).map_err(|err| {
        GovernancePublishError::other(format!("serialize governance CAR plan: {err}"))
    })?;
    write_atomic(&plan_path, plan_body.as_bytes())?;
    write_digest_sidecar(&plan_path, plan_body.as_bytes())?;

    let segment_json = governance_car_segment_json(
        root,
        entry,
        &stats,
        &file_records,
        &car_path,
        &plan_path,
        &manifest_path,
    );
    let segment_body =
        json::to_json_pretty(&JsonValue::Object(segment_json.clone())).map_err(|err| {
            GovernancePublishError::other(format!("serialize governance CAR segment: {err}"))
        })?;
    write_atomic(&manifest_path, segment_body.as_bytes())?;
    write_digest_sidecar(&manifest_path, segment_body.as_bytes())?;
    Ok(segment_json)
}

fn governance_car_segment_base_path(root: &Path, entry: &PublishIndexEntryForCar) -> PathBuf {
    let digest_prefix = &entry.encoded_blake3[..entry.encoded_blake3.len().min(16)];
    let base = format!(
        "{:020}_{}_{}",
        entry.position,
        sanitize_label(&entry.payload_kind),
        digest_prefix
    );
    root.join(GOVERNANCE_CAR_SEGMENTS_DIR).join(base)
}

fn governance_car_segment_files(
    root: &Path,
    entry: &PublishIndexEntryForCar,
) -> Result<(Vec<FileEntry>, Vec<JsonValue>), GovernancePublishError> {
    let encoded_path = resolve_index_path(root, &entry.encoded_path)?;
    let json_path = resolve_index_path(root, &entry.json_path)?;
    let encoded_sidecar = digest_sidecar_path_for(&encoded_path);
    let json_sidecar = digest_sidecar_path_for(&json_path);
    let encoded_sidecar_path = index_path_string(root, &encoded_sidecar);
    let json_sidecar_path = index_path_string(root, &json_sidecar);
    let specs = [
        ("encoded", entry.encoded_path.as_str(), encoded_path),
        (
            "encoded_blake3_sidecar",
            encoded_sidecar_path.as_str(),
            encoded_sidecar,
        ),
        ("json", entry.json_path.as_str(), json_path),
        (
            "json_blake3_sidecar",
            json_sidecar_path.as_str(),
            json_sidecar,
        ),
    ];
    let mut files = Vec::with_capacity(specs.len());
    let mut records = Vec::with_capacity(specs.len());
    for (role, relative_path, absolute_path) in specs {
        let bytes = fs::read(&absolute_path).map_err(|err| {
            GovernancePublishError::other(format!(
                "read governance CAR segment source `{}`: {err}",
                absolute_path.display()
            ))
        })?;
        let mut record = JsonMap::new();
        record.insert("role".into(), JsonValue::from(role));
        record.insert("path".into(), JsonValue::from(relative_path));
        record.insert("bytes".into(), JsonValue::from(bytes.len() as u64));
        record.insert(
            "blake3".into(),
            JsonValue::from(blake3::hash(&bytes).to_hex().to_string()),
        );
        files.push(FileEntry {
            path: index_path_components(relative_path)?,
            data: bytes,
        });
        records.push(JsonValue::Object(record));
    }
    Ok((files, records))
}

fn governance_car_plan_json(
    entry: &PublishIndexEntryForCar,
    plan: &CarBuildPlan,
    stats: &sorafs_car::CarWriteStats,
    file_records: &[JsonValue],
) -> JsonMap {
    let mut root = JsonMap::new();
    root.insert("schema".into(), JsonValue::from(GOVERNANCE_CAR_PLAN_SCHEMA));
    root.insert(
        "source_publish_index_position".into(),
        JsonValue::from(entry.position as u64),
    );
    root.insert(
        "payload_kind".into(),
        JsonValue::from(entry.payload_kind.clone()),
    );
    root.insert(
        "encoded_blake3".into(),
        JsonValue::from(entry.encoded_blake3.clone()),
    );
    root.insert(
        "encoded_len".into(),
        JsonValue::from(u64::try_from(entry.encoded_len).unwrap_or(u64::MAX)),
    );
    root.insert(
        "content_length".into(),
        JsonValue::from(plan.content_length),
    );
    root.insert(
        "payload_blake3".into(),
        JsonValue::from(plan.payload_digest.to_hex().to_string()),
    );
    root.insert("dag_codec".into(), JsonValue::from(stats.dag_codec));
    root.insert(
        "chunk_count".into(),
        JsonValue::from(plan.chunks.len() as u64),
    );
    root.insert("files".into(), JsonValue::Array(file_records.to_vec()));
    root.insert("chunk_profile".into(), chunk_profile_json(plan));
    root.insert("chunks".into(), governance_car_chunks_json(plan));
    root
}

fn governance_car_segment_json(
    root: &Path,
    entry: &PublishIndexEntryForCar,
    stats: &sorafs_car::CarWriteStats,
    file_records: &[JsonValue],
    car_path: &Path,
    plan_path: &Path,
    manifest_path: &Path,
) -> JsonMap {
    let mut segment = JsonMap::new();
    segment.insert(
        "schema".into(),
        JsonValue::from(GOVERNANCE_CAR_SEGMENT_SCHEMA),
    );
    segment.insert("status".into(), JsonValue::from("assembled"));
    segment.insert(
        "source".into(),
        JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
    );
    segment.insert(
        "source_publish_index_position".into(),
        JsonValue::from(entry.position as u64),
    );
    segment.insert(
        "payload_kind".into(),
        JsonValue::from(entry.payload_kind.clone()),
    );
    segment.insert(
        "encoded_path".into(),
        JsonValue::from(entry.encoded_path.clone()),
    );
    segment.insert("json_path".into(), JsonValue::from(entry.json_path.clone()));
    segment.insert(
        "encoded_blake3".into(),
        JsonValue::from(entry.encoded_blake3.clone()),
    );
    segment.insert(
        "encoded_len".into(),
        JsonValue::from(u64::try_from(entry.encoded_len).unwrap_or(u64::MAX)),
    );
    segment.insert(
        "car_path".into(),
        JsonValue::from(index_path_string(root, car_path)),
    );
    segment.insert(
        "plan_path".into(),
        JsonValue::from(index_path_string(root, plan_path)),
    );
    segment.insert(
        "manifest_path".into(),
        JsonValue::from(index_path_string(root, manifest_path)),
    );
    segment.insert("car_size".into(), JsonValue::from(stats.car_size));
    segment.insert(
        "car_archive_blake3".into(),
        JsonValue::from(stats.car_archive_digest.to_hex().to_string()),
    );
    segment.insert(
        "car_payload_blake3".into(),
        JsonValue::from(stats.car_payload_digest.to_hex().to_string()),
    );
    segment.insert(
        "car_cid_hex".into(),
        JsonValue::from(hex::encode(&stats.car_cid)),
    );
    segment.insert(
        "root_cids_hex".into(),
        JsonValue::Array(
            stats
                .root_cids
                .iter()
                .map(|cid| JsonValue::from(hex::encode(cid)))
                .collect(),
        ),
    );
    segment.insert("dag_codec".into(), JsonValue::from(stats.dag_codec));
    segment.insert(
        "chunk_count".into(),
        JsonValue::from(stats.chunk_count as u64),
    );
    segment.insert("payload_bytes".into(), JsonValue::from(stats.payload_bytes));
    segment.insert(
        "assembled_at_unix".into(),
        JsonValue::from(current_unix_timestamp_seconds()),
    );
    segment.insert("files".into(), JsonValue::Array(file_records.to_vec()));
    segment.insert("chunk_profile".into(), chunk_profile_json_from_stats(stats));
    segment
}

fn chunk_profile_json(plan: &CarBuildPlan) -> JsonValue {
    let profile = plan.chunk_profile;
    let mut value = JsonMap::new();
    value.insert("min_size".into(), JsonValue::from(profile.min_size as u64));
    value.insert(
        "target_size".into(),
        JsonValue::from(profile.target_size as u64),
    );
    value.insert("max_size".into(), JsonValue::from(profile.max_size as u64));
    value.insert("break_mask".into(), JsonValue::from(profile.break_mask));
    JsonValue::Object(value)
}

fn chunk_profile_json_from_stats(stats: &sorafs_car::CarWriteStats) -> JsonValue {
    let profile = stats.chunk_profile;
    let mut value = JsonMap::new();
    value.insert("min_size".into(), JsonValue::from(profile.min_size as u64));
    value.insert(
        "target_size".into(),
        JsonValue::from(profile.target_size as u64),
    );
    value.insert("max_size".into(), JsonValue::from(profile.max_size as u64));
    value.insert("break_mask".into(), JsonValue::from(profile.break_mask));
    JsonValue::Object(value)
}

fn governance_car_chunks_json(plan: &CarBuildPlan) -> JsonValue {
    JsonValue::Array(
        plan.chunks
            .iter()
            .enumerate()
            .map(|(index, chunk)| {
                let mut value = JsonMap::new();
                value.insert("index".into(), JsonValue::from(index as u64));
                value.insert("offset".into(), JsonValue::from(chunk.offset));
                value.insert("length".into(), JsonValue::from(chunk.length as u64));
                value.insert("blake3".into(), JsonValue::from(hex::encode(chunk.digest)));
                JsonValue::Object(value)
            })
            .collect(),
    )
}

fn resolve_index_path(root: &Path, relative_path: &str) -> Result<PathBuf, GovernancePublishError> {
    let components = index_path_components(relative_path)?;
    let mut path = root.to_path_buf();
    for component in components {
        path.push(component);
    }
    Ok(path)
}

fn index_path_components(relative_path: &str) -> Result<Vec<String>, GovernancePublishError> {
    if relative_path.is_empty()
        || relative_path == "."
        || relative_path.starts_with('/')
        || relative_path.contains('\\')
    {
        return Err(GovernancePublishError::other(
            "governance CAR queue path must be a relative slash-separated path",
        ));
    }
    let mut components = Vec::new();
    for component in relative_path.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(GovernancePublishError::other(
                "governance CAR queue path contains an invalid component",
            ));
        }
        components.push(component.to_owned());
    }
    Ok(components)
}

#[derive(Debug, Clone)]
struct RuntimeDagTip {
    sequence: u64,
    block_cid: Vec<u8>,
    node_cid: Vec<u8>,
}

// The runtime DAG append helper keeps the filesystem, signer, payload, and
// derived artifact metadata together so every publish path indexes identical
// evidence fields.
#[allow(clippy::too_many_arguments)]
fn append_runtime_signed_dag_payload(
    root: &Path,
    signer: &GovernanceRuntimeDagSigner,
    payload_kind: &str,
    payload: GovernanceLogPayloadV1,
    encoded_path: &Path,
    json_path: &Path,
    digest_hex: &str,
    encoded_len: usize,
) -> Result<(), GovernancePublishError> {
    let index_path = root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE);
    let mut index = read_runtime_dag_index(root, signer, &index_path)?;
    let mut blocks = match index.remove("blocks") {
        Some(JsonValue::Array(blocks)) => blocks,
        Some(_) => {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index has non-array `blocks`",
            ));
        }
        None => Vec::new(),
    };

    let duplicate_position = blocks.iter().position(|entry| {
        entry.get("payload_kind").and_then(JsonValue::as_str) == Some(payload_kind)
            && entry
                .get("source_payload_blake3")
                .and_then(JsonValue::as_str)
                == Some(digest_hex)
    });
    if let Some(position) = duplicate_position {
        if runtime_dag_index_entry_files_exist(root, &blocks[position]) {
            record_governance_dag_head_age_from_index(&index);
            return Ok(());
        }
        return Err(GovernancePublishError::other(
            "governance runtime DAG index references a missing block file",
        ));
    }

    let tip = runtime_dag_tip_from_entries(&blocks)?;
    let sequence = match tip.as_ref() {
        Some(tip) => tip.sequence.checked_add(1).ok_or_else(|| {
            GovernancePublishError::other("governance runtime DAG sequence exhausted")
        })?,
        None => 0,
    };
    let timestamp = current_unix_timestamp_seconds();
    let mut node = GovernanceLogNodeV1 {
        version: GOVERNANCE_LOG_VERSION_V1,
        node_cid: Vec::new(),
        prev_cid: tip.as_ref().map(|tip| tip.node_cid.clone()),
        timestamp,
        publisher_peer_id: signer.publisher_peer_id.clone(),
        payload,
        publisher_signature: empty_governance_ed25519_signature(),
    };
    node.node_cid = node.recompute_node_cid().map_err(|err| {
        GovernancePublishError::other(format!("derive governance runtime DAG node CID: {err}"))
    })?;
    let node_payload = node.signature_payload_bytes().map_err(|err| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG node signing payload: {err}"
        ))
    })?;
    node.publisher_signature = signer.sign(&node_payload)?;
    node.validate().map_err(|err| {
        GovernancePublishError::other(format!("validate governance runtime DAG node: {err}"))
    })?;
    node.verify_publisher_signature().map_err(|err| {
        GovernancePublishError::other(format!(
            "verify governance runtime DAG node signature: {err}"
        ))
    })?;

    let prev_block_cid = tip.as_ref().map(|tip| tip.block_cid.clone());
    let block_cid = governance_dag_block_cid_v1(
        prev_block_cid.as_deref(),
        sequence,
        timestamp,
        &signer.publisher_peer_id,
        &node,
    )
    .map_err(|err| {
        GovernancePublishError::other(format!("derive governance runtime DAG block CID: {err}"))
    })?;
    let mut block = GovernanceDagBlockV1 {
        version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
        block_cid,
        prev_block_cid,
        sequence,
        timestamp,
        publisher_peer_id: signer.publisher_peer_id.clone(),
        node,
        block_signature: empty_governance_ed25519_signature(),
    };
    let block_payload = block.signature_payload_bytes().map_err(|err| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG block signing payload: {err}"
        ))
    })?;
    block.block_signature = signer.sign(&block_payload)?;
    block.validate().map_err(|err| {
        GovernancePublishError::other(format!("validate governance runtime DAG block: {err}"))
    })?;

    let block_count = sequence.checked_add(1).ok_or_else(|| {
        GovernancePublishError::other("governance runtime DAG block count exhausted")
    })?;
    let checkpoint_cid = runtime_dag_checkpoint_cid(&blocks, block_count)?;
    let mut head = GovernanceDagHeadV1 {
        version: GOVERNANCE_DAG_HEAD_VERSION_V1,
        head_block_cid: block.block_cid.clone(),
        block_count,
        generated_at: timestamp,
        publisher_peer_id: signer.publisher_peer_id.clone(),
        checkpoint_cid,
        head_signature: empty_governance_ed25519_signature(),
    };
    let head_payload = head.signature_payload_bytes().map_err(|err| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG head signing payload: {err}"
        ))
    })?;
    head.head_signature = signer.sign(&head_payload)?;
    head.validate().map_err(|err| {
        GovernancePublishError::other(format!("validate governance runtime DAG head: {err}"))
    })?;

    let block_bytes = norito::to_bytes(&block).map_err(|err| {
        GovernancePublishError::other(format!("encode governance runtime DAG block: {err}"))
    })?;
    let block_position = u64::try_from(blocks.len()).map_err(|_| {
        GovernancePublishError::other("governance runtime DAG block position exceeds u64")
    })?;
    let block_encoded_len = u64::try_from(block_bytes.len()).map_err(|_| {
        GovernancePublishError::other("governance runtime DAG block length exceeds u64")
    })?;
    let source_payload_len = u64::try_from(encoded_len).map_err(|_| {
        GovernancePublishError::other("governance runtime DAG source payload length exceeds u64")
    })?;
    let block_digest_hex = blake3::hash(&block_bytes).to_hex().to_string();
    let block_cid_hex = hex::encode(&block.block_cid);
    let block_path = runtime_dag_block_path(root, sequence, &block_cid_hex);
    write_atomic(&block_path, &block_bytes)?;
    write_digest_sidecar(&block_path, &block_bytes)?;

    let head_bytes = norito::to_bytes(&head).map_err(|err| {
        GovernancePublishError::other(format!("encode governance runtime DAG head: {err}"))
    })?;
    let head_path = runtime_dag_head_path(root);
    write_atomic(&head_path, &head_bytes)?;
    write_digest_sidecar(&head_path, &head_bytes)?;

    let mut entry = JsonMap::new();
    entry.insert("position".into(), JsonValue::from(block_position));
    entry.insert("sequence".into(), JsonValue::from(sequence));
    entry.insert("payload_kind".into(), JsonValue::from(payload_kind));
    entry.insert("encoded_blake3".into(), JsonValue::from(block_digest_hex));
    entry.insert("encoded_len".into(), JsonValue::from(block_encoded_len));
    entry.insert(
        "source_payload_blake3".into(),
        JsonValue::from(digest_hex.to_owned()),
    );
    entry.insert(
        "source_payload_len".into(),
        JsonValue::from(source_payload_len),
    );
    entry.insert(
        "encoded_path".into(),
        JsonValue::from(index_path_string(root, encoded_path)),
    );
    entry.insert(
        "json_path".into(),
        JsonValue::from(index_path_string(root, json_path)),
    );
    entry.insert(
        "node_cid_hex".into(),
        JsonValue::from(hex::encode(&block.node.node_cid)),
    );
    entry.insert(
        "prev_node_cid_hex".into(),
        tip.as_ref()
            .map(|tip| JsonValue::from(hex::encode(&tip.node_cid)))
            .unwrap_or(JsonValue::Null),
    );
    entry.insert(
        "block_cid_hex".into(),
        JsonValue::from(block_cid_hex.clone()),
    );
    entry.insert(
        "prev_block_cid_hex".into(),
        tip.as_ref()
            .map(|tip| JsonValue::from(hex::encode(&tip.block_cid)))
            .unwrap_or(JsonValue::Null),
    );
    entry.insert(
        "block_path".into(),
        JsonValue::from(index_path_string(root, &block_path)),
    );
    entry.insert("published_at_unix".into(), JsonValue::from(timestamp));
    blocks.push(JsonValue::Object(entry));

    rebuild_runtime_dag_index(root, signer, index, blocks, &head, &head_path, &index_path)
}

fn read_runtime_dag_index(
    root: &Path,
    signer: &GovernanceRuntimeDagSigner,
    index_path: &Path,
) -> Result<JsonMap, GovernancePublishError> {
    match read_bounded_governance_state_file(index_path, GOVERNANCE_MUTABLE_INDEX_MAX_BYTES) {
        Ok(bytes) => {
            let value: JsonValue = json::from_slice(&bytes).map_err(|err| {
                GovernancePublishError::other(format!(
                    "failed to parse governance runtime DAG index `{}`: {err}",
                    index_path.display()
                ))
            })?;
            let JsonValue::Object(map) = value else {
                return Err(GovernancePublishError::other(
                    "governance runtime DAG index root is not an object",
                ));
            };
            if map.get("schema").and_then(JsonValue::as_str)
                != Some(GOVERNANCE_RUNTIME_DAG_INDEX_SCHEMA)
            {
                return Err(GovernancePublishError::other(
                    "governance runtime DAG index uses an unsupported schema",
                ));
            }
            validate_runtime_dag_signer_fields(&map, signer)?;
            Ok(map)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            let mut map = JsonMap::new();
            map.insert(
                "schema".into(),
                JsonValue::from(GOVERNANCE_RUNTIME_DAG_INDEX_SCHEMA),
            );
            map.insert(
                "source".into(),
                JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
            );
            map.insert("root".into(), JsonValue::from(root.display().to_string()));
            insert_runtime_dag_signer_fields(&mut map, signer);
            map.insert("blocks".into(), JsonValue::Array(Vec::new()));
            Ok(map)
        }
        Err(err) => Err(err.into()),
    }
}

fn validate_runtime_dag_signer_fields(
    index: &JsonMap,
    signer: &GovernanceRuntimeDagSigner,
) -> Result<(), GovernancePublishError> {
    let handle = index
        .get("signer_handle")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            GovernancePublishError::other("governance runtime DAG index is missing `signer_handle`")
        })?;
    if handle != signer.handle {
        return Err(GovernancePublishError::other(
            "governance runtime DAG index signer handle does not match configured signer",
        ));
    }
    let expected_peer = signer.publisher_peer_id_hex();
    let expected_public_key = signer.publisher_public_key_hex();
    let peer = index
        .get("publisher_peer_id_hex")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG index is missing `publisher_peer_id_hex`",
            )
        })?;
    if peer != expected_peer {
        return Err(GovernancePublishError::other(
            "governance runtime DAG index publisher peer id does not match configured signer",
        ));
    }
    let public_key = index
        .get("publisher_public_key_hex")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG index is missing `publisher_public_key_hex`",
            )
        })?;
    if public_key != expected_public_key {
        return Err(GovernancePublishError::other(
            "governance runtime DAG index publisher public key does not match configured signer",
        ));
    }
    Ok(())
}

fn insert_runtime_dag_signer_fields(index: &mut JsonMap, signer: &GovernanceRuntimeDagSigner) {
    index.insert(
        "signer_handle".into(),
        JsonValue::from(signer.handle.clone()),
    );
    index.insert(
        "publisher_peer_id".into(),
        JsonValue::from(String::from_utf8_lossy(&signer.publisher_peer_id).to_string()),
    );
    index.insert(
        "publisher_peer_id_hex".into(),
        JsonValue::from(signer.publisher_peer_id_hex()),
    );
    index.insert(
        "publisher_public_key_hex".into(),
        JsonValue::from(signer.publisher_public_key_hex()),
    );
}

fn runtime_dag_tip_from_entries(
    blocks: &[JsonValue],
) -> Result<Option<RuntimeDagTip>, GovernancePublishError> {
    let Some(last) = blocks.last() else {
        return Ok(None);
    };
    let Some(map) = last.as_object() else {
        return Err(GovernancePublishError::other(
            "governance runtime DAG index block entry is not an object",
        ));
    };
    Ok(Some(RuntimeDagTip {
        sequence: required_runtime_u64(map, "sequence")?,
        block_cid: required_runtime_hex(map, "block_cid_hex")?,
        node_cid: required_runtime_hex(map, "node_cid_hex")?,
    }))
}

fn runtime_dag_checkpoint_cid(
    blocks: &[JsonValue],
    block_count: u64,
) -> Result<Option<Vec<u8>>, GovernancePublishError> {
    let window = u64::try_from(GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1)
        .expect("governance DAG checkpoint window fits u64");
    if block_count <= window {
        return Ok(None);
    }
    let checkpoint_sequence = block_count.checked_sub(window).ok_or_else(|| {
        GovernancePublishError::other(
            "governance runtime DAG checkpoint block count is smaller than its window",
        )
    })?;
    let checkpoint_position = usize::try_from(checkpoint_sequence).map_err(|_| {
        GovernancePublishError::other(
            "governance runtime DAG checkpoint sequence exceeds host limits",
        )
    })?;
    let checkpoint_entry = blocks.get(checkpoint_position).ok_or_else(|| {
        GovernancePublishError::other(
            "governance runtime DAG index is missing the checkpoint window root",
        )
    })?;
    let checkpoint_map = checkpoint_entry.as_object().ok_or_else(|| {
        GovernancePublishError::other(
            "governance runtime DAG checkpoint index entry is not an object",
        )
    })?;
    if required_runtime_u64(checkpoint_map, "sequence")? != checkpoint_sequence {
        return Err(GovernancePublishError::other(
            "governance runtime DAG checkpoint sequence does not match its index position",
        ));
    }
    required_runtime_hex(checkpoint_map, "block_cid_hex").map(Some)
}

fn rebuild_runtime_dag_index(
    root: &Path,
    signer: &GovernanceRuntimeDagSigner,
    mut index: JsonMap,
    mut blocks: Vec<JsonValue>,
    head: &GovernanceDagHeadV1,
    head_path: &Path,
    index_path: &Path,
) -> Result<(), GovernancePublishError> {
    let mut by_encoded_blake3 = JsonMap::new();
    let mut by_source_payload_blake3 = JsonMap::new();
    let mut by_payload_kind = JsonMap::new();
    let mut previous_block_cid_hex: Option<String> = None;
    let mut previous_node_cid_hex: Option<String> = None;

    for (position, block) in blocks.iter_mut().enumerate() {
        let position_u64 = u64::try_from(position).map_err(|_| {
            GovernancePublishError::other("governance runtime DAG index position exceeds u64")
        })?;
        let Some(block_map) = block.as_object_mut() else {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index block entry is not an object",
            ));
        };
        block_map.insert("position".into(), JsonValue::from(position_u64));
        let sequence = required_runtime_u64(block_map, "sequence")?;
        if sequence != position_u64 {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index sequence does not match block position",
            ));
        }
        let payload_kind = required_runtime_string(block_map, "payload_kind")?;
        append_runtime_index_position(&mut by_payload_kind, &payload_kind, position_u64);
        let encoded_blake3 = required_runtime_string(block_map, "encoded_blake3")?;
        append_runtime_index_position(&mut by_encoded_blake3, &encoded_blake3, position_u64);
        let source_payload_blake3 = required_runtime_string(block_map, "source_payload_blake3")?;
        append_runtime_index_position(
            &mut by_source_payload_blake3,
            &source_payload_blake3,
            position_u64,
        );
        let block_cid_hex = required_runtime_string(block_map, "block_cid_hex")?;
        let node_cid_hex = required_runtime_string(block_map, "node_cid_hex")?;
        let prev_block_cid_hex = optional_runtime_string(block_map, "prev_block_cid_hex")?;
        let prev_node_cid_hex = optional_runtime_string(block_map, "prev_node_cid_hex")?;
        if prev_block_cid_hex != previous_block_cid_hex
            || prev_node_cid_hex != previous_node_cid_hex
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index parent links are inconsistent",
            ));
        }
        previous_block_cid_hex = Some(block_cid_hex);
        previous_node_cid_hex = Some(node_cid_hex);
    }

    index.insert(
        "schema".into(),
        JsonValue::from(GOVERNANCE_RUNTIME_DAG_INDEX_SCHEMA),
    );
    index.insert(
        "source".into(),
        JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
    );
    index.insert("root".into(), JsonValue::from(root.display().to_string()));
    index.insert(
        "generated_at".into(),
        JsonValue::from(current_unix_timestamp_seconds()),
    );
    insert_runtime_dag_signer_fields(&mut index, signer);
    index.insert(
        "head_block_cid_hex".into(),
        JsonValue::from(hex::encode(&head.head_block_cid)),
    );
    index.insert(
        "head_generated_at".into(),
        JsonValue::from(head.generated_at),
    );
    index.insert(
        "head_path".into(),
        JsonValue::from(index_path_string(root, head_path)),
    );
    index.insert("block_count".into(), JsonValue::from(head.block_count));
    index.insert(
        "by_encoded_blake3".into(),
        JsonValue::Object(by_encoded_blake3),
    );
    index.insert(
        "by_source_payload_blake3".into(),
        JsonValue::Object(by_source_payload_blake3),
    );
    index.insert("by_payload_kind".into(), JsonValue::Object(by_payload_kind));
    index.insert("blocks".into(), JsonValue::Array(blocks));

    let body = json::to_json_pretty(&JsonValue::Object(index)).map_err(|err| {
        GovernancePublishError::other(format!("serialize governance runtime DAG index: {err}"))
    })?;
    write_atomic(index_path, body.as_bytes())?;
    write_digest_sidecar(index_path, body.as_bytes())?;
    record_governance_dag_head_age(head.generated_at);
    Ok(())
}

fn append_runtime_index_position(index: &mut JsonMap, key: &str, position: u64) {
    let position = JsonValue::from(position);
    match index.get_mut(key).and_then(JsonValue::as_array_mut) {
        Some(positions) => positions.push(position),
        None => {
            index.insert(key.to_owned(), JsonValue::Array(vec![position]));
        }
    }
}

fn runtime_dag_index_entry_files_exist(root: &Path, entry: &JsonValue) -> bool {
    entry
        .get("block_path")
        .and_then(JsonValue::as_str)
        .and_then(|path| resolve_index_path(root, path).ok())
        .is_some_and(|path| path.is_file())
}

fn runtime_dag_block_path(root: &Path, sequence: u64, block_cid_hex: &str) -> PathBuf {
    root.join(GOVERNANCE_RUNTIME_DAG_DIR)
        .join(GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR)
        .join(format!("{sequence:020}_{block_cid_hex}.to"))
}

fn runtime_dag_head_path(root: &Path) -> PathBuf {
    root.join(GOVERNANCE_RUNTIME_DAG_DIR)
        .join(GOVERNANCE_RUNTIME_DAG_HEAD_FILE)
}

fn empty_governance_ed25519_signature() -> GovernanceLogSignatureV1 {
    GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: Vec::new(),
        signature: Vec::new(),
    }
}

fn required_runtime_string(map: &JsonMap, field: &str) -> Result<String, GovernancePublishError> {
    map.get(field)
        .and_then(JsonValue::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            GovernancePublishError::other(format!(
                "governance runtime DAG index entry is missing `{field}`"
            ))
        })
}

fn optional_runtime_string(
    map: &JsonMap,
    field: &str,
) -> Result<Option<String>, GovernancePublishError> {
    match map.get(field) {
        Some(JsonValue::Null) | None => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| {
                GovernancePublishError::other(format!(
                    "governance runtime DAG index entry field `{field}` is not a string or null"
                ))
            }),
    }
}

fn required_runtime_u64(map: &JsonMap, field: &str) -> Result<u64, GovernancePublishError> {
    map.get(field).and_then(JsonValue::as_u64).ok_or_else(|| {
        GovernancePublishError::other(format!(
            "governance runtime DAG index entry is missing `{field}`"
        ))
    })
}

fn required_runtime_hex(map: &JsonMap, field: &str) -> Result<Vec<u8>, GovernancePublishError> {
    let value = required_runtime_string(map, field)?;
    if value.is_empty() {
        return Err(GovernancePublishError::other(format!(
            "governance runtime DAG index entry field `{field}` is empty"
        )));
    }
    hex::decode(&value).map_err(|err| {
        GovernancePublishError::other(format!(
            "governance runtime DAG index entry field `{field}` is not hex: {err}"
        ))
    })
}

fn record_governance_dag_publish_result(
    payload_kind: &str,
    result: &Result<(), GovernancePublishError>,
    encoded_len: usize,
) {
    let Some(metrics) = iroha_telemetry::metrics::global() else {
        return;
    };
    let result_label = if result.is_ok() { "success" } else { "failure" };
    let encoded_len = u64::try_from(encoded_len).unwrap_or(u64::MAX);
    metrics.record_sorafs_governance_dag_publish(
        payload_kind,
        result_label,
        GOVERNANCE_DAG_SINK_FILESYSTEM,
        encoded_len,
        current_unix_timestamp_seconds(),
    );
}

fn record_governance_dag_backlog(pending_count: u64) {
    let Some(metrics) = iroha_telemetry::metrics::global() else {
        return;
    };
    metrics.set_sorafs_governance_dag_backlog(GOVERNANCE_DAG_SINK_FILESYSTEM, pending_count);
}

fn record_governance_dag_head_age_from_index(index: &JsonMap) {
    if let Some(generated_at) = governance_dag_head_generated_at_from_index(index) {
        record_governance_dag_head_age(generated_at);
    }
}

fn governance_dag_head_generated_at_from_index(index: &JsonMap) -> Option<u64> {
    index
        .get("head_generated_at")
        .and_then(JsonValue::as_u64)
        .or_else(|| index.get("generated_at").and_then(JsonValue::as_u64))
}

fn record_governance_dag_head_age(generated_at: u64) {
    let Some(metrics) = iroha_telemetry::metrics::global() else {
        return;
    };
    metrics.set_sorafs_governance_dag_head_age_seconds(
        GOVERNANCE_DAG_SINK_FILESYSTEM,
        governance_dag_head_age_seconds(generated_at, current_unix_timestamp_seconds()),
    );
}

fn governance_dag_head_age_seconds(generated_at: u64, now: u64) -> u64 {
    now.saturating_sub(generated_at)
}

fn ensure_canonical_governance_encoding<T: norito::NoritoSerialize>(
    value: &T,
    encoded: &[u8],
    payload_kind: &'static str,
) -> Result<(), GovernancePublishError> {
    let canonical = norito::to_bytes(value).map_err(|err| {
        GovernancePublishError::other(format!(
            "failed to canonically encode {payload_kind} before publication: {err}"
        ))
    })?;
    if canonical != encoded {
        return Err(GovernancePublishError::other(format!(
            "{payload_kind} publication bytes do not match the canonical header-bearing Norito payload"
        )));
    }
    Ok(())
}

impl GovernancePublisher for FilesystemGovernancePublisher {
    fn publish_deal_settlement(
        &self,
        settlement: &DealSettlementV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(settlement, encoded, "deal settlement")?;
            settlement.validate().map_err(|err| {
                GovernancePublishError::other(format!("invalid deal settlement: {err}"))
            })?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let base_path = self.base_path(settlement, &digest_hex);

            let encoded_path = base_path.with_extension("to");
            write_atomic(&encoded_path, encoded)?;
            write_digest_sidecar(&encoded_path, encoded)?;

            let mut settlement_obj = JsonMap::new();
            settlement_obj.insert("version".into(), JsonValue::from(settlement.version as u64));
            settlement_obj.insert(
                "deal_id".into(),
                JsonValue::from(settlement.deal_id.encode_hex::<String>()),
            );
            settlement_obj.insert(
                "settlement_id".into(),
                JsonValue::from(settlement.settlement_id.encode_hex::<String>()),
            );
            settlement_obj.insert(
                "ledger_snapshot_id".into(),
                JsonValue::from(settlement.ledger.snapshot_id.encode_hex::<String>()),
            );
            settlement_obj.insert(
                "ledger_sequence".into(),
                JsonValue::from(settlement.ledger.sequence),
            );
            settlement_obj.insert(
                "provider_id".into(),
                JsonValue::from(settlement.ledger.provider_id.encode_hex::<String>()),
            );
            settlement_obj.insert(
                "client_id".into(),
                JsonValue::from(settlement.ledger.client_id.encode_hex::<String>()),
            );
            settlement_obj.insert(
                "status".into(),
                JsonValue::from(status_label(settlement.status)),
            );
            settlement_obj.insert("settled_at".into(), JsonValue::from(settlement.settled_at));
            settlement_obj.insert(
                "ledger_captured_at".into(),
                JsonValue::from(settlement.ledger.captured_at),
            );
            settlement_obj.insert(
                "window_start_epoch".into(),
                JsonValue::from(settlement.ledger.window_start_epoch),
            );
            settlement_obj.insert(
                "window_end_epoch".into(),
                JsonValue::from(settlement.ledger.window_end_epoch),
            );
            settlement_obj.insert(
                "settlement_window_epochs".into(),
                JsonValue::from(settlement.ledger.settlement_window_epochs),
            );
            settlement_obj.insert(
                "provider_accrual".into(),
                JsonValue::from(settlement.ledger.provider_accrual.to_string()),
            );
            settlement_obj.insert(
                "client_liability".into(),
                JsonValue::from(settlement.ledger.client_liability.to_string()),
            );
            settlement_obj.insert(
                "outstanding_liability".into(),
                JsonValue::from(settlement.ledger.outstanding_liability.to_string()),
            );
            settlement_obj.insert(
                "bond_total".into(),
                JsonValue::from(settlement.ledger.bond_total.to_string()),
            );
            settlement_obj.insert(
                "bond_locked".into(),
                JsonValue::from(settlement.ledger.bond_locked.to_string()),
            );
            settlement_obj.insert(
                "bond_slashed".into(),
                JsonValue::from(settlement.ledger.bond_slashed.to_string()),
            );
            settlement_obj.insert(
                "bond_released".into(),
                JsonValue::from(settlement.ledger.bond_released.to_string()),
            );
            if let Some(notes) = &settlement.audit_notes {
                settlement_obj.insert("audit_notes".into(), JsonValue::from(notes.clone()));
            }

            let mut payload = JsonMap::new();
            payload.insert("settlement".into(), JsonValue::Object(settlement_obj));

            let mut metadata = JsonMap::new();
            metadata.insert(
                "status".into(),
                JsonValue::from(status_label(settlement.status)),
            );
            metadata.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
            metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
            metadata.insert(
                "encoded_base64".into(),
                JsonValue::from(BASE64_STANDARD.encode(encoded)),
            );
            payload.insert("metadata".into(), JsonValue::Object(metadata));

            let json_body = json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
                GovernancePublishError::other(format!("serialize settlement json: {err}"))
            })?;

            let json_path = base_path.with_extension("json");
            write_atomic(&json_path, json_body.as_bytes())?;
            write_digest_sidecar(&json_path, json_body.as_bytes())?;
            let mut labels = JsonMap::new();
            labels.insert(
                "deal_id".into(),
                JsonValue::from(settlement.deal_id.encode_hex::<String>()),
            );
            labels.insert(
                "provider_id".into(),
                JsonValue::from(settlement.ledger.provider_id.encode_hex::<String>()),
            );
            labels.insert(
                "client_id".into(),
                JsonValue::from(settlement.ledger.client_id.encode_hex::<String>()),
            );
            labels.insert(
                "status".into(),
                JsonValue::from(status_label(settlement.status)),
            );
            labels.insert("settled_at".into(), JsonValue::from(settlement.settled_at));
            self.record_publish_index(
                "deal_settlement",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;
            self.record_runtime_signed_payload(
                "deal_settlement",
                GovernanceLogPayloadV1::DealSettlement(Box::new(settlement.clone())),
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;

            Ok(())
        })();
        record_governance_dag_publish_result("deal_settlement", &result, encoded.len());
        result
    }

    fn publish_pdp_archive(
        &self,
        archive: &PdpGovernanceArchiveV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(archive, encoded, "PDP governance archive")?;
            archive.validate().map_err(|error| {
                GovernancePublishError::other(format!("invalid PDP governance archive: {error}"))
            })?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let base_path = self.pdp_archive_root().join(format!(
                "{:020}-{}-{}-{digest_hex}",
                archive.epoch_id,
                hex::encode(archive.provider_id),
                hex::encode(archive.challenge_id),
            ));

            let encoded_path = base_path.with_extension("to");
            write_atomic(&encoded_path, encoded)?;
            write_digest_sidecar(&encoded_path, encoded)?;

            let decision = pdp_decision_label(archive.decision);
            let mut payload = JsonMap::new();
            payload.insert(
                "version".into(),
                JsonValue::from(u64::from(archive.version)),
            );
            payload.insert("sequence".into(), JsonValue::from(archive.sequence));
            payload.insert(
                "challenge_id_hex".into(),
                JsonValue::from(hex::encode(archive.challenge_id)),
            );
            payload.insert(
                "commitment_digest_hex".into(),
                JsonValue::from(hex::encode(archive.commitment_digest)),
            );
            payload.insert(
                "manifest_digest_hex".into(),
                JsonValue::from(hex::encode(archive.manifest_digest)),
            );
            payload.insert(
                "provider_id_hex".into(),
                JsonValue::from(hex::encode(archive.provider_id)),
            );
            payload.insert("epoch_id".into(), JsonValue::from(archive.epoch_id));
            payload.insert("decision".into(), JsonValue::from(decision));
            payload.insert(
                "proof_digest_hex".into(),
                archive
                    .proof_digest
                    .map(hex::encode)
                    .map_or(JsonValue::Null, JsonValue::from),
            );
            payload.insert(
                "sampled_segments".into(),
                JsonValue::from(u64::from(archive.sampled_segments)),
            );
            payload.insert(
                "sampled_hot_leaves".into(),
                JsonValue::from(u64::from(archive.sampled_hot_leaves)),
            );
            payload.insert(
                "sampled_bytes".into(),
                JsonValue::from(archive.sampled_bytes),
            );
            payload.insert(
                "issued_at_unix".into(),
                JsonValue::from(archive.issued_at_unix),
            );
            payload.insert(
                "response_deadline_unix".into(),
                JsonValue::from(archive.response_deadline_unix),
            );
            payload.insert(
                "decided_at_unix".into(),
                JsonValue::from(archive.decided_at_unix),
            );
            payload.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
            let json_body = json::to_json_pretty(&JsonValue::Object(payload)).map_err(|error| {
                GovernancePublishError::other(format!("serialize PDP archive json: {error}"))
            })?;
            let json_path = base_path.with_extension("json");
            write_atomic(&json_path, json_body.as_bytes())?;
            write_digest_sidecar(&json_path, json_body.as_bytes())?;

            let mut labels = JsonMap::new();
            labels.insert(
                "challenge_id_hex".into(),
                JsonValue::from(hex::encode(archive.challenge_id)),
            );
            labels.insert(
                "manifest_digest_hex".into(),
                JsonValue::from(hex::encode(archive.manifest_digest)),
            );
            labels.insert(
                "provider_id_hex".into(),
                JsonValue::from(hex::encode(archive.provider_id)),
            );
            labels.insert("epoch_id".into(), JsonValue::from(archive.epoch_id));
            labels.insert("decision".into(), JsonValue::from(decision));
            labels.insert("sequence".into(), JsonValue::from(archive.sequence));
            self.record_publish_index(
                "pdp_archive",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;
            self.record_runtime_signed_payload(
                "pdp_archive",
                GovernanceLogPayloadV1::PdpArchive(archive.clone()),
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;
            Ok(())
        })();
        record_governance_dag_publish_result("pdp_archive", &result, encoded.len());
        result
    }

    fn publish_por_challenge_publication(
        &self,
        publication: &PorChallengePublicationV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(
                publication,
                encoded,
                "PoR challenge publication",
            )?;
            publication.validate().map_err(|error| {
                GovernancePublishError::other(format!("invalid PoR challenge publication: {error}"))
            })?;
            let challenge = &publication.challenge;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let base_path = self
                .por_challenge_root()
                .join(format!("{:020}", challenge.epoch_id))
                .join(hex::encode(challenge.challenge_id));

            let encoded_path = base_path.with_extension("to");
            write_atomic(&encoded_path, encoded)?;
            write_digest_sidecar(&encoded_path, encoded)?;

            let mut payload = JsonMap::new();
            payload.insert(
                "publication".into(),
                json::to_value(publication).map_err(|error| {
                    GovernancePublishError::other(format!(
                        "serialize PoR challenge publication json: {error}"
                    ))
                })?,
            );
            payload.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
            let json_body = json::to_json_pretty(&JsonValue::Object(payload)).map_err(|error| {
                GovernancePublishError::other(format!(
                    "serialize PoR challenge publication json: {error}"
                ))
            })?;
            let json_path = base_path.with_extension("json");
            write_atomic(&json_path, json_body.as_bytes())?;
            write_digest_sidecar(&json_path, json_body.as_bytes())?;

            let mut labels = JsonMap::new();
            labels.insert(
                "challenge_id_hex".into(),
                JsonValue::from(hex::encode(challenge.challenge_id)),
            );
            labels.insert(
                "manifest_digest_hex".into(),
                JsonValue::from(hex::encode(challenge.manifest_digest)),
            );
            labels.insert(
                "provider_id_hex".into(),
                JsonValue::from(hex::encode(challenge.provider_id)),
            );
            labels.insert("epoch_id".into(), JsonValue::from(challenge.epoch_id));
            labels.insert(
                "duplicate_samples".into(),
                JsonValue::from(u64::from(publication.duplicate_samples)),
            );
            labels.insert("forced".into(), JsonValue::from(challenge.forced));
            self.record_publish_index(
                "por_challenge_publication",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;
            self.record_runtime_signed_payload(
                "por_challenge_publication",
                GovernanceLogPayloadV1::PorChallengePublication(publication.clone()),
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;
            Ok(())
        })();
        record_governance_dag_publish_result("por_challenge_publication", &result, encoded.len());
        result
    }

    fn publish_por_weekly_report(
        &self,
        report: &PorWeeklyReportV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(report, encoded, "PoR weekly report")?;
            report.validate().map_err(|error| {
                GovernancePublishError::other(format!("invalid PoR weekly report: {error}"))
            })?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let base_path = self.por_report_root().join(format!(
                "{:04}-W{:02}_{:020}_{}",
                report.cycle.year,
                report.cycle.week,
                report.generated_at,
                &digest_hex[..16],
            ));

            let encoded_path = base_path.with_extension("to");
            write_atomic(&encoded_path, encoded)?;
            write_digest_sidecar(&encoded_path, encoded)?;

            let mut payload = JsonMap::new();
            payload.insert(
                "report".into(),
                json::to_value(report).map_err(|error| {
                    GovernancePublishError::other(format!(
                        "serialize PoR weekly report json: {error}"
                    ))
                })?,
            );
            payload.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
            let json_body = json::to_json_pretty(&JsonValue::Object(payload)).map_err(|error| {
                GovernancePublishError::other(format!("serialize PoR weekly report json: {error}"))
            })?;
            let json_path = base_path.with_extension("json");
            write_atomic(&json_path, json_body.as_bytes())?;
            write_digest_sidecar(&json_path, json_body.as_bytes())?;

            let mut labels = JsonMap::new();
            labels.insert("cycle".into(), JsonValue::from(report.cycle.to_string()));
            labels.insert("generated_at".into(), JsonValue::from(report.generated_at));
            labels.insert(
                "challenges_total".into(),
                JsonValue::from(u64::from(report.challenges_total)),
            );
            labels.insert(
                "challenges_failed".into(),
                JsonValue::from(u64::from(report.challenges_failed)),
            );
            labels.insert(
                "forced_challenges".into(),
                JsonValue::from(u64::from(report.forced_challenges)),
            );
            self.record_publish_index(
                "por_weekly_report",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;
            self.record_runtime_signed_payload(
                "por_weekly_report",
                GovernanceLogPayloadV1::PorWeeklyReport(report.clone()),
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;
            Ok(())
        })();
        record_governance_dag_publish_result("por_weekly_report", &result, encoded.len());
        result
    }

    fn publish_gc_audit_event(
        &self,
        event: &GcAuditEventV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(event, encoded, "GC audit event")?;
            event.validate().map_err(|err| {
                GovernancePublishError::other(format!("invalid GC audit event: {err}"))
            })?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let base_path = self.gc_audit_path(event, &digest_hex);

            let encoded_path = base_path.with_extension("to");
            write_atomic(&encoded_path, encoded)?;
            write_digest_sidecar(&encoded_path, encoded)?;

            let mut payload = JsonMap::new();
            payload.insert(
                "event".into(),
                json::to_value(event).map_err(|err| {
                    GovernancePublishError::other(format!("serialize gc event: {err}"))
                })?,
            );

            let mut metadata = JsonMap::new();
            metadata.insert(
                "reason".into(),
                JsonValue::from(event.payload.reason.clone()),
            );
            if let Some(blocked) = &event.payload.blocked_reason {
                metadata.insert("blocked_reason".into(), JsonValue::from(blocked.clone()));
            }
            metadata.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
            metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
            metadata.insert(
                "encoded_base64".into(),
                JsonValue::from(BASE64_STANDARD.encode(encoded)),
            );
            payload.insert("metadata".into(), JsonValue::Object(metadata));

            let json_body = json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
                GovernancePublishError::other(format!("serialize gc audit json: {err}"))
            })?;

            let json_path = base_path.with_extension("json");
            write_atomic(&json_path, json_body.as_bytes())?;
            write_digest_sidecar(&json_path, json_body.as_bytes())?;
            let mut labels = JsonMap::new();
            labels.insert(
                "manifest".into(),
                JsonValue::from(hex::encode(event.payload.manifest_digest)),
            );
            labels.insert(
                "provider".into(),
                JsonValue::from(hex::encode(event.payload.provider_id)),
            );
            labels.insert(
                "reason".into(),
                JsonValue::from(event.payload.reason.clone()),
            );
            labels.insert("sequence".into(), JsonValue::from(event.header.sequence));
            labels.insert(
                "evicted_at_unix".into(),
                JsonValue::from(event.payload.evicted_at_unix),
            );
            self.record_publish_index(
                "gc_audit",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;
            let external = GovernanceExternalPayloadV1::from_gc_audit(event, encoded)
                .map_err(|err| GovernancePublishError::other(err.to_string()))?;
            self.record_runtime_signed_payload(
                "gc_audit",
                GovernanceLogPayloadV1::ExternalPayload(external),
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;

            Ok(())
        })();
        record_governance_dag_publish_result("gc_audit", &result, encoded.len());
        result
    }

    fn publish_reconciliation_report(
        &self,
        report: &SorafsReconciliationReportV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(report, encoded, "reconciliation report")?;
            report.validate().map_err(|err| {
                GovernancePublishError::other(format!("invalid reconciliation report: {err}"))
            })?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let base_path = self.reconciliation_path(report, &digest_hex);

            let encoded_path = base_path.with_extension("to");
            write_atomic(&encoded_path, encoded)?;
            write_digest_sidecar(&encoded_path, encoded)?;

            let mut payload = JsonMap::new();
            payload.insert(
                "report".into(),
                json::to_value(report).map_err(|err| {
                    GovernancePublishError::other(format!("serialize reconciliation report: {err}"))
                })?,
            );

            let mut metadata = JsonMap::new();
            metadata.insert(
                "provider".into(),
                JsonValue::from(hex::encode(report.provider_id)),
            );
            metadata.insert(
                "generated_at_unix".into(),
                JsonValue::from(report.generated_at_unix),
            );
            metadata.insert(
                "repair_snapshot_hash".into(),
                JsonValue::from(hex::encode(report.repair_snapshot_hash)),
            );
            metadata.insert(
                "retention_snapshot_hash".into(),
                JsonValue::from(hex::encode(report.retention_snapshot_hash)),
            );
            metadata.insert(
                "gc_snapshot_hash".into(),
                JsonValue::from(hex::encode(report.gc_snapshot_hash)),
            );
            metadata.insert(
                "divergence_count".into(),
                JsonValue::from(report.divergence_count as u64),
            );
            if let Some(appeal_finance) = &report.appeal_finance {
                metadata.insert(
                    "appeal_finance_rollup_snapshot_hash".into(),
                    JsonValue::from(hex::encode(appeal_finance.rollup_snapshot_hash)),
                );
                metadata.insert(
                    "appeal_finance_rollup_count".into(),
                    JsonValue::from(u64::from(appeal_finance.rollup_count)),
                );
                metadata.insert(
                    "appeal_finance_source_report_count".into(),
                    JsonValue::from(appeal_finance.source_report_count),
                );
                metadata.insert(
                    "appeal_finance_case_count".into(),
                    JsonValue::from(appeal_finance.case_count),
                );
                metadata.insert(
                    "appeal_finance_total_treasury_xor".into(),
                    JsonValue::from(appeal_finance.total_treasury_xor.to_string()),
                );
                metadata.insert(
                    "appeal_finance_total_rewards_forfeited_treasury_xor".into(),
                    JsonValue::from(
                        appeal_finance
                            .total_rewards_forfeited_treasury_xor
                            .to_string(),
                    ),
                );
            }
            metadata.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
            metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
            metadata.insert(
                "encoded_base64".into(),
                JsonValue::from(BASE64_STANDARD.encode(encoded)),
            );
            payload.insert("metadata".into(), JsonValue::Object(metadata));

            let json_body = json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
                GovernancePublishError::other(format!(
                    "serialize reconciliation report json: {err}"
                ))
            })?;

            let json_path = base_path.with_extension("json");
            write_atomic(&json_path, json_body.as_bytes())?;
            write_digest_sidecar(&json_path, json_body.as_bytes())?;
            let mut labels = JsonMap::new();
            labels.insert(
                "provider".into(),
                JsonValue::from(hex::encode(report.provider_id)),
            );
            labels.insert(
                "generated_at_unix".into(),
                JsonValue::from(report.generated_at_unix),
            );
            labels.insert(
                "divergence_count".into(),
                JsonValue::from(report.divergence_count as u64),
            );
            if let Some(appeal_finance) = &report.appeal_finance {
                labels.insert(
                    "appeal_finance_rollup_count".into(),
                    JsonValue::from(u64::from(appeal_finance.rollup_count)),
                );
                labels.insert(
                    "appeal_finance_source_report_count".into(),
                    JsonValue::from(appeal_finance.source_report_count),
                );
                labels.insert(
                    "appeal_finance_total_treasury_xor".into(),
                    JsonValue::from(appeal_finance.total_treasury_xor.to_string()),
                );
            }
            self.record_publish_index(
                "reconciliation",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;
            let external = GovernanceExternalPayloadV1::from_reconciliation(report, encoded)
                .map_err(|err| GovernancePublishError::other(err.to_string()))?;
            self.record_runtime_signed_payload(
                "reconciliation",
                GovernanceLogPayloadV1::ExternalPayload(external),
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;

            Ok(())
        })();
        record_governance_dag_publish_result("reconciliation", &result, encoded.len());
        result
    }

    fn publish_reputation_snapshot(
        &self,
        envelope: &SignedReputationSnapshotV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            let canonical = envelope.canonical_bytes().map_err(|err| {
                GovernancePublishError::other(format!("invalid signed reputation snapshot: {err}"))
            })?;
            if canonical != encoded {
                return Err(GovernancePublishError::other(
                    "signed reputation snapshot bytes are not canonical",
                ));
            }
            let snapshot = &envelope.snapshot;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let base_path = self.reputation_snapshot_path(envelope, &digest_hex);

            let encoded_path = base_path.with_extension("to");
            write_atomic(&encoded_path, encoded)?;
            write_digest_sidecar(&encoded_path, encoded)?;

            let json_body = reputation_snapshot_json(envelope, encoded, &digest_hex)?;
            let json_path = base_path.with_extension("json");
            write_atomic(&json_path, json_body.as_bytes())?;
            write_digest_sidecar(&json_path, json_body.as_bytes())?;

            let latest_path = self.reputation_root().join("latest");
            let latest_encoded_path = latest_path.with_extension("to");
            write_atomic(&latest_encoded_path, encoded)?;
            write_digest_sidecar(&latest_encoded_path, encoded)?;
            let latest_json_path = latest_path.with_extension("json");
            write_atomic(&latest_json_path, json_body.as_bytes())?;
            write_digest_sidecar(&latest_json_path, json_body.as_bytes())?;
            let mut labels = JsonMap::new();
            labels.insert(
                "snapshot_id_hex".into(),
                JsonValue::from(hex::encode(snapshot.snapshot_id)),
            );
            labels.insert(
                "generated_at_unix".into(),
                JsonValue::from(snapshot.generated_at_unix),
            );
            labels.insert(
                "provider_count".into(),
                JsonValue::from(snapshot.providers.len() as u64),
            );
            labels.insert(
                "merkle_root_hex".into(),
                JsonValue::from(hex::encode(snapshot.merkle_root)),
            );
            labels.insert(
                "policy_digest_hex".into(),
                JsonValue::from(hex::encode(envelope.policy_digest)),
            );
            labels.insert(
                "scoring_evidence_digest_hex".into(),
                JsonValue::from(hex::encode(envelope.scoring_evidence_digest)),
            );
            labels.insert(
                "signature_count".into(),
                JsonValue::from(envelope.signatures.len() as u64),
            );
            self.record_publish_index(
                "reputation_snapshot",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;
            self.record_runtime_signed_payload(
                "reputation_snapshot",
                GovernanceLogPayloadV1::SignedReputationSnapshot(envelope.clone()),
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;

            Ok(())
        })();
        record_governance_dag_publish_result("reputation_snapshot", &result, encoded.len());
        result
    }

    fn publish_moderation_ballot_event(
        &self,
        event: &SoraFsModerationBallotGovernanceEventV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(event, encoded, "moderation ballot event")?;
            event.validate().map_err(|err| {
                GovernancePublishError::other(format!("invalid moderation ballot event: {err}"))
            })?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let base_path = self.moderation_ballot_event_path(event, &digest_hex);

            let encoded_path = base_path.with_extension("to");
            write_atomic(&encoded_path, encoded)?;
            write_digest_sidecar(&encoded_path, encoded)?;

            let json_body = moderation_ballot_event_json(event, encoded, &digest_hex)?;
            let json_path = base_path.with_extension("json");
            write_atomic(&json_path, json_body.as_bytes())?;
            write_digest_sidecar(&json_path, json_body.as_bytes())?;

            let mut labels = JsonMap::new();
            labels.insert("case_id".into(), JsonValue::from(event.case_id.clone()));
            labels.insert("round_id".into(), JsonValue::from(event.round_id.clone()));
            labels.insert("kind".into(), JsonValue::from(event.kind.as_str()));
            labels.insert("sequence".into(), JsonValue::from(event.sequence));
            labels.insert(
                "generated_at_unix_ms".into(),
                JsonValue::from(event.generated_at_unix_ms),
            );
            labels.insert(
                "committed_count".into(),
                JsonValue::from(event.committed_count),
            );
            labels.insert(
                "revealed_count".into(),
                JsonValue::from(event.revealed_count),
            );
            if let Some(juror_id) = &event.juror_id {
                labels.insert("juror_id".into(), JsonValue::from(juror_id.clone()));
            }
            if let Some(tally) = &event.tally {
                labels.insert(
                    "votes_total".into(),
                    JsonValue::from(u64::from(tally.votes_total)),
                );
                labels.insert("quorum".into(), JsonValue::from(u64::from(tally.quorum)));
                labels.insert("contested".into(), JsonValue::from(tally.contested));
                if let Some(choice) = tally.winning_choice {
                    labels.insert("winning_choice".into(), JsonValue::from(choice.as_str()));
                }
            }
            self.record_publish_index(
                "moderation_ballot_event",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;
            self.record_runtime_signed_payload(
                "moderation_ballot_event",
                GovernanceLogPayloadV1::ModerationBallotEvent(event.clone()),
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;

            Ok(())
        })();
        record_governance_dag_publish_result("moderation_ballot_event", &result, encoded.len());
        result
    }

    fn publish_transparency_ledger_publication(
        &self,
        publication: &ModerationLedgerCyclePublicationV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(
                publication,
                encoded,
                "transparency ledger publication",
            )?;
            publication.validate().map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid transparency ledger publication: {err}"
                ))
            })?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let base_path = self.transparency_ledger_publication_path(publication, &digest_hex);

            let encoded_path = base_path.with_extension("to");
            write_atomic(&encoded_path, encoded)?;
            write_digest_sidecar(&encoded_path, encoded)?;

            let json_body =
                transparency_ledger_publication_json(publication, encoded, &digest_hex)?;
            let json_path = base_path.with_extension("json");
            write_atomic(&json_path, json_body.as_bytes())?;
            write_digest_sidecar(&json_path, json_body.as_bytes())?;

            let block_hash = publication.block.block_hash().map_err(|err| {
                GovernancePublishError::other(format!("hash transparency ledger block: {err}"))
            })?;
            let publication_hash = publication.publication_hash().map_err(|err| {
                GovernancePublishError::other(format!(
                    "hash transparency ledger publication: {err}"
                ))
            })?;
            let mut labels = JsonMap::new();
            labels.insert(
                "cycle_id_hex".into(),
                JsonValue::from(hex::encode(publication.block.cycle_id)),
            );
            labels.insert(
                "cycle_start_unix".into(),
                JsonValue::from(publication.block.cycle_start_unix),
            );
            labels.insert(
                "cycle_end_unix".into(),
                JsonValue::from(publication.block.cycle_end_unix),
            );
            labels.insert(
                "generated_at_unix".into(),
                JsonValue::from(publication.block.generated_at_unix),
            );
            labels.insert(
                "entry_count".into(),
                JsonValue::from(publication.block.entry_count),
            );
            labels.insert(
                "entry_root_hex".into(),
                JsonValue::from(hex::encode(publication.block.entry_root)),
            );
            labels.insert(
                "block_hash_hex".into(),
                JsonValue::from(hex::encode(block_hash)),
            );
            labels.insert(
                "publication_hash_hex".into(),
                JsonValue::from(hex::encode(publication_hash)),
            );
            self.record_publish_index(
                "transparency_ledger_publication",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;
            let external = GovernanceExternalPayloadV1::from_transparency_ledger_publication(
                publication,
                encoded,
            )
            .map_err(|err| GovernancePublishError::other(err.to_string()))?;
            self.record_runtime_signed_payload(
                "transparency_ledger_publication",
                GovernanceLogPayloadV1::ExternalPayload(external),
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;

            Ok(())
        })();
        record_governance_dag_publish_result(
            "transparency_ledger_publication",
            &result,
            encoded.len(),
        );
        result
    }

    fn publish_proof_token_issuance(
        &self,
        issuance: &ProofTokenIssuanceV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(issuance, encoded, "proof-token issuance")?;
            issuance.validate().map_err(|err| {
                GovernancePublishError::other(format!("invalid proof-token issuance: {err}"))
            })?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let base_path = self.proof_token_issuance_path(issuance, &digest_hex);

            let encoded_path = base_path.with_extension("to");
            write_atomic(&encoded_path, encoded)?;
            write_digest_sidecar(&encoded_path, encoded)?;

            let json_body = proof_token_issuance_json(issuance, encoded, &digest_hex)?;
            let json_path = base_path.with_extension("json");
            write_atomic(&json_path, json_body.as_bytes())?;
            write_digest_sidecar(&json_path, json_body.as_bytes())?;

            let mut labels = JsonMap::new();
            labels.insert(
                "token_id_hex".into(),
                JsonValue::from(hex::encode(issuance.token_id)),
            );
            labels.insert(
                "issued_at_unix".into(),
                JsonValue::from(issuance.issued_at_unix),
            );
            if let Some(expires_at_unix) = issuance.expires_at_unix {
                labels.insert("expires_at_unix".into(), JsonValue::from(expires_at_unix));
            }
            labels.insert(
                "moderation_action_code".into(),
                JsonValue::from(u64::from(issuance.moderation_action_code)),
            );
            labels.insert(
                "signer_key_hex".into(),
                JsonValue::from(hex::encode(issuance.signer_key)),
            );
            labels.insert(
                "token_blake3_hex".into(),
                JsonValue::from(hex::encode(issuance.token_blake3)),
            );
            labels.insert(
                "blinded_digest_hex".into(),
                JsonValue::from(hex::encode(issuance.blinded_digest)),
            );
            labels.insert(
                "entry_count".into(),
                JsonValue::from(issuance.entry_ids.len() as u64),
            );
            if let Some(first_entry_id) = issuance.entry_ids.first() {
                labels.insert(
                    "first_entry_id".into(),
                    JsonValue::from(first_entry_id.clone()),
                );
            }
            if let Some(evidence_digest) = issuance.evidence_digest {
                labels.insert(
                    "evidence_digest_hex".into(),
                    JsonValue::from(hex::encode(evidence_digest)),
                );
            }
            if let Some(policy_digest) = issuance.policy_digest {
                labels.insert(
                    "policy_digest_hex".into(),
                    JsonValue::from(hex::encode(policy_digest)),
                );
            }
            self.record_publish_index(
                "proof_token_issuance",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;
            let external =
                GovernanceExternalPayloadV1::from_proof_token_issuance(issuance, encoded)
                    .map_err(|err| GovernancePublishError::other(err.to_string()))?;
            self.record_runtime_signed_payload(
                "proof_token_issuance",
                GovernanceLogPayloadV1::ExternalPayload(external),
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;

            Ok(())
        })();
        record_governance_dag_publish_result("proof_token_issuance", &result, encoded.len());
        result
    }

    fn publish_appeal_finance_report(
        &self,
        report: &SoraFsAppealFinanceReportV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(report, encoded, "appeal finance report")?;
            report.validate().map_err(|err| {
                GovernancePublishError::other(format!("invalid appeal finance report: {err}"))
            })?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let base_path = self.appeal_finance_report_path(report, &digest_hex);

            let encoded_path = base_path.with_extension("to");
            write_atomic(&encoded_path, encoded)?;
            write_digest_sidecar(&encoded_path, encoded)?;

            let json_body = appeal_finance_report_json(report, encoded, &digest_hex)?;
            let json_path = base_path.with_extension("json");
            write_atomic(&json_path, json_body.as_bytes())?;
            write_digest_sidecar(&json_path, json_body.as_bytes())?;

            let mut labels = JsonMap::new();
            labels.insert("case_id".into(), JsonValue::from(report.case_id.clone()));
            if let Some(round_id) = &report.round_id {
                labels.insert("round_id".into(), JsonValue::from(round_id.clone()));
            }
            labels.insert(
                "report_id_hex".into(),
                JsonValue::from(hex::encode(report.report_id)),
            );
            labels.insert("outcome".into(), JsonValue::from(report.outcome.as_str()));
            labels.insert(
                "generated_at_unix_ms".into(),
                JsonValue::from(report.generated_at_unix_ms),
            );
            labels.insert(
                "appeal_finance_config_version".into(),
                JsonValue::from(report.appeal_finance_config_version.clone()),
            );
            labels.insert(
                "deposit_xor".into(),
                JsonValue::from(report.deposit_xor.to_string()),
            );
            labels.insert(
                "refund_xor".into(),
                JsonValue::from(report.refund.amount_xor.to_string()),
            );
            labels.insert(
                "treasury_xor".into(),
                JsonValue::from(report.treasury.amount_xor.to_string()),
            );
            labels.insert(
                "held_xor".into(),
                JsonValue::from(report.held.amount_xor.to_string()),
            );
            labels.insert(
                "panel_size".into(),
                JsonValue::from(u64::from(report.panel_size)),
            );
            labels.insert(
                "panel_reward_total_xor".into(),
                JsonValue::from(report.panel_reward_total_xor.to_string()),
            );
            labels.insert(
                "rewards_paid_total_xor".into(),
                JsonValue::from(report.rewards_paid_total_xor.to_string()),
            );
            labels.insert(
                "rewards_forfeited_treasury_xor".into(),
                JsonValue::from(report.rewards_forfeited_treasury_xor.to_string()),
            );
            labels.insert(
                "juror_payout_count".into(),
                JsonValue::from(report.juror_payouts.len() as u64),
            );
            labels.insert(
                "no_show_count".into(),
                JsonValue::from(report.no_show_juror_ids.len() as u64),
            );
            self.record_publish_index(
                "appeal_finance_report",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;
            self.record_runtime_signed_payload(
                "appeal_finance_report",
                GovernanceLogPayloadV1::AppealFinanceReport(report.clone()),
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;

            Ok(())
        })();
        record_governance_dag_publish_result("appeal_finance_report", &result, encoded.len());
        result
    }

    fn publish_appeal_finance_weekly_rollup(
        &self,
        rollup: &SoraFsAppealFinanceWeeklyRollupV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(rollup, encoded, "appeal finance weekly rollup")?;
            rollup.validate().map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid appeal finance weekly rollup: {err}"
                ))
            })?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let base_path = self.appeal_finance_weekly_rollup_path(rollup, &digest_hex);

            let encoded_path = base_path.with_extension("to");
            write_atomic(&encoded_path, encoded)?;
            write_digest_sidecar(&encoded_path, encoded)?;

            let json_body = appeal_finance_weekly_rollup_json(rollup, encoded, &digest_hex)?;
            let json_path = base_path.with_extension("json");
            write_atomic(&json_path, json_body.as_bytes())?;
            write_digest_sidecar(&json_path, json_body.as_bytes())?;

            let mut labels = JsonMap::new();
            labels.insert("cycle".into(), JsonValue::from(rollup.cycle.to_string()));
            labels.insert(
                "generated_at_unix_ms".into(),
                JsonValue::from(rollup.generated_at_unix_ms),
            );
            labels.insert("report_count".into(), JsonValue::from(rollup.report_count));
            labels.insert("case_count".into(), JsonValue::from(rollup.case_count));
            labels.insert(
                "config_version_count".into(),
                JsonValue::from(rollup.appeal_finance_config_versions.len() as u64),
            );
            labels.insert(
                "outcome_count".into(),
                JsonValue::from(rollup.outcomes.len() as u64),
            );
            labels.insert(
                "juror_payout_count".into(),
                JsonValue::from(rollup.juror_payout_count),
            );
            labels.insert(
                "no_show_count".into(),
                JsonValue::from(rollup.no_show_juror_count),
            );
            labels.insert(
                "total_treasury_xor".into(),
                JsonValue::from(rollup.total_treasury_xor.to_string()),
            );
            labels.insert(
                "total_rewards_forfeited_treasury_xor".into(),
                JsonValue::from(rollup.total_rewards_forfeited_treasury_xor.to_string()),
            );
            self.record_publish_index(
                "appeal_finance_weekly_rollup",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;
            self.record_runtime_signed_payload(
                "appeal_finance_weekly_rollup",
                GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(rollup.clone()),
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;

            Ok(())
        })();
        record_governance_dag_publish_result(
            "appeal_finance_weekly_rollup",
            &result,
            encoded.len(),
        );
        result
    }

    fn publish_appeal_finance_settlement_receipt(
        &self,
        receipt: &SoraFsAppealFinanceSettlementReceiptV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let _publication_guard = self.lock_publication()?;
            ensure_canonical_governance_encoding(
                receipt,
                encoded,
                "appeal finance settlement receipt",
            )?;
            receipt.validate().map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid appeal finance settlement receipt: {err}"
                ))
            })?;
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let base_path = self.appeal_finance_settlement_receipt_path(receipt, &digest_hex);

            let encoded_path = base_path.with_extension("to");
            write_atomic(&encoded_path, encoded)?;
            write_digest_sidecar(&encoded_path, encoded)?;

            let json_body = appeal_finance_settlement_receipt_json(receipt, encoded, &digest_hex)?;
            let json_path = base_path.with_extension("json");
            write_atomic(&json_path, json_body.as_bytes())?;
            write_digest_sidecar(&json_path, json_body.as_bytes())?;

            let mut labels = JsonMap::new();
            labels.insert("case_id".into(), JsonValue::from(receipt.case_id.clone()));
            if let Some(round_id) = &receipt.round_id {
                labels.insert("round_id".into(), JsonValue::from(round_id.clone()));
            }
            labels.insert(
                "receipt_id_hex".into(),
                JsonValue::from(hex::encode(receipt.receipt_id)),
            );
            labels.insert(
                "generated_at_unix_ms".into(),
                JsonValue::from(receipt.generated_at_unix_ms),
            );
            labels.insert(
                "appeal_finance_config_version".into(),
                JsonValue::from(receipt.appeal_finance_config_version.clone()),
            );
            labels.insert(
                "appeal_finance_policy_digest_hex".into(),
                JsonValue::from(hex::encode(receipt.appeal_finance_policy_digest)),
            );
            labels.insert("outcome".into(), JsonValue::from(receipt.outcome.as_str()));
            labels.insert(
                "escrow_id_hex".into(),
                JsonValue::from(receipt.escrow_id_hex.clone()),
            );
            labels.insert(
                "submitted_step".into(),
                JsonValue::from(receipt.submitted_step.clone()),
            );
            labels.insert(
                "required_authority".into(),
                JsonValue::from(receipt.required_authority.clone()),
            );
            labels.insert(
                "tx_hash_hex".into(),
                JsonValue::from(receipt.tx_hash_hex.clone()),
            );
            labels.insert(
                "reconciliation_digest_hex".into(),
                JsonValue::from(receipt.reconciliation_digest_hex.clone()),
            );
            labels.insert(
                "reconciliation_status".into(),
                JsonValue::from(receipt.reconciliation_status.clone()),
            );
            labels.insert(
                "observed_lifecycle_status".into(),
                JsonValue::from(receipt.observed_lifecycle_status.clone()),
            );
            labels.insert(
                "amount_xor".into(),
                JsonValue::from(receipt.amount_xor.to_string()),
            );
            labels.insert(
                "deposit_xor".into(),
                JsonValue::from(receipt.deposit_xor.to_string()),
            );
            labels.insert(
                "refund_xor".into(),
                JsonValue::from(receipt.refund_xor.to_string()),
            );
            labels.insert(
                "treasury_xor".into(),
                JsonValue::from(receipt.treasury_xor.to_string()),
            );
            labels.insert(
                "held_xor".into(),
                JsonValue::from(receipt.held_xor.to_string()),
            );
            labels.insert(
                "panel_size".into(),
                JsonValue::from(u64::from(receipt.panel_size)),
            );
            labels.insert(
                "configured_signer_count".into(),
                JsonValue::from(u64::from(receipt.configured_signer_count)),
            );
            self.record_publish_index(
                "appeal_finance_settlement_receipt",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;
            self.record_runtime_signed_payload(
                "appeal_finance_settlement_receipt",
                GovernanceLogPayloadV1::AppealFinanceSettlementReceipt(receipt.clone()),
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;

            Ok(())
        })();
        record_governance_dag_publish_result(
            "appeal_finance_settlement_receipt",
            &result,
            encoded.len(),
        );
        result
    }
}

fn reputation_snapshot_json(
    envelope: &SignedReputationSnapshotV1,
    encoded: &[u8],
    digest_hex: &str,
) -> Result<String, GovernancePublishError> {
    let mut payload = JsonMap::new();
    payload.insert(
        "signed_snapshot".into(),
        json::to_value(envelope).map_err(|err| {
            GovernancePublishError::other(format!("serialize signed reputation snapshot: {err}"))
        })?,
    );

    let snapshot = &envelope.snapshot;
    let mut metadata = JsonMap::new();
    metadata.insert(
        "snapshot_id_hex".into(),
        JsonValue::from(hex::encode(snapshot.snapshot_id)),
    );
    metadata.insert(
        "generated_at_unix".into(),
        JsonValue::from(snapshot.generated_at_unix),
    );
    metadata.insert(
        "provider_count".into(),
        JsonValue::from(snapshot.providers.len() as u64),
    );
    metadata.insert(
        "merkle_root_hex".into(),
        JsonValue::from(hex::encode(snapshot.merkle_root)),
    );
    metadata.insert(
        "policy_digest_hex".into(),
        JsonValue::from(hex::encode(envelope.policy_digest)),
    );
    metadata.insert(
        "scoring_evidence_digest_hex".into(),
        JsonValue::from(hex::encode(envelope.scoring_evidence_digest)),
    );
    metadata.insert(
        "signature_count".into(),
        JsonValue::from(envelope.signatures.len() as u64),
    );
    metadata.insert(
        "encoded_blake3".into(),
        JsonValue::from(digest_hex.to_string()),
    );
    metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
    metadata.insert(
        "encoded_base64".into(),
        JsonValue::from(BASE64_STANDARD.encode(encoded)),
    );
    payload.insert("metadata".into(), JsonValue::Object(metadata));

    json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
        GovernancePublishError::other(format!("serialize signed reputation snapshot json: {err}"))
    })
}

fn moderation_ballot_event_json(
    event: &SoraFsModerationBallotGovernanceEventV1,
    encoded: &[u8],
    digest_hex: &str,
) -> Result<String, GovernancePublishError> {
    let mut payload = JsonMap::new();
    payload.insert(
        "event".into(),
        json::to_value(event).map_err(|err| {
            GovernancePublishError::other(format!("serialize moderation ballot event: {err}"))
        })?,
    );

    let mut metadata = JsonMap::new();
    metadata.insert("case_id".into(), JsonValue::from(event.case_id.clone()));
    metadata.insert("round_id".into(), JsonValue::from(event.round_id.clone()));
    metadata.insert("kind".into(), JsonValue::from(event.kind.as_str()));
    metadata.insert("sequence".into(), JsonValue::from(event.sequence));
    metadata.insert(
        "generated_at_unix_ms".into(),
        JsonValue::from(event.generated_at_unix_ms),
    );
    metadata.insert(
        "committed_count".into(),
        JsonValue::from(event.committed_count),
    );
    metadata.insert(
        "revealed_count".into(),
        JsonValue::from(event.revealed_count),
    );
    metadata.insert(
        "challenge_count".into(),
        JsonValue::from(event.challenge_count),
    );
    if let Some(juror_id) = &event.juror_id {
        metadata.insert("juror_id".into(), JsonValue::from(juror_id.clone()));
    }
    if let Some(challenge) = &event.challenge {
        metadata.insert(
            "challenge_id".into(),
            JsonValue::from(challenge.challenge_id.clone()),
        );
        metadata.insert(
            "challenge_kind".into(),
            JsonValue::from(challenge.kind.as_str()),
        );
        if let Some(decision) = challenge.decision {
            metadata.insert(
                "challenge_decision".into(),
                JsonValue::from(decision.as_str()),
            );
        }
    }
    if let Some(tally) = &event.tally {
        metadata.insert(
            "votes_total".into(),
            JsonValue::from(u64::from(tally.votes_total)),
        );
        metadata.insert("quorum".into(), JsonValue::from(u64::from(tally.quorum)));
        metadata.insert("contested".into(), JsonValue::from(tally.contested));
        if let Some(choice) = tally.winning_choice {
            metadata.insert("winning_choice".into(), JsonValue::from(choice.as_str()));
        }
    }
    metadata.insert(
        "encoded_blake3".into(),
        JsonValue::from(digest_hex.to_string()),
    );
    metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
    metadata.insert(
        "encoded_base64".into(),
        JsonValue::from(BASE64_STANDARD.encode(encoded)),
    );
    payload.insert("metadata".into(), JsonValue::Object(metadata));

    json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
        GovernancePublishError::other(format!("serialize moderation ballot event json: {err}"))
    })
}

fn transparency_ledger_publication_json(
    publication: &ModerationLedgerCyclePublicationV1,
    encoded: &[u8],
    digest_hex: &str,
) -> Result<String, GovernancePublishError> {
    let block_hash = publication.block.block_hash().map_err(|err| {
        GovernancePublishError::other(format!("hash transparency ledger block: {err}"))
    })?;
    let publication_hash = publication.publication_hash().map_err(|err| {
        GovernancePublishError::other(format!("hash transparency ledger publication: {err}"))
    })?;

    let mut payload = JsonMap::new();
    payload.insert(
        "publication".into(),
        json::to_value(publication).map_err(|err| {
            GovernancePublishError::other(format!(
                "serialize transparency ledger publication: {err}"
            ))
        })?,
    );

    let mut metadata = JsonMap::new();
    metadata.insert(
        "cycle_id_hex".into(),
        JsonValue::from(hex::encode(publication.block.cycle_id)),
    );
    metadata.insert(
        "cycle_start_unix".into(),
        JsonValue::from(publication.block.cycle_start_unix),
    );
    metadata.insert(
        "cycle_end_unix".into(),
        JsonValue::from(publication.block.cycle_end_unix),
    );
    metadata.insert(
        "generated_at_unix".into(),
        JsonValue::from(publication.block.generated_at_unix),
    );
    metadata.insert(
        "entry_count".into(),
        JsonValue::from(publication.block.entry_count),
    );
    metadata.insert(
        "proof_count".into(),
        JsonValue::from(publication.proofs.len() as u64),
    );
    metadata.insert(
        "entry_root_hex".into(),
        JsonValue::from(hex::encode(publication.block.entry_root)),
    );
    metadata.insert(
        "block_hash_hex".into(),
        JsonValue::from(hex::encode(block_hash)),
    );
    metadata.insert(
        "publication_hash_hex".into(),
        JsonValue::from(hex::encode(publication_hash)),
    );
    metadata.insert(
        "encoded_blake3".into(),
        JsonValue::from(digest_hex.to_string()),
    );
    metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
    metadata.insert(
        "encoded_base64".into(),
        JsonValue::from(BASE64_STANDARD.encode(encoded)),
    );
    payload.insert("metadata".into(), JsonValue::Object(metadata));

    json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
        GovernancePublishError::other(format!(
            "serialize transparency ledger publication json: {err}"
        ))
    })
}

fn proof_token_issuance_json(
    issuance: &ProofTokenIssuanceV1,
    encoded: &[u8],
    digest_hex: &str,
) -> Result<String, GovernancePublishError> {
    let mut payload = JsonMap::new();
    payload.insert(
        "issuance".into(),
        json::to_value(issuance).map_err(|err| {
            GovernancePublishError::other(format!("serialize proof-token issuance: {err}"))
        })?,
    );

    let mut metadata = JsonMap::new();
    metadata.insert(
        "payload_version".into(),
        JsonValue::from(u64::from(PROOF_TOKEN_ISSUANCE_VERSION_V1)),
    );
    metadata.insert(
        "token_id_hex".into(),
        JsonValue::from(hex::encode(issuance.token_id)),
    );
    metadata.insert(
        "issued_at_unix".into(),
        JsonValue::from(issuance.issued_at_unix),
    );
    if let Some(expires_at_unix) = issuance.expires_at_unix {
        metadata.insert("expires_at_unix".into(), JsonValue::from(expires_at_unix));
    }
    metadata.insert(
        "moderation_action_code".into(),
        JsonValue::from(u64::from(issuance.moderation_action_code)),
    );
    metadata.insert(
        "signer_key_hex".into(),
        JsonValue::from(hex::encode(issuance.signer_key)),
    );
    metadata.insert(
        "token_blake3_hex".into(),
        JsonValue::from(hex::encode(issuance.token_blake3)),
    );
    metadata.insert(
        "blinded_digest_hex".into(),
        JsonValue::from(hex::encode(issuance.blinded_digest)),
    );
    metadata.insert(
        "entry_count".into(),
        JsonValue::from(issuance.entry_ids.len() as u64),
    );
    metadata.insert(
        "entry_ids".into(),
        JsonValue::Array(
            issuance
                .entry_ids
                .iter()
                .cloned()
                .map(JsonValue::from)
                .collect(),
        ),
    );
    if let Some(evidence_digest) = issuance.evidence_digest {
        metadata.insert(
            "evidence_digest_hex".into(),
            JsonValue::from(hex::encode(evidence_digest)),
        );
    }
    if let Some(policy_digest) = issuance.policy_digest {
        metadata.insert(
            "policy_digest_hex".into(),
            JsonValue::from(hex::encode(policy_digest)),
        );
    }
    metadata.insert(
        "encoded_blake3".into(),
        JsonValue::from(digest_hex.to_string()),
    );
    metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
    metadata.insert(
        "encoded_base64".into(),
        JsonValue::from(BASE64_STANDARD.encode(encoded)),
    );
    payload.insert("metadata".into(), JsonValue::Object(metadata));

    json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
        GovernancePublishError::other(format!("serialize proof-token issuance json: {err}"))
    })
}

fn appeal_finance_report_json(
    report: &SoraFsAppealFinanceReportV1,
    encoded: &[u8],
    digest_hex: &str,
) -> Result<String, GovernancePublishError> {
    let mut payload = JsonMap::new();
    payload.insert(
        "report".into(),
        json::to_value(report).map_err(|err| {
            GovernancePublishError::other(format!("serialize appeal finance report: {err}"))
        })?,
    );

    let mut metadata = JsonMap::new();
    metadata.insert(
        "report_id_hex".into(),
        JsonValue::from(hex::encode(report.report_id)),
    );
    metadata.insert("case_id".into(), JsonValue::from(report.case_id.clone()));
    if let Some(round_id) = &report.round_id {
        metadata.insert("round_id".into(), JsonValue::from(round_id.clone()));
    }
    metadata.insert("outcome".into(), JsonValue::from(report.outcome.as_str()));
    metadata.insert(
        "generated_at_unix_ms".into(),
        JsonValue::from(report.generated_at_unix_ms),
    );
    metadata.insert(
        "appeal_finance_config_version".into(),
        JsonValue::from(report.appeal_finance_config_version.clone()),
    );
    metadata.insert(
        "deposit_xor".into(),
        JsonValue::from(report.deposit_xor.to_string()),
    );
    metadata.insert(
        "refund_xor".into(),
        JsonValue::from(report.refund.amount_xor.to_string()),
    );
    metadata.insert(
        "treasury_xor".into(),
        JsonValue::from(report.treasury.amount_xor.to_string()),
    );
    metadata.insert(
        "held_xor".into(),
        JsonValue::from(report.held.amount_xor.to_string()),
    );
    metadata.insert(
        "panel_size".into(),
        JsonValue::from(u64::from(report.panel_size)),
    );
    metadata.insert(
        "juror_payout_count".into(),
        JsonValue::from(report.juror_payouts.len() as u64),
    );
    metadata.insert(
        "no_show_count".into(),
        JsonValue::from(report.no_show_juror_ids.len() as u64),
    );
    metadata.insert(
        "encoded_blake3".into(),
        JsonValue::from(digest_hex.to_string()),
    );
    metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
    metadata.insert(
        "encoded_base64".into(),
        JsonValue::from(BASE64_STANDARD.encode(encoded)),
    );
    payload.insert("metadata".into(), JsonValue::Object(metadata));

    json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
        GovernancePublishError::other(format!("serialize appeal finance report json: {err}"))
    })
}

fn appeal_finance_weekly_rollup_json(
    rollup: &SoraFsAppealFinanceWeeklyRollupV1,
    encoded: &[u8],
    digest_hex: &str,
) -> Result<String, GovernancePublishError> {
    let mut payload = JsonMap::new();
    payload.insert(
        "rollup".into(),
        json::to_value(rollup).map_err(|err| {
            GovernancePublishError::other(format!("serialize appeal finance weekly rollup: {err}"))
        })?,
    );

    let mut metadata = JsonMap::new();
    metadata.insert("cycle".into(), JsonValue::from(rollup.cycle.to_string()));
    metadata.insert(
        "generated_at_unix_ms".into(),
        JsonValue::from(rollup.generated_at_unix_ms),
    );
    metadata.insert("report_count".into(), JsonValue::from(rollup.report_count));
    metadata.insert("case_count".into(), JsonValue::from(rollup.case_count));
    metadata.insert(
        "config_versions".into(),
        JsonValue::Array(
            rollup
                .appeal_finance_config_versions
                .iter()
                .cloned()
                .map(JsonValue::from)
                .collect(),
        ),
    );
    metadata.insert(
        "total_deposit_xor".into(),
        JsonValue::from(rollup.total_deposit_xor.to_string()),
    );
    metadata.insert(
        "total_refund_xor".into(),
        JsonValue::from(rollup.total_refund_xor.to_string()),
    );
    metadata.insert(
        "total_treasury_xor".into(),
        JsonValue::from(rollup.total_treasury_xor.to_string()),
    );
    metadata.insert(
        "total_held_xor".into(),
        JsonValue::from(rollup.total_held_xor.to_string()),
    );
    metadata.insert(
        "total_rewards_forfeited_treasury_xor".into(),
        JsonValue::from(rollup.total_rewards_forfeited_treasury_xor.to_string()),
    );
    metadata.insert(
        "juror_payout_count".into(),
        JsonValue::from(rollup.juror_payout_count),
    );
    metadata.insert(
        "no_show_count".into(),
        JsonValue::from(rollup.no_show_juror_count),
    );
    metadata.insert(
        "encoded_blake3".into(),
        JsonValue::from(digest_hex.to_string()),
    );
    metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
    metadata.insert(
        "encoded_base64".into(),
        JsonValue::from(BASE64_STANDARD.encode(encoded)),
    );
    payload.insert("metadata".into(), JsonValue::Object(metadata));

    json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
        GovernancePublishError::other(format!(
            "serialize appeal finance weekly rollup json: {err}"
        ))
    })
}

fn appeal_finance_settlement_receipt_json(
    receipt: &SoraFsAppealFinanceSettlementReceiptV1,
    encoded: &[u8],
    digest_hex: &str,
) -> Result<String, GovernancePublishError> {
    let mut payload = JsonMap::new();
    payload.insert(
        "receipt".into(),
        json::to_value(receipt).map_err(|err| {
            GovernancePublishError::other(format!(
                "serialize appeal finance settlement receipt: {err}"
            ))
        })?,
    );

    let mut metadata = JsonMap::new();
    metadata.insert(
        "receipt_id_hex".into(),
        JsonValue::from(hex::encode(receipt.receipt_id)),
    );
    metadata.insert("case_id".into(), JsonValue::from(receipt.case_id.clone()));
    if let Some(round_id) = &receipt.round_id {
        metadata.insert("round_id".into(), JsonValue::from(round_id.clone()));
    }
    metadata.insert(
        "generated_at_unix_ms".into(),
        JsonValue::from(receipt.generated_at_unix_ms),
    );
    metadata.insert(
        "appeal_finance_config_version".into(),
        JsonValue::from(receipt.appeal_finance_config_version.clone()),
    );
    metadata.insert(
        "appeal_finance_policy_digest_hex".into(),
        JsonValue::from(hex::encode(receipt.appeal_finance_policy_digest)),
    );
    metadata.insert("outcome".into(), JsonValue::from(receipt.outcome.as_str()));
    metadata.insert(
        "escrow_id_hex".into(),
        JsonValue::from(receipt.escrow_id_hex.clone()),
    );
    metadata.insert(
        "submitted_step".into(),
        JsonValue::from(receipt.submitted_step.clone()),
    );
    metadata.insert(
        "required_authority".into(),
        JsonValue::from(receipt.required_authority.clone()),
    );
    metadata.insert(
        "tx_hash_hex".into(),
        JsonValue::from(receipt.tx_hash_hex.clone()),
    );
    metadata.insert(
        "reconciliation_digest_hex".into(),
        JsonValue::from(receipt.reconciliation_digest_hex.clone()),
    );
    metadata.insert(
        "reconciliation_status".into(),
        JsonValue::from(receipt.reconciliation_status.clone()),
    );
    metadata.insert(
        "observed_lifecycle_status".into(),
        JsonValue::from(receipt.observed_lifecycle_status.clone()),
    );
    metadata.insert(
        "encoded_blake3".into(),
        JsonValue::from(digest_hex.to_string()),
    );
    metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
    metadata.insert(
        "encoded_base64".into(),
        JsonValue::from(BASE64_STANDARD.encode(encoded)),
    );
    payload.insert("metadata".into(), JsonValue::Object(metadata));

    json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
        GovernancePublishError::other(format!(
            "serialize appeal finance settlement receipt json: {err}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs, io,
        panic::{AssertUnwindSafe, catch_unwind},
        path::{Path, PathBuf},
        sync::Arc,
        thread,
    };

    use iroha_crypto::{Algorithm, KeyPair, Signature as IrohaSignature};
    use norito::codec::Encode;
    use sorafs_manifest::PorReportIsoWeek;
    use sorafs_manifest::deal::{
        DEAL_LEDGER_VERSION_V1, DEAL_SETTLEMENT_VERSION_V1, DealLedgerSnapshotV1, XorQuantity,
    };
    use sorafs_manifest::por::{
        POR_CHALLENGE_VERSION_V1, POR_WEEKLY_REPORT_VERSION_V1, PorChallengeV1,
        derive_challenge_id, derive_challenge_seed,
    };
    use sorafs_manifest::repair::{
        GC_AUDIT_EVENT_VERSION_V1, GC_AUDIT_PAYLOAD_VERSION_V1, GC_AUDIT_SIGNER_V1, GcAuditEventV1,
        GcAuditPayloadV1, SorafsAuditHeaderV1, gc_audit_payload_digest_v1,
    };
    use sorafs_manifest::{
        GovernanceDagBlockV1, GovernanceDagHeadV1, GovernanceLogPayloadV1,
        MODERATION_LEDGER_PUBLICATION_VERSION_V1, REPUTATION_PROVIDER_INPUT_VERSION_V1,
        REPUTATION_PROVIDER_METRICS_VERSION_V1, REPUTATION_SCORING_EVIDENCE_VERSION_V1,
        ReputationProviderInputV1, ReputationProviderMetricsV1, ReputationReserveStageV1,
        ReputationScoringEvidenceV1, ReputationSnapshotSignatureV1, ReputationWeightsV1,
        SIGNED_REPUTATION_SNAPSHOT_VERSION_V1, SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
        SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1,
        SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
        SORAFS_RECONCILIATION_REPORT_VERSION_V1, SignedReputationSnapshotV1,
        SoraFsAppealFinanceAccountFlowV1, SoraFsAppealFinanceJurorPayoutV1,
        SoraFsAppealFinanceOutcomeV1, SoraFsAppealFinanceReportV1,
        SoraFsAppealFinanceSettlementReceiptV1, SoraFsAppealFinanceWeeklyRollupV1,
        SoraFsModerationBallotGovernanceEventKindV1, SoraFsModerationBallotGovernanceEventV1,
        SoraFsModerationBallotGovernanceTallyV1, SoraFsModerationVoteChoiceV1,
        SoraFsModerationVoteCountsV1, SorafsReconciliationReportV1, build_reputation_snapshot,
        validate_governance_dag_head_against_chain_v1,
    };
    use tempfile::TempDir;

    use super::*;

    fn xor(value: &str) -> sorafs_manifest::deal::XorQuantity {
        value.parse().expect("canonical XOR quantity")
    }

    struct CanonicalTempDir {
        _inner: TempDir,
        path: PathBuf,
    }

    impl CanonicalTempDir {
        fn path(&self) -> &Path {
            &self.path
        }
    }

    fn tempdir() -> std::io::Result<CanonicalTempDir> {
        let inner = tempfile::tempdir()?;
        let path = inner.path().canonicalize()?;
        Ok(CanonicalTempDir {
            _inner: inner,
            path,
        })
    }

    fn canonical_temp_path(dir: &CanonicalTempDir) -> PathBuf {
        dir.path().to_path_buf()
    }

    fn sample_settlement() -> (DealSettlementV1, Vec<u8>) {
        let deal_id = [0xAB; 32];
        let provider_id = [0xCD; 32];
        let client_id = [0xEF; 32];
        let mut ledger = DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            snapshot_id: [0; 32],
            sequence: 1,
            previous_snapshot_id: None,
            deal_id,
            terms_digest: [0xA4; 32],
            provider_id,
            client_id,
            deal_start_epoch: 1_699_999_990,
            deal_end_epoch: 1_699_999_999,
            settlement_window_epochs: 10,
            window_start_epoch: 1_699_999_990,
            window_end_epoch: 1_700_000_000,
            provider_accrual: xor("0.5"),
            client_liability: xor("0.5"),
            micropayment_credit_generated: XorQuantity::zero(),
            micropayment_credit_applied: XorQuantity::zero(),
            micropayment_credit_carry: XorQuantity::zero(),
            client_debit: xor("0.5"),
            outstanding_liability: XorQuantity::zero(),
            bond_total: xor("1"),
            bond_locked: XorQuantity::zero(),
            bond_slashed: XorQuantity::zero(),
            bond_released: xor("1"),
            window_expected_charge: xor("0.5"),
            window_micropayment_generated: XorQuantity::zero(),
            window_micropayment_applied: XorQuantity::zero(),
            window_client_debit: xor("0.5"),
            window_bond_slashed: XorQuantity::zero(),
            window_bond_released: xor("1"),
            captured_at: 1_700_000_000,
        };
        ledger.snapshot_id = ledger.derive_snapshot_id().expect("ledger id");
        let mut settlement = DealSettlementV1 {
            version: DEAL_SETTLEMENT_VERSION_V1,
            settlement_id: [0; 32],
            deal_id,
            ledger,
            status: DealSettlementStatusV1::Completed,
            settled_at: 1_700_000_000,
            audit_notes: None,
        };
        settlement.settlement_id = settlement.derive_settlement_id().expect("settlement id");
        let encoded = norito::to_bytes(&settlement).expect("encode settlement");
        (settlement, encoded)
    }

    fn sample_por_challenge_publication() -> (PorChallengePublicationV1, Vec<u8>) {
        let manifest_digest = [0x41; 32];
        let provider_id = [0x42; 32];
        let epoch_id = 7;
        let drand_round = 11;
        let drand_randomness = [0x43; 32];
        let seed = derive_challenge_seed(&drand_randomness, None, &manifest_digest, epoch_id);
        let challenge = PorChallengeV1 {
            version: POR_CHALLENGE_VERSION_V1,
            challenge_id: derive_challenge_id(
                &seed,
                &manifest_digest,
                &provider_id,
                epoch_id,
                drand_round,
            ),
            manifest_digest,
            provider_id,
            epoch_id,
            drand_round,
            drand_randomness,
            drand_signature: [0x44; iroha_crypto::drand::DRAND_SIGNATURE_BYTES],
            vrf_output: None,
            vrf_proof: None,
            forced: true,
            chunking_profile: "sorafs.sf1@1.0.0".to_owned(),
            seed,
            sample_tier: 1,
            sample_count: 3,
            sample_indices: vec![5, 5, 9],
            issued_at: 1_800_000_000,
            deadline_at: 1_800_000_900,
        };
        let publication =
            PorChallengePublicationV1::try_new(challenge, 1).expect("challenge publication");
        let encoded = norito::to_bytes(&publication).expect("encode challenge publication");
        (publication, encoded)
    }

    fn sample_por_weekly_report() -> (PorWeeklyReportV1, Vec<u8>) {
        let report = PorWeeklyReportV1 {
            version: POR_WEEKLY_REPORT_VERSION_V1,
            cycle: PorReportIsoWeek {
                year: 2026,
                week: 30,
            },
            generated_at: 1_800_604_800,
            challenges_total: 3,
            challenges_verified: 2,
            challenges_failed: 1,
            forced_challenges: 1,
            repairs_enqueued: 1,
            repairs_completed: 1,
            mean_latency_ms: Some(75),
            p95_latency_ms: Some(120),
            slashing_events: Vec::new(),
            providers_missing_vrf: vec![[0x42; 32]],
            top_offenders: Vec::new(),
            notes: None,
        };
        report.validate().expect("weekly report");
        let encoded = norito::to_bytes(&report).expect("encode weekly report");
        (report, encoded)
    }

    fn sample_reputation_snapshot() -> (SignedReputationSnapshotV1, Vec<u8>) {
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
        let input = ReputationProviderInputV1 {
            version: REPUTATION_PROVIDER_INPUT_VERSION_V1,
            provider_id: "provider-a".to_string(),
            metrics,
            reserve_stage: ReputationReserveStageV1::Active,
            previous_score_bps: None,
            active_dispute: false,
            slashing_event: false,
        };
        let inputs = vec![input];
        let snapshot = build_reputation_snapshot(
            [0x42; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &inputs,
            None,
        )
        .expect("reputation snapshot");
        let scoring_evidence = ReputationScoringEvidenceV1 {
            version: REPUTATION_SCORING_EVIDENCE_VERSION_V1,
            provider_inputs: inputs,
            trust_edges: Vec::new(),
        };
        let mut envelope = SignedReputationSnapshotV1 {
            version: SIGNED_REPUTATION_SNAPSHOT_VERSION_V1,
            policy_digest: [0xA5; 32],
            snapshot,
            scoring_evidence_digest: scoring_evidence
                .canonical_digest()
                .expect("scoring evidence digest"),
            scoring_evidence,
            signatures: Vec::new(),
        };
        let signing_key = KeyPair::try_from_seed(vec![0x5A; 32], Algorithm::Ed25519)
            .expect("derive reputation signing key");
        let signature = IrohaSignature::try_new(
            signing_key.private_key(),
            &envelope.signing_digest().expect("signing digest"),
        )
        .expect("sign reputation snapshot");
        envelope.signatures.push(ReputationSnapshotSignatureV1 {
            signer_id: "council-1".to_owned(),
            signature: signature
                .payload()
                .try_into()
                .expect("Ed25519 signature is fixed-width"),
        });
        let encoded = envelope
            .canonical_bytes()
            .expect("encode signed reputation snapshot");
        (envelope, encoded)
    }

    fn sample_moderation_ballot_event() -> (SoraFsModerationBallotGovernanceEventV1, Vec<u8>) {
        let event = SoraFsModerationBallotGovernanceEventV1 {
            version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
            sequence: 6,
            kind: SoraFsModerationBallotGovernanceEventKindV1::BallotTallied,
            generated_at_unix_ms: 1_800_000_030_000,
            case_id: "case-42".to_string(),
            round_id: "round-1".to_string(),
            juror_id: None,
            committed_count: 2,
            revealed_count: 2,
            challenge_count: 0,
            tally: Some(SoraFsModerationBallotGovernanceTallyV1 {
                case_id: "case-42".to_string(),
                round_id: "round-1".to_string(),
                counts: SoraFsModerationVoteCountsV1 {
                    uphold: 2,
                    overturn: 0,
                    modify: 0,
                    escalate: 0,
                },
                votes_total: 2,
                quorum: 2,
                winning_choice: Some(SoraFsModerationVoteChoiceV1::Uphold),
                contested: false,
                tallied_at_unix_ms: 1_800_000_030_000,
            }),
            challenge: None,
        };
        let encoded = norito::to_bytes(&event).expect("encode moderation ballot event");
        (event, encoded)
    }

    fn sample_transparency_ledger_publication() -> (ModerationLedgerCyclePublicationV1, Vec<u8>) {
        use iroha_data_model::sorafs::transparency::{
            MODERATION_LEDGER_ENTRY_VERSION_V1, ModerationLedgerEntryKindV1,
            ModerationLedgerEntryV1, ModerationLedgerMetadataV1,
        };

        let cycle_id = *b"cycle-2026-wk-03";
        let entries = [
            ModerationLedgerEntryV1 {
                version: MODERATION_LEDGER_ENTRY_VERSION_V1,
                cycle_id,
                entry_id: [0x32; 16],
                sequence: 2,
                occurred_at_unix: 1_800_000_032,
                kind: ModerationLedgerEntryKindV1::GarEnforcementReceipt,
                subject: "gar-receipt-32".to_string(),
                subject_digest: [0x32; 32],
                payload_digest: [0x33; 32],
                summary_digest: [0x34; 32],
                policy_digest: Some([0x35; 32]),
                evidence_uris: vec!["sora://transparency/32".to_string()],
                metadata: vec![ModerationLedgerMetadataV1 {
                    key: "source".to_string(),
                    value: "gar".to_string(),
                }],
            },
            ModerationLedgerEntryV1 {
                version: MODERATION_LEDGER_ENTRY_VERSION_V1,
                cycle_id,
                entry_id: [0x31; 16],
                sequence: 1,
                occurred_at_unix: 1_800_000_031,
                kind: ModerationLedgerEntryKindV1::ModerationAction,
                subject: "moderation-case-31".to_string(),
                subject_digest: [0x31; 32],
                payload_digest: [0x32; 32],
                summary_digest: [0x33; 32],
                policy_digest: Some([0x34; 32]),
                evidence_uris: vec!["sora://transparency/31".to_string()],
                metadata: vec![ModerationLedgerMetadataV1 {
                    key: "source".to_string(),
                    value: "moderation".to_string(),
                }],
            },
        ];
        let publication = ModerationLedgerCyclePublicationV1::from_entries(
            cycle_id,
            1_800_000_000,
            1_800_604_800,
            1_800_604_801,
            None,
            &entries,
        )
        .expect("transparency ledger publication");
        let encoded =
            norito::to_bytes(&publication).expect("encode transparency ledger publication");
        (publication, encoded)
    }

    fn sample_proof_token_issuance() -> (ProofTokenIssuanceV1, Vec<u8>) {
        let issuance = ProofTokenIssuanceV1 {
            version: PROOF_TOKEN_ISSUANCE_VERSION_V1,
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
            metadata: Vec::new(),
        };
        let encoded = norito::to_bytes(&issuance).expect("encode proof-token issuance");
        (issuance, encoded)
    }

    fn sample_appeal_finance_report() -> (SoraFsAppealFinanceReportV1, Vec<u8>) {
        let report = SoraFsAppealFinanceReportV1 {
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
        };
        let encoded = norito::to_bytes(&report).expect("encode appeal finance report");
        (report, encoded)
    }

    fn sample_appeal_finance_weekly_rollup() -> (SoraFsAppealFinanceWeeklyRollupV1, Vec<u8>) {
        let (report, _) = sample_appeal_finance_report();
        let rollup = SoraFsAppealFinanceWeeklyRollupV1::from_reports(
            PorReportIsoWeek {
                year: 2026,
                week: 26,
            },
            1_800_000_100_000,
            &[report],
        )
        .expect("appeal finance weekly rollup");
        let encoded = norito::to_bytes(&rollup).expect("encode appeal finance weekly rollup");
        (rollup, encoded)
    }

    fn sample_appeal_finance_settlement_receipt()
    -> (SoraFsAppealFinanceSettlementReceiptV1, Vec<u8>) {
        let receipt = SoraFsAppealFinanceSettlementReceiptV1 {
            version: SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1,
            receipt_id: [0x52; 16],
            case_id: "case-42".to_string(),
            round_id: Some("round-1".to_string()),
            generated_at_unix_ms: 1_800_000_032_000,
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
            reconciliation_status: "pending_forwarder_submission".to_string(),
            observed_lifecycle_status: "locked".to_string(),
            observed_remaining_xor: xor("420"),
            deposit_xor: xor("420"),
            refund_xor: xor("0"),
            treasury_xor: xor("210"),
            held_xor: xor("210"),
            panel_size: 7,
            configured_signer_count: 1,
        };
        let encoded = norito::to_bytes(&receipt).expect("encode appeal finance settlement receipt");
        (receipt, encoded)
    }

    #[test]
    fn governance_car_queue_pending_count_tracks_unassembled_segments() {
        let mut assembled = JsonMap::new();
        assembled.insert("status".into(), JsonValue::from("assembled"));
        let mut pending = JsonMap::new();
        pending.insert("status".into(), JsonValue::from("pending"));
        let malformed = JsonValue::from("not-a-segment");

        assert_eq!(
            governance_car_queue_pending_count(&[
                JsonValue::Object(assembled),
                JsonValue::Object(pending),
                malformed,
            ]),
            2
        );
    }

    #[test]
    fn governance_dag_head_age_seconds_saturates_for_future_heads() {
        assert_eq!(
            governance_dag_head_age_seconds(1_800_000_000, 1_800_000_045),
            45
        );
        assert_eq!(
            governance_dag_head_age_seconds(1_800_000_100, 1_800_000_045),
            0
        );
    }

    #[test]
    fn governance_dag_head_generated_at_from_index_prefers_head_timestamp() {
        let mut index = JsonMap::new();
        assert_eq!(governance_dag_head_generated_at_from_index(&index), None);

        index.insert("generated_at".into(), JsonValue::from(1_800_000_000u64));
        assert_eq!(
            governance_dag_head_generated_at_from_index(&index),
            Some(1_800_000_000)
        );

        index.insert(
            "head_generated_at".into(),
            JsonValue::from(1_800_000_045u64),
        );
        assert_eq!(
            governance_dag_head_generated_at_from_index(&index),
            Some(1_800_000_045)
        );
    }

    #[test]
    fn bounded_governance_state_reader_rejects_oversized_file() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("index.json");
        fs::write(&path, b"123456789").expect("write oversized state");

        let error = read_bounded_governance_state_file(&path, 8)
            .expect_err("oversized governance state must fail before allocation");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("exceeds 8 bytes"));
    }

    #[cfg(unix)]
    #[test]
    fn bounded_governance_state_reader_rejects_symlink() {
        let temp = tempdir().expect("tempdir");
        let target = temp.path().join("target.json");
        let path = temp.path().join("index.json");
        fs::write(&target, b"{}").expect("write target");
        std::os::unix::fs::symlink(&target, &path).expect("create index symlink");

        let error = read_bounded_governance_state_file(&path, 8)
            .expect_err("governance state symlink must fail closed");
        assert!(error.to_string().contains("must not be a symlink"));
    }

    #[cfg(unix)]
    #[test]
    fn bounded_governance_state_reader_rejects_hard_link() {
        let temp = tempdir().expect("tempdir");
        let target = temp.path().join("target.json");
        let path = temp.path().join("index.json");
        fs::write(&target, b"{}").expect("write target");
        fs::hard_link(&target, &path).expect("create index hard link");

        let error = read_bounded_governance_state_file(&path, 8)
            .expect_err("hard-linked governance state must fail closed");
        assert!(error.to_string().contains("exactly one hard link"));
    }

    struct TestRuntimeDagSigner {
        handle: String,
        publisher_peer_id: Vec<u8>,
        key_pair: KeyPair,
        public_key_override: Option<[u8; 32]>,
        refuse_with: Option<String>,
        corrupt_signature: bool,
    }

    impl fmt::Debug for TestRuntimeDagSigner {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("TestRuntimeDagSigner")
                .field("handle", &self.handle)
                .field("publisher_peer_id", &self.publisher_peer_id)
                .finish_non_exhaustive()
        }
    }

    impl TestRuntimeDagSigner {
        fn new(handle: &str, publisher_peer_id: &[u8], seed: u8) -> Self {
            Self {
                handle: handle.to_owned(),
                publisher_peer_id: publisher_peer_id.to_vec(),
                key_pair: KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .expect("derive test runtime DAG signer"),
                public_key_override: None,
                refuse_with: None,
                corrupt_signature: false,
            }
        }

        fn public_key_bytes(&self) -> [u8; 32] {
            let (algorithm, bytes) = self
                .key_pair
                .public_key()
                .try_to_bytes()
                .expect("serialize test public key");
            assert_eq!(algorithm, Algorithm::Ed25519);
            bytes.try_into().expect("Ed25519 public key is fixed-width")
        }
    }

    impl GovernanceDagRuntimeSigner for TestRuntimeDagSigner {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn publisher_peer_id(&self) -> &[u8] {
            &self.publisher_peer_id
        }

        fn public_key(&self) -> [u8; 32] {
            self.public_key_override
                .unwrap_or_else(|| self.public_key_bytes())
        }

        fn sign(&self, payload: &[u8]) -> Result<[u8; 64], String> {
            if let Some(error) = &self.refuse_with {
                return Err(error.clone());
            }
            let mut signature: [u8; 64] =
                IrohaSignature::try_new(self.key_pair.private_key(), payload)
                    .expect("test runtime signer can sign")
                    .payload()
                    .try_into()
                    .expect("Ed25519 signature is fixed-width");
            if self.corrupt_signature {
                signature[0] ^= 0x80;
            }
            Ok(signature)
        }
    }

    fn signed_runtime_publisher(root: &Path) -> FilesystemGovernancePublisher {
        let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
        let signer = Arc::new(TestRuntimeDagSigner::new(
            "pkcs11:governance-dag:test",
            &peer_id,
            0x31,
        ));
        let public_key = signer.public_key();
        FilesystemGovernancePublisher::try_new(root.to_path_buf())
            .expect("publisher")
            .with_runtime_dag_signer_provider(
                "pkcs11:governance-dag:test",
                peer_id,
                public_key,
                signer,
            )
            .expect("runtime DAG signer")
    }

    fn runtime_index(root: &Path) -> JsonValue {
        let bytes =
            fs::read(root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE)).expect("runtime index exists");
        norito::json::from_slice(&bytes).expect("runtime index parses")
    }

    fn runtime_blocks_from_index(root: &Path, index: &JsonValue) -> Vec<GovernanceDagBlockV1> {
        index
            .get("blocks")
            .and_then(JsonValue::as_array)
            .expect("runtime blocks")
            .iter()
            .map(|entry| {
                let block_path = entry
                    .get("block_path")
                    .and_then(JsonValue::as_str)
                    .expect("block path");
                let block_path = resolve_index_path(root, block_path).expect("resolve block path");
                let bytes = fs::read(block_path).expect("read runtime block");
                norito::decode_from_bytes(&bytes).expect("decode runtime block")
            })
            .collect()
    }

    fn assert_single_runtime_external(root: &Path, kind: &str, encoded: &[u8]) {
        let index = runtime_index(root);
        let blocks = runtime_blocks_from_index(root, &index);
        assert_eq!(blocks.len(), 1);
        match &blocks[0].node.payload {
            GovernanceLogPayloadV1::ExternalPayload(payload) => {
                payload.validate().expect("external payload validates");
                assert_eq!(payload.payload_kind, kind);
                assert_eq!(payload.encoded_payload, encoded);
                assert_eq!(payload.encoded_blake3, *blake3::hash(encoded).as_bytes());
            }
            other => panic!("expected external runtime payload, found {other:?}"),
        }
    }

    #[test]
    fn filesystem_publisher_rejects_noncanonical_or_mismatched_payload_bytes() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (settlement, canonical) = sample_settlement();

        let bare = settlement.encode();
        let error = publisher
            .publish_deal_settlement(&settlement, &bare)
            .expect_err("bare payload without a Norito header must fail");
        assert!(error.to_string().contains("canonical header-bearing"));

        let mut conflicting = settlement.clone();
        conflicting.audit_notes = Some("different typed payload".to_owned());
        let error = publisher
            .publish_deal_settlement(&conflicting, &canonical)
            .expect_err("typed payload and canonical bytes must match");
        assert!(error.to_string().contains("do not match"));
        assert!(
            !temp.path().join("settlements").exists(),
            "validation must fail before any governance artifact is written"
        );
    }

    #[test]
    fn filesystem_publisher_rejects_semantically_invalid_payload_before_writes() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (mut settlement, _) = sample_settlement();
        settlement.deal_id[0] ^= 0x80;
        let encoded = norito::to_bytes(&settlement).expect("encode invalid settlement");

        let error = publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect_err("ledger and settlement deal identifiers must match");
        assert!(error.to_string().contains("invalid deal settlement"));
        assert!(
            !temp.path().join("settlements").exists(),
            "semantic validation must fail before any governance artifact is written"
        );
    }

    #[test]
    fn filesystem_publisher_writes_por_payloads_into_one_signed_canonical_chain() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (publication, publication_encoded) = sample_por_challenge_publication();
        let (report, report_encoded) = sample_por_weekly_report();

        publisher
            .publish_por_challenge_publication(&publication, &publication_encoded)
            .expect("publish PoR challenge");
        publisher
            .publish_por_weekly_report(&report, &report_encoded)
            .expect("publish PoR weekly report");

        let challenge_path = temp
            .path()
            .join("por")
            .join("challenges")
            .join(format!("{:020}", publication.challenge.epoch_id))
            .join(hex::encode(publication.challenge.challenge_id))
            .with_extension("to");
        assert_eq!(
            fs::read(&challenge_path).expect("read canonical challenge publication"),
            publication_encoded
        );

        let report_digest = blake3::hash(&report_encoded).to_hex().to_string();
        let report_path = temp
            .path()
            .join("por")
            .join("reports")
            .join(format!(
                "{:04}-W{:02}_{:020}_{}",
                report.cycle.year,
                report.cycle.week,
                report.generated_at,
                &report_digest[..16],
            ))
            .with_extension("to");
        assert_eq!(
            fs::read(&report_path).expect("read canonical weekly report"),
            report_encoded
        );

        let index = runtime_index(temp.path());
        let blocks = runtime_blocks_from_index(temp.path(), &index);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[1].prev_block_cid, Some(blocks[0].block_cid.clone()));
        assert_eq!(
            blocks[1].node.prev_cid,
            Some(blocks[0].node.node_cid.clone())
        );
        assert_eq!(
            blocks[0].node.payload,
            GovernanceLogPayloadV1::PorChallengePublication(publication)
        );
        assert_eq!(
            blocks[1].node.payload,
            GovernanceLogPayloadV1::PorWeeklyReport(report)
        );
        let head_bytes =
            fs::read(runtime_dag_head_path(temp.path())).expect("read signed runtime head");
        let head: GovernanceDagHeadV1 =
            norito::decode_from_bytes(&head_bytes).expect("decode signed runtime head");
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("PoR runtime chain and head validate");
    }

    #[test]
    fn filesystem_publisher_root_has_a_single_process_owner() {
        let temp = tempdir().expect("tempdir");
        let owner = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("acquire publisher root");

        let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect_err("a second publisher must not share mutable index state");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert!(error.to_string().contains("already in use"));

        drop(owner);
        FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect("publisher root ownership releases on drop");
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_publisher_root_lock_rejects_symlink() {
        let temp = tempdir().expect("tempdir");
        let target = temp.path().join("lock-target");
        fs::write(&target, b"must remain untouched").expect("write lock target");
        std::os::unix::fs::symlink(&target, temp.path().join(GOVERNANCE_PUBLISHER_LOCK_FILE))
            .expect("create publisher lock symlink");

        let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect_err("publisher lock symlink must fail closed");
        assert!(error.to_string().contains("must not be a symlink"));
        assert_eq!(
            fs::read(&target).expect("read lock target"),
            b"must remain untouched"
        );
    }

    #[cfg(unix)]
    #[test]
    fn filesystem_publisher_root_lock_rejects_hard_link() {
        let temp = tempdir().expect("tempdir");
        let target = temp.path().join("lock-target");
        fs::write(&target, b"must remain untouched").expect("write lock target");
        fs::hard_link(&target, temp.path().join(GOVERNANCE_PUBLISHER_LOCK_FILE))
            .expect("create publisher lock hard link");

        let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
            .expect_err("publisher lock hard link must fail closed");
        assert!(error.to_string().contains("exactly one hard link"));
        assert_eq!(
            fs::read(&target).expect("read lock target"),
            b"must remain untouched"
        );
    }

    #[test]
    fn runtime_dag_signer_rejects_invalid_handle_and_oversized_identity() {
        let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
        let signer = Arc::new(TestRuntimeDagSigner::new(
            "pkcs11:governance-dag:test",
            &peer_id,
            0x31,
        ));
        let public_key = signer.public_key();

        let error = GovernanceRuntimeDagSigner::try_new(
            "contains whitespace".to_owned(),
            peer_id.clone(),
            public_key,
            signer.clone(),
        )
        .expect_err("whitespace-bearing signer handle must fail closed");
        assert!(error.to_string().contains("without whitespace"));

        let error = GovernanceRuntimeDagSigner::try_new(
            signer.handle().to_owned(),
            vec![0x41; GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1 + 1],
            public_key,
            signer,
        )
        .expect_err("oversized governance publisher identity must fail closed");
        assert!(
            error
                .to_string()
                .contains("publisher peer id exceeds 128 bytes")
        );
    }

    #[test]
    fn runtime_dag_signer_rejects_handle_peer_and_public_key_mismatch() {
        let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
        let signer = Arc::new(TestRuntimeDagSigner::new(
            "pkcs11:governance-dag:test",
            &peer_id,
            0x31,
        ));
        let public_key = signer.public_key();

        for (handle, peer, key, expected) in [
            (
                "pkcs11:governance-dag:other",
                peer_id.clone(),
                public_key,
                "handle does not match",
            ),
            (
                signer.handle(),
                b"12D3KooWOtherPublisher".to_vec(),
                public_key,
                "publisher identity does not match",
            ),
            (
                signer.handle(),
                peer_id.clone(),
                [0xA5; 32],
                "public key does not match",
            ),
        ] {
            let error =
                GovernanceRuntimeDagSigner::try_new(handle.to_owned(), peer, key, signer.clone())
                    .expect_err("mismatched runtime signer must fail closed");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn runtime_dag_signer_rejects_malformed_and_weak_ed25519_keys() {
        let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
        for (public_key, expected) in [
            ([0xFF; 32], "canonical Ed25519 point"),
            (
                {
                    let mut identity = [0_u8; 32];
                    identity[0] = 1;
                    identity
                },
                "non-canonical or weak",
            ),
        ] {
            let mut signer =
                TestRuntimeDagSigner::new("pkcs11:governance-dag:test", &peer_id, 0x31);
            signer.public_key_override = Some(public_key);
            let signer = Arc::new(signer);
            let error = GovernanceRuntimeDagSigner::try_new(
                signer.handle().to_owned(),
                peer_id.clone(),
                public_key,
                signer,
            )
            .expect_err("malformed or weak Ed25519 key must fail during provider binding");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn runtime_dag_signer_redacts_provider_error_and_rejects_wrong_signature() {
        let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
        let mut refusing = TestRuntimeDagSigner::new("pkcs11:governance-dag:test", &peer_id, 0x31);
        refusing.refuse_with = Some("bearer=must-never-escape".to_owned());
        let refusing = Arc::new(refusing);
        let wrapped = GovernanceRuntimeDagSigner::try_new(
            refusing.handle().to_owned(),
            peer_id.clone(),
            refusing.public_key(),
            refusing,
        )
        .expect("bind refusing test provider");
        let error = wrapped
            .sign(b"canonical governance payload")
            .expect_err("provider outage must fail closed");
        assert!(error.to_string().contains("refused"));
        assert!(!error.to_string().contains("must-never-escape"));

        let mut corrupt = TestRuntimeDagSigner::new("pkcs11:governance-dag:test", &peer_id, 0x31);
        corrupt.corrupt_signature = true;
        let corrupt = Arc::new(corrupt);
        let wrapped = GovernanceRuntimeDagSigner::try_new(
            corrupt.handle().to_owned(),
            peer_id,
            corrupt.public_key(),
            corrupt,
        )
        .expect("bind corrupt test provider");
        let error = wrapped
            .sign(b"canonical governance payload")
            .expect_err("wrong signature must fail closed");
        assert!(error.to_string().contains("another key or payload"));
    }

    #[test]
    fn filesystem_publisher_serializes_concurrent_index_and_signed_head_updates() {
        const PUBLICATION_COUNT: usize = 16;

        let temp = tempdir().expect("tempdir");
        let publisher = Arc::new(signed_runtime_publisher(temp.path()));
        let (template, _) = sample_settlement();
        let threads = (0..PUBLICATION_COUNT)
            .map(|index| {
                let publisher = Arc::clone(&publisher);
                let mut settlement = template.clone();
                let marker = u8::try_from(index + 1).expect("small publication count");
                settlement.deal_id = [marker; 32];
                settlement.ledger.deal_id = settlement.deal_id;
                settlement.ledger.snapshot_id = settlement
                    .ledger
                    .derive_snapshot_id()
                    .expect("reseal ledger snapshot");
                settlement.settlement_id = settlement
                    .derive_settlement_id()
                    .expect("reseal settlement");
                thread::spawn(move || {
                    let encoded = norito::to_bytes(&settlement).expect("encode settlement");
                    publisher
                        .publish_deal_settlement(&settlement, &encoded)
                        .expect("publish settlement concurrently");
                })
            })
            .collect::<Vec<_>>();

        for thread in threads {
            thread.join().expect("publisher thread");
        }

        let publish_index: JsonValue = json::from_slice(
            &fs::read(temp.path().join(GOVERNANCE_PUBLISH_INDEX_FILE))
                .expect("publish index exists"),
        )
        .expect("publish index parses");
        assert_eq!(
            publish_index.get("entry_count").and_then(JsonValue::as_u64),
            Some(PUBLICATION_COUNT as u64)
        );
        let entries = publish_index
            .get("entries")
            .and_then(JsonValue::as_array)
            .expect("publish index entries");
        assert_eq!(entries.len(), PUBLICATION_COUNT);
        for (expected_position, entry) in entries.iter().enumerate() {
            assert_eq!(
                entry.get("position").and_then(JsonValue::as_u64),
                Some(expected_position as u64)
            );
        }

        let runtime_index = runtime_index(temp.path());
        assert_eq!(
            runtime_index.get("block_count").and_then(JsonValue::as_u64),
            Some(PUBLICATION_COUNT as u64)
        );
        assert_eq!(
            runtime_index
                .get("blocks")
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(PUBLICATION_COUNT)
        );
    }

    #[test]
    fn filesystem_publisher_poisoned_transaction_lock_fails_before_writes() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let poisoned = catch_unwind(AssertUnwindSafe(|| {
            let _guard = publisher
                .publication_lock
                .lock()
                .expect("publication lock starts healthy");
            panic!("poison publication transaction lock");
        }));
        assert!(poisoned.is_err());

        let (settlement, encoded) = sample_settlement();
        let error = publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect_err("poisoned publisher must fail closed");
        assert!(error.to_string().contains("transaction lock is poisoned"));
        assert!(
            !temp.path().join("settlements").exists(),
            "poison detection must happen before artifact writes"
        );
        assert!(
            !temp.path().join(GOVERNANCE_PUBLISH_INDEX_FILE).exists(),
            "poison detection must happen before index writes"
        );
    }

    #[test]
    fn filesystem_publisher_appends_signed_runtime_dag_for_supported_payloads() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (settlement, encoded) = sample_settlement();

        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("publish settlement into runtime DAG");
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("duplicate publish is idempotent");
        let index = runtime_index(temp.path());
        assert_eq!(
            index.get("block_count").and_then(JsonValue::as_u64),
            Some(1)
        );
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("deal_settlement"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );

        let (snapshot, snapshot_encoded) = sample_reputation_snapshot();
        publisher
            .publish_reputation_snapshot(&snapshot, &snapshot_encoded)
            .expect("publish reputation snapshot into runtime DAG");

        let (finance_report, finance_encoded) = sample_appeal_finance_report();
        publisher
            .publish_appeal_finance_report(&finance_report, &finance_encoded)
            .expect("publish appeal finance report into runtime DAG");

        let (finance_rollup, rollup_encoded) = sample_appeal_finance_weekly_rollup();
        publisher
            .publish_appeal_finance_weekly_rollup(&finance_rollup, &rollup_encoded)
            .expect("publish appeal finance weekly rollup into runtime DAG");

        let (finance_receipt, receipt_encoded) = sample_appeal_finance_settlement_receipt();
        publisher
            .publish_appeal_finance_settlement_receipt(&finance_receipt, &receipt_encoded)
            .expect("publish appeal finance settlement receipt into runtime DAG");

        let (transparency_publication, transparency_encoded) =
            sample_transparency_ledger_publication();
        publisher
            .publish_transparency_ledger_publication(
                &transparency_publication,
                &transparency_encoded,
            )
            .expect("publish transparency ledger publication into runtime DAG");

        let index = runtime_index(temp.path());
        assert_eq!(
            index.get("block_count").and_then(JsonValue::as_u64),
            Some(6)
        );
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("reputation_snapshot"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("appeal_finance_report"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("appeal_finance_weekly_rollup"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("appeal_finance_settlement_receipt"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("transparency_ledger_publication"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );

        let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
        let head: GovernanceDagHeadV1 =
            norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
        let blocks = runtime_blocks_from_index(temp.path(), &index);
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("runtime head validates against signed blocks");
        assert_eq!(blocks.len(), 6);
        assert_eq!(blocks[0].sequence, 0);
        assert_eq!(blocks[1].sequence, 1);
        assert_eq!(blocks[2].sequence, 2);
        assert_eq!(blocks[3].sequence, 3);
        assert_eq!(blocks[4].sequence, 4);
        assert_eq!(blocks[5].sequence, 5);
        assert_eq!(blocks[1].prev_block_cid, Some(blocks[0].block_cid.clone()));
        assert_eq!(blocks[2].prev_block_cid, Some(blocks[1].block_cid.clone()));
        assert_eq!(blocks[3].prev_block_cid, Some(blocks[2].block_cid.clone()));
        assert_eq!(blocks[4].prev_block_cid, Some(blocks[3].block_cid.clone()));
        assert_eq!(blocks[5].prev_block_cid, Some(blocks[4].block_cid.clone()));
        assert_eq!(
            blocks[1].node.prev_cid,
            Some(blocks[0].node.node_cid.clone())
        );
        assert_eq!(
            blocks[2].node.prev_cid,
            Some(blocks[1].node.node_cid.clone())
        );
        assert_eq!(
            blocks[3].node.prev_cid,
            Some(blocks[2].node.node_cid.clone())
        );
        assert_eq!(
            blocks[4].node.prev_cid,
            Some(blocks[3].node.node_cid.clone())
        );
        assert_eq!(
            blocks[5].node.prev_cid,
            Some(blocks[4].node.node_cid.clone())
        );
        match &blocks[0].node.payload {
            GovernanceLogPayloadV1::DealSettlement(value) => {
                assert_eq!(value.deal_id, settlement.deal_id);
            }
            other => panic!("unexpected first runtime DAG payload: {other:?}"),
        }
        match &blocks[1].node.payload {
            GovernanceLogPayloadV1::SignedReputationSnapshot(value) => {
                assert_eq!(value.snapshot.snapshot_id, snapshot.snapshot.snapshot_id);
            }
            other => panic!("unexpected second runtime DAG payload: {other:?}"),
        }
        match &blocks[2].node.payload {
            GovernanceLogPayloadV1::AppealFinanceReport(value) => {
                assert_eq!(value.report_id, finance_report.report_id);
                assert_eq!(value.case_id, finance_report.case_id);
            }
            other => panic!("unexpected third runtime DAG payload: {other:?}"),
        }
        match &blocks[3].node.payload {
            GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(value) => {
                assert_eq!(value.cycle, finance_rollup.cycle);
                assert_eq!(value.report_count, finance_rollup.report_count);
                assert_eq!(value.total_deposit_xor, finance_rollup.total_deposit_xor);
            }
            other => panic!("unexpected fourth runtime DAG payload: {other:?}"),
        }
        match &blocks[4].node.payload {
            GovernanceLogPayloadV1::AppealFinanceSettlementReceipt(value) => {
                assert_eq!(value.receipt_id, finance_receipt.receipt_id);
                assert_eq!(value.tx_hash_hex, finance_receipt.tx_hash_hex);
                assert_eq!(
                    value.reconciliation_digest_hex,
                    finance_receipt.reconciliation_digest_hex
                );
            }
            other => panic!("unexpected fifth runtime DAG payload: {other:?}"),
        }
        match &blocks[5].node.payload {
            GovernanceLogPayloadV1::ExternalPayload(value) => {
                assert_eq!(value.payload_kind, "transparency_ledger_publication");
                assert_eq!(
                    value.payload_version,
                    MODERATION_LEDGER_PUBLICATION_VERSION_V1
                );
                assert_eq!(
                    value.encoded_blake3,
                    *blake3::hash(&transparency_encoded).as_bytes()
                );
                assert_eq!(value.encoded_len, transparency_encoded.len() as u64);
                assert_eq!(value.encoded_payload, transparency_encoded);
                assert_eq!(
                    value
                        .metadata
                        .iter()
                        .map(|item| item.key.as_str())
                        .collect::<Vec<_>>(),
                    vec![
                        "block_hash_hex",
                        "cycle_id_hex",
                        "entry_count",
                        "entry_root_hex",
                        "publication_hash_hex"
                    ]
                );
            }
            other => panic!("unexpected sixth runtime DAG payload: {other:?}"),
        }
    }

    #[test]
    fn filesystem_publisher_keeps_full_history_and_signs_checkpoint_window_with_one_identity() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (template, _) = sample_settlement();

        for marker in 1_u8..=GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 as u8 {
            let mut settlement = template.clone();
            settlement.deal_id = [marker; 32];
            settlement.ledger.deal_id = settlement.deal_id;
            settlement.ledger.snapshot_id = settlement
                .ledger
                .derive_snapshot_id()
                .expect("reseal ledger snapshot");
            settlement.settlement_id = settlement
                .derive_settlement_id()
                .expect("reseal settlement");
            let encoded = norito::to_bytes(&settlement).expect("encode settlement");
            publisher
                .publish_deal_settlement(&settlement, &encoded)
                .expect("publish settlement into runtime DAG");
        }

        let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
        let head_at_window: GovernanceDagHeadV1 =
            norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
        assert_eq!(
            head_at_window.block_count,
            GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 as u64
        );
        assert_eq!(head_at_window.checkpoint_cid, None);

        let mut settlement = template;
        settlement.deal_id = [0xFF; 32];
        settlement.ledger.deal_id = settlement.deal_id;
        settlement.ledger.snapshot_id = settlement
            .ledger
            .derive_snapshot_id()
            .expect("reseal ledger snapshot");
        settlement.settlement_id = settlement
            .derive_settlement_id()
            .expect("reseal settlement");
        let encoded = norito::to_bytes(&settlement).expect("encode settlement");
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("publish first checkpointed settlement");

        let index = runtime_index(temp.path());
        let blocks = runtime_blocks_from_index(temp.path(), &index);
        assert_eq!(
            blocks.len(),
            GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 + 1,
            "checkpointing must not truncate the root history"
        );
        assert_eq!(blocks[0].sequence, 0);
        assert_eq!(blocks[0].prev_block_cid, None);
        assert_eq!(blocks[0].node.prev_cid, None);
        for (position, pair) in blocks.windows(2).enumerate() {
            assert_eq!(pair[1].sequence, (position + 1) as u64);
            assert_eq!(pair[1].prev_block_cid, Some(pair[0].block_cid.clone()));
            assert_eq!(pair[1].node.prev_cid, Some(pair[0].node.node_cid.clone()));
        }

        let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
        let head: GovernanceDagHeadV1 =
            norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
        assert_eq!(head.block_count, blocks.len() as u64);
        assert_eq!(head.checkpoint_cid, Some(blocks[1].block_cid.clone()));
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("full root chain validates against checkpointed head");
        validate_governance_dag_head_against_chain_v1(
            &head,
            &blocks[blocks.len() - GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1..],
        )
        .expect("canonical checkpoint tail validates against checkpointed head");

        let governed_public_key = &head.head_signature.public_key;
        assert_eq!(
            head.head_signature.algorithm,
            GovernanceSignatureAlgorithm::Ed25519
        );
        for block in &blocks {
            assert_eq!(block.publisher_peer_id, head.publisher_peer_id);
            assert_eq!(block.node.publisher_peer_id, head.publisher_peer_id);
            assert_eq!(
                block.block_signature.algorithm,
                GovernanceSignatureAlgorithm::Ed25519
            );
            assert_eq!(
                block.node.publisher_signature.algorithm,
                GovernanceSignatureAlgorithm::Ed25519
            );
            assert_eq!(&block.block_signature.public_key, governed_public_key);
            assert_eq!(
                &block.node.publisher_signature.public_key,
                governed_public_key
            );
        }
    }

    #[test]
    fn filesystem_publisher_writes_moderation_ballot_event_files_and_runtime_dag() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (event, encoded) = sample_moderation_ballot_event();

        publisher
            .publish_moderation_ballot_event(&event, &encoded)
            .expect("publish moderation ballot event");

        let ballot_dir = temp
            .path()
            .join("moderation")
            .join("ballots")
            .join("case-42")
            .join("round-1");
        let mut encoded_files = fs::read_dir(&ballot_dir)
            .expect("read moderation ballot dir")
            .map(|entry| entry.expect("dir entry").path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("to"))
            .collect::<Vec<_>>();
        encoded_files.sort();
        assert_eq!(encoded_files.len(), 1);
        let bytes = fs::read(&encoded_files[0]).expect("read moderation event payload");
        assert_eq!(bytes, encoded);
        let decoded: SoraFsModerationBallotGovernanceEventV1 =
            norito::decode_from_bytes(&bytes).expect("decode moderation event payload");
        assert_eq!(decoded, event);
        assert!(encoded_files[0].with_extension("json").exists());

        let index_bytes =
            fs::read(temp.path().join(GOVERNANCE_PUBLISH_INDEX_FILE)).expect("publish index");
        let index: JsonValue = json::from_slice(&index_bytes).expect("publish index json");
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("moderation_ballot_event"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );

        let runtime_index = runtime_index(temp.path());
        assert_eq!(
            runtime_index
                .get("by_payload_kind")
                .and_then(|value| value.get("moderation_ballot_event"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
        let head: GovernanceDagHeadV1 =
            norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
        let blocks = runtime_blocks_from_index(temp.path(), &runtime_index);
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("runtime head validates against signed blocks");
        assert_eq!(blocks.len(), 1);
        match &blocks[0].node.payload {
            GovernanceLogPayloadV1::ModerationBallotEvent(value) => {
                assert_eq!(value.case_id, event.case_id);
                assert_eq!(value.round_id, event.round_id);
                assert_eq!(value.kind, event.kind);
            }
            other => panic!("unexpected runtime DAG payload: {other:?}"),
        }
    }

    #[test]
    fn filesystem_publisher_writes_transparency_ledger_publication_files_and_car_queue() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (publication, encoded) = sample_transparency_ledger_publication();

        publisher
            .publish_transparency_ledger_publication(&publication, &encoded)
            .expect("publish transparency ledger publication");

        let publication_dir = temp
            .path()
            .join("transparency")
            .join("ledger")
            .join(hex::encode(publication.block.cycle_id));
        let mut encoded_files = fs::read_dir(&publication_dir)
            .expect("read transparency ledger dir")
            .map(|entry| entry.expect("dir entry").path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("to"))
            .collect::<Vec<_>>();
        encoded_files.sort();
        assert_eq!(encoded_files.len(), 1);
        let bytes = fs::read(&encoded_files[0]).expect("read transparency ledger payload");
        assert_eq!(bytes, encoded);
        let decoded: ModerationLedgerCyclePublicationV1 =
            norito::decode_from_bytes(&bytes).expect("decode transparency ledger publication");
        assert_eq!(decoded, publication);
        assert!(encoded_files[0].with_extension("json").exists());

        let index_bytes =
            fs::read(temp.path().join(GOVERNANCE_PUBLISH_INDEX_FILE)).expect("publish index");
        let index: JsonValue = json::from_slice(&index_bytes).expect("publish index json");
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("transparency_ledger_publication"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        let entry = index
            .get("entries")
            .and_then(JsonValue::as_array)
            .and_then(|entries| entries.first())
            .and_then(JsonValue::as_object)
            .expect("publish index entry");
        let labels = entry
            .get("labels")
            .and_then(JsonValue::as_object)
            .expect("publish labels");
        let expected_cycle_id = hex::encode(publication.block.cycle_id);
        assert_eq!(
            labels.get("cycle_id_hex").and_then(JsonValue::as_str),
            Some(expected_cycle_id.as_str())
        );
        assert_eq!(
            labels.get("entry_count").and_then(JsonValue::as_u64),
            Some(u64::from(publication.block.entry_count))
        );

        let queue_bytes = fs::read(temp.path().join(GOVERNANCE_CAR_QUEUE_FILE)).expect("car queue");
        let queue: JsonValue = json::from_slice(&queue_bytes).expect("car queue json");
        assert_eq!(
            queue
                .get("by_payload_kind")
                .and_then(|value| value.get("transparency_ledger_publication"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            queue.get("assembled_count").and_then(JsonValue::as_u64),
            Some(1)
        );
    }

    #[test]
    fn filesystem_publisher_writes_proof_token_issuance_files_and_car_queue() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (issuance, encoded) = sample_proof_token_issuance();

        publisher
            .publish_proof_token_issuance(&issuance, &encoded)
            .expect("publish proof-token issuance");

        let token_id_hex = hex::encode(issuance.token_id);
        let issuance_dir = temp
            .path()
            .join("transparency")
            .join("proof-tokens")
            .join(&token_id_hex);
        let mut encoded_files = fs::read_dir(&issuance_dir)
            .expect("read proof-token issuance dir")
            .map(|entry| entry.expect("dir entry").path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("to"))
            .collect::<Vec<_>>();
        encoded_files.sort();
        assert_eq!(encoded_files.len(), 1);
        let bytes = fs::read(&encoded_files[0]).expect("read proof-token issuance payload");
        assert_eq!(bytes, encoded);
        let decoded: ProofTokenIssuanceV1 =
            norito::decode_from_bytes(&bytes).expect("decode proof-token issuance");
        assert_eq!(decoded, issuance);

        let json_path = encoded_files[0].with_extension("json");
        assert!(json_path.exists());
        let json_body = fs::read(&json_path).expect("read proof-token issuance json");
        let json_value: JsonValue = json::from_slice(&json_body).expect("issuance json");
        assert_eq!(
            json_value
                .get("metadata")
                .and_then(|value| value.get("token_id_hex"))
                .and_then(JsonValue::as_str),
            Some(token_id_hex.as_str())
        );

        let index_bytes =
            fs::read(temp.path().join(GOVERNANCE_PUBLISH_INDEX_FILE)).expect("publish index");
        let index: JsonValue = json::from_slice(&index_bytes).expect("publish index json");
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("proof_token_issuance"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        let entry = index
            .get("entries")
            .and_then(JsonValue::as_array)
            .and_then(|entries| entries.first())
            .and_then(JsonValue::as_object)
            .expect("publish index entry");
        let labels = entry
            .get("labels")
            .and_then(JsonValue::as_object)
            .expect("publish labels");
        assert_eq!(
            labels.get("token_id_hex").and_then(JsonValue::as_str),
            Some(token_id_hex.as_str())
        );
        assert_eq!(
            labels.get("entry_count").and_then(JsonValue::as_u64),
            Some(2)
        );
        assert_single_runtime_external(temp.path(), "proof_token_issuance", &encoded);

        let queue_bytes = fs::read(temp.path().join(GOVERNANCE_CAR_QUEUE_FILE)).expect("car queue");
        let queue: JsonValue = json::from_slice(&queue_bytes).expect("car queue json");
        assert_eq!(
            queue
                .get("by_payload_kind")
                .and_then(|value| value.get("proof_token_issuance"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            queue.get("assembled_count").and_then(JsonValue::as_u64),
            Some(1)
        );
    }

    #[test]
    fn filesystem_publisher_writes_appeal_finance_report_files_and_runtime_dag() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (report, encoded) = sample_appeal_finance_report();

        publisher
            .publish_appeal_finance_report(&report, &encoded)
            .expect("publish appeal finance report");

        let report_dir = temp.path().join("appeals").join("finance").join("case-42");
        let mut encoded_files = fs::read_dir(&report_dir)
            .expect("read appeal finance dir")
            .map(|entry| entry.expect("dir entry").path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("to"))
            .collect::<Vec<_>>();
        encoded_files.sort();
        assert_eq!(encoded_files.len(), 1);
        let bytes = fs::read(&encoded_files[0]).expect("read appeal finance report payload");
        assert_eq!(bytes, encoded);
        let decoded: SoraFsAppealFinanceReportV1 =
            norito::decode_from_bytes(&bytes).expect("decode appeal finance report");
        assert_eq!(decoded, report);
        assert!(encoded_files[0].with_extension("json").exists());

        let index_bytes =
            fs::read(temp.path().join(GOVERNANCE_PUBLISH_INDEX_FILE)).expect("publish index");
        let index: JsonValue = json::from_slice(&index_bytes).expect("publish index json");
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("appeal_finance_report"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );

        let runtime_index = runtime_index(temp.path());
        assert_eq!(
            runtime_index
                .get("by_payload_kind")
                .and_then(|value| value.get("appeal_finance_report"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
        let head: GovernanceDagHeadV1 =
            norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
        let blocks = runtime_blocks_from_index(temp.path(), &runtime_index);
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("runtime head validates against signed blocks");
        assert_eq!(blocks.len(), 1);
        match &blocks[0].node.payload {
            GovernanceLogPayloadV1::AppealFinanceReport(value) => {
                assert_eq!(value.report_id, report.report_id);
                assert_eq!(value.case_id, report.case_id);
                assert_eq!(value.outcome, report.outcome);
            }
            other => panic!("unexpected runtime DAG payload: {other:?}"),
        }
    }

    #[test]
    fn filesystem_publisher_writes_appeal_finance_weekly_rollup_files_and_runtime_dag() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (rollup, encoded) = sample_appeal_finance_weekly_rollup();

        publisher
            .publish_appeal_finance_weekly_rollup(&rollup, &encoded)
            .expect("publish appeal finance weekly rollup");

        let rollup_dir = temp
            .path()
            .join("appeals")
            .join("finance")
            .join("weekly")
            .join("2026-W26");
        let mut encoded_files = fs::read_dir(&rollup_dir)
            .expect("read appeal finance weekly rollup dir")
            .map(|entry| entry.expect("dir entry").path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("to"))
            .collect::<Vec<_>>();
        encoded_files.sort();
        assert_eq!(encoded_files.len(), 1);
        let bytes = fs::read(&encoded_files[0]).expect("read appeal finance weekly rollup payload");
        assert_eq!(bytes, encoded);
        let decoded: SoraFsAppealFinanceWeeklyRollupV1 =
            norito::decode_from_bytes(&bytes).expect("decode appeal finance weekly rollup");
        assert_eq!(decoded, rollup);
        let json_path = encoded_files[0].with_extension("json");
        assert!(json_path.exists());
        let json_body = fs::read(&json_path).expect("read appeal finance weekly rollup json");
        let json_value: JsonValue = json::from_slice(&json_body).expect("weekly rollup json");
        assert_eq!(
            json_value
                .get("metadata")
                .and_then(|value| value.get("cycle"))
                .and_then(JsonValue::as_str),
            Some("2026-W26")
        );

        let index_bytes =
            fs::read(temp.path().join(GOVERNANCE_PUBLISH_INDEX_FILE)).expect("publish index");
        let index: JsonValue = json::from_slice(&index_bytes).expect("publish index json");
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("appeal_finance_weekly_rollup"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );

        let runtime_index = runtime_index(temp.path());
        assert_eq!(
            runtime_index
                .get("by_payload_kind")
                .and_then(|value| value.get("appeal_finance_weekly_rollup"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
        let head: GovernanceDagHeadV1 =
            norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
        let blocks = runtime_blocks_from_index(temp.path(), &runtime_index);
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("runtime head validates against signed blocks");
        assert_eq!(blocks.len(), 1);
        match &blocks[0].node.payload {
            GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(value) => {
                assert_eq!(value.cycle, rollup.cycle);
                assert_eq!(value.report_count, rollup.report_count);
                assert_eq!(value.total_deposit_xor, rollup.total_deposit_xor);
            }
            other => panic!("unexpected runtime DAG payload: {other:?}"),
        }
    }

    #[test]
    fn filesystem_publisher_writes_appeal_finance_settlement_receipt_files_and_runtime_dag() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (receipt, encoded) = sample_appeal_finance_settlement_receipt();

        publisher
            .publish_appeal_finance_settlement_receipt(&receipt, &encoded)
            .expect("publish appeal finance settlement receipt");

        let receipt_dir = temp
            .path()
            .join("appeals")
            .join("finance")
            .join("settlement-receipts")
            .join("case-42");
        let mut encoded_files = fs::read_dir(&receipt_dir)
            .expect("read appeal finance settlement receipt dir")
            .map(|entry| entry.expect("dir entry").path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("to"))
            .collect::<Vec<_>>();
        encoded_files.sort();
        assert_eq!(encoded_files.len(), 1);
        let bytes = fs::read(&encoded_files[0]).expect("read settlement receipt payload");
        assert_eq!(bytes, encoded);
        let decoded: SoraFsAppealFinanceSettlementReceiptV1 =
            norito::decode_from_bytes(&bytes).expect("decode settlement receipt");
        assert_eq!(decoded, receipt);
        let json_path = encoded_files[0].with_extension("json");
        assert!(json_path.exists());
        let json_body = fs::read(&json_path).expect("read settlement receipt json");
        let json_value: JsonValue = json::from_slice(&json_body).expect("receipt json");
        let expected_policy_digest_hex = hex::encode(receipt.appeal_finance_policy_digest);
        assert_eq!(
            json_value
                .get("metadata")
                .and_then(|value| value.get("tx_hash_hex"))
                .and_then(JsonValue::as_str),
            Some(receipt.tx_hash_hex.as_str())
        );
        assert_eq!(
            json_value
                .get("metadata")
                .and_then(|value| value.get("appeal_finance_policy_digest_hex"))
                .and_then(JsonValue::as_str),
            Some(expected_policy_digest_hex.as_str())
        );

        let index_bytes =
            fs::read(temp.path().join(GOVERNANCE_PUBLISH_INDEX_FILE)).expect("publish index");
        let index: JsonValue = json::from_slice(&index_bytes).expect("publish index json");
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("appeal_finance_settlement_receipt"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            index
                .get("entries")
                .and_then(JsonValue::as_array)
                .and_then(|entries| entries.first())
                .and_then(|entry| entry.get("labels"))
                .and_then(|labels| labels.get("appeal_finance_policy_digest_hex"))
                .and_then(JsonValue::as_str),
            Some(expected_policy_digest_hex.as_str())
        );

        let runtime_index = runtime_index(temp.path());
        assert_eq!(
            runtime_index
                .get("by_payload_kind")
                .and_then(|value| value.get("appeal_finance_settlement_receipt"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );
        let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
        let head: GovernanceDagHeadV1 =
            norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
        let blocks = runtime_blocks_from_index(temp.path(), &runtime_index);
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("runtime head validates against signed blocks");
        assert_eq!(blocks.len(), 1);
        match &blocks[0].node.payload {
            GovernanceLogPayloadV1::AppealFinanceSettlementReceipt(value) => {
                assert_eq!(value.receipt_id, receipt.receipt_id);
                assert_eq!(value.case_id, receipt.case_id);
                assert_eq!(value.submitted_step, receipt.submitted_step);
            }
            other => panic!("unexpected runtime DAG payload: {other:?}"),
        }
    }

    #[test]
    fn filesystem_publisher_rejects_malformed_runtime_dag_index() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        fs::write(
            temp.path().join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE),
            br#"{"schema":"sorafs.governance_dag.wrong","blocks":[]}"#,
        )
        .expect("write bad runtime index");
        let (settlement, encoded) = sample_settlement();

        let err = publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect_err("malformed runtime DAG index must fail closed");
        assert!(
            err.to_string().contains("unsupported schema"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn filesystem_publisher_writes_settlement_files() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());

        let (settlement, encoded) = sample_settlement();

        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("publish");

        let deal_hex = settlement.deal_id.encode_hex::<String>();
        let dir = temp.path().join("settlements").join(deal_hex);

        let entries = fs::read_dir(&dir)
            .expect("directory exists")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 4, "expected encoded + json + digests");

        let mut encoded_paths = entries
            .iter()
            .filter(|path| path.extension().map(|ext| ext == "to").unwrap_or(false));
        let encoded_path = encoded_paths.next().expect("encoded artefact present");
        assert_eq!(
            fs::read(encoded_path).expect("read encoded"),
            encoded,
            "encoded payload must match original bytes"
        );

        let json_path = entries
            .iter()
            .find(|path| path.extension().map(|ext| ext == "json").unwrap_or(false))
            .expect("json artefact present");
        let json_bytes = fs::read(json_path).expect("read json");
        let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
        let status = value
            .get("metadata")
            .and_then(|meta| meta.get("status"))
            .and_then(JsonValue::as_str)
            .expect("status");
        assert_eq!(status, "completed");

        let encoded_digest_path = entries
            .iter()
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.ends_with("to.blake3"))
                    .unwrap_or(false)
            })
            .expect("encoded digest present");
        let encoded_digest = fs::read_to_string(encoded_digest_path).expect("read encoded digest");
        let encoded_digest = encoded_digest.trim();
        assert_eq!(encoded_digest, blake3::hash(&encoded).to_hex().as_str());

        let json_digest_path = entries
            .iter()
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.ends_with("json.blake3"))
                    .unwrap_or(false)
            })
            .expect("json digest present");
        let json_digest = fs::read_to_string(json_digest_path).expect("read json digest");
        let json_digest = json_digest.trim();
        assert_eq!(json_digest, blake3::hash(&json_bytes).to_hex().as_str());

        let index_path = temp.path().join(GOVERNANCE_PUBLISH_INDEX_FILE);
        let index_bytes = fs::read(&index_path).expect("read publish index");
        let index: JsonValue = norito::json::from_slice(&index_bytes).expect("index json");
        assert_eq!(
            index.get("schema").and_then(JsonValue::as_str),
            Some(GOVERNANCE_PUBLISH_INDEX_SCHEMA)
        );
        assert_eq!(
            index.get("entry_count").and_then(JsonValue::as_u64),
            Some(1)
        );
        assert_eq!(
            index
                .get("payload_kind_counts")
                .and_then(JsonValue::as_object)
                .and_then(|counts| counts.get("deal_settlement"))
                .and_then(JsonValue::as_u64),
            Some(1)
        );
        let digest_hex = blake3::hash(&encoded).to_hex().to_string();
        let digest_positions = index
            .get("by_encoded_blake3")
            .and_then(JsonValue::as_object)
            .and_then(|map| map.get(digest_hex.as_str()))
            .and_then(JsonValue::as_array)
            .expect("digest lookup");
        assert_eq!(digest_positions.len(), 1);
        assert_eq!(digest_positions[0].as_u64(), Some(0));
        let kind_positions = index
            .get("by_payload_kind")
            .and_then(JsonValue::as_object)
            .and_then(|map| map.get("deal_settlement"))
            .and_then(JsonValue::as_array)
            .expect("kind lookup");
        assert_eq!(kind_positions[0].as_u64(), Some(0));
        let entry = index
            .get("entries")
            .and_then(JsonValue::as_array)
            .and_then(|entries| entries.first())
            .and_then(JsonValue::as_object)
            .expect("first index entry");
        assert_eq!(
            entry.get("payload_kind").and_then(JsonValue::as_str),
            Some("deal_settlement")
        );
        assert_eq!(
            entry.get("encoded_path").and_then(JsonValue::as_str),
            Some(index_path_string(temp.path(), encoded_path).as_str())
        );
        assert_eq!(
            entry
                .get("labels")
                .and_then(JsonValue::as_object)
                .and_then(|labels| labels.get("status"))
                .and_then(JsonValue::as_str),
            Some("completed")
        );
        let index_digest_path = index_path.with_extension("json.blake3");
        let index_digest = fs::read_to_string(index_digest_path).expect("read index digest");
        assert_eq!(
            index_digest.trim(),
            blake3::hash(&index_bytes).to_hex().as_str()
        );

        let queue_path = temp.path().join(GOVERNANCE_CAR_QUEUE_FILE);
        let queue_bytes = fs::read(&queue_path).expect("read CAR queue");
        let queue: JsonValue = norito::json::from_slice(&queue_bytes).expect("queue json");
        assert_eq!(
            queue.get("schema").and_then(JsonValue::as_str),
            Some(GOVERNANCE_CAR_QUEUE_SCHEMA)
        );
        assert_eq!(
            queue.get("segment_count").and_then(JsonValue::as_u64),
            Some(1)
        );
        assert_eq!(
            queue.get("assembled_count").and_then(JsonValue::as_u64),
            Some(1)
        );
        let queue_digest_path = queue_path.with_extension("json.blake3");
        let queue_digest = fs::read_to_string(queue_digest_path).expect("read queue digest");
        assert_eq!(
            queue_digest.trim(),
            blake3::hash(&queue_bytes).to_hex().as_str()
        );
        let segment = queue
            .get("segments")
            .and_then(JsonValue::as_array)
            .and_then(|segments| segments.first())
            .and_then(JsonValue::as_object)
            .expect("first CAR segment");
        assert_eq!(
            segment.get("schema").and_then(JsonValue::as_str),
            Some(GOVERNANCE_CAR_SEGMENT_SCHEMA)
        );
        assert_eq!(
            segment.get("status").and_then(JsonValue::as_str),
            Some("assembled")
        );
        assert_eq!(
            segment
                .get("source_publish_index_position")
                .and_then(JsonValue::as_u64),
            Some(0)
        );
        assert_eq!(
            segment.get("encoded_blake3").and_then(JsonValue::as_str),
            Some(digest_hex.as_str())
        );
        let car_path = resolve_index_path(
            temp.path(),
            segment
                .get("car_path")
                .and_then(JsonValue::as_str)
                .expect("car path"),
        )
        .expect("resolve car path");
        let car_bytes = fs::read(&car_path).expect("read CAR segment");
        assert_eq!(
            segment.get("car_size").and_then(JsonValue::as_u64),
            Some(car_bytes.len() as u64)
        );
        assert_eq!(
            segment
                .get("car_archive_blake3")
                .and_then(JsonValue::as_str),
            Some(blake3::hash(&car_bytes).to_hex().as_str())
        );
        let car_digest =
            fs::read_to_string(digest_sidecar_path_for(&car_path)).expect("read car sidecar");
        assert_eq!(
            car_digest.trim(),
            blake3::hash(&car_bytes).to_hex().as_str()
        );

        let plan_path = resolve_index_path(
            temp.path(),
            segment
                .get("plan_path")
                .and_then(JsonValue::as_str)
                .expect("plan path"),
        )
        .expect("resolve plan path");
        let plan_bytes = fs::read(&plan_path).expect("read CAR plan");
        let plan: JsonValue = norito::json::from_slice(&plan_bytes).expect("plan json");
        assert_eq!(
            plan.get("schema").and_then(JsonValue::as_str),
            Some(GOVERNANCE_CAR_PLAN_SCHEMA)
        );
        assert_eq!(
            plan.get("source_publish_index_position")
                .and_then(JsonValue::as_u64),
            Some(0)
        );
        assert_eq!(
            plan.get("files")
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(4)
        );
        assert!(
            plan.get("chunks")
                .and_then(JsonValue::as_array)
                .is_some_and(|chunks| !chunks.is_empty()),
            "CAR plan should expose deterministic chunks"
        );
        let manifest_path = resolve_index_path(
            temp.path(),
            segment
                .get("manifest_path")
                .and_then(JsonValue::as_str)
                .expect("manifest path"),
        )
        .expect("resolve segment manifest path");
        let manifest_bytes = fs::read(&manifest_path).expect("read segment manifest");
        let manifest: JsonValue =
            norito::json::from_slice(&manifest_bytes).expect("segment manifest json");
        assert_eq!(
            manifest.get("schema").and_then(JsonValue::as_str),
            Some(GOVERNANCE_CAR_SEGMENT_SCHEMA)
        );

        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("republish same settlement");
        let index_bytes = fs::read(&index_path).expect("read republished index");
        let index: JsonValue = norito::json::from_slice(&index_bytes).expect("index json");
        assert_eq!(
            index.get("entry_count").and_then(JsonValue::as_u64),
            Some(1),
            "republishing the same artifact must not duplicate the index entry"
        );
        let queue_bytes = fs::read(&queue_path).expect("read republished queue");
        let queue: JsonValue = norito::json::from_slice(&queue_bytes).expect("queue json");
        assert_eq!(
            queue.get("segment_count").and_then(JsonValue::as_u64),
            Some(1),
            "republishing the same artifact must not duplicate the CAR queue segment"
        );
    }

    #[test]
    fn filesystem_publisher_settlement_json_preserves_exact_wide_quantities() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (mut settlement, _) = sample_settlement();
        let wide = xor("340282366920938463463374607431768211456");
        let sub_micro = xor("0.0000001");
        let applied = xor("0.00000004");
        let client_debit = xor("0.00000006");
        let slash = xor("0.000000001");
        let satisfied_without_outstanding = applied
            .checked_add(&client_debit)
            .and_then(|amount| amount.checked_add(&slash))
            .expect("fixture liability components");
        let outstanding = wide
            .checked_sub(&satisfied_without_outstanding)
            .expect("wide liability exceeds fixture payments");
        settlement.status = DealSettlementStatusV1::WindowSettled;
        settlement.ledger.deal_end_epoch = settlement.ledger.window_end_epoch + 10;
        settlement.ledger.provider_accrual = "0.0000001".parse().expect("sub-micro quantity");
        settlement.ledger.client_liability = wide.clone();
        settlement.ledger.micropayment_credit_generated = applied.clone();
        settlement.ledger.micropayment_credit_applied = applied.clone();
        settlement.ledger.micropayment_credit_carry = XorQuantity::zero();
        settlement.ledger.client_debit = client_debit.clone();
        settlement.ledger.outstanding_liability = outstanding;
        settlement.ledger.bond_total = xor("1.000000002");
        settlement.ledger.bond_locked = xor("1.000000001");
        settlement.ledger.bond_slashed = slash.clone();
        settlement.ledger.bond_released = XorQuantity::zero();
        settlement.ledger.window_expected_charge = wide;
        settlement.ledger.window_micropayment_generated = applied.clone();
        settlement.ledger.window_micropayment_applied = applied;
        settlement.ledger.window_client_debit = client_debit;
        settlement.ledger.window_bond_slashed = slash;
        settlement.ledger.window_bond_released = XorQuantity::zero();
        settlement.audit_notes = Some("exact wide-quantity settlement fixture".to_owned());
        assert_eq!(settlement.ledger.provider_accrual, sub_micro);
        settlement.ledger.snapshot_id = settlement.ledger.derive_snapshot_id().expect("ledger id");
        settlement.settlement_id = settlement.derive_settlement_id().expect("settlement id");
        settlement
            .validate_transition(None)
            .expect("coherent exact settlement fixture");
        let encoded = norito::to_bytes(&settlement).expect("encode canonical settlement");

        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("publish exact settlement");

        let dir = temp
            .path()
            .join("settlements")
            .join(settlement.deal_id.encode_hex::<String>());
        let json_path = fs::read_dir(dir)
            .expect("settlement directory")
            .map(|entry| entry.expect("dir entry").path())
            .find(|path| path.extension().is_some_and(|ext| ext == "json"))
            .expect("settlement json");
        let body = fs::read(json_path).expect("read settlement json");
        let value: JsonValue = json::from_slice(&body).expect("parse settlement json");
        let object = value
            .get("settlement")
            .and_then(JsonValue::as_object)
            .expect("settlement object");
        for (field, expected) in [
            ("provider_accrual", "0.0000001"),
            (
                "client_liability",
                "340282366920938463463374607431768211456",
            ),
            ("bond_locked", "1.000000001"),
            ("bond_slashed", "0.000000001"),
        ] {
            assert_eq!(
                object.get(field).and_then(JsonValue::as_str),
                Some(expected),
                "exact quantity field {field}"
            );
        }
        for retired in [
            "provider_accrual_micro",
            "client_liability_micro",
            "bond_locked_micro",
            "bond_slashed_micro",
        ] {
            assert!(!object.contains_key(retired), "retired field {retired}");
        }
    }

    #[test]
    fn filesystem_publisher_rejects_malformed_car_queue() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (settlement, encoded) = sample_settlement();
        fs::write(
            temp.path().join(GOVERNANCE_CAR_QUEUE_FILE),
            br#"{"schema":"wrong","segments":[]}"#,
        )
        .expect("write malformed queue");

        let err = publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect_err("malformed CAR queue must fail closed");
        assert!(
            err.to_string()
                .contains("governance CAR queue uses an unsupported schema"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn atomic_temp_path_preserves_extensions_and_hides_file() {
        let base = Path::new("/tmp/settlement/artifact.norito.to");
        let tmp = temp_path_for_atomic(base, 42, 7);
        let tmp_name = tmp
            .file_name()
            .and_then(|name| name.to_str())
            .expect("name");
        assert!(
            tmp_name.starts_with(".artifact.norito.to.tmp-42-7"),
            "tmp name should keep extensions and add suffix, got {tmp_name}"
        );
        assert!(
            tmp.as_os_str()
                .to_string_lossy()
                .ends_with(".norito.to.tmp-42-7"),
            "tmp path should append to existing extensions"
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_rejects_symlink_output() {
        let dir = tempdir().expect("tempdir");
        let temp_path = canonical_temp_path(&dir);
        let target_path = temp_path.join("target.to");
        fs::write(&target_path, b"unchanged\n").expect("write target");
        let output_path = temp_path.join("governance.to");
        std::os::unix::fs::symlink(&target_path, &output_path).expect("create symlink");

        let err = write_atomic(&output_path, b"replace").expect_err("reject symlink output");
        let message = err.to_string();

        assert!(
            message.contains("must not be a symlink"),
            "unexpected error: {message}"
        );
        assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged\n");
    }

    #[test]
    fn write_atomic_surfaces_post_rename_directory_sync_failure() {
        let dir = tempdir().expect("tempdir");
        let output_path = dir.path().join("governance.to");
        let error = write_atomic_with_directory_sync(&output_path, b"committed", |_| {
            Err(io::Error::other("injected directory sync failure"))
        })
        .expect_err("directory sync failure must be reported");

        assert!(
            error
                .to_string()
                .contains("injected directory sync failure")
        );
        assert_eq!(
            fs::read(&output_path).expect("renamed output remains visible"),
            b"committed",
            "the caller must treat this as committed-unknown and retry idempotently"
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_rejects_symlink_parent() {
        let dir = tempdir().expect("tempdir");
        let temp_path = canonical_temp_path(&dir);
        let real_dir = temp_path.join("real");
        fs::create_dir(&real_dir).expect("create real dir");
        let linked_dir = temp_path.join("linked");
        std::os::unix::fs::symlink(&real_dir, &linked_dir).expect("create symlink");
        let output_path = linked_dir.join("governance.to");

        let err = write_atomic(&output_path, b"replace").expect_err("reject symlink parent");
        let message = err.to_string();

        assert!(
            message.contains("parent") && message.contains("must not be a symlink"),
            "unexpected error: {message}"
        );
        assert!(
            !real_dir.join("governance.to").exists(),
            "symlink parent should not receive output"
        );
    }

    #[cfg(unix)]
    #[test]
    fn open_atomic_temp_file_rejects_preexisting_symlink() {
        let dir = tempdir().expect("tempdir");
        let temp_path = canonical_temp_path(&dir);
        let target_path = temp_path.join("target.tmp");
        fs::write(&target_path, b"unchanged\n").expect("write target");
        let tmp_path = temp_path.join(".governance.to.tmp");
        std::os::unix::fs::symlink(&target_path, &tmp_path).expect("create symlink");

        let err = open_atomic_temp_file(&tmp_path).expect_err("reject temp symlink");
        let message = err.to_string();

        assert!(
            message.contains("failed to create atomic temp"),
            "unexpected error: {message}"
        );
        assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged\n");
    }

    #[test]
    fn filesystem_publisher_writes_gc_audit_files() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());

        let payload = GcAuditPayloadV1 {
            version: GC_AUDIT_PAYLOAD_VERSION_V1,
            manifest_digest: [0x33; 32],
            provider_id: [0x44; 32],
            evicted_at_unix: 1_700_000_333,
            freed_bytes: 4_096,
            reason: "retention_expired".into(),
            blocked_reason: None,
        };
        let header = SorafsAuditHeaderV1 {
            sequence: 7,
            occurred_at_unix: payload.evicted_at_unix,
            signer: GC_AUDIT_SIGNER_V1.into(),
            payload_digest: gc_audit_payload_digest_v1(&payload).expect("audit digest"),
        };
        let event = GcAuditEventV1 {
            version: GC_AUDIT_EVENT_VERSION_V1,
            header,
            payload,
        };
        let encoded = norito::to_bytes(&event).expect("encode GC audit event");

        publisher
            .publish_gc_audit_event(&event, &encoded)
            .expect("publish gc audit");

        let dir = temp.path().join("gc").join("audit");
        let entries = fs::read_dir(&dir)
            .expect("directory exists")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 4, "expected encoded + json + digests");

        let json_path = entries
            .iter()
            .find(|path| path.extension().map(|ext| ext == "json").unwrap_or(false))
            .expect("json artefact present");
        let json_bytes = fs::read(json_path).expect("read json");
        let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
        let reason = value
            .get("metadata")
            .and_then(|meta| meta.get("reason"))
            .and_then(JsonValue::as_str)
            .expect("reason");
        assert_eq!(reason, "retention_expired");
        assert_single_runtime_external(temp.path(), "gc_audit", &encoded);
    }

    #[test]
    fn filesystem_publisher_writes_reconciliation_report_files() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());

        let report = SorafsReconciliationReportV1 {
            version: SORAFS_RECONCILIATION_REPORT_VERSION_V1,
            provider_id: [0x55; 32],
            generated_at_unix: 1_700_000_444,
            repair_snapshot_hash: [0x01; 32],
            retention_snapshot_hash: [0x02; 32],
            gc_snapshot_hash: [0x03; 32],
            repair_task_count: 2,
            retention_manifest_count: 3,
            gc_evictions_total: 4,
            gc_freed_bytes_total: 5,
            divergence_count: 1,
            appeal_finance: None,
        };
        let encoded = norito::to_bytes(&report).expect("encode reconciliation report");

        publisher
            .publish_reconciliation_report(&report, &encoded)
            .expect("publish reconciliation report");

        let dir = temp.path().join("reconciliation");
        let entries = fs::read_dir(&dir)
            .expect("directory exists")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 4, "expected encoded + json + digests");

        let json_path = entries
            .iter()
            .find(|path| path.extension().map(|ext| ext == "json").unwrap_or(false))
            .expect("json artefact present");
        let json_bytes = fs::read(json_path).expect("read json");
        let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
        let metadata = value
            .get("metadata")
            .and_then(JsonValue::as_object)
            .expect("metadata");
        let provider = metadata
            .get("provider")
            .and_then(JsonValue::as_str)
            .expect("provider");
        let divergence = metadata
            .get("divergence_count")
            .and_then(JsonValue::as_u64)
            .expect("divergence_count");
        assert_eq!(provider, hex::encode(report.provider_id));
        assert_eq!(divergence, 1);
        assert_single_runtime_external(temp.path(), "reconciliation", &encoded);
    }

    #[test]
    fn filesystem_publisher_writes_reputation_snapshot_files() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (snapshot, encoded) = sample_reputation_snapshot();

        publisher
            .publish_reputation_snapshot(&snapshot, &encoded)
            .expect("publish reputation snapshot");

        let snapshot_hex = hex::encode(snapshot.snapshot.snapshot_id);
        let dir = temp
            .path()
            .join("reputation")
            .join("snapshots")
            .join(&snapshot_hex);
        let entries = fs::read_dir(&dir)
            .expect("snapshot directory exists")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 4, "expected encoded + json + digests");

        let latest_to = temp.path().join("reputation").join("latest.to");
        assert_eq!(
            fs::read(&latest_to).expect("read latest reputation snapshot"),
            encoded,
            "latest pointer must contain canonical Norito bytes"
        );

        let latest_json = temp.path().join("reputation").join("latest.json");
        let json_bytes = fs::read(latest_json).expect("read latest reputation json");
        let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
        let metadata = value
            .get("metadata")
            .and_then(JsonValue::as_object)
            .expect("metadata");
        assert_eq!(
            metadata.get("snapshot_id_hex").and_then(JsonValue::as_str),
            Some(snapshot_hex.as_str())
        );
        assert_eq!(
            metadata.get("provider_count").and_then(JsonValue::as_u64),
            Some(snapshot.snapshot.providers.len() as u64)
        );
    }
}
