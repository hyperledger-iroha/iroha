//! Repair automation payloads for SoraFS (SF-8b).
//!
//! These types capture auditor-submitted repair reports, scheduler state, and
//! slash proposals emitted when storage providers miss remediation SLAs. All
//! payloads are Norito-encoded so governance, Torii, and tooling can exchange
//! deterministic artefacts without bespoke serializers.

#![allow(clippy::size_of_ref)]

use std::fmt;

use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

use crate::{deal::BASIS_POINTS_PER_UNIT, provider_advert::SignatureAlgorithm};

/// Schema version for [`RepairEvidenceV1`].
pub const REPAIR_EVIDENCE_VERSION_V1: u8 = 1;
/// Schema version for [`RepairReportV1`].
pub const REPAIR_REPORT_VERSION_V1: u8 = 1;
/// Schema version for [`RepairTaskRecordV1`].
pub const REPAIR_TASK_VERSION_V1: u8 = 1;
/// Schema version for [`RepairSlashProposalV1`].
pub const REPAIR_SLASH_PROPOSAL_VERSION_V1: u8 = 1;
/// Schema version for [`RepairEscalationPolicyV1`].
pub const REPAIR_ESCALATION_POLICY_VERSION_V1: u8 = 1;
/// Schema version for [`RepairEscalationApprovalV1`].
pub const REPAIR_ESCALATION_APPROVAL_VERSION_V1: u8 = 1;
/// Schema version for [`RepairTaskEventV1`].
pub const REPAIR_TASK_EVENT_VERSION_V1: u8 = 1;
/// Schema version for [`RepairAuditEventV1`].
pub const REPAIR_AUDIT_EVENT_VERSION_V1: u8 = 1;
/// Schema version for [`GcAuditPayloadV1`].
pub const GC_AUDIT_PAYLOAD_VERSION_V1: u8 = 1;
/// Schema version for [`GcAuditEventV1`].
pub const GC_AUDIT_EVENT_VERSION_V1: u8 = 1;
/// Schema version for [`SignedAuditorRequestV1`].
pub const SIGNED_AUDITOR_REQUEST_VERSION_V1: u8 = 1;
/// Schema version for [`RepairWorkerSignaturePayloadV1`].
pub const REPAIR_WORKER_SIGNATURE_VERSION_V1: u8 = 1;

/// Maximum length permitted for ticket identifiers and string fields.
const MAX_STRING_BYTES: usize = 256;

/// Identifier assigned to a repair ticket (e.g., `REP-351`).
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(transparent)]
pub struct RepairTicketId(pub String);

impl RepairTicketId {
    /// Validate the ticket identifier.
    pub fn validate(&self) -> Result<(), RepairValidationError> {
        validate_non_empty_string(&self.0, "ticket_id")?;
        if self.0.len() > MAX_STRING_BYTES {
            return Err(RepairValidationError::StringTooLong {
                field: "ticket_id",
                length: self.0.len(),
                max: MAX_STRING_BYTES,
            });
        }
        if !self
            .0
            .chars()
            .all(|ch| matches!(ch, 'A'..='Z' | '0'..='9' | '-' | '_' ))
        {
            return Err(RepairValidationError::InvalidTicketId {
                ticket_id: self.0.clone(),
            });
        }
        Ok(())
    }
}

impl fmt::Display for RepairTicketId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Worker actions supported by the repair scheduler.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
#[norito(tag = "action", content = "details")]
pub enum RepairWorkerActionV1 {
    /// Claim a queued repair ticket.
    #[norito(rename = "claim")]
    Claim {
        /// Unix timestamp when the claim was issued.
        claimed_at_unix: u64,
    },
    /// Emit a worker heartbeat extending the lease.
    #[norito(rename = "heartbeat")]
    Heartbeat {
        /// Unix timestamp when the heartbeat was issued.
        heartbeat_at_unix: u64,
    },
    /// Mark the ticket as completed.
    #[norito(rename = "complete")]
    Complete {
        /// Unix timestamp when the repair completed.
        completed_at_unix: u64,
        /// Optional resolution notes.
        #[norito(default)]
        resolution_notes: Option<String>,
    },
    /// Mark the ticket as failed.
    #[norito(rename = "fail")]
    Fail {
        /// Unix timestamp when the failure was recorded.
        failed_at_unix: u64,
        /// Failure reason.
        reason: String,
    },
}

/// Canonical payload signed by repair workers to authenticate actions.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct RepairWorkerSignaturePayloadV1 {
    /// Schema version (`REPAIR_WORKER_SIGNATURE_VERSION_V1`).
    pub version: u8,
    /// Ticket identifier associated with the action.
    pub ticket_id: RepairTicketId,
    /// Manifest digest under repair.
    pub manifest_digest: [u8; 32],
    /// Provider identifier owning the ticket.
    pub provider_id: [u8; 32],
    /// Worker account identifier (i105 form).
    pub worker_id: String,
    /// Idempotency key for the action.
    pub idempotency_key: String,
    /// Worker action details.
    pub action: RepairWorkerActionV1,
}

impl RepairWorkerSignaturePayloadV1 {
    /// Validate the worker signature payload.
    pub fn validate(&self) -> Result<(), RepairValidationError> {
        if self.version != REPAIR_WORKER_SIGNATURE_VERSION_V1 {
            return Err(RepairValidationError::UnsupportedVersion {
                field: "RepairWorkerSignaturePayloadV1",
                version: self.version,
            });
        }
        self.ticket_id.validate()?;
        ensure_digest(&self.manifest_digest, "manifest_digest")?;
        ensure_digest(&self.provider_id, "provider_id")?;
        validate_non_empty_string(&self.worker_id, "worker_id")?;
        if self.worker_id.len() > MAX_STRING_BYTES {
            return Err(RepairValidationError::StringTooLong {
                field: "worker_id",
                length: self.worker_id.len(),
                max: MAX_STRING_BYTES,
            });
        }
        validate_non_empty_string(&self.idempotency_key, "idempotency_key")?;
        if self.idempotency_key.len() > MAX_STRING_BYTES {
            return Err(RepairValidationError::StringTooLong {
                field: "idempotency_key",
                length: self.idempotency_key.len(),
                max: MAX_STRING_BYTES,
            });
        }
        match &self.action {
            RepairWorkerActionV1::Claim { claimed_at_unix } => {
                ensure_timestamp(*claimed_at_unix, "claimed_at_unix")?;
            }
            RepairWorkerActionV1::Heartbeat { heartbeat_at_unix } => {
                ensure_timestamp(*heartbeat_at_unix, "heartbeat_at_unix")?;
            }
            RepairWorkerActionV1::Complete {
                completed_at_unix,
                resolution_notes,
            } => {
                ensure_timestamp(*completed_at_unix, "completed_at_unix")?;
                if let Some(notes) = resolution_notes {
                    validate_optional_string(notes, "resolution_notes")?;
                }
            }
            RepairWorkerActionV1::Fail {
                failed_at_unix,
                reason,
            } => {
                ensure_timestamp(*failed_at_unix, "failed_at_unix")?;
                validate_non_empty_string(reason, "reason")?;
                if reason.len() > MAX_STRING_BYTES {
                    return Err(RepairValidationError::StringTooLong {
                        field: "reason",
                        length: reason.len(),
                        max: MAX_STRING_BYTES,
                    });
                }
            }
        }
        Ok(())
    }
}

/// Root cause captured by an auditor when scheduling repairs.
#[allow(clippy::size_of_ref)]
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
#[norito(tag = "kind", content = "data")]
pub enum RepairCauseV1 {
    /// Proof-of-retrievability failure exceeding the allowed threshold.
    #[norito(rename = "por_failure")]
    PorFailure {
        /// PoR challenge identifier (BLAKE3-256 digest).
        challenge_id: [u8; 32],
        /// Number of samples that failed validation.
        failed_samples: u16,
        /// Optional digest of the offending proof, if available.
        #[norito(default)]
        proof_digest: Option<[u8; 32]>,
    },
    /// Latency SLA breach for proof-of-time-to-retrieval sampling.
    #[norito(rename = "latency_sla")]
    LatencySla {
        /// Observed latency in milliseconds.
        observed_latency_ms: u32,
        /// Optional digest of the PoTR receipt associated with the breach.
        #[norito(default)]
        receipt_digest: Option<[u8; 32]>,
    },
    /// Replica shortfall discovered during sampling.
    #[norito(rename = "replica_shortfall")]
    ReplicaShortfall {
        /// Estimated number of missing chunks.
        missing_chunks: u32,
    },
    /// Manually triggered remediation (operator supplied reason).
    #[norito(rename = "manual")]
    Manual {
        /// Free-form description of the trigger.
        reason: String,
    },
}

impl RepairCauseV1 {
    /// Validate the repair cause payload.
    pub fn validate(&self) -> Result<(), RepairValidationError> {
        match self {
            Self::PorFailure {
                challenge_id,
                failed_samples,
                ..
            } => {
                ensure_digest(challenge_id, "challenge_id")?;
                if *failed_samples == 0 {
                    return Err(RepairValidationError::InvalidSamples);
                }
            }
            Self::LatencySla {
                observed_latency_ms,
                ..
            } => {
                if *observed_latency_ms == 0 {
                    return Err(RepairValidationError::InvalidLatency);
                }
            }
            Self::ReplicaShortfall { missing_chunks } => {
                if *missing_chunks == 0 {
                    return Err(RepairValidationError::InvalidMissingChunks);
                }
            }
            Self::Manual { reason } => {
                validate_non_empty_string(reason, "reason")?;
                if reason.len() > MAX_STRING_BYTES {
                    return Err(RepairValidationError::StringTooLong {
                        field: "reason",
                        length: reason.len(),
                        max: MAX_STRING_BYTES,
                    });
                }
            }
        }
        Ok(())
    }
}

/// Evidence accompanying a repair ticket.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct RepairEvidenceV1 {
    /// Schema version (`REPAIR_EVIDENCE_VERSION_V1`).
    pub version: u8,
    /// Manifest digest affected by the incident.
    pub manifest_digest: [u8; 32],
    /// Provider identifier associated with the incident.
    pub provider_id: [u8; 32],
    /// Optional PoR history entry linked to the incident.
    #[norito(default)]
    pub por_history_id: Option<u64>,
    /// Root cause of the incident.
    pub cause: RepairCauseV1,
    /// Optional JSON evidence blob encoded as UTF-8 text.
    #[norito(default)]
    pub evidence_json: Option<String>,
    /// Optional free-form notes.
    #[norito(default)]
    pub notes: Option<String>,
}

impl RepairEvidenceV1 {
    /// Validate the evidence payload.
    pub fn validate(&self) -> Result<(), RepairValidationError> {
        if self.version != REPAIR_EVIDENCE_VERSION_V1 {
            return Err(RepairValidationError::UnsupportedVersion {
                field: "RepairEvidenceV1",
                version: self.version,
            });
        }
        ensure_digest(&self.manifest_digest, "manifest_digest")?;
        ensure_digest(&self.provider_id, "provider_id")?;
        self.cause.validate()?;
        if let Some(notes) = &self.notes {
            validate_optional_string(notes, "notes")?;
        }
        Ok(())
    }
}

/// Auditor-submitted repair report.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct RepairReportV1 {
    /// Schema version (`REPAIR_REPORT_VERSION_V1`).
    pub version: u8,
    /// Repair ticket identifier.
    pub ticket_id: RepairTicketId,
    /// Auditor account (I105 string) submitting the report.
    pub auditor_account: String,
    /// Unix timestamp (seconds) when the report was submitted.
    pub submitted_at_unix: u64,
    /// Evidence describing the incident.
    pub evidence: RepairEvidenceV1,
    /// Optional free-form notes supplied by the auditor.
    #[norito(default)]
    pub notes: Option<String>,
}

impl RepairReportV1 {
    /// Validate the repair report payload.
    pub fn validate(&self) -> Result<(), RepairValidationError> {
        if self.version != REPAIR_REPORT_VERSION_V1 {
            return Err(RepairValidationError::UnsupportedVersion {
                field: "RepairReportV1",
                version: self.version,
            });
        }
        self.ticket_id.validate()?;
        validate_non_empty_string(&self.auditor_account, "auditor_account")?;
        if self.auditor_account.len() > MAX_STRING_BYTES {
            return Err(RepairValidationError::StringTooLong {
                field: "auditor_account",
                length: self.auditor_account.len(),
                max: MAX_STRING_BYTES,
            });
        }
        if self.submitted_at_unix == 0 {
            return Err(RepairValidationError::InvalidTimestamp {
                field: "submitted_at_unix",
                timestamp: self.submitted_at_unix,
            });
        }
        self.evidence.validate()?;
        if let Some(notes) = &self.notes {
            validate_optional_string(notes, "notes")?;
        }
        Ok(())
    }
}

/// Payload describing a queued repair ticket.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct QueuedRepairStateV1 {
    /// Epoch when the ticket was enqueued.
    pub queued_at_unix: u64,
    /// SLA deadline (seconds since epoch) for remediation.
    #[norito(default)]
    pub sla_deadline_unix: Option<u64>,
}

/// Payload describing an in-progress repair ticket.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct InProgressRepairStateV1 {
    /// Epoch when the ticket was enqueued.
    pub queued_at_unix: u64,
    /// Epoch when work started.
    pub started_at_unix: u64,
    /// Optional repair agent identity.
    #[norito(default)]
    pub repair_agent: Option<String>,
}

/// Payload describing a completed repair ticket.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct CompletedRepairStateV1 {
    /// Epoch when the ticket was enqueued.
    pub queued_at_unix: u64,
    /// Epoch when work started (may equal queued time for immediate repairs).
    pub started_at_unix: u64,
    /// Epoch when remediation finished.
    pub completed_at_unix: u64,
    /// Optional resolution notes.
    #[norito(default)]
    pub resolution_notes: Option<String>,
}

/// Payload describing a failed repair ticket.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct FailedRepairStateV1 {
    /// Epoch when the ticket was enqueued.
    pub queued_at_unix: u64,
    /// Epoch when the failure was recorded.
    pub failed_at_unix: u64,
    /// Human-readable reason.
    pub reason: String,
}

/// Payload describing an escalated repair ticket.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct EscalatedRepairStateV1 {
    /// Epoch when the ticket was enqueued.
    pub queued_at_unix: u64,
    /// Epoch when the escalation occurred.
    pub escalated_at_unix: u64,
    /// Escalation reason.
    pub reason: String,
}

/// Lifecycle state for a repair ticket.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[norito(tag = "state", content = "details")]
pub enum RepairTaskStateV1 {
    /// Ticket queued awaiting assignment.
    #[norito(rename = "queued")]
    Queued(QueuedRepairStateV1),
    /// Ticket acknowledged and actively being repaired.
    #[norito(rename = "in_progress")]
    InProgress(InProgressRepairStateV1),
    /// Ticket completed successfully.
    #[norito(rename = "completed")]
    Completed(CompletedRepairStateV1),
    /// Ticket failed (repair attempt unsuccessful).
    #[norito(rename = "failed")]
    Failed(FailedRepairStateV1),
    /// Ticket escalated to governance.
    #[norito(rename = "escalated")]
    Escalated(EscalatedRepairStateV1),
}

impl RepairTaskStateV1 {
    /// Validate state invariants.
    pub fn validate(&self) -> Result<(), RepairValidationError> {
        match self {
            Self::Queued(QueuedRepairStateV1 {
                queued_at_unix,
                sla_deadline_unix,
            }) => {
                ensure_timestamp(*queued_at_unix, "queued_at_unix")?;
                if let Some(deadline) = sla_deadline_unix {
                    ensure_timestamp(*deadline, "sla_deadline_unix")?;
                    if deadline <= queued_at_unix {
                        return Err(RepairValidationError::InvalidTimestampOrder {
                            earlier_field: "queued_at_unix",
                            earlier: *queued_at_unix,
                            later_field: "sla_deadline_unix",
                            later: *deadline,
                        });
                    }
                }
            }
            Self::InProgress(InProgressRepairStateV1 {
                queued_at_unix,
                started_at_unix,
                repair_agent,
            }) => {
                ensure_timestamp(*queued_at_unix, "queued_at_unix")?;
                ensure_timestamp(*started_at_unix, "started_at_unix")?;
                if started_at_unix < queued_at_unix {
                    return Err(RepairValidationError::InvalidTimestampOrder {
                        earlier_field: "queued_at_unix",
                        earlier: *queued_at_unix,
                        later_field: "started_at_unix",
                        later: *started_at_unix,
                    });
                }
                if let Some(agent) = repair_agent {
                    validate_non_empty_string(agent, "repair_agent")?;
                    if agent.len() > MAX_STRING_BYTES {
                        return Err(RepairValidationError::StringTooLong {
                            field: "repair_agent",
                            length: agent.len(),
                            max: MAX_STRING_BYTES,
                        });
                    }
                }
            }
            Self::Completed(CompletedRepairStateV1 {
                queued_at_unix,
                started_at_unix,
                completed_at_unix,
                resolution_notes,
            }) => {
                ensure_timestamp(*queued_at_unix, "queued_at_unix")?;
                ensure_timestamp(*started_at_unix, "started_at_unix")?;
                ensure_timestamp(*completed_at_unix, "completed_at_unix")?;
                if started_at_unix < queued_at_unix {
                    return Err(RepairValidationError::InvalidTimestampOrder {
                        earlier_field: "queued_at_unix",
                        earlier: *queued_at_unix,
                        later_field: "started_at_unix",
                        later: *started_at_unix,
                    });
                }
                if completed_at_unix < started_at_unix {
                    return Err(RepairValidationError::InvalidTimestampOrder {
                        earlier_field: "started_at_unix",
                        earlier: *started_at_unix,
                        later_field: "completed_at_unix",
                        later: *completed_at_unix,
                    });
                }
                if let Some(notes) = resolution_notes {
                    validate_optional_string(notes, "resolution_notes")?;
                }
            }
            Self::Failed(FailedRepairStateV1 {
                queued_at_unix,
                failed_at_unix,
                reason,
            }) => {
                ensure_timestamp(*queued_at_unix, "queued_at_unix")?;
                ensure_timestamp(*failed_at_unix, "failed_at_unix")?;
                if failed_at_unix < queued_at_unix {
                    return Err(RepairValidationError::InvalidTimestampOrder {
                        earlier_field: "queued_at_unix",
                        earlier: *queued_at_unix,
                        later_field: "failed_at_unix",
                        later: *failed_at_unix,
                    });
                }
                validate_non_empty_string(reason, "reason")?;
                if reason.len() > MAX_STRING_BYTES {
                    return Err(RepairValidationError::StringTooLong {
                        field: "reason",
                        length: reason.len(),
                        max: MAX_STRING_BYTES,
                    });
                }
            }
            Self::Escalated(EscalatedRepairStateV1 {
                queued_at_unix,
                escalated_at_unix,
                reason,
            }) => {
                ensure_timestamp(*queued_at_unix, "queued_at_unix")?;
                ensure_timestamp(*escalated_at_unix, "escalated_at_unix")?;
                if escalated_at_unix < queued_at_unix {
                    return Err(RepairValidationError::InvalidTimestampOrder {
                        earlier_field: "queued_at_unix",
                        earlier: *queued_at_unix,
                        later_field: "escalated_at_unix",
                        later: *escalated_at_unix,
                    });
                }
                validate_non_empty_string(reason, "reason")?;
                if reason.len() > MAX_STRING_BYTES {
                    return Err(RepairValidationError::StringTooLong {
                        field: "reason",
                        length: reason.len(),
                        max: MAX_STRING_BYTES,
                    });
                }
            }
        }
        Ok(())
    }
}

/// Scheduler record describing the current state of a repair ticket.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct RepairTaskRecordV1 {
    /// Schema version (`REPAIR_TASK_VERSION_V1`).
    pub version: u8,
    /// Ticket identifier.
    pub ticket_id: RepairTicketId,
    /// Manifest digest affected by the incident.
    pub manifest_digest: [u8; 32],
    /// Provider identifier associated with the ticket.
    pub provider_id: [u8; 32],
    /// Auditor who submitted the originating report.
    pub auditor_account: String,
    /// Current lifecycle state.
    pub state: RepairTaskStateV1,
    /// Optional PoR history linkage.
    #[norito(default)]
    pub por_history_id: Option<u64>,
    /// Optional uptime SLA deadline for remediation (seconds since epoch).
    #[norito(default)]
    pub sla_deadline_unix: Option<u64>,
    /// Optional notes injected by the scheduler (e.g., escalation context).
    #[norito(default)]
    pub scheduler_notes: Option<String>,
    /// Optional pending slash proposal digest associated with the ticket.
    #[norito(default)]
    pub slash_proposal_digest: Option<[u8; 32]>,
}

impl RepairTaskRecordV1 {
    /// Validate the task record.
    pub fn validate(&self) -> Result<(), RepairValidationError> {
        if self.version != REPAIR_TASK_VERSION_V1 {
            return Err(RepairValidationError::UnsupportedVersion {
                field: "RepairTaskRecordV1",
                version: self.version,
            });
        }
        self.ticket_id.validate()?;
        ensure_digest(&self.manifest_digest, "manifest_digest")?;
        ensure_digest(&self.provider_id, "provider_id")?;
        validate_non_empty_string(&self.auditor_account, "auditor_account")?;
        if self.auditor_account.len() > MAX_STRING_BYTES {
            return Err(RepairValidationError::StringTooLong {
                field: "auditor_account",
                length: self.auditor_account.len(),
                max: MAX_STRING_BYTES,
            });
        }
        self.state.validate()?;
        if let Some(deadline) = self.sla_deadline_unix {
            ensure_timestamp(deadline, "sla_deadline_unix")?;
        }
        if let Some(notes) = &self.scheduler_notes {
            validate_optional_string(notes, "scheduler_notes")?;
        }
        Ok(())
    }
}

/// Slash proposal generated after a repair escalation.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(tag = "status", content = "value", rename_all = "snake_case")]
pub enum RepairTaskStatusV1 {
    /// Ticket queued awaiting verification.
    Queued,
    /// Evidence verification in progress.
    Verifying,
    /// Ticket assigned to a worker.
    InProgress,
    /// Ticket remediation completed successfully.
    Completed,
    /// Ticket remediation failed.
    Failed,
    /// Ticket escalated to governance.
    Escalated,
}

impl fmt::Display for RepairTaskStatusV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Queued => "queued",
            Self::Verifying => "verifying",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Escalated => "escalated",
        };
        f.write_str(label)
    }
}

/// Append-only event emitted whenever a repair ticket changes status.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct RepairTaskEventV1 {
    /// Schema version (`REPAIR_TASK_EVENT_VERSION_V1`).
    pub version: u8,
    /// Ticket identifier referenced by the event.
    pub ticket_id: RepairTicketId,
    /// Manifest digest associated with the ticket.
    pub manifest_digest: [u8; 32],
    /// Provider identifier associated with the ticket.
    pub provider_id: [u8; 32],
    /// Status after the transition.
    pub status: RepairTaskStatusV1,
    /// Unix timestamp (seconds) when the event occurred.
    pub occurred_at_unix: u64,
    /// Optional scheduler/worker identity that emitted the event.
    #[norito(default)]
    pub actor: Option<String>,
    /// Optional human-readable message (e.g., escalation reason).
    #[norito(default)]
    pub message: Option<String>,
}

impl RepairTaskEventV1 {
    /// Validate the event payload.
    pub fn validate(&self) -> Result<(), RepairValidationError> {
        if self.version != REPAIR_TASK_EVENT_VERSION_V1 {
            return Err(RepairValidationError::UnsupportedVersion {
                field: "RepairTaskEventV1",
                version: self.version,
            });
        }
        self.ticket_id.validate()?;
        ensure_digest(&self.manifest_digest, "manifest_digest")?;
        ensure_digest(&self.provider_id, "provider_id")?;
        ensure_timestamp(self.occurred_at_unix, "occurred_at_unix")?;
        if let Some(actor) = &self.actor {
            validate_optional_string(actor, "actor")?;
        }
        if let Some(message) = &self.message {
            validate_optional_string(message, "message")?;
        }
        Ok(())
    }
}

/// Header metadata for audit trail payloads (ordering + signer + digest).
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct SorafsAuditHeaderV1 {
    /// Monotonic sequence number used for deterministic ordering.
    pub sequence: u64,
    /// Unix timestamp (seconds) when the audit event was recorded.
    pub occurred_at_unix: u64,
    /// Account identifier that signed the audit payload.
    pub signer: String,
    /// Digest of the payload encoded with Norito.
    pub payload_digest: [u8; 32],
}

/// Canonical audit event emitted for repair status transitions.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct RepairAuditEventV1 {
    /// Schema version (`REPAIR_AUDIT_EVENT_VERSION_V1`).
    pub version: u8,
    /// Shared audit header (ordering + signer + digest).
    pub header: SorafsAuditHeaderV1,
    /// Repair task transition payload.
    pub payload: RepairTaskEventV1,
}

/// Canonical GC audit payload emitted when retention evicts data.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct GcAuditPayloadV1 {
    /// Schema version (`GC_AUDIT_PAYLOAD_VERSION_V1`).
    pub version: u8,
    /// Manifest digest evicted by the GC sweep.
    pub manifest_digest: [u8; 32],
    /// Provider identifier associated with the eviction.
    pub provider_id: [u8; 32],
    /// Unix timestamp (seconds) when eviction completed.
    pub evicted_at_unix: u64,
    /// Total bytes freed by the eviction.
    pub freed_bytes: u64,
    /// Reason label for the eviction.
    pub reason: String,
    /// Optional block reason when eviction could not proceed.
    #[norito(default)]
    pub blocked_reason: Option<String>,
}

/// Canonical audit event emitted for GC/retention actions.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct GcAuditEventV1 {
    /// Schema version (`GC_AUDIT_EVENT_VERSION_V1`).
    pub version: u8,
    /// Shared audit header (ordering + signer + digest).
    pub header: SorafsAuditHeaderV1,
    /// GC eviction payload.
    pub payload: GcAuditPayloadV1,
}

/// Governance policy applied to repair escalations and slash proposals.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct RepairEscalationPolicyV1 {
    /// Schema version (`REPAIR_ESCALATION_POLICY_VERSION_V1`).
    pub version: u8,
    /// Approval quorum (basis points) required for escalation/slash decisions.
    pub quorum_bps: u16,
    /// Minimum number of distinct voters required.
    pub minimum_voters: u32,
    /// Dispute window in seconds after escalation before governance finalizes.
    pub dispute_window_secs: u64,
    /// Appeal window in seconds after approval before a decision is final.
    pub appeal_window_secs: u64,
    /// Maximum slash penalty allowed for repair escalations (nano-XOR).
    pub max_penalty_nano: u128,
}

impl RepairEscalationPolicyV1 {
    /// Validate the governance policy payload.
    pub fn validate(&self) -> Result<(), RepairValidationError> {
        if self.version != REPAIR_ESCALATION_POLICY_VERSION_V1 {
            return Err(RepairValidationError::UnsupportedVersion {
                field: "RepairEscalationPolicyV1",
                version: self.version,
            });
        }
        if self.quorum_bps > BASIS_POINTS_PER_UNIT {
            return Err(RepairValidationError::InvalidQuorumBps {
                quorum_bps: self.quorum_bps,
            });
        }
        if self.minimum_voters == 0 {
            return Err(RepairValidationError::InvalidMinimumVoters);
        }
        Ok(())
    }
}

/// Governance approval summary attached to an escalation proposal.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct RepairEscalationApprovalV1 {
    /// Schema version (`REPAIR_ESCALATION_APPROVAL_VERSION_V1`).
    pub version: u8,
    /// Votes approving the escalation decision.
    pub approve_votes: u32,
    /// Votes rejecting the escalation decision.
    pub reject_votes: u32,
    /// Votes abstaining from the escalation decision.
    pub abstain_votes: u32,
    /// Unix timestamp (seconds) when approval was recorded.
    pub approved_at_unix: u64,
    /// Unix timestamp (seconds) when the decision became final after appeals.
    pub finalized_at_unix: u64,
}

impl RepairEscalationApprovalV1 {
    /// Validate the approval summary payload.
    pub fn validate(&self) -> Result<(), RepairValidationError> {
        if self.version != REPAIR_ESCALATION_APPROVAL_VERSION_V1 {
            return Err(RepairValidationError::UnsupportedVersion {
                field: "RepairEscalationApprovalV1",
                version: self.version,
            });
        }
        let total_votes = u64::from(self.approve_votes)
            + u64::from(self.reject_votes)
            + u64::from(self.abstain_votes);
        if total_votes == 0 {
            return Err(RepairValidationError::InvalidVoteCount);
        }
        ensure_timestamp(self.approved_at_unix, "approved_at_unix")?;
        ensure_timestamp(self.finalized_at_unix, "finalized_at_unix")?;
        if self.finalized_at_unix < self.approved_at_unix {
            return Err(RepairValidationError::InvalidTimestampOrder {
                earlier_field: "approved_at_unix",
                earlier: self.approved_at_unix,
                later_field: "finalized_at_unix",
                later: self.finalized_at_unix,
            });
        }
        Ok(())
    }
}

/// Slash proposal generated after a repair escalation.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct RepairSlashProposalV1 {
    /// Schema version (`REPAIR_SLASH_PROPOSAL_VERSION_V1`).
    pub version: u8,
    /// Ticket identifier associated with the proposal.
    pub ticket_id: RepairTicketId,
    /// Provider identifier to be slashed.
    pub provider_id: [u8; 32],
    /// Manifest digest affected by the incident.
    pub manifest_digest: [u8; 32],
    /// Auditor submitting the proposal.
    pub auditor_account: String,
    /// Proposed bond penalty (nano-XOR).
    pub proposed_penalty_nano: u128,
    /// Unix timestamp when the proposal was created.
    pub submitted_at_unix: u64,
    /// Human-readable rationale for governance review.
    pub rationale: String,
    /// Optional governance approval summary attached to the proposal.
    #[norito(default)]
    pub approval: Option<RepairEscalationApprovalV1>,
}

impl RepairSlashProposalV1 {
    /// Validate the slash proposal payload.
    pub fn validate(&self) -> Result<(), RepairValidationError> {
        if self.version != REPAIR_SLASH_PROPOSAL_VERSION_V1 {
            return Err(RepairValidationError::UnsupportedVersion {
                field: "RepairSlashProposalV1",
                version: self.version,
            });
        }
        self.ticket_id.validate()?;
        ensure_digest(&self.provider_id, "provider_id")?;
        ensure_digest(&self.manifest_digest, "manifest_digest")?;
        validate_non_empty_string(&self.auditor_account, "auditor_account")?;
        if self.auditor_account.len() > MAX_STRING_BYTES {
            return Err(RepairValidationError::StringTooLong {
                field: "auditor_account",
                length: self.auditor_account.len(),
                max: MAX_STRING_BYTES,
            });
        }
        if self.proposed_penalty_nano == 0 {
            return Err(RepairValidationError::InvalidPenalty);
        }
        ensure_timestamp(self.submitted_at_unix, "submitted_at_unix")?;
        validate_non_empty_string(&self.rationale, "rationale")?;
        if self.rationale.len() > MAX_STRING_BYTES {
            return Err(RepairValidationError::StringTooLong {
                field: "rationale",
                length: self.rationale.len(),
                max: MAX_STRING_BYTES,
            });
        }
        if let Some(approval) = &self.approval {
            approval.validate()?;
        }
        Ok(())
    }
}

/// Signature envelope used by auditors.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct AuditorSignatureV1 {
    /// Signature algorithm identifier.
    pub algorithm: SignatureAlgorithm,
    /// Public key bytes corresponding to the signing key.
    pub public_key: Vec<u8>,
    /// Raw signature bytes.
    pub signature: Vec<u8>,
}

impl AuditorSignatureV1 {
    fn validate(&self) -> Result<(), RepairValidationError> {
        if self.public_key.is_empty() {
            return Err(RepairValidationError::InvalidPublicKey);
        }
        if self.signature.is_empty() {
            return Err(RepairValidationError::InvalidSignature);
        }
        Ok(())
    }
}

/// Payloads supported inside a signed auditor request.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[norito(tag = "kind", content = "payload")]
pub enum SignedAuditorRequestPayloadV1 {
    /// Submit a new repair report.
    #[norito(rename = "repair_report")]
    RepairReport(RepairReportV1),
    /// Submit a slash proposal linked to an escalation.
    #[norito(rename = "slash_proposal")]
    SlashProposal(RepairSlashProposalV1),
}

impl SignedAuditorRequestPayloadV1 {
    fn auditor_account(&self) -> &str {
        match self {
            Self::RepairReport(report) => &report.auditor_account,
            Self::SlashProposal(proposal) => &proposal.auditor_account,
        }
    }

    fn validate_inner(&self) -> Result<(), RepairValidationError> {
        match self {
            Self::RepairReport(report) => report.validate(),
            Self::SlashProposal(proposal) => proposal.validate(),
        }
    }
}

/// Signed envelope authorising an auditor action.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct SignedAuditorRequestV1 {
    /// Schema version (`SIGNED_AUDITOR_REQUEST_VERSION_V1`).
    pub version: u8,
    /// Auditor account submitting the request (I105 string form).
    pub auditor_account: String,
    /// Monotonic nonce used for replay protection.
    pub nonce: u64,
    /// Wrapped payload (`RepairReport` or `RepairSlashProposal`).
    pub payload: SignedAuditorRequestPayloadV1,
    /// Signature authenticating the payload.
    pub signature: AuditorSignatureV1,
}

impl SignedAuditorRequestV1 {
    /// Validate the signed request envelope.
    pub fn validate(&self) -> Result<(), RepairValidationError> {
        if self.version != SIGNED_AUDITOR_REQUEST_VERSION_V1 {
            return Err(RepairValidationError::UnsupportedVersion {
                field: "SignedAuditorRequestV1",
                version: self.version,
            });
        }
        validate_non_empty_string(&self.auditor_account, "auditor_account")?;
        if self.auditor_account.len() > MAX_STRING_BYTES {
            return Err(RepairValidationError::StringTooLong {
                field: "auditor_account",
                length: self.auditor_account.len(),
                max: MAX_STRING_BYTES,
            });
        }
        if self.nonce == 0 {
            return Err(RepairValidationError::InvalidNonce);
        }
        self.payload.validate_inner()?;
        if self.payload.auditor_account() != self.auditor_account {
            return Err(RepairValidationError::EnvelopeAccountMismatch {
                envelope: self.auditor_account.clone(),
                payload: self.payload.auditor_account().to_owned(),
            });
        }
        self.signature.validate()?;
        Ok(())
    }
}

/// Errors emitted while validating repair payloads.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RepairValidationError {
    /// Ticket identifier contains unsupported characters.
    #[error(
        "ticket identifier `{ticket_id}` contains unsupported characters (allowed: A-Z, 0-9, '-', '_')"
    )]
    InvalidTicketId {
        /// Identifier provided by the caller.
        ticket_id: String,
    },
    /// String field exceeded the permitted length.
    #[error("{field} length {length} exceeds maximum {max}")]
    StringTooLong {
        /// Field name.
        field: &'static str,
        /// Observed length.
        length: usize,
        /// Maximum allowed length.
        max: usize,
    },
    /// String field must not be empty.
    #[error("{field} must not be empty")]
    EmptyString {
        /// Field name.
        field: &'static str,
    },
    /// String contains only whitespace.
    #[error("{field} must not be blank")]
    BlankString {
        /// Field name.
        field: &'static str,
    },
    /// Timestamp field invalid.
    #[error("{field} timestamp must be non-zero")]
    InvalidTimestamp {
        /// Field name.
        field: &'static str,
        /// Provided timestamp.
        timestamp: u64,
    },
    /// Timestamp ordering invalid.
    #[error("{earlier_field} ({earlier}) must be <= {later_field} ({later})")]
    InvalidTimestampOrder {
        /// Earlier field name.
        earlier_field: &'static str,
        /// Earlier timestamp.
        earlier: u64,
        /// Later field name.
        later_field: &'static str,
        /// Later timestamp.
        later: u64,
    },
    /// Unsupported schema version encountered.
    #[error("{field} version {version} unsupported")]
    UnsupportedVersion {
        /// Field name.
        field: &'static str,
        /// Version observed.
        version: u8,
    },
    /// PoR failure reported zero samples.
    #[error("failed sample count must be > 0")]
    InvalidSamples,
    /// Latency SLA breach missing latency measurement.
    #[error("observed latency must be > 0 ms")]
    InvalidLatency,
    /// Replica shortfall missing chunk count.
    #[error("missing chunk count must be > 0")]
    InvalidMissingChunks,
    /// Proposed penalty must be positive.
    #[error("proposed penalty must be greater than zero")]
    InvalidPenalty,
    /// Approval quorum exceeds basis point bounds.
    #[error("quorum_bps must be within 0..=10_000 (got {quorum_bps})")]
    InvalidQuorumBps {
        /// Quorum basis points provided.
        quorum_bps: u16,
    },
    /// Minimum voters must be greater than zero.
    #[error("minimum_voters must be greater than zero")]
    InvalidMinimumVoters,
    /// Approval vote counts invalid.
    #[error("approval vote counts must be non-zero")]
    InvalidVoteCount,
    /// Auditor signature public key missing.
    #[error("auditor signature public key must not be empty")]
    InvalidPublicKey,
    /// Auditor signature payload missing.
    #[error("auditor signature must not be empty")]
    InvalidSignature,
    /// Auditor request nonce invalid.
    #[error("nonce must be greater than zero")]
    InvalidNonce,
    /// Envelope and payload auditor accounts disagree.
    #[error("auditor account mismatch between envelope ({envelope}) and payload ({payload})")]
    EnvelopeAccountMismatch { envelope: String, payload: String },
}

fn validate_non_empty_string(
    value: &str,
    field: &'static str,
) -> Result<(), RepairValidationError> {
    if value.is_empty() {
        return Err(RepairValidationError::EmptyString { field });
    }
    if value.trim().is_empty() {
        return Err(RepairValidationError::BlankString { field });
    }
    Ok(())
}

fn validate_optional_string(value: &str, field: &'static str) -> Result<(), RepairValidationError> {
    validate_non_empty_string(value, field)?;
    if value.len() > MAX_STRING_BYTES {
        return Err(RepairValidationError::StringTooLong {
            field,
            length: value.len(),
            max: MAX_STRING_BYTES,
        });
    }
    Ok(())
}

fn ensure_timestamp(timestamp: u64, field: &'static str) -> Result<(), RepairValidationError> {
    if timestamp == 0 {
        return Err(RepairValidationError::InvalidTimestamp { field, timestamp });
    }
    Ok(())
}

fn ensure_digest(digest: &[u8; 32], field: &'static str) -> Result<(), RepairValidationError> {
    if digest.iter().all(|byte| *byte == 0) {
        return Err(RepairValidationError::EmptyString { field });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::codec::{Decode, Encode};

    fn provider_id() -> [u8; 32] {
        [0xAA; 32]
    }

    fn manifest_digest() -> [u8; 32] {
        [0xBB; 32]
    }

    fn sample_evidence() -> RepairEvidenceV1 {
        RepairEvidenceV1 {
            version: REPAIR_EVIDENCE_VERSION_V1,
            manifest_digest: manifest_digest(),
            provider_id: provider_id(),
            por_history_id: Some(42),
            cause: RepairCauseV1::PorFailure {
                challenge_id: [0xCC; 32],
                failed_samples: 4,
                proof_digest: None,
            },
            evidence_json: None,
            notes: Some("provider reported disk failure".into()),
        }
    }

    #[test]
    fn ticket_validation_succeeds() {
        let id = RepairTicketId("REP-351".into());
        assert!(id.validate().is_ok());
    }

    #[test]
    fn ticket_validation_rejects_lowercase() {
        let id = RepairTicketId("rep-351".into());
        assert!(matches!(
            id.validate(),
            Err(RepairValidationError::InvalidTicketId { .. })
        ));
    }

    #[test]
    fn evidence_validation_succeeds() {
        let evidence = sample_evidence();
        assert!(evidence.validate().is_ok());
    }

    #[test]
    fn report_validation_succeeds() {
        let report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-351".into()),
            auditor_account: "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ".into(),
            submitted_at_unix: 1_704_361_600,
            evidence: sample_evidence(),
            notes: Some("auto-generated from PoR pipeline".into()),
        };
        assert!(report.validate().is_ok());
    }

    #[test]
    fn task_state_transitions_validate() {
        let queued = RepairTaskStateV1::Queued(QueuedRepairStateV1 {
            queued_at_unix: 1,
            sla_deadline_unix: Some(2),
        });
        assert!(queued.validate().is_ok());

        let in_progress = RepairTaskStateV1::InProgress(InProgressRepairStateV1 {
            queued_at_unix: 1,
            started_at_unix: 2,
            repair_agent: Some("soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ".into()),
        });
        assert!(in_progress.validate().is_ok());

        let completed = RepairTaskStateV1::Completed(CompletedRepairStateV1 {
            queued_at_unix: 1,
            started_at_unix: 2,
            completed_at_unix: 3,
            resolution_notes: None,
        });
        assert!(completed.validate().is_ok());
    }

    #[test]
    fn task_record_validation_succeeds() {
        let record = RepairTaskRecordV1 {
            version: REPAIR_TASK_VERSION_V1,
            ticket_id: RepairTicketId("REP-351".into()),
            manifest_digest: manifest_digest(),
            provider_id: provider_id(),
            auditor_account: "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ".into(),
            state: RepairTaskStateV1::Queued(QueuedRepairStateV1 {
                queued_at_unix: 1,
                sla_deadline_unix: Some(2),
            }),
            por_history_id: Some(42),
            sla_deadline_unix: Some(2),
            scheduler_notes: Some("awaiting provider acknowledgement".into()),
            slash_proposal_digest: None,
        };
        assert!(record.validate().is_ok());
    }

    #[test]
    fn slash_proposal_validation_succeeds() {
        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: RepairTicketId("REP-351".into()),
            provider_id: provider_id(),
            manifest_digest: manifest_digest(),
            auditor_account: "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ".into(),
            proposed_penalty_nano: 1_000_000_000,
            submitted_at_unix: 1_704_361_600,
            rationale: "Repeated PoR failures beyond SLA".into(),
            approval: None,
        };
        assert!(proposal.validate().is_ok());
    }

    #[test]
    fn escalation_approval_validation_succeeds() {
        let approval = RepairEscalationApprovalV1 {
            version: REPAIR_ESCALATION_APPROVAL_VERSION_V1,
            approve_votes: 2,
            reject_votes: 1,
            abstain_votes: 0,
            approved_at_unix: 1_704_361_700,
            finalized_at_unix: 1_704_361_900,
        };
        assert!(approval.validate().is_ok());
    }

    #[test]
    fn escalation_policy_validation_succeeds() {
        let policy = RepairEscalationPolicyV1 {
            version: REPAIR_ESCALATION_POLICY_VERSION_V1,
            quorum_bps: 6_667,
            minimum_voters: 3,
            dispute_window_secs: 86_400,
            appeal_window_secs: 604_800,
            max_penalty_nano: 1_000,
        };
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn escalation_approval_validation_rejects_empty_votes() {
        let approval = RepairEscalationApprovalV1 {
            version: REPAIR_ESCALATION_APPROVAL_VERSION_V1,
            approve_votes: 0,
            reject_votes: 0,
            abstain_votes: 0,
            approved_at_unix: 1_704_361_700,
            finalized_at_unix: 1_704_361_900,
        };
        assert!(matches!(
            approval.validate(),
            Err(RepairValidationError::InvalidVoteCount)
        ));
    }

    fn sample_signature() -> AuditorSignatureV1 {
        AuditorSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: vec![1u8; 32],
            signature: vec![2u8; 64],
        }
    }

    #[test]
    fn task_event_validation_succeeds() {
        let event = RepairTaskEventV1 {
            version: REPAIR_TASK_EVENT_VERSION_V1,
            ticket_id: RepairTicketId("REP-351".into()),
            manifest_digest: manifest_digest(),
            provider_id: provider_id(),
            status: RepairTaskStatusV1::InProgress,
            occurred_at_unix: 1_704_361_600,
            actor: Some("worker-1".into()),
            message: Some("accepted by worker".into()),
        };
        assert!(event.validate().is_ok());
    }

    #[test]
    fn task_event_rejects_blank_actor() {
        let event = RepairTaskEventV1 {
            version: REPAIR_TASK_EVENT_VERSION_V1,
            ticket_id: RepairTicketId("REP-351".into()),
            manifest_digest: manifest_digest(),
            provider_id: provider_id(),
            status: RepairTaskStatusV1::Queued,
            occurred_at_unix: 1,
            actor: Some("   ".into()),
            message: None,
        };
        assert!(matches!(
            event.validate(),
            Err(RepairValidationError::BlankString { field: "actor" })
        ));
    }

    #[test]
    fn task_event_rejects_empty_manifest_digest() {
        let event = RepairTaskEventV1 {
            version: REPAIR_TASK_EVENT_VERSION_V1,
            ticket_id: RepairTicketId("REP-352".into()),
            manifest_digest: [0u8; 32],
            provider_id: provider_id(),
            status: RepairTaskStatusV1::Queued,
            occurred_at_unix: 1,
            actor: None,
            message: None,
        };
        assert!(matches!(
            event.validate(),
            Err(RepairValidationError::EmptyString {
                field: "manifest_digest"
            })
        ));
    }

    #[test]
    fn task_event_rejects_empty_provider_id() {
        let event = RepairTaskEventV1 {
            version: REPAIR_TASK_EVENT_VERSION_V1,
            ticket_id: RepairTicketId("REP-353".into()),
            manifest_digest: manifest_digest(),
            provider_id: [0u8; 32],
            status: RepairTaskStatusV1::Queued,
            occurred_at_unix: 1,
            actor: None,
            message: None,
        };
        assert!(matches!(
            event.validate(),
            Err(RepairValidationError::EmptyString {
                field: "provider_id"
            })
        ));
    }

    #[test]
    fn signed_auditor_request_validation_succeeds() {
        let report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-351".into()),
            auditor_account: "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ".into(),
            submitted_at_unix: 1_704_361_600,
            evidence: sample_evidence(),
            notes: None,
        };
        let envelope = SignedAuditorRequestV1 {
            version: SIGNED_AUDITOR_REQUEST_VERSION_V1,
            auditor_account: "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ".into(),
            nonce: 42,
            payload: SignedAuditorRequestPayloadV1::RepairReport(report),
            signature: sample_signature(),
        };
        assert!(envelope.validate().is_ok());
    }

    #[test]
    fn signed_auditor_request_rejects_account_mismatch() {
        let report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-351".into()),
            auditor_account: "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ".into(),
            submitted_at_unix: 1_704_361_600,
            evidence: sample_evidence(),
            notes: None,
        };
        let envelope = SignedAuditorRequestV1 {
            version: SIGNED_AUDITOR_REQUEST_VERSION_V1,
            auditor_account: "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ".into(),
            nonce: 7,
            payload: SignedAuditorRequestPayloadV1::RepairReport(report),
            signature: sample_signature(),
        };
        assert!(matches!(
            envelope.validate(),
            Err(RepairValidationError::EnvelopeAccountMismatch { .. })
        ));
    }

    #[test]
    fn worker_signature_payload_validation_succeeds() {
        let payload = RepairWorkerSignaturePayloadV1 {
            version: REPAIR_WORKER_SIGNATURE_VERSION_V1,
            ticket_id: RepairTicketId("REP-500".into()),
            manifest_digest: manifest_digest(),
            provider_id: provider_id(),
            worker_id: "soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ".into(),
            idempotency_key: "claim-500".into(),
            action: RepairWorkerActionV1::Claim {
                claimed_at_unix: 1_704_361_700,
            },
        };
        assert!(payload.validate().is_ok());
    }

    #[test]
    fn worker_signature_payload_rejects_blank_reason() {
        let payload = RepairWorkerSignaturePayloadV1 {
            version: REPAIR_WORKER_SIGNATURE_VERSION_V1,
            ticket_id: RepairTicketId("REP-501".into()),
            manifest_digest: manifest_digest(),
            provider_id: provider_id(),
            worker_id: "soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ".into(),
            idempotency_key: "fail-501".into(),
            action: RepairWorkerActionV1::Fail {
                failed_at_unix: 1_704_361_800,
                reason: " ".into(),
            },
        };
        assert!(matches!(
            payload.validate(),
            Err(RepairValidationError::BlankString { field: "reason" })
        ));
    }

    #[test]
    fn worker_signature_payload_rejects_empty_manifest_digest() {
        let payload = RepairWorkerSignaturePayloadV1 {
            version: REPAIR_WORKER_SIGNATURE_VERSION_V1,
            ticket_id: RepairTicketId("REP-502".into()),
            manifest_digest: [0u8; 32],
            provider_id: provider_id(),
            worker_id: "soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ".into(),
            idempotency_key: "claim-502".into(),
            action: RepairWorkerActionV1::Claim {
                claimed_at_unix: 1_704_361_900,
            },
        };
        assert!(matches!(
            payload.validate(),
            Err(RepairValidationError::EmptyString {
                field: "manifest_digest"
            })
        ));
    }

    #[test]
    fn repair_audit_event_roundtrips() {
        let payload = RepairTaskEventV1 {
            version: REPAIR_TASK_EVENT_VERSION_V1,
            ticket_id: RepairTicketId("REP-600".into()),
            manifest_digest: manifest_digest(),
            provider_id: provider_id(),
            status: RepairTaskStatusV1::Queued,
            occurred_at_unix: 1_704_400_000,
            actor: Some("soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ".into()),
            message: Some("queued".into()),
        };
        let digest = iroha_crypto::Hash::new(payload.encode());
        let header = SorafsAuditHeaderV1 {
            sequence: 7,
            occurred_at_unix: 1_704_400_000,
            signer: "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ".into(),
            payload_digest: *digest.as_ref(),
        };
        let event = RepairAuditEventV1 {
            version: REPAIR_AUDIT_EVENT_VERSION_V1,
            header,
            payload,
        };
        let bytes = event.encode();
        let mut input = bytes.as_slice();
        let decoded = RepairAuditEventV1::decode(&mut input).expect("decode repair audit event");
        assert_eq!(decoded, event);
    }

    #[test]
    fn gc_audit_event_roundtrips() {
        let payload = GcAuditPayloadV1 {
            version: GC_AUDIT_PAYLOAD_VERSION_V1,
            manifest_digest: manifest_digest(),
            provider_id: provider_id(),
            evicted_at_unix: 1_704_400_100,
            freed_bytes: 4_096,
            reason: "retention_expired".into(),
            blocked_reason: None,
        };
        let digest = iroha_crypto::Hash::new(payload.encode());
        let header = SorafsAuditHeaderV1 {
            sequence: 8,
            occurred_at_unix: 1_704_400_100,
            signer: "operator#sora".into(),
            payload_digest: *digest.as_ref(),
        };
        let event = GcAuditEventV1 {
            version: GC_AUDIT_EVENT_VERSION_V1,
            header,
            payload,
        };
        let bytes = event.encode();
        let mut input = bytes.as_slice();
        let decoded = GcAuditEventV1::decode(&mut input).expect("decode gc audit event");
        assert_eq!(decoded, event);
    }
}
