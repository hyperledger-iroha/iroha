//! Repair scheduler supporting SoraFS auditor workflows.
//!
//! The scheduler maintains an in-memory queue of repair tickets reported by
//! auditors, tracks proof-of-retrievability failures, and emits metrics so
//! operators can monitor SLA adherence. It is intentionally lightweight: the
//! canonical durability layer (Postgres) will be introduced once Torii wiring
//! and governance integration mature.

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

use blake3::hash;
use hex;
use iroha_logger::debug;
use iroha_telemetry::metrics::{global_or_default, global_sorafs_repair_otel};
use sorafs_manifest::{
    por::AuditVerdictV1,
    repair::{
        CompletedRepairStateV1, EscalatedRepairStateV1, FailedRepairStateV1,
        InProgressRepairStateV1, QueuedRepairStateV1, REPAIR_TASK_VERSION_V1, RepairEvidenceV1,
        RepairReportV1, RepairSlashProposalV1, RepairTaskRecordV1, RepairTaskStateV1,
        RepairTaskStatusV1, RepairTicketId, RepairValidationError,
    },
};
use thiserror::Error;

use crate::config::RepairConfig;

const DEFAULT_REPAIR_SLA_SECS: u64 = 4 * 60 * 60;
const MAX_IDEMPOTENCY_KEY_BYTES: usize = 256;
const MAX_WORKER_ID_BYTES: usize = 256;
const MAX_REPAIR_NOTES_BYTES: usize = 256;
const DEFAULT_IDEMPOTENCY_CACHE_SIZE: usize = 64;

/// Manages repair tickets and PoR failure history.
#[derive(Debug, Clone)]
pub struct RepairManager {
    tasks: Arc<RwLock<HashMap<String, RepairTaskInternal>>>,
    por_history: Arc<RwLock<HashMap<u64, PorHistoryEntry>>>,
    next_history_id: Arc<AtomicU64>,
    default_sla_secs: u64,
    config: RepairConfig,
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
        if let Some(digest) = self.manifest_digest {
            if record.manifest_digest != digest {
                return false;
            }
        }
        if let Some(provider_id) = self.provider_id {
            if record.provider_id != provider_id {
                return false;
            }
        }
        if let Some(status) = self.status {
            if repair_task_status(&record.state) != status {
                return false;
            }
        }
        true
    }
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
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            por_history: Arc::new(RwLock::new(HashMap::new())),
            next_history_id: Arc::new(AtomicU64::new(1)),
            default_sla_secs: DEFAULT_REPAIR_SLA_SECS,
            config,
        }
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
        let entry = PorHistoryEntry {
            id: self.next_history_id.fetch_add(1, Ordering::Relaxed),
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
        let mut history = self.por_history.write().expect("por_history poisoned");
        let history_id = entry.id;
        history.insert(history_id, entry);
        Some(history_id)
    }

    /// Enqueue a repair report submitted by an auditor.
    pub fn enqueue_report(
        &self,
        report: RepairReportV1,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        report
            .validate()
            .map_err(RepairSchedulerError::InvalidReport)?;
        if let Some(por_id) = report.evidence.por_history_id {
            self.ensure_por_history_match(por_id, &report.evidence)?;
        }

        let mut guard = self.tasks.write().expect("repair tasks lock poisoned");
        let key = report.ticket_id.0.clone();
        if let Some(existing) = guard.get(&key) {
            if existing.report.evidence != report.evidence {
                return Err(RepairSchedulerError::DuplicateTicket {
                    ticket_id: report.ticket_id.to_string(),
                });
            }
            return Ok(existing.to_record());
        }

        let sla_deadline = report.submitted_at_unix.checked_add(self.default_sla_secs);
        let state = RepairTaskStateV1::Queued(QueuedRepairStateV1 {
            queued_at_unix: report.submitted_at_unix,
            sla_deadline_unix: sla_deadline,
        });
        let internal = RepairTaskInternal {
            report,
            state,
            sla_deadline_unix: sla_deadline,
            scheduler_notes: None,
            slash_proposal_digest: None,
            lease: None,
            idempotency: RepairTaskIdempotency::new(DEFAULT_IDEMPOTENCY_CACHE_SIZE),
        };
        let record = internal.to_record();
        guard.insert(key, internal);
        drop(guard);

        global_sorafs_repair_otel().record_task_transition("queued");
        global_or_default().inc_sorafs_repair_tasks("queued");
        Ok(record)
    }

    /// Submit a slash proposal associated with an escalated repair.
    pub fn submit_slash_proposal(
        &self,
        proposal: RepairSlashProposalV1,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        proposal
            .validate()
            .map_err(RepairSchedulerError::InvalidSlashProposal)?;

        let mut guard = self.tasks.write().expect("repair tasks lock poisoned");
        let key = proposal.ticket_id.0.clone();
        let task = guard
            .get_mut(&key)
            .ok_or_else(|| RepairSchedulerError::UnknownTicket {
                ticket_id: proposal.ticket_id.to_string(),
            })?;

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

        let queued_at = queued_at_unix(&task.state);
        ensure_transition_allowed(&task.state, "escalated", &proposal.ticket_id)?;
        if proposal.submitted_at_unix <= queued_at {
            return Err(RepairSchedulerError::InvalidTimestamp {
                ticket_id: proposal.ticket_id.to_string(),
            });
        }

        let mut digest = [0u8; 32];
        let mut buf = Vec::new();
        norito::core::NoritoSerialize::serialize(&proposal, &mut buf)
            .expect("serialize slash proposal");
        digest.copy_from_slice(hash(&buf).as_bytes());

        task.state = RepairTaskStateV1::Escalated(EscalatedRepairStateV1 {
            queued_at_unix: queued_at,
            escalated_at_unix: proposal.submitted_at_unix,
            reason: proposal.rationale.clone(),
        });
        task.slash_proposal_digest = Some(digest);
        global_sorafs_repair_otel().record_task_transition("escalated");
        self.observe_latency(queued_at, proposal.submitted_at_unix, "escalated");
        global_sorafs_repair_otel().record_slash_proposal("submitted");
        global_or_default().inc_sorafs_repair_tasks("escalated");
        global_or_default().inc_sorafs_slash_proposals("submitted");
        let record = task.to_record();
        Ok(record)
    }

    /// Fetch all repair tasks associated with `manifest_digest`.
    #[must_use]
    pub fn tasks_for_manifest(&self, manifest_digest: &[u8; 32]) -> Vec<RepairTaskRecordV1> {
        self.list_tasks(RepairTaskFilters::for_manifest(*manifest_digest))
    }

    /// List repair tasks with optional filters applied.
    #[must_use]
    pub fn list_tasks(&self, filters: RepairTaskFilters) -> Vec<RepairTaskRecordV1> {
        let guard = self.tasks.read().expect("repair tasks lock poisoned");
        let mut records: Vec<RepairTaskRecordV1> = guard
            .values()
            .map(RepairTaskInternal::to_record)
            .filter(|record| filters.matches(record))
            .collect();
        sort_repair_task_records(&mut records);
        records
    }

    /// Retrieve a repair task record by ticket id.
    #[must_use]
    pub fn task_record(&self, ticket_id: &RepairTicketId) -> Option<RepairTaskRecordV1> {
        let guard = self.tasks.read().expect("repair tasks lock poisoned");
        guard.get(&ticket_id.0).map(RepairTaskInternal::to_record)
    }

    /// Mark a repair ticket as actively being addressed.
    pub fn mark_in_progress(
        &self,
        ticket_id: &RepairTicketId,
        started_at_unix: u64,
        repair_agent: Option<String>,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        if started_at_unix == 0 {
            return Err(RepairSchedulerError::InvalidTimestamp {
                ticket_id: ticket_id.to_string(),
            });
        }
        let mut guard = self.tasks.write().expect("repair tasks lock poisoned");
        let task =
            guard
                .get_mut(&ticket_id.0)
                .ok_or_else(|| RepairSchedulerError::UnknownTicket {
                    ticket_id: ticket_id.to_string(),
                })?;
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
            }
            _ => {
                return Err(RepairSchedulerError::InvalidState {
                    ticket_id: ticket_id.to_string(),
                    state: format!("{:?}", task.state),
                });
            }
        }
        global_sorafs_repair_otel().record_task_transition("in_progress");
        global_or_default().inc_sorafs_repair_tasks("in_progress");
        Ok(task.to_record())
    }

    /// Mark a repair ticket as successfully resolved.
    pub fn mark_completed(
        &self,
        ticket_id: &RepairTicketId,
        completed_at_unix: u64,
        resolution_notes: Option<String>,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        if completed_at_unix == 0 {
            return Err(RepairSchedulerError::InvalidTimestamp {
                ticket_id: ticket_id.to_string(),
            });
        }
        let mut guard = self.tasks.write().expect("repair tasks lock poisoned");
        let task =
            guard
                .get_mut(&ticket_id.0)
                .ok_or_else(|| RepairSchedulerError::UnknownTicket {
                    ticket_id: ticket_id.to_string(),
                })?;
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
        task.scheduler_notes = resolution_notes;
        task.lease = None;
        global_sorafs_repair_otel().record_task_transition("completed");
        global_or_default().inc_sorafs_repair_tasks("completed");
        self.observe_latency(queued_at, completed_at_unix, "completed");
        Ok(task.to_record())
    }

    /// Mark a repair ticket as failed after an unsuccessful attempt.
    pub fn mark_failed(
        &self,
        ticket_id: &RepairTicketId,
        failed_at_unix: u64,
        reason: String,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        if failed_at_unix == 0 {
            return Err(RepairSchedulerError::InvalidTimestamp {
                ticket_id: ticket_id.to_string(),
            });
        }
        let mut guard = self.tasks.write().expect("repair tasks lock poisoned");
        let task =
            guard
                .get_mut(&ticket_id.0)
                .ok_or_else(|| RepairSchedulerError::UnknownTicket {
                    ticket_id: ticket_id.to_string(),
                })?;
        let queued_at = queued_at_unix(&task.state);
        if failed_at_unix <= queued_at {
            return Err(RepairSchedulerError::InvalidTimestamp {
                ticket_id: ticket_id.to_string(),
            });
        }
        task.state = RepairTaskStateV1::Failed(FailedRepairStateV1 {
            queued_at_unix: queued_at,
            failed_at_unix,
            reason: reason.clone(),
        });
        task.scheduler_notes = Some(reason);
        task.lease = None;
        global_sorafs_repair_otel().record_task_transition("failed");
        global_or_default().inc_sorafs_repair_tasks("failed");
        self.observe_latency(queued_at, failed_at_unix, "failed");
        Ok(task.to_record())
    }

    /// Claim a repair ticket for a worker.
    pub fn claim_ticket(
        &self,
        ticket_id: &RepairTicketId,
        worker_id: &str,
        claimed_at_unix: u64,
        idempotency_key: &str,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        ensure_idempotency_key(idempotency_key, ticket_id)?;
        ensure_worker_field(worker_id, "worker_id", MAX_WORKER_ID_BYTES, ticket_id)?;
        if claimed_at_unix == 0 {
            return Err(RepairSchedulerError::InvalidTimestamp {
                ticket_id: ticket_id.to_string(),
            });
        }

        let mut guard = self.tasks.write().expect("repair tasks lock poisoned");
        let task =
            guard
                .get_mut(&ticket_id.0)
                .ok_or_else(|| RepairSchedulerError::UnknownTicket {
                    ticket_id: ticket_id.to_string(),
                })?;
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
            return Ok(record);
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

        if let Some(lease) = &task.lease {
            if !lease.is_expired_at(claimed_at_unix) {
                return Err(RepairSchedulerError::LeaseHeld {
                    ticket_id: ticket_id.to_string(),
                    worker_id: lease.worker_id.clone(),
                });
            }
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
        global_sorafs_repair_otel().record_task_transition("in_progress");
        global_or_default().inc_sorafs_repair_tasks("in_progress");

        let record = task.to_record();
        task.idempotency
            .claim
            .remember(idempotency_key, signature, record.clone());
        Ok(record)
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

        let mut guard = self.tasks.write().expect("repair tasks lock poisoned");
        let task =
            guard
                .get_mut(&ticket_id.0)
                .ok_or_else(|| RepairSchedulerError::UnknownTicket {
                    ticket_id: ticket_id.to_string(),
                })?;
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
        Ok(record)
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

        let mut guard = self.tasks.write().expect("repair tasks lock poisoned");
        let task =
            guard
                .get_mut(&ticket_id.0)
                .ok_or_else(|| RepairSchedulerError::UnknownTicket {
                    ticket_id: ticket_id.to_string(),
                })?;
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
            return Ok(record);
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
        task.scheduler_notes = resolution_notes;
        task.lease = None;
        global_sorafs_repair_otel().record_task_transition("completed");
        global_or_default().inc_sorafs_repair_tasks("completed");
        self.observe_latency(queued_at, completed_at_unix, "completed");

        let record = task.to_record();
        task.idempotency
            .complete
            .remember(idempotency_key, signature, record.clone());
        Ok(record)
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
        ensure_idempotency_key(idempotency_key, ticket_id)?;
        ensure_worker_field(worker_id, "worker_id", MAX_WORKER_ID_BYTES, ticket_id)?;
        ensure_worker_field(&reason, "reason", MAX_REPAIR_NOTES_BYTES, ticket_id)?;
        if failed_at_unix == 0 {
            return Err(RepairSchedulerError::InvalidTimestamp {
                ticket_id: ticket_id.to_string(),
            });
        }

        let mut guard = self.tasks.write().expect("repair tasks lock poisoned");
        let task =
            guard
                .get_mut(&ticket_id.0)
                .ok_or_else(|| RepairSchedulerError::UnknownTicket {
                    ticket_id: ticket_id.to_string(),
                })?;
        let signature = RepairFailSignature {
            worker_id: worker_id.to_string(),
            failed_at_unix,
            reason: reason.clone(),
        };
        if let Some(record) =
            task.idempotency
                .fail
                .check_existing(idempotency_key, &signature, "fail", ticket_id)?
        {
            return Ok(record);
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

        task.state = RepairTaskStateV1::Failed(FailedRepairStateV1 {
            queued_at_unix: queued_at,
            failed_at_unix,
            reason: reason.clone(),
        });
        task.scheduler_notes = Some(reason);
        task.lease = None;
        global_sorafs_repair_otel().record_task_transition("failed");
        global_or_default().inc_sorafs_repair_tasks("failed");
        self.observe_latency(queued_at, failed_at_unix, "failed");

        let record = task.to_record();
        task.idempotency
            .fail
            .remember(idempotency_key, signature, record.clone());
        Ok(record)
    }

    fn ensure_por_history_match(
        &self,
        por_history_id: u64,
        evidence: &RepairEvidenceV1,
    ) -> Result<(), RepairSchedulerError> {
        let guard = self.por_history.read().expect("por history lock poisoned");
        let entry =
            guard
                .get(&por_history_id)
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
    report: RepairReportV1,
    state: RepairTaskStateV1,
    sla_deadline_unix: Option<u64>,
    scheduler_notes: Option<String>,
    slash_proposal_digest: Option<[u8; 32]>,
    lease: Option<RepairTaskLease>,
    idempotency: RepairTaskIdempotency,
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
}

#[derive(Debug)]
#[allow(dead_code)]
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

fn sort_repair_task_records(records: &mut Vec<RepairTaskRecordV1>) {
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

fn ensure_transition_allowed(
    state: &RepairTaskStateV1,
    next: &'static str,
    ticket_id: &RepairTicketId,
) -> Result<(), RepairSchedulerError> {
    match (state, next) {
        (RepairTaskStateV1::Queued(..), "escalated") => Ok(()),
        (RepairTaskStateV1::InProgress(..), "escalated") => Ok(()),
        _ => Err(RepairSchedulerError::InvalidState {
            ticket_id: ticket_id.to_string(),
            state: format!("{state:?}"),
        }),
    }
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
        if self.entries.len() >= self.capacity {
            if let Some(evicted) = self.order.pop_front() {
                self.entries.remove(&evicted);
            }
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
    use sorafs_manifest::repair::{
        REPAIR_EVIDENCE_VERSION_V1, REPAIR_REPORT_VERSION_V1, RepairCauseV1,
    };

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

    #[test]
    fn list_tasks_sorts_by_deadline_then_ticket() {
        let manager = RepairManager::new();
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
        let manager = RepairManager::new();
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
    fn task_record_returns_ticket() {
        let manager = RepairManager::new();
        let report = report("REP-320", [0x12; 32], [0x34; 32], 1_700_100_000);
        manager
            .enqueue_report(report.clone())
            .expect("enqueue report");

        let record = manager.task_record(&report.ticket_id).expect("task record");
        assert_eq!(record.ticket_id, report.ticket_id);
        assert_eq!(record.provider_id, report.evidence.provider_id);
    }

    #[test]
    fn claim_ticket_sets_in_progress_and_is_idempotent() {
        let manager = RepairManager::new();
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
    fn heartbeat_ticket_rejects_out_of_order_updates() {
        let manager = RepairManager::new();
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
        let manager = RepairManager::new();
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
    fn fail_ticket_transitions_to_failed() {
        let manager = RepairManager::new();
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
}
