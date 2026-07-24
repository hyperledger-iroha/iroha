//! Repair scheduler supporting SoraFS auditor workflows.
//!
//! The scheduler persists repair tickets via a repair store abstraction, tracks
//! proof-of-retrievability failures, and emits metrics so operators can monitor
//! SLA adherence. Repair state is stored in bounded, canonical Norito snapshots
//! using private atomic files, inter-process writer exclusion, stale-writer
//! detection, and fail-closed startup validation. Sequence allocation, replay
//! guards, and worker idempotency results share the same durable checkpoint.

use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    fs, io,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
#[cfg(unix)]
use std::os::unix::io::AsRawFd;
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt as _;

use blake3::hash;
use hex;
use iroha_logger::{debug, error, warn};
use iroha_telemetry::metrics::{global_or_default, global_sorafs_repair_otel};
use rand::{rand_core::TryRngCore as _, rngs::OsRng};
use sorafs_manifest::{
    deal::XorQuantity,
    por::AuditVerdictV1,
    repair::{
        CompletedRepairStateV1, EscalatedRepairStateV1, FailedRepairStateV1,
        InProgressRepairStateV1, QueuedRepairStateV1, REPAIR_TASK_EVENT_VERSION_V1,
        REPAIR_TASK_VERSION_V1, RepairCauseV1, RepairEvidenceV1, RepairReportV1,
        RepairSlashProposalV1, RepairTaskEventV1, RepairTaskRecordV1, RepairTaskStateV1,
        RepairTaskStatusV1, RepairTicketId, RepairValidationError,
    },
};
use thiserror::Error;

use crate::config::{RepairConfig, RepairEscalationPolicy};
use norito::derive::{NoritoDeserialize, NoritoSerialize};

const DEFAULT_REPAIR_SLA_SECS: u64 = 4 * 60 * 60;
const MAX_IDEMPOTENCY_KEY_BYTES: usize = 256;
const MAX_WORKER_ID_BYTES: usize = 256;
const MAX_REPAIR_NOTES_BYTES: usize = 256;
const MAX_GOVERNANCE_ACTOR_BYTES: usize = 256;
const MAX_GOVERNANCE_REASON_BYTES: usize = 256;
const DEFAULT_IDEMPOTENCY_CACHE_SIZE: usize = 64;
const MAX_REPAIR_STORE_RETRIES: usize = 3;
const DEFAULT_REPAIR_EVENT_HISTORY_LIMIT: usize = 64;
const DEFAULT_REPAIR_STORE_ENTRY_LIMIT: usize = 65_536;
const DEFAULT_REPAIR_STORE_MAX_BYTES: u64 = 64 * 1024 * 1024;
/// Maximum canonical byte length of the embedded v1 slash proposal.
///
/// Its variable fields are each capped at 256 bytes and the remaining fields
/// are fixed-width. Four KiB leaves ample codec overhead while preventing the
/// embedded `Vec<u8>` from turning the checkpoint byte cap into an allocation
/// limit.
const MAX_CANONICAL_REPAIR_SLASH_PROPOSAL_BYTES: usize = 16 * MAX_REPAIR_NOTES_BYTES;
const REPAIR_REPORT_SOURCE_IDENTITY_DOMAIN_V1: &[u8] = b"sorafs.repair.report-source-identity.v1";
const REPAIR_STORE_VERSION_V1: u8 = 1;
const REPAIR_STORE_FILE_NAME: &str = "repair_state.to";
const REPAIR_STORE_TMP_EXT: &str = "tmp";
const REPAIR_STORE_LOCK_EXT: &str = "lock";
const NORITO_COMPRESSION_OFFSET: usize = 4 + 1 + 1 + 16;
const NORITO_LENGTH_OFFSET: usize = NORITO_COMPRESSION_OFFSET + 1;
static REPAIR_STORE_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
static REPAIR_STORE_WRITE_LOCK: Mutex<()> = Mutex::new(());

fn repair_checkpoint_decode_limits(entry_limit: usize, max_bytes: u64) -> norito::DecodeLimits {
    // Checkpoint sequences are either entry-limited histories/indices or the
    // one fixed-bounded canonical proposal blob. The per-sequence bound stays
    // schema-derived; cumulative element, field, and allocation budgets are
    // additionally tied to the configured checkpoint byte ceiling.
    let fixed_field_limit = MAX_CANONICAL_REPAIR_SLASH_PROPOSAL_BYTES
        .max(DEFAULT_IDEMPOTENCY_CACHE_SIZE)
        .max(DEFAULT_REPAIR_EVENT_HISTORY_LIMIT);
    let byte_limit = usize::try_from(max_bytes).unwrap_or(usize::MAX);
    let allocation_limit = byte_limit.saturating_mul(4);
    norito::DecodeLimits::new(
        entry_limit.max(fixed_field_limit),
        byte_limit,
        byte_limit,
        allocation_limit,
        64,
    )
}

fn repair_slash_proposal_decode_limits() -> norito::DecodeLimits {
    norito::DecodeLimits::new(
        MAX_CANONICAL_REPAIR_SLASH_PROPOSAL_BYTES,
        MAX_CANONICAL_REPAIR_SLASH_PROPOSAL_BYTES,
        65_536,
        256 * 1024,
        32,
    )
}

#[cfg(unix)]
const LOCK_EXCLUSIVE_NONBLOCKING: std::os::raw::c_int = 2 | 4;

#[cfg(unix)]
unsafe extern "C" {
    fn flock(fd: std::os::raw::c_int, operation: std::os::raw::c_int) -> std::os::raw::c_int;
    fn geteuid() -> std::os::raw::c_uint;
}

/// Result of inserting a repair task into storage.
#[derive(Debug, Clone)]
enum RepairStoreInsertResult {
    Inserted(RepairTaskInternal),
    Existing(RepairTaskInternal),
}

/// Errors returned by the repair storage backend.
#[derive(Debug, Error)]
pub enum RepairStoreError {
    /// Ticket already exists.
    #[error("repair ticket `{ticket_id}` already exists")]
    Duplicate {
        /// Repair ticket identifier.
        ticket_id: String,
    },
    /// Ticket not found.
    #[error("repair ticket `{ticket_id}` not found")]
    NotFound {
        /// Repair ticket identifier.
        ticket_id: String,
    },
    /// Ticket was modified concurrently.
    #[error("repair ticket `{ticket_id}` modified concurrently")]
    Conflict {
        /// Repair ticket identifier.
        ticket_id: String,
    },
    /// On-disk checkpoint changed after this store instance loaded it.
    #[error("repair checkpoint changed concurrently; stale writer rejected")]
    StaleCheckpoint,
    /// The new snapshot became visible but its directory entry could not be
    /// durably synchronized, so the store stopped accepting operations.
    #[error("repair checkpoint commit durability is uncertain: {0}")]
    DurabilityUncertain(String),
    /// Store rejected the update.
    #[error("repair store error: {0}")]
    Other(String),
    /// Auditor nonce is not greater than the persisted nonce.
    #[error(
        "auditor `{auditor_account}` nonce replay rejected: nonce {nonce} is not greater than stored nonce {highest_nonce}"
    )]
    AuditorNonceReplay {
        /// Auditor account whose nonce was replayed.
        auditor_account: String,
        /// Submitted nonce.
        nonce: u64,
        /// Highest nonce already accepted for this auditor.
        highest_nonce: u64,
    },
}

/// Storage backend for repair tickets and PoR history.
trait RepairStore: std::fmt::Debug + Send + Sync {
    fn next_audit_sequence(&self) -> Result<u64, RepairStoreError>;
    fn append_por_history(
        &self,
        observation: PorHistoryObservation,
    ) -> Result<u64, RepairStoreError>;
    fn por_history_entry(
        &self,
        por_history_id: u64,
    ) -> Result<Option<PorHistoryEntry>, RepairStoreError>;
    fn insert_task(
        &self,
        task: RepairTaskInternal,
    ) -> Result<RepairStoreInsertResult, RepairStoreError>;
    fn task(
        &self,
        ticket_id: &RepairTicketId,
    ) -> Result<Option<RepairTaskInternal>, RepairStoreError>;
    fn compare_and_set_task(
        &self,
        ticket_id: &RepairTicketId,
        expected_revision: u64,
        task: RepairTaskInternal,
    ) -> Result<(), RepairStoreError>;
    fn list_tasks(&self) -> Result<Vec<RepairTaskInternal>, RepairStoreError>;
    fn record_auditor_nonce(
        &self,
        auditor_account: &str,
        nonce: u64,
    ) -> Result<(), RepairStoreError>;
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct RepairStoreSnapshot {
    version: u8,
    next_por_history_id: u64,
    next_audit_sequence: u64,
    tasks: Vec<StoredRepairTask>,
    por_history: Vec<PorHistoryEntry>,
    #[norito(default)]
    auditor_nonces: Vec<StoredAuditorNonce>,
}

impl RepairStoreSnapshot {
    fn from_state(state: &RepairStoreState) -> Result<Self, RepairStoreError> {
        Ok(Self {
            version: REPAIR_STORE_VERSION_V1,
            next_por_history_id: state.next_por_history_id,
            next_audit_sequence: state.next_audit_sequence,
            tasks: state
                .tasks
                .values()
                .cloned()
                .map(StoredRepairTask::from_internal)
                .collect::<Result<Vec<_>, _>>()?,
            por_history: state.por_history.values().cloned().collect(),
            auditor_nonces: state
                .auditor_nonces
                .iter()
                .map(|(auditor_account, highest_nonce)| StoredAuditorNonce {
                    auditor_account: auditor_account.clone(),
                    highest_nonce: *highest_nonce,
                })
                .collect(),
        })
    }

    fn into_state(
        self,
        entry_limit: usize,
        checkpoint_digest: [u8; 32],
        escalation_policy: RepairEscalationPolicy,
    ) -> Result<RepairStoreState, RepairStoreError> {
        if self.version != REPAIR_STORE_VERSION_V1 {
            return Err(RepairStoreError::Other(format!(
                "unsupported repair store version {} (expected {})",
                self.version, REPAIR_STORE_VERSION_V1
            )));
        }
        if self.tasks.len() > entry_limit
            || self.por_history.len() > entry_limit
            || self.auditor_nonces.len() > entry_limit
        {
            return Err(RepairStoreError::Other(format!(
                "repair store checkpoint exceeds entry limit {entry_limit}"
            )));
        }
        ensure_strictly_sorted_by(
            &self.tasks,
            |left, right| left.report.ticket_id.0.cmp(&right.report.ticket_id.0),
            "tasks",
        )?;
        ensure_strictly_sorted_by(
            &self.por_history,
            |left, right| left.id.cmp(&right.id),
            "PoR history",
        )?;
        ensure_strictly_sorted_by(
            &self.auditor_nonces,
            |left, right| left.auditor_account.cmp(&right.auditor_account),
            "auditor nonces",
        )?;

        let mut tasks = BTreeMap::new();
        let mut source_identities = BTreeSet::new();
        for task in self.tasks {
            let internal = task.into_internal(entry_limit, &escalation_policy)?;
            if internal.events.len() > entry_limit {
                return Err(RepairStoreError::Other(format!(
                    "repair task event history exceeds entry limit {entry_limit}"
                )));
            }
            let key = internal.report.ticket_id.0.clone();
            if !source_identities.insert(internal.source_identity) {
                return Err(RepairStoreError::Other(format!(
                    "duplicate repair source identity in checkpoint for task `{key}`"
                )));
            }
            if tasks.insert(key.clone(), internal).is_some() {
                return Err(RepairStoreError::Other(format!(
                    "duplicate repair task `{key}` in checkpoint"
                )));
            }
        }
        let expected_next_por_history_id = u64::try_from(self.por_history.len())
            .ok()
            .and_then(|length| length.checked_add(1))
            .ok_or_else(|| {
                RepairStoreError::Other("repair PoR history length overflow".to_owned())
            })?;
        if self.next_por_history_id != expected_next_por_history_id {
            return Err(RepairStoreError::Other(format!(
                "repair store PoR sequence high-water must equal {expected_next_por_history_id}"
            )));
        }
        let mut por_history = BTreeMap::new();
        let mut por_challenges = HashMap::new();
        for (index, entry) in self.por_history.into_iter().enumerate() {
            entry.validate_persisted()?;
            let expected_id = u64::try_from(index)
                .ok()
                .and_then(|index| index.checked_add(1))
                .ok_or_else(|| {
                    RepairStoreError::Other("repair PoR history index overflow".to_owned())
                })?;
            if entry.id != expected_id {
                return Err(RepairStoreError::Other(format!(
                    "repair store PoR history id {} is not contiguous (expected {expected_id})",
                    entry.id
                )));
            }
            if por_challenges
                .insert(entry.challenge_id, entry.id)
                .is_some()
            {
                return Err(RepairStoreError::Other(
                    "duplicate PoR challenge in repair checkpoint".to_owned(),
                ));
            }
            if por_history.insert(entry.id, entry).is_some() {
                return Err(RepairStoreError::Other(
                    "duplicate PoR history id in repair checkpoint".to_owned(),
                ));
            }
        }
        for task in tasks.values() {
            validate_persisted_por_history_binding(&task.report, &por_history)?;
        }
        if self.next_audit_sequence == 0 {
            return Err(RepairStoreError::Other(
                "repair store sequence high-water marks are invalid".to_owned(),
            ));
        }
        let mut auditor_nonces = BTreeMap::new();
        for nonce in self.auditor_nonces {
            if nonce.auditor_account.is_empty()
                || nonce.auditor_account.len() > MAX_GOVERNANCE_ACTOR_BYTES
                || nonce.highest_nonce == 0
            {
                return Err(RepairStoreError::Other(
                    "repair store auditor nonce entry is invalid".to_owned(),
                ));
            }
            if auditor_nonces
                .insert(nonce.auditor_account.clone(), nonce.highest_nonce)
                .is_some()
            {
                return Err(RepairStoreError::Other(format!(
                    "duplicate repair store auditor nonce entry for `{}`",
                    nonce.auditor_account
                )));
            }
        }
        Ok(RepairStoreState {
            tasks,
            por_history,
            auditor_nonces,
            next_por_history_id: self.next_por_history_id,
            next_audit_sequence: self.next_audit_sequence,
            checkpoint_digest: Some(checkpoint_digest),
            healthy: true,
        })
    }
}

fn ensure_strictly_sorted_by<T>(
    values: &[T],
    compare: impl Fn(&T, &T) -> std::cmp::Ordering,
    field: &str,
) -> Result<(), RepairStoreError> {
    if values
        .windows(2)
        .any(|window| compare(&window[0], &window[1]) != std::cmp::Ordering::Less)
    {
        return Err(RepairStoreError::Other(format!(
            "repair store {field} must be strictly sorted and unique"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct StoredAuditorNonce {
    auditor_account: String,
    highest_nonce: u64,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct StoredRepairTask {
    source_identity: [u8; 32],
    revision: u64,
    report: RepairReportV1,
    state: RepairTaskStateV1,
    sla_deadline_unix: Option<u64>,
    scheduler_notes: Option<String>,
    slash_proposal_digest: Option<[u8; 32]>,
    slash_proposal_bytes: Option<Vec<u8>>,
    slash_proposal_stage: Option<RepairSlashProposalStage>,
    #[norito(default)]
    governance: RepairGovernanceState,
    governance_policy: Option<RepairGovernancePolicySnapshot>,
    lease: Option<StoredRepairTaskLease>,
    idempotency: StoredRepairTaskIdempotency,
    attempts: u32,
    next_attempt_after_unix: Option<u64>,
    events_dropped: u64,
    events: Vec<RepairTaskEventV1>,
}

impl StoredRepairTask {
    fn from_internal(task: RepairTaskInternal) -> Result<Self, RepairStoreError> {
        let idempotency = StoredRepairTaskIdempotency::from_runtime(&task.idempotency)?;
        Ok(Self {
            source_identity: task.source_identity,
            revision: task.revision,
            report: task.report,
            state: task.state,
            sla_deadline_unix: task.sla_deadline_unix,
            scheduler_notes: task.scheduler_notes,
            slash_proposal_digest: task.slash_proposal_digest,
            slash_proposal_bytes: task.slash_proposal_bytes,
            slash_proposal_stage: task.slash_proposal_stage,
            governance: task.governance,
            governance_policy: task.governance_policy,
            lease: task.lease.map(StoredRepairTaskLease::from_lease),
            idempotency,
            attempts: task.attempts,
            next_attempt_after_unix: task.next_attempt_after_unix,
            events_dropped: task.events_dropped,
            events: task.events,
        })
    }

    fn into_internal(
        self,
        entry_limit: usize,
        escalation_policy: &RepairEscalationPolicy,
    ) -> Result<RepairTaskInternal, RepairStoreError> {
        let idempotency = self
            .idempotency
            .into_runtime(&self.report, DEFAULT_IDEMPOTENCY_CACHE_SIZE)?;
        let internal = RepairTaskInternal {
            source_identity: self.source_identity,
            revision: self.revision,
            report: self.report,
            state: self.state,
            sla_deadline_unix: self.sla_deadline_unix,
            scheduler_notes: self.scheduler_notes,
            slash_proposal_digest: self.slash_proposal_digest,
            slash_proposal_bytes: self.slash_proposal_bytes,
            slash_proposal_stage: self.slash_proposal_stage,
            governance: self.governance,
            governance_policy: self.governance_policy,
            lease: self.lease.map(StoredRepairTaskLease::into_lease),
            idempotency,
            attempts: self.attempts,
            next_attempt_after_unix: self.next_attempt_after_unix,
            events_dropped: self.events_dropped,
            events: self.events,
        };
        internal.validate_persisted(entry_limit, escalation_policy)?;
        Ok(internal)
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct StoredRepairTaskLease {
    worker_id: String,
    last_heartbeat_unix: u64,
    expires_at_unix: u64,
}

impl StoredRepairTaskLease {
    fn from_lease(lease: RepairTaskLease) -> Self {
        Self {
            worker_id: lease.worker_id,
            last_heartbeat_unix: lease.last_heartbeat_unix,
            expires_at_unix: lease.expires_at_unix,
        }
    }

    fn into_lease(self) -> RepairTaskLease {
        RepairTaskLease {
            worker_id: self.worker_id,
            last_heartbeat_unix: self.last_heartbeat_unix,
            expires_at_unix: self.expires_at_unix,
        }
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct StoredRepairTaskIdempotency {
    claim: Vec<StoredRepairClaimIdempotency>,
    heartbeat: Vec<StoredRepairHeartbeatIdempotency>,
    complete: Vec<StoredRepairCompleteIdempotency>,
    fail: Vec<StoredRepairFailIdempotency>,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct StoredRepairClaimIdempotency {
    key: String,
    signature: RepairClaimSignature,
    record: RepairTaskRecordV1,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct StoredRepairHeartbeatIdempotency {
    key: String,
    signature: RepairHeartbeatSignature,
    record: RepairTaskRecordV1,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct StoredRepairCompleteIdempotency {
    key: String,
    signature: RepairCompleteSignature,
    record: RepairTaskRecordV1,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct StoredRepairFailIdempotency {
    key: String,
    signature: RepairFailSignature,
    record: RepairTaskRecordV1,
}

impl StoredRepairTaskIdempotency {
    fn from_runtime(idempotency: &RepairTaskIdempotency) -> Result<Self, RepairStoreError> {
        Ok(Self {
            claim: ordered_idempotency_entries(&idempotency.claim, |key, entry| {
                StoredRepairClaimIdempotency {
                    key,
                    signature: entry.signature.clone(),
                    record: entry.record.clone(),
                }
            })?,
            heartbeat: ordered_idempotency_entries(&idempotency.heartbeat, |key, entry| {
                StoredRepairHeartbeatIdempotency {
                    key,
                    signature: entry.signature.clone(),
                    record: entry.record.clone(),
                }
            })?,
            complete: ordered_idempotency_entries(&idempotency.complete, |key, entry| {
                StoredRepairCompleteIdempotency {
                    key,
                    signature: entry.signature.clone(),
                    record: entry.record.clone(),
                }
            })?,
            fail: ordered_idempotency_entries(&idempotency.fail, |key, entry| {
                StoredRepairFailIdempotency {
                    key,
                    signature: entry.signature.clone(),
                    record: entry.record.clone(),
                }
            })?,
        })
    }

    fn into_runtime(
        self,
        report: &RepairReportV1,
        capacity: usize,
    ) -> Result<RepairTaskIdempotency, RepairStoreError> {
        validate_stored_idempotency_lengths(
            [
                self.claim.len(),
                self.heartbeat.len(),
                self.complete.len(),
                self.fail.len(),
            ],
            capacity,
        )?;
        let mut runtime = RepairTaskIdempotency::new(capacity);
        let mut keys = HashSet::new();
        for entry in self.claim {
            validate_stored_idempotency_key(&entry.key, &mut keys)?;
            validate_idempotency_record(&entry.record, report)?;
            validate_claim_signature(&entry.signature, &entry.record)?;
            runtime
                .claim
                .remember(&entry.key, entry.signature, entry.record);
        }
        keys.clear();
        for entry in self.heartbeat {
            validate_stored_idempotency_key(&entry.key, &mut keys)?;
            validate_idempotency_record(&entry.record, report)?;
            validate_heartbeat_signature(&entry.signature, &entry.record)?;
            runtime
                .heartbeat
                .remember(&entry.key, entry.signature, entry.record);
        }
        keys.clear();
        for entry in self.complete {
            validate_stored_idempotency_key(&entry.key, &mut keys)?;
            validate_idempotency_record(&entry.record, report)?;
            validate_complete_signature(&entry.signature, &entry.record)?;
            runtime
                .complete
                .remember(&entry.key, entry.signature, entry.record);
        }
        keys.clear();
        for entry in self.fail {
            validate_stored_idempotency_key(&entry.key, &mut keys)?;
            validate_idempotency_record(&entry.record, report)?;
            validate_fail_signature(&entry.signature, &entry.record)?;
            runtime
                .fail
                .remember(&entry.key, entry.signature, entry.record);
        }
        Ok(runtime)
    }
}

fn ordered_idempotency_entries<S, T>(
    cache: &IdempotencyCache<S>,
    convert: impl Fn(String, &IdempotencyEntry<S>) -> T,
) -> Result<Vec<T>, RepairStoreError> {
    if cache.entries.len() != cache.order.len() || cache.entries.len() > cache.capacity {
        return Err(RepairStoreError::Other(
            "repair idempotency cache structure is inconsistent".to_owned(),
        ));
    }
    let mut seen = HashSet::with_capacity(cache.order.len());
    cache
        .order
        .iter()
        .map(|key| {
            if !seen.insert(key.as_str()) {
                return Err(RepairStoreError::Other(
                    "repair idempotency cache contains a duplicate key".to_owned(),
                ));
            }
            let entry = cache.entries.get(key).ok_or_else(|| {
                RepairStoreError::Other(
                    "repair idempotency cache order references a missing entry".to_owned(),
                )
            })?;
            Ok(convert(key.clone(), entry))
        })
        .collect()
}

fn validate_stored_idempotency_lengths(
    lengths: [usize; 4],
    capacity: usize,
) -> Result<(), RepairStoreError> {
    if lengths.into_iter().any(|length| length > capacity) {
        return Err(RepairStoreError::Other(format!(
            "repair idempotency history exceeds per-action limit {capacity}"
        )));
    }
    Ok(())
}

fn validate_stored_idempotency_key(
    key: &str,
    seen: &mut HashSet<String>,
) -> Result<(), RepairStoreError> {
    if key.trim().is_empty()
        || key.len() > MAX_IDEMPOTENCY_KEY_BYTES
        || !seen.insert(key.to_owned())
    {
        return Err(RepairStoreError::Other(
            "repair idempotency history contains an invalid or duplicate key".to_owned(),
        ));
    }
    Ok(())
}

fn validate_idempotency_record(
    record: &RepairTaskRecordV1,
    report: &RepairReportV1,
) -> Result<(), RepairStoreError> {
    record.validate().map_err(|err| {
        RepairStoreError::Other(format!("invalid repair idempotency result record: {err}"))
    })?;
    if record.ticket_id != report.ticket_id
        || record.manifest_digest != report.evidence.manifest_digest
        || record.provider_id != report.evidence.provider_id
        || record.auditor_account != report.auditor_account
        || record.por_history_id != report.evidence.por_history_id
    {
        return Err(RepairStoreError::Other(
            "repair idempotency result record identity mismatch".to_owned(),
        ));
    }
    Ok(())
}

fn valid_worker_identity(worker_id: &str) -> bool {
    !worker_id.trim().is_empty() && worker_id.len() <= MAX_WORKER_ID_BYTES
}

fn validate_claim_signature(
    signature: &RepairClaimSignature,
    record: &RepairTaskRecordV1,
) -> Result<(), RepairStoreError> {
    let valid = valid_worker_identity(&signature.worker_id)
        && signature.claimed_at_unix != 0
        && matches!(
            &record.state,
            RepairTaskStateV1::InProgress(state)
                if state.started_at_unix == signature.claimed_at_unix
                    && state.repair_agent.as_deref() == Some(signature.worker_id.as_str())
        );
    if !valid {
        return Err(RepairStoreError::Other(
            "repair claim idempotency signature/result mismatch".to_owned(),
        ));
    }
    Ok(())
}

fn validate_heartbeat_signature(
    signature: &RepairHeartbeatSignature,
    record: &RepairTaskRecordV1,
) -> Result<(), RepairStoreError> {
    let valid = valid_worker_identity(&signature.worker_id)
        && signature.heartbeat_at_unix != 0
        && matches!(
            &record.state,
            RepairTaskStateV1::InProgress(state)
                if signature.heartbeat_at_unix > state.started_at_unix
                    && state.repair_agent.as_deref() == Some(signature.worker_id.as_str())
        );
    if !valid {
        return Err(RepairStoreError::Other(
            "repair heartbeat idempotency signature/result mismatch".to_owned(),
        ));
    }
    Ok(())
}

fn validate_complete_signature(
    signature: &RepairCompleteSignature,
    record: &RepairTaskRecordV1,
) -> Result<(), RepairStoreError> {
    let valid = valid_worker_identity(&signature.worker_id)
        && signature.completed_at_unix != 0
        && signature
            .resolution_notes
            .as_ref()
            .is_none_or(|notes| !notes.trim().is_empty() && notes.len() <= MAX_REPAIR_NOTES_BYTES)
        && matches!(
            &record.state,
            RepairTaskStateV1::Completed(state)
                if state.completed_at_unix == signature.completed_at_unix
                    && state.resolution_notes == signature.resolution_notes
        );
    if !valid {
        return Err(RepairStoreError::Other(
            "repair completion idempotency signature/result mismatch".to_owned(),
        ));
    }
    Ok(())
}

fn validate_fail_signature(
    signature: &RepairFailSignature,
    record: &RepairTaskRecordV1,
) -> Result<(), RepairStoreError> {
    let state_matches = match &record.state {
        RepairTaskStateV1::Failed(state) => {
            state.failed_at_unix == signature.failed_at_unix && state.reason == signature.reason
        }
        RepairTaskStateV1::Escalated(state) => state.escalated_at_unix == signature.failed_at_unix,
        _ => false,
    };
    if !valid_worker_identity(&signature.worker_id)
        || signature.failed_at_unix == 0
        || signature.reason.trim().is_empty()
        || signature.reason.len() > MAX_REPAIR_NOTES_BYTES
        || !state_matches
    {
        return Err(RepairStoreError::Other(
            "repair failure idempotency signature/result mismatch".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct RepairStoreState {
    tasks: BTreeMap<String, RepairTaskInternal>,
    por_history: BTreeMap<u64, PorHistoryEntry>,
    auditor_nonces: BTreeMap<String, u64>,
    next_por_history_id: u64,
    next_audit_sequence: u64,
    checkpoint_digest: Option<[u8; 32]>,
    healthy: bool,
}

impl RepairStoreState {
    fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
            por_history: BTreeMap::new(),
            auditor_nonces: BTreeMap::new(),
            next_por_history_id: 1,
            next_audit_sequence: 1,
            checkpoint_digest: None,
            healthy: true,
        }
    }
}

#[derive(Debug)]
struct FileRepairStore {
    path: PathBuf,
    /// Held for the complete store lifetime so a second process or manager
    /// cannot load a snapshot and indefinitely serve stale in-memory reads.
    _checkpoint_lock: fs::File,
    state: RwLock<RepairStoreState>,
    entry_limit: usize,
    max_bytes: u64,
    escalation_policy: RepairEscalationPolicy,
    parent_sync: fn(&Path) -> io::Result<()>,
}

impl FileRepairStore {
    #[cfg(test)]
    fn load_or_new(
        path: PathBuf,
        entry_limit: usize,
        max_bytes: u64,
    ) -> Result<Self, RepairStoreError> {
        Self::load_or_new_with_policy(
            path,
            entry_limit,
            max_bytes,
            RepairEscalationPolicy::default(),
        )
    }

    fn load_or_new_with_policy(
        path: PathBuf,
        entry_limit: usize,
        max_bytes: u64,
        escalation_policy: RepairEscalationPolicy,
    ) -> Result<Self, RepairStoreError> {
        let entry_limit = entry_limit.max(1);
        let max_bytes = max_bytes.max(1);
        let path = absolute_secure_store_path(&path)
            .map_err(|err| RepairStoreError::Other(format!("invalid repair store path: {err}")))?;
        ensure_secure_store_parent(&path)
            .map_err(|err| RepairStoreError::Other(format!("invalid repair store path: {err}")))?;
        let had_lock_marker = checkpoint_lock_marker_exists(&path)?;
        let checkpoint_lock = acquire_checkpoint_write_lock(&path)?;
        let checkpoint_bytes = read_repair_store_bounded(&path, max_bytes)?;
        let initialize_checkpoint = checkpoint_bytes.is_none();
        if initialize_checkpoint && had_lock_marker {
            return Err(RepairStoreError::Other(format!(
                "repair checkpoint `{}` is missing after prior initialization",
                path.display()
            )));
        }
        let state = if let Some(bytes) = checkpoint_bytes {
            validate_bounded_uncompressed_norito(&bytes, max_bytes, "repair store checkpoint")?;
            let snapshot: RepairStoreSnapshot = norito::decode_from_bytes_with_limits(
                &bytes,
                repair_checkpoint_decode_limits(entry_limit, max_bytes),
            )
            .map_err(|err| {
                RepairStoreError::Other(format!("failed to decode repair store: {err}"))
            })?;
            let canonical = norito::to_bytes(&snapshot).map_err(|err| {
                RepairStoreError::Other(format!("failed to re-encode repair store: {err}"))
            })?;
            if canonical != bytes {
                return Err(RepairStoreError::Other(
                    "repair store checkpoint is not canonically encoded".to_owned(),
                ));
            }
            snapshot.into_state(
                entry_limit,
                checkpoint_digest(&bytes),
                escalation_policy.clone(),
            )?
        } else {
            RepairStoreState::new()
        };
        let store = Self {
            path,
            _checkpoint_lock: checkpoint_lock,
            state: RwLock::new(state),
            entry_limit,
            max_bytes,
            escalation_policy,
            parent_sync: sync_parent_directory,
        };
        if initialize_checkpoint {
            let mut state = store.state.write().map_err(|_| repair_store_poisoned())?;
            store.persist(&mut state)?;
        }
        Ok(store)
    }

    fn persist(&self, state: &mut RepairStoreState) -> Result<(), RepairStoreError> {
        ensure_repair_state_healthy(state)?;
        let snapshot = RepairStoreSnapshot::from_state(state)?;
        let bytes = norito::to_bytes(&snapshot).map_err(|err| {
            RepairStoreError::Other(format!("failed to encode repair store: {err}"))
        })?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > self.max_bytes {
            return Err(RepairStoreError::Other(format!(
                "encoded repair store is {} bytes, exceeding limit {}",
                bytes.len(),
                self.max_bytes
            )));
        }
        let _write_guard = REPAIR_STORE_WRITE_LOCK.lock().map_err(|_| {
            RepairStoreError::Other("repair store process write lock poisoned".to_owned())
        })?;
        let current_digest = read_repair_store_bounded(&self.path, self.max_bytes)?
            .as_deref()
            .map(checkpoint_digest);
        if current_digest != state.checkpoint_digest {
            state.healthy = false;
            return Err(RepairStoreError::StaleCheckpoint);
        }
        if let Err(err) = write_atomic(&self.path, &bytes, self.parent_sync) {
            if err.committed {
                state.checkpoint_digest = Some(checkpoint_digest(&bytes));
                state.healthy = false;
                return Err(RepairStoreError::DurabilityUncertain(err.error.to_string()));
            }
            return Err(RepairStoreError::Other(format!(
                "failed to persist repair store: {}",
                err.error
            )));
        }
        state.checkpoint_digest = Some(checkpoint_digest(&bytes));
        Ok(())
    }
}

fn ensure_repair_state_healthy(state: &RepairStoreState) -> Result<(), RepairStoreError> {
    if !state.healthy {
        return Err(RepairStoreError::Other(
            "repair store is unavailable after a checkpoint consistency failure; restart required"
                .to_owned(),
        ));
    }
    Ok(())
}

fn checkpoint_digest(bytes: &[u8]) -> [u8; 32] {
    *hash(bytes).as_bytes()
}

fn validate_bounded_uncompressed_norito(
    bytes: &[u8],
    max_uncompressed_bytes: u64,
    payload: &str,
) -> Result<(), RepairStoreError> {
    if bytes.len() < norito::core::Header::SIZE {
        return Err(RepairStoreError::Other(format!(
            "{payload} is truncated before its Norito header"
        )));
    }
    let compression = bytes[NORITO_COMPRESSION_OFFSET];
    if compression != norito::Compression::None as u8 {
        return Err(RepairStoreError::Other(format!(
            "{payload} must use uncompressed canonical Norito encoding"
        )));
    }
    let length_end = NORITO_LENGTH_OFFSET + std::mem::size_of::<u64>();
    let encoded_length: [u8; 8] = bytes[NORITO_LENGTH_OFFSET..length_end]
        .try_into()
        .map_err(|_| RepairStoreError::Other(format!("{payload} has an invalid Norito header")))?;
    let advertised_length = u64::from_le_bytes(encoded_length);
    if advertised_length > max_uncompressed_bytes {
        return Err(RepairStoreError::Other(format!(
            "{payload} advertises {advertised_length} uncompressed bytes, exceeding limit {max_uncompressed_bytes}"
        )));
    }
    Ok(())
}

fn acquire_checkpoint_write_lock(path: &Path) -> Result<fs::File, RepairStoreError> {
    ensure_secure_store_parent(path)
        .map_err(|err| RepairStoreError::Other(format!("invalid repair store lock path: {err}")))?;
    let lock_path = path.with_added_extension(REPAIR_STORE_LOCK_EXT);
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    options.mode(0o600);
    #[cfg(windows)]
    options.share_mode(0);
    set_no_follow_flag(&mut options);
    let file = options.open(&lock_path).map_err(|err| {
        RepairStoreError::Other(format!(
            "failed to acquire repair checkpoint lock `{}`: {err}",
            lock_path.display()
        ))
    })?;
    let metadata = file.metadata().map_err(|err| {
        RepairStoreError::Other(format!(
            "failed to inspect repair checkpoint lock `{}`: {err}",
            lock_path.display()
        ))
    })?;
    validate_checkpoint_file_metadata(&lock_path, &metadata)
        .map_err(|err| RepairStoreError::Other(err.to_string()))?;

    #[cfg(unix)]
    {
        // SAFETY: `file` owns a valid live descriptor for the duration of the
        // call, and `flock` neither takes ownership nor dereferences pointers.
        let result = unsafe { flock(file.as_raw_fd(), LOCK_EXCLUSIVE_NONBLOCKING) };
        if result != 0 {
            return Err(RepairStoreError::Other(format!(
                "repair checkpoint `{}` is locked by another writer: {}",
                path.display(),
                io::Error::last_os_error()
            )));
        }
    }
    Ok(file)
}

fn checkpoint_lock_marker_exists(path: &Path) -> Result<bool, RepairStoreError> {
    let lock_path = path.with_added_extension(REPAIR_STORE_LOCK_EXT);
    match fs::symlink_metadata(&lock_path) {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(RepairStoreError::Other(format!(
            "failed to inspect repair checkpoint lock marker `{}`: {err}",
            lock_path.display()
        ))),
    }
}

fn read_repair_store_bounded(
    path: &Path,
    max_bytes: u64,
) -> Result<Option<Vec<u8>>, RepairStoreError> {
    ensure_secure_store_parent(path)
        .map_err(|err| RepairStoreError::Other(format!("invalid repair store path: {err}")))?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(RepairStoreError::Other(format!(
                "failed to inspect repair store: {err}"
            )));
        }
    };
    validate_checkpoint_file_metadata(path, &metadata)
        .map_err(|err| RepairStoreError::Other(err.to_string()))?;
    if metadata.len() > max_bytes {
        return Err(RepairStoreError::Other(format!(
            "repair store is {} bytes, exceeding limit {max_bytes}",
            metadata.len()
        )));
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    set_no_follow_flag(&mut options);
    let file = options
        .open(path)
        .map_err(|err| RepairStoreError::Other(format!("failed to open repair store: {err}")))?;
    let opened = file.metadata().map_err(|err| {
        RepairStoreError::Other(format!("failed to inspect opened repair store: {err}"))
    })?;
    validate_checkpoint_file_metadata(path, &opened)
        .map_err(|err| RepairStoreError::Other(err.to_string()))?;
    if !same_file_identity(&metadata, &opened) || opened.len() > max_bytes {
        return Err(RepairStoreError::Other(
            "repair store changed identity or size while opening".to_owned(),
        ));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).map_err(|_| {
        RepairStoreError::Other("repair store length does not fit usize".to_owned())
    })?);
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|err| RepairStoreError::Other(format!("failed to read repair store: {err}")))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(RepairStoreError::Other(
            "repair store grew beyond its size limit while reading".to_owned(),
        ));
    }
    Ok(Some(bytes))
}

#[derive(Debug)]
struct AtomicWriteError {
    error: io::Error,
    committed: bool,
}

impl std::fmt::Display for AtomicWriteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for AtomicWriteError {}

fn write_atomic(
    path: &Path,
    data: &[u8],
    parent_sync: fn(&Path) -> io::Result<()>,
) -> Result<(), AtomicWriteError> {
    let before_commit = |error| AtomicWriteError {
        error,
        committed: false,
    };
    let after_commit = |error| AtomicWriteError {
        error,
        committed: true,
    };
    let parent = path
        .parent()
        .ok_or_else(|| before_commit(io::Error::other("missing parent directory")))?;
    ensure_secure_store_parent(path).map_err(before_commit)?;
    validate_atomic_output_path(path).map_err(before_commit)?;
    let counter = REPAIR_STORE_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut nonce = [0_u8; 16];
    OsRng.try_fill_bytes(&mut nonce).map_err(|err| {
        before_commit(io::Error::other(format!(
            "failed to generate repair store temporary-file nonce: {err}"
        )))
    })?;
    let nonce = u128::from_le_bytes(nonce);
    let tmp_path = temp_path_for_atomic(path, std::process::id(), counter, nonce);

    let write_result = (|| -> Result<(), AtomicWriteError> {
        let mut file = open_atomic_temp_file(&tmp_path).map_err(before_commit)?;
        file.write_all(data).map_err(before_commit)?;
        file.sync_all().map_err(before_commit)?;
        drop(file);
        validate_atomic_output_path(path).map_err(before_commit)?;
        fs::rename(&tmp_path, path).map_err(before_commit)?;
        validate_atomic_output_path(path).map_err(after_commit)?;
        parent_sync(parent).map_err(after_commit)?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    write_result
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) -> io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    set_no_follow_flag(&mut options);
    let directory = options.open(parent).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "repair store rename committed but parent directory `{}` could not be opened for sync: {err}",
                parent.display()
            ),
        )
    })?;
    if !directory.metadata()?.is_dir() {
        return Err(io::Error::other(format!(
            "repair store parent `{}` changed identity before sync",
            parent.display()
        )));
    }
    directory.sync_all().map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "repair store rename committed but parent directory `{}` could not be synced: {err}",
                parent.display()
            ),
        )
    })
}

#[cfg(not(unix))]
fn sync_parent_directory(parent: &Path) -> io::Result<()> {
    let metadata = fs::metadata(parent)?;
    if !metadata.is_dir() {
        return Err(io::Error::other(format!(
            "repair store parent `{}` changed identity after rename",
            parent.display()
        )));
    }
    Ok(())
}

fn temp_path_for_atomic(path: &Path, pid: u32, counter: u64, nonce: u128) -> PathBuf {
    let suffix = format!("{REPAIR_STORE_TMP_EXT}-{pid}-{counter}-{nonce:032x}");
    let candidate = path.with_added_extension(&suffix);
    match candidate.file_name().and_then(|name| name.to_str()) {
        Some(name) => candidate.with_file_name(format!(".{name}")),
        None => candidate,
    }
}

fn open_atomic_temp_file(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
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
    validate_checkpoint_file_metadata(path, &metadata)?;
    Ok(file)
}

fn validate_atomic_output_path(path: &Path) -> io::Result<()> {
    validate_store_path_shape(path)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            validate_checkpoint_file_metadata(path, &metadata)?;
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(io::Error::new(
                err.kind(),
                format!("failed to inspect output `{}`: {err}", path.display()),
            ));
        }
    }
    Ok(())
}

fn ensure_secure_store_parent(path: &Path) -> io::Result<()> {
    validate_store_path_shape(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("repair store path has no parent"))?;
    let mut current = PathBuf::new();
    for component in parent.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                validate_parent_component(&current, &metadata)?;
                validate_ancestor_permissions(&current, &metadata)?;
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                create_private_directory(&current)?;
                let metadata = fs::symlink_metadata(&current)?;
                validate_parent_component(&current, &metadata)?;
                validate_ancestor_permissions(&current, &metadata)?;
            }
            Err(err) => {
                return Err(io::Error::new(
                    err.kind(),
                    format!(
                        "failed to inspect repair store parent `{}`: {err}",
                        current.display()
                    ),
                ));
            }
        }
    }
    let metadata = fs::symlink_metadata(parent)?;
    #[cfg(unix)]
    {
        if metadata.uid() != effective_user_id() {
            return Err(io::Error::other(format!(
                "repair store parent `{}` is not owned by the effective user",
                parent.display()
            )));
        }
        if metadata.permissions().mode() & 0o022 != 0 {
            return Err(io::Error::other(format!(
                "repair store parent `{}` must not be group- or world-writable",
                parent.display()
            )));
        }
    }
    Ok(())
}

fn validate_store_path_shape(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(io::Error::other(format!(
            "repair store path `{}` must be absolute",
            path.display()
        )));
    }
    if path.file_name().is_none() {
        return Err(io::Error::other("repair store path must name a file"));
    }
    for component in path.components() {
        if matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        ) {
            return Err(io::Error::other(format!(
                "repair store path `{}` must not contain `.` or `..` components",
                path.display()
            )));
        }
    }
    Ok(())
}

fn absolute_secure_store_path(path: &Path) -> io::Result<PathBuf> {
    if path.file_name().is_none() {
        return Err(io::Error::other("repair store path must name a file"));
    }
    let mut normalized = if path.is_absolute() {
        PathBuf::new()
    } else {
        std::env::current_dir().map_err(|err| {
            io::Error::new(
                err.kind(),
                format!("failed to resolve repair store working directory: {err}"),
            )
        })?
    };
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                return Err(io::Error::other(format!(
                    "repair store path `{}` must not contain `..` components",
                    path.display()
                )));
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    validate_store_path_shape(&normalized)?;
    Ok(normalized)
}

fn validate_parent_component(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    if metadata.file_type().is_symlink() {
        return Err(io::Error::other(format!(
            "repair store parent `{}` must not be a symlink",
            path.display()
        )));
    }
    if !metadata.is_dir() {
        return Err(io::Error::other(format!(
            "repair store parent `{}` must be a directory",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn validate_ancestor_permissions(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    let mode = metadata.permissions().mode();
    let owner = metadata.uid();
    let effective_user = effective_user_id();
    if owner != 0 && owner != effective_user {
        return Err(io::Error::other(format!(
            "repair store ancestor `{}` is owned by untrusted uid {owner}",
            path.display()
        )));
    }
    if mode & 0o022 != 0 && mode & 0o1000 == 0 {
        return Err(io::Error::other(format!(
            "repair store ancestor `{}` is writable by other users without sticky-directory protection",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_ancestor_permissions(_path: &Path, _metadata: &fs::Metadata) -> io::Result<()> {
    Ok(())
}

fn create_private_directory(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    builder.mode(0o700);
    match builder.create(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(err) => Err(io::Error::new(
            err.kind(),
            format!(
                "failed to create repair store parent `{}`: {err}",
                path.display()
            ),
        )),
    }
}

fn validate_checkpoint_file_metadata(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::other(format!(
            "repair store `{}` must be a regular file, not a symlink or directory",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        if metadata.nlink() != 1 {
            return Err(io::Error::other(format!(
                "repair store `{}` must have exactly one hard link",
                path.display()
            )));
        }
        if metadata.uid() != effective_user_id() {
            return Err(io::Error::other(format!(
                "repair store `{}` is not owned by the effective user",
                path.display()
            )));
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::other(format!(
                "repair store `{}` must not be accessible by group or other users",
                path.display()
            )));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn effective_user_id() -> u32 {
    // SAFETY: `geteuid` has no arguments, owns no resources, and has no failure
    // mode; it returns the effective UID of the calling process.
    unsafe { geteuid() }
}

#[cfg(unix)]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len()
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

impl RepairStore for FileRepairStore {
    fn next_audit_sequence(&self) -> Result<u64, RepairStoreError> {
        let mut guard = self.state.write().map_err(|_| repair_store_poisoned())?;
        ensure_repair_state_healthy(&guard)?;
        let sequence = guard.next_audit_sequence;
        guard.next_audit_sequence = guard
            .next_audit_sequence
            .checked_add(1)
            .ok_or_else(|| RepairStoreError::Other("repair audit sequence exhausted".to_owned()))?;
        if let Err(err) = self.persist(&mut guard) {
            if !matches!(err, RepairStoreError::DurabilityUncertain(_)) {
                guard.next_audit_sequence = sequence;
            }
            return Err(err);
        }
        Ok(sequence)
    }

    fn append_por_history(
        &self,
        observation: PorHistoryObservation,
    ) -> Result<u64, RepairStoreError> {
        observation.validate_persisted()?;
        let mut guard = self.state.write().map_err(|_| repair_store_poisoned())?;
        ensure_repair_state_healthy(&guard)?;
        if let Some(existing) = guard
            .por_history
            .values()
            .find(|entry| entry.challenge_id == observation.challenge_id)
        {
            if existing.matches_observation(&observation) {
                return Ok(existing.id);
            }
            return Err(RepairStoreError::Other(format!(
                "conflicting PoR repair history replay for challenge {}",
                hex::encode(observation.challenge_id)
            )));
        }
        if guard.por_history.len() >= self.entry_limit {
            return Err(RepairStoreError::Other(format!(
                "repair PoR history retention exhausted (limit {})",
                self.entry_limit
            )));
        }
        let id = guard.next_por_history_id;
        let next_id = id.checked_add(1).ok_or_else(|| {
            RepairStoreError::Other("repair PoR history sequence exhausted".to_owned())
        })?;
        let entry = PorHistoryEntry::from_observation(id, observation);
        guard.por_history.insert(id, entry);
        guard.next_por_history_id = next_id;
        if let Err(err) = self.persist(&mut guard) {
            if !matches!(err, RepairStoreError::DurabilityUncertain(_)) {
                guard.por_history.remove(&id);
                guard.next_por_history_id = id;
            }
            return Err(err);
        }
        Ok(id)
    }

    fn por_history_entry(
        &self,
        por_history_id: u64,
    ) -> Result<Option<PorHistoryEntry>, RepairStoreError> {
        let guard = self.state.read().map_err(|_| repair_store_poisoned())?;
        ensure_repair_state_healthy(&guard)?;
        Ok(guard.por_history.get(&por_history_id).cloned())
    }

    fn insert_task(
        &self,
        task: RepairTaskInternal,
    ) -> Result<RepairStoreInsertResult, RepairStoreError> {
        task.validate_persisted(self.entry_limit, &self.escalation_policy)?;
        let mut guard = self.state.write().map_err(|_| repair_store_poisoned())?;
        ensure_repair_state_healthy(&guard)?;
        let key = task.report.ticket_id.0.clone();
        if let Some(existing) = guard
            .tasks
            .values()
            .find(|existing| existing.source_identity == task.source_identity)
        {
            return Ok(RepairStoreInsertResult::Existing(existing.clone()));
        }
        if let Some(existing) = guard.tasks.get(&key) {
            return Ok(RepairStoreInsertResult::Existing(existing.clone()));
        }
        if guard.tasks.len() >= self.entry_limit {
            return Err(RepairStoreError::Other(format!(
                "repair task retention exhausted (limit {})",
                self.entry_limit
            )));
        }
        guard.tasks.insert(key, task.clone());
        if let Err(err) = self.persist(&mut guard) {
            if !matches!(err, RepairStoreError::DurabilityUncertain(_)) {
                guard.tasks.remove(&task.report.ticket_id.0);
            }
            return Err(err);
        }
        Ok(RepairStoreInsertResult::Inserted(task))
    }

    fn task(
        &self,
        ticket_id: &RepairTicketId,
    ) -> Result<Option<RepairTaskInternal>, RepairStoreError> {
        let guard = self.state.read().map_err(|_| repair_store_poisoned())?;
        ensure_repair_state_healthy(&guard)?;
        Ok(guard.tasks.get(&ticket_id.0).cloned())
    }

    fn compare_and_set_task(
        &self,
        ticket_id: &RepairTicketId,
        expected_revision: u64,
        task: RepairTaskInternal,
    ) -> Result<(), RepairStoreError> {
        task.validate_persisted(self.entry_limit, &self.escalation_policy)?;
        if task.report.ticket_id != *ticket_id {
            return Err(RepairStoreError::Other(format!(
                "repair task key `{ticket_id}` does not match persisted ticket `{}`",
                task.report.ticket_id
            )));
        }
        let mut guard = self.state.write().map_err(|_| repair_store_poisoned())?;
        ensure_repair_state_healthy(&guard)?;
        let existing = guard
            .tasks
            .get(&ticket_id.0)
            .ok_or_else(|| RepairStoreError::NotFound {
                ticket_id: ticket_id.to_string(),
            })?;
        if existing.revision != expected_revision {
            return Err(RepairStoreError::Conflict {
                ticket_id: ticket_id.to_string(),
            });
        }
        if existing.source_identity != task.source_identity {
            return Err(RepairStoreError::Other(format!(
                "repair task `{ticket_id}` cannot change its source identity"
            )));
        }
        if existing.slash_proposal_stage == Some(RepairSlashProposalStage::Submitted)
            && task.slash_proposal_stage != Some(RepairSlashProposalStage::Submitted)
        {
            return Err(RepairStoreError::Other(format!(
                "repair task `{ticket_id}` cannot downgrade a submitted slash proposal"
            )));
        }
        if existing.slash_proposal_stage.is_some()
            && (task.slash_proposal_digest != existing.slash_proposal_digest
                || task.slash_proposal_bytes.as_deref() != existing.slash_proposal_bytes.as_deref())
        {
            return Err(RepairStoreError::Other(format!(
                "repair task `{ticket_id}` cannot replace persisted slash proposal bytes"
            )));
        }
        let Some(previous) = guard.tasks.insert(ticket_id.0.clone(), task) else {
            return Err(RepairStoreError::NotFound {
                ticket_id: ticket_id.to_string(),
            });
        };
        if let Err(err) = self.persist(&mut guard) {
            if !matches!(err, RepairStoreError::DurabilityUncertain(_)) {
                guard.tasks.insert(ticket_id.0.clone(), previous);
            }
            return Err(err);
        }
        Ok(())
    }

    fn list_tasks(&self) -> Result<Vec<RepairTaskInternal>, RepairStoreError> {
        let guard = self.state.read().map_err(|_| repair_store_poisoned())?;
        ensure_repair_state_healthy(&guard)?;
        Ok(guard.tasks.values().cloned().collect())
    }

    fn record_auditor_nonce(
        &self,
        auditor_account: &str,
        nonce: u64,
    ) -> Result<(), RepairStoreError> {
        if auditor_account.is_empty() || auditor_account.len() > MAX_GOVERNANCE_ACTOR_BYTES {
            return Err(RepairStoreError::Other(format!(
                "repair auditor account length must be between 1 and {MAX_GOVERNANCE_ACTOR_BYTES} bytes"
            )));
        }
        if nonce == 0 {
            return Err(RepairStoreError::Other(
                "repair auditor nonce must be greater than zero".to_owned(),
            ));
        }
        let mut guard = self.state.write().map_err(|_| repair_store_poisoned())?;
        ensure_repair_state_healthy(&guard)?;
        let highest_nonce = guard
            .auditor_nonces
            .get(auditor_account)
            .copied()
            .unwrap_or(0);
        if nonce <= highest_nonce {
            return Err(RepairStoreError::AuditorNonceReplay {
                auditor_account: auditor_account.to_owned(),
                nonce,
                highest_nonce,
            });
        }
        if !guard.auditor_nonces.contains_key(auditor_account)
            && guard.auditor_nonces.len() >= self.entry_limit
        {
            return Err(RepairStoreError::Other(format!(
                "repair auditor nonce retention exhausted (limit {})",
                self.entry_limit
            )));
        }
        guard
            .auditor_nonces
            .insert(auditor_account.to_owned(), nonce);
        if let Err(err) = self.persist(&mut guard) {
            if !matches!(err, RepairStoreError::DurabilityUncertain(_)) {
                if highest_nonce == 0 {
                    guard.auditor_nonces.remove(auditor_account);
                } else {
                    guard
                        .auditor_nonces
                        .insert(auditor_account.to_owned(), highest_nonce);
                }
            }
            return Err(err);
        }
        Ok(())
    }
}

#[derive(Debug)]
struct UnavailableRepairStore {
    reason: String,
}

impl UnavailableRepairStore {
    fn unavailable<T>(&self) -> Result<T, RepairStoreError> {
        Err(RepairStoreError::Other(self.reason.clone()))
    }
}

impl RepairStore for UnavailableRepairStore {
    fn next_audit_sequence(&self) -> Result<u64, RepairStoreError> {
        self.unavailable()
    }

    fn append_por_history(
        &self,
        _observation: PorHistoryObservation,
    ) -> Result<u64, RepairStoreError> {
        self.unavailable()
    }

    fn por_history_entry(
        &self,
        _por_history_id: u64,
    ) -> Result<Option<PorHistoryEntry>, RepairStoreError> {
        self.unavailable()
    }

    fn insert_task(
        &self,
        _task: RepairTaskInternal,
    ) -> Result<RepairStoreInsertResult, RepairStoreError> {
        self.unavailable()
    }

    fn task(
        &self,
        _ticket_id: &RepairTicketId,
    ) -> Result<Option<RepairTaskInternal>, RepairStoreError> {
        self.unavailable()
    }

    fn compare_and_set_task(
        &self,
        _ticket_id: &RepairTicketId,
        _expected_revision: u64,
        _task: RepairTaskInternal,
    ) -> Result<(), RepairStoreError> {
        self.unavailable()
    }

    fn list_tasks(&self) -> Result<Vec<RepairTaskInternal>, RepairStoreError> {
        self.unavailable()
    }

    fn record_auditor_nonce(
        &self,
        _auditor_account: &str,
        _nonce: u64,
    ) -> Result<(), RepairStoreError> {
        self.unavailable()
    }
}

fn repair_store_poisoned() -> RepairStoreError {
    RepairStoreError::Other("repair store state lock poisoned".to_owned())
}

/// Manages repair tickets and PoR failure history.
#[derive(Debug, Clone)]
pub struct RepairManager {
    store: Arc<dyn RepairStore>,
    default_sla_secs: u64,
    event_history_limit: usize,
    config: RepairConfig,
    escalation_policy: RepairEscalationPolicy,
}

/// Filters for listing repair tasks.
#[derive(Debug, Clone, Default)]
pub struct RepairTaskFilters {
    /// Optional manifest digest to filter by.
    pub manifest_digest: Option<[u8; 32]>,
    /// Optional provider identifier to filter by.
    pub provider_id: Option<[u8; 32]>,
    /// Optional task status to filter by.
    pub status: Option<RepairTaskStatusV1>,
}

/// Authoritative publication stage of a persisted repair slash proposal.
///
/// The stage is checkpointed atomically with the proposal's canonical bytes
/// and digest so a publisher can reconcile drafts after a process restart
/// without resubmitting proposals already accepted locally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub enum RepairSlashProposalStage {
    /// The scheduler created and persisted the proposal, but submission has not
    /// yet been accepted.
    Drafted,
    /// [`RepairManager::submit_slash_proposal`] accepted this exact canonical
    /// proposal.
    Submitted,
}

/// Snapshot of a repair task with its event history.
#[derive(Debug, Clone)]
pub struct RepairTaskSnapshot {
    /// Current task record.
    pub record: RepairTaskRecordV1,
    /// Number of oldest events omitted from the retained suffix.
    pub events_dropped: u64,
    /// Event log ordered by occurrence.
    pub events: Vec<RepairTaskEventV1>,
    /// Canonically decoded slash proposal, when this task is escalated.
    pub slash_proposal: Option<RepairSlashProposalV1>,
    /// Authoritative publication stage paired with [`Self::slash_proposal`].
    pub slash_proposal_stage: Option<RepairSlashProposalStage>,
}

/// Result of applying a repair task transition.
#[derive(Debug, Clone)]
pub struct RepairTaskUpdate {
    /// Updated task record.
    pub record: RepairTaskRecordV1,
    /// Optional audit event emitted for the transition.
    pub event: Option<RepairTaskEventV1>,
    /// Optional slash proposal drafted during escalation.
    pub slash_proposal: Option<RepairSlashProposalV1>,
}

/// Summary of actions taken by the repair watchdog.
#[derive(Debug, Clone, Default)]
pub struct RepairWatchdogReport {
    /// Draft slash proposals emitted for escalations.
    pub escalated: Vec<RepairSlashProposalV1>,
    /// Tickets re-queued by the watchdog.
    pub requeued: Vec<RepairTicketId>,
    /// Events emitted during watchdog transitions.
    pub events: Vec<RepairTaskEventV1>,
    /// Number of lease expirations detected by the watchdog.
    pub lease_expired: u32,
}

#[derive(Debug, Clone, Default)]
struct RepairBacklogStats {
    oldest_age_secs: u64,
    per_provider: BTreeMap<[u8; 32], u64>,
}

/// Summary of work performed by an automated repair worker tick.
#[derive(Debug, Clone, Default)]
pub struct RepairWorkerReport {
    /// Tickets successfully claimed by the worker.
    pub claimed: u32,
    /// Tickets marked completed during the tick.
    pub completed: u32,
    /// Tickets marked failed (without escalation) during the tick.
    pub failed: u32,
    /// Tickets escalated during the tick.
    pub escalated: u32,
    /// Claim attempts skipped due to lease/backoff contention.
    pub skipped: u32,
    /// Unexpected errors encountered while running the worker.
    pub errors: u32,
}

impl RepairWorkerReport {
    pub(crate) fn record_claim(&mut self) {
        self.claimed = self.claimed.saturating_add(1);
    }

    pub(crate) fn record_skipped(&mut self) {
        self.skipped = self.skipped.saturating_add(1);
    }

    pub(crate) fn record_error(&mut self) {
        self.errors = self.errors.saturating_add(1);
    }

    pub(crate) fn record_state(&mut self, state: &RepairTaskStateV1) {
        match state {
            RepairTaskStateV1::Completed(_) => {
                self.completed = self.completed.saturating_add(1);
            }
            RepairTaskStateV1::Failed(_) => {
                self.failed = self.failed.saturating_add(1);
            }
            RepairTaskStateV1::Escalated(_) => {
                self.escalated = self.escalated.saturating_add(1);
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
struct EscalationOutcome {
    proposal: RepairSlashProposalV1,
    event: Option<RepairTaskEventV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RepairGovernanceVote {
    voter_id: String,
    voted_at_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RepairGovernanceVoteKind {
    Approve,
    Reject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[norito(tag = "reason", content = "details", rename_all = "snake_case")]
enum RepairGovernanceRejectReason {
    InsufficientQuorum,
    Tie,
    QuorumNotMet,
    RejectMajority,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[norito(tag = "decision", content = "details", rename_all = "snake_case")]
enum RepairGovernanceDecision {
    Approved {
        decided_at_unix: u64,
        approvals: u32,
        rejections: u32,
    },
    Rejected {
        decided_at_unix: u64,
        approvals: u32,
        rejections: u32,
        reason: RepairGovernanceRejectReason,
    },
    Appealed {
        approved_at_unix: u64,
        appealed_at_unix: u64,
        approvals: u32,
        rejections: u32,
        appellant: String,
        #[norito(default)]
        reason: Option<String>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RepairGovernanceState {
    #[norito(default)]
    approvals: Vec<RepairGovernanceVote>,
    #[norito(default)]
    rejections: Vec<RepairGovernanceVote>,
    #[norito(default)]
    decision: Option<RepairGovernanceDecision>,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RepairGovernancePolicySnapshot {
    quorum_bps: u16,
    minimum_voters: u32,
    dispute_window_secs: u64,
    appeal_window_secs: u64,
    max_penalty: XorQuantity,
}

impl RepairGovernancePolicySnapshot {
    fn from_runtime(policy: &RepairEscalationPolicy) -> Self {
        Self {
            quorum_bps: policy.quorum_bps(),
            minimum_voters: policy.minimum_voters(),
            dispute_window_secs: policy.dispute_window_secs(),
            appeal_window_secs: policy.appeal_window_secs(),
            max_penalty: policy.max_penalty().clone(),
        }
    }

    fn validate(&self) -> Result<(), RepairStoreError> {
        if self.quorum_bps > 10_000 || self.minimum_voters == 0 || self.max_penalty.is_zero() {
            return Err(RepairStoreError::Other(
                "repair governance policy snapshot is invalid".to_owned(),
            ));
        }
        Ok(())
    }
}

impl RepairTaskFilters {
    /// Filter tasks by manifest digest.
    #[must_use]
    pub fn for_manifest(manifest_digest: [u8; 32]) -> Self {
        Self {
            manifest_digest: Some(manifest_digest),
            ..Self::default()
        }
    }

    fn matches(&self, record: &RepairTaskRecordV1) -> bool {
        if let Some(digest) = self.manifest_digest
            && record.manifest_digest != digest
        {
            return false;
        }
        if let Some(provider_id) = self.provider_id
            && record.provider_id != provider_id
        {
            return false;
        }
        if let Some(status) = self.status
            && repair_task_status(&record.state) != status
        {
            return false;
        }
        true
    }
}

fn build_repair_store(
    config: &RepairConfig,
    entry_limit: usize,
    max_bytes: u64,
) -> Arc<dyn RepairStore> {
    match try_build_repair_store(config, entry_limit, max_bytes) {
        Ok(store) => store,
        Err(err) => {
            let reason = err.to_string();
            error!(%reason, "repair subsystem is unavailable and will fail closed");
            Arc::new(UnavailableRepairStore { reason })
        }
    }
}

fn try_build_repair_store(
    config: &RepairConfig,
    entry_limit: usize,
    max_bytes: u64,
) -> Result<Arc<dyn RepairStore>, RepairStoreError> {
    if !config.enabled() {
        return Ok(Arc::new(UnavailableRepairStore {
            reason: "repair subsystem is disabled".to_owned(),
        }));
    }
    let Some(state_dir) = config.state_dir().cloned() else {
        return Err(RepairStoreError::Other(
            "repair store state_dir is not configured".to_owned(),
        ));
    };
    let path = state_dir.join(REPAIR_STORE_FILE_NAME);
    FileRepairStore::load_or_new_with_policy(
        path.clone(),
        entry_limit,
        max_bytes,
        config.escalation_policy().clone(),
    )
    .map(|store| Arc::new(store) as Arc<dyn RepairStore>)
    .map_err(|err| {
        RepairStoreError::Other(format!(
            "failed to load repair store `{}`: {err}",
            path.display()
        ))
    })
}

/// Resolve the configured repair checkpoint path for startup diagnostics.
pub(crate) fn repair_store_checkpoint_path(config: &RepairConfig) -> Option<PathBuf> {
    config
        .state_dir()
        .map(|state_dir| state_dir.join(REPAIR_STORE_FILE_NAME))
}

impl RepairManager {
    /// Construct a new repair manager with the default SLA window.
    #[must_use]
    pub fn new() -> Self {
        Self::new_with_config(RepairConfig::default())
    }

    /// Construct a new repair manager using the provided configuration.
    #[must_use]
    pub fn new_with_config(config: RepairConfig) -> Self {
        Self::new_with_config_policy_and_limits(
            config.clone(),
            config.escalation_policy().clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
    }

    /// Construct a repair manager with explicit durable-store safety ceilings.
    #[must_use]
    pub fn new_with_config_policy_and_limits(
        config: RepairConfig,
        escalation_policy: RepairEscalationPolicy,
        entry_limit: usize,
        max_bytes: u64,
    ) -> Self {
        let config = config.with_escalation_policy(escalation_policy);
        let store = build_repair_store(&config, entry_limit, max_bytes);
        let escalation_policy = config.escalation_policy().clone();
        Self {
            store,
            default_sla_secs: DEFAULT_REPAIR_SLA_SECS,
            event_history_limit: DEFAULT_REPAIR_EVENT_HISTORY_LIMIT,
            config,
            escalation_policy,
        }
    }

    /// Construct a repair manager with explicit durable-store safety ceilings.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured checkpoint path is absent, unsafe,
    /// unreadable, corrupt, non-canonical, or violates persisted invariants.
    pub fn try_new_with_config_policy_and_limits(
        config: RepairConfig,
        escalation_policy: RepairEscalationPolicy,
        entry_limit: usize,
        max_bytes: u64,
    ) -> Result<Self, RepairStoreError> {
        let config = config.with_escalation_policy(escalation_policy);
        let store = try_build_repair_store(&config, entry_limit, max_bytes)?;
        let escalation_policy = config.escalation_policy().clone();
        Ok(Self {
            store,
            default_sla_secs: DEFAULT_REPAIR_SLA_SECS,
            event_history_limit: DEFAULT_REPAIR_EVENT_HISTORY_LIMIT,
            config,
            escalation_policy,
        })
    }

    /// Construct a new repair manager with an explicit governance escalation policy.
    #[must_use]
    pub fn new_with_config_and_policy(
        config: RepairConfig,
        policy: RepairEscalationPolicy,
    ) -> Self {
        Self::new_with_config_policy_and_limits(
            config,
            policy,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
    }

    /// Reserve the next audit sequence number for governance events.
    pub fn next_audit_sequence(&self) -> Result<u64, RepairStoreError> {
        self.store.next_audit_sequence()
    }

    /// Register a PoR verdict; returns a history identifier when the verdict recorded failures.
    pub fn register_por_verdict(
        &self,
        verdict: &AuditVerdictV1,
        failed_samples: u64,
    ) -> Result<Option<u64>, RepairStoreError> {
        if failed_samples == 0 {
            return Ok(None);
        }
        let observation = PorHistoryObservation {
            manifest_digest: verdict.manifest_digest,
            provider_id: verdict.provider_id,
            challenge_id: verdict.challenge_id,
            decided_at: verdict.decided_at,
            failed_samples,
        };
        let history_id = self.store.append_por_history(observation)?;
        debug!(
            manifest = %hex::encode(verdict.manifest_digest),
            provider = %hex::encode(verdict.provider_id),
            challenge = %hex::encode(verdict.challenge_id),
            failed_samples,
            history_id,
            "registered PoR failure for repair history"
        );
        Ok(Some(history_id))
    }

    /// Record a signed auditor request nonce, rejecting stale or replayed values.
    pub fn record_auditor_nonce(
        &self,
        auditor_account: &str,
        nonce: u64,
    ) -> Result<(), RepairSchedulerError> {
        match self.store.record_auditor_nonce(auditor_account, nonce) {
            Ok(()) => Ok(()),
            Err(RepairStoreError::AuditorNonceReplay {
                auditor_account,
                nonce,
                highest_nonce,
            }) => Err(RepairSchedulerError::AuditorNonceReplay {
                auditor_account,
                nonce,
                highest_nonce,
            }),
            Err(err) => Err(RepairSchedulerError::Store(err)),
        }
    }

    /// Enqueue a repair report submitted by an auditor.
    pub fn enqueue_report(
        &self,
        report: RepairReportV1,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        Ok(self.enqueue_report_with_event(report)?.record)
    }

    /// Enqueue a repair report exactly once for a stable subsystem event or
    /// signed-receipt identity.
    ///
    /// An exact replay returns the original task record. Reusing the source
    /// identity for a different canonical report fails closed even when the
    /// conflicting report carries a different ticket identifier.
    pub fn enqueue_repair_report_idempotent(
        &self,
        source_identity: [u8; 32],
        report: RepairReportV1,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        Ok(self
            .enqueue_repair_report_idempotent_with_event(source_identity, report)?
            .record)
    }

    /// Enqueue an exactly-once subsystem report and return its local projection
    /// update for event publication.
    pub fn enqueue_repair_report_idempotent_with_event(
        &self,
        source_identity: [u8; 32],
        report: RepairReportV1,
    ) -> Result<RepairTaskUpdate, RepairSchedulerError> {
        self.enqueue_report_with_source_identity_and_event(source_identity, report)
    }

    /// Enqueue a repair report submitted by an auditor, returning any emitted event.
    pub fn enqueue_report_with_event(
        &self,
        report: RepairReportV1,
    ) -> Result<RepairTaskUpdate, RepairSchedulerError> {
        let source_identity = repair_report_source_identity(&report)?;
        self.enqueue_report_with_source_identity_and_event(source_identity, report)
    }

    fn enqueue_report_with_source_identity_and_event(
        &self,
        source_identity: [u8; 32],
        report: RepairReportV1,
    ) -> Result<RepairTaskUpdate, RepairSchedulerError> {
        if source_identity == [0; 32] {
            return Err(RepairSchedulerError::InvalidSourceIdentity);
        }
        report
            .validate()
            .map_err(RepairSchedulerError::InvalidReport)?;
        match (&report.evidence.cause, report.evidence.por_history_id) {
            (RepairCauseV1::PorFailure(_), Some(por_id)) => {
                self.ensure_por_history_match(por_id, report.submitted_at_unix, &report.evidence)?;
            }
            (RepairCauseV1::PorFailure(_), None) => {
                return Err(RepairSchedulerError::MissingPorHistory {
                    ticket_id: report.ticket_id.to_string(),
                });
            }
            (_, Some(por_history_id)) => {
                return Err(RepairSchedulerError::UnexpectedPorHistory {
                    ticket_id: report.ticket_id.to_string(),
                    por_history_id,
                });
            }
            (_, None) => {}
        }

        let canonical_report = report.clone();
        let ticket_id = report.ticket_id.to_string();
        let sla_deadline = report
            .submitted_at_unix
            .checked_add(self.default_sla_secs)
            .ok_or_else(|| RepairSchedulerError::InvalidTimestamp {
                ticket_id: report.ticket_id.to_string(),
            })?;
        let state = RepairTaskStateV1::Queued(QueuedRepairStateV1 {
            queued_at_unix: report.submitted_at_unix,
            sla_deadline_unix: Some(sla_deadline),
        });
        let mut internal = RepairTaskInternal {
            source_identity,
            report,
            state,
            sla_deadline_unix: Some(sla_deadline),
            scheduler_notes: None,
            slash_proposal_digest: None,
            slash_proposal_bytes: None,
            slash_proposal_stage: None,
            governance: RepairGovernanceState::default(),
            governance_policy: None,
            lease: None,
            idempotency: RepairTaskIdempotency::new(DEFAULT_IDEMPOTENCY_CACHE_SIZE),
            attempts: 0,
            next_attempt_after_unix: None,
            revision: 0,
            events_dropped: 0,
            events: Vec::new(),
        };
        let event = internal.push_event(
            RepairTaskStatusV1::Queued,
            internal.report.submitted_at_unix,
            Some(internal.report.auditor_account.clone()),
            internal
                .report
                .notes
                .clone()
                .or_else(|| internal.report.evidence.notes.clone()),
            self.event_history_limit,
        );
        let insert = self.store.insert_task(internal)?;
        let (record, event) = match insert {
            RepairStoreInsertResult::Inserted(inserted) => (inserted.to_record(), event),
            RepairStoreInsertResult::Existing(existing) => {
                if existing.source_identity == source_identity
                    && existing.report != canonical_report
                {
                    return Err(RepairSchedulerError::SourceIdentityConflict { source_identity });
                }
                if existing.report != canonical_report {
                    return Err(RepairSchedulerError::DuplicateTicket { ticket_id });
                }
                if existing.source_identity != source_identity {
                    return Err(RepairSchedulerError::DuplicateTicket { ticket_id });
                }
                return Ok(RepairTaskUpdate {
                    record: existing.to_record(),
                    event: None,
                    slash_proposal: None,
                });
            }
        };

        if event.is_some() {
            global_sorafs_repair_otel().record_task_transition("queued");
            global_or_default().inc_sorafs_repair_tasks("queued");
        }
        Ok(RepairTaskUpdate {
            record,
            event,
            slash_proposal: None,
        })
    }

    /// Submit a slash proposal associated with an escalated repair.
    pub fn submit_slash_proposal(
        &self,
        proposal: RepairSlashProposalV1,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        Ok(self.submit_slash_proposal_with_event(proposal)?.record)
    }

    /// Submit a slash proposal associated with an escalated repair, returning any event.
    pub fn submit_slash_proposal_with_event(
        &self,
        proposal: RepairSlashProposalV1,
    ) -> Result<RepairTaskUpdate, RepairSchedulerError> {
        proposal
            .validate()
            .map_err(RepairSchedulerError::InvalidSlashProposal)?;
        let proposal_bytes = norito::to_bytes(&proposal)
            .map_err(|err| repair_encoding_error("slash proposal", err))?;
        let proposal_digest = *hash(&proposal_bytes).as_bytes();
        let mut event = None;
        let mut publication_transitioned = false;
        let task = self.update_task_with_retry(&proposal.ticket_id, |task| {
            event = None;
            publication_transitioned = false;
            if task.report.evidence.manifest_digest != proposal.manifest_digest {
                return Err(RepairSchedulerError::ManifestMismatch {
                    ticket_id: proposal.ticket_id.to_string(),
                });
            }
            if task.report.evidence.provider_id != proposal.provider_id {
                return Err(RepairSchedulerError::ProviderMismatch {
                    ticket_id: proposal.ticket_id.to_string(),
                });
            }
            if task.report.auditor_account != proposal.auditor_account {
                return Err(policy_violation(
                    &proposal.ticket_id,
                    "slash proposal auditor does not match the repair report",
                ));
            }
            if proposal.approval.is_some() {
                return Err(policy_violation(
                    &proposal.ticket_id,
                    "embedded approval summaries are not authoritative; submit authenticated governance votes",
                ));
            }
            if let Some(existing_digest) = task.slash_proposal_digest {
                if existing_digest == proposal_digest
                    && task.slash_proposal_bytes.as_deref() == Some(proposal_bytes.as_slice())
                {
                    return match task.slash_proposal_stage {
                        Some(RepairSlashProposalStage::Drafted) => {
                            task.slash_proposal_stage =
                                Some(RepairSlashProposalStage::Submitted);
                            publication_transitioned = true;
                            Ok(())
                        }
                        Some(RepairSlashProposalStage::Submitted) => Ok(()),
                        None => Err(RepairSchedulerError::Store(RepairStoreError::Other(
                            format!(
                                "repair task `{}` has slash proposal bytes without a publication stage",
                                proposal.ticket_id
                            ),
                        ))),
                    };
                }
                return Err(policy_violation(
                    &proposal.ticket_id,
                    "conflicting slash proposal already recorded",
                ));
            }
            if task.slash_proposal_bytes.is_some() || task.slash_proposal_stage.is_some() {
                return Err(RepairSchedulerError::Store(RepairStoreError::Other(
                    format!(
                        "repair task `{}` has incomplete slash proposal publication state",
                        proposal.ticket_id
                    ),
                )));
            }

            if task.governance.decision.is_some() {
                return Err(policy_violation(
                    &proposal.ticket_id,
                    "governance decision already finalized",
                ));
            }
            let queued_at = queued_at_unix(&task.state);
            ensure_transition_allowed(&task.state, "escalated", &proposal.ticket_id)?;
            if proposal.submitted_at_unix <= queued_at {
                return Err(RepairSchedulerError::InvalidTimestamp {
                    ticket_id: proposal.ticket_id.to_string(),
                });
            }
            let was_escalated = matches!(task.state, RepairTaskStateV1::Escalated(..));
            let (escalated_at_unix, existing_reason) = match &task.state {
                RepairTaskStateV1::Escalated(state) => {
                    (state.escalated_at_unix, Some(state.reason.clone()))
                }
                _ => (proposal.submitted_at_unix, None),
            };
            if was_escalated && proposal.submitted_at_unix < escalated_at_unix {
                return Err(RepairSchedulerError::InvalidTimestamp {
                    ticket_id: proposal.ticket_id.to_string(),
                });
            }
            if !was_escalated {
                task.governance = RepairGovernanceState::default();
                task.governance_policy = Some(RepairGovernancePolicySnapshot::from_runtime(
                    &self.escalation_policy,
                ));
            }
            let policy = task.governance_policy.clone().ok_or_else(|| {
                RepairSchedulerError::Store(RepairStoreError::Other(format!(
                    "repair task `{}` is missing its governance policy snapshot",
                    proposal.ticket_id
                )))
            })?;
            checked_add_secs(
                proposal.submitted_at_unix,
                policy.dispute_window_secs,
                &proposal.ticket_id,
            )?;
            if proposal.proposed_penalty > policy.max_penalty {
                return Err(policy_violation(
                    &proposal.ticket_id,
                    "proposed penalty exceeds policy cap",
                ));
            }
            let reason = existing_reason.unwrap_or_else(|| proposal.rationale.clone());
            task.state = RepairTaskStateV1::Escalated(EscalatedRepairStateV1 {
                queued_at_unix: queued_at,
                escalated_at_unix,
                reason,
            });
            task.slash_proposal_digest = Some(proposal_digest);
            task.slash_proposal_bytes = Some(proposal_bytes.clone());
            task.slash_proposal_stage = Some(RepairSlashProposalStage::Submitted);
            publication_transitioned = true;
            task.next_attempt_after_unix = None;
            if !was_escalated {
                event = task.push_event(
                    RepairTaskStatusV1::Escalated,
                    escalated_at_unix,
                    Some(proposal.auditor_account.clone()),
                    Some(proposal.rationale.clone()),
                    self.event_history_limit,
                );
            }
            Ok(())
        })?;

        if event.is_some() {
            let queued_at = queued_at_unix(&task.state);
            global_sorafs_repair_otel().record_task_transition("escalated");
            self.observe_latency(queued_at, proposal.submitted_at_unix, "escalated");
            global_or_default().inc_sorafs_repair_tasks("escalated");
        }
        if publication_transitioned {
            global_sorafs_repair_otel().record_slash_proposal("submitted");
            global_or_default().inc_sorafs_slash_proposals("submitted");
        }
        Ok(RepairTaskUpdate {
            record: task.to_record(),
            event,
            slash_proposal: Some(proposal),
        })
    }

    /// Record a governance approval vote for an escalated repair.
    pub fn submit_slash_approval(
        &self,
        ticket_id: &RepairTicketId,
        voter_id: &str,
        voted_at_unix: u64,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        Ok(self
            .submit_slash_vote(
                ticket_id,
                voter_id,
                voted_at_unix,
                RepairGovernanceVoteKind::Approve,
            )?
            .to_record())
    }

    /// Record a governance rejection vote for an escalated repair.
    pub fn submit_slash_rejection(
        &self,
        ticket_id: &RepairTicketId,
        voter_id: &str,
        voted_at_unix: u64,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        Ok(self
            .submit_slash_vote(
                ticket_id,
                voter_id,
                voted_at_unix,
                RepairGovernanceVoteKind::Reject,
            )?
            .to_record())
    }

    /// Record a governance appeal for an approved escalation decision.
    pub fn submit_slash_appeal(
        &self,
        ticket_id: &RepairTicketId,
        appellant: &str,
        appealed_at_unix: u64,
        reason: Option<String>,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        ensure_worker_field(
            appellant,
            "appellant",
            MAX_GOVERNANCE_ACTOR_BYTES,
            ticket_id,
        )?;
        ensure_optional_field(
            reason.as_deref(),
            "appeal_reason",
            MAX_GOVERNANCE_REASON_BYTES,
            ticket_id,
        )?;
        if appealed_at_unix == 0 {
            return Err(RepairSchedulerError::InvalidTimestamp {
                ticket_id: ticket_id.to_string(),
            });
        }
        let reason_clone = reason.clone();
        let task = self.update_task_with_retry(ticket_id, |task| {
            let decision = match &task.governance.decision {
                Some(decision) => decision,
                None => {
                    return Err(policy_violation(
                        ticket_id,
                        "governance decision not yet finalized",
                    ));
                }
            };
            let (approved_at_unix, approvals, rejections) = match decision {
                RepairGovernanceDecision::Approved {
                    decided_at_unix,
                    approvals,
                    rejections,
                } => (*decided_at_unix, *approvals, *rejections),
                RepairGovernanceDecision::Appealed { .. } => {
                    return Err(policy_violation(
                        ticket_id,
                        "appeal already recorded for this decision",
                    ));
                }
                RepairGovernanceDecision::Rejected { .. } => {
                    return Err(policy_violation(
                        ticket_id,
                        "cannot appeal a rejected escalation",
                    ));
                }
            };
            if appealed_at_unix <= approved_at_unix {
                return Err(RepairSchedulerError::InvalidTimestamp {
                    ticket_id: ticket_id.to_string(),
                });
            }
            let policy = persisted_task_governance_policy(task, ticket_id)?;
            let appeal_deadline =
                checked_add_secs(approved_at_unix, policy.appeal_window_secs, ticket_id)?;
            if appealed_at_unix > appeal_deadline {
                return Err(policy_violation(ticket_id, "appeal window closed"));
            }
            task.governance.decision = Some(RepairGovernanceDecision::Appealed {
                approved_at_unix,
                appealed_at_unix,
                approvals,
                rejections,
                appellant: appellant.to_string(),
                reason: reason_clone.clone(),
            });
            Ok(())
        })?;
        Ok(task.to_record())
    }

    fn submit_slash_vote(
        &self,
        ticket_id: &RepairTicketId,
        voter_id: &str,
        voted_at_unix: u64,
        kind: RepairGovernanceVoteKind,
    ) -> Result<RepairTaskInternal, RepairSchedulerError> {
        ensure_worker_field(voter_id, "voter_id", MAX_GOVERNANCE_ACTOR_BYTES, ticket_id)?;
        if voted_at_unix == 0 {
            return Err(RepairSchedulerError::InvalidTimestamp {
                ticket_id: ticket_id.to_string(),
            });
        }
        self.update_task_with_retry(ticket_id, |task| {
            let escalated_at_unix = match &task.state {
                RepairTaskStateV1::Escalated(state) => state.escalated_at_unix,
                other => {
                    return Err(RepairSchedulerError::InvalidState {
                        ticket_id: ticket_id.to_string(),
                        state: format!("{other:?}"),
                    });
                }
            };
            if task.governance.decision.is_some() {
                return Err(policy_violation(
                    ticket_id,
                    "governance decision already finalized",
                ));
            }
            if voted_at_unix <= escalated_at_unix {
                return Err(RepairSchedulerError::InvalidTimestamp {
                    ticket_id: ticket_id.to_string(),
                });
            }
            let policy = persisted_task_governance_policy(task, ticket_id)?;
            let dispute_deadline =
                checked_add_secs(escalated_at_unix, policy.dispute_window_secs, ticket_id)?;
            if voted_at_unix > dispute_deadline {
                return Err(policy_violation(ticket_id, "dispute window closed"));
            }
            match kind {
                RepairGovernanceVoteKind::Approve => {
                    if vote_exists(&task.governance.rejections, voter_id) {
                        return Err(policy_violation(
                            ticket_id,
                            "voter already cast a rejecting vote",
                        ));
                    }
                    insert_vote(
                        &mut task.governance.approvals,
                        voter_id,
                        voted_at_unix,
                        ticket_id,
                    )?;
                }
                RepairGovernanceVoteKind::Reject => {
                    if vote_exists(&task.governance.approvals, voter_id) {
                        return Err(policy_violation(
                            ticket_id,
                            "voter already cast an approving vote",
                        ));
                    }
                    insert_vote(
                        &mut task.governance.rejections,
                        voter_id,
                        voted_at_unix,
                        ticket_id,
                    )?;
                }
            }
            Ok(())
        })
    }

    /// Fetch all repair tasks associated with `manifest_digest`.
    pub fn tasks_for_manifest(
        &self,
        manifest_digest: &[u8; 32],
    ) -> Result<Vec<RepairTaskRecordV1>, RepairStoreError> {
        self.list_tasks(RepairTaskFilters::for_manifest(*manifest_digest))
    }

    /// List repair tasks with optional filters applied.
    pub fn list_tasks(
        &self,
        filters: RepairTaskFilters,
    ) -> Result<Vec<RepairTaskRecordV1>, RepairStoreError> {
        let tasks = self.store.list_tasks()?;
        let mut records: Vec<RepairTaskRecordV1> = tasks
            .into_iter()
            .map(|task| task.to_record())
            .filter(|record| filters.matches(record))
            .collect();
        sort_repair_task_records(&mut records);
        Ok(records)
    }

    /// List repair task snapshots with optional filters applied.
    pub fn list_task_snapshots(
        &self,
        filters: RepairTaskFilters,
    ) -> Result<Vec<RepairTaskSnapshot>, RepairStoreError> {
        let tasks = self.store.list_tasks()?;
        let mut snapshots: Vec<RepairTaskSnapshot> = tasks
            .into_iter()
            .map(|task| task.to_snapshot())
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|snapshot| filters.matches(&snapshot.record))
            .collect();
        sort_repair_task_snapshots(&mut snapshots);
        Ok(snapshots)
    }

    fn backlog_stats(&self, now_unix: u64) -> Result<RepairBacklogStats, RepairStoreError> {
        let tasks = self.store.list_tasks()?;
        Ok(compute_backlog_stats(&tasks, now_unix))
    }

    fn record_backlog_metrics(&self, stats: &RepairBacklogStats) {
        let mut provider_depths = Vec::with_capacity(stats.per_provider.len());
        for (provider_id, depth) in &stats.per_provider {
            provider_depths.push((hex::encode(provider_id), *depth));
        }
        global_or_default().record_sorafs_repair_queue_depths(&provider_depths);
        global_or_default().set_sorafs_repair_backlog_oldest_age_seconds(stats.oldest_age_secs);
        let otel = global_sorafs_repair_otel();
        otel.record_backlog_oldest_age_seconds(stats.oldest_age_secs as f64);
        for (provider_hex, depth) in provider_depths {
            otel.record_queue_depth(depth, &provider_hex);
        }
    }

    /// List claimable repair tasks ordered by priority.
    pub fn claimable_tasks(
        &self,
        now_unix: u64,
    ) -> Result<Vec<RepairTaskRecordV1>, RepairStoreError> {
        let tasks = self.store.list_tasks()?;
        let mut candidates: Vec<RepairTaskInternal> = tasks
            .into_iter()
            .filter(|task| matches!(task.state, RepairTaskStateV1::Queued(..)))
            .filter(|task| {
                task.next_attempt_after_unix
                    .is_none_or(|retry_after| now_unix >= retry_after)
            })
            .collect();

        let mut provider_backlog: HashMap<[u8; 32], u32> = HashMap::new();
        for task in &candidates {
            let entry = provider_backlog
                .entry(task.report.evidence.provider_id)
                .or_insert(0);
            *entry = entry.saturating_add(1);
        }

        candidates.sort_by(|left, right| {
            let left_deadline = left.sla_deadline_unix.unwrap_or(u64::MAX);
            let right_deadline = right.sla_deadline_unix.unwrap_or(u64::MAX);
            let left_severity = repair_severity_score(&left.report.evidence.cause);
            let right_severity = repair_severity_score(&right.report.evidence.cause);
            let left_impact = provider_backlog
                .get(&left.report.evidence.provider_id)
                .copied()
                .unwrap_or(0);
            let right_impact = provider_backlog
                .get(&right.report.evidence.provider_id)
                .copied()
                .unwrap_or(0);

            left_deadline
                .cmp(&right_deadline)
                .then_with(|| Reverse(left_severity.0).cmp(&Reverse(right_severity.0)))
                .then_with(|| Reverse(left_severity.1).cmp(&Reverse(right_severity.1)))
                .then_with(|| Reverse(left_impact).cmp(&Reverse(right_impact)))
                .then_with(|| queued_at_unix(&left.state).cmp(&queued_at_unix(&right.state)))
                .then_with(|| {
                    left.report
                        .evidence
                        .manifest_digest
                        .cmp(&right.report.evidence.manifest_digest)
                })
                .then_with(|| left.report.ticket_id.0.cmp(&right.report.ticket_id.0))
        });

        Ok(candidates
            .into_iter()
            .map(|task| task.to_record())
            .collect())
    }

    /// Run the repair watchdog to requeue expired leases and escalate SLA breaches.
    pub fn run_watchdog(
        &self,
        now_unix: u64,
    ) -> Result<RepairWatchdogReport, RepairSchedulerError> {
        if now_unix == 0 {
            return Ok(RepairWatchdogReport::default());
        }
        let mut report = RepairWatchdogReport::default();
        let mut tasks = self.store.list_tasks()?;
        tasks.sort_by(|left, right| left.report.ticket_id.0.cmp(&right.report.ticket_id.0));

        for task in tasks {
            let ticket_id = task.report.ticket_id.clone();
            let status = repair_task_status(&task.state);
            if matches!(status, RepairTaskStatusV1::Completed) {
                continue;
            }
            if matches!(status, RepairTaskStatusV1::Escalated) {
                self.update_task_with_retry(&ticket_id, |task| {
                    let _ = self.resolve_governance_decision(task, now_unix)?;
                    Ok(())
                })?;
                continue;
            }

            if let Some(deadline) = task.sla_deadline_unix
                && now_unix >= deadline
            {
                let mut drafted = None;
                let mut event = None;
                let mut escalated = false;
                let rationale = format!("SLA deadline {deadline} breached at {now_unix}");
                let updated = self.update_task_with_retry(&ticket_id, |task| {
                    drafted = None;
                    event = None;
                    escalated = false;
                    if matches!(
                        repair_task_status(&task.state),
                        RepairTaskStatusV1::Completed | RepairTaskStatusV1::Escalated
                    ) {
                        return Ok(());
                    }
                    let Some(task_deadline) = task.sla_deadline_unix else {
                        return Ok(());
                    };
                    if now_unix < task_deadline {
                        return Ok(());
                    }
                    let escalation =
                        self.apply_escalation(task, now_unix, rationale.clone(), "scheduler")?;
                    drafted = Some(escalation.proposal);
                    event = escalation.event;
                    escalated = true;
                    Ok(())
                })?;
                if escalated {
                    if let Some(proposal) = drafted {
                        report.escalated.push(proposal);
                    }
                    if let Some(event) = event {
                        report.events.push(event);
                    }
                    let queued_at = queued_at_unix(&updated.state);
                    global_sorafs_repair_otel().record_task_transition("escalated");
                    global_or_default().inc_sorafs_repair_tasks("escalated");
                    global_sorafs_repair_otel().record_slash_proposal("drafted");
                    global_or_default().inc_sorafs_slash_proposals("drafted");
                    self.observe_latency(queued_at, now_unix, "escalated");
                }
                continue;
            }

            if let RepairTaskStateV1::InProgress(..) = &task.state
                && let Some(lease) = &task.lease
                && lease.is_expired_at(now_unix)
            {
                let mut requeued = false;
                let mut drafted = None;
                let mut event = None;
                let reason = "lease expired; requeued".to_string();
                let updated = self.update_task_with_retry(&ticket_id, |task| {
                    requeued = false;
                    drafted = None;
                    event = None;
                    let lease = match &task.lease {
                        Some(lease) => lease,
                        None => return Ok(()),
                    };
                    if !lease.is_expired_at(now_unix) {
                        return Ok(());
                    }
                    task.attempts = task.attempts.saturating_add(1);
                    let max_attempts = self.config.max_attempts();
                    if task.attempts >= max_attempts {
                        let rationale = format!(
                            "lease expired; attempts {}/{} exceeded",
                            task.attempts, max_attempts
                        );
                        let escalation =
                            self.apply_escalation(task, now_unix, rationale, "scheduler")?;
                        drafted = Some(escalation.proposal);
                        event = escalation.event;
                        return Ok(());
                    }
                    let queued_at = queued_at_unix(&task.state);
                    let retry_after = next_attempt_after_unix(
                        now_unix,
                        task.attempts,
                        &self.config,
                        &task.report.ticket_id,
                    )?;
                    task.state = RepairTaskStateV1::Queued(QueuedRepairStateV1 {
                        queued_at_unix: queued_at,
                        sla_deadline_unix: task.sla_deadline_unix,
                    });
                    task.scheduler_notes = Some(reason.clone());
                    task.lease = None;
                    task.next_attempt_after_unix = Some(retry_after);
                    event = task.push_event(
                        RepairTaskStatusV1::Queued,
                        now_unix,
                        Some("scheduler".into()),
                        Some(reason.clone()),
                        self.event_history_limit,
                    );
                    requeued = true;
                    Ok(())
                })?;
                if let Some(proposal) = drafted {
                    report.escalated.push(proposal);
                    if let Some(event) = event {
                        report.events.push(event);
                    }
                    let queued_at = queued_at_unix(&updated.state);
                    global_sorafs_repair_otel().record_task_transition("escalated");
                    global_sorafs_repair_otel().record_lease_expired("escalated");
                    global_or_default().inc_sorafs_repair_tasks("escalated");
                    global_or_default().inc_sorafs_repair_lease_expired("escalated");
                    global_sorafs_repair_otel().record_slash_proposal("drafted");
                    global_or_default().inc_sorafs_slash_proposals("drafted");
                    self.observe_latency(queued_at, now_unix, "escalated");
                } else if requeued {
                    report.requeued.push(ticket_id.clone());
                    report.lease_expired = report.lease_expired.saturating_add(1);
                    if let Some(event) = event {
                        report.events.push(event);
                    }
                    global_sorafs_repair_otel().record_task_transition("queued");
                    global_sorafs_repair_otel().record_lease_expired("requeued");
                    global_or_default().inc_sorafs_repair_tasks("queued");
                    global_or_default().inc_sorafs_repair_lease_expired("requeued");
                }
            }

            if matches!(task.state, RepairTaskStateV1::Failed(_)) {
                let mut requeued = false;
                let mut drafted = None;
                let mut event = None;
                let updated = self.update_task_with_retry(&ticket_id, |task| {
                    requeued = false;
                    drafted = None;
                    event = None;
                    let failed_state = match &task.state {
                        RepairTaskStateV1::Failed(state) => state,
                        _ => return Ok(()),
                    };
                    let max_attempts = self.config.max_attempts();
                    if task.attempts >= max_attempts {
                        let rationale = format!(
                            "attempts {}/{} exceeded after failure",
                            task.attempts, max_attempts
                        );
                        let escalation =
                            self.apply_escalation(task, now_unix, rationale, "scheduler")?;
                        drafted = Some(escalation.proposal);
                        event = escalation.event;
                        return Ok(());
                    }
                    if let Some(retry_after) = task.next_attempt_after_unix
                        && now_unix < retry_after
                    {
                        return Ok(());
                    }
                    let queued_at = failed_state.queued_at_unix;
                    let reason = format!("retry after failure: {}", failed_state.reason);
                    task.state = RepairTaskStateV1::Queued(QueuedRepairStateV1 {
                        queued_at_unix: queued_at,
                        sla_deadline_unix: task.sla_deadline_unix,
                    });
                    task.scheduler_notes = Some(reason.clone());
                    task.lease = None;
                    task.next_attempt_after_unix = None;
                    event = task.push_event(
                        RepairTaskStatusV1::Queued,
                        now_unix,
                        Some("scheduler".into()),
                        Some(reason),
                        self.event_history_limit,
                    );
                    requeued = true;
                    Ok(())
                })?;
                if let Some(proposal) = drafted {
                    report.escalated.push(proposal);
                    if let Some(event) = event {
                        report.events.push(event);
                    }
                    let queued_at = queued_at_unix(&updated.state);
                    global_sorafs_repair_otel().record_task_transition("escalated");
                    global_or_default().inc_sorafs_repair_tasks("escalated");
                    global_sorafs_repair_otel().record_slash_proposal("drafted");
                    global_or_default().inc_sorafs_slash_proposals("drafted");
                    self.observe_latency(queued_at, now_unix, "escalated");
                } else if requeued {
                    report.requeued.push(ticket_id.clone());
                    if let Some(event) = event {
                        report.events.push(event);
                    }
                    global_sorafs_repair_otel().record_task_transition("queued");
                    global_or_default().inc_sorafs_repair_tasks("queued");
                }
            }
        }

        match self.backlog_stats(now_unix) {
            Ok(stats) => self.record_backlog_metrics(&stats),
            Err(err) => {
                warn!(?err, "failed to refresh repair backlog metrics");
            }
        }

        Ok(report)
    }

    fn resolve_governance_decision(
        &self,
        task: &mut RepairTaskInternal,
        now_unix: u64,
    ) -> Result<Option<RepairGovernanceDecision>, RepairSchedulerError> {
        if now_unix == 0 || task.governance.decision.is_some() {
            return Ok(None);
        }
        let escalated = match &task.state {
            RepairTaskStateV1::Escalated(state) => state,
            _ => return Ok(None),
        };
        let policy = persisted_task_governance_policy(task, &task.report.ticket_id)?;
        let dispute_deadline = checked_add_secs(
            escalated.escalated_at_unix,
            policy.dispute_window_secs,
            &task.report.ticket_id,
        )?;
        if now_unix < dispute_deadline {
            return Ok(None);
        }
        let approvals = u32::try_from(task.governance.approvals.len()).unwrap_or(u32::MAX);
        let rejections = u32::try_from(task.governance.rejections.len()).unwrap_or(u32::MAX);
        let total = approvals.saturating_add(rejections);
        let decision = if total < policy.minimum_voters {
            RepairGovernanceDecision::Rejected {
                decided_at_unix: dispute_deadline,
                approvals,
                rejections,
                reason: RepairGovernanceRejectReason::InsufficientQuorum,
            }
        } else if approvals == rejections {
            RepairGovernanceDecision::Rejected {
                decided_at_unix: dispute_deadline,
                approvals,
                rejections,
                reason: RepairGovernanceRejectReason::Tie,
            }
        } else {
            let ratio_bps = div_ceil_u128(
                u128::from(approvals).saturating_mul(10_000),
                u128::from(total),
            );
            if ratio_bps < u128::from(policy.quorum_bps) {
                RepairGovernanceDecision::Rejected {
                    decided_at_unix: dispute_deadline,
                    approvals,
                    rejections,
                    reason: RepairGovernanceRejectReason::QuorumNotMet,
                }
            } else if approvals < rejections {
                RepairGovernanceDecision::Rejected {
                    decided_at_unix: dispute_deadline,
                    approvals,
                    rejections,
                    reason: RepairGovernanceRejectReason::RejectMajority,
                }
            } else {
                RepairGovernanceDecision::Approved {
                    decided_at_unix: dispute_deadline,
                    approvals,
                    rejections,
                }
            }
        };
        task.governance.decision = Some(decision.clone());
        Ok(Some(decision))
    }

    /// Retrieve a repair task record by ticket id.
    pub fn task_record(
        &self,
        ticket_id: &RepairTicketId,
    ) -> Result<Option<RepairTaskRecordV1>, RepairStoreError> {
        Ok(self.store.task(ticket_id)?.map(|task| task.to_record()))
    }

    /// Retrieve a repair task snapshot by ticket id.
    pub fn task_snapshot(
        &self,
        ticket_id: &RepairTicketId,
    ) -> Result<Option<RepairTaskSnapshot>, RepairStoreError> {
        self.store
            .task(ticket_id)?
            .map(|task| task.to_snapshot())
            .transpose()
    }

    /// Mark a repair ticket as actively being addressed.
    pub fn mark_in_progress(
        &self,
        ticket_id: &RepairTicketId,
        started_at_unix: u64,
        repair_agent: Option<String>,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        Ok(self
            .mark_in_progress_with_event(ticket_id, started_at_unix, repair_agent)?
            .record)
    }

    /// Mark a repair ticket as actively being addressed, returning any event.
    pub fn mark_in_progress_with_event(
        &self,
        ticket_id: &RepairTicketId,
        started_at_unix: u64,
        repair_agent: Option<String>,
    ) -> Result<RepairTaskUpdate, RepairSchedulerError> {
        if started_at_unix == 0 {
            return Err(RepairSchedulerError::InvalidTimestamp {
                ticket_id: ticket_id.to_string(),
            });
        }
        let mut event = None;
        let task = self.update_task_with_retry(ticket_id, |task| {
            event = None;
            let queued_at = queued_at_unix(&task.state);
            if started_at_unix <= queued_at {
                return Err(RepairSchedulerError::InvalidTimestamp {
                    ticket_id: ticket_id.to_string(),
                });
            }
            match &task.state {
                RepairTaskStateV1::Queued(..) => {
                    task.state = RepairTaskStateV1::InProgress(InProgressRepairStateV1 {
                        queued_at_unix: queued_at,
                        started_at_unix,
                        repair_agent: repair_agent.clone(),
                    });
                    task.lease = None;
                    task.next_attempt_after_unix = None;
                    event = task.push_event(
                        RepairTaskStatusV1::InProgress,
                        started_at_unix,
                        repair_agent.clone(),
                        None,
                        self.event_history_limit,
                    );
                }
                _ => {
                    return Err(RepairSchedulerError::InvalidState {
                        ticket_id: ticket_id.to_string(),
                        state: format!("{:?}", task.state),
                    });
                }
            }
            Ok(())
        })?;
        if event.is_some() {
            global_sorafs_repair_otel().record_task_transition("in_progress");
            global_or_default().inc_sorafs_repair_tasks("in_progress");
        }
        Ok(RepairTaskUpdate {
            record: task.to_record(),
            event,
            slash_proposal: None,
        })
    }

    /// Mark a repair ticket as successfully resolved.
    pub fn mark_completed(
        &self,
        ticket_id: &RepairTicketId,
        completed_at_unix: u64,
        resolution_notes: Option<String>,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        Ok(self
            .mark_completed_with_event(ticket_id, completed_at_unix, resolution_notes)?
            .record)
    }

    /// Mark a repair ticket as successfully resolved, returning any event.
    pub fn mark_completed_with_event(
        &self,
        ticket_id: &RepairTicketId,
        completed_at_unix: u64,
        resolution_notes: Option<String>,
    ) -> Result<RepairTaskUpdate, RepairSchedulerError> {
        if completed_at_unix == 0 {
            return Err(RepairSchedulerError::InvalidTimestamp {
                ticket_id: ticket_id.to_string(),
            });
        }
        let mut event = None;
        let task = self.update_task_with_retry(ticket_id, |task| {
            event = None;
            let queued_at = queued_at_unix(&task.state);
            if completed_at_unix <= queued_at {
                return Err(RepairSchedulerError::InvalidTimestamp {
                    ticket_id: ticket_id.to_string(),
                });
            }
            let started_at = match &task.state {
                RepairTaskStateV1::Queued(..) => queued_at,
                RepairTaskStateV1::InProgress(InProgressRepairStateV1 {
                    started_at_unix, ..
                }) => *started_at_unix,
                _ => {
                    return Err(RepairSchedulerError::InvalidState {
                        ticket_id: ticket_id.to_string(),
                        state: format!("{:?}", task.state),
                    });
                }
            };
            task.state = RepairTaskStateV1::Completed(CompletedRepairStateV1 {
                queued_at_unix: queued_at,
                started_at_unix: started_at,
                completed_at_unix,
                resolution_notes: resolution_notes.clone(),
            });
            task.scheduler_notes = resolution_notes.clone();
            task.lease = None;
            task.next_attempt_after_unix = None;
            event = task.push_event(
                RepairTaskStatusV1::Completed,
                completed_at_unix,
                None,
                resolution_notes.clone(),
                self.event_history_limit,
            );
            Ok(())
        })?;
        let queued_at = queued_at_unix(&task.state);
        if event.is_some() {
            global_sorafs_repair_otel().record_task_transition("completed");
            global_or_default().inc_sorafs_repair_tasks("completed");
            self.observe_latency(queued_at, completed_at_unix, "completed");
        }
        Ok(RepairTaskUpdate {
            record: task.to_record(),
            event,
            slash_proposal: None,
        })
    }

    /// Mark a repair ticket as failed after an unsuccessful attempt.
    pub fn mark_failed(
        &self,
        ticket_id: &RepairTicketId,
        failed_at_unix: u64,
        reason: String,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        Ok(self
            .mark_failed_with_event(ticket_id, failed_at_unix, reason)?
            .record)
    }

    /// Mark a repair ticket as failed after an unsuccessful attempt, returning any event.
    pub fn mark_failed_with_event(
        &self,
        ticket_id: &RepairTicketId,
        failed_at_unix: u64,
        reason: String,
    ) -> Result<RepairTaskUpdate, RepairSchedulerError> {
        if failed_at_unix == 0 {
            return Err(RepairSchedulerError::InvalidTimestamp {
                ticket_id: ticket_id.to_string(),
            });
        }
        let mut escalated = false;
        let mut event = None;
        let mut slash_proposal = None;
        let task = self.update_task_with_retry(ticket_id, |task| {
            escalated = false;
            event = None;
            slash_proposal = None;
            let queued_at = queued_at_unix(&task.state);
            if failed_at_unix <= queued_at {
                return Err(RepairSchedulerError::InvalidTimestamp {
                    ticket_id: ticket_id.to_string(),
                });
            }
            task.attempts = task.attempts.saturating_add(1);
            let max_attempts = self.config.max_attempts();
            if task.attempts >= max_attempts {
                let rationale = format!(
                    "attempts {}/{} exceeded after failure",
                    task.attempts, max_attempts
                );
                let escalation =
                    self.apply_escalation(task, failed_at_unix, rationale, "scheduler")?;
                slash_proposal = Some(escalation.proposal);
                event = escalation.event;
                escalated = true;
                return Ok(());
            }
            let retry_after =
                next_attempt_after_unix(failed_at_unix, task.attempts, &self.config, ticket_id)?;
            task.state = RepairTaskStateV1::Failed(FailedRepairStateV1 {
                queued_at_unix: queued_at,
                failed_at_unix,
                reason: reason.clone(),
            });
            task.scheduler_notes = Some(reason.clone());
            task.lease = None;
            task.next_attempt_after_unix = Some(retry_after);
            event = task.push_event(
                RepairTaskStatusV1::Failed,
                failed_at_unix,
                None,
                Some(reason.clone()),
                self.event_history_limit,
            );
            Ok(())
        })?;
        let queued_at = queued_at_unix(&task.state);
        if event.is_some() {
            if escalated {
                global_sorafs_repair_otel().record_task_transition("escalated");
                global_or_default().inc_sorafs_repair_tasks("escalated");
                global_sorafs_repair_otel().record_slash_proposal("drafted");
                global_or_default().inc_sorafs_slash_proposals("drafted");
                self.observe_latency(queued_at, failed_at_unix, "escalated");
            } else {
                global_sorafs_repair_otel().record_task_transition("failed");
                global_or_default().inc_sorafs_repair_tasks("failed");
                self.observe_latency(queued_at, failed_at_unix, "failed");
            }
        }
        Ok(RepairTaskUpdate {
            record: task.to_record(),
            event,
            slash_proposal,
        })
    }

    /// Claim a repair ticket for a worker.
    pub fn claim_ticket(
        &self,
        ticket_id: &RepairTicketId,
        worker_id: &str,
        claimed_at_unix: u64,
        idempotency_key: &str,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        Ok(self
            .claim_ticket_with_event(ticket_id, worker_id, claimed_at_unix, idempotency_key)?
            .record)
    }

    /// Claim a repair ticket for a worker, returning any event.
    pub fn claim_ticket_with_event(
        &self,
        ticket_id: &RepairTicketId,
        worker_id: &str,
        claimed_at_unix: u64,
        idempotency_key: &str,
    ) -> Result<RepairTaskUpdate, RepairSchedulerError> {
        ensure_idempotency_key(idempotency_key, ticket_id)?;
        ensure_worker_field(worker_id, "worker_id", MAX_WORKER_ID_BYTES, ticket_id)?;
        if claimed_at_unix == 0 {
            return Err(RepairSchedulerError::InvalidTimestamp {
                ticket_id: ticket_id.to_string(),
            });
        }

        for _ in 0..=MAX_REPAIR_STORE_RETRIES {
            let mut task = self.load_task(ticket_id)?;
            let signature = RepairClaimSignature {
                worker_id: worker_id.to_string(),
                claimed_at_unix,
            };
            if let Some(record) = task.idempotency.claim.check_existing(
                idempotency_key,
                &signature,
                "claim",
                ticket_id,
            )? {
                return Ok(RepairTaskUpdate {
                    record,
                    event: None,
                    slash_proposal: None,
                });
            }

            match &task.state {
                RepairTaskStateV1::Queued(..) | RepairTaskStateV1::InProgress(..) => {}
                _ => {
                    return Err(RepairSchedulerError::InvalidState {
                        ticket_id: ticket_id.to_string(),
                        state: format!("{:?}", task.state),
                    });
                }
            }

            if let Some(lease) = &task.lease
                && !lease.is_expired_at(claimed_at_unix)
            {
                return Err(RepairSchedulerError::LeaseHeld {
                    ticket_id: ticket_id.to_string(),
                    worker_id: lease.worker_id.clone(),
                });
            }
            if let Some(retry_after) = task.next_attempt_after_unix
                && claimed_at_unix < retry_after
            {
                return Err(RepairSchedulerError::BackoffActive {
                    ticket_id: ticket_id.to_string(),
                    retry_after_unix: retry_after,
                });
            }

            let queued_at = queued_at_unix(&task.state);
            let min_claim_at = match &task.state {
                RepairTaskStateV1::InProgress(InProgressRepairStateV1 {
                    started_at_unix, ..
                }) => *started_at_unix,
                _ => queued_at,
            };
            if claimed_at_unix <= min_claim_at {
                return Err(RepairSchedulerError::InvalidTimestamp {
                    ticket_id: ticket_id.to_string(),
                });
            }

            let expires_at =
                checked_add_secs(claimed_at_unix, self.config.claim_ttl_secs(), ticket_id)?;
            task.state = RepairTaskStateV1::InProgress(InProgressRepairStateV1 {
                queued_at_unix: queued_at,
                started_at_unix: claimed_at_unix,
                repair_agent: Some(worker_id.to_string()),
            });
            task.lease = Some(RepairTaskLease {
                worker_id: worker_id.to_string(),
                last_heartbeat_unix: claimed_at_unix,
                expires_at_unix: expires_at,
            });
            task.next_attempt_after_unix = None;
            let event = task.push_event(
                RepairTaskStatusV1::InProgress,
                claimed_at_unix,
                Some(worker_id.to_string()),
                Some("claimed".to_string()),
                self.event_history_limit,
            );

            let record = task.to_record();
            task.idempotency
                .claim
                .remember(idempotency_key, signature, record.clone());

            let expected_revision = task.revision;
            task.revision = checked_next_revision(task.revision, ticket_id)?;
            match self
                .store
                .compare_and_set_task(ticket_id, expected_revision, task)
            {
                Ok(()) => {
                    if event.is_some() {
                        global_sorafs_repair_otel().record_task_transition("in_progress");
                        global_or_default().inc_sorafs_repair_tasks("in_progress");
                    }
                    return Ok(RepairTaskUpdate {
                        record,
                        event,
                        slash_proposal: None,
                    });
                }
                Err(RepairStoreError::Conflict { .. }) => continue,
                Err(err) => return Err(err.into()),
            }
        }
        Err(RepairSchedulerError::StoreConflict {
            ticket_id: ticket_id.to_string(),
        })
    }

    /// Record a heartbeat for a claimed repair ticket.
    pub fn heartbeat_ticket(
        &self,
        ticket_id: &RepairTicketId,
        worker_id: &str,
        heartbeat_at_unix: u64,
        idempotency_key: &str,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        ensure_idempotency_key(idempotency_key, ticket_id)?;
        ensure_worker_field(worker_id, "worker_id", MAX_WORKER_ID_BYTES, ticket_id)?;
        if heartbeat_at_unix == 0 {
            return Err(RepairSchedulerError::InvalidTimestamp {
                ticket_id: ticket_id.to_string(),
            });
        }
        for _ in 0..=MAX_REPAIR_STORE_RETRIES {
            let mut task = self.load_task(ticket_id)?;
            let signature = RepairHeartbeatSignature {
                worker_id: worker_id.to_string(),
                heartbeat_at_unix,
            };
            if let Some(record) = task.idempotency.heartbeat.check_existing(
                idempotency_key,
                &signature,
                "heartbeat",
                ticket_id,
            )? {
                return Ok(record);
            }

            match &task.state {
                RepairTaskStateV1::InProgress(..) => {}
                _ => {
                    return Err(RepairSchedulerError::InvalidState {
                        ticket_id: ticket_id.to_string(),
                        state: format!("{:?}", task.state),
                    });
                }
            }

            let lease = task
                .lease
                .as_mut()
                .ok_or_else(|| RepairSchedulerError::LeaseExpired {
                    ticket_id: ticket_id.to_string(),
                })?;
            if lease.worker_id != worker_id {
                return Err(RepairSchedulerError::WorkerMismatch {
                    ticket_id: ticket_id.to_string(),
                    worker_id: worker_id.to_string(),
                });
            }
            if heartbeat_at_unix <= lease.last_heartbeat_unix {
                return Err(RepairSchedulerError::InvalidTimestamp {
                    ticket_id: ticket_id.to_string(),
                });
            }
            if lease.is_expired_at(heartbeat_at_unix) {
                return Err(RepairSchedulerError::LeaseExpired {
                    ticket_id: ticket_id.to_string(),
                });
            }

            lease.last_heartbeat_unix = heartbeat_at_unix;
            lease.expires_at_unix = checked_add_secs(
                heartbeat_at_unix,
                self.config.heartbeat_interval_secs(),
                ticket_id,
            )?;

            let record = task.to_record();
            task.idempotency
                .heartbeat
                .remember(idempotency_key, signature, record.clone());

            let expected_revision = task.revision;
            task.revision = checked_next_revision(task.revision, ticket_id)?;
            match self
                .store
                .compare_and_set_task(ticket_id, expected_revision, task)
            {
                Ok(()) => return Ok(record),
                Err(RepairStoreError::Conflict { .. }) => continue,
                Err(err) => return Err(err.into()),
            }
        }
        Err(RepairSchedulerError::StoreConflict {
            ticket_id: ticket_id.to_string(),
        })
    }

    /// Mark a claimed repair ticket as successfully resolved.
    pub fn complete_ticket(
        &self,
        ticket_id: &RepairTicketId,
        worker_id: &str,
        completed_at_unix: u64,
        resolution_notes: Option<String>,
        idempotency_key: &str,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        Ok(self
            .complete_ticket_with_event(
                ticket_id,
                worker_id,
                completed_at_unix,
                resolution_notes,
                idempotency_key,
            )?
            .record)
    }

    /// Mark a claimed repair ticket as successfully resolved, returning any event.
    pub fn complete_ticket_with_event(
        &self,
        ticket_id: &RepairTicketId,
        worker_id: &str,
        completed_at_unix: u64,
        resolution_notes: Option<String>,
        idempotency_key: &str,
    ) -> Result<RepairTaskUpdate, RepairSchedulerError> {
        ensure_idempotency_key(idempotency_key, ticket_id)?;
        ensure_worker_field(worker_id, "worker_id", MAX_WORKER_ID_BYTES, ticket_id)?;
        ensure_optional_field(
            resolution_notes.as_deref(),
            "resolution_notes",
            MAX_REPAIR_NOTES_BYTES,
            ticket_id,
        )?;
        if completed_at_unix == 0 {
            return Err(RepairSchedulerError::InvalidTimestamp {
                ticket_id: ticket_id.to_string(),
            });
        }
        for _ in 0..=MAX_REPAIR_STORE_RETRIES {
            let mut task = self.load_task(ticket_id)?;
            let signature = RepairCompleteSignature {
                worker_id: worker_id.to_string(),
                completed_at_unix,
                resolution_notes: resolution_notes.clone(),
            };
            if let Some(record) = task.idempotency.complete.check_existing(
                idempotency_key,
                &signature,
                "complete",
                ticket_id,
            )? {
                return Ok(RepairTaskUpdate {
                    record,
                    event: None,
                    slash_proposal: None,
                });
            }

            let (queued_at, started_at) = match &task.state {
                RepairTaskStateV1::InProgress(InProgressRepairStateV1 {
                    queued_at_unix,
                    started_at_unix,
                    ..
                }) => (*queued_at_unix, *started_at_unix),
                _ => {
                    return Err(RepairSchedulerError::InvalidState {
                        ticket_id: ticket_id.to_string(),
                        state: format!("{:?}", task.state),
                    });
                }
            };

            let lease = task
                .lease
                .as_ref()
                .ok_or_else(|| RepairSchedulerError::LeaseExpired {
                    ticket_id: ticket_id.to_string(),
                })?;
            if lease.worker_id != worker_id {
                return Err(RepairSchedulerError::WorkerMismatch {
                    ticket_id: ticket_id.to_string(),
                    worker_id: worker_id.to_string(),
                });
            }
            if completed_at_unix < started_at || completed_at_unix < lease.last_heartbeat_unix {
                return Err(RepairSchedulerError::InvalidTimestamp {
                    ticket_id: ticket_id.to_string(),
                });
            }
            if lease.is_expired_at(completed_at_unix) {
                return Err(RepairSchedulerError::LeaseExpired {
                    ticket_id: ticket_id.to_string(),
                });
            }

            task.state = RepairTaskStateV1::Completed(CompletedRepairStateV1 {
                queued_at_unix: queued_at,
                started_at_unix: started_at,
                completed_at_unix,
                resolution_notes: resolution_notes.clone(),
            });
            task.scheduler_notes = resolution_notes.clone();
            task.lease = None;
            let event = task.push_event(
                RepairTaskStatusV1::Completed,
                completed_at_unix,
                Some(worker_id.to_string()),
                resolution_notes.clone(),
                self.event_history_limit,
            );

            let record = task.to_record();
            task.idempotency
                .complete
                .remember(idempotency_key, signature, record.clone());

            let expected_revision = task.revision;
            task.revision = checked_next_revision(task.revision, ticket_id)?;
            match self
                .store
                .compare_and_set_task(ticket_id, expected_revision, task)
            {
                Ok(()) => {
                    if event.is_some() {
                        global_sorafs_repair_otel().record_task_transition("completed");
                        global_or_default().inc_sorafs_repair_tasks("completed");
                        self.observe_latency(queued_at, completed_at_unix, "completed");
                    }
                    return Ok(RepairTaskUpdate {
                        record,
                        event,
                        slash_proposal: None,
                    });
                }
                Err(RepairStoreError::Conflict { .. }) => continue,
                Err(err) => return Err(err.into()),
            }
        }
        Err(RepairSchedulerError::StoreConflict {
            ticket_id: ticket_id.to_string(),
        })
    }

    /// Mark a claimed repair ticket as failed.
    pub fn fail_ticket(
        &self,
        ticket_id: &RepairTicketId,
        worker_id: &str,
        failed_at_unix: u64,
        reason: String,
        idempotency_key: &str,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        Ok(self
            .fail_ticket_with_event(
                ticket_id,
                worker_id,
                failed_at_unix,
                reason,
                idempotency_key,
            )?
            .record)
    }

    /// Mark a claimed repair ticket as failed, returning any event.
    pub fn fail_ticket_with_event(
        &self,
        ticket_id: &RepairTicketId,
        worker_id: &str,
        failed_at_unix: u64,
        reason: String,
        idempotency_key: &str,
    ) -> Result<RepairTaskUpdate, RepairSchedulerError> {
        ensure_idempotency_key(idempotency_key, ticket_id)?;
        ensure_worker_field(worker_id, "worker_id", MAX_WORKER_ID_BYTES, ticket_id)?;
        ensure_worker_field(&reason, "reason", MAX_REPAIR_NOTES_BYTES, ticket_id)?;
        if failed_at_unix == 0 {
            return Err(RepairSchedulerError::InvalidTimestamp {
                ticket_id: ticket_id.to_string(),
            });
        }
        for _ in 0..=MAX_REPAIR_STORE_RETRIES {
            let mut task = self.load_task(ticket_id)?;
            let signature = RepairFailSignature {
                worker_id: worker_id.to_string(),
                failed_at_unix,
                reason: reason.clone(),
            };
            if let Some(record) = task.idempotency.fail.check_existing(
                idempotency_key,
                &signature,
                "fail",
                ticket_id,
            )? {
                return Ok(RepairTaskUpdate {
                    record,
                    event: None,
                    slash_proposal: None,
                });
            }

            let (queued_at, started_at) = match &task.state {
                RepairTaskStateV1::InProgress(InProgressRepairStateV1 {
                    queued_at_unix,
                    started_at_unix,
                    ..
                }) => (*queued_at_unix, *started_at_unix),
                _ => {
                    return Err(RepairSchedulerError::InvalidState {
                        ticket_id: ticket_id.to_string(),
                        state: format!("{:?}", task.state),
                    });
                }
            };

            let lease = task
                .lease
                .as_ref()
                .ok_or_else(|| RepairSchedulerError::LeaseExpired {
                    ticket_id: ticket_id.to_string(),
                })?;
            if lease.worker_id != worker_id {
                return Err(RepairSchedulerError::WorkerMismatch {
                    ticket_id: ticket_id.to_string(),
                    worker_id: worker_id.to_string(),
                });
            }
            if failed_at_unix < started_at || failed_at_unix < lease.last_heartbeat_unix {
                return Err(RepairSchedulerError::InvalidTimestamp {
                    ticket_id: ticket_id.to_string(),
                });
            }
            if lease.is_expired_at(failed_at_unix) {
                return Err(RepairSchedulerError::LeaseExpired {
                    ticket_id: ticket_id.to_string(),
                });
            }

            task.attempts = task.attempts.saturating_add(1);
            let max_attempts = self.config.max_attempts();
            let (event, slash_proposal) = if task.attempts >= max_attempts {
                let rationale = format!(
                    "attempts {}/{} exceeded after failure",
                    task.attempts, max_attempts
                );
                let escalation =
                    self.apply_escalation(&mut task, failed_at_unix, rationale, worker_id)?;
                (escalation.event, Some(escalation.proposal))
            } else {
                let retry_after = next_attempt_after_unix(
                    failed_at_unix,
                    task.attempts,
                    &self.config,
                    ticket_id,
                )?;
                task.state = RepairTaskStateV1::Failed(FailedRepairStateV1 {
                    queued_at_unix: queued_at,
                    failed_at_unix,
                    reason: reason.clone(),
                });
                task.scheduler_notes = Some(reason.clone());
                task.lease = None;
                task.next_attempt_after_unix = Some(retry_after);
                (
                    task.push_event(
                        RepairTaskStatusV1::Failed,
                        failed_at_unix,
                        Some(worker_id.to_string()),
                        Some(reason.clone()),
                        self.event_history_limit,
                    ),
                    None,
                )
            };

            let record = task.to_record();
            task.idempotency
                .fail
                .remember(idempotency_key, signature, record.clone());

            let expected_revision = task.revision;
            task.revision = checked_next_revision(task.revision, ticket_id)?;
            match self
                .store
                .compare_and_set_task(ticket_id, expected_revision, task)
            {
                Ok(()) => {
                    if event.is_some() {
                        if matches!(record.state, RepairTaskStateV1::Escalated(..)) {
                            global_sorafs_repair_otel().record_task_transition("escalated");
                            global_or_default().inc_sorafs_repair_tasks("escalated");
                            global_sorafs_repair_otel().record_slash_proposal("drafted");
                            global_or_default().inc_sorafs_slash_proposals("drafted");
                            self.observe_latency(queued_at, failed_at_unix, "escalated");
                        } else {
                            global_sorafs_repair_otel().record_task_transition("failed");
                            global_or_default().inc_sorafs_repair_tasks("failed");
                            self.observe_latency(queued_at, failed_at_unix, "failed");
                        }
                    }
                    return Ok(RepairTaskUpdate {
                        record,
                        event,
                        slash_proposal,
                    });
                }
                Err(RepairStoreError::Conflict { .. }) => continue,
                Err(err) => return Err(err.into()),
            }
        }
        Err(RepairSchedulerError::StoreConflict {
            ticket_id: ticket_id.to_string(),
        })
    }

    fn ensure_por_history_match(
        &self,
        por_history_id: u64,
        submitted_at_unix: u64,
        evidence: &RepairEvidenceV1,
    ) -> Result<(), RepairSchedulerError> {
        let entry = self
            .store
            .por_history_entry(por_history_id)?
            .ok_or_else(|| RepairSchedulerError::UnknownPorHistory {
                por_history_id,
                ticket_id: evidence
                    .manifest_digest
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>(),
            })?;
        if entry.manifest_digest != evidence.manifest_digest
            || entry.provider_id != evidence.provider_id
            || submitted_at_unix < entry.decided_at
        {
            return Err(RepairSchedulerError::PorHistoryMismatch { por_history_id });
        }
        if let RepairCauseV1::PorFailure(cause) = &evidence.cause
            && (entry.challenge_id != cause.challenge_id
                || entry.failed_samples != u64::from(cause.failed_samples))
        {
            return Err(RepairSchedulerError::PorHistoryMismatch { por_history_id });
        }
        Ok(())
    }

    fn load_task(
        &self,
        ticket_id: &RepairTicketId,
    ) -> Result<RepairTaskInternal, RepairSchedulerError> {
        self.store
            .task(ticket_id)?
            .ok_or_else(|| RepairSchedulerError::UnknownTicket {
                ticket_id: ticket_id.to_string(),
            })
    }

    fn update_task_with_retry<F>(
        &self,
        ticket_id: &RepairTicketId,
        mut update: F,
    ) -> Result<RepairTaskInternal, RepairSchedulerError>
    where
        F: FnMut(&mut RepairTaskInternal) -> Result<(), RepairSchedulerError>,
    {
        for _ in 0..=MAX_REPAIR_STORE_RETRIES {
            let mut task = self.load_task(ticket_id)?;
            let previous = task.clone();
            update(&mut task)?;
            if task == previous {
                return Ok(task);
            }
            let expected_revision = task.revision;
            task.revision = checked_next_revision(task.revision, ticket_id)?;
            match self
                .store
                .compare_and_set_task(ticket_id, expected_revision, task.clone())
            {
                Ok(()) => return Ok(task),
                Err(RepairStoreError::Conflict { .. }) => continue,
                Err(err) => return Err(err.into()),
            }
        }
        Err(RepairSchedulerError::StoreConflict {
            ticket_id: ticket_id.to_string(),
        })
    }

    fn apply_escalation(
        &self,
        task: &mut RepairTaskInternal,
        escalated_at_unix: u64,
        rationale: String,
        actor: &str,
    ) -> Result<EscalationOutcome, RepairSchedulerError> {
        if matches!(task.state, RepairTaskStateV1::Escalated(_)) {
            return Err(policy_violation(
                &task.report.ticket_id,
                "repair task is already escalated",
            ));
        }
        ensure_transition_allowed(&task.state, "escalated", &task.report.ticket_id)?;
        let queued_at = queued_at_unix(&task.state);
        if escalated_at_unix <= queued_at {
            return Err(RepairSchedulerError::InvalidTimestamp {
                ticket_id: task.report.ticket_id.to_string(),
            });
        }
        checked_add_secs(
            escalated_at_unix,
            self.escalation_policy.dispute_window_secs(),
            &task.report.ticket_id,
        )?;

        let penalty = self
            .escalation_policy
            .cap_penalty(self.config.default_slash_penalty());
        let proposal = RepairSlashProposalV1 {
            version: sorafs_manifest::repair::REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: task.report.ticket_id.clone(),
            provider_id: task.report.evidence.provider_id,
            manifest_digest: task.report.evidence.manifest_digest,
            auditor_account: task.report.auditor_account.clone(),
            proposed_penalty: penalty,
            submitted_at_unix: escalated_at_unix,
            rationale: rationale.clone(),
            approval: None,
        };
        proposal
            .validate()
            .map_err(RepairSchedulerError::InvalidSlashProposal)?;

        let bytes = norito::to_bytes(&proposal)
            .map_err(|err| repair_encoding_error("slash proposal", err))?;
        let mut digest = [0u8; 32];
        digest.copy_from_slice(hash(&bytes).as_bytes());

        task.state = RepairTaskStateV1::Escalated(EscalatedRepairStateV1 {
            queued_at_unix: queued_at,
            escalated_at_unix,
            reason: rationale.clone(),
        });
        task.scheduler_notes = Some(rationale.clone());
        task.slash_proposal_digest = Some(digest);
        task.slash_proposal_bytes = Some(bytes);
        task.slash_proposal_stage = Some(RepairSlashProposalStage::Drafted);
        task.lease = None;
        task.next_attempt_after_unix = None;
        task.governance = RepairGovernanceState::default();
        task.governance_policy = Some(RepairGovernancePolicySnapshot::from_runtime(
            &self.escalation_policy,
        ));
        let event = task.push_event(
            RepairTaskStatusV1::Escalated,
            escalated_at_unix,
            Some(actor.to_string()),
            Some(rationale),
            self.event_history_limit,
        );
        Ok(EscalationOutcome { proposal, event })
    }

    fn observe_latency(&self, queued_at: u64, finished_at: u64, outcome: &'static str) {
        if finished_at <= queued_at {
            return;
        }
        let duration_secs = finished_at.saturating_sub(queued_at);
        let duration_minutes = duration_secs as f64 / 60.0;
        global_sorafs_repair_otel().record_latency(duration_minutes, outcome);
        global_or_default().observe_sorafs_repair_latency(outcome, duration_minutes);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepairTaskLease {
    worker_id: String,
    last_heartbeat_unix: u64,
    expires_at_unix: u64,
}

impl RepairTaskLease {
    fn is_expired_at(&self, now_unix: u64) -> bool {
        now_unix > self.expires_at_unix
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepairTaskIdempotency {
    claim: IdempotencyCache<RepairClaimSignature>,
    heartbeat: IdempotencyCache<RepairHeartbeatSignature>,
    complete: IdempotencyCache<RepairCompleteSignature>,
    fail: IdempotencyCache<RepairFailSignature>,
}

impl RepairTaskIdempotency {
    fn new(capacity: usize) -> Self {
        Self {
            claim: IdempotencyCache::new(capacity),
            heartbeat: IdempotencyCache::new(capacity),
            complete: IdempotencyCache::new(capacity),
            fail: IdempotencyCache::new(capacity),
        }
    }

    fn prune_to_retained_events(&mut self, events: &[RepairTaskEventV1]) {
        self.claim
            .retain(|entry| claim_idempotency_has_event(entry, events));
        let retained_claims: Vec<_> = self.claim.entries.values().cloned().collect();
        self.heartbeat
            .retain(|entry| heartbeat_idempotency_has_claim(entry, retained_claims.as_slice()));
        self.complete
            .retain(|entry| complete_idempotency_has_event(entry, events));
        self.fail
            .retain(|entry| fail_idempotency_has_evidence(entry, events));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IdempotencyCache<S> {
    entries: HashMap<String, IdempotencyEntry<S>>,
    order: VecDeque<String>,
    capacity: usize,
}

impl<S> IdempotencyCache<S> {
    fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            capacity,
        }
    }

    fn retain(&mut self, keep: impl Fn(&IdempotencyEntry<S>) -> bool) {
        self.entries.retain(|_, entry| keep(entry));
        self.order.retain(|key| self.entries.contains_key(key));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IdempotencyEntry<S> {
    signature: S,
    record: RepairTaskRecordV1,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RepairClaimSignature {
    worker_id: String,
    claimed_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RepairHeartbeatSignature {
    worker_id: String,
    heartbeat_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RepairCompleteSignature {
    worker_id: String,
    completed_at_unix: u64,
    resolution_notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct RepairFailSignature {
    worker_id: String,
    failed_at_unix: u64,
    reason: String,
}

fn claim_idempotency_has_event(
    entry: &IdempotencyEntry<RepairClaimSignature>,
    events: &[RepairTaskEventV1],
) -> bool {
    events.iter().any(|event| {
        event.status == RepairTaskStatusV1::InProgress
            && event.occurred_at_unix == entry.signature.claimed_at_unix
            && event.actor.as_deref() == Some(entry.signature.worker_id.as_str())
            && event.message.as_deref() == Some("claimed")
    })
}

fn claim_record_scheduler_context_is_valid(
    entry: &IdempotencyEntry<RepairClaimSignature>,
    events: &[RepairTaskEventV1],
    submitted_at_unix: u64,
) -> bool {
    let preceding_queue = events.iter().rev().find(|event| {
        event.status == RepairTaskStatusV1::Queued
            && event.occurred_at_unix < entry.signature.claimed_at_unix
    });
    match preceding_queue {
        Some(event) if event.occurred_at_unix == submitted_at_unix => {
            entry.record.scheduler_notes.is_none()
        }
        Some(event) => entry.record.scheduler_notes.as_ref() == event.message.as_ref(),
        None => true,
    }
}

fn heartbeat_idempotency_has_claim(
    entry: &IdempotencyEntry<RepairHeartbeatSignature>,
    claims: &[IdempotencyEntry<RepairClaimSignature>],
) -> bool {
    claims.iter().any(|claim| {
        claim.signature.worker_id == entry.signature.worker_id
            && claim.signature.claimed_at_unix < entry.signature.heartbeat_at_unix
            && claim.record == entry.record
    })
}

fn complete_idempotency_has_event(
    entry: &IdempotencyEntry<RepairCompleteSignature>,
    events: &[RepairTaskEventV1],
) -> bool {
    events.iter().any(|event| {
        event.status == RepairTaskStatusV1::Completed
            && event.occurred_at_unix == entry.signature.completed_at_unix
            && event.actor.as_deref() == Some(entry.signature.worker_id.as_str())
            && event.message.as_ref() == entry.signature.resolution_notes.as_ref()
    })
}

fn fail_idempotency_has_evidence(
    entry: &IdempotencyEntry<RepairFailSignature>,
    events: &[RepairTaskEventV1],
) -> bool {
    match &entry.record.state {
        RepairTaskStateV1::Failed(_) => events.iter().any(|event| {
            event.status == RepairTaskStatusV1::Failed
                && event.occurred_at_unix == entry.signature.failed_at_unix
                && event.actor.as_deref() == Some(entry.signature.worker_id.as_str())
                && event.message.as_deref() == Some(entry.signature.reason.as_str())
        }),
        RepairTaskStateV1::Escalated(_) => events.iter().any(|event| {
            event.status == RepairTaskStatusV1::Escalated
                && event.occurred_at_unix == entry.signature.failed_at_unix
                && event.actor.as_deref() == Some(entry.signature.worker_id.as_str())
        }),
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepairTaskInternal {
    /// Stable, non-zero identity of the subsystem event or signed receipt that
    /// originated this task.
    source_identity: [u8; 32],
    /// Monotonic revision used for compare-and-set updates.
    revision: u64,
    /// Parsed report payload.
    report: RepairReportV1,
    state: RepairTaskStateV1,
    sla_deadline_unix: Option<u64>,
    scheduler_notes: Option<String>,
    slash_proposal_digest: Option<[u8; 32]>,
    /// Norito-encoded bytes for the slash proposal when present.
    slash_proposal_bytes: Option<Vec<u8>>,
    slash_proposal_stage: Option<RepairSlashProposalStage>,
    governance: RepairGovernanceState,
    governance_policy: Option<RepairGovernancePolicySnapshot>,
    lease: Option<RepairTaskLease>,
    idempotency: RepairTaskIdempotency,
    attempts: u32,
    next_attempt_after_unix: Option<u64>,
    /// Number of oldest audit events pruned from the retained suffix.
    events_dropped: u64,
    /// Event log for auditability.
    events: Vec<RepairTaskEventV1>,
}

impl RepairTaskInternal {
    fn to_record(&self) -> RepairTaskRecordV1 {
        RepairTaskRecordV1 {
            version: REPAIR_TASK_VERSION_V1,
            ticket_id: self.report.ticket_id.clone(),
            manifest_digest: self.report.evidence.manifest_digest,
            provider_id: self.report.evidence.provider_id,
            auditor_account: self.report.auditor_account.clone(),
            state: self.state.clone(),
            por_history_id: self.report.evidence.por_history_id,
            sla_deadline_unix: self.sla_deadline_unix,
            scheduler_notes: self.scheduler_notes.clone(),
            slash_proposal_digest: self.slash_proposal_digest,
        }
    }

    fn decoded_slash_proposal(&self) -> Result<Option<RepairSlashProposalV1>, RepairStoreError> {
        let (expected_digest, bytes) = match (
            &self.slash_proposal_digest,
            &self.slash_proposal_bytes,
            self.slash_proposal_stage,
        ) {
            (None, None, None) => return Ok(None),
            (Some(expected_digest), Some(bytes), Some(_)) => (expected_digest, bytes),
            _ => {
                return Err(RepairStoreError::Other(format!(
                    "repair task `{}` must persist slash proposal bytes, digest, and publication stage together",
                    self.report.ticket_id
                )));
            }
        };
        if bytes.len() > MAX_CANONICAL_REPAIR_SLASH_PROPOSAL_BYTES {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` slash proposal is {} bytes, exceeding fixed limit {}",
                self.report.ticket_id,
                bytes.len(),
                MAX_CANONICAL_REPAIR_SLASH_PROPOSAL_BYTES
            )));
        }
        let encoded_len = u64::try_from(bytes.len()).map_err(|_| {
            RepairStoreError::Other(format!(
                "repair task `{}` slash proposal length does not fit u64",
                self.report.ticket_id
            ))
        })?;
        validate_bounded_uncompressed_norito(
            bytes,
            encoded_len,
            "persisted repair slash proposal",
        )?;
        let proposal: RepairSlashProposalV1 =
            norito::decode_from_bytes_with_limits(bytes, repair_slash_proposal_decode_limits())
                .map_err(|err| {
                    RepairStoreError::Other(format!(
                        "repair task `{}` contains an invalid slash proposal: {err}",
                        self.report.ticket_id
                    ))
                })?;
        proposal.validate().map_err(|err| {
            RepairStoreError::Other(format!(
                "repair task `{}` contains an invalid slash proposal: {err}",
                self.report.ticket_id
            ))
        })?;
        let canonical = norito::to_bytes(&proposal).map_err(|err| {
            RepairStoreError::Other(format!(
                "repair task `{}` slash proposal could not be re-encoded: {err}",
                self.report.ticket_id
            ))
        })?;
        if canonical.as_slice() != bytes.as_slice() || checkpoint_digest(bytes) != *expected_digest
        {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` has non-canonical or digest-mismatched slash proposal bytes",
                self.report.ticket_id
            )));
        }
        if proposal.approval.is_some() {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` contains an unauthenticated embedded approval summary",
                self.report.ticket_id
            )));
        }
        Ok(Some(proposal))
    }

    fn to_snapshot(&self) -> Result<RepairTaskSnapshot, RepairStoreError> {
        Ok(RepairTaskSnapshot {
            record: self.to_record(),
            events_dropped: self.events_dropped,
            events: self.events.clone(),
            slash_proposal: self.decoded_slash_proposal()?,
            slash_proposal_stage: self.slash_proposal_stage,
        })
    }

    fn push_event(
        &mut self,
        status: RepairTaskStatusV1,
        occurred_at_unix: u64,
        actor: Option<String>,
        message: Option<String>,
        limit: usize,
    ) -> Option<RepairTaskEventV1> {
        if limit == 0 {
            return None;
        }
        let event = RepairTaskEventV1 {
            version: REPAIR_TASK_EVENT_VERSION_V1,
            ticket_id: self.report.ticket_id.clone(),
            manifest_digest: self.report.evidence.manifest_digest,
            provider_id: self.report.evidence.provider_id,
            status,
            occurred_at_unix,
            actor,
            message,
        };
        if let Err(err) = event.validate() {
            warn!(
                ?err,
                ticket_id = %self.report.ticket_id,
                "skipping invalid repair task event"
            );
            return None;
        }
        self.events.push(event.clone());
        if self.events.len() > limit {
            let excess = self.events.len().saturating_sub(limit);
            self.events.drain(0..excess);
            self.events_dropped = self
                .events_dropped
                .saturating_add(u64::try_from(excess).unwrap_or(u64::MAX));
            self.idempotency
                .prune_to_retained_events(self.events.as_slice());
        }
        Some(event)
    }

    fn validate_persisted(
        &self,
        entry_limit: usize,
        escalation_policy: &RepairEscalationPolicy,
    ) -> Result<(), RepairStoreError> {
        if self.source_identity == [0; 32] {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` has a zero source identity",
                self.report.ticket_id
            )));
        }
        if self.events.len() > entry_limit
            || self.idempotency.claim.entries.len() > entry_limit
            || self.idempotency.heartbeat.entries.len() > entry_limit
            || self.idempotency.complete.entries.len() > entry_limit
            || self.idempotency.fail.entries.len() > entry_limit
        {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` nested history exceeds entry limit {entry_limit}",
                self.report.ticket_id
            )));
        }
        self.report.validate().map_err(|err| {
            RepairStoreError::Other(format!("invalid persisted repair report: {err}"))
        })?;
        self.state.validate().map_err(|err| {
            RepairStoreError::Other(format!("invalid persisted repair state: {err}"))
        })?;
        self.to_record().validate().map_err(|err| {
            RepairStoreError::Other(format!("invalid persisted repair task record: {err}"))
        })?;

        let queued_at = queued_at_unix(&self.state);
        if queued_at != self.report.submitted_at_unix {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` queued timestamp does not match its report",
                self.report.ticket_id
            )));
        }
        let expected_sla_deadline =
            queued_at
                .checked_add(DEFAULT_REPAIR_SLA_SECS)
                .ok_or_else(|| {
                    RepairStoreError::Other(format!(
                        "repair task `{}` SLA deadline overflows",
                        self.report.ticket_id
                    ))
                })?;
        if self.sla_deadline_unix != Some(expected_sla_deadline) {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` has an absent or inconsistent SLA deadline",
                self.report.ticket_id
            )));
        }
        if let RepairTaskStateV1::Queued(state) = &self.state
            && state.sla_deadline_unix != self.sla_deadline_unix
        {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` has inconsistent queued SLA deadlines",
                self.report.ticket_id
            )));
        }

        let effective_governance_policy = if matches!(self.state, RepairTaskStateV1::Escalated(_)) {
            let policy = self.governance_policy.clone().ok_or_else(|| {
                RepairStoreError::Other(format!(
                    "escalated repair task `{}` is missing its governance policy snapshot",
                    self.report.ticket_id
                ))
            })?;
            policy.validate()?;
            policy
        } else {
            if self.governance_policy.is_some() {
                return Err(RepairStoreError::Other(format!(
                    "non-escalated repair task `{}` must not retain a governance policy snapshot",
                    self.report.ticket_id
                )));
            }
            RepairGovernancePolicySnapshot::from_runtime(escalation_policy)
        };

        let slash_proposal = self.decoded_slash_proposal()?;
        if let Some(proposal) = slash_proposal.as_ref()
            && (proposal.ticket_id != self.report.ticket_id
                || proposal.manifest_digest != self.report.evidence.manifest_digest
                || proposal.provider_id != self.report.evidence.provider_id
                || proposal.auditor_account != self.report.auditor_account
                || proposal.proposed_penalty > effective_governance_policy.max_penalty
                || !matches!(self.state, RepairTaskStateV1::Escalated(_))
                || proposal.submitted_at_unix != queued_state_transition_timestamp(&self.state))
        {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` has an inconsistent slash proposal",
                self.report.ticket_id
            )));
        }
        if matches!(self.state, RepairTaskStateV1::Escalated(_)) && slash_proposal.is_none() {
            return Err(RepairStoreError::Other(format!(
                "escalated repair task `{}` is missing its slash proposal",
                self.report.ticket_id
            )));
        }

        self.validate_persisted_lease()?;
        self.validate_persisted_governance(entry_limit, &effective_governance_policy)?;
        self.validate_persisted_events()?;
        self.validate_persisted_idempotency_history()?;
        Ok(())
    }

    fn validate_persisted_idempotency_history(&self) -> Result<(), RepairStoreError> {
        let common_record_is_valid = |record: &RepairTaskRecordV1| {
            record.sla_deadline_unix == self.sla_deadline_unix
                && record.por_history_id == self.report.evidence.por_history_id
                && queued_at_unix(&record.state) == self.report.submitted_at_unix
        };
        if self.idempotency.claim.entries.values().any(|entry| {
            !common_record_is_valid(&entry.record)
                || entry.record.slash_proposal_digest.is_some()
                || !claim_record_scheduler_context_is_valid(
                    entry,
                    self.events.as_slice(),
                    self.report.submitted_at_unix,
                )
                || !claim_idempotency_has_event(entry, self.events.as_slice())
        }) {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` has a claim idempotency result without retained transition evidence",
                self.report.ticket_id
            )));
        }
        let claims: Vec<_> = self.idempotency.claim.entries.values().cloned().collect();
        if self.idempotency.heartbeat.entries.values().any(|entry| {
            !common_record_is_valid(&entry.record)
                || entry.record.slash_proposal_digest.is_some()
                || !heartbeat_idempotency_has_claim(entry, claims.as_slice())
        }) {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` has a heartbeat idempotency result without a retained claim",
                self.report.ticket_id
            )));
        }
        if self.idempotency.complete.entries.values().any(|entry| {
            !common_record_is_valid(&entry.record)
                || entry.record != self.to_record()
                || !complete_idempotency_has_event(entry, self.events.as_slice())
        }) {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` has an unreachable completion idempotency result",
                self.report.ticket_id
            )));
        }
        if self.idempotency.fail.entries.values().any(|entry| {
            let record_is_reachable = match &entry.record.state {
                RepairTaskStateV1::Failed(state) => {
                    entry.record.scheduler_notes.as_deref() == Some(state.reason.as_str())
                        && entry.record.slash_proposal_digest.is_none()
                }
                RepairTaskStateV1::Escalated(_) => entry.record == self.to_record(),
                _ => false,
            };
            !common_record_is_valid(&entry.record)
                || !record_is_reachable
                || !fail_idempotency_has_evidence(entry, self.events.as_slice())
        }) {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` has a failure idempotency result without retained transition evidence",
                self.report.ticket_id
            )));
        }
        Ok(())
    }

    fn validate_persisted_lease(&self) -> Result<(), RepairStoreError> {
        match (&self.state, &self.lease) {
            (RepairTaskStateV1::InProgress(state), Some(lease)) => {
                if lease.worker_id.is_empty()
                    || lease.worker_id.len() > MAX_WORKER_ID_BYTES
                    || state.repair_agent.as_deref() != Some(lease.worker_id.as_str())
                    || lease.last_heartbeat_unix < state.started_at_unix
                    || lease.expires_at_unix <= lease.last_heartbeat_unix
                {
                    return Err(RepairStoreError::Other(format!(
                        "repair task `{}` has an invalid worker lease",
                        self.report.ticket_id
                    )));
                }
            }
            (RepairTaskStateV1::InProgress(_), None) => {}
            (_, Some(_)) => {
                return Err(RepairStoreError::Other(format!(
                    "non-active repair task `{}` must not retain a worker lease",
                    self.report.ticket_id
                )));
            }
            (_, None) => {}
        }
        if self.next_attempt_after_unix.is_some()
            && !matches!(
                self.state,
                RepairTaskStateV1::Queued(_) | RepairTaskStateV1::Failed(_)
            )
        {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` has retry backoff outside the queued or failed state",
                self.report.ticket_id
            )));
        }
        if let (RepairTaskStateV1::Failed(state), Some(retry_at)) =
            (&self.state, self.next_attempt_after_unix)
            && retry_at <= state.failed_at_unix
        {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` has an invalid retry backoff",
                self.report.ticket_id
            )));
        }
        if matches!(self.state, RepairTaskStateV1::Failed(_))
            && (self.attempts == 0 || self.next_attempt_after_unix.is_none())
        {
            return Err(RepairStoreError::Other(format!(
                "failed repair task `{}` must retain its attempt and retry state",
                self.report.ticket_id
            )));
        }
        Ok(())
    }

    fn validate_persisted_governance(
        &self,
        entry_limit: usize,
        escalation_policy: &RepairGovernancePolicySnapshot,
    ) -> Result<(), RepairStoreError> {
        let vote_count = self
            .governance
            .approvals
            .len()
            .checked_add(self.governance.rejections.len())
            .ok_or_else(|| {
                RepairStoreError::Other("repair governance vote count overflow".to_owned())
            })?;
        if vote_count > entry_limit {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` governance votes exceed entry limit {entry_limit}",
                self.report.ticket_id
            )));
        }
        ensure_strictly_sorted_by(
            &self.governance.approvals,
            |left, right| left.voter_id.cmp(&right.voter_id),
            "governance approval votes",
        )?;
        ensure_strictly_sorted_by(
            &self.governance.rejections,
            |left, right| left.voter_id.cmp(&right.voter_id),
            "governance rejection votes",
        )?;
        for vote in self
            .governance
            .approvals
            .iter()
            .chain(&self.governance.rejections)
        {
            if vote.voter_id.is_empty()
                || vote.voter_id.len() > MAX_GOVERNANCE_ACTOR_BYTES
                || vote.voted_at_unix == 0
            {
                return Err(RepairStoreError::Other(format!(
                    "repair task `{}` has an invalid governance vote",
                    self.report.ticket_id
                )));
            }
        }
        let mut approval_index = 0;
        let mut rejection_index = 0;
        while approval_index < self.governance.approvals.len()
            && rejection_index < self.governance.rejections.len()
        {
            match self.governance.approvals[approval_index]
                .voter_id
                .cmp(&self.governance.rejections[rejection_index].voter_id)
            {
                std::cmp::Ordering::Less => approval_index += 1,
                std::cmp::Ordering::Greater => rejection_index += 1,
                std::cmp::Ordering::Equal => {
                    return Err(RepairStoreError::Other(format!(
                        "repair task `{}` has a voter on both sides",
                        self.report.ticket_id
                    )));
                }
            }
        }
        if (!self.governance.approvals.is_empty()
            || !self.governance.rejections.is_empty()
            || self.governance.decision.is_some())
            && !matches!(self.state, RepairTaskStateV1::Escalated(_))
        {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` has governance state before escalation",
                self.report.ticket_id
            )));
        }
        if let RepairTaskStateV1::Escalated(escalated) = &self.state {
            let dispute_deadline = escalated
                .escalated_at_unix
                .checked_add(escalation_policy.dispute_window_secs)
                .ok_or_else(|| {
                    RepairStoreError::Other(format!(
                        "repair task `{}` dispute deadline overflow",
                        self.report.ticket_id
                    ))
                })?;
            if self
                .governance
                .approvals
                .iter()
                .chain(&self.governance.rejections)
                .any(|vote| {
                    vote.voted_at_unix <= escalated.escalated_at_unix
                        || vote.voted_at_unix > dispute_deadline
                })
            {
                return Err(RepairStoreError::Other(format!(
                    "repair task `{}` has a governance vote outside the dispute window",
                    self.report.ticket_id
                )));
            }
            let vector_approvals =
                u32::try_from(self.governance.approvals.len()).map_err(|_| {
                    RepairStoreError::Other(
                        "repair approval vote count does not fit u32".to_owned(),
                    )
                })?;
            let vector_rejections =
                u32::try_from(self.governance.rejections.len()).map_err(|_| {
                    RepairStoreError::Other(
                        "repair rejection vote count does not fit u32".to_owned(),
                    )
                })?;
            let expected_approvals = vector_approvals;
            let expected_rejections = vector_rejections;
            let decision_at = dispute_deadline;
            let expected_rejection = expected_governance_rejection(
                vector_approvals,
                vector_rejections,
                escalation_policy,
            );
            if let Some(decision) = &self.governance.decision {
                let valid = match decision {
                    RepairGovernanceDecision::Approved {
                        decided_at_unix,
                        approvals,
                        rejections,
                    } => {
                        expected_rejection.is_none()
                            && *decided_at_unix == decision_at
                            && *approvals == expected_approvals
                            && *rejections == expected_rejections
                    }
                    RepairGovernanceDecision::Rejected {
                        decided_at_unix,
                        approvals,
                        rejections,
                        reason,
                    } => {
                        expected_rejection == Some(*reason)
                            && *decided_at_unix == decision_at
                            && *approvals == expected_approvals
                            && *rejections == expected_rejections
                    }
                    RepairGovernanceDecision::Appealed {
                        approved_at_unix,
                        appealed_at_unix,
                        approvals,
                        rejections,
                        appellant,
                        reason,
                        ..
                    } => {
                        let appeal_deadline =
                            decision_at.checked_add(escalation_policy.appeal_window_secs);
                        expected_rejection.is_none()
                            && *approvals == expected_approvals
                            && *rejections == expected_rejections
                            && *approved_at_unix == decision_at
                            && *appealed_at_unix > *approved_at_unix
                            && appeal_deadline.is_some_and(|deadline| *appealed_at_unix <= deadline)
                            && !appellant.is_empty()
                            && appellant.len() <= MAX_GOVERNANCE_ACTOR_BYTES
                            && reason.as_ref().is_none_or(|reason| {
                                !reason.is_empty() && reason.len() <= MAX_GOVERNANCE_REASON_BYTES
                            })
                    }
                };
                if !valid {
                    return Err(RepairStoreError::Other(format!(
                        "repair task `{}` has an invalid governance decision",
                        self.report.ticket_id
                    )));
                }
            }
        }
        Ok(())
    }

    fn validate_persisted_events(&self) -> Result<(), RepairStoreError> {
        if self.events.is_empty() {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` is missing its retained event suffix",
                self.report.ticket_id
            )));
        }
        let retained = u64::try_from(self.events.len()).map_err(|_| {
            RepairStoreError::Other("repair event count does not fit u64".to_owned())
        })?;
        let total_events = self
            .events_dropped
            .checked_add(retained)
            .ok_or_else(|| RepairStoreError::Other("repair event count overflow".to_owned()))?;
        if total_events > self.revision.saturating_add(1) {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` event history exceeds its revision",
                self.report.ticket_id
            )));
        }
        for event in &self.events {
            event.validate().map_err(|err| {
                RepairStoreError::Other(format!(
                    "repair task `{}` contains an invalid event: {err}",
                    self.report.ticket_id
                ))
            })?;
            if event.ticket_id != self.report.ticket_id
                || event.manifest_digest != self.report.evidence.manifest_digest
                || event.provider_id != self.report.evidence.provider_id
                || event.occurred_at_unix < self.report.submitted_at_unix
                || matches!(event.status, RepairTaskStatusV1::Verifying)
            {
                return Err(RepairStoreError::Other(format!(
                    "repair task `{}` contains an event with inconsistent identity or status",
                    self.report.ticket_id
                )));
            }
        }
        if self.events.windows(2).any(|window| {
            window[0].occurred_at_unix > window[1].occurred_at_unix
                || !valid_retained_event_transition(window[0].status, window[1].status)
        }) {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` contains an invalid retained event transition",
                self.report.ticket_id
            )));
        }
        if self.events_dropped == 0 {
            let first = &self.events[0];
            if first.status != RepairTaskStatusV1::Queued
                || first.occurred_at_unix != self.report.submitted_at_unix
                || first.actor.as_deref() != Some(self.report.auditor_account.as_str())
                || first.message.as_ref()
                    != self
                        .report
                        .notes
                        .as_ref()
                        .or(self.report.evidence.notes.as_ref())
            {
                return Err(RepairStoreError::Other(format!(
                    "repair task `{}` has a forged initial event",
                    self.report.ticket_id
                )));
            }
        }
        for (index, event) in self.events.iter().enumerate() {
            let is_initial = self.events_dropped == 0 && index == 0;
            let payload_shape_is_valid = match event.status {
                RepairTaskStatusV1::Queued if is_initial => true,
                RepairTaskStatusV1::Queued => {
                    event.actor.as_deref() == Some("scheduler") && event.message.is_some()
                }
                RepairTaskStatusV1::InProgress => match event.message.as_deref() {
                    Some("claimed") => event.actor.as_deref().is_some_and(|actor| {
                        self.idempotency.claim.entries.values().any(|entry| {
                            entry.signature.worker_id == actor
                                && entry.signature.claimed_at_unix == event.occurred_at_unix
                        })
                    }),
                    None => true,
                    Some(_) => false,
                },
                RepairTaskStatusV1::Failed => {
                    event.message.is_some()
                        && event.actor.as_deref().is_none_or(|actor| {
                            self.idempotency.fail.entries.values().any(|entry| {
                                entry.signature.worker_id == actor
                                    && entry.signature.failed_at_unix == event.occurred_at_unix
                                    && event.message.as_deref()
                                        == Some(entry.signature.reason.as_str())
                            })
                        })
                }
                RepairTaskStatusV1::Completed | RepairTaskStatusV1::Escalated => true,
                RepairTaskStatusV1::Verifying => false,
            };
            if !payload_shape_is_valid {
                return Err(RepairStoreError::Other(format!(
                    "repair task `{}` contains an event with an invalid actor or message shape",
                    self.report.ticket_id
                )));
            }
        }
        for transition in self.events.windows(2) {
            let previous = &transition[0];
            let next = &transition[1];
            let messages_reconcile = match (previous.status, next.status) {
                (RepairTaskStatusV1::InProgress, RepairTaskStatusV1::Queued) => {
                    next.message.as_deref() == Some("lease expired; requeued")
                }
                (RepairTaskStatusV1::Failed, RepairTaskStatusV1::Queued) => {
                    previous.message.as_deref().is_some_and(|reason| {
                        next.message.as_deref()
                            == Some(format!("retry after failure: {reason}").as_str())
                    })
                }
                _ => true,
            };
            if !messages_reconcile {
                return Err(RepairStoreError::Other(format!(
                    "repair task `{}` retained transition messages do not reconcile",
                    self.report.ticket_id
                )));
            }
        }
        let final_event = self.events.last().ok_or_else(|| {
            RepairStoreError::Other("repair event suffix unexpectedly empty".to_owned())
        })?;
        if final_event.status != repair_task_status(&self.state) {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` final event does not match its state",
                self.report.ticket_id
            )));
        }
        let final_state_timestamp = match &self.state {
            RepairTaskStateV1::Queued(_) => None,
            RepairTaskStateV1::InProgress(state) => Some(state.started_at_unix),
            RepairTaskStateV1::Completed(state) => Some(state.completed_at_unix),
            RepairTaskStateV1::Failed(state) => Some(state.failed_at_unix),
            RepairTaskStateV1::Escalated(state) => Some(state.escalated_at_unix),
        };
        if final_state_timestamp.is_some_and(|timestamp| timestamp != final_event.occurred_at_unix)
        {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` final event timestamp does not match its state",
                self.report.ticket_id
            )));
        }
        let final_payload_is_valid = match &self.state {
            RepairTaskStateV1::Queued(_) => {
                if self.revision == 0 {
                    true
                } else {
                    final_event.actor.as_deref() == Some("scheduler")
                        && final_event.message.as_ref() == self.scheduler_notes.as_ref()
                }
            }
            RepairTaskStateV1::InProgress(state) => match &self.lease {
                Some(lease) => {
                    final_event.actor.as_deref() == Some(lease.worker_id.as_str())
                        && final_event.message.as_deref() == Some("claimed")
                }
                None => {
                    final_event.actor.as_deref() == state.repair_agent.as_deref()
                        && final_event.message.is_none()
                }
            },
            RepairTaskStateV1::Completed(state) => {
                let actor_is_valid = match final_event.actor.as_deref() {
                    None => true,
                    Some(actor) => self.idempotency.complete.entries.values().any(|entry| {
                        entry.signature.worker_id == actor && entry.record.state == self.state
                    }),
                };
                actor_is_valid && final_event.message.as_ref() == state.resolution_notes.as_ref()
            }
            RepairTaskStateV1::Failed(state) => {
                let actor_is_valid = match final_event.actor.as_deref() {
                    None => true,
                    Some(actor) => self.idempotency.fail.entries.values().any(|entry| {
                        entry.signature.worker_id == actor && entry.record.state == self.state
                    }),
                };
                actor_is_valid && final_event.message.as_deref() == Some(state.reason.as_str())
            }
            RepairTaskStateV1::Escalated(state) => {
                let actor = final_event.actor.as_deref();
                let actor_is_worker = actor.is_some_and(|actor| {
                    self.idempotency.fail.entries.values().any(|entry| {
                        entry.signature.worker_id == actor && entry.record.state == self.state
                    })
                });
                (actor == Some("scheduler")
                    || actor == Some(self.report.auditor_account.as_str())
                    || actor_is_worker)
                    && final_event.message.as_deref() == Some(state.reason.as_str())
            }
        };
        if !final_payload_is_valid {
            return Err(RepairStoreError::Other(format!(
                "repair task `{}` final event actor or message does not match its state",
                self.report.ticket_id
            )));
        }
        Ok(())
    }
}

fn expected_governance_rejection(
    approvals: u32,
    rejections: u32,
    policy: &RepairGovernancePolicySnapshot,
) -> Option<RepairGovernanceRejectReason> {
    let total = approvals.saturating_add(rejections);
    if total < policy.minimum_voters {
        return Some(RepairGovernanceRejectReason::InsufficientQuorum);
    }
    if approvals == rejections {
        return Some(RepairGovernanceRejectReason::Tie);
    }
    let ratio_bps = div_ceil_u128(
        u128::from(approvals).saturating_mul(10_000),
        u128::from(total),
    );
    if ratio_bps < u128::from(policy.quorum_bps) {
        return Some(RepairGovernanceRejectReason::QuorumNotMet);
    }
    if approvals < rejections {
        return Some(RepairGovernanceRejectReason::RejectMajority);
    }
    None
}

fn valid_retained_event_transition(from: RepairTaskStatusV1, to: RepairTaskStatusV1) -> bool {
    matches!(
        (from, to),
        (RepairTaskStatusV1::Queued, RepairTaskStatusV1::InProgress)
            | (RepairTaskStatusV1::Queued, RepairTaskStatusV1::Completed)
            | (RepairTaskStatusV1::Queued, RepairTaskStatusV1::Failed)
            | (RepairTaskStatusV1::Queued, RepairTaskStatusV1::Escalated)
            | (
                RepairTaskStatusV1::InProgress,
                RepairTaskStatusV1::Completed
            )
            | (RepairTaskStatusV1::InProgress, RepairTaskStatusV1::Failed)
            | (
                RepairTaskStatusV1::InProgress,
                RepairTaskStatusV1::Escalated
            )
            | (
                RepairTaskStatusV1::InProgress,
                RepairTaskStatusV1::InProgress
            )
            | (RepairTaskStatusV1::InProgress, RepairTaskStatusV1::Queued)
            | (RepairTaskStatusV1::Failed, RepairTaskStatusV1::Queued)
            | (RepairTaskStatusV1::Failed, RepairTaskStatusV1::Failed)
            | (RepairTaskStatusV1::Failed, RepairTaskStatusV1::Escalated)
            | (RepairTaskStatusV1::Escalated, RepairTaskStatusV1::Escalated)
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PorHistoryObservation {
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    challenge_id: [u8; 32],
    decided_at: u64,
    failed_samples: u64,
}

impl PorHistoryObservation {
    fn validate_persisted(&self) -> Result<(), RepairStoreError> {
        if self.manifest_digest == [0; 32]
            || self.provider_id == [0; 32]
            || self.challenge_id == [0; 32]
            || self.decided_at == 0
            || self.failed_samples == 0
        {
            return Err(RepairStoreError::Other(
                "PoR repair history observation contains zero-valued required fields".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct PorHistoryEntry {
    id: u64,
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    challenge_id: [u8; 32],
    decided_at: u64,
    failed_samples: u64,
}

impl PorHistoryEntry {
    fn from_observation(id: u64, observation: PorHistoryObservation) -> Self {
        Self {
            id,
            manifest_digest: observation.manifest_digest,
            provider_id: observation.provider_id,
            challenge_id: observation.challenge_id,
            decided_at: observation.decided_at,
            failed_samples: observation.failed_samples,
        }
    }

    fn validate_persisted(&self) -> Result<(), RepairStoreError> {
        if self.id == 0 {
            return Err(RepairStoreError::Other(
                "PoR repair history id must be greater than zero".to_owned(),
            ));
        }
        self.observation().validate_persisted()
    }

    fn observation(&self) -> PorHistoryObservation {
        PorHistoryObservation {
            manifest_digest: self.manifest_digest,
            provider_id: self.provider_id,
            challenge_id: self.challenge_id,
            decided_at: self.decided_at,
            failed_samples: self.failed_samples,
        }
    }

    fn matches_observation(&self, observation: &PorHistoryObservation) -> bool {
        self.observation() == *observation
    }
}

fn validate_persisted_por_history_binding(
    report: &RepairReportV1,
    por_history: &BTreeMap<u64, PorHistoryEntry>,
) -> Result<(), RepairStoreError> {
    match (&report.evidence.cause, report.evidence.por_history_id) {
        (RepairCauseV1::PorFailure(cause), Some(history_id)) => {
            let entry = por_history.get(&history_id).ok_or_else(|| {
                RepairStoreError::Other(format!(
                    "PoR failure repair task `{}` references unknown history entry {history_id}",
                    report.ticket_id
                ))
            })?;
            if entry.manifest_digest != report.evidence.manifest_digest
                || entry.provider_id != report.evidence.provider_id
                || entry.challenge_id != cause.challenge_id
                || entry.failed_samples != u64::from(cause.failed_samples)
            {
                return Err(RepairStoreError::Other(format!(
                    "PoR failure repair task `{}` does not match history entry {history_id}",
                    report.ticket_id
                )));
            }
        }
        (RepairCauseV1::PorFailure(_), None) => {
            return Err(RepairStoreError::Other(format!(
                "PoR failure repair task `{}` is missing its history reference",
                report.ticket_id
            )));
        }
        (_, Some(history_id)) => {
            return Err(RepairStoreError::Other(format!(
                "non-PoR repair task `{}` references history entry {history_id}",
                report.ticket_id
            )));
        }
        (_, None) => {}
    }
    Ok(())
}

fn queued_at_unix(state: &RepairTaskStateV1) -> u64 {
    match state {
        RepairTaskStateV1::Queued(QueuedRepairStateV1 { queued_at_unix, .. })
        | RepairTaskStateV1::InProgress(InProgressRepairStateV1 { queued_at_unix, .. })
        | RepairTaskStateV1::Completed(CompletedRepairStateV1 { queued_at_unix, .. })
        | RepairTaskStateV1::Failed(FailedRepairStateV1 { queued_at_unix, .. })
        | RepairTaskStateV1::Escalated(EscalatedRepairStateV1 { queued_at_unix, .. }) => {
            *queued_at_unix
        }
    }
}

fn queued_state_transition_timestamp(state: &RepairTaskStateV1) -> u64 {
    match state {
        RepairTaskStateV1::Queued(state) => state.queued_at_unix,
        RepairTaskStateV1::InProgress(state) => state.started_at_unix,
        RepairTaskStateV1::Completed(state) => state.completed_at_unix,
        RepairTaskStateV1::Failed(state) => state.failed_at_unix,
        RepairTaskStateV1::Escalated(state) => state.escalated_at_unix,
    }
}

fn repair_task_status(state: &RepairTaskStateV1) -> RepairTaskStatusV1 {
    match state {
        RepairTaskStateV1::Queued(..) => RepairTaskStatusV1::Queued,
        RepairTaskStateV1::InProgress(..) => RepairTaskStatusV1::InProgress,
        RepairTaskStateV1::Completed(..) => RepairTaskStatusV1::Completed,
        RepairTaskStateV1::Failed(..) => RepairTaskStatusV1::Failed,
        RepairTaskStateV1::Escalated(..) => RepairTaskStatusV1::Escalated,
    }
}

fn repair_severity_score(cause: &RepairCauseV1) -> (u8, u64) {
    match cause {
        RepairCauseV1::PorFailure(cause) => (3, u64::from(cause.failed_samples)),
        RepairCauseV1::PdpFailure(cause) => (3, u64::from(cause.failed_samples)),
        RepairCauseV1::ReplicaShortfall(cause) => (2, u64::from(cause.missing_chunks)),
        RepairCauseV1::LatencySla(cause) => (1, u64::from(cause.observed_latency_ms)),
        RepairCauseV1::Manual(_) => (0, 0),
    }
}

fn backoff_secs(attempts: u32, config: &RepairConfig) -> u64 {
    if attempts == 0 {
        return 0;
    }
    let base = config.backoff_initial_secs();
    let max = config.backoff_max_secs();
    let shift = attempts.saturating_sub(1).min(30);
    let scaled = base.saturating_mul(1u64 << shift);
    scaled.min(max)
}

fn next_attempt_after_unix(
    failed_at_unix: u64,
    attempts: u32,
    config: &RepairConfig,
    ticket_id: &RepairTicketId,
) -> Result<u64, RepairSchedulerError> {
    let delay = backoff_secs(attempts, config);
    checked_add_secs(failed_at_unix, delay, ticket_id)
}

fn sort_repair_task_records(records: &mut [RepairTaskRecordV1]) {
    records.sort_by(|left, right| {
        let left_deadline = left.sla_deadline_unix.unwrap_or(u64::MAX);
        let right_deadline = right.sla_deadline_unix.unwrap_or(u64::MAX);
        left_deadline
            .cmp(&right_deadline)
            .then_with(|| queued_at_unix(&left.state).cmp(&queued_at_unix(&right.state)))
            .then_with(|| left.manifest_digest.cmp(&right.manifest_digest))
            .then_with(|| left.ticket_id.0.cmp(&right.ticket_id.0))
    });
}

fn sort_repair_task_snapshots(snapshots: &mut [RepairTaskSnapshot]) {
    snapshots.sort_by(|left, right| {
        let left_deadline = left.record.sla_deadline_unix.unwrap_or(u64::MAX);
        let right_deadline = right.record.sla_deadline_unix.unwrap_or(u64::MAX);
        left_deadline
            .cmp(&right_deadline)
            .then_with(|| {
                queued_at_unix(&left.record.state).cmp(&queued_at_unix(&right.record.state))
            })
            .then_with(|| {
                left.record
                    .manifest_digest
                    .cmp(&right.record.manifest_digest)
            })
            .then_with(|| left.record.ticket_id.0.cmp(&right.record.ticket_id.0))
    });
}

fn compute_backlog_stats(tasks: &[RepairTaskInternal], now_unix: u64) -> RepairBacklogStats {
    if now_unix == 0 {
        return RepairBacklogStats::default();
    }
    let mut stats = RepairBacklogStats::default();
    let mut oldest_queued_at: Option<u64> = None;

    for task in tasks {
        if let RepairTaskStateV1::Queued(state) = &task.state {
            let entry = stats
                .per_provider
                .entry(task.report.evidence.provider_id)
                .or_insert(0);
            *entry = entry.saturating_add(1);
            let queued_at: u64 = state.queued_at_unix;
            oldest_queued_at = Some(match oldest_queued_at {
                Some(prev) => prev.min(queued_at),
                None => queued_at,
            });
        }
    }

    if let Some(queued_at) = oldest_queued_at {
        stats.oldest_age_secs = now_unix.saturating_sub(queued_at);
    }

    stats
}

fn ensure_transition_allowed(
    state: &RepairTaskStateV1,
    next: &'static str,
    ticket_id: &RepairTicketId,
) -> Result<(), RepairSchedulerError> {
    match (state, next) {
        (RepairTaskStateV1::Queued(..), "escalated") => Ok(()),
        (RepairTaskStateV1::InProgress(..), "escalated") => Ok(()),
        (RepairTaskStateV1::Failed(..), "escalated") => Ok(()),
        (RepairTaskStateV1::Escalated(..), "escalated") => Ok(()),
        _ => Err(RepairSchedulerError::InvalidState {
            ticket_id: ticket_id.to_string(),
            state: format!("{state:?}"),
        }),
    }
}

fn policy_violation(ticket_id: &RepairTicketId, reason: impl Into<String>) -> RepairSchedulerError {
    RepairSchedulerError::PolicyViolation {
        ticket_id: ticket_id.to_string(),
        reason: reason.into(),
    }
}

fn persisted_task_governance_policy(
    task: &RepairTaskInternal,
    ticket_id: &RepairTicketId,
) -> Result<RepairGovernancePolicySnapshot, RepairSchedulerError> {
    task.governance_policy.clone().ok_or_else(|| {
        RepairSchedulerError::Store(RepairStoreError::Other(format!(
            "repair task `{ticket_id}` is missing its governance policy snapshot"
        )))
    })
}

fn div_ceil_u128(numerator: u128, denominator: u128) -> u128 {
    if denominator == 0 {
        return 0;
    }
    let adjusted = numerator.saturating_add(denominator.saturating_sub(1));
    adjusted / denominator
}

fn vote_exists(votes: &[RepairGovernanceVote], voter_id: &str) -> bool {
    votes.iter().any(|vote| vote.voter_id == voter_id)
}

fn insert_vote(
    votes: &mut Vec<RepairGovernanceVote>,
    voter_id: &str,
    voted_at_unix: u64,
    ticket_id: &RepairTicketId,
) -> Result<(), RepairSchedulerError> {
    if let Some(existing) = votes.iter().find(|vote| vote.voter_id == voter_id) {
        if existing.voted_at_unix == voted_at_unix {
            return Ok(());
        }
        return Err(policy_violation(
            ticket_id,
            "duplicate vote recorded with mismatched timestamp",
        ));
    }
    votes.push(RepairGovernanceVote {
        voter_id: voter_id.to_string(),
        voted_at_unix,
    });
    votes.sort_by(|left, right| left.voter_id.cmp(&right.voter_id));
    Ok(())
}

fn repair_encoding_error(payload: &str, err: impl std::fmt::Display) -> RepairSchedulerError {
    RepairSchedulerError::Store(RepairStoreError::Other(format!(
        "failed to encode repair {payload}: {err}"
    )))
}

fn repair_report_source_identity(
    report: &RepairReportV1,
) -> Result<[u8; 32], RepairSchedulerError> {
    let canonical = norito::to_bytes(report)
        .map_err(|error| repair_encoding_error("report source identity", error))?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(REPAIR_REPORT_SOURCE_IDENTITY_DOMAIN_V1);
    hasher.update(
        &u64::try_from(canonical.len())
            .map_err(|_| {
                RepairSchedulerError::Store(RepairStoreError::Other(
                    "repair report length does not fit u64".to_owned(),
                ))
            })?
            .to_le_bytes(),
    );
    hasher.update(&canonical);
    Ok(*hasher.finalize().as_bytes())
}

impl RepairManager {
    /// Returns the configured claim TTL (seconds).
    #[must_use]
    pub fn claim_ttl_secs(&self) -> u64 {
        self.config.claim_ttl_secs()
    }

    /// Returns the configured heartbeat interval/TTL (seconds).
    #[must_use]
    pub fn heartbeat_interval_secs(&self) -> u64 {
        self.config.heartbeat_interval_secs()
    }
}

impl Default for RepairManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors returned by [`RepairManager`].
#[derive(Debug, Error)]
pub enum RepairSchedulerError {
    /// The originating subsystem did not supply a usable immutable identity.
    #[error("repair source identity must be non-zero")]
    InvalidSourceIdentity,
    /// One subsystem identity was replayed with a different canonical report.
    #[error(
        "repair source identity {source_identity:?} is already bound to a different canonical report"
    )]
    SourceIdentityConflict {
        /// Conflicting subsystem event or signed-receipt digest.
        source_identity: [u8; 32],
    },
    /// Repair report failed validation.
    #[error("repair report invalid: {0}")]
    InvalidReport(#[source] RepairValidationError),
    /// Slash proposal failed validation.
    #[error("slash proposal invalid: {0}")]
    InvalidSlashProposal(#[source] RepairValidationError),
    /// Escalation policy check failed.
    #[error("repair escalation policy violation for ticket `{ticket_id}`: {reason}")]
    PolicyViolation {
        /// Ticket identifier.
        ticket_id: String,
        /// Validation failure reason.
        reason: String,
    },
    /// Ticket already exists with conflicting evidence.
    #[error("repair ticket `{ticket_id}` already exists with conflicting evidence")]
    DuplicateTicket {
        /// Conflicting ticket identifier.
        ticket_id: String,
    },
    /// Signed auditor request nonce was already accepted or is stale.
    #[error(
        "auditor `{auditor_account}` nonce replay rejected: nonce {nonce} is not greater than stored nonce {highest_nonce}"
    )]
    AuditorNonceReplay {
        /// Auditor account whose nonce was replayed.
        auditor_account: String,
        /// Submitted nonce.
        nonce: u64,
        /// Highest nonce already accepted for this auditor.
        highest_nonce: u64,
    },
    /// Ticket not known to the scheduler.
    #[error("repair ticket `{ticket_id}` not found")]
    UnknownTicket {
        /// Missing ticket identifier.
        ticket_id: String,
    },
    /// Referenced PoR history entry was not recorded.
    #[error("por history entry {por_history_id} unknown")]
    UnknownPorHistory {
        /// Missing history identifier.
        por_history_id: u64,
        /// Ticket identifier (for diagnostics).
        ticket_id: String,
    },
    /// PoR history entry does not match the repair evidence.
    #[error("por history entry {por_history_id} does not match the repair evidence")]
    PorHistoryMismatch {
        /// History identifier.
        por_history_id: u64,
    },
    /// PoR failure evidence omitted its required durable history reference.
    #[error("PoR failure repair ticket `{ticket_id}` must reference PoR history")]
    MissingPorHistory {
        /// Ticket missing the history reference.
        ticket_id: String,
    },
    /// Non-PoR evidence attempted to reference a PoR history row.
    #[error(
        "non-PoR repair ticket `{ticket_id}` must not reference PoR history entry {por_history_id}"
    )]
    UnexpectedPorHistory {
        /// Ticket carrying the invalid reference.
        ticket_id: String,
        /// Unexpected history identifier.
        por_history_id: u64,
    },
    /// Ticket manifest digest mismatch.
    #[error("manifest mismatch for ticket `{ticket_id}`")]
    ManifestMismatch {
        /// Ticket identifier.
        ticket_id: String,
    },
    /// Ticket provider mismatch.
    #[error("provider mismatch for ticket `{ticket_id}`")]
    ProviderMismatch {
        /// Ticket identifier.
        ticket_id: String,
    },
    /// Repair store conflict while applying updates.
    #[error("repair store conflict for ticket `{ticket_id}`")]
    StoreConflict {
        /// Ticket identifier.
        ticket_id: String,
    },
    /// Underlying repair store error.
    #[error(transparent)]
    Store(#[from] RepairStoreError),
    /// Invalid timestamp sequencing supplied by the caller.
    #[error("timestamp monotonicity violated for ticket `{ticket_id}`")]
    InvalidTimestamp {
        /// Ticket identifier.
        ticket_id: String,
    },
    /// Worker payload is invalid.
    #[error("repair worker payload invalid for ticket `{ticket_id}`: {reason}")]
    InvalidWorkerPayload {
        /// Ticket identifier.
        ticket_id: String,
        /// Validation reason.
        reason: String,
    },
    /// Idempotency key reused with different payload.
    #[error("idempotency key `{key}` already used for `{action}` on ticket `{ticket_id}`")]
    IdempotencyMismatch {
        /// Ticket identifier.
        ticket_id: String,
        /// Action name.
        action: &'static str,
        /// Conflicting idempotency key.
        key: String,
    },
    /// Ticket is currently held by another worker.
    #[error("repair ticket `{ticket_id}` already claimed by worker `{worker_id}`")]
    LeaseHeld {
        /// Ticket identifier.
        ticket_id: String,
        /// Current worker identifier.
        worker_id: String,
    },
    /// Ticket is in retry backoff and not yet claimable.
    #[error("repair ticket `{ticket_id}` retry backoff active until {retry_after_unix}")]
    BackoffActive {
        /// Ticket identifier.
        ticket_id: String,
        /// Earliest allowed retry timestamp.
        retry_after_unix: u64,
    },
    /// Lease expired or missing for the ticket.
    #[error("repair ticket `{ticket_id}` lease expired")]
    LeaseExpired {
        /// Ticket identifier.
        ticket_id: String,
    },
    /// Worker identifier does not match the active lease.
    #[error("repair ticket `{ticket_id}` not leased to worker `{worker_id}`")]
    WorkerMismatch {
        /// Ticket identifier.
        ticket_id: String,
        /// Worker identifier.
        worker_id: String,
    },
    /// State transition not permitted.
    #[error("invalid state transition for ticket `{ticket_id}` from {state}")]
    InvalidState {
        /// Ticket identifier.
        ticket_id: String,
        /// Current state.
        state: String,
    },
}

impl<S: PartialEq> IdempotencyCache<S> {
    fn check_existing(
        &self,
        key: &str,
        signature: &S,
        action: &'static str,
        ticket_id: &RepairTicketId,
    ) -> Result<Option<RepairTaskRecordV1>, RepairSchedulerError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if entry.signature == *signature {
            return Ok(Some(entry.record.clone()));
        }
        Err(RepairSchedulerError::IdempotencyMismatch {
            ticket_id: ticket_id.to_string(),
            action,
            key: key.to_string(),
        })
    }

    fn remember(&mut self, key: &str, signature: S, record: RepairTaskRecordV1) {
        if self.capacity == 0 {
            return;
        }
        if self.entries.contains_key(key) {
            return;
        }
        if self.entries.len() >= self.capacity
            && let Some(evicted) = self.order.pop_front()
        {
            self.entries.remove(&evicted);
        }
        self.order.push_back(key.to_string());
        self.entries
            .insert(key.to_string(), IdempotencyEntry { signature, record });
    }
}

fn ensure_worker_field(
    value: &str,
    field: &'static str,
    max_len: usize,
    ticket_id: &RepairTicketId,
) -> Result<(), RepairSchedulerError> {
    if value.trim().is_empty() {
        return Err(RepairSchedulerError::InvalidWorkerPayload {
            ticket_id: ticket_id.to_string(),
            reason: format!("{field} must not be blank"),
        });
    }
    if value.len() > max_len {
        return Err(RepairSchedulerError::InvalidWorkerPayload {
            ticket_id: ticket_id.to_string(),
            reason: format!("{field} exceeds {max_len} bytes"),
        });
    }
    Ok(())
}

fn ensure_optional_field(
    value: Option<&str>,
    field: &'static str,
    max_len: usize,
    ticket_id: &RepairTicketId,
) -> Result<(), RepairSchedulerError> {
    if let Some(value) = value {
        ensure_worker_field(value, field, max_len, ticket_id)?;
    }
    Ok(())
}

fn ensure_idempotency_key(
    key: &str,
    ticket_id: &RepairTicketId,
) -> Result<(), RepairSchedulerError> {
    ensure_worker_field(key, "idempotency_key", MAX_IDEMPOTENCY_KEY_BYTES, ticket_id)
}

fn checked_add_secs(
    base: u64,
    secs: u64,
    ticket_id: &RepairTicketId,
) -> Result<u64, RepairSchedulerError> {
    base.checked_add(secs)
        .ok_or_else(|| RepairSchedulerError::InvalidTimestamp {
            ticket_id: ticket_id.to_string(),
        })
}

fn checked_next_revision(
    revision: u64,
    ticket_id: &RepairTicketId,
) -> Result<u64, RepairSchedulerError> {
    revision.checked_add(1).ok_or_else(|| {
        RepairSchedulerError::Store(RepairStoreError::Other(format!(
            "repair task `{ticket_id}` revision sequence exhausted"
        )))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_config::parameters::actual;
    use iroha_data_model::prelude::{Numeric, Quantity};
    use sorafs_manifest::por::{AUDIT_VERDICT_VERSION_V1, AuditOutcomeV1, AuditVerdictV1};
    use sorafs_manifest::repair::{
        REPAIR_ESCALATION_APPROVAL_VERSION_V1, REPAIR_EVIDENCE_VERSION_V1,
        REPAIR_REPORT_VERSION_V1, REPAIR_SLASH_PROPOSAL_VERSION_V1, RepairCauseV1,
        RepairEscalationApprovalV1, RepairManualCauseV1, RepairPorFailureCauseV1,
        RepairReplicaShortfallCauseV1,
    };
    use std::fs;
    use tempfile::{TempDir, tempdir};

    fn quantity_from_nanos(value: u128) -> XorQuantity {
        let numeric = Numeric::try_new(value, 9).expect("nano-XOR fixture fits numeric domain");
        let quantity = Quantity::from_canonical_numeric(numeric)
            .expect("non-negative nano-XOR fixture is a quantity");
        XorQuantity::try_from_quantity(quantity).expect("nano-XOR fixture has supported precision")
    }

    fn canonical_temp_path(temp_dir: &TempDir) -> PathBuf {
        temp_dir.path().canonicalize().expect("canonical tempdir")
    }

    fn write_private_file(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).expect("write private test file");
        #[cfg(unix)]
        fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .expect("set private test file permissions");
    }

    fn report(
        ticket: &str,
        manifest_digest: [u8; 32],
        provider_id: [u8; 32],
        submitted_at_unix: u64,
    ) -> RepairReportV1 {
        RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId(ticket.to_string()),
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            submitted_at_unix,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest,
                provider_id,
                por_history_id: None,
                cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                    reason: "test".into(),
                }),
                evidence_json: None,
                notes: None,
            },
            notes: None,
        }
    }

    fn task_internal(report: RepairReportV1) -> RepairTaskInternal {
        let sla_deadline = report
            .submitted_at_unix
            .checked_add(DEFAULT_REPAIR_SLA_SECS);
        let state = RepairTaskStateV1::Queued(QueuedRepairStateV1 {
            queued_at_unix: report.submitted_at_unix,
            sla_deadline_unix: sla_deadline,
        });
        let mut task = RepairTaskInternal {
            source_identity: repair_report_source_identity(&report)
                .expect("test report source identity"),
            revision: 0,
            report: report.clone(),
            state,
            sla_deadline_unix: sla_deadline,
            scheduler_notes: None,
            slash_proposal_digest: None,
            slash_proposal_bytes: None,
            slash_proposal_stage: None,
            governance: RepairGovernanceState::default(),
            governance_policy: None,
            lease: None,
            idempotency: RepairTaskIdempotency::new(DEFAULT_IDEMPOTENCY_CACHE_SIZE),
            attempts: 0,
            next_attempt_after_unix: None,
            events_dropped: 0,
            events: Vec::new(),
        };
        assert!(
            task.push_event(
                RepairTaskStatusV1::Queued,
                report.submitted_at_unix,
                Some(report.auditor_account),
                report.notes,
                DEFAULT_REPAIR_EVENT_HISTORY_LIMIT,
            )
            .is_some()
        );
        task
    }

    fn approval_for_policy(
        policy: &RepairEscalationPolicy,
        escalated_at_unix: u64,
    ) -> RepairEscalationApprovalV1 {
        let approved_at_unix = escalated_at_unix
            .saturating_add(policy.dispute_window_secs())
            .saturating_add(1);
        let finalized_at_unix = approved_at_unix
            .saturating_add(policy.appeal_window_secs())
            .saturating_add(1);
        RepairEscalationApprovalV1 {
            version: REPAIR_ESCALATION_APPROVAL_VERSION_V1,
            approve_votes: 2,
            reject_votes: 1,
            abstain_votes: 0,
            approved_at_unix,
            finalized_at_unix,
        }
    }

    fn enable_repair_config(config: RepairConfig) -> RepairConfig {
        let mut repair = actual::SorafsRepair::default();
        repair.enabled = true;
        repair.state_dir = config.state_dir().cloned();
        repair.claim_ttl_secs = config.claim_ttl_secs();
        repair.heartbeat_interval_secs = config.heartbeat_interval_secs();
        repair.max_attempts = config.max_attempts();
        repair.worker_concurrency = config.worker_concurrency();
        repair.backoff_initial_secs = config.backoff_initial_secs();
        repair.backoff_max_secs = config.backoff_max_secs();
        repair.default_slash_penalty = config.default_slash_penalty().clone();
        let policy = actual::RepairEscalationPolicyV1 {
            quorum_bps: config.escalation_policy().quorum_bps(),
            minimum_voters: config.escalation_policy().minimum_voters(),
            dispute_window_secs: config.escalation_policy().dispute_window_secs(),
            appeal_window_secs: config.escalation_policy().appeal_window_secs(),
            max_penalty: config.escalation_policy().max_penalty().clone(),
        };
        RepairConfig::from_repair_and_policy(&repair, &policy)
    }

    fn manager_with_config(config: RepairConfig) -> (RepairManager, TempDir) {
        let temp_dir = tempdir().expect("tempdir");
        let temp_path = canonical_temp_path(&temp_dir);
        let config = enable_repair_config(config).with_default_state_dir(&temp_path);
        let manager = RepairManager::new_with_config(config);
        (manager, temp_dir)
    }

    fn manager_with_temp_dir() -> (RepairManager, TempDir) {
        manager_with_config(RepairConfig::default())
    }

    #[test]
    fn compute_backlog_stats_tracks_oldest_queued() {
        let report_a = report("REP-001", [0x01; 32], [0xA1; 32], 1_700_000_000);
        let report_b = report("REP-002", [0x02; 32], [0xB2; 32], 1_700_000_100);
        let tasks = vec![task_internal(report_b), task_internal(report_a)];

        let stats = compute_backlog_stats(&tasks, 1_700_000_250);

        assert_eq!(stats.oldest_age_secs, 250);
        assert_eq!(stats.per_provider.get(&[0xA1; 32]).copied(), Some(1));
        assert_eq!(stats.per_provider.get(&[0xB2; 32]).copied(), Some(1));
    }

    #[test]
    fn repair_store_persists_state_dir_snapshot() {
        let (manager, temp_dir) = manager_with_temp_dir();
        let report = report("REP-009", [0x21; 32], [0x22; 32], 1_700_000_010);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");

        let verdict = AuditVerdictV1 {
            version: AUDIT_VERDICT_VERSION_V1,
            manifest_digest: report.evidence.manifest_digest,
            provider_id: report.evidence.provider_id,
            challenge_id: [0x33; 32],
            proof_digest: None,
            outcome: AuditOutcomeV1::Failed,
            failure_reason: Some("fail".into()),
            decided_at: report.submitted_at_unix + 1,
            auditor_signatures: Vec::new(),
            metadata: Vec::new(),
        };
        manager
            .register_por_verdict(&verdict, 1)
            .expect("register por verdict");

        let store_path = canonical_temp_path(&temp_dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        let bytes = fs::read(&store_path).expect("read repair store");
        let snapshot: RepairStoreSnapshot =
            norito::decode_from_bytes(&bytes).expect("decode repair store");
        assert_eq!(snapshot.tasks.len(), 1);
        assert_eq!(snapshot.por_history.len(), 1);
        assert_eq!(
            snapshot.por_history[0].manifest_digest,
            verdict.manifest_digest
        );
    }

    #[test]
    fn por_failure_reports_require_exact_history_binding() {
        let (manager, _dir) = manager_with_temp_dir();
        let verdict = AuditVerdictV1 {
            version: AUDIT_VERDICT_VERSION_V1,
            manifest_digest: [0x21; 32],
            provider_id: [0x22; 32],
            challenge_id: [0x23; 32],
            proof_digest: None,
            outcome: AuditOutcomeV1::Failed,
            failure_reason: Some("failed".to_owned()),
            decided_at: 1_700_000_050,
            auditor_signatures: Vec::new(),
            metadata: Vec::new(),
        };
        let history_id = manager
            .register_por_verdict(&verdict, 4)
            .expect("record history")
            .expect("failure has history id");

        let por_cause = |challenge_id, failed_samples| {
            RepairCauseV1::PorFailure(RepairPorFailureCauseV1 {
                challenge_id,
                failed_samples,
                proof_digest: None,
            })
        };
        let mut valid = report(
            "REP-POR-BIND-VALID",
            verdict.manifest_digest,
            verdict.provider_id,
            verdict.decided_at + 1,
        );
        valid.evidence.por_history_id = Some(history_id);
        valid.evidence.cause = por_cause(verdict.challenge_id, 4);
        manager
            .enqueue_report(valid)
            .expect("exact history binding accepted");

        let mut predates_verdict = report(
            "REP-POR-BIND-PREDATES",
            verdict.manifest_digest,
            verdict.provider_id,
            verdict.decided_at - 1,
        );
        predates_verdict.evidence.por_history_id = Some(history_id);
        predates_verdict.evidence.cause = por_cause(verdict.challenge_id, 4);
        assert!(matches!(
            manager.enqueue_report(predates_verdict),
            Err(RepairSchedulerError::PorHistoryMismatch { .. })
        ));

        let mut missing = report(
            "REP-POR-BIND-MISSING",
            verdict.manifest_digest,
            verdict.provider_id,
            verdict.decided_at + 2,
        );
        missing.evidence.cause = por_cause(verdict.challenge_id, 4);
        assert!(matches!(
            manager.enqueue_report(missing),
            Err(RepairSchedulerError::MissingPorHistory { .. })
        ));

        let mut wrong_challenge = report(
            "REP-POR-BIND-CHALLENGE",
            verdict.manifest_digest,
            verdict.provider_id,
            verdict.decided_at + 3,
        );
        wrong_challenge.evidence.por_history_id = Some(history_id);
        wrong_challenge.evidence.cause = por_cause([0x99; 32], 4);
        assert!(matches!(
            manager.enqueue_report(wrong_challenge),
            Err(RepairSchedulerError::PorHistoryMismatch { .. })
        ));

        let mut wrong_samples = report(
            "REP-POR-BIND-SAMPLES",
            verdict.manifest_digest,
            verdict.provider_id,
            verdict.decided_at + 4,
        );
        wrong_samples.evidence.por_history_id = Some(history_id);
        wrong_samples.evidence.cause = por_cause(verdict.challenge_id, 3);
        assert!(matches!(
            manager.enqueue_report(wrong_samples),
            Err(RepairSchedulerError::PorHistoryMismatch { .. })
        ));

        let mut unrelated = report(
            "REP-POR-BIND-UNRELATED",
            verdict.manifest_digest,
            verdict.provider_id,
            verdict.decided_at + 5,
        );
        unrelated.evidence.por_history_id = Some(history_id);
        assert!(matches!(
            manager.enqueue_report(unrelated),
            Err(RepairSchedulerError::UnexpectedPorHistory { .. })
        ));
    }

    #[test]
    fn auditor_nonce_replay_rejection_persists() {
        let temp_dir = tempdir().expect("tempdir");
        let temp_path = canonical_temp_path(&temp_dir);
        let config =
            enable_repair_config(RepairConfig::default()).with_default_state_dir(&temp_path);
        let manager = RepairManager::new_with_config(config.clone());
        let auditor_account = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB";

        manager
            .record_auditor_nonce(auditor_account, 42)
            .expect("first nonce accepted");
        let replay = manager
            .record_auditor_nonce(auditor_account, 42)
            .expect_err("equal nonce rejected");
        assert!(matches!(
            replay,
            RepairSchedulerError::AuditorNonceReplay {
                nonce: 42,
                highest_nonce: 42,
                ..
            }
        ));
        let stale = manager
            .record_auditor_nonce(auditor_account, 41)
            .expect_err("stale nonce rejected");
        assert!(matches!(
            stale,
            RepairSchedulerError::AuditorNonceReplay {
                nonce: 41,
                highest_nonce: 42,
                ..
            }
        ));

        let store_path = temp_path.join("repair").join(REPAIR_STORE_FILE_NAME);
        let bytes = fs::read(&store_path).expect("read repair store");
        let snapshot: RepairStoreSnapshot =
            norito::decode_from_bytes(&bytes).expect("decode repair store");
        assert_eq!(snapshot.auditor_nonces.len(), 1);
        assert_eq!(snapshot.auditor_nonces[0].auditor_account, auditor_account);
        assert_eq!(snapshot.auditor_nonces[0].highest_nonce, 42);

        drop(manager);
        let reloaded = RepairManager::new_with_config(config);
        let persisted_replay = reloaded
            .record_auditor_nonce(auditor_account, 42)
            .expect_err("persisted nonce rejects replay after reload");
        assert!(matches!(
            persisted_replay,
            RepairSchedulerError::AuditorNonceReplay {
                nonce: 42,
                highest_nonce: 42,
                ..
            }
        ));
        reloaded
            .record_auditor_nonce(auditor_account, 43)
            .expect("higher nonce accepted after reload");
    }

    #[test]
    fn auditor_nonce_account_length_boundary_is_restart_safe() {
        let dir = tempdir().expect("tempdir");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        let store = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("store");
        let boundary = "a".repeat(MAX_GOVERNANCE_ACTOR_BYTES);
        store
            .record_auditor_nonce(&boundary, 1)
            .expect("boundary account accepted");
        let oversized = "b".repeat(MAX_GOVERNANCE_ACTOR_BYTES + 1);
        let error = store
            .record_auditor_nonce(&oversized, 1)
            .expect_err("oversized account rejected before persistence");
        assert!(error.to_string().contains("length"));
        drop(store);

        let reloaded = FileRepairStore::load_or_new(
            path,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("valid boundary checkpoint reloads");
        assert!(matches!(
            reloaded.record_auditor_nonce(&boundary, 1),
            Err(RepairStoreError::AuditorNonceReplay { .. })
        ));
    }

    #[test]
    fn register_por_verdict_propagates_store_error() {
        let dir = tempdir().expect("tempdir");
        let file_path = dir.path().join("not-a-dir");
        fs::write(&file_path, b"blocked").expect("write guard file");

        let mut actual = actual::SorafsRepair::default();
        actual.enabled = true;
        actual.state_dir = Some(file_path);
        let config = RepairConfig::from(&actual);
        let manager = RepairManager::new_with_config(config);
        let verdict = AuditVerdictV1 {
            version: AUDIT_VERDICT_VERSION_V1,
            manifest_digest: [0x11; 32],
            provider_id: [0x22; 32],
            challenge_id: [0x33; 32],
            proof_digest: None,
            outcome: AuditOutcomeV1::Failed,
            failure_reason: Some("fail".into()),
            decided_at: 1_700_000_020,
            auditor_signatures: Vec::new(),
            metadata: Vec::new(),
        };

        let err = manager
            .register_por_verdict(&verdict, 1)
            .expect_err("expected store error");
        assert!(matches!(err, RepairStoreError::Other(_)));
    }

    #[test]
    fn manager_startup_corruption_is_unavailable_without_panicking_or_replacement() {
        let dir = tempdir().expect("tempdir");
        let state_dir = canonical_temp_path(&dir).join("repair-state");
        fs::create_dir(&state_dir).expect("create state dir");
        let checkpoint = state_dir.join(REPAIR_STORE_FILE_NAME);
        write_private_file(&checkpoint, b"corrupt-checkpoint");
        let mut actual = actual::SorafsRepair::default();
        actual.enabled = true;
        actual.state_dir = Some(state_dir);

        let manager = RepairManager::new_with_config(RepairConfig::from(&actual));
        let error = manager
            .enqueue_report(report(
                "REP-STARTUP-CORRUPT",
                [0x31; 32],
                [0x41; 32],
                1_700_000_030,
            ))
            .expect_err("corrupt startup store remains unavailable");
        assert!(error.to_string().contains("truncated"));
        assert_eq!(
            fs::read(&checkpoint).expect("checkpoint remains present"),
            b"corrupt-checkpoint"
        );
    }

    #[test]
    fn missing_state_directory_configuration_fails_closed() {
        let mut actual = actual::SorafsRepair::default();
        actual.enabled = true;
        actual.state_dir = None;
        let manager = RepairManager::new_with_config(RepairConfig::from(&actual));
        let error = manager
            .record_auditor_nonce("auditor", 1)
            .expect_err("unconfigured durable state rejected");
        assert!(error.to_string().contains("state_dir is not configured"));
        assert!(
            manager.list_tasks(RepairTaskFilters::default()).is_err(),
            "list outage must not look like an empty queue"
        );
        assert!(
            manager
                .task_record(&RepairTicketId("REP-MISSING".to_owned()))
                .is_err(),
            "lookup outage must not look like a missing ticket"
        );
        assert!(
            manager.claimable_tasks(1).is_err(),
            "scheduler outage must not look like no claimable work"
        );
    }

    #[test]
    fn disabled_repair_skips_unsafe_state_path_but_operations_fail_closed() {
        let dir = tempdir().expect("tempdir");
        let blocked_parent = canonical_temp_path(&dir).join("not-a-directory");
        write_private_file(&blocked_parent, b"must remain unchanged");
        let mut actual = actual::SorafsRepair::default();
        actual.enabled = false;
        actual.state_dir = Some(blocked_parent.join("unusable"));
        let config = RepairConfig::from(&actual);
        let manager = RepairManager::try_new_with_config_policy_and_limits(
            config.clone(),
            config.escalation_policy().clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("disabled repair must not inspect or initialize its state path");

        let error = manager
            .list_tasks(RepairTaskFilters::default())
            .expect_err("disabled repair operations fail closed");
        assert!(error.to_string().contains("disabled"));
        assert_eq!(
            fs::read(&blocked_parent).expect("read unchanged blocking file"),
            b"must remain unchanged"
        );
    }

    #[test]
    fn list_tasks_sorts_by_deadline_then_ticket() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let manifest = [0x11; 32];
        let provider = [0x22; 32];

        manager
            .enqueue_report(report("REP-200", manifest, provider, 1_000))
            .expect("enqueue report");
        manager
            .enqueue_report(report("REP-100", manifest, provider, 1_000))
            .expect("enqueue report");
        manager
            .enqueue_report(report("REP-050", [0x10; 32], provider, 900))
            .expect("enqueue report");

        let tasks = manager
            .list_tasks(RepairTaskFilters::default())
            .expect("list tasks");
        let ids: Vec<_> = tasks.iter().map(|task| task.ticket_id.0.as_str()).collect();
        assert_eq!(ids, vec!["REP-050", "REP-100", "REP-200"]);
    }

    #[test]
    fn duplicate_ticket_requires_full_canonical_report_equality() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-DUPLICATE-FULL", [0x14; 32], [0x24; 32], 1_700_000_040);
        let first = manager
            .enqueue_report_with_event(report.clone())
            .expect("first report");
        assert!(first.event.is_some());
        let replay = manager
            .enqueue_report_with_event(report.clone())
            .expect("exact report replay");
        assert!(replay.event.is_none());
        assert_eq!(replay.record, first.record);

        let mut changed_notes = report.clone();
        changed_notes.notes = Some("different".to_owned());
        assert!(matches!(
            manager.enqueue_report(changed_notes),
            Err(RepairSchedulerError::DuplicateTicket { .. })
        ));
        let mut changed_time = report;
        changed_time.submitted_at_unix += 1;
        assert!(matches!(
            manager.enqueue_report(changed_time),
            Err(RepairSchedulerError::DuplicateTicket { .. })
        ));
    }

    #[test]
    fn subsystem_source_identity_is_exactly_once_and_conflicts_fail_closed() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let source_identity = [0xA5; 32];
        let baseline = report("REP-SOURCE-ONCE", [0x16; 32], [0x26; 32], 1_700_000_041);
        let first = manager
            .enqueue_repair_report_idempotent(source_identity, baseline.clone())
            .expect("first subsystem event");
        let replay = manager
            .enqueue_repair_report_idempotent(source_identity, baseline.clone())
            .expect("exact subsystem event replay");
        assert_eq!(replay, first);

        let conflicting = report(
            "REP-SOURCE-CONFLICT",
            baseline.evidence.manifest_digest,
            baseline.evidence.provider_id,
            baseline.submitted_at_unix,
        );
        assert!(matches!(
            manager.enqueue_repair_report_idempotent(source_identity, conflicting),
            Err(RepairSchedulerError::SourceIdentityConflict {
                source_identity: found,
            }) if found == source_identity
        ));
        assert!(matches!(
            manager.enqueue_repair_report_idempotent([0; 32], baseline),
            Err(RepairSchedulerError::InvalidSourceIdentity)
        ));
        assert_eq!(
            manager
                .list_tasks(RepairTaskFilters::default())
                .expect("list tasks")
                .len(),
            1
        );
    }

    #[test]
    fn enqueue_report_rejects_sla_timestamp_overflow_without_retaining_task() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-SLA-OVERFLOW", [0x15; 32], [0x25; 32], u64::MAX);
        let error = manager
            .enqueue_report(report.clone())
            .expect_err("SLA overflow must reject the report");
        assert!(matches!(
            error,
            RepairSchedulerError::InvalidTimestamp { .. }
        ));
        assert!(
            manager
                .task_record(&report.ticket_id)
                .expect("task lookup")
                .is_none()
        );
    }

    #[test]
    fn list_tasks_filters_by_provider_and_status() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let manifest = [0x33; 32];
        let provider_a = [0x01; 32];
        let provider_b = [0x02; 32];

        let report_a = report("REP-300", manifest, provider_a, 1_700_000_000);
        let report_b = report("REP-301", manifest, provider_b, 1_700_000_100);

        manager
            .enqueue_report(report_a.clone())
            .expect("enqueue report");
        manager
            .enqueue_report(report_b.clone())
            .expect("enqueue report");

        manager
            .mark_in_progress(&report_a.ticket_id, report_a.submitted_at_unix + 30, None)
            .expect("mark in progress");
        manager
            .mark_completed(&report_a.ticket_id, report_a.submitted_at_unix + 90, None)
            .expect("mark completed");

        let provider_tasks = manager
            .list_tasks(RepairTaskFilters {
                provider_id: Some(provider_a),
                ..RepairTaskFilters::default()
            })
            .expect("list provider tasks");
        assert_eq!(provider_tasks.len(), 1);
        assert_eq!(provider_tasks[0].ticket_id, report_a.ticket_id);

        let status_tasks = manager
            .list_tasks(RepairTaskFilters {
                status: Some(RepairTaskStatusV1::Completed),
                ..RepairTaskFilters::default()
            })
            .expect("list status tasks");
        assert_eq!(status_tasks.len(), 1);
        assert_eq!(status_tasks[0].ticket_id, report_a.ticket_id);
    }

    #[test]
    fn backlog_stats_tracks_oldest_and_per_provider() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let provider_a = [0x11; 32];
        let provider_b = [0x22; 32];
        let report_a = report("REP-310", [0x01; 32], provider_a, 1_000);
        let report_b = report("REP-311", [0x02; 32], provider_b, 1_100);
        let report_c = report("REP-312", [0x03; 32], provider_a, 1_200);

        manager
            .enqueue_report(report_a.clone())
            .expect("enqueue report a");
        manager
            .enqueue_report(report_b.clone())
            .expect("enqueue report b");
        manager
            .enqueue_report(report_c.clone())
            .expect("enqueue report c");
        manager
            .claim_ticket(&report_c.ticket_id, "worker", 1_210, "claim-310")
            .expect("claim report c");

        let stats = manager.backlog_stats(1_300).expect("backlog stats");
        assert_eq!(stats.oldest_age_secs, 300);
        assert_eq!(stats.per_provider.get(&provider_a).copied(), Some(1));
        assert_eq!(stats.per_provider.get(&provider_b).copied(), Some(1));
    }

    #[test]
    fn task_record_returns_ticket() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-320", [0x12; 32], [0x34; 32], 1_700_100_000);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");

        let record = manager
            .task_record(&report.ticket_id)
            .expect("load task record")
            .expect("task record");
        assert_eq!(record.ticket_id, report.ticket_id);
        assert_eq!(record.provider_id, report.evidence.provider_id);
    }

    #[test]
    fn mark_in_progress_with_event_emits_transition() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-321", [0x12; 32], [0x34; 32], 1_700_100_100);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");

        let update = manager
            .mark_in_progress_with_event(
                &report.ticket_id,
                report.submitted_at_unix + 5,
                Some("agent#sora".into()),
            )
            .expect("mark in progress");
        let event = update.event.expect("event emitted");
        assert_eq!(event.status, RepairTaskStatusV1::InProgress);
        assert_eq!(event.actor.as_deref(), Some("agent#sora"));
    }

    #[test]
    fn mark_completed_with_event_emits_transition() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-322", [0x13; 32], [0x35; 32], 1_700_100_200);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        manager
            .mark_in_progress(&report.ticket_id, report.submitted_at_unix + 10, None)
            .expect("mark in progress");

        let update = manager
            .mark_completed_with_event(
                &report.ticket_id,
                report.submitted_at_unix + 40,
                Some("ok".into()),
            )
            .expect("mark completed");
        let event = update.event.expect("event emitted");
        assert_eq!(event.status, RepairTaskStatusV1::Completed);
        assert_eq!(event.message.as_deref(), Some("ok"));
    }

    #[test]
    fn mark_failed_with_event_escalates_and_returns_slash() {
        let actual = actual::SorafsRepair {
            max_attempts: 1,
            default_slash_penalty: quantity_from_nanos(12_000),
            ..Default::default()
        };
        let (manager, _temp_dir) = manager_with_config(RepairConfig::from(&actual));
        let report = report("REP-323", [0x14; 32], [0x36; 32], 1_700_100_300);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");

        let update = manager
            .mark_failed_with_event(
                &report.ticket_id,
                report.submitted_at_unix + 20,
                "loss".into(),
            )
            .expect("mark failed");
        assert!(matches!(
            update.record.state,
            RepairTaskStateV1::Escalated(..)
        ));
        let proposal = update.slash_proposal.expect("slash proposal");
        assert_eq!(proposal.ticket_id, report.ticket_id);
        assert!(proposal.approval.is_none());
    }

    #[test]
    fn apply_escalation_caps_slash_penalty() {
        let policy = actual::RepairEscalationPolicyV1 {
            max_penalty: quantity_from_nanos(500),
            ..Default::default()
        };
        let actual = actual::SorafsRepair {
            max_attempts: 1,
            default_slash_penalty: quantity_from_nanos(12_000),
            ..Default::default()
        };
        let config = RepairConfig::from_repair_and_policy(&actual, &policy);
        let (manager, _temp_dir) = manager_with_config(config);
        let report = report("REP-323B", [0x15; 32], [0x37; 32], 1_700_100_310);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");

        let update = manager
            .mark_failed_with_event(
                &report.ticket_id,
                report.submitted_at_unix + 20,
                "loss".into(),
            )
            .expect("mark failed");
        let proposal = update.slash_proposal.expect("slash proposal");
        assert_eq!(proposal.proposed_penalty, quantity_from_nanos(500));
    }

    #[test]
    fn submit_slash_proposal_rejects_manifest_mismatch() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-324", [0x21; 32], [0x31; 32], 1_700_100_350);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        let policy = RepairConfig::default().escalation_policy().clone();
        let approval = approval_for_policy(&policy, report.submitted_at_unix + 10);

        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: report.ticket_id.clone(),
            provider_id: report.evidence.provider_id,
            manifest_digest: [0xFF; 32],
            auditor_account: report.auditor_account.clone(),
            proposed_penalty: quantity_from_nanos(1_000),
            submitted_at_unix: report.submitted_at_unix + 10,
            rationale: "manifest mismatch".into(),
            approval: Some(approval),
        };

        let err = manager
            .submit_slash_proposal_with_event(proposal)
            .expect_err("mismatched manifest should error");
        assert!(matches!(err, RepairSchedulerError::ManifestMismatch { .. }));
    }

    #[test]
    fn submit_slash_proposal_rejects_auditor_mismatch() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-324-AUDITOR", [0x21; 32], [0x31; 32], 1_700_100_355);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: report.ticket_id.clone(),
            provider_id: report.evidence.provider_id,
            manifest_digest: report.evidence.manifest_digest,
            auditor_account: "different-auditor".to_owned(),
            proposed_penalty: quantity_from_nanos(1_000),
            submitted_at_unix: report.submitted_at_unix + 10,
            rationale: "auditor mismatch".into(),
            approval: None,
        };

        let error = manager
            .submit_slash_proposal_with_event(proposal)
            .expect_err("mismatched auditor should error");
        assert!(matches!(
            error,
            RepairSchedulerError::PolicyViolation { reason, .. }
                if reason.contains("auditor")
        ));
    }

    #[test]
    fn submit_slash_proposal_rejects_dispute_deadline_overflow() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-PROPOSAL-OVERFLOW", [0x22; 32], [0x32; 32], 100);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: report.ticket_id.clone(),
            provider_id: report.evidence.provider_id,
            manifest_digest: report.evidence.manifest_digest,
            auditor_account: report.auditor_account.clone(),
            proposed_penalty: quantity_from_nanos(1),
            submitted_at_unix: u64::MAX,
            rationale: "overflow".to_owned(),
            approval: None,
        };
        let error = manager
            .submit_slash_proposal(proposal)
            .expect_err("overflowing dispute deadline rejected");
        assert!(matches!(
            error,
            RepairSchedulerError::InvalidTimestamp { .. }
        ));
        assert!(matches!(
            manager
                .task_record(&report.ticket_id)
                .expect("task lookup")
                .expect("task remains")
                .state,
            RepairTaskStateV1::Queued(_)
        ));
    }

    #[test]
    fn submit_slash_proposal_accepts_missing_approval() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-325", [0x21; 32], [0x31; 32], 1_700_100_360);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");

        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: report.ticket_id.clone(),
            provider_id: report.evidence.provider_id,
            manifest_digest: report.evidence.manifest_digest,
            auditor_account: report.auditor_account.clone(),
            proposed_penalty: quantity_from_nanos(1_000),
            submitted_at_unix: report.submitted_at_unix + 10,
            rationale: "missing approval".into(),
            approval: None,
        };

        let update = manager
            .submit_slash_proposal_with_event(proposal)
            .expect("missing approval should be accepted");
        assert!(matches!(
            update.record.state,
            RepairTaskStateV1::Escalated(..)
        ));
        let task = manager
            .load_task(&report.ticket_id)
            .expect("load repair task");
        assert!(task.governance.decision.is_none());
    }

    #[test]
    fn framed_submitted_and_auto_drafted_proposals_survive_restart() {
        let dir = tempdir().expect("tempdir");
        let root = canonical_temp_path(&dir);
        let mut actual = actual::SorafsRepair::default();
        actual.enabled = true;
        actual.max_attempts = 1;
        let config = RepairConfig::from(&actual).with_default_state_dir(&root);
        let manager = RepairManager::new_with_config(config.clone());

        let submitted = report("REP-PROPOSAL-RELOAD-SUBMITTED", [0x25; 32], [0x35; 32], 100);
        manager
            .enqueue_report(submitted.clone())
            .expect("enqueue submitted-proposal task");
        let submitted_proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: submitted.ticket_id.clone(),
            provider_id: submitted.evidence.provider_id,
            manifest_digest: submitted.evidence.manifest_digest,
            auditor_account: submitted.auditor_account.clone(),
            proposed_penalty: quantity_from_nanos(1),
            submitted_at_unix: 101,
            rationale: "submitted proposal".to_owned(),
            approval: None,
        };
        manager
            .submit_slash_proposal(submitted_proposal.clone())
            .expect("persist submitted proposal");

        let drafted = report("REP-PROPOSAL-RELOAD-DRAFTED", [0x26; 32], [0x36; 32], 200);
        manager
            .enqueue_report(drafted.clone())
            .expect("enqueue auto-drafted task");
        let drafted_proposal = manager
            .mark_failed_with_event(&drafted.ticket_id, 201, "failure".to_owned())
            .expect("persist auto-drafted proposal");
        let drafted_proposal = drafted_proposal
            .slash_proposal
            .expect("scheduler returned drafted proposal");
        drop(manager);

        let reloaded = RepairManager::try_new_with_config_policy_and_limits(
            config.clone(),
            config.escalation_policy().clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("framed proposals reload");
        for (ticket_id, expected_proposal, expected_stage) in [
            (
                &submitted.ticket_id,
                &submitted_proposal,
                RepairSlashProposalStage::Submitted,
            ),
            (
                &drafted.ticket_id,
                &drafted_proposal,
                RepairSlashProposalStage::Drafted,
            ),
        ] {
            let task = reloaded
                .load_task(ticket_id)
                .expect("load escalated task after restart");
            assert!(matches!(task.state, RepairTaskStateV1::Escalated(_)));
            assert_eq!(task.slash_proposal_stage, Some(expected_stage));
            let bytes = task
                .slash_proposal_bytes
                .as_deref()
                .expect("persisted proposal bytes");
            validate_bounded_uncompressed_norito(
                bytes,
                u64::try_from(bytes.len()).expect("proposal length fits u64"),
                "test proposal",
            )
            .expect("proposal uses framed uncompressed Norito");
            let decoded = norito::decode_from_bytes::<RepairSlashProposalV1>(bytes)
                .expect("decode framed proposal");
            assert_eq!(&decoded, expected_proposal);
            assert_eq!(
                task.slash_proposal_digest,
                Some(*hash(bytes).as_bytes()),
                "canonical proposal digest survives restart"
            );

            let snapshot = reloaded
                .task_snapshot(ticket_id)
                .expect("load public snapshot")
                .expect("snapshot exists");
            assert_eq!(snapshot.slash_proposal.as_ref(), Some(expected_proposal));
            assert_eq!(snapshot.slash_proposal_stage, Some(expected_stage));
        }
    }

    #[test]
    fn drafted_slash_proposal_submission_is_atomic_and_idempotent() {
        let mut actual = actual::SorafsRepair::default();
        actual.max_attempts = 1;
        let (manager, _temp_dir) = manager_with_config(RepairConfig::from(&actual));
        let report = report(
            "REP-PROPOSAL-DRAFT-SUBMIT",
            [0x27; 32],
            [0x37; 32],
            1_700_100_365,
        );
        manager
            .enqueue_report(report.clone())
            .expect("enqueue auto-drafted task");
        let update = manager
            .mark_failed_with_event(
                &report.ticket_id,
                report.submitted_at_unix + 1,
                "failure".to_owned(),
            )
            .expect("scheduler drafts proposal");
        let proposal = update.slash_proposal.expect("drafted proposal returned");

        let drafted = manager
            .load_task(&report.ticket_id)
            .expect("load drafted task");
        let drafted_revision = drafted.revision;
        let drafted_bytes = drafted
            .slash_proposal_bytes
            .clone()
            .expect("drafted canonical bytes");
        let drafted_digest = drafted
            .slash_proposal_digest
            .expect("drafted canonical digest");
        assert_eq!(
            drafted.slash_proposal_stage,
            Some(RepairSlashProposalStage::Drafted)
        );
        let snapshot = manager
            .task_snapshot(&report.ticket_id)
            .expect("load drafted snapshot")
            .expect("drafted snapshot exists");
        assert_eq!(snapshot.slash_proposal.as_ref(), Some(&proposal));
        assert_eq!(
            snapshot.slash_proposal_stage,
            Some(RepairSlashProposalStage::Drafted)
        );

        manager
            .submit_slash_proposal(proposal.clone())
            .expect("accept exact scheduler draft");
        let submitted = manager
            .load_task(&report.ticket_id)
            .expect("load submitted task");
        assert_eq!(submitted.revision, drafted_revision + 1);
        assert_eq!(submitted.slash_proposal_bytes, Some(drafted_bytes));
        assert_eq!(submitted.slash_proposal_digest, Some(drafted_digest));
        assert_eq!(
            submitted.slash_proposal_stage,
            Some(RepairSlashProposalStage::Submitted)
        );

        let submitted_revision = submitted.revision;
        manager
            .submit_slash_proposal(proposal.clone())
            .expect("exact submitted replay is idempotent");
        let replayed = manager
            .load_task(&report.ticket_id)
            .expect("load replayed task");
        assert_eq!(replayed.revision, submitted_revision);
        assert_eq!(
            replayed.slash_proposal_stage,
            Some(RepairSlashProposalStage::Submitted)
        );
        let snapshot = manager
            .task_snapshot(&report.ticket_id)
            .expect("load submitted snapshot")
            .expect("submitted snapshot exists");
        assert_eq!(snapshot.slash_proposal, Some(proposal));
        assert_eq!(
            snapshot.slash_proposal_stage,
            Some(RepairSlashProposalStage::Submitted)
        );
    }

    #[test]
    fn submitted_slash_proposal_cannot_downgrade_to_drafted() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report(
            "REP-PROPOSAL-NO-DOWNGRADE",
            [0x28; 32],
            [0x38; 32],
            1_700_100_366,
        );
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        manager
            .submit_slash_proposal(RepairSlashProposalV1 {
                version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
                ticket_id: report.ticket_id.clone(),
                provider_id: report.evidence.provider_id,
                manifest_digest: report.evidence.manifest_digest,
                auditor_account: report.auditor_account.clone(),
                proposed_penalty: quantity_from_nanos(1),
                submitted_at_unix: report.submitted_at_unix + 1,
                rationale: "submitted proposal".to_owned(),
                approval: None,
            })
            .expect("submit proposal");
        let submitted_revision = manager
            .load_task(&report.ticket_id)
            .expect("load submitted task")
            .revision;

        let error = manager
            .update_task_with_retry(&report.ticket_id, |task| {
                task.slash_proposal_stage = Some(RepairSlashProposalStage::Drafted);
                Ok(())
            })
            .expect_err("submitted stage cannot be downgraded");
        assert!(matches!(
            error,
            RepairSchedulerError::Store(RepairStoreError::Other(message))
                if message.contains("cannot downgrade")
        ));
        let retained = manager
            .load_task(&report.ticket_id)
            .expect("load retained submitted task");
        assert_eq!(retained.revision, submitted_revision);
        assert_eq!(
            retained.slash_proposal_stage,
            Some(RepairSlashProposalStage::Submitted)
        );
    }

    #[test]
    fn checkpoint_rejects_partial_slash_proposal_publication_state() {
        let mut actual = actual::SorafsRepair::default();
        actual.max_attempts = 1;
        let (manager, temp_dir) = manager_with_config(RepairConfig::from(&actual));
        let report = report(
            "REP-PROPOSAL-PARTIAL-CHECKPOINT",
            [0x29; 32],
            [0x39; 32],
            1_700_100_367,
        );
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        manager
            .mark_failed(
                &report.ticket_id,
                report.submitted_at_unix + 1,
                "failure".to_owned(),
            )
            .expect("draft proposal");
        let path = canonical_temp_path(&temp_dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        let bytes = fs::read(&path).expect("read valid drafted checkpoint");
        let snapshot: RepairStoreSnapshot =
            norito::decode_from_bytes(&bytes).expect("decode valid drafted checkpoint");
        assert_eq!(
            snapshot.tasks[0].slash_proposal_stage,
            Some(RepairSlashProposalStage::Drafted)
        );
        assert!(snapshot.tasks[0].slash_proposal_digest.is_some());
        assert!(snapshot.tasks[0].slash_proposal_bytes.is_some());
        drop(manager);

        let mut proposal_without_stage = snapshot.clone();
        proposal_without_stage.tasks[0].slash_proposal_stage = None;
        write_private_file(
            &path,
            &norito::to_bytes(&proposal_without_stage)
                .expect("encode proposal without publication stage"),
        );
        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("proposal bytes without stage rejected");
        assert!(
            error
                .to_string()
                .contains("bytes, digest, and publication stage together")
        );

        let mut stage_without_proposal = snapshot;
        stage_without_proposal.tasks[0].slash_proposal_digest = None;
        stage_without_proposal.tasks[0].slash_proposal_bytes = None;
        write_private_file(
            &path,
            &norito::to_bytes(&stage_without_proposal)
                .expect("encode publication stage without proposal"),
        );
        let error = FileRepairStore::load_or_new(
            path,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("stage without proposal bytes and digest rejected");
        assert!(
            error
                .to_string()
                .contains("bytes, digest, and publication stage together")
        );
    }

    #[test]
    fn checkpoint_rejects_corrupt_canonical_slash_proposal_bytes() {
        let mut actual = actual::SorafsRepair::default();
        actual.max_attempts = 1;
        let (manager, temp_dir) = manager_with_config(RepairConfig::from(&actual));
        let report = report(
            "REP-PROPOSAL-CORRUPT-CHECKPOINT",
            [0x2A; 32],
            [0x3A; 32],
            1_700_100_368,
        );
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        manager
            .mark_failed(
                &report.ticket_id,
                report.submitted_at_unix + 1,
                "failure".to_owned(),
            )
            .expect("draft proposal");
        let path = canonical_temp_path(&temp_dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        let bytes = fs::read(&path).expect("read valid drafted checkpoint");
        let mut snapshot: RepairStoreSnapshot =
            norito::decode_from_bytes(&bytes).expect("decode valid drafted checkpoint");
        drop(manager);

        let proposal_bytes = snapshot.tasks[0]
            .slash_proposal_bytes
            .as_mut()
            .expect("drafted proposal bytes");
        proposal_bytes.truncate(norito::core::Header::SIZE - 1);
        snapshot.tasks[0].slash_proposal_digest = Some(checkpoint_digest(proposal_bytes));
        write_private_file(
            &path,
            &norito::to_bytes(&snapshot).expect("encode corrupt nested proposal checkpoint"),
        );
        let error = FileRepairStore::load_or_new(
            path,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("truncated canonical proposal rejected");
        let message = error.to_string();
        assert!(
            message.contains("persisted repair slash proposal")
                && message.contains("truncated before its Norito header"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn submit_slash_proposal_rejects_unverifiable_embedded_approval() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-326", [0x21; 32], [0x31; 32], 1_700_100_370);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        let policy = RepairConfig::default().escalation_policy().clone();
        let approval = approval_for_policy(&policy, report.submitted_at_unix + 10);
        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: report.ticket_id.clone(),
            provider_id: report.evidence.provider_id,
            manifest_digest: report.evidence.manifest_digest,
            auditor_account: report.auditor_account.clone(),
            proposed_penalty: quantity_from_nanos(1_000),
            submitted_at_unix: report.submitted_at_unix + 10,
            rationale: "valid approval".into(),
            approval: Some(approval),
        };

        let error = manager
            .submit_slash_proposal_with_event(proposal)
            .expect_err("self-reported approval summary must not authorize slashing");
        assert!(matches!(
            error,
            RepairSchedulerError::PolicyViolation { reason, .. }
                if reason.contains("not authoritative")
        ));
        let task = manager
            .load_task(&report.ticket_id)
            .expect("load repair task");
        assert!(matches!(task.state, RepairTaskStateV1::Queued(_)));
        assert!(task.governance.decision.is_none());
    }

    #[test]
    fn submit_slash_proposal_rejects_conflicting_proposal() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-327", [0x21; 32], [0x31; 32], 1_700_100_380);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");

        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: report.ticket_id.clone(),
            provider_id: report.evidence.provider_id,
            manifest_digest: report.evidence.manifest_digest,
            auditor_account: report.auditor_account.clone(),
            proposed_penalty: quantity_from_nanos(1_000),
            submitted_at_unix: report.submitted_at_unix + 10,
            rationale: "first proposal".into(),
            approval: None,
        };
        manager
            .submit_slash_proposal_with_event(proposal)
            .expect("initial proposal should be accepted");

        let conflicting = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: report.ticket_id.clone(),
            provider_id: report.evidence.provider_id,
            manifest_digest: report.evidence.manifest_digest,
            auditor_account: report.auditor_account.clone(),
            proposed_penalty: quantity_from_nanos(2_000),
            submitted_at_unix: report.submitted_at_unix + 20,
            rationale: "conflicting proposal".into(),
            approval: None,
        };
        let err = manager
            .submit_slash_proposal_with_event(conflicting)
            .expect_err("conflicting proposal should be rejected");
        assert!(matches!(err, RepairSchedulerError::PolicyViolation { .. }));
    }

    #[test]
    fn exact_proposal_and_vote_replays_do_not_rewrite_checkpoint() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-NOOP-REPLAY", [0x31; 32], [0x41; 32], 1_700_030_000);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: report.ticket_id.clone(),
            provider_id: report.evidence.provider_id,
            manifest_digest: report.evidence.manifest_digest,
            auditor_account: report.auditor_account.clone(),
            proposed_penalty: quantity_from_nanos(1),
            submitted_at_unix: report.submitted_at_unix + 1,
            rationale: "escalate".to_owned(),
            approval: None,
        };
        manager
            .submit_slash_proposal(proposal.clone())
            .expect("first proposal");
        let proposal_revision = manager
            .load_task(&report.ticket_id)
            .expect("load proposal task")
            .revision;
        manager
            .submit_slash_proposal(proposal)
            .expect("exact proposal replay");
        assert_eq!(
            manager
                .load_task(&report.ticket_id)
                .expect("reload proposal task")
                .revision,
            proposal_revision
        );

        manager
            .submit_slash_approval(&report.ticket_id, "voter-a", report.submitted_at_unix + 2)
            .expect("first vote");
        let vote_revision = manager
            .load_task(&report.ticket_id)
            .expect("load vote task")
            .revision;
        manager
            .submit_slash_approval(&report.ticket_id, "voter-a", report.submitted_at_unix + 2)
            .expect("exact vote replay");
        assert_eq!(
            manager
                .load_task(&report.ticket_id)
                .expect("reload vote task")
                .revision,
            vote_revision
        );
    }

    #[test]
    fn escalated_task_keeps_policy_snapshot_across_config_change() {
        let dir = tempdir().expect("tempdir");
        let root = canonical_temp_path(&dir);
        let config = enable_repair_config(RepairConfig::default()).with_default_state_dir(&root);
        let initial_policy =
            RepairEscalationPolicy::from_policy(&actual::RepairEscalationPolicyV1 {
                quorum_bps: 5_000,
                minimum_voters: 2,
                dispute_window_secs: 100,
                appeal_window_secs: 100,
                max_penalty: quantity_from_nanos(10_000),
            });
        let manager = RepairManager::new_with_config_and_policy(config.clone(), initial_policy);
        let report = report("REP-POLICY-SNAPSHOT", [0x51; 32], [0x61; 32], 1_700_040_000);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        manager
            .submit_slash_proposal(RepairSlashProposalV1 {
                version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
                ticket_id: report.ticket_id.clone(),
                provider_id: report.evidence.provider_id,
                manifest_digest: report.evidence.manifest_digest,
                auditor_account: report.auditor_account.clone(),
                proposed_penalty: quantity_from_nanos(9_000),
                submitted_at_unix: report.submitted_at_unix + 1,
                rationale: "snapshot policy".to_owned(),
                approval: None,
            })
            .expect("escalate under initial policy");
        drop(manager);

        let changed_policy =
            RepairEscalationPolicy::from_policy(&actual::RepairEscalationPolicyV1 {
                quorum_bps: 9_000,
                minimum_voters: 9,
                dispute_window_secs: 1,
                appeal_window_secs: 1,
                max_penalty: quantity_from_nanos(1),
            });
        let reloaded = RepairManager::try_new_with_config_policy_and_limits(
            config,
            changed_policy,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("historical task validates against its persisted policy");
        reloaded
            .submit_slash_approval(
                &report.ticket_id,
                "voter-after-restart",
                report.submitted_at_unix + 50,
            )
            .expect("vote uses persisted 100-second dispute window");
    }

    #[test]
    fn governance_votes_finalize_after_dispute_window() {
        let policy = actual::RepairEscalationPolicyV1 {
            quorum_bps: 6_000,
            minimum_voters: 3,
            dispute_window_secs: 10,
            appeal_window_secs: 120,
            max_penalty: quantity_from_nanos(1_000_000_000),
        };
        let actual = actual::SorafsRepair {
            max_attempts: 1,
            default_slash_penalty: quantity_from_nanos(5_000),
            ..Default::default()
        };
        let config = RepairConfig::from_repair_and_policy(&actual, &policy);
        let (manager, _temp_dir) = manager_with_config(config);
        let report = report("REP-328", [0x22; 32], [0x32; 32], 1_700_100_390);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");

        let update = manager
            .mark_failed_with_event(
                &report.ticket_id,
                report.submitted_at_unix + 20,
                "loss".into(),
            )
            .expect("mark failed");
        let escalated_at_unix = match update.record.state {
            RepairTaskStateV1::Escalated(state) => state.escalated_at_unix,
            other => panic!("expected escalated state, got {other:?}"),
        };

        manager
            .submit_slash_approval(&report.ticket_id, "voter-a", escalated_at_unix + 1)
            .expect("approval vote");
        manager
            .submit_slash_approval(&report.ticket_id, "voter-b", escalated_at_unix + 2)
            .expect("approval vote");
        manager
            .submit_slash_rejection(&report.ticket_id, "voter-c", escalated_at_unix + 3)
            .expect("rejection vote");

        let now_unix = escalated_at_unix + policy.dispute_window_secs + 1;
        manager.run_watchdog(now_unix).expect("watchdog");
        let task = manager
            .load_task(&report.ticket_id)
            .expect("load repair task");
        match task.governance.decision {
            Some(RepairGovernanceDecision::Approved {
                decided_at_unix,
                approvals,
                rejections,
            }) => {
                assert_eq!(
                    decided_at_unix,
                    escalated_at_unix + policy.dispute_window_secs
                );
                assert_eq!(approvals, 2);
                assert_eq!(rejections, 1);
            }
            other => panic!("expected approval decision, got {other:?}"),
        }
    }

    #[test]
    fn governance_rejects_insufficient_quorum() {
        let policy = actual::RepairEscalationPolicyV1 {
            quorum_bps: 6_000,
            minimum_voters: 2,
            dispute_window_secs: 5,
            appeal_window_secs: 60,
            max_penalty: quantity_from_nanos(1_000_000_000),
        };
        let actual = actual::SorafsRepair {
            max_attempts: 1,
            default_slash_penalty: quantity_from_nanos(5_000),
            ..Default::default()
        };
        let config = RepairConfig::from_repair_and_policy(&actual, &policy);
        let (manager, _temp_dir) = manager_with_config(config);
        let report = report("REP-329", [0x23; 32], [0x33; 32], 1_700_100_400);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");

        let update = manager
            .mark_failed_with_event(
                &report.ticket_id,
                report.submitted_at_unix + 20,
                "loss".into(),
            )
            .expect("mark failed");
        let escalated_at_unix = match update.record.state {
            RepairTaskStateV1::Escalated(state) => state.escalated_at_unix,
            other => panic!("expected escalated state, got {other:?}"),
        };

        manager
            .submit_slash_approval(&report.ticket_id, "voter-a", escalated_at_unix + 1)
            .expect("approval vote");

        let now_unix = escalated_at_unix + policy.dispute_window_secs + 1;
        manager.run_watchdog(now_unix).expect("watchdog");
        let task = manager
            .load_task(&report.ticket_id)
            .expect("load repair task");
        match task.governance.decision {
            Some(RepairGovernanceDecision::Rejected { reason, .. }) => {
                assert_eq!(reason, RepairGovernanceRejectReason::InsufficientQuorum);
            }
            other => panic!("expected rejection decision, got {other:?}"),
        }
    }

    #[test]
    fn submit_slash_appeal_records_appeal() {
        let policy = actual::RepairEscalationPolicyV1 {
            quorum_bps: 6_000,
            minimum_voters: 2,
            dispute_window_secs: 5,
            appeal_window_secs: 30,
            max_penalty: quantity_from_nanos(1_000_000_000),
        };
        let actual = actual::SorafsRepair {
            max_attempts: 1,
            default_slash_penalty: quantity_from_nanos(5_000),
            ..Default::default()
        };
        let config = RepairConfig::from_repair_and_policy(&actual, &policy);
        let (manager, _temp_dir) = manager_with_config(config);
        let report = report("REP-330", [0x24; 32], [0x34; 32], 1_700_100_410);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");

        let update = manager
            .mark_failed_with_event(
                &report.ticket_id,
                report.submitted_at_unix + 20,
                "loss".into(),
            )
            .expect("mark failed");
        let escalated_at_unix = match update.record.state {
            RepairTaskStateV1::Escalated(state) => state.escalated_at_unix,
            other => panic!("expected escalated state, got {other:?}"),
        };

        manager
            .submit_slash_approval(&report.ticket_id, "voter-a", escalated_at_unix + 1)
            .expect("approval vote");
        manager
            .submit_slash_approval(&report.ticket_id, "voter-b", escalated_at_unix + 2)
            .expect("approval vote");

        let expected_approved_at = escalated_at_unix + policy.dispute_window_secs;
        let now_unix = expected_approved_at + 1;
        manager.run_watchdog(now_unix).expect("watchdog");

        let appeal_at = expected_approved_at + 10;
        manager
            .submit_slash_appeal(
                &report.ticket_id,
                "provider#sora",
                appeal_at,
                Some("appeal".into()),
            )
            .expect("appeal should be accepted");
        let task = manager
            .load_task(&report.ticket_id)
            .expect("load repair task");
        match task.governance.decision {
            Some(RepairGovernanceDecision::Appealed {
                approved_at_unix,
                appealed_at_unix,
                appellant,
                ..
            }) => {
                assert_eq!(approved_at_unix, expected_approved_at);
                assert_eq!(appealed_at_unix, appeal_at);
                assert_eq!(appellant, "provider#sora");
            }
            other => panic!("expected appeal decision, got {other:?}"),
        }
    }

    #[test]
    fn submit_slash_proposal_rejects_insufficient_quorum() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-327", [0x21; 32], [0x31; 32], 1_700_100_380);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        let policy = RepairConfig::default().escalation_policy().clone();
        let mut approval = approval_for_policy(&policy, report.submitted_at_unix + 10);
        approval.approve_votes = 1;
        approval.reject_votes = 2;
        approval.abstain_votes = 0;

        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: report.ticket_id.clone(),
            provider_id: report.evidence.provider_id,
            manifest_digest: report.evidence.manifest_digest,
            auditor_account: report.auditor_account.clone(),
            proposed_penalty: quantity_from_nanos(1_000),
            submitted_at_unix: report.submitted_at_unix + 10,
            rationale: "low quorum".into(),
            approval: Some(approval),
        };

        let err = manager
            .submit_slash_proposal_with_event(proposal)
            .expect_err("quorum should be enforced");
        assert!(matches!(err, RepairSchedulerError::PolicyViolation { .. }));
    }

    #[test]
    fn submit_slash_proposal_rejects_tied_votes() {
        let policy = actual::RepairEscalationPolicyV1 {
            quorum_bps: 5_000,
            minimum_voters: 2,
            ..Default::default()
        };
        let repair_cfg = actual::SorafsRepair {
            ..Default::default()
        };
        let config = RepairConfig::from_repair_and_policy(&repair_cfg, &policy);
        let (manager, _temp_dir) = manager_with_config(config.clone());
        let report = report("REP-328", [0x21; 32], [0x31; 32], 1_700_100_390);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        let policy = config.escalation_policy().clone();
        let mut approval = approval_for_policy(&policy, report.submitted_at_unix + 10);
        approval.approve_votes = 1;
        approval.reject_votes = 1;
        approval.abstain_votes = 0;

        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: report.ticket_id.clone(),
            provider_id: report.evidence.provider_id,
            manifest_digest: report.evidence.manifest_digest,
            auditor_account: report.auditor_account.clone(),
            proposed_penalty: quantity_from_nanos(1_000),
            submitted_at_unix: report.submitted_at_unix + 10,
            rationale: "tie votes".into(),
            approval: Some(approval),
        };

        let err = manager
            .submit_slash_proposal_with_event(proposal)
            .expect_err("tie votes should be rejected");
        assert!(matches!(err, RepairSchedulerError::PolicyViolation { .. }));
    }

    #[test]
    fn submit_slash_proposal_rejects_appeal_window() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-329", [0x21; 32], [0x31; 32], 1_700_100_400);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        let policy = RepairConfig::default().escalation_policy().clone();
        let mut approval = approval_for_policy(&policy, report.submitted_at_unix + 10);
        approval.finalized_at_unix = approval.approved_at_unix;

        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: report.ticket_id.clone(),
            provider_id: report.evidence.provider_id,
            manifest_digest: report.evidence.manifest_digest,
            auditor_account: report.auditor_account.clone(),
            proposed_penalty: quantity_from_nanos(1_000),
            submitted_at_unix: report.submitted_at_unix + 10,
            rationale: "appeal window".into(),
            approval: Some(approval),
        };

        let err = manager
            .submit_slash_proposal_with_event(proposal)
            .expect_err("appeal window should be enforced");
        assert!(matches!(err, RepairSchedulerError::PolicyViolation { .. }));
    }

    #[test]
    fn claim_ticket_sets_in_progress_and_is_idempotent() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-450", [0x44; 32], [0x55; 32], 1_700_000_000);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");

        let record = manager
            .claim_ticket(
                &report.ticket_id,
                "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                report.submitted_at_unix + 10,
                "key-1",
            )
            .expect("claim ticket");
        match record.state {
            RepairTaskStateV1::InProgress(ref state) => {
                assert_eq!(state.started_at_unix, report.submitted_at_unix + 10);
                assert_eq!(
                    state.repair_agent.as_deref(),
                    Some("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
                );
            }
            other => panic!("unexpected state {other:?}"),
        }

        let replay = manager
            .claim_ticket(
                &report.ticket_id,
                "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                report.submitted_at_unix + 10,
                "key-1",
            )
            .expect("idempotent claim");
        assert_eq!(replay, record);
    }

    #[test]
    fn claim_ticket_with_event_emits_once() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-451", [0x44; 32], [0x55; 32], 1_700_000_050);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");

        let update = manager
            .claim_ticket_with_event(
                &report.ticket_id,
                "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                report.submitted_at_unix + 10,
                "key-1",
            )
            .expect("claim ticket");
        assert!(update.event.is_some());

        let replay = manager
            .claim_ticket_with_event(
                &report.ticket_id,
                "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                report.submitted_at_unix + 10,
                "key-1",
            )
            .expect("idempotent claim");
        assert!(replay.event.is_none());
    }

    #[test]
    fn heartbeat_ticket_rejects_out_of_order_updates() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-451", [0x66; 32], [0x77; 32], 1_700_000_100);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        manager
            .claim_ticket(
                &report.ticket_id,
                "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
                report.submitted_at_unix + 5,
                "claim-1",
            )
            .expect("claim ticket");

        manager
            .heartbeat_ticket(
                &report.ticket_id,
                "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
                report.submitted_at_unix + 15,
                "hb-1",
            )
            .expect("heartbeat ticket");
        let stale = manager.heartbeat_ticket(
            &report.ticket_id,
            "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
            report.submitted_at_unix + 12,
            "hb-2",
        );
        assert!(matches!(
            stale,
            Err(RepairSchedulerError::InvalidTimestamp { .. })
        ));
    }

    #[test]
    fn complete_ticket_transitions_to_completed() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-452", [0x88; 32], [0x99; 32], 1_700_000_200);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        manager
            .claim_ticket(
                &report.ticket_id,
                "worker-c",
                report.submitted_at_unix + 5,
                "claim-2",
            )
            .expect("claim ticket");

        let record = manager
            .complete_ticket(
                &report.ticket_id,
                "worker-c",
                report.submitted_at_unix + 25,
                Some("resolved".into()),
                "complete-1",
            )
            .expect("complete ticket");
        match record.state {
            RepairTaskStateV1::Completed(state) => {
                assert_eq!(state.completed_at_unix, report.submitted_at_unix + 25);
                assert_eq!(state.resolution_notes.as_deref(), Some("resolved"));
            }
            other => panic!("unexpected state {other:?}"),
        }
    }

    #[test]
    fn complete_ticket_with_event_emits_transition() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-452B", [0x88; 32], [0x99; 32], 1_700_000_210);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        manager
            .claim_ticket(
                &report.ticket_id,
                "worker-c",
                report.submitted_at_unix + 5,
                "claim-2",
            )
            .expect("claim ticket");

        let update = manager
            .complete_ticket_with_event(
                &report.ticket_id,
                "worker-c",
                report.submitted_at_unix + 25,
                Some("resolved".into()),
                "complete-1",
            )
            .expect("complete ticket");
        let event = update.event.expect("event emitted");
        assert_eq!(event.status, RepairTaskStatusV1::Completed);
    }

    #[test]
    fn fail_ticket_transitions_to_failed() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-453", [0xaa; 32], [0xbb; 32], 1_700_000_300);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        manager
            .claim_ticket(
                &report.ticket_id,
                "worker-d",
                report.submitted_at_unix + 8,
                "claim-3",
            )
            .expect("claim ticket");

        let record = manager
            .fail_ticket(
                &report.ticket_id,
                "worker-d",
                report.submitted_at_unix + 18,
                "disk error".into(),
                "fail-1",
            )
            .expect("fail ticket");
        match record.state {
            RepairTaskStateV1::Failed(state) => {
                assert_eq!(state.failed_at_unix, report.submitted_at_unix + 18);
                assert_eq!(state.reason, "disk error");
            }
            other => panic!("unexpected state {other:?}"),
        }
    }

    #[test]
    fn fail_ticket_with_event_returns_slash_on_escalation() {
        let actual = actual::SorafsRepair {
            max_attempts: 1,
            default_slash_penalty: quantity_from_nanos(12_345),
            ..Default::default()
        };
        let (manager, _temp_dir) = manager_with_config(RepairConfig::from(&actual));
        let report = report("REP-453B", [0xaa; 32], [0xbb; 32], 1_700_000_310);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        manager
            .claim_ticket(
                &report.ticket_id,
                "worker-d",
                report.submitted_at_unix + 8,
                "claim-3",
            )
            .expect("claim ticket");

        let update = manager
            .fail_ticket_with_event(
                &report.ticket_id,
                "worker-d",
                report.submitted_at_unix + 18,
                "disk error".into(),
                "fail-1",
            )
            .expect("fail ticket");
        assert!(matches!(
            update.record.state,
            RepairTaskStateV1::Escalated(..)
        ));
        assert!(update.slash_proposal.is_some());
    }

    #[test]
    fn attempt_cap_escalates_failed_ticket() {
        let actual = actual::SorafsRepair {
            max_attempts: 1,
            default_slash_penalty: quantity_from_nanos(12_345),
            ..Default::default()
        };
        let (manager, _temp_dir) = manager_with_config(RepairConfig::from(&actual));
        let report = report("REP-454", [0xca; 32], [0xdd; 32], 1_700_000_400);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        manager
            .claim_ticket(
                &report.ticket_id,
                "worker-e",
                report.submitted_at_unix + 5,
                "claim-5",
            )
            .expect("claim ticket");

        let record = manager
            .fail_ticket(
                &report.ticket_id,
                "worker-e",
                report.submitted_at_unix + 12,
                "media loss".into(),
                "fail-2",
            )
            .expect("fail ticket");
        assert!(matches!(record.state, RepairTaskStateV1::Escalated(..)));

        let snapshot = manager
            .task_snapshot(&report.ticket_id)
            .expect("load snapshot")
            .expect("snapshot");
        assert_eq!(snapshot.events_dropped, 0);
        let statuses: Vec<_> = snapshot.events.iter().map(|event| event.status).collect();
        assert_eq!(
            statuses,
            vec![
                RepairTaskStatusV1::Queued,
                RepairTaskStatusV1::InProgress,
                RepairTaskStatusV1::Escalated
            ]
        );
    }

    #[test]
    fn watchdog_escalates_sla_breach_with_draft() {
        let actual = actual::SorafsRepair {
            default_slash_penalty: quantity_from_nanos(98_765),
            ..Default::default()
        };
        let (manager, _temp_dir) = manager_with_config(RepairConfig::from(&actual));
        let report = report("REP-455", [0xee; 32], [0xff; 32], 1_700_000_500);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");

        let now = report.submitted_at_unix + DEFAULT_REPAIR_SLA_SECS + 1;
        let outcome = manager.run_watchdog(now).expect("run watchdog");
        assert_eq!(outcome.escalated.len(), 1);
        assert_eq!(outcome.escalated[0].ticket_id, report.ticket_id);
        assert_eq!(
            outcome.escalated[0].proposed_penalty,
            actual.default_slash_penalty
        );
        assert_eq!(outcome.events.len(), 1);
        assert_eq!(outcome.events[0].status, RepairTaskStatusV1::Escalated);

        let record = manager
            .task_record(&report.ticket_id)
            .expect("load record")
            .expect("record");
        assert!(matches!(record.state, RepairTaskStateV1::Escalated(..)));
    }

    #[test]
    fn watchdog_requeues_expired_lease_with_backoff() {
        let actual = actual::SorafsRepair {
            claim_ttl_secs: 10,
            backoff_initial_secs: 5,
            backoff_max_secs: 5,
            ..Default::default()
        };
        let (manager, _temp_dir) = manager_with_config(RepairConfig::from(&actual));
        let report = report("REP-456", [0x01; 32], [0x02; 32], 1_700_000_600);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        manager
            .claim_ticket(
                &report.ticket_id,
                "worker-f",
                report.submitted_at_unix + 10,
                "claim-6",
            )
            .expect("claim ticket");

        let watchdog_at = report.submitted_at_unix + 10 + actual.claim_ttl_secs + 1;
        let outcome = manager.run_watchdog(watchdog_at).expect("run watchdog");
        assert_eq!(outcome.requeued, vec![report.ticket_id.clone()]);
        assert_eq!(outcome.lease_expired, 1);
        assert_eq!(outcome.events.len(), 1);
        assert_eq!(outcome.events[0].status, RepairTaskStatusV1::Queued);

        let backoff_claim =
            manager.claim_ticket(&report.ticket_id, "worker-f", watchdog_at, "claim-7");
        assert!(matches!(
            backoff_claim,
            Err(RepairSchedulerError::BackoffActive { .. })
        ));

        let record = manager
            .claim_ticket(
                &report.ticket_id,
                "worker-f",
                watchdog_at + actual.backoff_initial_secs + 1,
                "claim-8",
            )
            .expect("claim after backoff");
        assert!(matches!(record.state, RepairTaskStateV1::InProgress(..)));
    }

    #[test]
    fn watchdog_requeues_failed_tasks_after_backoff() {
        let actual = actual::SorafsRepair {
            backoff_initial_secs: 5,
            backoff_max_secs: 5,
            ..Default::default()
        };
        let (manager, _temp_dir) = manager_with_config(RepairConfig::from(&actual));
        let report = report("REP-457", [0x03; 32], [0x04; 32], 1_700_000_700);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        manager
            .claim_ticket(
                &report.ticket_id,
                "worker-g",
                report.submitted_at_unix + 5,
                "claim-9",
            )
            .expect("claim ticket");
        manager
            .fail_ticket(
                &report.ticket_id,
                "worker-g",
                report.submitted_at_unix + 12,
                "disk".into(),
                "fail-3",
            )
            .expect("fail ticket");

        let before_backoff = report.submitted_at_unix + 12 + actual.backoff_initial_secs - 1;
        let _ = manager.run_watchdog(before_backoff).expect("watchdog");
        let failed = manager
            .task_record(&report.ticket_id)
            .expect("load failed record")
            .expect("record");
        assert!(matches!(failed.state, RepairTaskStateV1::Failed(..)));

        let after_backoff = report.submitted_at_unix + 12 + actual.backoff_initial_secs + 1;
        let outcome = manager.run_watchdog(after_backoff).expect("watchdog");
        assert_eq!(outcome.requeued, vec![report.ticket_id.clone()]);
        assert_eq!(outcome.events.len(), 1);
        assert_eq!(outcome.events[0].status, RepairTaskStatusV1::Queued);
        let queued = manager
            .task_record(&report.ticket_id)
            .expect("load queued record")
            .expect("record");
        assert!(matches!(queued.state, RepairTaskStateV1::Queued(..)));
    }

    #[test]
    fn claimable_tasks_prioritize_deadline_severity_and_provider_impact() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let provider_a = [0x10; 32];
        let provider_b = [0x11; 32];
        let provider_c = [0x12; 32];

        let early = report("REP-500", [0x20; 32], provider_c, 1_000);
        let mut severe = report("REP-501", [0x21; 32], provider_b, 2_000);
        severe.evidence.cause =
            RepairCauseV1::ReplicaShortfall(RepairReplicaShortfallCauseV1 { missing_chunks: 5 });
        let later_a = report("REP-502", [0x22; 32], provider_a, 2_000);
        let later_b = report("REP-503", [0x23; 32], provider_a, 2_000);

        manager.enqueue_report(early).expect("enqueue early");
        manager.enqueue_report(severe).expect("enqueue severe");
        manager.enqueue_report(later_a).expect("enqueue later a");
        manager.enqueue_report(later_b).expect("enqueue later b");

        let ordered = manager.claimable_tasks(2_500).expect("claimable tasks");
        let ids: Vec<_> = ordered
            .iter()
            .map(|task| task.ticket_id.0.as_str())
            .collect();
        assert_eq!(ids, vec!["REP-500", "REP-501", "REP-502", "REP-503"]);
    }

    #[test]
    fn task_snapshots_include_event_log() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-460", [0x10; 32], [0x20; 32], 1_700_000_000);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        manager
            .claim_ticket(
                &report.ticket_id,
                "worker-e",
                report.submitted_at_unix + 5,
                "claim-4",
            )
            .expect("claim ticket");
        manager
            .complete_ticket(
                &report.ticket_id,
                "worker-e",
                report.submitted_at_unix + 15,
                Some("ok".into()),
                "complete-4",
            )
            .expect("complete ticket");

        let snapshot = manager
            .task_snapshot(&report.ticket_id)
            .expect("load snapshot")
            .expect("snapshot");
        let statuses: Vec<_> = snapshot.events.iter().map(|event| event.status).collect();
        assert_eq!(
            statuses,
            vec![
                RepairTaskStatusV1::Queued,
                RepairTaskStatusV1::InProgress,
                RepairTaskStatusV1::Completed
            ]
        );
        assert_eq!(
            snapshot.events[0].actor.as_deref(),
            Some("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        );
        assert_eq!(snapshot.events[1].actor.as_deref(), Some("worker-e"));
        assert_eq!(snapshot.events[2].actor.as_deref(), Some("worker-e"));
    }

    #[test]
    fn event_log_trims_oldest_entries() {
        let report = report("REP-700", [0x30; 32], [0x40; 32], 1_700_000_000);
        let mut task = task_internal(report);
        task.push_event(RepairTaskStatusV1::Queued, 1, None, Some("one".into()), 2);
        task.push_event(
            RepairTaskStatusV1::InProgress,
            2,
            None,
            Some("two".into()),
            2,
        );
        task.push_event(
            RepairTaskStatusV1::Completed,
            3,
            None,
            Some("three".into()),
            2,
        );

        assert_eq!(task.events.len(), 2);
        assert_eq!(task.events[0].occurred_at_unix, 2);
        assert_eq!(task.events[1].occurred_at_unix, 3);
    }

    #[test]
    fn task_snapshot_discloses_retained_event_suffix_after_restart() {
        let (mut manager, _temp_dir) = manager_with_temp_dir();
        manager.event_history_limit = 2;
        let config = manager.config.clone();
        let report = report("REP-EVENT-SUFFIX", [0x31; 32], [0x41; 32], 1_700_000_100);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        manager
            .mark_in_progress(
                &report.ticket_id,
                report.submitted_at_unix + 1,
                Some("repair-agent".to_owned()),
            )
            .expect("mark in progress");
        manager
            .mark_completed(
                &report.ticket_id,
                report.submitted_at_unix + 2,
                Some("complete".to_owned()),
            )
            .expect("mark complete");
        let snapshot = manager
            .task_snapshot(&report.ticket_id)
            .expect("read snapshot")
            .expect("snapshot exists");
        assert_eq!(snapshot.events_dropped, 1);
        assert_eq!(snapshot.events.len(), 2);
        assert_ne!(snapshot.events[0].status, RepairTaskStatusV1::Queued);
        drop(manager);

        let reloaded = RepairManager::new_with_config(config);
        let snapshot = reloaded
            .task_snapshot(&report.ticket_id)
            .expect("read reloaded snapshot")
            .expect("reloaded snapshot exists");
        assert_eq!(snapshot.events_dropped, 1);
        assert_eq!(snapshot.events.len(), 2);
    }

    #[test]
    fn file_store_compare_and_set_updates_task() {
        let dir = tempdir().expect("tempdir");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        let store = FileRepairStore::load_or_new(
            path,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("store");
        let report = report("REP-900", [0x10; 32], [0x20; 32], 1_700_000_000);
        let task = task_internal(report.clone());
        match store.insert_task(task).expect("insert task") {
            RepairStoreInsertResult::Inserted(_) => {}
            RepairStoreInsertResult::Existing(_) => panic!("expected insert"),
        }

        let mut updated = store
            .task(&report.ticket_id)
            .expect("load task")
            .expect("task present");
        updated.state = RepairTaskStateV1::InProgress(InProgressRepairStateV1 {
            queued_at_unix: report.submitted_at_unix,
            started_at_unix: report.submitted_at_unix + 1,
            repair_agent: None,
        });
        updated.push_event(
            RepairTaskStatusV1::InProgress,
            report.submitted_at_unix + 1,
            None,
            None,
            DEFAULT_REPAIR_EVENT_HISTORY_LIMIT,
        );
        updated.revision = updated.revision.saturating_add(1);
        store
            .compare_and_set_task(&report.ticket_id, 0, updated.clone())
            .expect("compare and set");

        let fetched = store
            .task(&report.ticket_id)
            .expect("load task")
            .expect("task present");
        assert!(matches!(fetched.state, RepairTaskStateV1::InProgress(_)));
        assert_eq!(fetched.revision, 1);

        let mut conflict = fetched.clone();
        conflict.scheduler_notes = Some("conflict".into());
        let err = store
            .compare_and_set_task(&report.ticket_id, 0, conflict)
            .expect_err("expected conflict");
        assert!(matches!(err, RepairStoreError::Conflict { .. }));
    }

    #[test]
    fn file_store_audit_sequence_increments() {
        let dir = tempdir().expect("tempdir");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        let store = FileRepairStore::load_or_new(
            path,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("store");
        let first = store.next_audit_sequence().expect("first sequence");
        let second = store.next_audit_sequence().expect("second sequence");
        assert_eq!(first, 1);
        assert_eq!(second, 2);
    }

    #[test]
    fn file_store_persists_tasks_and_history() {
        let dir = tempdir().expect("tempdir");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        let store = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("store");

        let report = report("REP-950", [0x44; 32], [0x55; 32], 1_700_111_000);
        let task = task_internal(report.clone());
        store.insert_task(task).expect("insert task");

        let observation = PorHistoryObservation {
            manifest_digest: report.evidence.manifest_digest,
            provider_id: report.evidence.provider_id,
            challenge_id: [0x99; 32],
            decided_at: report.submitted_at_unix + 60,
            failed_samples: 4,
        };
        let history_id = store
            .append_por_history(observation.clone())
            .expect("record history");

        let sequence = store.next_audit_sequence().expect("audit sequence");
        assert_eq!(sequence, 1);
        drop(store);

        let reloaded = FileRepairStore::load_or_new(
            path,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("reload store");
        let loaded_task = reloaded
            .task(&report.ticket_id)
            .expect("load task")
            .expect("task present");
        assert_eq!(loaded_task.report.ticket_id, report.ticket_id);
        let loaded_history = reloaded
            .por_history_entry(history_id)
            .expect("history lookup")
            .expect("history entry present");
        assert_eq!(loaded_history.manifest_digest, observation.manifest_digest);
        assert_eq!(loaded_history.failed_samples, observation.failed_samples);
        let next_sequence = reloaded
            .next_audit_sequence()
            .expect("reloaded audit sequence");
        assert_eq!(next_sequence, 2);
    }

    #[test]
    fn worker_idempotency_results_survive_restart_and_reject_equivocation() {
        let dir = tempdir().expect("tempdir");
        let root = canonical_temp_path(&dir);
        let config = enable_repair_config(RepairConfig::default()).with_default_state_dir(&root);
        let manager = RepairManager::new_with_config(config.clone());
        let completed_report = report(
            "REP-IDEMPOTENT-COMPLETE",
            [0xA1; 32],
            [0xB1; 32],
            1_700_010_000,
        );
        manager
            .enqueue_report(completed_report.clone())
            .expect("enqueue completed report");
        let claimed = manager
            .claim_ticket(
                &completed_report.ticket_id,
                "worker-a",
                completed_report.submitted_at_unix + 1,
                "claim-key",
            )
            .expect("claim ticket");
        let heartbeat = manager
            .heartbeat_ticket(
                &completed_report.ticket_id,
                "worker-a",
                completed_report.submitted_at_unix + 2,
                "heartbeat-key",
            )
            .expect("heartbeat ticket");
        let completed = manager
            .complete_ticket(
                &completed_report.ticket_id,
                "worker-a",
                completed_report.submitted_at_unix + 3,
                Some("done".to_owned()),
                "complete-key",
            )
            .expect("complete ticket");

        let failed_report = report("REP-IDEMPOTENT-FAIL", [0xA2; 32], [0xB2; 32], 1_700_020_000);
        manager
            .enqueue_report(failed_report.clone())
            .expect("enqueue failed report");
        manager
            .claim_ticket(
                &failed_report.ticket_id,
                "worker-b",
                failed_report.submitted_at_unix + 1,
                "claim-fail-key",
            )
            .expect("claim failed ticket");
        let failed = manager
            .fail_ticket(
                &failed_report.ticket_id,
                "worker-b",
                failed_report.submitted_at_unix + 2,
                "disk".to_owned(),
                "fail-key",
            )
            .expect("fail ticket");
        drop(manager);

        let reloaded = RepairManager::new_with_config(config);
        assert_eq!(
            reloaded
                .claim_ticket(
                    &completed_report.ticket_id,
                    "worker-a",
                    completed_report.submitted_at_unix + 1,
                    "claim-key",
                )
                .expect("replay claim after restart"),
            claimed
        );
        assert_eq!(
            reloaded
                .heartbeat_ticket(
                    &completed_report.ticket_id,
                    "worker-a",
                    completed_report.submitted_at_unix + 2,
                    "heartbeat-key",
                )
                .expect("replay heartbeat after restart"),
            heartbeat
        );
        assert_eq!(
            reloaded
                .complete_ticket(
                    &completed_report.ticket_id,
                    "worker-a",
                    completed_report.submitted_at_unix + 3,
                    Some("done".to_owned()),
                    "complete-key",
                )
                .expect("replay completion after restart"),
            completed
        );
        assert_eq!(
            reloaded
                .fail_ticket(
                    &failed_report.ticket_id,
                    "worker-b",
                    failed_report.submitted_at_unix + 2,
                    "disk".to_owned(),
                    "fail-key",
                )
                .expect("replay failure after restart"),
            failed
        );
        let equivocation = reloaded
            .complete_ticket(
                &completed_report.ticket_id,
                "worker-a",
                completed_report.submitted_at_unix + 4,
                Some("different".to_owned()),
                "complete-key",
            )
            .expect_err("same key with different payload rejected after restart");
        assert!(matches!(
            equivocation,
            RepairSchedulerError::IdempotencyMismatch { .. }
        ));
    }

    #[test]
    fn checkpoint_rejects_idempotency_results_without_transition_evidence() {
        let (manager, dir) = manager_with_temp_dir();
        let report = report(
            "REP-IDEMPOTENCY-FORGE",
            [0xA3; 32],
            [0xB3; 32],
            1_700_030_000,
        );
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        let bytes = fs::read(&path).expect("checkpoint bytes");
        let snapshot: RepairStoreSnapshot =
            norito::decode_from_bytes(&bytes).expect("decode checkpoint");
        drop(manager);

        let mut forged_complete = snapshot.clone();
        let stored = &mut forged_complete.tasks[0];
        let completed_at = report.submitted_at_unix + 2;
        let complete_record = RepairTaskRecordV1 {
            version: REPAIR_TASK_VERSION_V1,
            ticket_id: report.ticket_id.clone(),
            manifest_digest: report.evidence.manifest_digest,
            provider_id: report.evidence.provider_id,
            auditor_account: report.auditor_account.clone(),
            state: RepairTaskStateV1::Completed(CompletedRepairStateV1 {
                queued_at_unix: report.submitted_at_unix,
                started_at_unix: report.submitted_at_unix + 1,
                completed_at_unix: completed_at,
                resolution_notes: Some("forged".to_owned()),
            }),
            por_history_id: None,
            sla_deadline_unix: stored.sla_deadline_unix,
            scheduler_notes: Some("forged".to_owned()),
            slash_proposal_digest: None,
        };
        stored
            .idempotency
            .complete
            .push(StoredRepairCompleteIdempotency {
                key: "forged-complete".to_owned(),
                signature: RepairCompleteSignature {
                    worker_id: "forged-worker".to_owned(),
                    completed_at_unix: completed_at,
                    resolution_notes: Some("forged".to_owned()),
                },
                record: complete_record,
            });
        write_private_file(
            &path,
            &norito::to_bytes(&forged_complete).expect("encode forged completion"),
        );
        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("unreachable completion result rejected");
        assert!(error.to_string().contains("unreachable completion"));

        let mut forged_claim = snapshot;
        let stored = &mut forged_claim.tasks[0];
        let claimed_at = report.submitted_at_unix + 1;
        stored.idempotency.claim.push(StoredRepairClaimIdempotency {
            key: "forged-claim".to_owned(),
            signature: RepairClaimSignature {
                worker_id: "forged-worker".to_owned(),
                claimed_at_unix: claimed_at,
            },
            record: RepairTaskRecordV1 {
                version: REPAIR_TASK_VERSION_V1,
                ticket_id: report.ticket_id.clone(),
                manifest_digest: report.evidence.manifest_digest,
                provider_id: report.evidence.provider_id,
                auditor_account: report.auditor_account.clone(),
                state: RepairTaskStateV1::InProgress(InProgressRepairStateV1 {
                    queued_at_unix: report.submitted_at_unix,
                    started_at_unix: claimed_at,
                    repair_agent: Some("forged-worker".to_owned()),
                }),
                por_history_id: None,
                sla_deadline_unix: stored.sla_deadline_unix,
                scheduler_notes: None,
                slash_proposal_digest: None,
            },
        });
        write_private_file(
            &path,
            &norito::to_bytes(&forged_claim).expect("encode forged claim"),
        );
        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("claim result without event rejected");
        assert!(error.to_string().contains("claim idempotency result"));

        let mut forged_heartbeat = norito::decode_from_bytes::<RepairStoreSnapshot>(&bytes)
            .expect("decode original checkpoint for heartbeat forgery");
        let stored = &mut forged_heartbeat.tasks[0];
        stored
            .idempotency
            .heartbeat
            .push(StoredRepairHeartbeatIdempotency {
                key: "forged-heartbeat".to_owned(),
                signature: RepairHeartbeatSignature {
                    worker_id: "forged-worker".to_owned(),
                    heartbeat_at_unix: claimed_at + 1,
                },
                record: RepairTaskRecordV1 {
                    version: REPAIR_TASK_VERSION_V1,
                    ticket_id: report.ticket_id.clone(),
                    manifest_digest: report.evidence.manifest_digest,
                    provider_id: report.evidence.provider_id,
                    auditor_account: report.auditor_account.clone(),
                    state: RepairTaskStateV1::InProgress(InProgressRepairStateV1 {
                        queued_at_unix: report.submitted_at_unix,
                        started_at_unix: claimed_at,
                        repair_agent: Some("forged-worker".to_owned()),
                    }),
                    por_history_id: None,
                    sla_deadline_unix: stored.sla_deadline_unix,
                    scheduler_notes: None,
                    slash_proposal_digest: None,
                },
            });
        write_private_file(
            &path,
            &norito::to_bytes(&forged_heartbeat).expect("encode forged heartbeat"),
        );
        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("heartbeat result without claim rejected");
        assert!(error.to_string().contains("heartbeat idempotency result"));

        let mut forged_fail = norito::decode_from_bytes::<RepairStoreSnapshot>(&bytes)
            .expect("decode original checkpoint for failure forgery");
        let stored = &mut forged_fail.tasks[0];
        let failed_at = claimed_at + 1;
        stored.idempotency.fail.push(StoredRepairFailIdempotency {
            key: "forged-fail".to_owned(),
            signature: RepairFailSignature {
                worker_id: "forged-worker".to_owned(),
                failed_at_unix: failed_at,
                reason: "forged failure".to_owned(),
            },
            record: RepairTaskRecordV1 {
                version: REPAIR_TASK_VERSION_V1,
                ticket_id: report.ticket_id.clone(),
                manifest_digest: report.evidence.manifest_digest,
                provider_id: report.evidence.provider_id,
                auditor_account: report.auditor_account.clone(),
                state: RepairTaskStateV1::Failed(FailedRepairStateV1 {
                    queued_at_unix: report.submitted_at_unix,
                    failed_at_unix: failed_at,
                    reason: "forged failure".to_owned(),
                }),
                por_history_id: None,
                sla_deadline_unix: stored.sla_deadline_unix,
                scheduler_notes: Some("forged failure".to_owned()),
                slash_proposal_digest: None,
            },
        });
        write_private_file(
            &path,
            &norito::to_bytes(&forged_fail).expect("encode forged failure"),
        );
        let error = FileRepairStore::load_or_new(
            path,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("failure result without transition event rejected");
        assert!(error.to_string().contains("failure idempotency result"));
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_rejects_symlink_output() {
        let dir = tempdir().expect("tempdir");
        let temp_path = canonical_temp_path(&dir);
        let target_path = temp_path.join("target.to");
        fs::write(&target_path, b"unchanged\n").expect("write target");
        let output_path = temp_path.join(REPAIR_STORE_FILE_NAME);
        std::os::unix::fs::symlink(&target_path, &output_path).expect("create symlink");

        let err = write_atomic(&output_path, b"replace", sync_parent_directory)
            .expect_err("reject symlink output");
        let message = err.to_string();

        assert!(message.contains("symlink"), "unexpected error: {message}");
        assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged\n");
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
        let output_path = linked_dir.join(REPAIR_STORE_FILE_NAME);

        let err = write_atomic(&output_path, b"replace", sync_parent_directory)
            .expect_err("reject symlink parent");
        let message = err.to_string();

        assert!(
            message.contains("parent") && message.contains("must not be a symlink"),
            "unexpected error: {message}"
        );
        assert!(
            !real_dir.join(REPAIR_STORE_FILE_NAME).exists(),
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
        let tmp_path = temp_path.join(".repair_state.to.tmp");
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
    fn corrupt_repair_store_fails_closed_without_replacement() {
        let dir = tempdir().expect("tempdir");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
        write_private_file(&path, b"corrupt");

        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("corrupt store must fail");
        assert!(error.to_string().contains("truncated"));
        assert_eq!(fs::read(path).unwrap(), b"corrupt");
    }

    #[test]
    fn repair_checkpoint_decode_limit_uses_schema_bounds_not_checkpoint_bytes() {
        let default_limits = repair_checkpoint_decode_limits(1, DEFAULT_REPAIR_STORE_MAX_BYTES);
        assert_eq!(
            default_limits.max_sequence_elements(),
            MAX_CANONICAL_REPAIR_SLASH_PROPOSAL_BYTES
        );
        assert_eq!(
            default_limits.max_field_bytes(),
            usize::try_from(DEFAULT_REPAIR_STORE_MAX_BYTES).expect("default byte limit fits usize")
        );
        assert_eq!(
            default_limits.max_total_elements(),
            usize::try_from(DEFAULT_REPAIR_STORE_MAX_BYTES).expect("default byte limit fits usize")
        );
        assert_eq!(
            default_limits.max_total_allocated_bytes(),
            usize::try_from(DEFAULT_REPAIR_STORE_MAX_BYTES).expect("default byte limit fits usize")
                * 4
        );
        assert_eq!(default_limits.max_nesting_depth(), 64);
        let larger_entry_limit = MAX_CANONICAL_REPAIR_SLASH_PROPOSAL_BYTES + 1;
        assert_eq!(
            repair_checkpoint_decode_limits(larger_entry_limit, DEFAULT_REPAIR_STORE_MAX_BYTES)
                .max_sequence_elements(),
            larger_entry_limit
        );
        assert!(
            u64::try_from(
                repair_checkpoint_decode_limits(1, DEFAULT_REPAIR_STORE_MAX_BYTES)
                    .max_sequence_elements(),
            )
            .expect("limit fits u64")
                < DEFAULT_REPAIR_STORE_MAX_BYTES
        );
    }

    #[test]
    fn repair_slash_proposal_decode_limits_are_schema_bounded() {
        let limits = repair_slash_proposal_decode_limits();
        assert_eq!(
            limits.max_sequence_elements(),
            MAX_CANONICAL_REPAIR_SLASH_PROPOSAL_BYTES
        );
        assert_eq!(
            limits.max_field_bytes(),
            MAX_CANONICAL_REPAIR_SLASH_PROPOSAL_BYTES
        );
        assert_eq!(limits.max_total_elements(), 65_536);
        assert_eq!(limits.max_total_allocated_bytes(), 256 * 1024);
        assert_eq!(limits.max_nesting_depth(), 32);
    }

    #[test]
    fn repair_checkpoint_rejects_oversized_nested_sequence_before_semantic_validation() {
        let dir = tempdir().expect("tempdir");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        fs::create_dir_all(path.parent().expect("parent")).expect("create checkpoint parent");

        let mut stored = StoredRepairTask::from_internal(task_internal(report(
            "REP-DECODE-LIMIT",
            [0x31; 32],
            [0x32; 32],
            1_700_000_000,
        )))
        .expect("store task");
        stored.slash_proposal_bytes = Some(vec![0; MAX_CANONICAL_REPAIR_SLASH_PROPOSAL_BYTES + 1]);
        let forged = RepairStoreSnapshot {
            version: REPAIR_STORE_VERSION_V1,
            next_por_history_id: 1,
            next_audit_sequence: 1,
            tasks: vec![stored],
            por_history: Vec::new(),
            auditor_nonces: Vec::new(),
        };
        write_private_file(
            &path,
            &norito::to_bytes(&forged).expect("encode adversarial checkpoint"),
        );

        let error = FileRepairStore::load_or_new(path, 1, DEFAULT_REPAIR_STORE_MAX_BYTES)
            .expect_err("oversized nested sequence must fail at decode boundary");
        let message = error.to_string();
        assert!(
            message.contains(&format!(
                "sequence length {} exceeds decode limit {}",
                MAX_CANONICAL_REPAIR_SLASH_PROPOSAL_BYTES + 1,
                MAX_CANONICAL_REPAIR_SLASH_PROPOSAL_BYTES
            )),
            "unexpected error: {message}"
        );
        assert!(
            !message.contains("persist slash proposal bytes and digest together"),
            "semantic validation ran before the allocation guard: {message}"
        );
    }

    #[test]
    fn fixed_slash_proposal_bound_covers_maximum_v1_fields() {
        let max_string = "A".repeat(MAX_REPAIR_NOTES_BYTES);
        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: RepairTicketId(max_string.clone()),
            provider_id: [0x41; 32],
            manifest_digest: [0x42; 32],
            auditor_account: max_string.clone(),
            proposed_penalty: quantity_from_nanos(u128::MAX),
            submitted_at_unix: u64::MAX,
            rationale: max_string,
            approval: Some(RepairEscalationApprovalV1 {
                version: REPAIR_ESCALATION_APPROVAL_VERSION_V1,
                approve_votes: u32::MAX,
                reject_votes: u32::MAX,
                abstain_votes: u32::MAX,
                approved_at_unix: u64::MAX,
                finalized_at_unix: u64::MAX,
            }),
        };
        proposal
            .validate()
            .expect("maximum-shaped proposal is valid");
        let bytes = norito::to_bytes(&proposal).expect("encode maximum-shaped proposal");
        assert!(
            bytes.len() <= MAX_CANONICAL_REPAIR_SLASH_PROPOSAL_BYTES,
            "maximum-shaped proposal encoded to {} bytes (limit {})",
            bytes.len(),
            MAX_CANONICAL_REPAIR_SLASH_PROPOSAL_BYTES
        );
    }

    #[test]
    fn deleting_initialized_checkpoint_does_not_reset_replay_state() {
        let dir = tempdir().expect("tempdir");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        let store = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("initialize store");
        store
            .record_auditor_nonce("auditor", 7)
            .expect("persist replay state");
        drop(store);
        fs::remove_file(&path).expect("delete checkpoint");

        let error = FileRepairStore::load_or_new(
            path,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("missing initialized checkpoint fails closed");
        assert!(
            error
                .to_string()
                .contains("missing after prior initialization")
        );
    }

    #[test]
    fn repair_store_refuses_entry_and_byte_limit_exhaustion() {
        let dir = tempdir().expect("tempdir");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        let store = FileRepairStore::load_or_new(path.clone(), 1, DEFAULT_REPAIR_STORE_MAX_BYTES)
            .expect("store");
        store
            .insert_task(task_internal(report(
                "REP-LIMIT-1",
                [0x10; 32],
                [0x20; 32],
                1_700_000_000,
            )))
            .unwrap();
        let error = store
            .insert_task(task_internal(report(
                "REP-LIMIT-2",
                [0x11; 32],
                [0x21; 32],
                1_700_000_001,
            )))
            .expect_err("second task exceeds retention");
        assert!(error.to_string().contains("retention exhausted"));

        let ticket_id = RepairTicketId("REP-LIMIT-1".to_owned());
        let mut transition = store
            .task(&ticket_id)
            .expect("load bounded task")
            .expect("bounded task exists");
        transition.state = RepairTaskStateV1::InProgress(InProgressRepairStateV1 {
            queued_at_unix: 1_700_000_000,
            started_at_unix: 1_700_000_001,
            repair_agent: None,
        });
        transition.push_event(
            RepairTaskStatusV1::InProgress,
            1_700_000_001,
            None,
            None,
            DEFAULT_REPAIR_EVENT_HISTORY_LIMIT,
        );
        transition.revision = 1;
        let error = store
            .compare_and_set_task(&ticket_id, 0, transition)
            .expect_err("nested event history cannot exceed the configured entry limit");
        assert!(error.to_string().contains("nested history exceeds"));
        drop(store);
        let reloaded = FileRepairStore::load_or_new(path, 1, DEFAULT_REPAIR_STORE_MAX_BYTES)
            .expect("rejected transition leaves a reloadable checkpoint");
        assert!(matches!(
            reloaded
                .task(&ticket_id)
                .expect("load task after restart")
                .expect("task remains")
                .state,
            RepairTaskStateV1::Queued(_)
        ));

        let path = canonical_temp_path(&dir)
            .join("small")
            .join(REPAIR_STORE_FILE_NAME);
        let mut store =
            FileRepairStore::load_or_new(path.clone(), 8, DEFAULT_REPAIR_STORE_MAX_BYTES)
                .expect("small store");
        store.max_bytes = fs::metadata(&path)
            .expect("empty checkpoint metadata")
            .len();
        let error = store
            .insert_task(task_internal(report(
                "REP-BYTES",
                [0x12; 32],
                [0x22; 32],
                1_700_000_002,
            )))
            .expect_err("encoded checkpoint exceeds byte limit");
        assert!(error.to_string().contains("exceeding limit"));
        assert!(
            store
                .task(&RepairTicketId("REP-BYTES".to_owned()))
                .unwrap()
                .is_none()
        );
    }

    #[cfg(unix)]
    #[test]
    fn repair_store_load_rejects_symlink_and_oversize_files() {
        let dir = tempdir().expect("tempdir");
        let root = canonical_temp_path(&dir);
        let victim = root.join("victim.to");
        fs::write(&victim, b"victim").unwrap();
        let linked = root.join("linked.to");
        std::os::unix::fs::symlink(&victim, &linked).unwrap();
        assert!(FileRepairStore::load_or_new(linked, 8, 128).is_err());

        let oversized = root.join("oversized.to");
        write_private_file(&oversized, &[0_u8; 129]);
        let error =
            FileRepairStore::load_or_new(oversized, 8, 128).expect_err("oversize store rejected");
        assert!(error.to_string().contains("exceeding limit"));
    }

    #[test]
    fn por_history_append_is_exactly_idempotent_and_conflict_safe() {
        let dir = tempdir().expect("tempdir");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        let store = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("store");
        let observation = PorHistoryObservation {
            manifest_digest: [0x11; 32],
            provider_id: [0x22; 32],
            challenge_id: [0x33; 32],
            decided_at: 1_700_000_000,
            failed_samples: 3,
        };

        let first = store
            .append_por_history(observation.clone())
            .expect("first append");
        let replay = store
            .append_por_history(observation.clone())
            .expect("exact replay");
        assert_eq!(first, replay);

        let mut conflict = observation;
        conflict.failed_samples += 1;
        let error = store
            .append_por_history(conflict)
            .expect_err("conflicting replay rejected");
        assert!(error.to_string().contains("conflicting"));

        drop(store);
        let reloaded = FileRepairStore::load_or_new(
            path,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("reload store");
        assert_eq!(
            reloaded
                .append_por_history(PorHistoryObservation {
                    manifest_digest: [0x11; 32],
                    provider_id: [0x22; 32],
                    challenge_id: [0x44; 32],
                    decided_at: 1_700_000_001,
                    failed_samples: 3,
                })
                .expect("next observation"),
            first + 1
        );
    }

    #[test]
    fn lifetime_lock_rejects_second_store_and_external_changes_are_terminal() {
        let dir = tempdir().expect("tempdir");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        let first = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("first store");
        let lock_error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("second live store must not load stale state");
        assert!(lock_error.to_string().contains("locked"));
        let report_a = report("REP-CAS-A", [0x10; 32], [0x20; 32], 1_700_000_001);
        let report_b = report("REP-CAS-B", [0x11; 32], [0x21; 32], 1_700_000_002);

        first
            .insert_task(task_internal(report_a.clone()))
            .expect("first writer commits");
        let canonical_bytes = fs::read(&path).expect("read canonical checkpoint");
        let mut externally_changed: RepairStoreSnapshot =
            norito::decode_from_bytes(&canonical_bytes).expect("decode checkpoint");
        externally_changed.next_audit_sequence = 2;
        write_private_file(
            &path,
            &norito::to_bytes(&externally_changed).expect("encode external checkpoint change"),
        );
        let error = first
            .insert_task(task_internal(report_b.clone()))
            .expect_err("external checkpoint change rejected");
        assert!(matches!(error, RepairStoreError::StaleCheckpoint));
        let terminal = first
            .task(&report_b.ticket_id)
            .expect_err("changed store stops serving reads");
        assert!(terminal.to_string().contains("restart required"));
        write_private_file(&path, &canonical_bytes);
        drop(first);

        let reloaded = FileRepairStore::load_or_new(
            path,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("reload canonical store");
        assert!(
            reloaded
                .task(&report_a.ticket_id)
                .expect("first task lookup")
                .is_some()
        );
        assert!(
            reloaded
                .task(&report_b.ticket_id)
                .expect("second task lookup")
                .is_none()
        );
    }

    #[test]
    fn concurrent_audit_sequence_reservations_are_unique_and_durable() {
        let dir = tempdir().expect("tempdir");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        let store = Arc::new(
            FileRepairStore::load_or_new(
                path.clone(),
                DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
                DEFAULT_REPAIR_STORE_MAX_BYTES,
            )
            .expect("store"),
        );
        let barrier = Arc::new(std::sync::Barrier::new(8));
        let mut workers = Vec::new();
        for _ in 0..8 {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                store.next_audit_sequence().expect("reserve sequence")
            }));
        }
        let mut sequences: Vec<u64> = workers
            .into_iter()
            .map(|worker| worker.join().expect("worker did not panic"))
            .collect();
        sequences.sort_unstable();
        assert_eq!(sequences, (1..=8).collect::<Vec<_>>());
        drop(store);

        let reloaded = FileRepairStore::load_or_new(
            path,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("reload store");
        assert_eq!(reloaded.next_audit_sequence().expect("next sequence"), 9);
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn checkpoint_write_lock_excludes_another_process() {
        const PROBE_ENV: &str = "IROHA_TEST_REPAIR_CHECKPOINT_LOCK_PROBE";
        if let Some(path) = std::env::var_os(PROBE_ENV) {
            let error = acquire_checkpoint_write_lock(Path::new(&path))
                .expect_err("parent process owns the checkpoint lock");
            assert!(error.to_string().contains("lock"));
            return;
        }

        let dir = tempdir().expect("tempdir");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        ensure_secure_store_parent(&path).expect("create secure parent");
        let _lock = acquire_checkpoint_write_lock(&path).expect("acquire parent lock");
        let status = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .arg("--exact")
            .arg("repair::tests::checkpoint_write_lock_excludes_another_process")
            .arg("--nocapture")
            .env(PROBE_ENV, &path)
            .status()
            .expect("run lock probe process");
        assert!(status.success(), "child lock probe failed: {status}");
    }

    #[test]
    fn post_rename_sync_failure_is_terminal_and_restart_observes_commit() {
        fn reject_parent_sync(_parent: &Path) -> io::Result<()> {
            Err(io::Error::other("injected parent sync failure"))
        }

        let dir = tempdir().expect("tempdir");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        let mut store = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("store");
        store.parent_sync = reject_parent_sync;
        let report = report(
            "REP-AMBIGUOUS-COMMIT",
            [0x61; 32],
            [0x71; 32],
            1_700_000_011,
        );
        let error = store
            .insert_task(task_internal(report.clone()))
            .expect_err("post-rename failure reported as uncertain");
        assert!(matches!(error, RepairStoreError::DurabilityUncertain(_)));
        let terminal = store
            .task(&report.ticket_id)
            .expect_err("live store stops after ambiguous durability");
        assert!(terminal.to_string().contains("restart required"));
        drop(store);

        let reloaded = FileRepairStore::load_or_new(
            path,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("restart accepts visible committed checkpoint");
        assert!(
            reloaded
                .task(&report.ticket_id)
                .expect("task lookup")
                .is_some()
        );
    }

    #[cfg(unix)]
    #[test]
    fn sequence_write_failure_returns_error_and_rolls_back_reservation() {
        let dir = tempdir().expect("tempdir");
        let root = canonical_temp_path(&dir);
        let path = root.join("repair").join(REPAIR_STORE_FILE_NAME);
        let store = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("store");
        assert_eq!(store.next_audit_sequence().expect("first sequence"), 1);
        let checkpoint = fs::read(&path).expect("checkpoint bytes");

        fs::remove_file(&path).expect("remove checkpoint");
        let victim = root.join("victim.to");
        write_private_file(&victim, b"unchanged");
        std::os::unix::fs::symlink(&victim, &path).expect("replace with symlink");
        let error = store
            .next_audit_sequence()
            .expect_err("symlink makes reservation fail closed");
        assert!(error.to_string().contains("regular file"));
        assert_eq!(fs::read(&victim).expect("victim bytes"), b"unchanged");

        fs::remove_file(&path).expect("remove symlink");
        write_private_file(&path, &checkpoint);
        assert_eq!(
            store
                .next_audit_sequence()
                .expect("rolled-back sequence can be retried"),
            2
        );
    }

    #[test]
    fn checkpoint_rejects_forged_event_suffix_and_invalid_high_water() {
        let dir = tempdir().expect("tempdir");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        let store = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("store");
        let report = report("REP-FORGE", [0x55; 32], [0x66; 32], 1_700_000_003);
        store
            .insert_task(task_internal(report))
            .expect("insert task");
        let bytes = fs::read(&path).expect("checkpoint bytes");
        let snapshot: RepairStoreSnapshot =
            norito::decode_from_bytes(&bytes).expect("decode checkpoint");
        drop(store);

        let mut forged_event = snapshot.clone();
        forged_event.tasks[0].events[0].provider_id = [0x77; 32];
        write_private_file(
            &path,
            &norito::to_bytes(&forged_event).expect("encode forged event"),
        );
        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("forged event rejected");
        assert!(error.to_string().contains("inconsistent identity"));

        let mut forged_actor = snapshot.clone();
        forged_actor.tasks[0].events[0].actor = Some("forged-auditor".to_owned());
        write_private_file(
            &path,
            &norito::to_bytes(&forged_actor).expect("encode forged actor"),
        );
        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("forged initial actor rejected");
        assert!(error.to_string().contains("forged initial event"));

        let mut forged_message = snapshot.clone();
        forged_message.tasks[0].events[0].message = Some("forged message".to_owned());
        write_private_file(
            &path,
            &norito::to_bytes(&forged_message).expect("encode forged message"),
        );
        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("forged initial message rejected");
        assert!(error.to_string().contains("forged initial event"));

        let mut invalid_history = snapshot;
        invalid_history.por_history.push(PorHistoryEntry {
            id: 1,
            manifest_digest: [0x01; 32],
            provider_id: [0x02; 32],
            challenge_id: [0x03; 32],
            decided_at: 1,
            failed_samples: 1,
        });
        invalid_history.next_por_history_id = 1;
        write_private_file(
            &path,
            &norito::to_bytes(&invalid_history).expect("encode invalid history"),
        );
        let error = FileRepairStore::load_or_new(
            path,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("invalid high-water rejected");
        assert!(error.to_string().contains("high-water"));
    }

    #[test]
    fn checkpoint_rejects_por_history_gaps_and_max_high_water() {
        let dir = tempdir().expect("tempdir");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        fs::create_dir_all(path.parent().expect("checkpoint parent"))
            .expect("create checkpoint parent");
        let base = RepairStoreSnapshot::from_state(&RepairStoreState::new())
            .expect("encode empty state snapshot");

        let mut gap = base.clone();
        gap.por_history.push(PorHistoryEntry {
            id: 2,
            manifest_digest: [0x01; 32],
            provider_id: [0x02; 32],
            challenge_id: [0x03; 32],
            decided_at: 1,
            failed_samples: 1,
        });
        gap.next_por_history_id = 2;
        write_private_file(&path, &norito::to_bytes(&gap).expect("encode gap"));
        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("history gap rejected");
        assert!(error.to_string().contains("contiguous"));

        let mut maximum = base;
        maximum.next_por_history_id = u64::MAX;
        write_private_file(&path, &norito::to_bytes(&maximum).expect("encode maximum"));
        let error = FileRepairStore::load_or_new(
            path,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("maximum high-water rejected");
        assert!(error.to_string().contains("high-water"));
    }

    #[test]
    fn checkpoint_rejects_forged_por_task_history_bindings() {
        let dir = tempdir().expect("tempdir");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        fs::create_dir_all(path.parent().expect("checkpoint parent"))
            .expect("create checkpoint parent");

        let history = PorHistoryEntry {
            id: 1,
            manifest_digest: [0x11; 32],
            provider_id: [0x22; 32],
            challenge_id: [0x33; 32],
            decided_at: 1_700_000_010,
            failed_samples: 4,
        };
        let mut por_report = report(
            "REP-POR-CHECKPOINT",
            history.manifest_digest,
            history.provider_id,
            history.decided_at + 1,
        );
        por_report.evidence.por_history_id = Some(history.id);
        por_report.evidence.cause = RepairCauseV1::PorFailure(RepairPorFailureCauseV1 {
            challenge_id: history.challenge_id,
            failed_samples: u16::try_from(history.failed_samples).expect("sample count fits u16"),
            proof_digest: None,
        });
        let mut state = RepairStoreState::new();
        state
            .tasks
            .insert(por_report.ticket_id.0.clone(), task_internal(por_report));
        state.por_history.insert(history.id, history);
        state.next_por_history_id = 2;
        let valid = RepairStoreSnapshot::from_state(&state).expect("encode valid state snapshot");
        write_private_file(
            &path,
            &norito::to_bytes(&valid).expect("encode valid checkpoint"),
        );
        FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("valid PoR task history binding loads");

        let mut missing = valid.clone();
        missing.tasks[0].report.evidence.por_history_id = None;
        write_private_file(
            &path,
            &norito::to_bytes(&missing).expect("encode missing reference"),
        );
        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("missing PoR task history reference rejected");
        assert!(error.to_string().contains("missing its history reference"));

        let mut wrong_entry = valid.clone();
        wrong_entry.tasks[0].report.evidence.por_history_id = Some(2);
        write_private_file(
            &path,
            &norito::to_bytes(&wrong_entry).expect("encode unknown reference"),
        );
        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("unknown PoR task history reference rejected");
        assert!(error.to_string().contains("unknown history entry"));

        let mut mismatch = valid.clone();
        let RepairCauseV1::PorFailure(cause) = &mut mismatch.tasks[0].report.evidence.cause else {
            panic!("expected PoR failure cause");
        };
        cause.challenge_id = [0x44; 32];
        write_private_file(
            &path,
            &norito::to_bytes(&mismatch).expect("encode mismatched reference"),
        );
        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("mismatched PoR task history reference rejected");
        assert!(error.to_string().contains("does not match history entry"));

        let mut unrelated = valid;
        unrelated.tasks[0].report.evidence.cause = RepairCauseV1::Manual(RepairManualCauseV1 {
            reason: "not a PoR failure".to_owned(),
        });
        write_private_file(
            &path,
            &norito::to_bytes(&unrelated).expect("encode unrelated reference"),
        );
        let error = FileRepairStore::load_or_new(
            path,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("non-PoR task history reference rejected");
        assert!(error.to_string().contains("non-PoR repair task"));
    }

    #[test]
    fn checkpoint_rejects_governance_vote_dos_duplicates_and_forged_counts() {
        let mut actual = actual::SorafsRepair::default();
        actual.max_attempts = 1;
        let (manager, dir) = manager_with_config(RepairConfig::from(&actual));
        let report = report("REP-GOV-CHECKPOINT", [0x81; 32], [0x91; 32], 1_700_000_100);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        manager
            .mark_failed(
                &report.ticket_id,
                report.submitted_at_unix + 1,
                "failure".to_owned(),
            )
            .expect("escalate report");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        let bytes = fs::read(&path).expect("checkpoint bytes");
        let snapshot: RepairStoreSnapshot =
            norito::decode_from_bytes(&bytes).expect("decode checkpoint");
        drop(manager);
        let escalated_at = match snapshot.tasks[0].state {
            RepairTaskStateV1::Escalated(ref state) => state.escalated_at_unix,
            ref other => panic!("expected escalated task, got {other:?}"),
        };

        let mut oversized = snapshot.clone();
        oversized.tasks[0].governance.approvals = (0..3)
            .map(|index| RepairGovernanceVote {
                voter_id: format!("voter-{index}"),
                voted_at_unix: escalated_at + 1,
            })
            .collect();
        write_private_file(
            &path,
            &norito::to_bytes(&oversized).expect("encode oversized votes"),
        );
        let error = FileRepairStore::load_or_new(path.clone(), 2, DEFAULT_REPAIR_STORE_MAX_BYTES)
            .expect_err("oversized governance vectors rejected");
        assert!(error.to_string().contains("governance votes exceed"));

        let mut duplicate = snapshot.clone();
        duplicate.tasks[0].governance.approvals = vec![RepairGovernanceVote {
            voter_id: "same-voter".to_owned(),
            voted_at_unix: escalated_at + 1,
        }];
        duplicate.tasks[0].governance.rejections = vec![RepairGovernanceVote {
            voter_id: "same-voter".to_owned(),
            voted_at_unix: escalated_at + 1,
        }];
        write_private_file(
            &path,
            &norito::to_bytes(&duplicate).expect("encode duplicate voter"),
        );
        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("cross-list duplicate rejected");
        assert!(error.to_string().contains("both sides"));

        let policy = snapshot.tasks[0]
            .governance_policy
            .clone()
            .expect("persisted governance policy");
        let decision_at = escalated_at + policy.dispute_window_secs;
        let mut wrong_reason = snapshot.clone();
        wrong_reason.tasks[0].governance.decision = Some(RepairGovernanceDecision::Rejected {
            decided_at_unix: decision_at,
            approvals: 0,
            rejections: 0,
            reason: RepairGovernanceRejectReason::Tie,
        });
        write_private_file(
            &path,
            &norito::to_bytes(&wrong_reason).expect("encode wrong rejection reason"),
        );
        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("wrong rejection reason rejected");
        assert!(error.to_string().contains("invalid governance decision"));

        let mut below_policy = snapshot.clone();
        let mut proposal: RepairSlashProposalV1 = norito::decode_from_bytes(
            below_policy.tasks[0]
                .slash_proposal_bytes
                .as_ref()
                .expect("slash proposal bytes"),
        )
        .expect("decode slash proposal");
        let approval = RepairEscalationApprovalV1 {
            version: REPAIR_ESCALATION_APPROVAL_VERSION_V1,
            approve_votes: 1,
            reject_votes: 0,
            abstain_votes: 0,
            approved_at_unix: decision_at,
            finalized_at_unix: decision_at + policy.appeal_window_secs,
        };
        proposal.approval = Some(approval.clone());
        let proposal_bytes = norito::to_bytes(&proposal).expect("encode below-policy proposal");
        below_policy.tasks[0].slash_proposal_digest = Some(*hash(&proposal_bytes).as_bytes());
        below_policy.tasks[0].slash_proposal_bytes = Some(proposal_bytes);
        below_policy.tasks[0].governance.decision = Some(RepairGovernanceDecision::Approved {
            decided_at_unix: approval.approved_at_unix,
            approvals: approval.approve_votes,
            rejections: approval.reject_votes,
        });
        write_private_file(
            &path,
            &norito::to_bytes(&below_policy).expect("encode below-policy checkpoint"),
        );
        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("unauthenticated embedded approval rejected");
        assert!(
            error
                .to_string()
                .contains("unauthenticated embedded approval")
        );

        let mut wrong_auditor = snapshot.clone();
        let mut proposal: RepairSlashProposalV1 = norito::decode_from_bytes(
            wrong_auditor.tasks[0]
                .slash_proposal_bytes
                .as_ref()
                .expect("slash proposal bytes"),
        )
        .expect("decode slash proposal");
        proposal.auditor_account = "different-auditor".to_owned();
        let proposal_bytes = norito::to_bytes(&proposal).expect("encode wrong-auditor proposal");
        wrong_auditor.tasks[0].slash_proposal_digest = Some(*hash(&proposal_bytes).as_bytes());
        wrong_auditor.tasks[0].slash_proposal_bytes = Some(proposal_bytes);
        write_private_file(
            &path,
            &norito::to_bytes(&wrong_auditor).expect("encode wrong-auditor checkpoint"),
        );
        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("proposal auditor mismatch rejected");
        assert!(error.to_string().contains("inconsistent slash proposal"));

        let mut forged = snapshot;
        forged.tasks[0].governance.decision = Some(RepairGovernanceDecision::Approved {
            decided_at_unix: escalated_at,
            approvals: 1,
            rejections: 0,
        });
        write_private_file(
            &path,
            &norito::to_bytes(&forged).expect("encode forged decision"),
        );
        let error = FileRepairStore::load_or_new(
            path,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("forged decision counts rejected");
        assert!(error.to_string().contains("invalid governance decision"));
    }

    #[test]
    fn checkpoint_rejects_compressed_or_oversized_norito_before_decode() {
        let dir = tempdir().expect("tempdir");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        let store = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("store");
        drop(store);
        let canonical = fs::read(&path).expect("canonical checkpoint");

        let mut compressed_bomb = canonical.clone();
        compressed_bomb[NORITO_COMPRESSION_OFFSET] = norito::Compression::Zstd as u8;
        compressed_bomb[NORITO_LENGTH_OFFSET..NORITO_LENGTH_OFFSET + 8]
            .copy_from_slice(&u64::MAX.to_le_bytes());
        write_private_file(&path, &compressed_bomb);
        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("compressed allocation bomb rejected before decode");
        assert!(error.to_string().contains("uncompressed canonical Norito"));

        let mut oversized_header = canonical.clone();
        oversized_header[NORITO_LENGTH_OFFSET..NORITO_LENGTH_OFFSET + 8]
            .copy_from_slice(&u64::MAX.to_le_bytes());
        write_private_file(&path, &oversized_header);
        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("oversized uncompressed header rejected before decode");
        assert!(error.to_string().contains("advertises"));
        assert!(error.to_string().contains("exceeding limit"));

        write_private_file(&path, &canonical);
        let mut actual = actual::SorafsRepair::default();
        actual.enabled = true;
        actual.state_dir = path.parent().map(Path::to_path_buf);
        let manager = RepairManager::new_with_config(RepairConfig::from(&actual));
        let report = report("REP-NESTED-COMPRESSED", [0x91; 32], [0xA1; 32], 500);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue nested proposal task");
        manager
            .submit_slash_proposal(RepairSlashProposalV1 {
                version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
                ticket_id: report.ticket_id.clone(),
                provider_id: report.evidence.provider_id,
                manifest_digest: report.evidence.manifest_digest,
                auditor_account: report.auditor_account.clone(),
                proposed_penalty: quantity_from_nanos(1),
                submitted_at_unix: 501,
                rationale: "nested proposal".to_owned(),
                approval: None,
            })
            .expect("persist nested proposal");
        drop(manager);

        let bytes = fs::read(&path).expect("checkpoint with proposal");
        let mut nested: RepairStoreSnapshot =
            norito::decode_from_bytes(&bytes).expect("decode checkpoint with proposal");
        let proposal_bytes = nested.tasks[0]
            .slash_proposal_bytes
            .as_mut()
            .expect("proposal bytes");
        proposal_bytes[NORITO_COMPRESSION_OFFSET] = norito::Compression::Zstd as u8;
        proposal_bytes[NORITO_LENGTH_OFFSET..NORITO_LENGTH_OFFSET + 8]
            .copy_from_slice(&u64::MAX.to_le_bytes());
        nested.tasks[0].slash_proposal_digest = Some(*hash(proposal_bytes).as_bytes());
        write_private_file(
            &path,
            &norito::to_bytes(&nested).expect("encode nested compressed proposal"),
        );
        let error = FileRepairStore::load_or_new(
            path,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("nested compressed allocation bomb rejected before decode");
        assert!(error.to_string().contains("uncompressed canonical Norito"));
    }

    #[test]
    fn checkpoint_rejects_unsorted_and_trailing_data() {
        let dir = tempdir().expect("tempdir");
        let path = canonical_temp_path(&dir)
            .join("repair")
            .join(REPAIR_STORE_FILE_NAME);
        let store = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("store");
        store
            .insert_task(task_internal(report(
                "REP-SORT-A",
                [0x01; 32],
                [0x11; 32],
                1_700_000_001,
            )))
            .expect("insert first task");
        store
            .insert_task(task_internal(report(
                "REP-SORT-B",
                [0x02; 32],
                [0x12; 32],
                1_700_000_002,
            )))
            .expect("insert second task");
        let canonical = fs::read(&path).expect("checkpoint bytes");
        let mut snapshot: RepairStoreSnapshot =
            norito::decode_from_bytes(&canonical).expect("decode checkpoint");
        drop(store);

        snapshot.tasks.reverse();
        write_private_file(
            &path,
            &norito::to_bytes(&snapshot).expect("encode unsorted checkpoint"),
        );
        let error = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("unsorted checkpoint rejected");
        assert!(error.to_string().contains("strictly sorted"));

        let truncated = &canonical[..canonical.len() - 1];
        write_private_file(&path, truncated);
        assert!(
            FileRepairStore::load_or_new(
                path.clone(),
                DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
                DEFAULT_REPAIR_STORE_MAX_BYTES,
            )
            .is_err(),
            "truncated checkpoint must fail closed"
        );

        let mut trailing = canonical;
        trailing.extend_from_slice(&[0, 1, 2, 3]);
        write_private_file(&path, &trailing);
        assert!(
            FileRepairStore::load_or_new(
                path,
                DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
                DEFAULT_REPAIR_STORE_MAX_BYTES,
            )
            .is_err(),
            "trailing or ambiguous checkpoint bytes must fail closed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn repair_store_rejects_hardlinks_traversal_and_writable_parent() {
        let dir = tempdir().expect("tempdir");
        let root = canonical_temp_path(&dir);
        let path = root.join("repair").join(REPAIR_STORE_FILE_NAME);
        let store = FileRepairStore::load_or_new(
            path.clone(),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect("store");
        store.next_audit_sequence().expect("create checkpoint");
        drop(store);

        let alias = root.join("checkpoint-alias.to");
        fs::hard_link(&path, &alias).expect("create hard link");
        let error = FileRepairStore::load_or_new(
            path,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("hard-linked checkpoint rejected");
        assert!(error.to_string().contains("hard link"));

        let traversal = root
            .join("missing")
            .join("..")
            .join("escaped")
            .join(REPAIR_STORE_FILE_NAME);
        let error = FileRepairStore::load_or_new(
            traversal,
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("parent traversal rejected");
        assert!(error.to_string().contains("must not contain"));

        let writable = root.join("writable");
        fs::create_dir(&writable).expect("create writable parent");
        fs::set_permissions(&writable, fs::Permissions::from_mode(0o777))
            .expect("set writable permissions");
        let error = FileRepairStore::load_or_new(
            writable.join(REPAIR_STORE_FILE_NAME),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("writable parent rejected");
        assert!(error.to_string().contains("writable"));

        let unsafe_ancestor = root.join("unsafe-ancestor");
        let private_child = unsafe_ancestor.join("private-child");
        fs::create_dir(&unsafe_ancestor).expect("create unsafe ancestor");
        fs::set_permissions(&unsafe_ancestor, fs::Permissions::from_mode(0o777))
            .expect("set unsafe ancestor permissions");
        fs::create_dir(&private_child).expect("create private child");
        fs::set_permissions(&private_child, fs::Permissions::from_mode(0o700))
            .expect("set private child permissions");
        let error = FileRepairStore::load_or_new(
            private_child.join(REPAIR_STORE_FILE_NAME),
            DEFAULT_REPAIR_STORE_ENTRY_LIMIT,
            DEFAULT_REPAIR_STORE_MAX_BYTES,
        )
        .expect_err("non-sticky writable ancestor rejected");
        assert!(error.to_string().contains("unsafe-ancestor"));
    }
}
