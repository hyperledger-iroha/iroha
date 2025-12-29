//! Repair scheduler supporting SoraFS auditor workflows.
//!
//! The scheduler maintains an in-memory queue of repair tickets reported by
//! auditors, tracks proof-of-retrievability failures, and emits metrics so
//! operators can monitor SLA adherence. It is intentionally lightweight: the
//! canonical durability layer (Postgres) will be introduced once Torii wiring
//! and governance integration mature.

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
        RepairTicketId, RepairValidationError,
    },
};
use std::{
    collections::HashMap,
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};
use thiserror::Error;

const DEFAULT_REPAIR_SLA_SECS: u64 = 4 * 60 * 60;

/// Manages repair tickets and PoR failure history.
#[derive(Debug, Clone, Default)]
pub struct RepairManager {
    tasks: Arc<RwLock<HashMap<String, RepairTaskInternal>>>,
    por_history: Arc<RwLock<HashMap<u64, PorHistoryEntry>>>,
    next_history_id: Arc<AtomicU64>,
    default_sla_secs: u64,
}

impl RepairManager {
    /// Construct a new repair manager with the default SLA window.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            por_history: Arc::new(RwLock::new(HashMap::new())),
            next_history_id: Arc::new(AtomicU64::new(1)),
            default_sla_secs: DEFAULT_REPAIR_SLA_SECS,
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
        let guard = self.tasks.read().expect("repair tasks lock poisoned");
        guard
            .values()
            .filter(|task| task.report.evidence.manifest_digest == *manifest_digest)
            .map(RepairTaskInternal::to_record)
            .collect()
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
        global_sorafs_repair_otel().record_task_transition("failed");
        global_or_default().inc_sorafs_repair_tasks("failed");
        self.observe_latency(queued_at, failed_at_unix, "failed");
        Ok(task.to_record())
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
struct RepairTaskInternal {
    report: RepairReportV1,
    state: RepairTaskStateV1,
    sla_deadline_unix: Option<u64>,
    scheduler_notes: Option<String>,
    slash_proposal_digest: Option<[u8; 32]>,
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
    /// State transition not permitted.
    #[error("invalid state transition for ticket `{ticket_id}` from {state}")]
    InvalidState {
        /// Ticket identifier.
        ticket_id: String,
        /// Current state.
        state: String,
    },
}
