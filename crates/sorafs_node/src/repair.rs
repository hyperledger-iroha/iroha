//! Repair scheduler supporting SoraFS auditor workflows.
//!
//! The scheduler persists repair tickets via a repair store abstraction, tracks
//! proof-of-retrievability failures, and emits metrics so operators can monitor
//! SLA adherence. Repair state is stored on disk using Norito snapshots.

use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashMap, VecDeque},
    fs, io,
    path::{Path, PathBuf},
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

use blake3::hash;
use hex;
use iroha_logger::{debug, error, warn};
use iroha_telemetry::metrics::{global_or_default, global_sorafs_repair_otel};
use sorafs_manifest::{
    por::AuditVerdictV1,
    repair::{
        CompletedRepairStateV1, EscalatedRepairStateV1, FailedRepairStateV1,
        InProgressRepairStateV1, QueuedRepairStateV1, REPAIR_ESCALATION_APPROVAL_VERSION_V1,
        REPAIR_TASK_EVENT_VERSION_V1, REPAIR_TASK_VERSION_V1, RepairCauseV1,
        RepairEscalationApprovalV1, RepairEvidenceV1, RepairReportV1, RepairSlashProposalV1,
        RepairTaskEventV1, RepairTaskRecordV1, RepairTaskStateV1, RepairTaskStatusV1,
        RepairTicketId, RepairValidationError,
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
const REPAIR_STORE_VERSION_V1: u8 = 1;
const REPAIR_STORE_FILE_NAME: &str = "repair_state.to";
const REPAIR_STORE_TMP_EXT: &str = "tmp";
static REPAIR_STORE_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    /// Store rejected the update.
    #[error("repair store error: {0}")]
    Other(String),
}

/// Storage backend for repair tickets and PoR history.
trait RepairStore: std::fmt::Debug + Send + Sync {
    fn next_por_history_id(&self) -> u64;
    fn next_audit_sequence(&self) -> u64;
    fn record_por_history(&self, entry: PorHistoryEntry) -> Result<(), RepairStoreError>;
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
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct RepairStoreSnapshot {
    version: u8,
    next_por_history_id: u64,
    next_audit_sequence: u64,
    tasks: Vec<StoredRepairTask>,
    por_history: Vec<PorHistoryEntry>,
}

impl RepairStoreSnapshot {
    fn from_state(state: &RepairStoreState) -> Self {
        Self {
            version: REPAIR_STORE_VERSION_V1,
            next_por_history_id: state.next_por_history_id,
            next_audit_sequence: state.next_audit_sequence,
            tasks: state
                .tasks
                .values()
                .cloned()
                .map(StoredRepairTask::from_internal)
                .collect(),
            por_history: state.por_history.values().cloned().collect(),
        }
    }

    fn into_state(self) -> Result<RepairStoreState, RepairStoreError> {
        if self.version != REPAIR_STORE_VERSION_V1 {
            return Err(RepairStoreError::Other(format!(
                "unsupported repair store version {} (expected {})",
                self.version, REPAIR_STORE_VERSION_V1
            )));
        }
        let mut tasks = BTreeMap::new();
        for task in self.tasks {
            let internal = task.into_internal();
            let key = internal.report.ticket_id.0.clone();
            tasks.insert(key, internal);
        }
        let mut por_history = BTreeMap::new();
        for entry in self.por_history {
            por_history.insert(entry.id, entry);
        }
        Ok(RepairStoreState {
            tasks,
            por_history,
            next_por_history_id: self.next_por_history_id,
            next_audit_sequence: self.next_audit_sequence,
        })
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct StoredRepairTask {
    revision: u64,
    report: RepairReportV1,
    state: RepairTaskStateV1,
    sla_deadline_unix: Option<u64>,
    scheduler_notes: Option<String>,
    slash_proposal_digest: Option<[u8; 32]>,
    slash_proposal_bytes: Option<Vec<u8>>,
    #[norito(default)]
    governance: RepairGovernanceState,
    lease: Option<StoredRepairTaskLease>,
    attempts: u32,
    next_attempt_after_unix: Option<u64>,
    events: Vec<RepairTaskEventV1>,
}

impl StoredRepairTask {
    fn from_internal(task: RepairTaskInternal) -> Self {
        Self {
            revision: task.revision,
            report: task.report,
            state: task.state,
            sla_deadline_unix: task.sla_deadline_unix,
            scheduler_notes: task.scheduler_notes,
            slash_proposal_digest: task.slash_proposal_digest,
            slash_proposal_bytes: task.slash_proposal_bytes,
            governance: task.governance,
            lease: task.lease.map(StoredRepairTaskLease::from_lease),
            attempts: task.attempts,
            next_attempt_after_unix: task.next_attempt_after_unix,
            events: task.events,
        }
    }

    fn into_internal(self) -> RepairTaskInternal {
        RepairTaskInternal {
            revision: self.revision,
            report: self.report,
            state: self.state,
            sla_deadline_unix: self.sla_deadline_unix,
            scheduler_notes: self.scheduler_notes,
            slash_proposal_digest: self.slash_proposal_digest,
            slash_proposal_bytes: self.slash_proposal_bytes,
            governance: self.governance,
            lease: self.lease.map(StoredRepairTaskLease::into_lease),
            idempotency: RepairTaskIdempotency::new(DEFAULT_IDEMPOTENCY_CACHE_SIZE),
            attempts: self.attempts,
            next_attempt_after_unix: self.next_attempt_after_unix,
            events: self.events,
        }
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

#[derive(Debug)]
struct RepairStoreState {
    tasks: BTreeMap<String, RepairTaskInternal>,
    por_history: BTreeMap<u64, PorHistoryEntry>,
    next_por_history_id: u64,
    next_audit_sequence: u64,
}

impl RepairStoreState {
    fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
            por_history: BTreeMap::new(),
            next_por_history_id: 1,
            next_audit_sequence: 1,
        }
    }
}

#[derive(Debug)]
struct FileRepairStore {
    path: PathBuf,
    state: RwLock<RepairStoreState>,
}

impl FileRepairStore {
    fn load_or_new(path: PathBuf) -> Result<Self, RepairStoreError> {
        let state = if path.exists() {
            let bytes = fs::read(&path).map_err(|err| {
                RepairStoreError::Other(format!("failed to read repair store: {err}"))
            })?;
            let snapshot: RepairStoreSnapshot =
                norito::decode_from_bytes(&bytes).map_err(|err| {
                    RepairStoreError::Other(format!("failed to decode repair store: {err}"))
                })?;
            snapshot.into_state()?
        } else {
            RepairStoreState::new()
        };
        Ok(Self {
            path,
            state: RwLock::new(state),
        })
    }

    fn persist(&self, state: &RepairStoreState) -> Result<(), RepairStoreError> {
        let snapshot = RepairStoreSnapshot::from_state(state);
        let bytes = norito::to_bytes(&snapshot).map_err(|err| {
            RepairStoreError::Other(format!("failed to encode repair store: {err}"))
        })?;
        write_atomic(&self.path, &bytes).map_err(|err| {
            RepairStoreError::Other(format!("failed to persist repair store: {err}"))
        })
    }
}

fn write_atomic(path: &Path, data: &[u8]) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let counter = REPAIR_STORE_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_path = temp_path_for_atomic(path, std::process::id(), counter);
    fs::write(&tmp_path, data)?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

fn archive_corrupt_store(path: &Path) -> Result<(), io::Error> {
    if !path.exists() {
        return Ok(());
    }
    let counter = REPAIR_STORE_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let suffix = format!("corrupt-{pid}-{counter}");
    let archive_path = path.with_added_extension(&suffix);
    fs::rename(path, archive_path)?;
    Ok(())
}

fn temp_path_for_atomic(path: &Path, pid: u32, counter: u64) -> PathBuf {
    let suffix = format!("{REPAIR_STORE_TMP_EXT}-{pid}-{counter}");
    let candidate = path.with_added_extension(&suffix);
    match candidate.file_name().and_then(|name| name.to_str()) {
        Some(name) => candidate.with_file_name(format!(".{name}")),
        None => candidate,
    }
}

impl RepairStore for FileRepairStore {
    fn next_por_history_id(&self) -> u64 {
        let mut guard = self.state.write().expect("repair store poisoned");
        let id = guard.next_por_history_id;
        guard.next_por_history_id = guard.next_por_history_id.saturating_add(1);
        if let Err(err) = self.persist(&guard) {
            warn!(?err, "failed to persist repair store after por history id");
        }
        id
    }

    fn next_audit_sequence(&self) -> u64 {
        let mut guard = self.state.write().expect("repair store poisoned");
        let sequence = guard.next_audit_sequence;
        guard.next_audit_sequence = guard.next_audit_sequence.saturating_add(1);
        if let Err(err) = self.persist(&guard) {
            warn!(?err, "failed to persist repair store after audit sequence");
        }
        sequence
    }

    fn record_por_history(&self, entry: PorHistoryEntry) -> Result<(), RepairStoreError> {
        let mut guard = self.state.write().expect("repair store poisoned");
        if guard.por_history.contains_key(&entry.id) {
            return Err(RepairStoreError::Other(format!(
                "por history id {} already stored",
                entry.id
            )));
        }
        guard.por_history.insert(entry.id, entry);
        self.persist(&guard)
    }

    fn por_history_entry(
        &self,
        por_history_id: u64,
    ) -> Result<Option<PorHistoryEntry>, RepairStoreError> {
        let guard = self.state.read().expect("repair store poisoned");
        Ok(guard.por_history.get(&por_history_id).cloned())
    }

    fn insert_task(
        &self,
        task: RepairTaskInternal,
    ) -> Result<RepairStoreInsertResult, RepairStoreError> {
        let mut guard = self.state.write().expect("repair store poisoned");
        let key = task.report.ticket_id.0.clone();
        if let Some(existing) = guard.tasks.get(&key) {
            return Ok(RepairStoreInsertResult::Existing(existing.clone()));
        }
        guard.tasks.insert(key, task.clone());
        self.persist(&guard)?;
        Ok(RepairStoreInsertResult::Inserted(task))
    }

    fn task(
        &self,
        ticket_id: &RepairTicketId,
    ) -> Result<Option<RepairTaskInternal>, RepairStoreError> {
        let guard = self.state.read().expect("repair store poisoned");
        Ok(guard.tasks.get(&ticket_id.0).cloned())
    }

    fn compare_and_set_task(
        &self,
        ticket_id: &RepairTicketId,
        expected_revision: u64,
        task: RepairTaskInternal,
    ) -> Result<(), RepairStoreError> {
        let mut guard = self.state.write().expect("repair store poisoned");
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
        guard.tasks.insert(ticket_id.0.clone(), task);
        self.persist(&guard)
    }

    fn list_tasks(&self) -> Result<Vec<RepairTaskInternal>, RepairStoreError> {
        let guard = self.state.read().expect("repair store poisoned");
        Ok(guard.tasks.values().cloned().collect())
    }
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

/// Snapshot of a repair task with its event history.
#[derive(Debug, Clone)]
pub struct RepairTaskSnapshot {
    /// Current task record.
    pub record: RepairTaskRecordV1,
    /// Event log ordered by occurrence.
    pub events: Vec<RepairTaskEventV1>,
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

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
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

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
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

#[derive(Debug, Clone, Default, NoritoSerialize, NoritoDeserialize)]
struct RepairGovernanceState {
    #[norito(default)]
    approvals: Vec<RepairGovernanceVote>,
    #[norito(default)]
    rejections: Vec<RepairGovernanceVote>,
    #[norito(default)]
    decision: Option<RepairGovernanceDecision>,
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

fn build_repair_store(config: &RepairConfig) -> Arc<dyn RepairStore> {
    let state_dir = match config.state_dir() {
        Some(dir) => dir.clone(),
        None => {
            let fallback = default_repair_state_dir();
            warn!(
                path = ?fallback,
                "repair store state_dir not configured; using temporary directory"
            );
            fallback
        }
    };
    let path = state_dir.join(REPAIR_STORE_FILE_NAME);
    match FileRepairStore::load_or_new(path.clone()) {
        Ok(store) => Arc::new(store),
        Err(err) => {
            error!(?err, path = ?path, "failed to load repair store");
            if let Err(archive_err) = archive_corrupt_store(&path) {
                warn!(
                    ?archive_err,
                    path = ?path,
                    "failed to archive corrupt repair store"
                );
            }
            match FileRepairStore::load_or_new(path) {
                Ok(store) => Arc::new(store),
                Err(err) => {
                    error!(?err, "failed to reinitialise repair store after archive");
                    let fallback = default_repair_state_dir().join(REPAIR_STORE_FILE_NAME);
                    Arc::new(
                        FileRepairStore::load_or_new(fallback)
                            .expect("repair store fallback should initialise"),
                    )
                }
            }
        }
    }
}

fn default_repair_state_dir() -> PathBuf {
    let pid = std::process::id();
    let counter = REPAIR_STORE_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut dir = std::env::temp_dir();
    dir.push("iroha-sorafs-repair");
    dir.push(format!("pid-{pid}-{counter}"));
    dir
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
        let store = build_repair_store(&config);
        let escalation_policy = *config.escalation_policy();
        Self {
            store,
            default_sla_secs: DEFAULT_REPAIR_SLA_SECS,
            event_history_limit: DEFAULT_REPAIR_EVENT_HISTORY_LIMIT,
            config,
            escalation_policy,
        }
    }

    /// Construct a new repair manager with an explicit governance escalation policy.
    #[must_use]
    pub fn new_with_config_and_policy(
        config: RepairConfig,
        policy: RepairEscalationPolicy,
    ) -> Self {
        let config = config.with_escalation_policy(policy);
        Self::new_with_config(config)
    }

    /// Reserve the next audit sequence number for governance events.
    #[must_use]
    pub fn next_audit_sequence(&self) -> u64 {
        self.store.next_audit_sequence()
    }

    /// Register a PoR verdict; returns a history identifier when the verdict recorded failures.
    pub fn register_por_verdict(
        &self,
        verdict: &AuditVerdictV1,
        failed_samples: u64,
    ) -> Option<u64> {
        if failed_samples == 0 {
            return None;
        }
        let history_id = self.store.next_por_history_id();
        let entry = PorHistoryEntry {
            id: history_id,
            manifest_digest: verdict.manifest_digest,
            provider_id: verdict.provider_id,
            challenge_id: verdict.challenge_id,
            decided_at: verdict.decided_at,
            failed_samples,
        };
        debug!(
            manifest = %hex::encode(verdict.manifest_digest),
            provider = %hex::encode(verdict.provider_id),
            challenge = %hex::encode(verdict.challenge_id),
            failed_samples,
            history_id = entry.id,
            "registered PoR failure for repair history"
        );
        if let Err(err) = self.store.record_por_history(entry) {
            error!(?err, history_id, "failed to persist PoR repair history");
            // TODO: propagate store errors once PoR ingestion is permitted to fail hard.
            return None;
        }
        Some(history_id)
    }

    /// Enqueue a repair report submitted by an auditor.
    pub fn enqueue_report(
        &self,
        report: RepairReportV1,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        Ok(self.enqueue_report_with_event(report)?.record)
    }

    /// Enqueue a repair report submitted by an auditor, returning any emitted event.
    pub fn enqueue_report_with_event(
        &self,
        report: RepairReportV1,
    ) -> Result<RepairTaskUpdate, RepairSchedulerError> {
        report
            .validate()
            .map_err(RepairSchedulerError::InvalidReport)?;
        if let Some(por_id) = report.evidence.por_history_id {
            self.ensure_por_history_match(por_id, &report.evidence)?;
        }

        let report_evidence = report.evidence.clone();
        let ticket_id = report.ticket_id.to_string();
        let sla_deadline = report.submitted_at_unix.checked_add(self.default_sla_secs);
        let state = RepairTaskStateV1::Queued(QueuedRepairStateV1 {
            queued_at_unix: report.submitted_at_unix,
            sla_deadline_unix: sla_deadline,
        });
        let mut internal = RepairTaskInternal {
            report,
            state,
            sla_deadline_unix: sla_deadline,
            scheduler_notes: None,
            slash_proposal_digest: None,
            slash_proposal_bytes: None,
            governance: RepairGovernanceState::default(),
            lease: None,
            idempotency: RepairTaskIdempotency::new(DEFAULT_IDEMPOTENCY_CACHE_SIZE),
            attempts: 0,
            next_attempt_after_unix: None,
            revision: 0,
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
                if existing.report.evidence != report_evidence {
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
        let approval = proposal.approval.as_ref();
        if proposal.proposed_penalty_nano > self.escalation_policy.max_penalty_nano() {
            return Err(policy_violation(
                &proposal.ticket_id,
                "proposed penalty exceeds policy cap",
            ));
        }
        let mut event = None;
        let task = self.update_task_with_retry(&proposal.ticket_id, |task| {
            event = None;
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
            }
            if let Some(approval) = approval {
                self.ensure_escalation_policy(&proposal.ticket_id, approval, escalated_at_unix)?;
                task.governance.decision = Some(RepairGovernanceDecision::Approved {
                    decided_at_unix: approval.approved_at_unix,
                    approvals: approval.approve_votes,
                    rejections: approval.reject_votes,
                });
            }

            let mut digest = [0u8; 32];
            let mut buf = Vec::new();
            norito::core::NoritoSerialize::serialize(&proposal, &mut buf)
                .expect("serialize slash proposal");
            digest.copy_from_slice(hash(&buf).as_bytes());
            if let Some(existing_digest) = task.slash_proposal_digest {
                if existing_digest != digest {
                    return Err(policy_violation(
                        &proposal.ticket_id,
                        "conflicting slash proposal already recorded",
                    ));
                }
            }

            let reason = existing_reason.unwrap_or_else(|| proposal.rationale.clone());
            task.state = RepairTaskStateV1::Escalated(EscalatedRepairStateV1 {
                queued_at_unix: queued_at,
                escalated_at_unix,
                reason,
            });
            task.slash_proposal_digest = Some(digest);
            task.slash_proposal_bytes = Some(buf);
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
        global_sorafs_repair_otel().record_slash_proposal("submitted");
        global_or_default().inc_sorafs_slash_proposals("submitted");
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
            let appeal_deadline =
                appeal_deadline_unix(approved_at_unix, &self.escalation_policy, ticket_id)?;
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
            let dispute_deadline =
                dispute_deadline_unix(escalated_at_unix, &self.escalation_policy, ticket_id)?;
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

    fn ensure_escalation_policy(
        &self,
        ticket_id: &RepairTicketId,
        approval: &RepairEscalationApprovalV1,
        escalated_at_unix: u64,
    ) -> Result<(), RepairSchedulerError> {
        let policy = self.escalation_policy;
        let total_votes = u64::from(approval.approve_votes)
            + u64::from(approval.reject_votes)
            + u64::from(approval.abstain_votes);
        if total_votes < u64::from(policy.minimum_voters()) {
            return Err(policy_violation(
                ticket_id,
                "approval vote count below minimum voters",
            ));
        }
        let counted_votes = u64::from(approval.approve_votes) + u64::from(approval.reject_votes);
        if counted_votes == 0 {
            return Err(policy_violation(
                ticket_id,
                "approval must include at least one approve/reject vote",
            ));
        }
        let approval_ratio_bps = div_ceil_u128(
            u128::from(approval.approve_votes).saturating_mul(10_000),
            u128::from(counted_votes),
        );
        if approval_ratio_bps < u128::from(policy.quorum_bps()) {
            return Err(policy_violation(
                ticket_id,
                "approval ratio below quorum threshold",
            ));
        }
        if approval.approve_votes <= approval.reject_votes {
            return Err(policy_violation(
                ticket_id,
                "approval votes must exceed reject votes",
            ));
        }
        let dispute_deadline = escalated_at_unix.saturating_add(policy.dispute_window_secs());
        if approval.approved_at_unix < dispute_deadline {
            return Err(policy_violation(
                ticket_id,
                "approval recorded before dispute window elapsed",
            ));
        }
        let appeal_deadline = approval
            .approved_at_unix
            .saturating_add(policy.appeal_window_secs());
        if approval.finalized_at_unix < appeal_deadline {
            return Err(policy_violation(
                ticket_id,
                "approval finalized before appeal window elapsed",
            ));
        }
        Ok(())
    }

    /// Fetch all repair tasks associated with `manifest_digest`.
    #[must_use]
    pub fn tasks_for_manifest(&self, manifest_digest: &[u8; 32]) -> Vec<RepairTaskRecordV1> {
        self.list_tasks(RepairTaskFilters::for_manifest(*manifest_digest))
    }

    /// List repair tasks with optional filters applied.
    #[must_use]
    pub fn list_tasks(&self, filters: RepairTaskFilters) -> Vec<RepairTaskRecordV1> {
        let tasks = match self.store.list_tasks() {
            Ok(tasks) => tasks,
            Err(err) => {
                error!(?err, "failed to load repair tasks");
                return Vec::new();
            }
        };
        let mut records: Vec<RepairTaskRecordV1> = tasks
            .into_iter()
            .map(|task| task.to_record())
            .filter(|record| filters.matches(record))
            .collect();
        sort_repair_task_records(&mut records);
        records
    }

    /// List repair task snapshots with optional filters applied.
    #[must_use]
    pub fn list_task_snapshots(&self, filters: RepairTaskFilters) -> Vec<RepairTaskSnapshot> {
        let tasks = match self.store.list_tasks() {
            Ok(tasks) => tasks,
            Err(err) => {
                error!(?err, "failed to load repair tasks");
                return Vec::new();
            }
        };
        let mut snapshots: Vec<RepairTaskSnapshot> = tasks
            .into_iter()
            .map(|task| task.to_snapshot())
            .filter(|snapshot| filters.matches(&snapshot.record))
            .collect();
        sort_repair_task_snapshots(&mut snapshots);
        snapshots
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
    #[must_use]
    pub fn claimable_tasks(&self, now_unix: u64) -> Vec<RepairTaskRecordV1> {
        let tasks = match self.store.list_tasks() {
            Ok(tasks) => tasks,
            Err(err) => {
                error!(?err, "failed to load repair tasks");
                return Vec::new();
            }
        };
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

        candidates
            .into_iter()
            .map(|task| task.to_record())
            .collect()
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
                    event = None;
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
        let dispute_deadline = dispute_deadline_unix(
            escalated.escalated_at_unix,
            &self.escalation_policy,
            &task.report.ticket_id,
        )?;
        if now_unix < dispute_deadline {
            return Ok(None);
        }
        let approvals = u32::try_from(task.governance.approvals.len()).unwrap_or(u32::MAX);
        let rejections = u32::try_from(task.governance.rejections.len()).unwrap_or(u32::MAX);
        let total = approvals.saturating_add(rejections);
        let decision = if total < self.escalation_policy.minimum_voters() {
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
            if ratio_bps < u128::from(self.escalation_policy.quorum_bps()) {
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
    #[must_use]
    pub fn task_record(&self, ticket_id: &RepairTicketId) -> Option<RepairTaskRecordV1> {
        match self.store.task(ticket_id) {
            Ok(Some(task)) => Some(task.to_record()),
            Ok(None) => None,
            Err(err) => {
                error!(?err, ticket_id = %ticket_id, "failed to load repair ticket");
                None
            }
        }
    }

    /// Retrieve a repair task snapshot by ticket id.
    #[must_use]
    pub fn task_snapshot(&self, ticket_id: &RepairTicketId) -> Option<RepairTaskSnapshot> {
        match self.store.task(ticket_id) {
            Ok(Some(task)) => Some(task.to_snapshot()),
            Ok(None) => None,
            Err(err) => {
                error!(?err, ticket_id = %ticket_id, "failed to load repair ticket");
                None
            }
        }
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
            task.revision = task.revision.saturating_add(1);
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
            task.revision = task.revision.saturating_add(1);
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
            task.revision = task.revision.saturating_add(1);
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
                    self.apply_escalation(&mut task, failed_at_unix, rationale, "scheduler")?;
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
            task.revision = task.revision.saturating_add(1);
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
            update(&mut task)?;
            let expected_revision = task.revision;
            task.revision = task.revision.saturating_add(1);
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
        actor: &'static str,
    ) -> Result<EscalationOutcome, RepairSchedulerError> {
        ensure_transition_allowed(&task.state, "escalated", &task.report.ticket_id)?;
        let queued_at = queued_at_unix(&task.state);
        if escalated_at_unix <= queued_at {
            return Err(RepairSchedulerError::InvalidTimestamp {
                ticket_id: task.report.ticket_id.to_string(),
            });
        }

        let penalty_nano = self
            .escalation_policy
            .cap_penalty(self.config.default_slash_penalty_nano());
        let proposal = RepairSlashProposalV1 {
            version: sorafs_manifest::repair::REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: task.report.ticket_id.clone(),
            provider_id: task.report.evidence.provider_id,
            manifest_digest: task.report.evidence.manifest_digest,
            auditor_account: task.report.auditor_account.clone(),
            proposed_penalty_nano: penalty_nano,
            submitted_at_unix: escalated_at_unix,
            rationale: rationale.clone(),
            approval: None,
        };
        proposal
            .validate()
            .map_err(RepairSchedulerError::InvalidSlashProposal)?;

        let mut bytes = Vec::new();
        norito::core::NoritoSerialize::serialize(&proposal, &mut bytes)
            .expect("serialize slash proposal");
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
        task.lease = None;
        task.next_attempt_after_unix = None;
        task.governance = RepairGovernanceState::default();
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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
}

#[derive(Debug, Clone)]
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
}

#[derive(Debug, Clone)]
struct IdempotencyEntry<S> {
    signature: S,
    record: RepairTaskRecordV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepairClaimSignature {
    worker_id: String,
    claimed_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepairHeartbeatSignature {
    worker_id: String,
    heartbeat_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepairCompleteSignature {
    worker_id: String,
    completed_at_unix: u64,
    resolution_notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepairFailSignature {
    worker_id: String,
    failed_at_unix: u64,
    reason: String,
}

#[derive(Debug, Clone)]
struct RepairTaskInternal {
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
    governance: RepairGovernanceState,
    lease: Option<RepairTaskLease>,
    idempotency: RepairTaskIdempotency,
    attempts: u32,
    next_attempt_after_unix: Option<u64>,
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

    fn to_snapshot(&self) -> RepairTaskSnapshot {
        RepairTaskSnapshot {
            record: self.to_record(),
            events: self.events.clone(),
        }
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
        }
        Some(event)
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct PorHistoryEntry {
    id: u64,
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    challenge_id: [u8; 32],
    decided_at: u64,
    failed_samples: u64,
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
        RepairCauseV1::PorFailure { failed_samples, .. } => (3, u64::from(*failed_samples)),
        RepairCauseV1::ReplicaShortfall { missing_chunks } => (2, u64::from(*missing_chunks)),
        RepairCauseV1::LatencySla {
            observed_latency_ms,
            ..
        } => (1, u64::from(*observed_latency_ms)),
        RepairCauseV1::Manual { .. } => (0, 0),
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

fn div_ceil_u128(numerator: u128, denominator: u128) -> u128 {
    if denominator == 0 {
        return 0;
    }
    let adjusted = numerator.saturating_add(denominator.saturating_sub(1));
    adjusted / denominator
}

fn dispute_deadline_unix(
    escalated_at_unix: u64,
    policy: &RepairEscalationPolicy,
    ticket_id: &RepairTicketId,
) -> Result<u64, RepairSchedulerError> {
    checked_add_secs(escalated_at_unix, policy.dispute_window_secs(), ticket_id)
}

fn appeal_deadline_unix(
    approved_at_unix: u64,
    policy: &RepairEscalationPolicy,
    ticket_id: &RepairTicketId,
) -> Result<u64, RepairSchedulerError> {
    checked_add_secs(approved_at_unix, policy.appeal_window_secs(), ticket_id)
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

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_config::parameters::actual;
    use sorafs_manifest::por::{AUDIT_VERDICT_VERSION_V1, AuditOutcomeV1, AuditVerdictV1};
    use sorafs_manifest::repair::{
        REPAIR_EVIDENCE_VERSION_V1, REPAIR_REPORT_VERSION_V1, REPAIR_SLASH_PROPOSAL_VERSION_V1,
        RepairCauseV1,
    };
    use std::fs;
    use tempfile::{TempDir, tempdir};

    fn report(
        ticket: &str,
        manifest_digest: [u8; 32],
        provider_id: [u8; 32],
        submitted_at_unix: u64,
    ) -> RepairReportV1 {
        RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId(ticket.to_string()),
            auditor_account: "auditor#sora".into(),
            submitted_at_unix,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest,
                provider_id,
                por_history_id: None,
                cause: RepairCauseV1::Manual {
                    reason: "test".into(),
                },
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
        RepairTaskInternal {
            revision: 0,
            report,
            state,
            sla_deadline_unix: sla_deadline,
            scheduler_notes: None,
            slash_proposal_digest: None,
            slash_proposal_bytes: None,
            governance: RepairGovernanceState::default(),
            lease: None,
            idempotency: RepairTaskIdempotency::new(DEFAULT_IDEMPOTENCY_CACHE_SIZE),
            attempts: 0,
            next_attempt_after_unix: None,
            events: Vec::new(),
        }
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

    fn manager_with_config(config: RepairConfig) -> (RepairManager, TempDir) {
        let temp_dir = tempdir().expect("tempdir");
        let config = config.with_default_state_dir(temp_dir.path());
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
        manager.register_por_verdict(&verdict, 1);

        let store_path = temp_dir.path().join("repair").join(REPAIR_STORE_FILE_NAME);
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

        let tasks = manager.list_tasks(RepairTaskFilters::default());
        let ids: Vec<_> = tasks.iter().map(|task| task.ticket_id.0.as_str()).collect();
        assert_eq!(ids, vec!["REP-050", "REP-100", "REP-200"]);
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

        let provider_tasks = manager.list_tasks(RepairTaskFilters {
            provider_id: Some(provider_a),
            ..RepairTaskFilters::default()
        });
        assert_eq!(provider_tasks.len(), 1);
        assert_eq!(provider_tasks[0].ticket_id, report_a.ticket_id);

        let status_tasks = manager.list_tasks(RepairTaskFilters {
            status: Some(RepairTaskStatusV1::Completed),
            ..RepairTaskFilters::default()
        });
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

        let record = manager.task_record(&report.ticket_id).expect("task record");
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
            default_slash_penalty_nano: 12_000,
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
            max_penalty_nano: 500,
            ..Default::default()
        };
        let actual = actual::SorafsRepair {
            max_attempts: 1,
            default_slash_penalty_nano: 12_000,
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
        assert_eq!(proposal.proposed_penalty_nano, 500);
    }

    #[test]
    fn submit_slash_proposal_rejects_manifest_mismatch() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-324", [0x21; 32], [0x31; 32], 1_700_100_350);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        let policy = *RepairConfig::default().escalation_policy();
        let approval = approval_for_policy(&policy, report.submitted_at_unix + 10);
        let expected_approved_at = approval.approved_at_unix;
        let expected_approvals = approval.approve_votes;
        let expected_rejections = approval.reject_votes;

        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: report.ticket_id.clone(),
            provider_id: report.evidence.provider_id,
            manifest_digest: [0xFF; 32],
            auditor_account: report.auditor_account.clone(),
            proposed_penalty_nano: 1_000,
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
            proposed_penalty_nano: 1_000,
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
    fn submit_slash_proposal_accepts_valid_approval() {
        let (manager, _temp_dir) = manager_with_temp_dir();
        let report = report("REP-326", [0x21; 32], [0x31; 32], 1_700_100_370);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");
        let policy = *RepairConfig::default().escalation_policy();
        let approval = approval_for_policy(&policy, report.submitted_at_unix + 10);

        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: report.ticket_id.clone(),
            provider_id: report.evidence.provider_id,
            manifest_digest: report.evidence.manifest_digest,
            auditor_account: report.auditor_account.clone(),
            proposed_penalty_nano: 1_000,
            submitted_at_unix: report.submitted_at_unix + 10,
            rationale: "valid approval".into(),
            approval: Some(approval),
        };

        let update = manager
            .submit_slash_proposal_with_event(proposal)
            .expect("valid approval should be accepted");
        assert!(matches!(
            update.record.state,
            RepairTaskStateV1::Escalated(..)
        ));
        let task = manager
            .load_task(&report.ticket_id)
            .expect("load repair task");
        match task.governance.decision {
            Some(RepairGovernanceDecision::Approved {
                decided_at_unix,
                approvals,
                rejections,
            }) => {
                assert_eq!(decided_at_unix, expected_approved_at);
                assert_eq!(approvals, expected_approvals);
                assert_eq!(rejections, expected_rejections);
            }
            other => panic!("expected approval decision, got {other:?}"),
        }
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
            proposed_penalty_nano: 1_000,
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
            proposed_penalty_nano: 2_000,
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
    fn governance_votes_finalize_after_dispute_window() {
        let policy = actual::RepairEscalationPolicyV1 {
            quorum_bps: 6_000,
            minimum_voters: 3,
            dispute_window_secs: 10,
            appeal_window_secs: 120,
            max_penalty_nano: 1_000_000_000,
        };
        let actual = actual::SorafsRepair {
            max_attempts: 1,
            default_slash_penalty_nano: 5_000,
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
            max_penalty_nano: 1_000_000_000,
        };
        let actual = actual::SorafsRepair {
            max_attempts: 1,
            default_slash_penalty_nano: 5_000,
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
            max_penalty_nano: 1_000_000_000,
        };
        let actual = actual::SorafsRepair {
            max_attempts: 1,
            default_slash_penalty_nano: 5_000,
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
        let policy = *RepairConfig::default().escalation_policy();
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
            proposed_penalty_nano: 1_000,
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
        let policy = *config.escalation_policy();
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
            proposed_penalty_nano: 1_000,
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
        let policy = *RepairConfig::default().escalation_policy();
        let mut approval = approval_for_policy(&policy, report.submitted_at_unix + 10);
        approval.finalized_at_unix = approval.approved_at_unix;

        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: report.ticket_id.clone(),
            provider_id: report.evidence.provider_id,
            manifest_digest: report.evidence.manifest_digest,
            auditor_account: report.auditor_account.clone(),
            proposed_penalty_nano: 1_000,
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
                "worker-a",
                report.submitted_at_unix + 10,
                "key-1",
            )
            .expect("claim ticket");
        match record.state {
            RepairTaskStateV1::InProgress(ref state) => {
                assert_eq!(state.started_at_unix, report.submitted_at_unix + 10);
                assert_eq!(state.repair_agent.as_deref(), Some("worker-a"));
            }
            other => panic!("unexpected state {other:?}"),
        }

        let replay = manager
            .claim_ticket(
                &report.ticket_id,
                "worker-a",
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
                "worker-a",
                report.submitted_at_unix + 10,
                "key-1",
            )
            .expect("claim ticket");
        assert!(update.event.is_some());

        let replay = manager
            .claim_ticket_with_event(
                &report.ticket_id,
                "worker-a",
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
                "worker-b",
                report.submitted_at_unix + 5,
                "claim-1",
            )
            .expect("claim ticket");

        manager
            .heartbeat_ticket(
                &report.ticket_id,
                "worker-b",
                report.submitted_at_unix + 15,
                "hb-1",
            )
            .expect("heartbeat ticket");
        let stale = manager.heartbeat_ticket(
            &report.ticket_id,
            "worker-b",
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
            default_slash_penalty_nano: 12_345,
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
            default_slash_penalty_nano: 12_345,
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

        let snapshot = manager.task_snapshot(&report.ticket_id).expect("snapshot");
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
            default_slash_penalty_nano: 98_765,
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
            outcome.escalated[0].proposed_penalty_nano,
            actual.default_slash_penalty_nano
        );
        assert_eq!(outcome.events.len(), 1);
        assert_eq!(outcome.events[0].status, RepairTaskStatusV1::Escalated);

        let record = manager.task_record(&report.ticket_id).expect("record");
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
        let failed = manager.task_record(&report.ticket_id).expect("record");
        assert!(matches!(failed.state, RepairTaskStateV1::Failed(..)));

        let after_backoff = report.submitted_at_unix + 12 + actual.backoff_initial_secs + 1;
        let outcome = manager.run_watchdog(after_backoff).expect("watchdog");
        assert_eq!(outcome.requeued, vec![report.ticket_id.clone()]);
        assert_eq!(outcome.events.len(), 1);
        assert_eq!(outcome.events[0].status, RepairTaskStatusV1::Queued);
        let queued = manager.task_record(&report.ticket_id).expect("record");
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
        severe.evidence.cause = RepairCauseV1::PorFailure {
            challenge_id: [0xAA; 32],
            failed_samples: 5,
            proof_digest: None,
        };
        let later_a = report("REP-502", [0x22; 32], provider_a, 2_000);
        let later_b = report("REP-503", [0x23; 32], provider_a, 2_000);

        manager.enqueue_report(early).expect("enqueue early");
        manager.enqueue_report(severe).expect("enqueue severe");
        manager.enqueue_report(later_a).expect("enqueue later a");
        manager.enqueue_report(later_b).expect("enqueue later b");

        let ordered = manager.claimable_tasks(2_500);
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

        let snapshot = manager.task_snapshot(&report.ticket_id).expect("snapshot");
        let statuses: Vec<_> = snapshot.events.iter().map(|event| event.status).collect();
        assert_eq!(
            statuses,
            vec![
                RepairTaskStatusV1::Queued,
                RepairTaskStatusV1::InProgress,
                RepairTaskStatusV1::Completed
            ]
        );
        assert_eq!(snapshot.events[0].actor.as_deref(), Some("auditor#sora"));
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
    fn file_store_compare_and_set_updates_task() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("repair").join(REPAIR_STORE_FILE_NAME);
        let store = FileRepairStore::load_or_new(path).expect("store");
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
        updated.scheduler_notes = Some("updated".into());
        updated.revision = updated.revision.saturating_add(1);
        store
            .compare_and_set_task(&report.ticket_id, 0, updated.clone())
            .expect("compare and set");

        let fetched = store
            .task(&report.ticket_id)
            .expect("load task")
            .expect("task present");
        assert_eq!(fetched.scheduler_notes.as_deref(), Some("updated"));
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
        let path = dir.path().join("repair").join(REPAIR_STORE_FILE_NAME);
        let store = FileRepairStore::load_or_new(path).expect("store");
        let first = store.next_audit_sequence();
        let second = store.next_audit_sequence();
        assert_eq!(first, 1);
        assert_eq!(second, 2);
    }

    #[test]
    fn file_store_persists_tasks_and_history() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("repair").join(REPAIR_STORE_FILE_NAME);
        let store = FileRepairStore::load_or_new(path.clone()).expect("store");

        let report = report("REP-950", [0x44; 32], [0x55; 32], 1_700_111_000);
        let task = task_internal(report.clone());
        store.insert_task(task).expect("insert task");

        let history_id = store.next_por_history_id();
        let entry = PorHistoryEntry {
            id: history_id,
            manifest_digest: report.evidence.manifest_digest,
            provider_id: report.evidence.provider_id,
            challenge_id: [0x99; 32],
            decided_at: report.submitted_at_unix + 60,
            failed_samples: 4,
        };
        store
            .record_por_history(entry.clone())
            .expect("record history");

        let sequence = store.next_audit_sequence();
        assert_eq!(sequence, 1);
        drop(store);

        let reloaded = FileRepairStore::load_or_new(path).expect("reload store");
        let loaded_task = reloaded
            .task(&report.ticket_id)
            .expect("load task")
            .expect("task present");
        assert_eq!(loaded_task.report.ticket_id, report.ticket_id);
        let loaded_history = reloaded
            .por_history_entry(history_id)
            .expect("history lookup")
            .expect("history entry present");
        assert_eq!(loaded_history.manifest_digest, entry.manifest_digest);
        assert_eq!(loaded_history.failed_samples, entry.failed_samples);
        let next_sequence = reloaded.next_audit_sequence();
        assert_eq!(next_sequence, 2);
    }

    #[test]
    fn archive_corrupt_store_moves_file_aside() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("repair").join(REPAIR_STORE_FILE_NAME);
        fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
        fs::write(&path, b"corrupt").expect("write corrupt store");

        archive_corrupt_store(&path).expect("archive");
        assert!(!path.exists());

        let entries = fs::read_dir(path.parent().expect("parent"))
            .expect("read dir")
            .map(|entry| entry.expect("entry").path())
            .collect::<Vec<_>>();
        assert!(
            entries.iter().any(|entry| {
                entry
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains("corrupt-"))
            }),
            "expected archived repair store"
        );
    }
}
