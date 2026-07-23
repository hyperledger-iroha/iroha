//! Repair automation payloads for SoraFS (SF-8b).
//!
//! These types capture auditor-submitted repair reports, scheduler state, and
//! slash proposals emitted when storage providers miss remediation SLAs. All
//! payloads are Norito-encoded so governance, Torii, and tooling can exchange
//! deterministic artefacts without bespoke serializers.

#![allow(clippy::size_of_ref)]

use std::fmt;

use iroha_crypto::{Algorithm, PublicKey};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

use crate::{
    deal::{BASIS_POINTS_PER_UNIT, XorQuantity},
    provider_advert::SignatureAlgorithm,
};

#[cfg(test)]
use iroha_crypto::{Signature, numeric::Quantity};

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
/// Canonical signer label for repair events without an explicit actor.
pub const REPAIR_AUDIT_DEFAULT_SIGNER_V1: &str = "sorafs-repair";
/// Canonical signer label for GC audit events.
pub const GC_AUDIT_SIGNER_V1: &str = "sorafs-gc";
/// GC reason used when an expired manifest has an associated provider.
pub const GC_AUDIT_REASON_RETENTION_EXPIRED_V1: &str = "retention_expired";
/// GC reason used when an expired manifest has no associated provider.
pub const GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1: &str =
    "retention_expired_provider_missing";
/// GC block reason used while a repair task is active.
pub const GC_AUDIT_BLOCKED_REPAIR_ACTIVE_V1: &str = "repair_active";
/// GC block reason used while a storage deal is active.
pub const GC_AUDIT_BLOCKED_DEAL_ACTIVE_V1: &str = "deal_active";
/// GC block reason used while chunks are referenced by another manifest.
pub const GC_AUDIT_BLOCKED_SHARED_CHUNKS_V1: &str = "shared_chunks";
/// Schema version for [`SignedAuditorRequestV1`].
pub const SIGNED_AUDITOR_REQUEST_VERSION_V1: u8 = 1;
/// Schema version for [`RepairWorkerSignaturePayloadV1`].
pub const REPAIR_WORKER_SIGNATURE_VERSION_V1: u8 = 1;

/// Maximum length permitted for ticket identifiers and string fields.
const MAX_STRING_BYTES: usize = 256;
const ED25519_PUBLIC_KEY_LENGTH: usize = 32;
const ED25519_SIGNATURE_LENGTH: usize = 64;

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

/// Proof-of-retrievability failure cause details.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct RepairPorFailureCauseV1 {
    /// PoR challenge identifier (BLAKE3-256 digest).
    pub challenge_id: [u8; 32],
    /// Number of samples that failed validation.
    pub failed_samples: u16,
    /// Optional digest of the offending proof, if available.
    #[norito(default)]
    pub proof_digest: Option<[u8; 32]>,
}

/// Stable PDP failure category used by repair automation and governance archives.
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
#[norito(tag = "kind", content = "value")]
pub enum RepairPdpFailureKindV1 {
    /// The provider did not submit a proof before the governed deadline.
    #[norito(rename = "deadline_expired")]
    DeadlineExpired,
    /// An authenticated provider submission failed PDP binding or witness verification.
    #[norito(rename = "invalid_proof")]
    InvalidProof,
    /// Governance revoked or otherwise removed the provider admission while pending.
    #[norito(rename = "admission_revoked")]
    AdmissionRevoked,
    /// The admitted provider could not read the locally retained payload safely.
    #[norito(rename = "storage_unavailable")]
    StorageUnavailable,
}

/// Proof-of-data-possession failure details handed to the repair scheduler.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct RepairPdpFailureCauseV1 {
    /// PDP challenge identifier (BLAKE3-256 digest).
    pub challenge_id: [u8; 32],
    /// Challenge epoch whose hot replica failed validation.
    pub epoch_id: u64,
    /// Number of challenged hot leaves considered failed.
    pub failed_samples: u16,
    /// Optional digest of the authenticated offending proof.
    #[norito(default)]
    pub proof_digest: Option<[u8; 32]>,
    /// Stable machine-readable failure category.
    pub failure_kind: RepairPdpFailureKindV1,
}

/// Latency SLA breach cause details.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct RepairLatencySlaCauseV1 {
    /// Observed latency in milliseconds.
    pub observed_latency_ms: u32,
    /// Optional digest of the PoTR receipt associated with the breach.
    #[norito(default)]
    pub receipt_digest: Option<[u8; 32]>,
}

/// Replica shortfall cause details.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct RepairReplicaShortfallCauseV1 {
    /// Estimated number of missing chunks.
    pub missing_chunks: u32,
}

/// Manual repair cause details.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct RepairManualCauseV1 {
    /// Free-form description of the trigger.
    pub reason: String,
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
    PorFailure(RepairPorFailureCauseV1),
    /// Proof-of-data-possession failure for a hot replica.
    #[norito(rename = "pdp_failure")]
    PdpFailure(RepairPdpFailureCauseV1),
    /// Latency SLA breach for proof-of-time-to-retrieval sampling.
    #[norito(rename = "latency_sla")]
    LatencySla(RepairLatencySlaCauseV1),
    /// Replica shortfall discovered during sampling.
    #[norito(rename = "replica_shortfall")]
    ReplicaShortfall(RepairReplicaShortfallCauseV1),
    /// Manually triggered remediation (operator supplied reason).
    #[norito(rename = "manual")]
    Manual(RepairManualCauseV1),
}

impl RepairCauseV1 {
    /// Validate the repair cause payload.
    pub fn validate(&self) -> Result<(), RepairValidationError> {
        match self {
            Self::PorFailure(cause) => {
                ensure_digest(&cause.challenge_id, "challenge_id")?;
                if cause.failed_samples == 0 {
                    return Err(RepairValidationError::InvalidSamples);
                }
            }
            Self::PdpFailure(cause) => {
                ensure_digest(&cause.challenge_id, "challenge_id")?;
                if cause.epoch_id == 0 || cause.failed_samples == 0 {
                    return Err(RepairValidationError::InvalidSamples);
                }
            }
            Self::LatencySla(cause) => {
                if cause.observed_latency_ms == 0 {
                    return Err(RepairValidationError::InvalidLatency);
                }
            }
            Self::ReplicaShortfall(cause) => {
                if cause.missing_chunks == 0 {
                    return Err(RepairValidationError::InvalidMissingChunks);
                }
            }
            Self::Manual(cause) => {
                validate_non_empty_string(&cause.reason, "reason")?;
                if cause.reason.len() > MAX_STRING_BYTES {
                    return Err(RepairValidationError::StringTooLong {
                        field: "reason",
                        length: cause.reason.len(),
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

impl RepairAuditEventV1 {
    /// Validate the canonical audit envelope and its payload binding.
    pub fn validate(&self) -> Result<(), RepairValidationError> {
        if self.version != REPAIR_AUDIT_EVENT_VERSION_V1 {
            return Err(RepairValidationError::UnsupportedVersion {
                field: "RepairAuditEventV1",
                version: self.version,
            });
        }
        self.payload.validate()?;
        let expected_signer = self
            .payload
            .actor
            .as_deref()
            .unwrap_or(REPAIR_AUDIT_DEFAULT_SIGNER_V1);
        validate_audit_header(
            &self.header,
            self.payload.occurred_at_unix,
            expected_signer,
            repair_audit_payload_digest_v1(&self.payload)?,
        )
    }
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

impl GcAuditPayloadV1 {
    /// Validate canonical GC reason, provider, and outcome invariants.
    pub fn validate(&self) -> Result<(), RepairValidationError> {
        if self.version != GC_AUDIT_PAYLOAD_VERSION_V1 {
            return Err(RepairValidationError::UnsupportedVersion {
                field: "GcAuditPayloadV1",
                version: self.version,
            });
        }
        ensure_digest(&self.manifest_digest, "manifest_digest")?;
        ensure_timestamp(self.evicted_at_unix, "evicted_at_unix")?;
        validate_optional_string(&self.reason, "reason")?;
        let provider_missing = match self.reason.as_str() {
            GC_AUDIT_REASON_RETENTION_EXPIRED_V1 => false,
            GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1 => true,
            _ => return Err(RepairValidationError::InvalidGcAuditReason),
        };
        if provider_missing != self.provider_id.iter().all(|byte| *byte == 0) {
            return Err(RepairValidationError::InvalidGcAuditProviderBinding);
        }
        if let Some(reason) = self.blocked_reason.as_deref() {
            validate_optional_string(reason, "blocked_reason")?;
            if !matches!(
                reason,
                GC_AUDIT_BLOCKED_REPAIR_ACTIVE_V1
                    | GC_AUDIT_BLOCKED_DEAL_ACTIVE_V1
                    | GC_AUDIT_BLOCKED_SHARED_CHUNKS_V1
            ) {
                return Err(RepairValidationError::InvalidGcAuditBlockedReason);
            }
            if self.freed_bytes != 0 {
                return Err(RepairValidationError::InvalidGcAuditFreedBytes);
            }
        }
        Ok(())
    }
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

impl GcAuditEventV1 {
    /// Validate the canonical audit envelope and its payload binding.
    pub fn validate(&self) -> Result<(), RepairValidationError> {
        if self.version != GC_AUDIT_EVENT_VERSION_V1 {
            return Err(RepairValidationError::UnsupportedVersion {
                field: "GcAuditEventV1",
                version: self.version,
            });
        }
        self.payload.validate()?;
        validate_audit_header(
            &self.header,
            self.payload.evicted_at_unix,
            GC_AUDIT_SIGNER_V1,
            gc_audit_payload_digest_v1(&self.payload)?,
        )
    }
}

/// Compute the canonical header-bearing digest for a repair audit payload.
pub fn repair_audit_payload_digest_v1(
    payload: &RepairTaskEventV1,
) -> Result<[u8; 32], RepairValidationError> {
    canonical_audit_payload_digest("repair task event", payload)
}

/// Compute the canonical header-bearing digest for a GC audit payload.
pub fn gc_audit_payload_digest_v1(
    payload: &GcAuditPayloadV1,
) -> Result<[u8; 32], RepairValidationError> {
    canonical_audit_payload_digest("GC audit payload", payload)
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
    /// Maximum exact XOR-denominated slash penalty allowed for repair escalations.
    pub max_penalty: XorQuantity,
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
        if self.max_penalty.is_zero() {
            return Err(RepairValidationError::InvalidMaxPenalty);
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
    /// Proposed exact XOR-denominated bond penalty.
    pub proposed_penalty: XorQuantity,
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
        if self.proposed_penalty.is_zero() {
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
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
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
        if self.public_key.is_empty() || crate::inert_bytes(&self.public_key) {
            return Err(RepairValidationError::InvalidPublicKey);
        }
        if self.signature.is_empty() || crate::inert_bytes(&self.signature) {
            return Err(RepairValidationError::InvalidSignature);
        }
        Ok(())
    }
}

/// Payloads supported inside a signed auditor request.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
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

/// Canonical payload covered by a [`SignedAuditorRequestV1`] signature.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct SignedAuditorRequestSignaturePayloadV1 {
    /// Schema version (`SIGNED_AUDITOR_REQUEST_VERSION_V1`).
    pub version: u8,
    /// Auditor account submitting the request (I105 string form).
    pub auditor_account: String,
    /// Monotonic nonce used for replay protection.
    pub nonce: u64,
    /// Wrapped payload (`RepairReport` or `RepairSlashProposal`).
    pub payload: SignedAuditorRequestPayloadV1,
}

/// Signed envelope authorising an auditor action.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
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
    /// Return the canonical payload covered by the auditor signature.
    pub fn signature_payload(&self) -> SignedAuditorRequestSignaturePayloadV1 {
        SignedAuditorRequestSignaturePayloadV1 {
            version: self.version,
            auditor_account: self.auditor_account.clone(),
            nonce: self.nonce,
            payload: self.payload.clone(),
        }
    }

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

    /// Verify the auditor signature against the canonical signed payload.
    pub fn verify_signature(&self) -> Result<PublicKey, AuditorSignatureVerificationError> {
        self.validate()?;
        match self.signature.algorithm {
            SignatureAlgorithm::Ed25519 => {}
            other => {
                return Err(AuditorSignatureVerificationError::UnsupportedAlgorithm(
                    other,
                ));
            }
        }

        if self.signature.public_key.len() != ED25519_PUBLIC_KEY_LENGTH {
            return Err(AuditorSignatureVerificationError::InvalidPublicKeyLength {
                length: self.signature.public_key.len(),
            });
        }
        if self.signature.signature.len() != ED25519_SIGNATURE_LENGTH {
            return Err(AuditorSignatureVerificationError::InvalidSignatureLength {
                length: self.signature.signature.len(),
            });
        }

        let public_key_bytes: [u8; ED25519_PUBLIC_KEY_LENGTH] = self
            .signature
            .public_key
            .as_slice()
            .try_into()
            .map_err(|err| AuditorSignatureVerificationError::InvalidPublicKey {
                reason: format!("invalid public key material: {err}"),
            })?;
        crate::checked_ed25519_verifying_key_from_bytes(&public_key_bytes)
            .map_err(|reason| AuditorSignatureVerificationError::InvalidPublicKey { reason })?;
        let signature_bytes: [u8; ED25519_SIGNATURE_LENGTH] = self
            .signature
            .signature
            .as_slice()
            .try_into()
            .map_err(|err| AuditorSignatureVerificationError::Verification {
                reason: format!("invalid signature material: {err}"),
            })?;
        crate::checked_ed25519_signature_from_bytes(&signature_bytes)
            .map_err(|reason| AuditorSignatureVerificationError::Verification { reason })?;
        let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &self.signature.public_key)
            .map_err(|err| AuditorSignatureVerificationError::InvalidPublicKey {
                reason: err.to_string(),
            })?;
        let signature =
            iroha_crypto::ed25519_parse_signature(&self.signature.signature).map_err(|err| {
                AuditorSignatureVerificationError::Verification {
                    reason: format!("invalid signature material: {err}"),
                }
            })?;
        let payload_bytes = norito::to_bytes(&self.signature_payload()).map_err(|err| {
            AuditorSignatureVerificationError::PayloadEncoding {
                reason: err.to_string(),
            }
        })?;
        signature
            .verify(&public_key, &payload_bytes)
            .map_err(|err| AuditorSignatureVerificationError::Verification {
                reason: err.to_string(),
            })?;
        Ok(public_key)
    }
}

/// Errors emitted while verifying signed auditor request signatures.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AuditorSignatureVerificationError {
    /// Structural validation failed before signature verification.
    #[error("invalid signed auditor request: {0}")]
    Validation(#[from] RepairValidationError),
    /// Signature algorithm is not supported by this validator.
    #[error("unsupported auditor signature algorithm: {0:?}")]
    UnsupportedAlgorithm(SignatureAlgorithm),
    /// Ed25519 public key length is invalid.
    #[error("auditor ed25519 public key must be 32 bytes, got {length}")]
    InvalidPublicKeyLength {
        /// Observed public key byte length.
        length: usize,
    },
    /// Ed25519 signature length is invalid.
    #[error("auditor ed25519 signature must be 64 bytes, got {length}")]
    InvalidSignatureLength {
        /// Observed signature byte length.
        length: usize,
    },
    /// Public key bytes could not be parsed.
    #[error("invalid auditor public key: {reason}")]
    InvalidPublicKey {
        /// Parsing failure reason.
        reason: String,
    },
    /// Signature payload could not be encoded into canonical bytes.
    #[error("failed to encode signed auditor request payload for signature verification: {reason}")]
    PayloadEncoding {
        /// Encoding failure reason.
        reason: String,
    },
    /// Signature verification failed.
    #[error("auditor signature verification failed: {reason}")]
    Verification {
        /// Verification failure reason.
        reason: String,
    },
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
    /// Maximum policy penalty must be positive.
    #[error("maximum penalty must be greater than zero")]
    InvalidMaxPenalty,
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
    /// Canonical audit payload encoding failed.
    #[error("failed to encode canonical {payload} audit payload: {reason}")]
    AuditPayloadEncoding {
        /// Logical payload type.
        payload: &'static str,
        /// Norito encoding failure.
        reason: String,
    },
    /// Audit header ordering, timestamp, signer, or digest binding is invalid.
    #[error("invalid audit header: {reason}")]
    InvalidAuditHeader {
        /// Stable rejected invariant.
        reason: &'static str,
    },
    /// GC reason is outside the canonical first-release reason set.
    #[error("GC audit reason is not canonical")]
    InvalidGcAuditReason,
    /// GC blocked reason is outside the canonical first-release reason set.
    #[error("GC audit blocked reason is not canonical")]
    InvalidGcAuditBlockedReason,
    /// GC provider presence does not match the reason label.
    #[error("GC audit provider identifier does not match the reason label")]
    InvalidGcAuditProviderBinding,
    /// A blocked GC outcome reported freed bytes.
    #[error("blocked GC audit outcomes must report zero freed_bytes")]
    InvalidGcAuditFreedBytes,
}

fn canonical_audit_payload_digest<T: norito::core::NoritoSerialize>(
    payload_name: &'static str,
    payload: &T,
) -> Result<[u8; 32], RepairValidationError> {
    let bytes =
        norito::to_bytes(payload).map_err(|err| RepairValidationError::AuditPayloadEncoding {
            payload: payload_name,
            reason: err.to_string(),
        })?;
    Ok(*blake3::hash(&bytes).as_bytes())
}

fn validate_audit_header(
    header: &SorafsAuditHeaderV1,
    expected_timestamp: u64,
    expected_signer: &str,
    expected_payload_digest: [u8; 32],
) -> Result<(), RepairValidationError> {
    if header.sequence == 0 {
        return Err(RepairValidationError::InvalidAuditHeader {
            reason: "sequence must be non-zero",
        });
    }
    ensure_timestamp(header.occurred_at_unix, "audit occurred_at_unix")?;
    validate_optional_string(&header.signer, "audit signer")?;
    if header.occurred_at_unix != expected_timestamp {
        return Err(RepairValidationError::InvalidAuditHeader {
            reason: "timestamp does not match payload",
        });
    }
    if header.signer != expected_signer {
        return Err(RepairValidationError::InvalidAuditHeader {
            reason: "signer does not match payload actor",
        });
    }
    if header.payload_digest != expected_payload_digest {
        return Err(RepairValidationError::InvalidAuditHeader {
            reason: "payload digest mismatch",
        });
    }
    Ok(())
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

    const SMALL_ORDER_R: [u8; ED25519_PUBLIC_KEY_LENGTH] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const NONCANONICAL_R: [u8; ED25519_PUBLIC_KEY_LENGTH] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

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
            cause: RepairCauseV1::PorFailure(RepairPorFailureCauseV1 {
                challenge_id: [0xCC; 32],
                failed_samples: 4,
                proof_digest: None,
            }),
            evidence_json: None,
            notes: Some("provider reported disk failure".into()),
        }
    }

    fn sample_escalation_policy(max_penalty: &str) -> RepairEscalationPolicyV1 {
        RepairEscalationPolicyV1 {
            version: REPAIR_ESCALATION_POLICY_VERSION_V1,
            quorum_bps: 6_667,
            minimum_voters: 3,
            dispute_window_secs: 86_400,
            appeal_window_secs: 604_800,
            max_penalty: max_penalty.parse().expect("canonical XOR maximum penalty"),
        }
    }

    fn sample_slash_proposal(proposed_penalty: &str) -> RepairSlashProposalV1 {
        RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: RepairTicketId("REP-351".into()),
            provider_id: provider_id(),
            manifest_digest: manifest_digest(),
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            proposed_penalty: proposed_penalty
                .parse()
                .expect("canonical XOR proposed penalty"),
            submitted_at_unix: 1_704_361_600,
            rationale: "Repeated PoR failures beyond SLA".into(),
            approval: None,
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
    fn evidence_norito_roundtrips() {
        let evidence = sample_evidence();
        let bytes = norito::to_bytes(&evidence).expect("encode evidence");
        let decoded: RepairEvidenceV1 = norito::decode_from_bytes(&bytes).expect("decode evidence");
        assert_eq!(decoded, evidence);
    }

    #[test]
    fn pdp_failure_evidence_is_typed_bounded_and_roundtrips() {
        let evidence = RepairEvidenceV1 {
            version: REPAIR_EVIDENCE_VERSION_V1,
            manifest_digest: manifest_digest(),
            provider_id: provider_id(),
            por_history_id: None,
            cause: RepairCauseV1::PdpFailure(RepairPdpFailureCauseV1 {
                challenge_id: [0xCD; 32],
                epoch_id: 7,
                failed_samples: 4,
                proof_digest: Some([0xCE; 32]),
                failure_kind: RepairPdpFailureKindV1::InvalidProof,
            }),
            evidence_json: None,
            notes: Some("pdp_failure".to_owned()),
        };
        evidence.validate().expect("valid PDP failure evidence");
        let encoded = norito::to_bytes(&evidence).expect("encode PDP failure evidence");
        let decoded: RepairEvidenceV1 =
            norito::decode_from_bytes(&encoded).expect("decode PDP failure evidence");
        assert_eq!(decoded, evidence);

        for (index, mut invalid) in [evidence.clone(), evidence.clone()].into_iter().enumerate() {
            let RepairCauseV1::PdpFailure(cause) = &mut invalid.cause else {
                unreachable!();
            };
            if index == 0 {
                cause.epoch_id = 0;
            } else {
                cause.failed_samples = 0;
            }
            assert!(invalid.validate().is_err());
        }
    }

    #[test]
    fn report_validation_succeeds() {
        let report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-351".into()),
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
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
            repair_agent: Some("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D".into()),
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
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
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
        let proposal = sample_slash_proposal("1000000000");
        assert!(proposal.validate().is_ok());
    }

    #[test]
    fn repair_escalation_xor_penalties_roundtrip_exactly() {
        let maximum = "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824.503042047";
        let policy = sample_escalation_policy(maximum);
        let proposal = sample_slash_proposal("340282366920938463463374607431768211456.000000001");

        let policy_bytes = norito::to_bytes(&policy).expect("encode escalation policy");
        let decoded_policy: RepairEscalationPolicyV1 =
            norito::decode_from_bytes(&policy_bytes).expect("decode escalation policy");
        assert_eq!(decoded_policy, policy);

        let proposal_bytes = norito::to_bytes(&proposal).expect("encode slash proposal");
        let decoded_proposal: RepairSlashProposalV1 =
            norito::decode_from_bytes(&proposal_bytes).expect("decode slash proposal");
        assert_eq!(decoded_proposal, proposal);

        let policy_json = norito::json::to_string(&policy).expect("encode escalation policy JSON");
        assert!(policy_json.contains(&format!("\"max_penalty\":\"{maximum}\"")));
        let decoded_policy: RepairEscalationPolicyV1 =
            norito::json::from_str(&policy_json).expect("decode escalation policy JSON");
        assert_eq!(decoded_policy, policy);

        let proposal_json = norito::json::to_string(&proposal).expect("encode slash proposal JSON");
        assert!(proposal_json.contains(
            "\"proposed_penalty\":\"340282366920938463463374607431768211456.000000001\""
        ));
        let decoded_proposal: RepairSlashProposalV1 =
            norito::json::from_str(&proposal_json).expect("decode slash proposal JSON");
        assert_eq!(decoded_proposal, proposal);
    }

    #[test]
    fn repair_escalation_xor_penalty_json_rejects_adversarial_values() {
        let policy_json =
            norito::json::to_string(&sample_escalation_policy("1")).expect("encode policy JSON");
        let proposal_json =
            norito::json::to_string(&sample_slash_proposal("1")).expect("encode proposal JSON");
        let overflow = "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042048";
        let invalid_json_values = [
            "\"0.0000000001\"".to_owned(),
            "\"-1\"".to_owned(),
            "\"01\"".to_owned(),
            "\"1.0\"".to_owned(),
            "\"+1\"".to_owned(),
            "\"1e3\"".to_owned(),
            "\"not-a-quantity\"".to_owned(),
            "1".to_owned(),
            format!("\"{overflow}\""),
        ];

        for invalid in invalid_json_values {
            let forged_policy = policy_json.replace(
                "\"max_penalty\":\"1\"",
                &format!("\"max_penalty\":{invalid}"),
            );
            assert_ne!(forged_policy, policy_json, "policy fixture must be mutated");
            assert!(
                norito::json::from_str::<RepairEscalationPolicyV1>(&forged_policy).is_err(),
                "policy accepted adversarial max_penalty {invalid}"
            );

            let forged_proposal = proposal_json.replace(
                "\"proposed_penalty\":\"1\"",
                &format!("\"proposed_penalty\":{invalid}"),
            );
            assert_ne!(
                forged_proposal, proposal_json,
                "proposal fixture must be mutated"
            );
            assert!(
                norito::json::from_str::<RepairSlashProposalV1>(&forged_proposal).is_err(),
                "proposal accepted adversarial proposed_penalty {invalid}"
            );
        }
    }

    #[derive(NoritoSerialize)]
    struct RawQuantityEscalationPolicyV1 {
        version: u8,
        quorum_bps: u16,
        minimum_voters: u32,
        dispute_window_secs: u64,
        appeal_window_secs: u64,
        max_penalty: Quantity,
    }

    #[derive(NoritoSerialize)]
    struct RawQuantitySlashProposalV1 {
        version: u8,
        ticket_id: RepairTicketId,
        provider_id: [u8; 32],
        manifest_digest: [u8; 32],
        auditor_account: String,
        proposed_penalty: Quantity,
        submitted_at_unix: u64,
        rationale: String,
        approval: Option<RepairEscalationApprovalV1>,
    }

    #[test]
    fn repair_escalation_norito_rejects_generic_scale_ten_quantities() {
        let too_precise: Quantity = "0.0000000001"
            .parse()
            .expect("generic quantity permits scale ten");
        let forged_policy = RawQuantityEscalationPolicyV1 {
            version: REPAIR_ESCALATION_POLICY_VERSION_V1,
            quorum_bps: 6_667,
            minimum_voters: 3,
            dispute_window_secs: 86_400,
            appeal_window_secs: 604_800,
            max_penalty: too_precise.clone(),
        };
        let policy_bytes = norito::to_bytes(&forged_policy).expect("encode raw quantity policy");
        assert!(norito::decode_from_bytes::<RepairEscalationPolicyV1>(&policy_bytes).is_err());

        let forged_proposal = RawQuantitySlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: RepairTicketId("REP-351".into()),
            provider_id: provider_id(),
            manifest_digest: manifest_digest(),
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            proposed_penalty: too_precise,
            submitted_at_unix: 1_704_361_600,
            rationale: "Repeated PoR failures beyond SLA".into(),
            approval: None,
        };
        let proposal_bytes =
            norito::to_bytes(&forged_proposal).expect("encode raw quantity slash proposal");
        assert!(norito::decode_from_bytes::<RepairSlashProposalV1>(&proposal_bytes).is_err());
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
        let policy = sample_escalation_policy("1000");
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn escalation_policy_validation_rejects_zero_maximum_penalty() {
        let policy = RepairEscalationPolicyV1 {
            version: REPAIR_ESCALATION_POLICY_VERSION_V1,
            quorum_bps: 6_667,
            minimum_voters: 3,
            dispute_window_secs: 86_400,
            appeal_window_secs: 604_800,
            max_penalty: XorQuantity::zero(),
        };
        assert_eq!(
            policy.validate(),
            Err(RepairValidationError::InvalidMaxPenalty)
        );
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

    fn checked_auditor_keypair() -> iroha_crypto::KeyPair {
        iroha_crypto::KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
            .expect("generate checked auditor fixture keypair")
    }

    fn repair_report_with_auditor(ticket: &str, auditor_account: &str) -> RepairReportV1 {
        RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId(ticket.into()),
            auditor_account: auditor_account.into(),
            submitted_at_unix: 1_704_361_600,
            evidence: sample_evidence(),
            notes: None,
        }
    }

    fn signed_auditor_report(
        key_pair: &iroha_crypto::KeyPair,
        ticket: &str,
    ) -> SignedAuditorRequestV1 {
        let public_key = {
            let (algorithm, public_key) = key_pair.public_key().to_bytes();
            assert_eq!(algorithm, Algorithm::Ed25519);
            public_key.to_vec()
        };
        let report = repair_report_with_auditor(ticket, "auditor-1");
        let mut envelope = SignedAuditorRequestV1 {
            version: SIGNED_AUDITOR_REQUEST_VERSION_V1,
            auditor_account: report.auditor_account.clone(),
            nonce: 42,
            payload: SignedAuditorRequestPayloadV1::RepairReport(report),
            signature: AuditorSignatureV1 {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key,
                signature: Vec::new(),
            },
        };
        let payload_bytes =
            norito::to_bytes(&envelope.signature_payload()).expect("encode auditor payload");
        let signature = Signature::try_new(key_pair.private_key(), &payload_bytes)
            .expect("sign auditor payload");
        envelope.signature.signature = signature.payload().to_vec();
        envelope
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
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            submitted_at_unix: 1_704_361_600,
            evidence: sample_evidence(),
            notes: None,
        };
        let envelope = SignedAuditorRequestV1 {
            version: SIGNED_AUDITOR_REQUEST_VERSION_V1,
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            nonce: 42,
            payload: SignedAuditorRequestPayloadV1::RepairReport(report),
            signature: sample_signature(),
        };
        assert!(envelope.validate().is_ok());
    }

    #[test]
    fn signed_auditor_request_json_roundtrip_succeeds() {
        let report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-352".into()),
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            submitted_at_unix: 1_704_361_601,
            evidence: sample_evidence(),
            notes: None,
        };
        let envelope = SignedAuditorRequestV1 {
            version: SIGNED_AUDITOR_REQUEST_VERSION_V1,
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            nonce: 43,
            payload: SignedAuditorRequestPayloadV1::RepairReport(report),
            signature: sample_signature(),
        };

        let json = norito::json::to_vec(&envelope).expect("encode signed auditor request json");
        let decoded: SignedAuditorRequestV1 =
            norito::json::from_slice(&json).expect("decode signed auditor request json");

        assert_eq!(decoded, envelope);
        assert!(decoded.validate().is_ok());
    }

    #[test]
    fn signed_auditor_request_rejects_account_mismatch() {
        let report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-351".into()),
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            submitted_at_unix: 1_704_361_600,
            evidence: sample_evidence(),
            notes: None,
        };
        let envelope = SignedAuditorRequestV1 {
            version: SIGNED_AUDITOR_REQUEST_VERSION_V1,
            auditor_account: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D".into(),
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
    fn signed_auditor_request_signature_verifies() {
        let key_pair = checked_auditor_keypair();
        let envelope = signed_auditor_report(&key_pair, "REP-360");
        let public_key = envelope
            .verify_signature()
            .expect("signed auditor request verifies");

        assert_eq!(&public_key, key_pair.public_key());
    }

    #[test]
    fn signed_auditor_request_signature_rejects_tampered_payload() {
        let key_pair = checked_auditor_keypair();
        let mut envelope = signed_auditor_report(&key_pair, "REP-361");
        if let SignedAuditorRequestPayloadV1::RepairReport(report) = &mut envelope.payload {
            report.notes = Some("tampered".into());
        }

        assert!(matches!(
            envelope.verify_signature(),
            Err(AuditorSignatureVerificationError::Verification { .. })
        ));
    }

    #[test]
    fn signed_auditor_request_signature_rejects_tampered_signature() {
        let key_pair = checked_auditor_keypair();
        let mut envelope = signed_auditor_report(&key_pair, "REP-362");
        envelope.signature.signature[0] ^= 0x01;

        assert!(matches!(
            envelope.verify_signature(),
            Err(AuditorSignatureVerificationError::Verification { .. })
        ));
    }

    #[test]
    fn signed_auditor_request_signature_rejects_all_zero_signature_material() {
        let key_pair = checked_auditor_keypair();
        let mut envelope = signed_auditor_report(&key_pair, "REP-366");
        envelope.signature.signature.fill(0);

        let err = envelope
            .verify_signature()
            .expect_err("all-zero signature material must fail closed");
        assert!(
            matches!(
                err,
                AuditorSignatureVerificationError::Validation(
                    RepairValidationError::InvalidSignature
                )
            ),
            "{err:?}"
        );
    }

    #[test]
    fn signed_auditor_request_signature_rejects_malformed_signature_r() {
        let key_pair = checked_auditor_keypair();

        for (label, replacement_r, expected_reason) in [
            ("small-order", SMALL_ORDER_R, "small-order"),
            ("noncanonical", NONCANONICAL_R, "not a canonical"),
        ] {
            let mut envelope = signed_auditor_report(&key_pair, "REP-367");
            envelope.signature.signature[..ED25519_PUBLIC_KEY_LENGTH]
                .copy_from_slice(&replacement_r);

            let err = envelope
                .verify_signature()
                .expect_err("malformed signature R must fail closed");
            assert!(
                matches!(
                    &err,
                    AuditorSignatureVerificationError::Verification { reason }
                        if reason.contains(expected_reason)
                ),
                "{label} signature R produced unexpected error: {err:?}"
            );
        }
    }

    #[test]
    fn signed_auditor_request_signature_rejects_unsupported_algorithm() {
        let key_pair = checked_auditor_keypair();
        let mut envelope = signed_auditor_report(&key_pair, "REP-363");
        envelope.signature.algorithm = SignatureAlgorithm::MultiSig;

        assert!(matches!(
            envelope.verify_signature(),
            Err(AuditorSignatureVerificationError::UnsupportedAlgorithm(
                SignatureAlgorithm::MultiSig
            ))
        ));
    }

    #[test]
    fn signed_auditor_request_signature_rejects_invalid_lengths() {
        let key_pair = checked_auditor_keypair();
        let mut envelope = signed_auditor_report(&key_pair, "REP-364");
        envelope.signature.public_key.pop();
        assert!(matches!(
            envelope.verify_signature(),
            Err(AuditorSignatureVerificationError::InvalidPublicKeyLength { .. })
        ));

        let mut envelope = signed_auditor_report(&key_pair, "REP-365");
        envelope.signature.signature.pop();
        assert!(matches!(
            envelope.verify_signature(),
            Err(AuditorSignatureVerificationError::InvalidSignatureLength { .. })
        ));
    }

    #[test]
    fn worker_signature_payload_validation_succeeds() {
        let payload = RepairWorkerSignaturePayloadV1 {
            version: REPAIR_WORKER_SIGNATURE_VERSION_V1,
            ticket_id: RepairTicketId("REP-500".into()),
            manifest_digest: manifest_digest(),
            provider_id: provider_id(),
            worker_id: "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE".into(),
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
            worker_id: "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE".into(),
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
            worker_id: "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE".into(),
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
        let actor = "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE";
        let payload = RepairTaskEventV1 {
            version: REPAIR_TASK_EVENT_VERSION_V1,
            ticket_id: RepairTicketId("REP-600".into()),
            manifest_digest: manifest_digest(),
            provider_id: provider_id(),
            status: RepairTaskStatusV1::Queued,
            occurred_at_unix: 1_704_400_000,
            actor: Some(actor.into()),
            message: Some("queued".into()),
        };
        let header = SorafsAuditHeaderV1 {
            sequence: 7,
            occurred_at_unix: 1_704_400_000,
            signer: actor.into(),
            payload_digest: repair_audit_payload_digest_v1(&payload).expect("audit digest"),
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
        decoded.validate().expect("validate repair audit event");
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
        let header = SorafsAuditHeaderV1 {
            sequence: 8,
            occurred_at_unix: 1_704_400_100,
            signer: GC_AUDIT_SIGNER_V1.into(),
            payload_digest: gc_audit_payload_digest_v1(&payload).expect("audit digest"),
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
        decoded.validate().expect("validate GC audit event");
    }

    #[test]
    fn repair_audit_event_rejects_header_tampering() {
        let payload = RepairTaskEventV1 {
            version: REPAIR_TASK_EVENT_VERSION_V1,
            ticket_id: RepairTicketId("REP-601".into()),
            manifest_digest: manifest_digest(),
            provider_id: provider_id(),
            status: RepairTaskStatusV1::Queued,
            occurred_at_unix: 1_704_400_001,
            actor: None,
            message: None,
        };
        let event = RepairAuditEventV1 {
            version: REPAIR_AUDIT_EVENT_VERSION_V1,
            header: SorafsAuditHeaderV1 {
                sequence: 1,
                occurred_at_unix: payload.occurred_at_unix,
                signer: REPAIR_AUDIT_DEFAULT_SIGNER_V1.into(),
                payload_digest: repair_audit_payload_digest_v1(&payload).expect("audit digest"),
            },
            payload,
        };
        event.validate().expect("valid audit event");

        let mut zero_sequence = event.clone();
        zero_sequence.header.sequence = 0;
        assert!(matches!(
            zero_sequence.validate(),
            Err(RepairValidationError::InvalidAuditHeader { .. })
        ));
        let mut wrong_version = event.clone();
        wrong_version.version = 2;
        assert!(matches!(
            wrong_version.validate(),
            Err(RepairValidationError::UnsupportedVersion {
                field: "RepairAuditEventV1",
                version: 2
            })
        ));
        let mut wrong_timestamp = event.clone();
        wrong_timestamp.header.occurred_at_unix += 1;
        assert!(matches!(
            wrong_timestamp.validate(),
            Err(RepairValidationError::InvalidAuditHeader { .. })
        ));
        let mut wrong_signer = event.clone();
        wrong_signer.header.signer = "wrong-signer".into();
        assert!(matches!(
            wrong_signer.validate(),
            Err(RepairValidationError::InvalidAuditHeader { .. })
        ));
        let mut wrong_digest = event;
        wrong_digest.header.payload_digest[0] ^= 0x80;
        assert!(matches!(
            wrong_digest.validate(),
            Err(RepairValidationError::InvalidAuditHeader { .. })
        ));
    }

    #[test]
    fn gc_audit_payload_rejects_reason_provider_and_outcome_drift() {
        let valid = GcAuditPayloadV1 {
            version: GC_AUDIT_PAYLOAD_VERSION_V1,
            manifest_digest: manifest_digest(),
            provider_id: provider_id(),
            evicted_at_unix: 1_704_400_100,
            freed_bytes: 4_096,
            reason: GC_AUDIT_REASON_RETENTION_EXPIRED_V1.into(),
            blocked_reason: None,
        };
        valid.validate().expect("valid GC payload");

        let mut unknown_reason = valid.clone();
        unknown_reason.reason = "operator_override".into();
        assert_eq!(
            unknown_reason.validate(),
            Err(RepairValidationError::InvalidGcAuditReason)
        );
        let mut missing_provider = valid.clone();
        missing_provider.provider_id = [0; 32];
        assert_eq!(
            missing_provider.validate(),
            Err(RepairValidationError::InvalidGcAuditProviderBinding)
        );
        let mut unexpected_provider = valid.clone();
        unexpected_provider.reason = GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1.into();
        assert_eq!(
            unexpected_provider.validate(),
            Err(RepairValidationError::InvalidGcAuditProviderBinding)
        );
        let mut unknown_blocked_reason = valid.clone();
        unknown_blocked_reason.freed_bytes = 0;
        unknown_blocked_reason.blocked_reason = Some("operator_hold".into());
        assert_eq!(
            unknown_blocked_reason.validate(),
            Err(RepairValidationError::InvalidGcAuditBlockedReason)
        );
        let mut blocked_with_freed_bytes = valid.clone();
        blocked_with_freed_bytes.blocked_reason = Some(GC_AUDIT_BLOCKED_REPAIR_ACTIVE_V1.into());
        assert_eq!(
            blocked_with_freed_bytes.validate(),
            Err(RepairValidationError::InvalidGcAuditFreedBytes)
        );
        let mut zero_byte_eviction = valid;
        zero_byte_eviction.freed_bytes = 0;
        zero_byte_eviction
            .validate()
            .expect("empty manifests may be evicted without freeing payload bytes");
    }

    #[test]
    fn gc_audit_event_rejects_header_tampering() {
        let payload = GcAuditPayloadV1 {
            version: GC_AUDIT_PAYLOAD_VERSION_V1,
            manifest_digest: manifest_digest(),
            provider_id: provider_id(),
            evicted_at_unix: 1_704_400_200,
            freed_bytes: 1,
            reason: GC_AUDIT_REASON_RETENTION_EXPIRED_V1.into(),
            blocked_reason: None,
        };
        let event = GcAuditEventV1 {
            version: GC_AUDIT_EVENT_VERSION_V1,
            header: SorafsAuditHeaderV1 {
                sequence: 9,
                occurred_at_unix: payload.evicted_at_unix,
                signer: GC_AUDIT_SIGNER_V1.into(),
                payload_digest: gc_audit_payload_digest_v1(&payload).expect("audit digest"),
            },
            payload,
        };
        event.validate().expect("valid GC audit event");

        let mut wrong_version = event.clone();
        wrong_version.version = 2;
        assert!(matches!(
            wrong_version.validate(),
            Err(RepairValidationError::UnsupportedVersion {
                field: "GcAuditEventV1",
                version: 2
            })
        ));
        let mut wrong_timestamp = event.clone();
        wrong_timestamp.header.occurred_at_unix += 1;
        assert!(matches!(
            wrong_timestamp.validate(),
            Err(RepairValidationError::InvalidAuditHeader { .. })
        ));
        let mut wrong_signer = event.clone();
        wrong_signer.header.signer = REPAIR_AUDIT_DEFAULT_SIGNER_V1.into();
        assert!(matches!(
            wrong_signer.validate(),
            Err(RepairValidationError::InvalidAuditHeader { .. })
        ));
        let mut wrong_digest = event;
        wrong_digest.header.payload_digest[31] ^= 1;
        assert!(matches!(
            wrong_digest.validate(),
            Err(RepairValidationError::InvalidAuditHeader { .. })
        ));
    }
}
