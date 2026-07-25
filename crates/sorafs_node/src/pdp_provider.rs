//! Durable admission-bound Sora-PDP provider challenge and proof lifecycle.
//!
//! The runtime in this module is deliberately transport-neutral. Torii supplies
//! council-verified admission records, while the runtime owns canonical replay
//! protection, deadline enforcement, exhaustive proof verification, durable
//! terminal handoff state, and deterministic queue ordering.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write as _},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

#[cfg(unix)]
use std::os::unix::{
    fs::{DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    io::AsRawFd as _,
};

use iroha_crypto::{Algorithm, KeyPair, Signature as IrohaSignature};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use sorafs_manifest::{
    AdmissionRecord,
    pdp::{
        PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1, PDP_GOVERNANCE_ARCHIVE_VERSION_V1,
        PDP_MAX_TOTAL_HOT_LEAF_SAMPLES_V1, PDP_PROOF_MAX_CANONICAL_BYTES_V1,
        PDP_PROOF_SIGNATURE_DOMAIN_V1, PDP_PROOF_VERSION_V1, PdpChallengeV1, PdpCommitmentV1,
        PdpEd25519SignatureV1, PdpProofLeafV1, PdpProofV1, PdpSignatureVerificationError,
        verify_pdp_bundle_v1,
    },
    repair::{
        REPAIR_EVIDENCE_VERSION_V1, REPAIR_REPORT_VERSION_V1, RepairCauseV1, RepairEvidenceV1,
        RepairPdpFailureCauseV1, RepairPdpFailureKindV1, RepairReportV1, RepairTicketId,
    },
};
use thiserror::Error;

pub use sorafs_manifest::pdp::{
    PdpGovernanceArchiveV1, PdpRejectionReasonV1, PdpTerminalDecisionV1,
};

/// PDP provider protocol policy schema version.
pub const PDP_PROVIDER_POLICY_VERSION_V1: u8 = 1;
/// PDP provider durable checkpoint schema version.
pub const PDP_PROVIDER_CHECKPOINT_VERSION_V1: u8 = 1;
/// PDP next-challenge response schema version.
pub const PDP_NEXT_CHALLENGE_VERSION_V1: u8 = 1;
/// Default checkpoint file name below the configured SoraFS storage root.
pub const PDP_PROVIDER_CHECKPOINT_FILE_NAME_V1: &str = "pdp-provider-state.to";
/// Maximum records returned by one bounded PDP status export.
pub const PDP_STATUS_EXPORT_MAX_RECORDS_V1: usize = 1_000;

const HANDOFF_IDEMPOTENCY_DOMAIN_V1: &[u8] = b"sorafs.pdp.terminal-handoff.v1\0";
const DEFAULT_MAX_PENDING: u32 = 4_096;
const DEFAULT_MAX_TERMINAL: u32 = 65_536;
const DEFAULT_CHECKPOINT_MAX_BYTES: u64 = 128 * 1024 * 1024;
const DEFAULT_MIN_RESPONSE_WINDOW_SECS: u64 = 4 * 60;
const DEFAULT_MAX_RESPONSE_WINDOW_SECS: u64 = 10 * 60;
const DEFAULT_MAX_FUTURE_SKEW_SECS: u64 = 5;
const DEFAULT_TERMINAL_RETENTION_SECS: u64 = 24 * 60 * 60;
const CHECKPOINT_LOCK_FILE_NAME: &str = "pdp-provider-state.lock";
static CHECKPOINT_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
static CHECKPOINT_PROCESS_LOCK: Mutex<()> = Mutex::new(());

#[cfg(unix)]
const LOCK_EXCLUSIVE_NONBLOCKING: std::os::raw::c_int = 2 | 4;
#[cfg(any(target_os = "linux", target_os = "android"))]
const SAFE_OPEN_FLAGS: std::os::raw::c_int = 0x0002_0000 | 0x0008_0000;
#[cfg(any(target_os = "macos", target_os = "ios"))]
const SAFE_OPEN_FLAGS: std::os::raw::c_int = 0x0000_0100 | 0x0100_0000;
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    ))
))]
const SAFE_OPEN_FLAGS: std::os::raw::c_int = 0;

#[cfg(unix)]
unsafe extern "C" {
    fn flock(fd: std::os::raw::c_int, operation: std::os::raw::c_int) -> std::os::raw::c_int;
}

/// Governance-controlled resource and timing bounds for the embedded PDP runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PdpProviderProtocolPolicyV1 {
    /// Schema version; must equal [`PDP_PROVIDER_POLICY_VERSION_V1`].
    pub version: u8,
    /// Maximum pending plus terminal-handoff records.
    pub max_pending_records: u32,
    /// Maximum compact terminal replay records.
    pub max_terminal_records: u32,
    /// Maximum canonical checkpoint byte length.
    pub checkpoint_max_bytes: u64,
    /// Maximum canonical challenge byte length accepted below the protocol cap.
    pub challenge_max_bytes: u32,
    /// Maximum canonical proof byte length accepted below the protocol cap.
    pub proof_max_bytes: u32,
    /// Minimum governed challenge response window.
    pub min_response_window_secs: u64,
    /// Maximum governed challenge response window.
    pub max_response_window_secs: u64,
    /// Maximum provider-issued timestamp skew ahead of server time.
    pub max_future_skew_secs: u64,
    /// Minimum age before compact terminal replay records may be pruned.
    pub terminal_retention_secs: u64,
}

impl Default for PdpProviderProtocolPolicyV1 {
    fn default() -> Self {
        Self {
            version: PDP_PROVIDER_POLICY_VERSION_V1,
            max_pending_records: DEFAULT_MAX_PENDING,
            max_terminal_records: DEFAULT_MAX_TERMINAL,
            checkpoint_max_bytes: DEFAULT_CHECKPOINT_MAX_BYTES,
            challenge_max_bytes: PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1 as u32,
            proof_max_bytes: PDP_PROOF_MAX_CANONICAL_BYTES_V1 as u32,
            min_response_window_secs: DEFAULT_MIN_RESPONSE_WINDOW_SECS,
            max_response_window_secs: DEFAULT_MAX_RESPONSE_WINDOW_SECS,
            max_future_skew_secs: DEFAULT_MAX_FUTURE_SKEW_SECS,
            terminal_retention_secs: DEFAULT_TERMINAL_RETENTION_SECS,
        }
    }
}

impl PdpProviderProtocolPolicyV1 {
    /// Validate all bounded first-release policy invariants.
    pub fn validate(&self) -> Result<(), PdpProviderProtocolError> {
        if self.version != PDP_PROVIDER_POLICY_VERSION_V1 {
            return Err(PdpProviderProtocolError::InvalidPolicy(format!(
                "unsupported PDP provider policy version {}",
                self.version
            )));
        }
        if self.max_pending_records == 0 || self.max_terminal_records == 0 {
            return Err(PdpProviderProtocolError::InvalidPolicy(
                "PDP pending and terminal record limits must be positive".to_owned(),
            ));
        }
        if self.checkpoint_max_bytes == 0
            || self.challenge_max_bytes == 0
            || self.proof_max_bytes == 0
            || self.challenge_max_bytes as usize > PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1
            || self.proof_max_bytes as usize > PDP_PROOF_MAX_CANONICAL_BYTES_V1
        {
            return Err(PdpProviderProtocolError::InvalidPolicy(
                "PDP checkpoint/challenge/proof byte limits are zero or exceed protocol caps"
                    .to_owned(),
            ));
        }
        let minimum_checkpoint_bytes = u64::from(self.challenge_max_bytes)
            .checked_add(u64::from(self.proof_max_bytes))
            .ok_or_else(|| {
                PdpProviderProtocolError::InvalidPolicy(
                    "PDP checkpoint payload-size arithmetic overflowed".to_owned(),
                )
            })?;
        if self.checkpoint_max_bytes < minimum_checkpoint_bytes {
            return Err(PdpProviderProtocolError::InvalidPolicy(
                "PDP checkpoint must fit one maximum challenge and proof".to_owned(),
            ));
        }
        if self.min_response_window_secs == 0
            || self.max_response_window_secs < self.min_response_window_secs
            || self.terminal_retention_secs < self.max_response_window_secs
        {
            return Err(PdpProviderProtocolError::InvalidPolicy(
                "PDP response and terminal-retention windows are inconsistent".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Compact terminal response retained after external handoffs complete.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PdpTerminalOutcomeV1 {
    /// Queue sequence.
    pub sequence: u64,
    /// Challenge identifier.
    pub challenge_id: [u8; 32],
    /// BLAKE3 digest of the exact canonical challenge payload retained for replay checks.
    pub challenge_payload_digest: [u8; 32],
    /// Manifest digest.
    pub manifest_digest: [u8; 32],
    /// Provider identifier.
    pub provider_id: [u8; 32],
    /// Challenge epoch.
    pub epoch_id: u64,
    /// Terminal decision.
    pub decision: PdpTerminalDecisionV1,
    /// Proof digest, when submitted.
    #[norito(default)]
    pub proof_digest: Option<[u8; 32]>,
    /// Server terminal timestamp.
    pub decided_at_unix: u64,
    /// Governance archive payload digest.
    pub archive_digest: [u8; 32],
    /// Receipt returned by the idempotent governance archive sink.
    pub archive_receipt_digest: [u8; 32],
    /// Receipt returned by the idempotent repair sink for rejected decisions.
    #[norito(default)]
    pub repair_receipt_digest: Option<[u8; 32]>,
}

/// Deterministic response returned by the provider's `next` operation.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PdpNextChallengeV1 {
    /// Schema version.
    pub version: u8,
    /// Queue sequence.
    pub sequence: u64,
    /// Canonical challenge.
    pub challenge: PdpChallengeV1,
    /// Server admission timestamp.
    pub enqueued_at_unix: u64,
}

/// Public lifecycle state for one retained PDP challenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub enum PdpChallengeLifecycleV1 {
    /// Awaiting an authenticated proof.
    Pending,
    /// Proof verdict is durable and an external handoff remains pending.
    HandoffPending,
    /// Governance archive and any repair handoff completed.
    Terminal,
}

/// Compact bounded status for one retained PDP challenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PdpChallengeStatusV1 {
    /// Provider queue sequence.
    pub sequence: u64,
    /// Challenge identity.
    pub challenge_id: [u8; 32],
    /// Manifest identity.
    pub manifest_digest: [u8; 32],
    /// Provider identity.
    pub provider_id: [u8; 32],
    /// Challenge epoch.
    pub epoch_id: u64,
    /// Provider response deadline while full challenge state is retained.
    #[norito(default)]
    pub response_deadline_unix: Option<u64>,
    /// Current durable lifecycle.
    pub lifecycle: PdpChallengeLifecycleV1,
    /// Terminal decision, once a verdict is durable.
    #[norito(default)]
    pub decision: Option<PdpTerminalDecisionV1>,
    /// Digest of a submitted canonical proof, when available.
    #[norito(default)]
    pub proof_digest: Option<[u8; 32]>,
}

/// Result of enqueueing a governed PDP challenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdpChallengeEnqueueOutcome {
    /// A new challenge was durably inserted.
    Inserted {
        /// Durable provider-queue sequence assigned to the challenge.
        sequence: u64,
    },
    /// The exact canonical challenge was already retained.
    Existing {
        /// Durable provider-queue sequence retained for the challenge.
        sequence: u64,
    },
}

/// Payload-free telemetry snapshot for the provider protocol.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PdpProviderTelemetrySnapshot {
    /// Newly enqueued challenges.
    pub challenges_enqueued: u64,
    /// Exact duplicate challenge or proof replays.
    pub duplicates: u64,
    /// Accepted proofs.
    pub proofs_accepted: u64,
    /// Rejected or expired challenges.
    pub proofs_rejected: u64,
    /// Deadline-expired challenges.
    pub expired: u64,
    /// Governance archive callback failures.
    pub archive_failures: u64,
    /// Repair callback failures.
    pub repair_failures: u64,
    /// Durable checkpoint failures.
    pub checkpoint_failures: u64,
}

#[derive(Debug, Default)]
struct PdpProviderTelemetry {
    challenges_enqueued: AtomicU64,
    duplicates: AtomicU64,
    proofs_accepted: AtomicU64,
    proofs_rejected: AtomicU64,
    expired: AtomicU64,
    archive_failures: AtomicU64,
    repair_failures: AtomicU64,
    checkpoint_failures: AtomicU64,
}

impl PdpProviderTelemetry {
    fn snapshot(&self) -> PdpProviderTelemetrySnapshot {
        PdpProviderTelemetrySnapshot {
            challenges_enqueued: self.challenges_enqueued.load(Ordering::Relaxed),
            duplicates: self.duplicates.load(Ordering::Relaxed),
            proofs_accepted: self.proofs_accepted.load(Ordering::Relaxed),
            proofs_rejected: self.proofs_rejected.load(Ordering::Relaxed),
            expired: self.expired.load(Ordering::Relaxed),
            archive_failures: self.archive_failures.load(Ordering::Relaxed),
            repair_failures: self.repair_failures.load(Ordering::Relaxed),
            checkpoint_failures: self.checkpoint_failures.load(Ordering::Relaxed),
        }
    }
}

/// Error returned by an external governance or repair handoff sink.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{0}")]
pub struct PdpExternalHandoffError(pub String);

/// Idempotent external effects required before a PDP result becomes terminal.
pub trait PdpTerminalHandoff: Send + Sync + std::fmt::Debug {
    /// Archive the canonical verdict/proof payload and return a non-zero receipt digest.
    ///
    /// Implementations must treat `idempotency_key` as exactly-once identity because a
    /// successful callback can be retried after a local post-callback checkpoint failure.
    fn archive(
        &self,
        idempotency_key: [u8; 32],
        payload: &PdpGovernanceArchiveV1,
    ) -> Result<[u8; 32], PdpExternalHandoffError>;

    /// Enqueue the canonical `pdp_failure` repair report and return a non-zero receipt digest.
    fn repair(
        &self,
        idempotency_key: [u8; 32],
        report: &RepairReportV1,
    ) -> Result<[u8; 32], PdpExternalHandoffError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ChallengeScope {
    provider_id: [u8; 32],
    manifest_digest: [u8; 32],
    epoch_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct PendingChallengeV1 {
    sequence: u64,
    commitment: PdpCommitmentV1,
    challenge: PdpChallengeV1,
    challenge_payload_digest: [u8; 32],
    admission_envelope_digest: [u8; 32],
    admitted_provider_key: [u8; 32],
    admitted_retention_epoch: u64,
    enqueued_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct HandoffPendingV1 {
    pending: PendingChallengeV1,
    archive: PdpGovernanceArchiveV1,
    #[norito(default)]
    repair_report: Option<RepairReportV1>,
    #[norito(default)]
    archive_receipt_digest: Option<[u8; 32]>,
    #[norito(default)]
    repair_receipt_digest: Option<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[norito(tag = "state", content = "record")]
#[expect(
    clippy::large_enum_variant,
    reason = "the persisted V1 checkpoint enum has a fixed Norito layout; boxing a variant would risk changing it"
)]
enum StoredChallengeV1 {
    Pending(PendingChallengeV1),
    HandoffPending(HandoffPendingV1),
    Terminal(PdpTerminalOutcomeV1),
}

impl StoredChallengeV1 {
    fn sequence(&self) -> u64 {
        match self {
            Self::Pending(record) => record.sequence,
            Self::HandoffPending(record) => record.pending.sequence,
            Self::Terminal(record) => record.sequence,
        }
    }

    fn challenge_id(&self) -> [u8; 32] {
        match self {
            Self::Pending(record) => record.challenge.challenge_id,
            Self::HandoffPending(record) => record.pending.challenge.challenge_id,
            Self::Terminal(record) => record.challenge_id,
        }
    }

    fn scope(&self) -> ChallengeScope {
        match self {
            Self::Pending(record) => challenge_scope(&record.challenge),
            Self::HandoffPending(record) => challenge_scope(&record.pending.challenge),
            Self::Terminal(record) => ChallengeScope {
                provider_id: record.provider_id,
                manifest_digest: record.manifest_digest,
                epoch_id: record.epoch_id,
            },
        }
    }

    fn challenge_payload_digest(&self) -> [u8; 32] {
        match self {
            Self::Pending(record) => record.challenge_payload_digest,
            Self::HandoffPending(record) => record.pending.challenge_payload_digest,
            Self::Terminal(record) => record.challenge_payload_digest,
        }
    }
}

fn stored_challenge_status(record: &StoredChallengeV1) -> PdpChallengeStatusV1 {
    match record {
        StoredChallengeV1::Pending(pending) => PdpChallengeStatusV1 {
            sequence: pending.sequence,
            challenge_id: pending.challenge.challenge_id,
            manifest_digest: pending.challenge.manifest_digest,
            provider_id: pending.challenge.provider_id,
            epoch_id: pending.challenge.epoch_id,
            response_deadline_unix: Some(pending.challenge.response_deadline_unix),
            lifecycle: PdpChallengeLifecycleV1::Pending,
            decision: None,
            proof_digest: None,
        },
        StoredChallengeV1::HandoffPending(handoff) => PdpChallengeStatusV1 {
            sequence: handoff.pending.sequence,
            challenge_id: handoff.pending.challenge.challenge_id,
            manifest_digest: handoff.pending.challenge.manifest_digest,
            provider_id: handoff.pending.challenge.provider_id,
            epoch_id: handoff.pending.challenge.epoch_id,
            response_deadline_unix: Some(handoff.pending.challenge.response_deadline_unix),
            lifecycle: PdpChallengeLifecycleV1::HandoffPending,
            decision: Some(handoff.archive.decision),
            proof_digest: handoff.archive.proof_digest,
        },
        StoredChallengeV1::Terminal(terminal) => PdpChallengeStatusV1 {
            sequence: terminal.sequence,
            challenge_id: terminal.challenge_id,
            manifest_digest: terminal.manifest_digest,
            provider_id: terminal.provider_id,
            epoch_id: terminal.epoch_id,
            response_deadline_unix: None,
            lifecycle: PdpChallengeLifecycleV1::Terminal,
            decision: Some(terminal.decision),
            proof_digest: terminal.proof_digest,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct PdpProviderCheckpointV1 {
    version: u8,
    next_sequence: u64,
    records: Vec<StoredChallengeV1>,
}

#[derive(Debug, Clone)]
struct RuntimeState {
    next_sequence: u64,
    records: BTreeMap<[u8; 32], StoredChallengeV1>,
    scopes: BTreeMap<ChallengeScope, [u8; 32]>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            next_sequence: 1,
            records: BTreeMap::new(),
            scopes: BTreeMap::new(),
        }
    }
}

impl RuntimeState {
    fn checkpoint(&self) -> PdpProviderCheckpointV1 {
        let mut records = self.records.values().cloned().collect::<Vec<_>>();
        records.sort_by_key(StoredChallengeV1::sequence);
        PdpProviderCheckpointV1 {
            version: PDP_PROVIDER_CHECKPOINT_VERSION_V1,
            next_sequence: self.next_sequence,
            records,
        }
    }

    fn from_checkpoint(
        checkpoint: PdpProviderCheckpointV1,
        policy: PdpProviderProtocolPolicyV1,
    ) -> Result<Self, PdpProviderProtocolError> {
        validate_checkpoint(&checkpoint, policy)?;
        let mut records = BTreeMap::new();
        let mut scopes = BTreeMap::new();
        for record in checkpoint.records {
            let challenge_id = record.challenge_id();
            let scope = record.scope();
            if records.insert(challenge_id, record).is_some()
                || scopes.insert(scope, challenge_id).is_some()
            {
                return Err(PdpProviderProtocolError::InvalidCheckpoint(
                    "duplicate PDP challenge or provider/manifest/epoch scope".to_owned(),
                ));
            }
        }
        Ok(Self {
            next_sequence: checkpoint.next_sequence,
            records,
            scopes,
        })
    }

    fn pending_count(&self) -> usize {
        self.records
            .values()
            .filter(|record| !matches!(record, StoredChallengeV1::Terminal(_)))
            .count()
    }

    fn terminal_count(&self) -> usize {
        self.records
            .values()
            .filter(|record| matches!(record, StoredChallengeV1::Terminal(_)))
            .count()
    }
}

#[derive(Debug)]
struct DurableState {
    runtime: RuntimeState,
    fingerprint: Option<[u8; 32]>,
    durability_failure: Option<String>,
}

/// Durable, deterministic PDP challenge queue and proof-submission service.
#[derive(Debug, Clone)]
pub struct PdpProviderProtocol {
    policy: PdpProviderProtocolPolicyV1,
    state: Arc<Mutex<DurableState>>,
    checkpoint_store: Option<Arc<PdpCheckpointStore>>,
    telemetry: Arc<PdpProviderTelemetry>,
}

impl PdpProviderProtocol {
    /// Construct a non-persistent runtime, intended for focused composition tests.
    pub fn in_memory(
        policy: PdpProviderProtocolPolicyV1,
    ) -> Result<Self, PdpProviderProtocolError> {
        policy.validate()?;
        Ok(Self {
            policy,
            state: Arc::new(Mutex::new(DurableState {
                runtime: RuntimeState::default(),
                fingerprint: None,
                durability_failure: None,
            })),
            checkpoint_store: None,
            telemetry: Arc::new(PdpProviderTelemetry::default()),
        })
    }

    /// Open or create a durable runtime below the configured storage directory.
    pub fn open(
        policy: PdpProviderProtocolPolicyV1,
        state_dir: &Path,
    ) -> Result<Self, PdpProviderProtocolError> {
        policy.validate()?;
        let store = Arc::new(PdpCheckpointStore::new(state_dir, policy)?);
        let (checkpoint, fingerprint) = store.load()?;
        let runtime = checkpoint.map_or_else(
            || Ok(RuntimeState::default()),
            |checkpoint| RuntimeState::from_checkpoint(checkpoint, policy),
        )?;
        Ok(Self {
            policy,
            state: Arc::new(Mutex::new(DurableState {
                runtime,
                fingerprint,
                durability_failure: None,
            })),
            checkpoint_store: Some(store),
            telemetry: Arc::new(PdpProviderTelemetry::default()),
        })
    }

    /// Return current payload-free telemetry counters.
    #[must_use]
    pub fn telemetry(&self) -> PdpProviderTelemetrySnapshot {
        self.telemetry.snapshot()
    }

    /// Enqueue one exact council-admitted challenge for the expected epoch.
    pub fn enqueue_challenge(
        &self,
        commitment: PdpCommitmentV1,
        challenge: PdpChallengeV1,
        admission: &AdmissionRecord,
        expected_epoch_id: u64,
        now_unix: u64,
    ) -> Result<PdpChallengeEnqueueOutcome, PdpProviderProtocolError> {
        let prepared = validate_enqueue(
            self.policy,
            commitment,
            challenge,
            admission,
            expected_epoch_id,
            now_unix,
        )?;
        let challenge_id = prepared.challenge.challenge_id;
        let scope = challenge_scope(&prepared.challenge);
        let mut durable = self.lock_state()?;
        if let Some(existing) = durable.runtime.records.get(&challenge_id) {
            if existing.scope() == scope
                && existing.challenge_payload_digest() == prepared.challenge_payload_digest
            {
                self.telemetry.duplicates.fetch_add(1, Ordering::Relaxed);
                return Ok(PdpChallengeEnqueueOutcome::Existing {
                    sequence: existing.sequence(),
                });
            }
            return Err(PdpProviderProtocolError::ChallengeConflict);
        }
        if durable.runtime.scopes.contains_key(&scope) {
            return Err(PdpProviderProtocolError::ChallengeScopeReplay);
        }
        if durable.runtime.pending_count() >= self.policy.max_pending_records as usize {
            return Err(PdpProviderProtocolError::PendingRetentionExhausted {
                limit: self.policy.max_pending_records,
            });
        }
        let sequence = durable.runtime.next_sequence;
        let next_sequence = sequence
            .checked_add(1)
            .ok_or(PdpProviderProtocolError::SequenceExhausted)?;
        let record = PendingChallengeV1 {
            sequence,
            commitment: prepared.commitment,
            challenge: prepared.challenge,
            challenge_payload_digest: prepared.challenge_payload_digest,
            admission_envelope_digest: *admission.envelope_digest(),
            admitted_provider_key: *admission.advert_key(),
            admitted_retention_epoch: admission.envelope().retention_epoch,
            enqueued_at_unix: now_unix,
        };
        let mut candidate = durable.runtime.clone();
        candidate.next_sequence = next_sequence;
        candidate
            .records
            .insert(challenge_id, StoredChallengeV1::Pending(record));
        candidate.scopes.insert(scope, challenge_id);
        self.commit_candidate(&mut durable, candidate)?;
        self.telemetry
            .challenges_enqueued
            .fetch_add(1, Ordering::Relaxed);
        Ok(PdpChallengeEnqueueOutcome::Inserted { sequence })
    }

    /// Return the oldest non-expired challenge for one provider.
    pub fn next_challenge(
        &self,
        provider_id: [u8; 32],
        now_unix: u64,
    ) -> Result<Option<PdpNextChallengeV1>, PdpProviderProtocolError> {
        if provider_id == [0; 32] || now_unix == 0 {
            return Err(PdpProviderProtocolError::InvalidLookup);
        }
        let durable = self.lock_state()?;
        let pending = durable
            .runtime
            .records
            .values()
            .filter_map(|record| match record {
                StoredChallengeV1::Pending(pending)
                    if pending.challenge.provider_id == provider_id =>
                {
                    Some(pending)
                }
                _ => None,
            })
            .min_by_key(|pending| pending.sequence);
        let Some(pending) = pending else {
            return Ok(None);
        };
        if now_unix > pending.challenge.response_deadline_unix {
            return Err(PdpProviderProtocolError::ChallengeRequiresExpiry {
                challenge_id: pending.challenge.challenge_id,
            });
        }
        Ok(Some(PdpNextChallengeV1 {
            version: PDP_NEXT_CHALLENGE_VERSION_V1,
            sequence: pending.sequence,
            challenge: pending.challenge.clone(),
            enqueued_at_unix: pending.enqueued_at_unix,
        }))
    }

    /// Submit exact canonical proof bytes for one authenticated challenge identity.
    ///
    /// Malformed, cross-challenge, wrong-signer, and otherwise invalid proof
    /// submissions all become a durable `invalid_proof` verdict and authoritative
    /// repair handoff for the named challenge.
    pub fn submit_proof_for_challenge_bytes(
        &self,
        challenge_id: [u8; 32],
        proof_bytes: &[u8],
        active_admission: &AdmissionRecord,
        now_unix: u64,
        handoff: &dyn PdpTerminalHandoff,
    ) -> Result<PdpTerminalOutcomeV1, PdpProviderProtocolError> {
        if challenge_id == [0; 32] {
            return Err(PdpProviderProtocolError::InvalidLookup);
        }
        {
            let durable = self.lock_state()?;
            if let Some(StoredChallengeV1::Pending(pending)) =
                durable.runtime.records.get(&challenge_id)
            {
                validate_active_admission(pending, active_admission)?;
            }
        }
        let proof = match decode_canonical_proof(proof_bytes, self.policy.proof_max_bytes as usize)
        {
            Ok(proof) if proof.challenge_id == challenge_id => proof,
            Ok(_) | Err(_) => {
                return self.reject_without_proof(
                    challenge_id,
                    PdpRejectionReasonV1::InvalidProof,
                    now_unix,
                    handoff,
                );
            }
        };
        self.submit_proof(
            proof,
            proof_bytes.to_vec(),
            active_admission,
            now_unix,
            handoff,
        )
    }

    /// Mark a pending challenge rejected because its active admission was revoked.
    pub fn reject_revoked(
        &self,
        challenge_id: [u8; 32],
        now_unix: u64,
        handoff: &dyn PdpTerminalHandoff,
    ) -> Result<PdpTerminalOutcomeV1, PdpProviderProtocolError> {
        self.reject_without_proof(
            challenge_id,
            PdpRejectionReasonV1::AdmissionRevoked,
            now_unix,
            handoff,
        )
    }

    /// Mark a pending challenge rejected because retained proof material is unavailable.
    pub fn reject_storage_unavailable(
        &self,
        challenge_id: [u8; 32],
        now_unix: u64,
        handoff: &dyn PdpTerminalHandoff,
    ) -> Result<PdpTerminalOutcomeV1, PdpProviderProtocolError> {
        self.reject_without_proof(
            challenge_id,
            PdpRejectionReasonV1::StorageUnavailable,
            now_unix,
            handoff,
        )
    }

    /// Expire a pending challenge after its response deadline.
    pub fn expire_challenge(
        &self,
        challenge_id: [u8; 32],
        now_unix: u64,
        handoff: &dyn PdpTerminalHandoff,
    ) -> Result<PdpTerminalOutcomeV1, PdpProviderProtocolError> {
        let pending = self.pending_record(challenge_id)?;
        if now_unix <= pending.challenge.response_deadline_unix {
            return Err(PdpProviderProtocolError::ChallengeNotExpired);
        }
        let outcome = self.reject_without_proof(
            challenge_id,
            PdpRejectionReasonV1::DeadlineExpired,
            now_unix,
            handoff,
        )?;
        self.telemetry.expired.fetch_add(1, Ordering::Relaxed);
        Ok(outcome)
    }

    /// Retry every durable terminal handoff in queue sequence order.
    pub fn resume_handoffs(
        &self,
        handoff: &dyn PdpTerminalHandoff,
        limit: usize,
    ) -> Result<Vec<PdpTerminalOutcomeV1>, PdpProviderProtocolError> {
        let ids = {
            let durable = self.lock_state()?;
            let mut records = durable
                .runtime
                .records
                .values()
                .filter_map(|record| match record {
                    StoredChallengeV1::HandoffPending(record) => Some((
                        record.pending.sequence,
                        record.pending.challenge.challenge_id,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            records.sort_by_key(|(sequence, _)| *sequence);
            records
                .into_iter()
                .take(limit.max(1))
                .map(|(_, challenge_id)| challenge_id)
                .collect::<Vec<_>>()
        };
        let mut outcomes = Vec::with_capacity(ids.len());
        for challenge_id in ids {
            outcomes.push(self.drive_handoff(challenge_id, handoff)?);
        }
        Ok(outcomes)
    }

    /// Prune sufficiently old compact terminal records in deterministic order.
    pub fn prune_terminal(
        &self,
        now_unix: u64,
        limit: usize,
    ) -> Result<usize, PdpProviderProtocolError> {
        let cutoff = now_unix.saturating_sub(self.policy.terminal_retention_secs);
        let mut durable = self.lock_state()?;
        let mut candidates = durable
            .runtime
            .records
            .values()
            .filter_map(|record| match record {
                StoredChallengeV1::Terminal(terminal) if terminal.decided_at_unix <= cutoff => {
                    Some((terminal.sequence, terminal.challenge_id, terminal.scope()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(sequence, _, _)| *sequence);
        candidates.truncate(limit);
        if candidates.is_empty() {
            return Ok(0);
        }
        let mut candidate = durable.runtime.clone();
        for (_, challenge_id, scope) in &candidates {
            candidate.records.remove(challenge_id);
            candidate.scopes.remove(scope);
        }
        self.commit_candidate(&mut durable, candidate)?;
        Ok(candidates.len())
    }

    /// Return a retained terminal outcome, when present.
    pub fn terminal_outcome(
        &self,
        challenge_id: &[u8; 32],
    ) -> Result<Option<PdpTerminalOutcomeV1>, PdpProviderProtocolError> {
        let durable = self.lock_state()?;
        Ok(match durable.runtime.records.get(challenge_id) {
            Some(StoredChallengeV1::Terminal(outcome)) => Some(outcome.clone()),
            _ => None,
        })
    }

    /// Return compact status for one retained challenge identity.
    pub fn challenge_status(
        &self,
        challenge_id: &[u8; 32],
    ) -> Result<Option<PdpChallengeStatusV1>, PdpProviderProtocolError> {
        if challenge_id == &[0; 32] {
            return Err(PdpProviderProtocolError::InvalidLookup);
        }
        let durable = self.lock_state()?;
        Ok(durable
            .runtime
            .records
            .get(challenge_id)
            .map(stored_challenge_status))
    }

    /// Export a bounded sequence-ordered page of retained challenge statuses.
    pub fn export_statuses(
        &self,
        after_sequence: u64,
        limit: usize,
    ) -> Result<Vec<PdpChallengeStatusV1>, PdpProviderProtocolError> {
        if limit == 0 || limit > PDP_STATUS_EXPORT_MAX_RECORDS_V1 {
            return Err(PdpProviderProtocolError::InvalidExportLimit {
                limit,
                max: PDP_STATUS_EXPORT_MAX_RECORDS_V1,
            });
        }
        let durable = self.lock_state()?;
        let mut statuses = durable
            .runtime
            .records
            .values()
            .filter(|record| record.sequence() > after_sequence)
            .map(stored_challenge_status)
            .collect::<Vec<_>>();
        statuses.sort_by_key(|status| status.sequence);
        statuses.truncate(limit);
        Ok(statuses)
    }

    fn submit_proof(
        &self,
        proof: PdpProofV1,
        canonical_proof: Vec<u8>,
        active_admission: &AdmissionRecord,
        now_unix: u64,
        handoff: &dyn PdpTerminalHandoff,
    ) -> Result<PdpTerminalOutcomeV1, PdpProviderProtocolError> {
        let proof_digest = proof.proof_digest().map_err(|error| {
            PdpProviderProtocolError::CanonicalEncoding(format!("digest PDP proof: {error}"))
        })?;
        {
            let durable = self.lock_state()?;
            match durable.runtime.records.get(&proof.challenge_id) {
                Some(StoredChallengeV1::Terminal(outcome)) => {
                    if outcome.proof_digest == Some(proof_digest) {
                        self.telemetry.duplicates.fetch_add(1, Ordering::Relaxed);
                        return Ok(outcome.clone());
                    }
                    return Err(PdpProviderProtocolError::TerminalReplayConflict);
                }
                Some(StoredChallengeV1::HandoffPending(record)) => {
                    if record.archive.proof_digest == Some(proof_digest) {
                        drop(durable);
                        self.telemetry.duplicates.fetch_add(1, Ordering::Relaxed);
                        return self.drive_handoff(proof.challenge_id, handoff);
                    }
                    return Err(PdpProviderProtocolError::TerminalReplayConflict);
                }
                Some(StoredChallengeV1::Pending(_)) => {}
                None => return Err(PdpProviderProtocolError::UnknownChallenge),
            }
        }

        let pending = self.pending_record(proof.challenge_id)?;
        validate_active_admission(&pending, active_admission)?;
        let signature_authorized = proof.signature.public_key == pending.admitted_provider_key
            && proof.verify_signature().is_ok();

        let future_limit = now_unix
            .checked_add(self.policy.max_future_skew_secs)
            .ok_or(PdpProviderProtocolError::TimestampOverflow)?;
        let (decision, verified_counts) = if now_unix > pending.challenge.response_deadline_unix {
            (
                PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::SubmissionLate),
                None,
            )
        } else if proof.issued_at_unix > future_limit {
            (
                PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::FutureTimestamp),
                None,
            )
        } else if !signature_authorized {
            (
                PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::InvalidProof),
                None,
            )
        } else {
            match verify_pdp_bundle_v1(
                &pending.commitment,
                &pending.challenge,
                &proof,
                active_admission,
            ) {
                Ok(verified) => (
                    PdpTerminalDecisionV1::Accepted,
                    Some((
                        verified.sampled_segments(),
                        verified.sampled_hot_leaves(),
                        verified.sampled_bytes(),
                    )),
                ),
                Err(_) => (
                    PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::InvalidProof),
                    None,
                ),
            }
        };
        self.prepare_handoff(
            pending,
            decision,
            Some(proof_digest),
            Some(canonical_proof),
            verified_counts,
            now_unix,
        )?;
        self.drive_handoff(proof.challenge_id, handoff)
    }

    fn reject_without_proof(
        &self,
        challenge_id: [u8; 32],
        reason: PdpRejectionReasonV1,
        now_unix: u64,
        handoff: &dyn PdpTerminalHandoff,
    ) -> Result<PdpTerminalOutcomeV1, PdpProviderProtocolError> {
        {
            let durable = self.lock_state()?;
            match durable.runtime.records.get(&challenge_id) {
                Some(StoredChallengeV1::Terminal(outcome)) => {
                    if outcome.decision == PdpTerminalDecisionV1::Rejected(reason) {
                        self.telemetry.duplicates.fetch_add(1, Ordering::Relaxed);
                        return Ok(outcome.clone());
                    }
                    return Err(PdpProviderProtocolError::TerminalReplayConflict);
                }
                Some(StoredChallengeV1::HandoffPending(record)) => {
                    if record.archive.decision == PdpTerminalDecisionV1::Rejected(reason) {
                        drop(durable);
                        return self.drive_handoff(challenge_id, handoff);
                    }
                    return Err(PdpProviderProtocolError::TerminalReplayConflict);
                }
                Some(StoredChallengeV1::Pending(_)) => {}
                None => return Err(PdpProviderProtocolError::UnknownChallenge),
            }
        }
        let pending = self.pending_record(challenge_id)?;
        self.prepare_handoff(
            pending,
            PdpTerminalDecisionV1::Rejected(reason),
            None,
            None,
            None,
            now_unix,
        )?;
        self.drive_handoff(challenge_id, handoff)
    }

    fn prepare_handoff(
        &self,
        pending: PendingChallengeV1,
        decision: PdpTerminalDecisionV1,
        proof_digest: Option<[u8; 32]>,
        canonical_proof: Option<Vec<u8>>,
        verified_counts: Option<(u16, u16, u64)>,
        now_unix: u64,
    ) -> Result<(), PdpProviderProtocolError> {
        if now_unix == 0 || now_unix < pending.challenge.issued_at_unix {
            return Err(PdpProviderProtocolError::InvalidTerminalTimestamp);
        }
        let mut durable = self.lock_state()?;
        if durable.runtime.terminal_count() >= self.policy.max_terminal_records as usize {
            return Err(PdpProviderProtocolError::TerminalRetentionExhausted {
                limit: self.policy.max_terminal_records,
            });
        }
        if !matches!(
            durable.runtime.records.get(&pending.challenge.challenge_id),
            Some(StoredChallengeV1::Pending(current)) if current == &pending
        ) {
            return Err(PdpProviderProtocolError::ConcurrentTransition);
        }
        let canonical_challenge = norito::to_bytes(&pending.challenge).map_err(|error| {
            PdpProviderProtocolError::CanonicalEncoding(format!("encode PDP challenge: {error}"))
        })?;
        let (sampled_segments, sampled_hot_leaves, sampled_bytes) = verified_counts.unwrap_or((
            u16::try_from(pending.challenge.samples.len()).unwrap_or(u16::MAX),
            challenge_hot_leaf_count(&pending.challenge)?,
            0,
        ));
        let archive = PdpGovernanceArchiveV1 {
            version: PDP_GOVERNANCE_ARCHIVE_VERSION_V1,
            sequence: pending.sequence,
            challenge_id: pending.challenge.challenge_id,
            commitment_digest: pending.challenge.commitment_digest,
            manifest_digest: pending.challenge.manifest_digest,
            provider_id: pending.challenge.provider_id,
            epoch_id: pending.challenge.epoch_id,
            decision,
            proof_digest,
            sampled_segments,
            sampled_hot_leaves,
            sampled_bytes,
            issued_at_unix: pending.challenge.issued_at_unix,
            response_deadline_unix: pending.challenge.response_deadline_unix,
            decided_at_unix: now_unix,
            admission_envelope_digest: pending.admission_envelope_digest,
            canonical_challenge,
            canonical_proof,
        };
        validate_archive(&archive, self.policy)?;
        let repair_report = match decision {
            PdpTerminalDecisionV1::Accepted => None,
            PdpTerminalDecisionV1::Rejected(reason) => Some(build_repair_report(&archive, reason)?),
        };
        let mut candidate = durable.runtime.clone();
        candidate.records.insert(
            pending.challenge.challenge_id,
            StoredChallengeV1::HandoffPending(HandoffPendingV1 {
                pending,
                archive,
                repair_report,
                archive_receipt_digest: None,
                repair_receipt_digest: None,
            }),
        );
        self.commit_candidate(&mut durable, candidate)
    }

    fn drive_handoff(
        &self,
        challenge_id: [u8; 32],
        handoff: &dyn PdpTerminalHandoff,
    ) -> Result<PdpTerminalOutcomeV1, PdpProviderProtocolError> {
        let mut durable = self.lock_state()?;
        let mut record = match durable.runtime.records.get(&challenge_id) {
            Some(StoredChallengeV1::HandoffPending(record)) => record.clone(),
            Some(StoredChallengeV1::Terminal(outcome)) => return Ok(outcome.clone()),
            Some(StoredChallengeV1::Pending(_)) => {
                return Err(PdpProviderProtocolError::HandoffNotPrepared);
            }
            None => return Err(PdpProviderProtocolError::UnknownChallenge),
        };
        let idempotency_key = handoff_idempotency_key(&record.archive)?;
        if record.archive_receipt_digest.is_none() {
            let receipt = handoff
                .archive(idempotency_key, &record.archive)
                .map_err(|error| {
                    self.telemetry
                        .archive_failures
                        .fetch_add(1, Ordering::Relaxed);
                    PdpProviderProtocolError::ArchiveHandoff(error.0)
                })?;
            if receipt == [0; 32] {
                return Err(PdpProviderProtocolError::ArchiveHandoff(
                    "governance archive returned a zero receipt digest".to_owned(),
                ));
            }
            record.archive_receipt_digest = Some(receipt);
            let mut candidate = durable.runtime.clone();
            candidate.records.insert(
                challenge_id,
                StoredChallengeV1::HandoffPending(record.clone()),
            );
            self.commit_candidate(&mut durable, candidate)?;
        }
        if let Some(report) = record.repair_report.as_ref()
            && record.repair_receipt_digest.is_none()
        {
            let receipt = handoff.repair(idempotency_key, report).map_err(|error| {
                self.telemetry
                    .repair_failures
                    .fetch_add(1, Ordering::Relaxed);
                PdpProviderProtocolError::RepairHandoff(error.0)
            })?;
            if receipt == [0; 32] {
                return Err(PdpProviderProtocolError::RepairHandoff(
                    "repair handoff returned a zero receipt digest".to_owned(),
                ));
            }
            record.repair_receipt_digest = Some(receipt);
            let mut candidate = durable.runtime.clone();
            candidate.records.insert(
                challenge_id,
                StoredChallengeV1::HandoffPending(record.clone()),
            );
            self.commit_candidate(&mut durable, candidate)?;
        }
        let archive_receipt_digest = record
            .archive_receipt_digest
            .ok_or(PdpProviderProtocolError::HandoffNotPrepared)?;
        if record.repair_report.is_some() != record.repair_receipt_digest.is_some() {
            return Err(PdpProviderProtocolError::HandoffNotPrepared);
        }
        let outcome = PdpTerminalOutcomeV1 {
            sequence: record.pending.sequence,
            challenge_id,
            challenge_payload_digest: record.pending.challenge_payload_digest,
            manifest_digest: record.pending.challenge.manifest_digest,
            provider_id: record.pending.challenge.provider_id,
            epoch_id: record.pending.challenge.epoch_id,
            decision: record.archive.decision,
            proof_digest: record.archive.proof_digest,
            decided_at_unix: record.archive.decided_at_unix,
            archive_digest: governance_archive_digest(&record.archive)?,
            archive_receipt_digest,
            repair_receipt_digest: record.repair_receipt_digest,
        };
        validate_terminal(&outcome)?;
        let mut candidate = durable.runtime.clone();
        candidate
            .records
            .insert(challenge_id, StoredChallengeV1::Terminal(outcome.clone()));
        self.commit_candidate(&mut durable, candidate)?;
        match outcome.decision {
            PdpTerminalDecisionV1::Accepted => {
                self.telemetry
                    .proofs_accepted
                    .fetch_add(1, Ordering::Relaxed);
            }
            PdpTerminalDecisionV1::Rejected(_) => {
                self.telemetry
                    .proofs_rejected
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
        Ok(outcome)
    }

    fn pending_record(
        &self,
        challenge_id: [u8; 32],
    ) -> Result<PendingChallengeV1, PdpProviderProtocolError> {
        let durable = self.lock_state()?;
        match durable.runtime.records.get(&challenge_id) {
            Some(StoredChallengeV1::Pending(pending)) => Ok(pending.clone()),
            Some(_) => Err(PdpProviderProtocolError::ConcurrentTransition),
            None => Err(PdpProviderProtocolError::UnknownChallenge),
        }
    }

    fn lock_state(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, DurableState>, PdpProviderProtocolError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| PdpProviderProtocolError::RuntimePoisoned)?;
        if let Some(reason) = guard.durability_failure.as_ref() {
            return Err(PdpProviderProtocolError::DurabilityPoisoned(reason.clone()));
        }
        Ok(guard)
    }

    fn commit_candidate(
        &self,
        durable: &mut DurableState,
        candidate: RuntimeState,
    ) -> Result<(), PdpProviderProtocolError> {
        validate_checkpoint(&candidate.checkpoint(), self.policy)?;
        if let Some(store) = self.checkpoint_store.as_ref() {
            match store.commit(&candidate.checkpoint(), durable.fingerprint) {
                Ok(fingerprint) => durable.fingerprint = Some(fingerprint),
                Err(error) => {
                    self.telemetry
                        .checkpoint_failures
                        .fetch_add(1, Ordering::Relaxed);
                    if matches!(
                        &error,
                        PdpProviderProtocolError::CheckpointDurabilityUncertain(_)
                    ) {
                        durable.durability_failure = Some(error.to_string());
                    }
                    return Err(error);
                }
            }
        }
        durable.runtime = candidate;
        Ok(())
    }
}

impl PdpTerminalOutcomeV1 {
    fn scope(&self) -> ChallengeScope {
        ChallengeScope {
            provider_id: self.provider_id,
            manifest_digest: self.manifest_digest,
            epoch_id: self.epoch_id,
        }
    }
}

#[derive(Debug)]
struct PdpCheckpointStore {
    root: PathBuf,
    checkpoint_path: PathBuf,
    lock_path: PathBuf,
    policy: PdpProviderProtocolPolicyV1,
}

impl PdpCheckpointStore {
    fn new(
        root: &Path,
        policy: PdpProviderProtocolPolicyV1,
    ) -> Result<Self, PdpProviderProtocolError> {
        ensure_private_state_directory(root)?;
        Ok(Self {
            root: root.to_path_buf(),
            checkpoint_path: root.join(PDP_PROVIDER_CHECKPOINT_FILE_NAME_V1),
            lock_path: root.join(CHECKPOINT_LOCK_FILE_NAME),
            policy,
        })
    }

    fn load(
        &self,
    ) -> Result<(Option<PdpProviderCheckpointV1>, Option<[u8; 32]>), PdpProviderProtocolError> {
        let _writer = CheckpointWriterGuard::acquire(&self.lock_path)?;
        let Some(bytes) =
            read_checkpoint_bytes(&self.checkpoint_path, self.policy.checkpoint_max_bytes)?
        else {
            return Ok((None, None));
        };
        let fingerprint = *blake3::hash(&bytes).as_bytes();
        let checkpoint: PdpProviderCheckpointV1 =
            norito::decode_from_bytes_with_limits(&bytes, checkpoint_decode_limits(self.policy))
                .map_err(|error| {
                    PdpProviderProtocolError::InvalidCheckpoint(format!(
                        "decode canonical PDP checkpoint: {error}"
                    ))
                })?;
        let canonical = norito::to_bytes(&checkpoint).map_err(|error| {
            PdpProviderProtocolError::CanonicalEncoding(format!(
                "re-encode PDP checkpoint: {error}"
            ))
        })?;
        if canonical != bytes {
            return Err(PdpProviderProtocolError::InvalidCheckpoint(
                "PDP checkpoint is not canonically encoded".to_owned(),
            ));
        }
        validate_checkpoint(&checkpoint, self.policy)?;
        Ok((Some(checkpoint), Some(fingerprint)))
    }

    fn commit(
        &self,
        checkpoint: &PdpProviderCheckpointV1,
        expected_fingerprint: Option<[u8; 32]>,
    ) -> Result<[u8; 32], PdpProviderProtocolError> {
        validate_checkpoint(checkpoint, self.policy)?;
        let bytes = norito::to_bytes(checkpoint).map_err(|error| {
            PdpProviderProtocolError::CanonicalEncoding(format!("encode PDP checkpoint: {error}"))
        })?;
        let observed_size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if observed_size > self.policy.checkpoint_max_bytes {
            return Err(PdpProviderProtocolError::PayloadTooLarge {
                kind: "checkpoint",
                size: bytes.len(),
                limit: usize::try_from(self.policy.checkpoint_max_bytes).unwrap_or(usize::MAX),
            });
        }

        let _writer = CheckpointWriterGuard::acquire(&self.lock_path)?;
        let current =
            read_checkpoint_bytes(&self.checkpoint_path, self.policy.checkpoint_max_bytes)?;
        let current_fingerprint = current
            .as_deref()
            .map(blake3::hash)
            .map(|digest| *digest.as_bytes());
        if current_fingerprint != expected_fingerprint {
            return Err(PdpProviderProtocolError::StaleCheckpoint);
        }

        let temp_path = self.root.join(format!(
            ".{PDP_PROVIDER_CHECKPOINT_FILE_NAME_V1}.{}.{}.tmp",
            std::process::id(),
            CHECKPOINT_TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let write_result = write_checkpoint_temp(&temp_path, &bytes).and_then(|()| {
            let latest =
                read_checkpoint_bytes(&self.checkpoint_path, self.policy.checkpoint_max_bytes)?;
            let latest_fingerprint = latest
                .as_deref()
                .map(blake3::hash)
                .map(|digest| *digest.as_bytes());
            if latest_fingerprint != expected_fingerprint {
                return Err(PdpProviderProtocolError::StaleCheckpoint);
            }
            fs::rename(&temp_path, &self.checkpoint_path).map_err(|error| {
                PdpProviderProtocolError::CheckpointIo(format!(
                    "rename PDP checkpoint into place: {error}"
                ))
            })?;
            sync_directory(&self.root).map_err(|error| {
                PdpProviderProtocolError::CheckpointDurabilityUncertain(error.to_string())
            })
        });
        if write_result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        write_result?;
        let persisted =
            read_checkpoint_bytes(&self.checkpoint_path, self.policy.checkpoint_max_bytes)
                .map_err(|error| {
                    PdpProviderProtocolError::CheckpointDurabilityUncertain(format!(
                        "could not verify PDP checkpoint after atomic rename: {error}"
                    ))
                })?
                .ok_or_else(|| {
                    PdpProviderProtocolError::CheckpointDurabilityUncertain(
                        "PDP checkpoint disappeared after atomic rename".to_owned(),
                    )
                })?;
        if persisted != bytes {
            return Err(PdpProviderProtocolError::CheckpointDurabilityUncertain(
                "PDP checkpoint bytes changed after atomic rename".to_owned(),
            ));
        }
        Ok(*blake3::hash(&bytes).as_bytes())
    }
}

struct CheckpointWriterGuard {
    _process_guard: std::sync::MutexGuard<'static, ()>,
    _file: File,
}

impl CheckpointWriterGuard {
    fn acquire(path: &Path) -> Result<Self, PdpProviderProtocolError> {
        let process_guard = CHECKPOINT_PROCESS_LOCK
            .try_lock()
            .map_err(|_| PdpProviderProtocolError::CheckpointBusy)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        {
            options.mode(0o600);
            options.custom_flags(SAFE_OPEN_FLAGS);
        }
        let file = options.open(path).map_err(|error| {
            PdpProviderProtocolError::CheckpointIo(format!(
                "open PDP checkpoint writer lock: {error}"
            ))
        })?;
        validate_open_regular_file(path, &file, 0, true)?;
        #[cfg(unix)]
        {
            // SAFETY: `flock` only borrows the live lock-file descriptor and does not
            // take ownership. The descriptor remains open in this guard.
            let result = unsafe { flock(file.as_raw_fd(), LOCK_EXCLUSIVE_NONBLOCKING) };
            if result != 0 {
                return Err(PdpProviderProtocolError::CheckpointBusy);
            }
        }
        Ok(Self {
            _process_guard: process_guard,
            _file: file,
        })
    }
}

fn ensure_private_state_directory(path: &Path) -> Result<(), PdpProviderProtocolError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(PdpProviderProtocolError::CheckpointIo(format!(
                    "PDP state root {path:?} must be a real directory"
                )));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            builder.mode(0o700);
            builder.create(path).map_err(|error| {
                PdpProviderProtocolError::CheckpointIo(format!(
                    "create PDP state root {path:?}: {error}"
                ))
            })?;
        }
        Err(error) => {
            return Err(PdpProviderProtocolError::CheckpointIo(format!(
                "inspect PDP state root {path:?}: {error}"
            )));
        }
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        PdpProviderProtocolError::CheckpointIo(format!(
            "reinspect PDP state root {path:?}: {error}"
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(PdpProviderProtocolError::CheckpointIo(format!(
            "PDP state root {path:?} changed during initialization"
        )));
    }
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| {
        PdpProviderProtocolError::CheckpointIo(format!(
            "set private PDP state-root permissions: {error}"
        ))
    })?;
    Ok(())
}

fn read_checkpoint_bytes(
    path: &Path,
    max_bytes: u64,
) -> Result<Option<Vec<u8>>, PdpProviderProtocolError> {
    let path_metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(PdpProviderProtocolError::CheckpointIo(format!(
                "inspect PDP checkpoint: {error}"
            )));
        }
    };
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(PdpProviderProtocolError::CheckpointIo(
            "PDP checkpoint must be a non-symlink regular file".to_owned(),
        ));
    }
    #[cfg(unix)]
    if path_metadata.nlink() != 1 {
        return Err(PdpProviderProtocolError::CheckpointIo(
            "PDP checkpoint must not have hard links".to_owned(),
        ));
    }
    if path_metadata.len() > max_bytes {
        return Err(PdpProviderProtocolError::PayloadTooLarge {
            kind: "checkpoint",
            size: usize::try_from(path_metadata.len()).unwrap_or(usize::MAX),
            limit: usize::try_from(max_bytes).unwrap_or(usize::MAX),
        });
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(SAFE_OPEN_FLAGS);
    let mut file = options.open(path).map_err(|error| {
        PdpProviderProtocolError::CheckpointIo(format!("open PDP checkpoint: {error}"))
    })?;
    validate_open_regular_file(path, &file, max_bytes, false)?;
    let allocation = usize::try_from(path_metadata.len())
        .unwrap_or(usize::MAX)
        .min(usize::try_from(max_bytes).unwrap_or(usize::MAX));
    let mut bytes = Vec::with_capacity(allocation);
    Read::by_ref(&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| {
            PdpProviderProtocolError::CheckpointIo(format!("read PDP checkpoint: {error}"))
        })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(PdpProviderProtocolError::PayloadTooLarge {
            kind: "checkpoint",
            size: bytes.len(),
            limit: usize::try_from(max_bytes).unwrap_or(usize::MAX),
        });
    }
    let reopened = fs::symlink_metadata(path).map_err(|error| {
        PdpProviderProtocolError::CheckpointIo(format!(
            "reinspect PDP checkpoint after read: {error}"
        ))
    })?;
    #[cfg(unix)]
    if reopened.dev() != path_metadata.dev()
        || reopened.ino() != path_metadata.ino()
        || reopened.nlink() != 1
    {
        return Err(PdpProviderProtocolError::CheckpointIo(
            "PDP checkpoint changed during bounded read".to_owned(),
        ));
    }
    Ok(Some(bytes))
}

fn validate_open_regular_file(
    path: &Path,
    file: &File,
    max_bytes: u64,
    allow_empty_lock: bool,
) -> Result<(), PdpProviderProtocolError> {
    let metadata = file.metadata().map_err(|error| {
        PdpProviderProtocolError::CheckpointIo(format!(
            "inspect opened PDP state file {path:?}: {error}"
        ))
    })?;
    if !metadata.is_file() || (!allow_empty_lock && metadata.len() > max_bytes) {
        return Err(PdpProviderProtocolError::CheckpointIo(format!(
            "opened PDP state path {path:?} is unsafe or oversized"
        )));
    }
    #[cfg(unix)]
    {
        let path_metadata = fs::symlink_metadata(path).map_err(|error| {
            PdpProviderProtocolError::CheckpointIo(format!(
                "inspect PDP state path {path:?}: {error}"
            ))
        })?;
        if path_metadata.file_type().is_symlink()
            || path_metadata.dev() != metadata.dev()
            || path_metadata.ino() != metadata.ino()
            || metadata.nlink() != 1
        {
            return Err(PdpProviderProtocolError::CheckpointIo(format!(
                "PDP state path {path:?} changed or has hard links"
            )));
        }
    }
    Ok(())
}

fn write_checkpoint_temp(path: &Path, bytes: &[u8]) -> Result<(), PdpProviderProtocolError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
        options.custom_flags(SAFE_OPEN_FLAGS);
    }
    let mut file = options.open(path).map_err(|error| {
        PdpProviderProtocolError::CheckpointIo(format!(
            "create private PDP checkpoint temporary file: {error}"
        ))
    })?;
    validate_open_regular_file(path, &file, u64::MAX, true)?;
    file.write_all(bytes).map_err(|error| {
        PdpProviderProtocolError::CheckpointIo(format!(
            "write PDP checkpoint temporary file: {error}"
        ))
    })?;
    file.sync_all().map_err(|error| {
        PdpProviderProtocolError::CheckpointIo(format!(
            "sync PDP checkpoint temporary file: {error}"
        ))
    })?;
    #[cfg(unix)]
    if file
        .metadata()
        .map_err(|error| {
            PdpProviderProtocolError::CheckpointIo(format!(
                "reinspect PDP checkpoint temporary file: {error}"
            ))
        })?
        .nlink()
        != 1
    {
        return Err(PdpProviderProtocolError::CheckpointIo(
            "PDP checkpoint temporary file acquired a hard link".to_owned(),
        ));
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), PdpProviderProtocolError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            PdpProviderProtocolError::CheckpointIo(format!(
                "sync PDP checkpoint directory: {error}"
            ))
        })
}

struct PreparedEnqueue {
    commitment: PdpCommitmentV1,
    challenge: PdpChallengeV1,
    challenge_payload_digest: [u8; 32],
}

fn validate_enqueue(
    policy: PdpProviderProtocolPolicyV1,
    commitment: PdpCommitmentV1,
    challenge: PdpChallengeV1,
    admission: &AdmissionRecord,
    expected_epoch_id: u64,
    now_unix: u64,
) -> Result<PreparedEnqueue, PdpProviderProtocolError> {
    commitment
        .validate()
        .map_err(|error| PdpProviderProtocolError::InvalidCommitment(error.to_string()))?;
    challenge
        .validate()
        .map_err(|error| PdpProviderProtocolError::InvalidChallenge(error.to_string()))?;
    if !admission.is_council_verified() {
        return Err(PdpProviderProtocolError::UntrustedAdmission);
    }
    if admission.provider_id() != &challenge.provider_id {
        return Err(PdpProviderProtocolError::AdmissionProviderMismatch);
    }
    if admission.advert_key() == &[0; 32] {
        return Err(PdpProviderProtocolError::UnauthorizedProviderKey);
    }
    if challenge.epoch_id != expected_epoch_id || expected_epoch_id == 0 {
        return Err(PdpProviderProtocolError::EpochMismatch {
            expected: expected_epoch_id,
            actual: challenge.epoch_id,
        });
    }
    if now_unix == 0 || now_unix > challenge.response_deadline_unix {
        return Err(PdpProviderProtocolError::ChallengeExpiredAtEnqueue);
    }
    let future_limit = now_unix
        .checked_add(policy.max_future_skew_secs)
        .ok_or(PdpProviderProtocolError::TimestampOverflow)?;
    if challenge.issued_at_unix > future_limit {
        return Err(PdpProviderProtocolError::ChallengeFromFuture);
    }
    let response_window = challenge
        .response_deadline_unix
        .checked_sub(challenge.issued_at_unix)
        .ok_or(PdpProviderProtocolError::InvalidResponseWindow)?;
    if response_window < policy.min_response_window_secs
        || response_window > policy.max_response_window_secs
    {
        return Err(PdpProviderProtocolError::InvalidResponseWindow);
    }
    if challenge.issued_at_unix < admission.envelope().issued_at
        || challenge.response_deadline_unix > admission.envelope().retention_epoch
    {
        return Err(PdpProviderProtocolError::AdmissionInactive);
    }
    let commitment_digest = commitment.commitment_digest().map_err(|error| {
        PdpProviderProtocolError::CanonicalEncoding(format!("digest PDP commitment: {error}"))
    })?;
    if challenge.commitment_digest != commitment_digest
        || challenge.manifest_digest != commitment.manifest_digest
        || challenge.chunk_profile != commitment.chunk_profile
        || challenge.samples.len() > usize::from(commitment.sample_window)
        || commitment.sealed_at > challenge.issued_at_unix
    {
        return Err(PdpProviderProtocolError::ChallengeCommitmentMismatch);
    }
    let canonical = norito::to_bytes(&challenge).map_err(|error| {
        PdpProviderProtocolError::CanonicalEncoding(format!("encode PDP challenge: {error}"))
    })?;
    if canonical.len() > policy.challenge_max_bytes as usize {
        return Err(PdpProviderProtocolError::PayloadTooLarge {
            kind: "challenge",
            size: canonical.len(),
            limit: policy.challenge_max_bytes as usize,
        });
    }
    Ok(PreparedEnqueue {
        commitment,
        challenge,
        challenge_payload_digest: *blake3::hash(&canonical).as_bytes(),
    })
}

fn validate_active_admission(
    pending: &PendingChallengeV1,
    admission: &AdmissionRecord,
) -> Result<(), PdpProviderProtocolError> {
    if !admission.is_council_verified() {
        return Err(PdpProviderProtocolError::UntrustedAdmission);
    }
    if admission.provider_id() != &pending.challenge.provider_id {
        return Err(PdpProviderProtocolError::AdmissionProviderMismatch);
    }
    if admission.envelope_digest() != &pending.admission_envelope_digest {
        return Err(PdpProviderProtocolError::AdmissionInactive);
    }
    if admission.advert_key() != &pending.admitted_provider_key {
        return Err(PdpProviderProtocolError::UnauthorizedProviderKey);
    }
    Ok(())
}

/// Build and sign a canonical PDP proof from verified storage witnesses.
pub fn build_signed_pdp_proof_v1(
    challenge: &PdpChallengeV1,
    proof_leaves: Vec<PdpProofLeafV1>,
    issued_at_unix: u64,
    provider_key: &KeyPair,
) -> Result<PdpProofV1, PdpProofBuildError> {
    challenge
        .validate()
        .map_err(|error| PdpProofBuildError::InvalidChallenge(error.to_string()))?;
    let (algorithm, public_key) = provider_key.public_key().to_bytes();
    if algorithm != Algorithm::Ed25519 {
        return Err(PdpProofBuildError::UnsupportedSigningKey);
    }
    let public_key: [u8; 32] = public_key
        .try_into()
        .map_err(|_| PdpProofBuildError::UnsupportedSigningKey)?;
    let mut proof = PdpProofV1 {
        version: PDP_PROOF_VERSION_V1,
        commitment_digest: challenge.commitment_digest,
        challenge_id: challenge.challenge_id,
        manifest_digest: challenge.manifest_digest,
        provider_id: challenge.provider_id,
        epoch_id: challenge.epoch_id,
        proof_leaves,
        issued_at_unix,
        signature: PdpEd25519SignatureV1 {
            public_key,
            signature: [0; 64],
        },
    };
    let digest = proof.proof_digest().map_err(|error| {
        PdpProofBuildError::CanonicalEncoding(format!("digest unsigned PDP proof: {error}"))
    })?;
    let mut message = Vec::with_capacity(PDP_PROOF_SIGNATURE_DOMAIN_V1.len() + digest.len());
    message.extend_from_slice(PDP_PROOF_SIGNATURE_DOMAIN_V1);
    message.extend_from_slice(&digest);
    let signature = IrohaSignature::try_new(provider_key.private_key(), &message)
        .map_err(|error| PdpProofBuildError::Signing(error.to_string()))?;
    proof.signature.signature = signature.payload().try_into().map_err(|_| {
        PdpProofBuildError::Signing("unexpected Ed25519 signature length".to_owned())
    })?;
    proof
        .validate()
        .map_err(|error| PdpProofBuildError::InvalidProof(error.to_string()))?;
    proof
        .verify_signature()
        .map_err(|error| PdpProofBuildError::Signing(error.to_string()))?;
    Ok(proof)
}

/// Errors while constructing a provider-signed proof from storage witnesses.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PdpProofBuildError {
    /// Challenge is malformed.
    #[error("invalid PDP challenge: {0}")]
    InvalidChallenge(String),
    /// Only canonical Ed25519 provider keys are supported by v1.
    #[error("PDP v1 requires an Ed25519 provider signing key")]
    UnsupportedSigningKey,
    /// Unsigned or signed proof shape is invalid.
    #[error("invalid PDP proof: {0}")]
    InvalidProof(String),
    /// Canonical payload encoding failed.
    #[error("PDP proof canonical encoding failed: {0}")]
    CanonicalEncoding(String),
    /// Ed25519 signing failed.
    #[error("PDP proof signing failed: {0}")]
    Signing(String),
}

fn challenge_scope(challenge: &PdpChallengeV1) -> ChallengeScope {
    ChallengeScope {
        provider_id: challenge.provider_id,
        manifest_digest: challenge.manifest_digest,
        epoch_id: challenge.epoch_id,
    }
}

fn challenge_hot_leaf_count(challenge: &PdpChallengeV1) -> Result<u16, PdpProviderProtocolError> {
    let count = challenge
        .samples
        .iter()
        .try_fold(0usize, |total, sample| {
            total.checked_add(sample.hot_leaf_indices.len())
        })
        .ok_or(PdpProviderProtocolError::SampleCountOverflow)?;
    if count == 0 || count > PDP_MAX_TOTAL_HOT_LEAF_SAMPLES_V1 {
        return Err(PdpProviderProtocolError::SampleCountOverflow);
    }
    u16::try_from(count).map_err(|_| PdpProviderProtocolError::SampleCountOverflow)
}

fn build_repair_report(
    archive: &PdpGovernanceArchiveV1,
    reason: PdpRejectionReasonV1,
) -> Result<RepairReportV1, PdpProviderProtocolError> {
    let failure_kind = match reason {
        PdpRejectionReasonV1::DeadlineExpired | PdpRejectionReasonV1::SubmissionLate => {
            RepairPdpFailureKindV1::DeadlineExpired
        }
        PdpRejectionReasonV1::AdmissionRevoked | PdpRejectionReasonV1::AdmissionInactive => {
            RepairPdpFailureKindV1::AdmissionRevoked
        }
        PdpRejectionReasonV1::StorageUnavailable => RepairPdpFailureKindV1::StorageUnavailable,
        PdpRejectionReasonV1::FutureTimestamp | PdpRejectionReasonV1::InvalidProof => {
            RepairPdpFailureKindV1::InvalidProof
        }
    };
    let report = RepairReportV1 {
        version: REPAIR_REPORT_VERSION_V1,
        ticket_id: RepairTicketId(format!("PDP-{}", hex::encode_upper(archive.challenge_id))),
        auditor_account: "sorafs-pdp-runtime".to_owned(),
        submitted_at_unix: archive.decided_at_unix,
        evidence: RepairEvidenceV1 {
            version: REPAIR_EVIDENCE_VERSION_V1,
            manifest_digest: archive.manifest_digest,
            provider_id: archive.provider_id,
            por_history_id: None,
            cause: RepairCauseV1::PdpFailure(RepairPdpFailureCauseV1 {
                challenge_id: archive.challenge_id,
                epoch_id: archive.epoch_id,
                failed_samples: archive.sampled_hot_leaves,
                proof_digest: archive.proof_digest,
                failure_kind,
            }),
            evidence_json: None,
            notes: Some("pdp_failure".to_owned()),
        },
        notes: None,
    };
    report
        .validate()
        .map_err(|error| PdpProviderProtocolError::RepairReport(error.to_string()))?;
    Ok(report)
}

fn validate_archive(
    archive: &PdpGovernanceArchiveV1,
    policy: PdpProviderProtocolPolicyV1,
) -> Result<(), PdpProviderProtocolError> {
    archive.validate().map_err(|error| {
        PdpProviderProtocolError::InvalidCheckpoint(format!(
            "invalid typed PDP governance archive: {error}"
        ))
    })?;
    if archive.version != PDP_GOVERNANCE_ARCHIVE_VERSION_V1
        || archive.sequence == 0
        || archive.challenge_id == [0; 32]
        || archive.commitment_digest == [0; 32]
        || archive.manifest_digest == [0; 32]
        || archive.provider_id == [0; 32]
        || archive.epoch_id == 0
        || archive.sampled_segments == 0
        || archive.sampled_hot_leaves == 0
        || archive.issued_at_unix == 0
        || archive.response_deadline_unix <= archive.issued_at_unix
        || archive.decided_at_unix < archive.issued_at_unix
        || archive.admission_envelope_digest == [0; 32]
        || archive.canonical_challenge.is_empty()
        || archive.canonical_challenge.len() > policy.challenge_max_bytes as usize
        || archive
            .canonical_proof
            .as_ref()
            .is_some_and(|proof| proof.is_empty() || proof.len() > policy.proof_max_bytes as usize)
        || matches!(archive.decision, PdpTerminalDecisionV1::Accepted)
            && (archive.proof_digest.is_none()
                || archive.canonical_proof.is_none()
                || archive.sampled_bytes == 0)
        || archive.proof_digest.is_some() != archive.canonical_proof.is_some()
    {
        return Err(PdpProviderProtocolError::InvalidCheckpoint(
            "PDP governance archive metadata is inconsistent".to_owned(),
        ));
    }
    let challenge = decode_canonical_challenge(
        &archive.canonical_challenge,
        policy.challenge_max_bytes as usize,
    )?;
    if challenge.challenge_id != archive.challenge_id
        || challenge.commitment_digest != archive.commitment_digest
        || challenge.manifest_digest != archive.manifest_digest
        || challenge.provider_id != archive.provider_id
        || challenge.epoch_id != archive.epoch_id
        || challenge.issued_at_unix != archive.issued_at_unix
        || challenge.response_deadline_unix != archive.response_deadline_unix
        || u16::try_from(challenge.samples.len()).ok() != Some(archive.sampled_segments)
        || challenge_hot_leaf_count(&challenge)? != archive.sampled_hot_leaves
    {
        return Err(PdpProviderProtocolError::InvalidCheckpoint(
            "PDP governance archive disagrees with its canonical challenge".to_owned(),
        ));
    }
    if let Some(bytes) = archive.canonical_proof.as_ref() {
        let proof = decode_canonical_proof(bytes, policy.proof_max_bytes as usize)?;
        let digest = proof.proof_digest().map_err(|error| {
            PdpProviderProtocolError::CanonicalEncoding(format!("digest archived proof: {error}"))
        })?;
        if Some(digest) != archive.proof_digest || proof.challenge_id != archive.challenge_id {
            return Err(PdpProviderProtocolError::InvalidCheckpoint(
                "PDP governance archive proof digest or challenge binding is invalid".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_terminal(outcome: &PdpTerminalOutcomeV1) -> Result<(), PdpProviderProtocolError> {
    if outcome.sequence == 0
        || outcome.challenge_id == [0; 32]
        || outcome.challenge_payload_digest == [0; 32]
        || outcome.manifest_digest == [0; 32]
        || outcome.provider_id == [0; 32]
        || outcome.epoch_id == 0
        || outcome.decided_at_unix == 0
        || outcome.archive_digest == [0; 32]
        || outcome.archive_receipt_digest == [0; 32]
        || matches!(outcome.decision, PdpTerminalDecisionV1::Accepted)
            && (outcome.proof_digest.is_none() || outcome.repair_receipt_digest.is_some())
        || matches!(outcome.decision, PdpTerminalDecisionV1::Rejected(_))
            && outcome.repair_receipt_digest.is_none()
    {
        return Err(PdpProviderProtocolError::InvalidCheckpoint(
            "compact PDP terminal outcome is inconsistent".to_owned(),
        ));
    }
    Ok(())
}

fn validate_pending(
    pending: &PendingChallengeV1,
    policy: PdpProviderProtocolPolicyV1,
) -> Result<(), PdpProviderProtocolError> {
    pending
        .commitment
        .validate()
        .map_err(|error| PdpProviderProtocolError::InvalidCheckpoint(error.to_string()))?;
    pending
        .challenge
        .validate()
        .map_err(|error| PdpProviderProtocolError::InvalidCheckpoint(error.to_string()))?;
    let commitment_digest = pending.commitment.commitment_digest().map_err(|error| {
        PdpProviderProtocolError::CanonicalEncoding(format!("digest stored commitment: {error}"))
    })?;
    let challenge_bytes = norito::to_bytes(&pending.challenge).map_err(|error| {
        PdpProviderProtocolError::CanonicalEncoding(format!("encode stored challenge: {error}"))
    })?;
    if pending.sequence == 0
        || commitment_digest != pending.challenge.commitment_digest
        || pending.commitment.manifest_digest != pending.challenge.manifest_digest
        || pending.commitment.chunk_profile != pending.challenge.chunk_profile
        || pending.challenge.samples.len() > usize::from(pending.commitment.sample_window)
        || pending.challenge_payload_digest != *blake3::hash(&challenge_bytes).as_bytes()
        || challenge_bytes.len() > policy.challenge_max_bytes as usize
        || pending.admission_envelope_digest == [0; 32]
        || pending.admitted_provider_key == [0; 32]
        || pending.admitted_retention_epoch < pending.challenge.response_deadline_unix
        || pending.enqueued_at_unix == 0
        || pending.enqueued_at_unix > pending.challenge.response_deadline_unix
    {
        return Err(PdpProviderProtocolError::InvalidCheckpoint(
            "pending PDP challenge metadata is inconsistent".to_owned(),
        ));
    }
    Ok(())
}

fn validate_checkpoint(
    checkpoint: &PdpProviderCheckpointV1,
    policy: PdpProviderProtocolPolicyV1,
) -> Result<(), PdpProviderProtocolError> {
    if checkpoint.version != PDP_PROVIDER_CHECKPOINT_VERSION_V1 || checkpoint.next_sequence == 0 {
        return Err(PdpProviderProtocolError::InvalidCheckpoint(
            "unsupported PDP checkpoint version or zero next sequence".to_owned(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut scopes = BTreeSet::new();
    let mut previous_sequence = None;
    let mut pending_count = 0usize;
    let mut terminal_count = 0usize;
    for record in &checkpoint.records {
        let sequence = record.sequence();
        if previous_sequence.is_some_and(|previous| previous >= sequence)
            || !ids.insert(record.challenge_id())
            || !scopes.insert(record.scope())
        {
            return Err(PdpProviderProtocolError::InvalidCheckpoint(
                "PDP checkpoint records must have unique ids/scopes and increasing sequences"
                    .to_owned(),
            ));
        }
        previous_sequence = Some(sequence);
        match record {
            StoredChallengeV1::Pending(pending) => {
                pending_count += 1;
                validate_pending(pending, policy)?;
            }
            StoredChallengeV1::HandoffPending(handoff) => {
                pending_count += 1;
                validate_pending(&handoff.pending, policy)?;
                validate_archive(&handoff.archive, policy)?;
                let decision_requires_repair =
                    matches!(handoff.archive.decision, PdpTerminalDecisionV1::Rejected(_));
                if handoff.archive.sequence != handoff.pending.sequence
                    || handoff.archive.challenge_id != handoff.pending.challenge.challenge_id
                    || handoff.repair_report.is_some() != decision_requires_repair
                    || handoff.archive_receipt_digest == Some([0; 32])
                    || handoff.repair_receipt_digest == Some([0; 32])
                    || handoff.repair_receipt_digest.is_some()
                        && handoff.archive_receipt_digest.is_none()
                {
                    return Err(PdpProviderProtocolError::InvalidCheckpoint(
                        "PDP terminal handoff checkpoint is inconsistent".to_owned(),
                    ));
                }
                if let Some(report) = handoff.repair_report.as_ref() {
                    report.validate().map_err(|error| {
                        PdpProviderProtocolError::InvalidCheckpoint(format!(
                            "invalid PDP repair handoff: {error}"
                        ))
                    })?;
                }
            }
            StoredChallengeV1::Terminal(outcome) => {
                terminal_count += 1;
                validate_terminal(outcome)?;
            }
        }
    }
    if previous_sequence.is_some_and(|sequence| sequence >= checkpoint.next_sequence)
        || pending_count > policy.max_pending_records as usize
        || terminal_count > policy.max_terminal_records as usize
    {
        return Err(PdpProviderProtocolError::InvalidCheckpoint(
            "PDP checkpoint counters or retention bounds are invalid".to_owned(),
        ));
    }
    Ok(())
}

fn handoff_idempotency_key(
    archive: &PdpGovernanceArchiveV1,
) -> Result<[u8; 32], PdpProviderProtocolError> {
    let digest = governance_archive_digest(archive)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(HANDOFF_IDEMPOTENCY_DOMAIN_V1);
    hasher.update(&archive.challenge_id);
    hasher.update(&digest);
    Ok(*hasher.finalize().as_bytes())
}

fn governance_archive_digest(
    archive: &PdpGovernanceArchiveV1,
) -> Result<[u8; 32], PdpProviderProtocolError> {
    archive.digest().map_err(|error| {
        PdpProviderProtocolError::CanonicalEncoding(format!(
            "encode PDP governance archive: {error}"
        ))
    })
}

fn proof_decode_limits(max_bytes: usize) -> norito::DecodeLimits {
    // A canonical proof's nested owned vectors require slightly more than
    // twice the wire length after decoding. Keep the allocation budget a hard
    // multiple of the already-enforced payload ceiling so structurally valid
    // proofs can reach authentication.
    let allocation = max_bytes.saturating_mul(4);
    norito::DecodeLimits::new(max_bytes.max(1), max_bytes, max_bytes, allocation, 64)
}

fn checkpoint_decode_limits(policy: PdpProviderProtocolPolicyV1) -> norito::DecodeLimits {
    let max_bytes = usize::try_from(policy.checkpoint_max_bytes).unwrap_or(usize::MAX);
    norito::DecodeLimits::new(
        max_bytes.max(1),
        max_bytes,
        max_bytes,
        max_bytes.saturating_mul(2),
        64,
    )
}

fn decode_canonical_challenge(
    bytes: &[u8],
    max_bytes: usize,
) -> Result<PdpChallengeV1, PdpProviderProtocolError> {
    if bytes.is_empty() || bytes.len() > max_bytes {
        return Err(PdpProviderProtocolError::PayloadTooLarge {
            kind: "challenge",
            size: bytes.len(),
            limit: max_bytes,
        });
    }
    let challenge: PdpChallengeV1 =
        norito::decode_from_bytes_with_limits(bytes, proof_decode_limits(max_bytes)).map_err(
            |error| PdpProviderProtocolError::MalformedPayload {
                kind: "challenge",
                reason: error.to_string(),
            },
        )?;
    challenge
        .validate()
        .map_err(|error| PdpProviderProtocolError::InvalidChallenge(error.to_string()))?;
    let canonical = norito::to_bytes(&challenge).map_err(|error| {
        PdpProviderProtocolError::CanonicalEncoding(format!("encode PDP challenge: {error}"))
    })?;
    if canonical != bytes {
        return Err(PdpProviderProtocolError::NonCanonicalPayload { kind: "challenge" });
    }
    Ok(challenge)
}

fn decode_canonical_proof(
    bytes: &[u8],
    max_bytes: usize,
) -> Result<PdpProofV1, PdpProviderProtocolError> {
    if bytes.is_empty() || bytes.len() > max_bytes {
        return Err(PdpProviderProtocolError::PayloadTooLarge {
            kind: "proof",
            size: bytes.len(),
            limit: max_bytes,
        });
    }
    let proof: PdpProofV1 =
        norito::decode_from_bytes_with_limits(bytes, proof_decode_limits(max_bytes)).map_err(
            |error| PdpProviderProtocolError::MalformedPayload {
                kind: "proof",
                reason: error.to_string(),
            },
        )?;
    proof
        .validate()
        .map_err(|error| PdpProviderProtocolError::InvalidProof(error.to_string()))?;
    let canonical = norito::to_bytes(&proof).map_err(|error| {
        PdpProviderProtocolError::CanonicalEncoding(format!("encode PDP proof: {error}"))
    })?;
    if canonical != bytes {
        return Err(PdpProviderProtocolError::NonCanonicalPayload { kind: "proof" });
    }
    Ok(proof)
}

/// Provider protocol errors, classified for deterministic transport mapping.
#[derive(Debug, Error)]
pub enum PdpProviderProtocolError {
    /// Governance policy is invalid.
    #[error("invalid PDP provider policy: {0}")]
    InvalidPolicy(String),
    /// Commitment validation failed.
    #[error("invalid PDP commitment: {0}")]
    InvalidCommitment(String),
    /// Challenge validation failed.
    #[error("invalid PDP challenge: {0}")]
    InvalidChallenge(String),
    /// Proof validation failed.
    #[error("invalid PDP proof: {0}")]
    InvalidProof(String),
    /// Canonical payload encoding failed.
    #[error("PDP canonical encoding failed: {0}")]
    CanonicalEncoding(String),
    /// Payload could not be decoded safely.
    #[error("malformed canonical PDP {kind}: {reason}")]
    MalformedPayload {
        /// Stable payload class used for transport mapping.
        kind: &'static str,
        /// Bounded decoder failure description.
        reason: String,
    },
    /// Payload decoded but was not the exact canonical encoding.
    #[error("non-canonical PDP {kind} payload")]
    NonCanonicalPayload {
        /// Stable payload class used for transport mapping.
        kind: &'static str,
    },
    /// Payload exceeds a governed byte ceiling.
    #[error("PDP {kind} payload has {size} bytes; maximum is {limit}")]
    PayloadTooLarge {
        /// Stable payload class used for transport mapping.
        kind: &'static str,
        /// Observed payload size in bytes.
        size: usize,
        /// Governed payload-size ceiling in bytes.
        limit: usize,
    },
    /// Admission was not established under a council trust policy.
    #[error("PDP provider admission is not council verified")]
    UntrustedAdmission,
    /// Admission names a different provider.
    #[error("PDP admission provider does not match the challenge")]
    AdmissionProviderMismatch,
    /// Admission does not cover the challenge lifetime.
    #[error("PDP provider admission is not active for the complete challenge window")]
    AdmissionInactive,
    /// Provider key differs from the admission-pinned key.
    #[error("PDP proof key is not authorised by the active admission")]
    UnauthorizedProviderKey,
    /// Provider signature failed strict verification.
    #[error("PDP proof signature is invalid or unauthorised: {0}")]
    UnauthorizedProofSignature(#[source] PdpSignatureVerificationError),
    /// Challenge epoch differs from the governance scheduler epoch.
    #[error("PDP challenge epoch mismatch: expected {expected}, got {actual}")]
    EpochMismatch {
        /// Scheduler epoch expected by the receiving service.
        expected: u64,
        /// Epoch carried by the submitted challenge.
        actual: u64,
    },
    /// Challenge was already expired when enqueue was attempted.
    #[error("PDP challenge was expired at enqueue")]
    ChallengeExpiredAtEnqueue,
    /// Challenge issued-at exceeds configured future skew.
    #[error("PDP challenge issued_at is too far in the future")]
    ChallengeFromFuture,
    /// Response window is outside governed limits.
    #[error("PDP challenge response window is outside governed limits")]
    InvalidResponseWindow,
    /// Challenge does not bind the supplied commitment exactly.
    #[error("PDP challenge does not bind the supplied commitment")]
    ChallengeCommitmentMismatch,
    /// Exact id replay carried different payload or scope.
    #[error("PDP challenge id conflicts with retained state")]
    ChallengeConflict,
    /// Provider/manifest/epoch already has a different retained challenge.
    #[error("PDP provider/manifest/epoch scope was already challenged")]
    ChallengeScopeReplay,
    /// Pending queue capacity is exhausted.
    #[error("PDP pending challenge retention exhausted (limit {limit})")]
    PendingRetentionExhausted {
        /// Configured maximum retained pending records.
        limit: u32,
    },
    /// Compact terminal replay capacity is exhausted.
    #[error("PDP terminal challenge retention exhausted (limit {limit})")]
    TerminalRetentionExhausted {
        /// Configured maximum retained terminal records.
        limit: u32,
    },
    /// Sequence counter overflowed.
    #[error("PDP queue sequence exhausted")]
    SequenceExhausted,
    /// Lookup fields are inert.
    #[error("PDP provider lookup requires non-zero provider and timestamp")]
    InvalidLookup,
    /// Status export limit is zero or above the protocol cap.
    #[error("PDP status export limit {limit} must be between 1 and {max}")]
    InvalidExportLimit {
        /// Requested record count.
        limit: usize,
        /// Protocol maximum.
        max: usize,
    },
    /// Oldest provider challenge must be expired through the terminal path first.
    #[error("PDP challenge {challenge_id:?} requires expiry finalization")]
    ChallengeRequiresExpiry {
        /// Expired challenge that must be finalized through the terminal path.
        challenge_id: [u8; 32],
    },
    /// Challenge id is not retained.
    #[error("unknown PDP challenge")]
    UnknownChallenge,
    /// Challenge has not reached its deadline.
    #[error("PDP challenge is not expired")]
    ChallengeNotExpired,
    /// Terminal replay differs from the retained proof or decision.
    #[error("PDP terminal replay conflicts with retained outcome")]
    TerminalReplayConflict,
    /// Another lifecycle transition already changed the challenge state.
    #[error("PDP challenge changed during lifecycle transition")]
    ConcurrentTransition,
    /// Terminal handoff has not been prepared completely.
    #[error("PDP terminal handoff is incomplete")]
    HandoffNotPrepared,
    /// Governance archival failed.
    #[error("PDP governance archive handoff failed: {0}")]
    ArchiveHandoff(String),
    /// Repair scheduling failed.
    #[error("PDP repair handoff failed: {0}")]
    RepairHandoff(String),
    /// Repair report construction failed.
    #[error("PDP repair report failed validation: {0}")]
    RepairReport(String),
    /// Timestamp arithmetic overflowed.
    #[error("PDP timestamp arithmetic overflow")]
    TimestampOverflow,
    /// Terminal timestamp predates the challenge.
    #[error("PDP terminal timestamp is invalid")]
    InvalidTerminalTimestamp,
    /// Challenge sample count overflowed its bounded representation.
    #[error("PDP challenge sample count overflow")]
    SampleCountOverflow,
    /// Runtime mutex was poisoned.
    #[error("PDP provider runtime lock poisoned")]
    RuntimePoisoned,
    /// Durable checkpoint is corrupt or inconsistent.
    #[error("invalid PDP provider checkpoint: {0}")]
    InvalidCheckpoint(String),
    /// Durable checkpoint path is unsafe or inaccessible.
    #[error("PDP provider checkpoint I/O failed: {0}")]
    CheckpointIo(String),
    /// Another runtime changed the checkpoint after this instance loaded it.
    #[error("PDP provider checkpoint changed concurrently; stale writer rejected")]
    StaleCheckpoint,
    /// Another process currently owns the checkpoint writer lock.
    #[error("PDP provider checkpoint writer is busy")]
    CheckpointBusy,
    /// The checkpoint rename became visible but parent-directory durability is uncertain.
    #[error("PDP provider checkpoint durability is uncertain: {0}")]
    CheckpointDurabilityUncertain(String),
    /// This runtime stopped after an uncertain durable commit and requires restart.
    #[error("PDP provider runtime durability is poisoned: {0}")]
    DurabilityPoisoned(String),
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        fs,
        sync::{Arc, Barrier, atomic::AtomicU64},
        thread,
    };

    use ed25519_dalek::{Signer as _, SigningKey};
    use sorafs_manifest::{
        AdvertEndpoint, AvailabilityTier, CapabilityTlv, CapabilityType, ChunkingProfileV1,
        CouncilSignature, EndpointAdmissionV1, EndpointAttestationKind, EndpointAttestationV1,
        EndpointKind, PathDiversityPolicy, ProfileId, ProviderAdmissionCouncilPolicy,
        ProviderAdmissionEnvelopeV1, ProviderAdmissionProposalV1, ProviderAdvertBodyV1,
        ProviderVrfPublicKeyV1, QosHints, StakePointer, compute_advert_body_digest,
        compute_envelope_authorization_digest, compute_proposal_digest,
        pdp::{PDP_HOT_LEAF_SIZE_V1, PDP_SEGMENT_SIZE_V1, PdpMerkleTreeV1},
        sign_pdp_proof_ed25519_v1, verify_pdp_bundle_v1,
    };
    use tempfile::TempDir;

    use crate::{
        NodeHandle, NodeInitError, config::StorageConfig,
        proof_outcome_forwarder::PROOF_OUTCOME_OUTBOX_CHECKPOINT_FILE_NAME_V1,
    };

    use super::*;

    const PROVIDER_ID: [u8; 32] = [0x31; 32];
    const MANIFEST_DIGEST: [u8; 32] = [0x42; 32];
    const ISSUED_AT: u64 = 1_000;
    const DEADLINE: u64 = 1_300;

    struct Fixture {
        payload: Vec<u8>,
        tree: PdpMerkleTreeV1,
        commitment: PdpCommitmentV1,
        challenge: PdpChallengeV1,
        proof: PdpProofV1,
        dalek_provider_key: SigningKey,
        admission: AdmissionRecord,
        envelope: ProviderAdmissionEnvelopeV1,
    }

    fn canonical_profile() -> ChunkingProfileV1 {
        ChunkingProfileV1::from_descriptor(
            sorafs_manifest::chunker_registry::lookup(ProfileId(1)).expect("SF1 profile"),
        )
    }

    fn provider_key() -> KeyPair {
        KeyPair::try_from_seed(vec![0x21; 32], Algorithm::Ed25519)
            .expect("deterministic provider key")
    }

    fn synthetic_admission(
        provider_id: [u8; 32],
        advert_key: [u8; 32],
    ) -> (AdmissionRecord, ProviderAdmissionEnvelopeV1) {
        let descriptor =
            sorafs_manifest::chunker_registry::lookup(ProfileId(1)).expect("SF1 profile");
        let profile_aliases = Some(
            descriptor
                .aliases
                .iter()
                .map(|alias| (*alias).to_owned())
                .collect(),
        );
        let stake = StakePointer {
            pool_id: [0x91; 32],
            stake_amount: "0.000000001".parse().expect("canonical XOR stake amount"),
        };
        let capability = CapabilityTlv {
            cap_type: CapabilityType::ToriiGateway,
            payload: Vec::new(),
        };
        let endpoint = AdvertEndpoint {
            kind: EndpointKind::Torii,
            host_pattern: "pdp.example.test".to_owned(),
            metadata: Vec::new(),
        };
        let endpoint_admission = EndpointAdmissionV1 {
            endpoint: endpoint.clone(),
            attestation: EndpointAttestationV1 {
                version: sorafs_manifest::ENDPOINT_ATTESTATION_VERSION_V1,
                kind: EndpointAttestationKind::Mtls,
                attested_at: 800,
                expires_at: 100_000,
                leaf_certificate: vec![1],
                intermediate_certificates: Vec::new(),
                alpn_ids: vec!["h2".to_owned()],
                report: Vec::new(),
            },
        };
        let (vrf_public, vrf_private) =
            iroha_crypto::BlsNormal::keypair(iroha_crypto::KeyGenOption::UseSeed(vec![0x34; 32]))
                .expect("fixture BLS keypair");
        let vrf_pair: KeyPair = (vrf_public, vrf_private).into();
        let proposal = ProviderAdmissionProposalV1 {
            version: sorafs_manifest::PROVIDER_ADMISSION_PROPOSAL_VERSION_V1,
            provider_id,
            profile_id: "sorafs.sf1@1.0.0".to_owned(),
            profile_aliases,
            stake: stake.clone(),
            capabilities: vec![capability.clone()],
            endpoints: vec![endpoint_admission],
            advert_key,
            por_vrf_key: ProviderVrfPublicKeyV1::BlsNormal(
                vrf_pair
                    .public_key()
                    .to_bytes()
                    .1
                    .try_into()
                    .expect("BLS key length"),
            ),
            jurisdiction_code: "US".to_owned(),
            contact_uri: None,
            stream_budget: None,
            transport_hints: None,
        };
        let advert_body = ProviderAdvertBodyV1 {
            provider_id,
            profile_id: proposal.profile_id.clone(),
            profile_aliases: proposal.profile_aliases.clone(),
            stake,
            qos: QosHints {
                availability: AvailabilityTier::Hot,
                max_retrieval_latency_ms: 1,
                max_concurrent_streams: 1,
            },
            capabilities: vec![capability],
            endpoints: vec![endpoint],
            rendezvous_topics: Vec::new(),
            path_policy: PathDiversityPolicy {
                min_guard_weight: 1,
                max_same_asn_per_path: 1,
                max_same_pool_per_path: 1,
            },
            notes: None,
            stream_budget: None,
            transport_hints: None,
        };
        let mut envelope = ProviderAdmissionEnvelopeV1 {
            version: sorafs_manifest::PROVIDER_ADMISSION_ENVELOPE_VERSION_V1,
            proposal_digest: compute_proposal_digest(&proposal).expect("proposal digest"),
            advert_body_digest: compute_advert_body_digest(&advert_body)
                .expect("advert body digest"),
            proposal,
            advert_body,
            issued_at: 800,
            retention_epoch: 100_000,
            council_signatures: Vec::new(),
            notes: None,
        };
        let council_key = SigningKey::from_bytes(&[0x61; 32]);
        let authorization_digest = compute_envelope_authorization_digest(&envelope)
            .expect("envelope authorization digest");
        envelope.council_signatures.push(CouncilSignature {
            signer: council_key.verifying_key().to_bytes(),
            signature: council_key.sign(&authorization_digest).to_bytes().to_vec(),
        });
        let policy =
            ProviderAdmissionCouncilPolicy::new([council_key.verifying_key().to_bytes()], 1)
                .expect("council policy");
        let admission = AdmissionRecord::new(envelope.clone(), &policy).expect("admission");
        (admission, envelope)
    }

    fn fixture(epoch_id: u64) -> Fixture {
        let payload = (0..(PDP_SEGMENT_SIZE_V1 as usize + PDP_HOT_LEAF_SIZE_V1 as usize + 37))
            .map(|index| ((index.wrapping_mul(131).wrapping_add(17)) % 251) as u8)
            .collect::<Vec<_>>();
        let tree = PdpMerkleTreeV1::from_bytes(&payload).expect("PDP tree");
        let commitment =
            PdpCommitmentV1::from_tree(&tree, MANIFEST_DIGEST, canonical_profile(), 4, 900)
                .expect("commitment");
        let mut seed = [0x51; 32];
        seed[..8].copy_from_slice(&epoch_id.to_le_bytes());
        let challenge = PdpChallengeV1::new(
            commitment.commitment_digest().expect("commitment digest"),
            MANIFEST_DIGEST,
            PROVIDER_ID,
            canonical_profile(),
            seed,
            epoch_id,
            11 + epoch_id,
            ISSUED_AT,
            DEADLINE,
            vec![
                sorafs_manifest::pdp::PdpSampleV1 {
                    segment_index: 0,
                    hot_leaf_indices: vec![0, 63],
                },
                sorafs_manifest::pdp::PdpSampleV1 {
                    segment_index: 1,
                    hot_leaf_indices: vec![0, 1],
                },
            ],
        )
        .expect("challenge");
        let provider_key = provider_key();
        let dalek_provider_key = SigningKey::from_bytes(&[0x21; 32]);
        assert_eq!(
            provider_key.public_key().to_bytes().1,
            dalek_provider_key.verifying_key().to_bytes()
        );
        let proof = build_signed_pdp_proof_v1(
            &challenge,
            tree.prove_samples(&challenge.samples, &payload)
                .expect("proof leaves"),
            1_100,
            &provider_key,
        )
        .expect("signed proof");
        let (admission, envelope) =
            synthetic_admission(PROVIDER_ID, dalek_provider_key.verifying_key().to_bytes());
        Fixture {
            payload,
            tree,
            commitment,
            challenge,
            proof,
            dalek_provider_key,
            admission,
            envelope,
        }
    }

    fn resign(fixture: &Fixture, proof: &mut PdpProofV1) {
        *proof = sign_pdp_proof_ed25519_v1(proof.clone(), &fixture.dalek_provider_key)
            .expect("re-sign proof");
    }

    fn rebind_challenge(challenge: &mut PdpChallengeV1) {
        challenge.challenge_id = challenge.derived_challenge_id().expect("challenge id");
    }

    type RecordedHandoffReceipts = BTreeMap<[u8; 32], ([u8; 32], [u8; 32])>;

    #[derive(Debug, Default)]
    struct RecordingHandoff {
        archives: Mutex<RecordedHandoffReceipts>,
        repairs: Mutex<RecordedHandoffReceipts>,
        archive_calls: AtomicU64,
        repair_calls: AtomicU64,
        fail_archives: AtomicU64,
        fail_repairs: AtomicU64,
    }

    impl RecordingHandoff {
        fn failing(archive_failures: u64, repair_failures: u64) -> Self {
            Self {
                fail_archives: AtomicU64::new(archive_failures),
                fail_repairs: AtomicU64::new(repair_failures),
                ..Self::default()
            }
        }

        fn archive_count(&self) -> u64 {
            self.archive_calls.load(Ordering::Relaxed)
        }

        fn repair_count(&self) -> u64 {
            self.repair_calls.load(Ordering::Relaxed)
        }
    }

    fn consume_failure(counter: &AtomicU64) -> bool {
        counter
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                (value != 0).then(|| value - 1)
            })
            .is_ok()
    }

    fn receipt(tag: &[u8], key: [u8; 32], payload_digest: [u8; 32]) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(tag);
        hasher.update(&key);
        hasher.update(&payload_digest);
        *hasher.finalize().as_bytes()
    }

    impl PdpTerminalHandoff for RecordingHandoff {
        fn archive(
            &self,
            idempotency_key: [u8; 32],
            payload: &PdpGovernanceArchiveV1,
        ) -> Result<[u8; 32], PdpExternalHandoffError> {
            self.archive_calls.fetch_add(1, Ordering::Relaxed);
            if consume_failure(&self.fail_archives) {
                return Err(PdpExternalHandoffError(
                    "injected archive failure".to_owned(),
                ));
            }
            let digest = payload
                .digest()
                .map_err(|error| PdpExternalHandoffError(error.to_string()))?;
            let receipt = receipt(b"archive", idempotency_key, digest);
            let mut archives = self.archives.lock().expect("archive lock");
            match archives.get(&idempotency_key) {
                Some((existing, existing_receipt)) if *existing == digest => Ok(*existing_receipt),
                Some(_) => Err(PdpExternalHandoffError(
                    "archive idempotency conflict".to_owned(),
                )),
                None => {
                    archives.insert(idempotency_key, (digest, receipt));
                    Ok(receipt)
                }
            }
        }

        fn repair(
            &self,
            idempotency_key: [u8; 32],
            report: &RepairReportV1,
        ) -> Result<[u8; 32], PdpExternalHandoffError> {
            self.repair_calls.fetch_add(1, Ordering::Relaxed);
            if consume_failure(&self.fail_repairs) {
                return Err(PdpExternalHandoffError(
                    "injected repair failure".to_owned(),
                ));
            }
            report
                .validate()
                .map_err(|error| PdpExternalHandoffError(error.to_string()))?;
            assert!(matches!(
                report.evidence.cause,
                RepairCauseV1::PdpFailure(_)
            ));
            let bytes = norito::to_bytes(report)
                .map_err(|error| PdpExternalHandoffError(error.to_string()))?;
            let digest = *blake3::hash(&bytes).as_bytes();
            let receipt = receipt(b"repair", idempotency_key, digest);
            let mut repairs = self.repairs.lock().expect("repair lock");
            match repairs.get(&idempotency_key) {
                Some((existing, existing_receipt)) if *existing == digest => Ok(*existing_receipt),
                Some(_) => Err(PdpExternalHandoffError(
                    "repair idempotency conflict".to_owned(),
                )),
                None => {
                    repairs.insert(idempotency_key, (digest, receipt));
                    Ok(receipt)
                }
            }
        }
    }

    fn enqueue(protocol: &PdpProviderProtocol, fixture: &Fixture) -> PdpChallengeEnqueueOutcome {
        protocol
            .enqueue_challenge(
                fixture.commitment.clone(),
                fixture.challenge.clone(),
                &fixture.admission,
                fixture.challenge.epoch_id,
                ISSUED_AT,
            )
            .expect("enqueue challenge")
    }

    #[test]
    fn policy_rejects_inert_or_inconsistent_bounds() {
        let mut policy = PdpProviderProtocolPolicyV1::default();
        policy.max_pending_records = 0;
        assert!(policy.validate().is_err());
        let mut policy = PdpProviderProtocolPolicyV1::default();
        policy.min_response_window_secs = 601;
        policy.max_response_window_secs = 600;
        assert!(policy.validate().is_err());
        let mut policy = PdpProviderProtocolPolicyV1::default();
        policy.proof_max_bytes = PDP_PROOF_MAX_CANONICAL_BYTES_V1 as u32 + 1;
        assert!(policy.validate().is_err());
        let mut policy = PdpProviderProtocolPolicyV1::default();
        policy.checkpoint_max_bytes =
            u64::from(policy.challenge_max_bytes) + u64::from(policy.proof_max_bytes) - 1;
        assert!(policy.validate().is_err());
    }

    #[test]
    fn proof_builder_is_admission_bound_and_verifies_both_roots() {
        let fixture = fixture(7);
        assert!(!fixture.payload.is_empty());
        assert!(fixture.tree.hot_leaf_count() > 1);
        let verified = verify_pdp_bundle_v1(
            &fixture.commitment,
            &fixture.challenge,
            &fixture.proof,
            &fixture.admission,
        )
        .expect("admission-bound proof");
        assert_eq!(verified.challenge_id(), &fixture.challenge.challenge_id);
        assert_eq!(verified.sampled_segments(), 2);
        assert_eq!(verified.sampled_hot_leaves(), 4);
    }

    #[test]
    fn durable_happy_path_is_ordered_idempotent_and_restart_safe() {
        let dir = TempDir::new().expect("tempdir");
        let policy = PdpProviderProtocolPolicyV1::default();
        let protocol = PdpProviderProtocol::open(policy, dir.path()).expect("open runtime");
        let fixture = fixture(7);
        assert_eq!(
            enqueue(&protocol, &fixture),
            PdpChallengeEnqueueOutcome::Inserted { sequence: 1 }
        );
        assert_eq!(
            enqueue(&protocol, &fixture),
            PdpChallengeEnqueueOutcome::Existing { sequence: 1 }
        );
        let next = protocol
            .next_challenge(PROVIDER_ID, 1_050)
            .expect("next")
            .expect("pending challenge");
        assert_eq!(next.sequence, 1);
        assert_eq!(next.challenge, fixture.challenge);

        let sink = RecordingHandoff::default();
        let proof_bytes = norito::to_bytes(&fixture.proof).expect("proof bytes");
        let accepted = protocol
            .submit_proof_for_challenge_bytes(
                fixture.challenge.challenge_id,
                &proof_bytes,
                &fixture.admission,
                1_100,
                &sink,
            )
            .expect("accepted proof");
        assert_eq!(accepted.decision, PdpTerminalDecisionV1::Accepted);
        assert_eq!(sink.archive_count(), 1);
        assert_eq!(sink.repair_count(), 0);
        assert!(
            protocol
                .next_challenge(PROVIDER_ID, 1_100)
                .expect("next")
                .is_none()
        );

        drop(protocol);
        let restored = PdpProviderProtocol::open(policy, dir.path()).expect("restart runtime");
        assert_eq!(
            restored
                .terminal_outcome(&fixture.challenge.challenge_id)
                .expect("terminal"),
            Some(accepted.clone())
        );
        assert_eq!(
            restored
                .submit_proof_for_challenge_bytes(
                    fixture.challenge.challenge_id,
                    &proof_bytes,
                    &fixture.admission,
                    1_101,
                    &sink,
                )
                .expect("idempotent proof replay"),
            accepted
        );
        assert_eq!(sink.archive_count(), 1);
    }

    #[test]
    fn enqueue_rejects_untrusted_wrong_epoch_future_expired_and_scope_grinding() {
        let protocol =
            PdpProviderProtocol::in_memory(PdpProviderProtocolPolicyV1::default()).unwrap();
        let fixture = fixture(7);
        let untrusted = AdmissionRecord::new_untrusted_signers(fixture.envelope.clone())
            .expect("integrity-only admission");
        assert!(matches!(
            protocol.enqueue_challenge(
                fixture.commitment.clone(),
                fixture.challenge.clone(),
                &untrusted,
                7,
                ISSUED_AT,
            ),
            Err(PdpProviderProtocolError::UntrustedAdmission)
        ));
        assert!(matches!(
            protocol.enqueue_challenge(
                fixture.commitment.clone(),
                fixture.challenge.clone(),
                &fixture.admission,
                8,
                ISSUED_AT,
            ),
            Err(PdpProviderProtocolError::EpochMismatch { .. })
        ));
        let mut future = fixture.challenge.clone();
        future.issued_at_unix = 2_000;
        future.response_deadline_unix = 2_300;
        rebind_challenge(&mut future);
        assert!(matches!(
            protocol.enqueue_challenge(
                fixture.commitment.clone(),
                future,
                &fixture.admission,
                7,
                ISSUED_AT,
            ),
            Err(PdpProviderProtocolError::ChallengeFromFuture)
        ));
        assert!(matches!(
            protocol.enqueue_challenge(
                fixture.commitment.clone(),
                fixture.challenge.clone(),
                &fixture.admission,
                7,
                DEADLINE + 1,
            ),
            Err(PdpProviderProtocolError::ChallengeExpiredAtEnqueue)
        ));
        enqueue(&protocol, &fixture);
        let mut alternate = fixture.challenge.clone();
        alternate.seed[0] ^= 1;
        alternate.drand_round += 1;
        rebind_challenge(&mut alternate);
        assert!(matches!(
            protocol.enqueue_challenge(
                fixture.commitment.clone(),
                alternate,
                &fixture.admission,
                7,
                ISSUED_AT,
            ),
            Err(PdpProviderProtocolError::ChallengeScopeReplay)
        ));
    }

    #[test]
    fn authenticated_wrong_manifest_provider_epoch_and_samples_are_terminal_failures() {
        type Mutation = fn(&mut PdpProofV1);
        let mutations: [Mutation; 4] = [
            |proof| proof.manifest_digest = [0x77; 32],
            |proof| proof.provider_id = [0x88; 32],
            |proof| proof.epoch_id += 1,
            |proof| {
                proof.proof_leaves[0].hot_leaves.pop();
            },
        ];
        for (offset, mutate) in mutations.into_iter().enumerate() {
            let fixture = fixture(20 + offset as u64);
            let protocol =
                PdpProviderProtocol::in_memory(PdpProviderProtocolPolicyV1::default()).unwrap();
            enqueue(&protocol, &fixture);
            let mut proof = fixture.proof.clone();
            mutate(&mut proof);
            resign(&fixture, &mut proof);
            let sink = RecordingHandoff::default();
            let outcome = protocol
                .submit_proof_for_challenge_bytes(
                    fixture.challenge.challenge_id,
                    &norito::to_bytes(&proof).expect("proof bytes"),
                    &fixture.admission,
                    1_100,
                    &sink,
                )
                .expect("authenticated invalid proof is terminal");
            assert_eq!(
                outcome.decision,
                PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::InvalidProof)
            );
            assert_eq!(sink.archive_count(), 1);
            assert_eq!(sink.repair_count(), 1);
        }
    }

    #[test]
    fn wrong_key_malformed_noncanonical_and_oversized_inputs_converge_to_repair() {
        let mut policy = PdpProviderProtocolPolicyV1::default();
        policy.proof_max_bytes = 64 * 1024;
        for (offset, payload) in (0_u64..4).map(|offset| {
            let fixture = fixture(70 + offset);
            let payload = match offset {
                0 => {
                    let other_key = SigningKey::from_bytes(&[0x22; 32]);
                    let forged = sign_pdp_proof_ed25519_v1(fixture.proof.clone(), &other_key)
                        .expect("wrong-key proof");
                    norito::to_bytes(&forged).unwrap()
                }
                1 => vec![1, 2, 3],
                2 => {
                    let mut trailing = norito::to_bytes(&fixture.proof).unwrap();
                    trailing.push(0);
                    trailing
                }
                3 => vec![0xAA; policy.proof_max_bytes as usize + 1],
                _ => unreachable!(),
            };
            (offset, (fixture, payload))
        }) {
            let (fixture, payload) = payload;
            let protocol = PdpProviderProtocol::in_memory(policy).unwrap();
            enqueue(&protocol, &fixture);
            let sink = RecordingHandoff::default();
            let outcome = protocol
                .submit_proof_for_challenge_bytes(
                    fixture.challenge.challenge_id,
                    &payload,
                    &fixture.admission,
                    1_100,
                    &sink,
                )
                .unwrap_or_else(|error| panic!("case {offset} failed to converge: {error}"));
            assert_eq!(
                outcome.decision,
                PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::InvalidProof)
            );
            assert_eq!(sink.archive_count(), 1);
            assert_eq!(sink.repair_count(), 1);
        }
    }

    #[test]
    fn explicit_challenge_identity_blocks_cross_challenge_proof_consumption() {
        let first = fixture(80);
        let second = fixture(81);
        let protocol =
            PdpProviderProtocol::in_memory(PdpProviderProtocolPolicyV1::default()).unwrap();
        enqueue(&protocol, &first);
        enqueue(&protocol, &second);
        let sink = RecordingHandoff::default();

        let first_outcome = protocol
            .submit_proof_for_challenge_bytes(
                first.challenge.challenge_id,
                &norito::to_bytes(&second.proof).expect("cross-bound proof bytes"),
                &first.admission,
                1_100,
                &sink,
            )
            .expect("cross-bound proof becomes authoritative failure");
        assert_eq!(
            first_outcome.decision,
            PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::InvalidProof)
        );
        assert_eq!(
            protocol
                .challenge_status(&second.challenge.challenge_id)
                .expect("second status")
                .expect("second retained")
                .lifecycle,
            PdpChallengeLifecycleV1::Pending,
            "a proof naming the second challenge must not consume it through the first endpoint"
        );

        let second_outcome = protocol
            .submit_proof_for_challenge_bytes(
                second.challenge.challenge_id,
                &norito::to_bytes(&second.proof).expect("second proof bytes"),
                &second.admission,
                1_100,
                &sink,
            )
            .expect("second challenge remains independently completable");
        assert_eq!(second_outcome.decision, PdpTerminalDecisionV1::Accepted);
        assert_eq!(sink.archive_count(), 2);
        assert_eq!(sink.repair_count(), 1);
    }

    #[test]
    fn status_export_is_bounded_ordered_and_exposes_pending_handoff_and_terminal_states() {
        let pending = fixture(82);
        let failing = fixture(83);
        let protocol =
            PdpProviderProtocol::in_memory(PdpProviderProtocolPolicyV1::default()).unwrap();
        enqueue(&protocol, &pending);
        enqueue(&protocol, &failing);
        let sink = RecordingHandoff::failing(1, 0);
        let mut invalid = failing.proof.clone();
        invalid.manifest_digest = [0x77; 32];
        resign(&failing, &mut invalid);
        assert!(matches!(
            protocol.submit_proof_for_challenge_bytes(
                failing.challenge.challenge_id,
                &norito::to_bytes(&invalid).expect("invalid proof bytes"),
                &failing.admission,
                1_100,
                &sink,
            ),
            Err(PdpProviderProtocolError::ArchiveHandoff(_))
        ));

        let statuses = protocol.export_statuses(0, 2).expect("bounded export");
        assert_eq!(statuses.len(), 2);
        assert!(statuses[0].sequence < statuses[1].sequence);
        assert_eq!(statuses[0].lifecycle, PdpChallengeLifecycleV1::Pending);
        assert_eq!(
            statuses[1].lifecycle,
            PdpChallengeLifecycleV1::HandoffPending
        );
        assert_eq!(
            statuses[1].decision,
            Some(PdpTerminalDecisionV1::Rejected(
                PdpRejectionReasonV1::InvalidProof
            ))
        );
        assert!(matches!(
            protocol.export_statuses(0, 0),
            Err(PdpProviderProtocolError::InvalidExportLimit { .. })
        ));
        assert!(matches!(
            protocol.export_statuses(0, PDP_STATUS_EXPORT_MAX_RECORDS_V1 + 1),
            Err(PdpProviderProtocolError::InvalidExportLimit { .. })
        ));

        protocol
            .resume_handoffs(&sink, 1)
            .expect("finish durable handoff");
        let status = protocol
            .challenge_status(&failing.challenge.challenge_id)
            .expect("status lookup")
            .expect("retained status");
        assert_eq!(status.lifecycle, PdpChallengeLifecycleV1::Terminal);
        assert!(status.proof_digest.is_some());
        assert_eq!(
            protocol
                .export_statuses(statuses[0].sequence, 1)
                .expect("cursor page")[0]
                .challenge_id,
            failing.challenge.challenge_id
        );
    }

    #[test]
    fn overlapping_proof_race_has_one_terminal_winner_and_one_replay_rejection() {
        let fixture = Arc::new(fixture(84));
        let protocol = Arc::new(
            PdpProviderProtocol::in_memory(PdpProviderProtocolPolicyV1::default()).unwrap(),
        );
        enqueue(&protocol, &fixture);
        let sink = Arc::new(RecordingHandoff::default());
        let barrier = Arc::new(Barrier::new(2));
        let valid_bytes = norito::to_bytes(&fixture.proof).expect("valid proof bytes");
        let mut invalid = fixture.proof.clone();
        invalid.manifest_digest = [0x77; 32];
        resign(&fixture, &mut invalid);
        let invalid_bytes = norito::to_bytes(&invalid).expect("invalid proof bytes");

        let handles = [valid_bytes, invalid_bytes].map(|proof_bytes| {
            let fixture = Arc::clone(&fixture);
            let protocol = Arc::clone(&protocol);
            let sink = Arc::clone(&sink);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                protocol.submit_proof_for_challenge_bytes(
                    fixture.challenge.challenge_id,
                    &proof_bytes,
                    &fixture.admission,
                    1_100,
                    sink.as_ref(),
                )
            })
        });
        let results = handles.map(|handle| handle.join().expect("proof thread"));
        let successes = results.iter().filter(|result| result.is_ok()).count();
        assert_eq!(successes, 1, "exactly one overlapping proof may win");
        assert!(
            results
                .iter()
                .filter_map(|result| result.as_ref().err())
                .all(|error| matches!(
                    error,
                    PdpProviderProtocolError::ConcurrentTransition
                        | PdpProviderProtocolError::TerminalReplayConflict
                ))
        );
        let terminal = protocol
            .terminal_outcome(&fixture.challenge.challenge_id)
            .expect("terminal lookup")
            .expect("one terminal winner");
        assert!(matches!(
            terminal.decision,
            PdpTerminalDecisionV1::Accepted
                | PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::InvalidProof)
        ));
        assert_eq!(sink.archive_count(), 1);
        assert_eq!(
            sink.repair_count(),
            u64::from(matches!(
                terminal.decision,
                PdpTerminalDecisionV1::Rejected(_)
            ))
        );
    }

    #[test]
    fn late_and_future_signed_proofs_are_rejected_and_repaired() {
        let late = fixture(30);
        let protocol =
            PdpProviderProtocol::in_memory(PdpProviderProtocolPolicyV1::default()).unwrap();
        enqueue(&protocol, &late);
        let sink = RecordingHandoff::default();
        let outcome = protocol
            .submit_proof_for_challenge_bytes(
                late.challenge.challenge_id,
                &norito::to_bytes(&late.proof).unwrap(),
                &late.admission,
                DEADLINE + 1,
                &sink,
            )
            .expect("late proof terminal");
        assert_eq!(
            outcome.decision,
            PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::SubmissionLate)
        );

        let future = fixture(31);
        let protocol =
            PdpProviderProtocol::in_memory(PdpProviderProtocolPolicyV1::default()).unwrap();
        enqueue(&protocol, &future);
        let mut proof = future.proof.clone();
        proof.issued_at_unix = 1_010;
        resign(&future, &mut proof);
        let outcome = protocol
            .submit_proof_for_challenge_bytes(
                future.challenge.challenge_id,
                &norito::to_bytes(&proof).unwrap(),
                &future.admission,
                1_001,
                &sink,
            )
            .expect("future proof terminal");
        assert_eq!(
            outcome.decision,
            PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::FutureTimestamp)
        );
    }

    #[test]
    fn expiry_and_revocation_create_repair_bound_terminal_outcomes() {
        let expiry_fixture = fixture(40);
        let protocol =
            PdpProviderProtocol::in_memory(PdpProviderProtocolPolicyV1::default()).unwrap();
        enqueue(&protocol, &expiry_fixture);
        let sink = RecordingHandoff::default();
        assert!(matches!(
            protocol.expire_challenge(expiry_fixture.challenge.challenge_id, DEADLINE, &sink),
            Err(PdpProviderProtocolError::ChallengeNotExpired)
        ));
        let expired = protocol
            .expire_challenge(expiry_fixture.challenge.challenge_id, DEADLINE + 1, &sink)
            .expect("expire challenge");
        assert_eq!(
            expired.decision,
            PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::DeadlineExpired)
        );

        let revocation_fixture = fixture(41);
        let protocol =
            PdpProviderProtocol::in_memory(PdpProviderProtocolPolicyV1::default()).unwrap();
        enqueue(&protocol, &revocation_fixture);
        let revoked = protocol
            .reject_revoked(revocation_fixture.challenge.challenge_id, 1_100, &sink)
            .expect("revoked challenge");
        assert_eq!(
            revoked.decision,
            PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::AdmissionRevoked)
        );
        assert_eq!(sink.repair_count(), 2);
    }

    #[test]
    fn archive_and_repair_failures_resume_without_premature_terminal_ack() {
        let dir = TempDir::new().expect("tempdir");
        let policy = PdpProviderProtocolPolicyV1::default();
        let fixture = fixture(50);
        let protocol = PdpProviderProtocol::open(policy, dir.path()).expect("runtime");
        enqueue(&protocol, &fixture);
        let mut invalid = fixture.proof.clone();
        invalid.manifest_digest = [0x77; 32];
        resign(&fixture, &mut invalid);
        let sink = RecordingHandoff::failing(1, 1);
        assert!(matches!(
            protocol.submit_proof_for_challenge_bytes(
                fixture.challenge.challenge_id,
                &norito::to_bytes(&invalid).unwrap(),
                &fixture.admission,
                1_100,
                &sink,
            ),
            Err(PdpProviderProtocolError::ArchiveHandoff(_))
        ));
        assert!(
            protocol
                .terminal_outcome(&fixture.challenge.challenge_id)
                .unwrap()
                .is_none()
        );
        drop(protocol);

        let restored =
            PdpProviderProtocol::open(policy, dir.path()).expect("restart after archive");
        assert!(matches!(
            restored.resume_handoffs(&sink, 10),
            Err(PdpProviderProtocolError::RepairHandoff(_))
        ));
        assert_eq!(sink.archive_count(), 2);
        assert_eq!(sink.repair_count(), 1);
        drop(restored);

        let restored = PdpProviderProtocol::open(policy, dir.path()).expect("restart after repair");
        let outcomes = restored.resume_handoffs(&sink, 10).expect("resume repair");
        assert_eq!(outcomes.len(), 1);
        assert!(matches!(
            outcomes[0].decision,
            PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::InvalidProof)
        ));
        assert_eq!(sink.archive_count(), 2, "archive receipt prevented replay");
        assert_eq!(sink.repair_count(), 2);
    }

    #[test]
    fn proof_outcome_forwarder_node_startup_resumes_pdp_handoff_exactly_once_and_fails_closed() {
        fn persist_archive_handoff_pending(config: &StorageConfig, epoch_id: u64) -> Fixture {
            let fixture = fixture(epoch_id);
            let state_dir = config.data_dir().join("pdp-provider");
            let protocol = PdpProviderProtocol::open(config.pdp_provider_policy(), &state_dir)
                .expect("open PDP protocol");
            enqueue(&protocol, &fixture);
            let sink = RecordingHandoff::failing(1, 0);
            assert!(matches!(
                protocol.submit_proof_for_challenge_bytes(
                    fixture.challenge.challenge_id,
                    &norito::to_bytes(&fixture.proof).expect("encode proof"),
                    &fixture.admission,
                    1_100,
                    &sink,
                ),
                Err(PdpProviderProtocolError::ArchiveHandoff(_))
            ));
            let status = protocol
                .challenge_status(&fixture.challenge.challenge_id)
                .expect("challenge status")
                .expect("retained challenge");
            assert_eq!(status.lifecycle, PdpChallengeLifecycleV1::HandoffPending);
            assert!(
                protocol
                    .terminal_outcome(&fixture.challenge.challenge_id)
                    .expect("terminal lookup")
                    .is_none(),
                "archive failure must not advance the durable terminal lifecycle"
            );
            drop(protocol);
            fixture
        }

        let happy_dir = TempDir::new().expect("happy-path tempdir");
        let happy_root = happy_dir.path().canonicalize().expect("canonical tempdir");
        let happy_config = StorageConfig::builder()
            .enabled(true)
            .data_dir(happy_root.join("storage"))
            .build();
        let happy_fixture = persist_archive_handoff_pending(&happy_config, 51);

        let first_restart =
            NodeHandle::try_new(happy_config.clone()).expect("startup resumes PDP handoff");
        let first_pending = first_restart
            .pending_proof_outcome_deliveries(8)
            .expect("pending proof outcomes");
        assert_eq!(first_pending.len(), 1);
        assert_eq!(
            first_pending[0].identity_digest,
            happy_fixture.challenge.challenge_id
        );
        assert!(
            first_restart
                .pdp_provider_protocol()
                .expect("durable PDP protocol")
                .terminal_outcome(&happy_fixture.challenge.challenge_id)
                .expect("terminal lookup")
                .is_some(),
            "the PDP terminal lifecycle advances only after the proof outcome is durable"
        );
        let operation_id = first_pending[0].operation_id;
        let outcome_digest = first_pending[0].outcome_digest;
        drop(first_restart);

        let second_restart =
            NodeHandle::try_new(happy_config).expect("second startup remains idempotent");
        let second_pending = second_restart
            .pending_proof_outcome_deliveries(8)
            .expect("pending proof outcomes after second restart");
        assert_eq!(
            second_pending.len(),
            1,
            "a terminal PDP handoff must enqueue one semantic operation"
        );
        assert_eq!(second_pending[0].operation_id, operation_id);
        assert_eq!(second_pending[0].outcome_digest, outcome_digest);
        drop(second_restart);

        let poisoned_dir = TempDir::new().expect("poisoned-path tempdir");
        let poisoned_root = poisoned_dir
            .path()
            .canonicalize()
            .expect("canonical tempdir");
        let poisoned_config = StorageConfig::builder()
            .enabled(true)
            .data_dir(poisoned_root.join("storage"))
            .build();
        let poisoned_fixture = persist_archive_handoff_pending(&poisoned_config, 52);
        let outbox_dir = poisoned_config.data_dir().join("proof-outcome-forwarder");
        fs::create_dir_all(&outbox_dir).expect("create outbox directory");
        fs::write(
            outbox_dir.join(PROOF_OUTCOME_OUTBOX_CHECKPOINT_FILE_NAME_V1),
            b"poisoned proof outcome checkpoint",
        )
        .expect("write poisoned outbox checkpoint");

        assert!(matches!(
            NodeHandle::try_new(poisoned_config.clone()),
            Err(NodeInitError::ProofOutcomeOutbox { .. })
        ));
        let restored = PdpProviderProtocol::open(
            poisoned_config.pdp_provider_policy(),
            &poisoned_config.data_dir().join("pdp-provider"),
        )
        .expect("reopen PDP protocol after failed node startup");
        let status = restored
            .challenge_status(&poisoned_fixture.challenge.challenge_id)
            .expect("challenge status")
            .expect("retained challenge");
        assert_eq!(status.lifecycle, PdpChallengeLifecycleV1::HandoffPending);
        assert!(
            restored
                .terminal_outcome(&poisoned_fixture.challenge.challenge_id)
                .expect("terminal lookup")
                .is_none(),
            "untrusted outbox durability must abort startup before terminal acknowledgement"
        );
    }

    #[test]
    fn pending_and_terminal_limits_fail_closed_until_safe_prune() {
        let mut policy = PdpProviderProtocolPolicyV1::default();
        policy.max_pending_records = 1;
        policy.max_terminal_records = 1;
        policy.terminal_retention_secs = policy.max_response_window_secs;
        let protocol = PdpProviderProtocol::in_memory(policy).unwrap();
        let first = fixture(60);
        let second = fixture(61);
        enqueue(&protocol, &first);
        assert!(matches!(
            protocol.enqueue_challenge(
                second.commitment.clone(),
                second.challenge.clone(),
                &second.admission,
                second.challenge.epoch_id,
                ISSUED_AT,
            ),
            Err(PdpProviderProtocolError::PendingRetentionExhausted { limit: 1 })
        ));
        let sink = RecordingHandoff::default();
        protocol
            .submit_proof_for_challenge_bytes(
                first.challenge.challenge_id,
                &norito::to_bytes(&first.proof).unwrap(),
                &first.admission,
                1_100,
                &sink,
            )
            .unwrap();
        enqueue(&protocol, &second);
        assert!(matches!(
            protocol.submit_proof_for_challenge_bytes(
                second.challenge.challenge_id,
                &norito::to_bytes(&second.proof).unwrap(),
                &second.admission,
                1_100,
                &sink,
            ),
            Err(PdpProviderProtocolError::TerminalRetentionExhausted { limit: 1 })
        ));
        assert_eq!(protocol.prune_terminal(1_200, 10).unwrap(), 0);
        assert_eq!(protocol.prune_terminal(1_701, 10).unwrap(), 1);
        let outcome = protocol
            .submit_proof_for_challenge_bytes(
                second.challenge.challenge_id,
                &norito::to_bytes(&second.proof).unwrap(),
                &second.admission,
                1_701,
                &sink,
            )
            .expect("submit after prune");
        assert!(matches!(
            outcome.decision,
            PdpTerminalDecisionV1::Rejected(PdpRejectionReasonV1::SubmissionLate)
        ));
    }

    #[test]
    fn checkpoint_rejects_corruption_symlinks_hardlinks_and_stale_writers() {
        let policy = PdpProviderProtocolPolicyV1::default();
        let dir = TempDir::new().expect("tempdir");
        let first_writer = PdpProviderProtocol::open(policy, dir.path()).expect("first writer");
        let stale_writer = PdpProviderProtocol::open(policy, dir.path()).expect("stale writer");
        let first = fixture(70);
        enqueue(&first_writer, &first);
        let second = fixture(71);
        assert!(matches!(
            stale_writer.enqueue_challenge(
                second.commitment.clone(),
                second.challenge.clone(),
                &second.admission,
                second.challenge.epoch_id,
                ISSUED_AT,
            ),
            Err(PdpProviderProtocolError::StaleCheckpoint)
        ));

        let checkpoint = dir.path().join(PDP_PROVIDER_CHECKPOINT_FILE_NAME_V1);
        let hardlink = dir.path().join("checkpoint-hardlink");
        fs::hard_link(&checkpoint, &hardlink).expect("hardlink checkpoint");
        assert!(PdpProviderProtocol::open(policy, dir.path()).is_err());
        fs::remove_file(&hardlink).expect("remove hardlink");
        fs::write(&checkpoint, [0xFF; 64]).expect("corrupt checkpoint");
        assert!(matches!(
            PdpProviderProtocol::open(policy, dir.path()),
            Err(PdpProviderProtocolError::InvalidCheckpoint(_))
                | Err(PdpProviderProtocolError::CheckpointIo(_))
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let target = TempDir::new().expect("target");
            let parent = TempDir::new().expect("parent");
            let linked_root = parent.path().join("linked-root");
            symlink(target.path(), &linked_root).expect("symlink root");
            assert!(PdpProviderProtocol::open(policy, &linked_root).is_err());

            let root = TempDir::new().expect("checkpoint root");
            let external = root.path().join("external");
            fs::write(&external, [1, 2, 3]).expect("external file");
            symlink(
                &external,
                root.path().join(PDP_PROVIDER_CHECKPOINT_FILE_NAME_V1),
            )
            .expect("symlink checkpoint");
            assert!(PdpProviderProtocol::open(policy, root.path()).is_err());
        }
    }
}
